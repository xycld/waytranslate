use clap::{Parser, Subcommand};

mod config;
mod doctor;
mod i18n;
mod pointer;
mod popup;
mod settings;
mod translate;
mod tray;
mod wayland;

#[derive(Parser)]
#[command(
    name = "waytranslate",
    about = "Select text anywhere on Wayland, get an inline translation popup"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon (default)
    Run,
    /// Probe this session's Wayland capabilities and report compatibility
    Doctor {
        #[command(subcommand)]
        action: Option<DoctorAction>,
    },
    /// Print primary-selection changes live (debug)
    Watch,
    /// Print pointer position + button events live (debug)
    Pointer {
        /// Primary output size as WxH (for coordinate scaling)
        #[arg(long, default_value = "2560x1440")]
        screen: String,
    },
    /// Translate text once and print it (debug / scripting)
    Translate {
        text: String,
        /// Backend id (see config sample; default google)
        #[arg(long, default_value = "google")]
        backend: String,
    },
    /// Print config path and the fully-commented sample config
    ConfigSample,
    /// Open the settings window (into the running daemon if present)
    Settings,
}

#[derive(Subcommand)]
enum DoctorAction {
    /// Auto-fix what's locally fixable (broken config, input group)
    Fix,
}

fn main() -> anyhow::Result<()> {
    i18n::init();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "waytranslate=info".into()),
        )
        .init();

    match Cli::parse().command.unwrap_or(Command::Run) {
        Command::Doctor { action } => match action {
            None => doctor::run(),
            Some(DoctorAction::Fix) => doctor::fix(),
        },
        Command::Watch => watch(),
        Command::Pointer { screen } => pointer_debug(&screen),
        Command::Translate { text, backend } => translate_once(&text, &backend),
        Command::ConfigSample => {
            println!("# path: {}", config::path().display());
            println!("{}", config::SAMPLE);
            Ok(())
        }
        Command::Run | Command::Settings => daemon::run(),
    }
}

fn watch() -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let (rx, _cmd) = wayland::spawn()?;
        println!("watching primary selection — select text anywhere (Ctrl+C to quit)");
        while let Ok(ev) = rx.recv().await {
            match ev {
                wayland::SelectionEvent::PrimaryChanged(text) => {
                    let preview: String = text.chars().take(80).collect();
                    println!("[{} chars] {preview:?}", text.chars().count());
                }
            }
        }
        Ok(())
    })
}

fn pointer_debug(screen: &str) -> anyhow::Result<()> {
    let (w, h) = screen
        .split_once('x')
        .and_then(|(w, h)| w.parse::<f64>().ok().zip(h.parse::<f64>().ok()))
        .ok_or_else(|| anyhow::anyhow!("--screen must be WxH, got {screen:?}"))?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let (tracker, mut events) = pointer::spawn((w, h))?;
        println!("tracking pointer (Ctrl+C to quit)");
        loop {
            tokio::select! {
                Some(ev) = events.recv() => {
                    let p = tracker.position();
                    println!("{ev:?} @ ({:.0}, {:.0})", p.x, p.y);
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    let p = tracker.position();
                    println!("pos ({:.0}, {:.0})", p.x, p.y);
                }
            }
        }
    })
}

fn translate_once(text: &str, backend: &str) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let mut cfg = config::load()?;
        cfg.backend = backend.into(); // CLI flag overrides config
        let backend = translate::make_backend(&cfg)?;
        let to = cfg
            .target_lang()
            .unwrap_or_else(|| translate::auto_direction(text));
        let out = backend.translate(text, to).await?;
        println!("{out}");
        Ok(())
    })
}

mod daemon;
