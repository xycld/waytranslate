//! Wayland plumbing on our own connection (separate from GTK's):
//! - ext-data-control: watch primary selection, own the clipboard on demand
//! - event loop: calloop (Wayland source + command channel in one loop)
//!
//! Offer reading is roundtrip-free: receive + flush, then poll the pipe —
//! the source process writes from its own loop, we just wait for bytes.
//! Clipboard ownership: after `SetClipboard` this connection must stay alive
//! and answer `send` events, which is why the source lives in this loop.

use std::io::{Read, Write};
use std::os::fd::AsFd;

use calloop::channel::Event as ChannelEvent;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, globals::registry_queue_init};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};

#[derive(Debug, Clone)]
pub enum SelectionEvent {
    /// Primary selection changed to a new text (may be empty after deselect).
    PrimaryChanged(String),
}

/// Cheap, clonable, callable from any thread (GTK callbacks included).
pub type CmdSender = calloop::channel::Sender<Cmd>;

#[derive(Debug)]
pub enum Cmd {
    /// Own the system clipboard with this text (no focus needed).
    SetClipboard(String),
}

struct Watcher {
    conn: Connection,
    qh: QueueHandle<Watcher>,
    manager: ExtDataControlManagerV1,
    device: ExtDataControlDeviceV1,
    tx: async_channel::Sender<SelectionEvent>,
    clipboard_text: Option<String>,
    clipboard_source: Option<ExtDataControlSourceV1>,
}

/// Spawn the `Wayland` loop thread. Returns (selection events, command sender).
pub fn spawn() -> anyhow::Result<(async_channel::Receiver<SelectionEvent>, CmdSender)> {
    let (ev_tx, ev_rx) = async_channel::unbounded();

    // Connect on the calling thread so a missing WAYLAND_DISPLAY fails loudly
    // here instead of inside the thread.
    let conn = Connection::connect_to_env()?;

    // calloop channel is created here so the sender can leave with us.
    let (cmd_tx, cmd_source) = calloop::channel::channel::<Cmd>();

    std::thread::Builder::new()
        .name("wayland-loop".into())
        .spawn(move || {
            if let Err(e) = run(conn, ev_tx, cmd_source) {
                tracing::error!("wayland loop died: {e:#}");
            }
        })?;

    Ok((ev_rx, cmd_tx))
}

fn run(
    conn: Connection,
    tx: async_channel::Sender<SelectionEvent>,
    cmd_source: calloop::channel::Channel<Cmd>,
) -> anyhow::Result<()> {
    let (globals, queue) = registry_queue_init::<Watcher>(&conn)?;
    let qh = queue.handle();

    let manager: ExtDataControlManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=7, ())?;
    let device = manager.get_data_device(&seat, &qh, ());

    let mut state = Watcher {
        conn: conn.clone(),
        qh,
        manager,
        device,
        tx,
        clipboard_text: None,
        clipboard_source: None,
    };

    let mut event_loop: calloop::EventLoop<Watcher> = calloop::EventLoop::try_new()?;
    let handle = event_loop.handle();

    calloop_wayland_source::WaylandSource::new(conn, queue).insert(handle.clone())?;

    // InsertError<Channel<Cmd>> is !Sync (mpsc inside), so no `?` here.
    if handle
        .insert_source(cmd_source, |event, _, state| {
            if let ChannelEvent::Msg(cmd) = event {
                state.handle_cmd(cmd);
            }
        })
        .is_err()
    {
        anyhow::bail!("failed to register command channel");
    }

    tracing::info!("wayland loop up");
    event_loop.run(None, &mut state, |_| {})?;
    Ok(())
}

impl Watcher {
    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::SetClipboard(text) => {
                if let Some(old) = self.clipboard_source.take() {
                    old.destroy();
                }
                let source = self.manager.create_data_source(&self.qh, ());
                source.offer("text/plain;charset=utf-8".into());
                source.offer("text/plain".into());
                source.offer("UTF8_STRING".into());
                self.device.set_selection(Some(&source));
                self.clipboard_text = Some(text);
                self.clipboard_source = Some(source);
                let _ = self.conn.flush();
                tracing::debug!("clipboard owned via data-control");
            }
        }
    }
}

/// Pull text out of an offer without a nested roundtrip:
/// request text/plain, flush, then read to EOF — the source process
/// answers from its own event loop and closes the pipe when done.
fn read_offer(conn: &Connection, offer: &ExtDataControlOfferV1) -> anyhow::Result<String> {
    let (read_fd, write_fd) = rustix::pipe::pipe()?;
    offer.receive("text/plain;charset=utf-8".into(), write_fd.as_fd());
    drop(write_fd);
    conn.flush()?;

    // A misbehaving source must not hang our event loop: wait at most 500ms
    // for bytes, then give up on this offer.
    let mut fds = [rustix::event::PollFd::new(
        &read_fd,
        rustix::event::PollFlags::IN,
    )];
    let timeout = rustix::time::Timespec {
        tv_sec: 0,
        tv_nsec: 500_000_000,
    };
    let ready = rustix::event::poll(&mut fds, Some(&timeout))?;
    anyhow::ensure!(ready > 0, "selection source timed out");

    let mut buf = String::new();
    std::fs::File::from(read_fd).read_to_string(&mut buf)?;
    Ok(buf)
}

impl Dispatch<wl_registry::WlRegistry, wayland_client::globals::GlobalListContents> for Watcher {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// seat sends capabilities on bind; genuinely ignore
impl Dispatch<wl_seat::WlSeat, ()> for Watcher {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

wayland_client::delegate_noop!(Watcher: ExtDataControlManagerV1);

// offers advertise mime types via events; we always ask for text/plain
impl Dispatch<ExtDataControlOfferV1, ()> for Watcher {
    fn event(
        _: &mut Self,
        _: &ExtDataControlOfferV1,
        _: wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_data_control_device_v1::Event as E;
        match event {
            E::PrimarySelection { id } => {
                if let Some(offer) = id {
                    match read_offer(&state.conn, &offer) {
                        Ok(text) => {
                            let _ = state.tx.send_blocking(SelectionEvent::PrimaryChanged(text));
                        }
                        Err(e) => tracing::warn!("failed to read primary selection: {e:#}"),
                    }
                    offer.destroy();
                } else {
                    let _ = state
                        .tx
                        .send_blocking(SelectionEvent::PrimaryChanged(String::new()));
                }
            }
            // Clipboard (Ctrl+C) is not a trigger path; drop the offer.
            E::Selection { id: Some(offer) } => offer.destroy(),
            E::Selection { id: None } => {}
            _ => {}
        }
    }

    // Events data_offer (0), selection (1), primary_selection (3) create
    // offer objects; the macro supplies `event_created_child` for this impl.
    wayland_client::event_created_child!(Self, ExtDataControlDeviceV1, [
        0 => (ExtDataControlOfferV1, ()),
        1 => (ExtDataControlOfferV1, ()),
        3 => (ExtDataControlOfferV1, ()),
    ]);
}

// Our clipboard source: answer paste requests with the stored text.
impl Dispatch<ExtDataControlSourceV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        source: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_data_control_source_v1::Event as E;
        match event {
            E::Send { mime_type, fd } => {
                tracing::debug!("clipboard send requested ({mime_type})");
                if let Some(text) = &state.clipboard_text {
                    let _ = std::fs::File::from(fd).write_all(text.as_bytes());
                }
            }
            E::Cancelled => {
                source.destroy();
                state.clipboard_source = None;
                state.clipboard_text = None;
            }
            _ => {}
        }
    }
}
