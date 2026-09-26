//! Lucide-style 16px inline SVG icons, colors baked
//! (librsvg renders currentColor as black).

pub(super) const GRAY: &str = "#cdd3d9";
pub(super) const ACCENT: &str = "#3daee9";

fn svg_doc(body: &str, stroke: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="{stroke}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
    )
}

pub(super) fn icon_svg(name: &str, stroke: &str) -> String {
    let body = match name {
        "copy" => {
            r#"<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>"#
        }
        "check" => r#"<path d="M20 6 9 17l-5-5"/>"#,
        "pin" => {
            r#"<path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1z"/>"#
        }
        "close" => r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#,
        "languages" => {
            r#"<path d="m5 8 6 6"/><path d="m4 14 6-6 2-3"/><path d="M2 5h12"/><path d="M7 2h1"/><path d="m22 22-5-10-5 10"/><path d="M14 18h6"/>"#
        }
        "alert" => {
            r#"<circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>"#
        }
        _ => "",
    };
    svg_doc(body, stroke)
}

pub(super) fn svg_image(name: &str, stroke: &str) -> gtk4::Image {
    let svg = icon_svg(name, stroke);
    let tex = gtk4::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from(svg.as_bytes()))
        .expect("embedded svg must parse");
    let img = gtk4::Image::from_paintable(Some(&tex));
    img.set_pixel_size(16);
    img
}
