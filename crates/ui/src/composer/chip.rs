//! What a chip looks like: its pill, icon and hover preview.
use super::*;

/// The hover card around an image chip's preview: padding on every side.
pub(super) const IMAGE_TOOLTIP_INSET: f32 = 5.0;
/// The name-and-size row under the preview.
pub(super) const IMAGE_TOOLTIP_LABEL_HEIGHT: f32 = 22.0;
pub(super) const IMAGE_TOOLTIP_RADIUS: f32 = 16.0;
const IMAGE_TOOLTIP_MAX: (f32, f32) = (240.0, 160.0);

/// "443 KB", "1.2 MB": what an attachment weighs, for hover cards.
pub(super) fn format_file_size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    let bytes = bytes as f64;
    if bytes < KB {
        format!("{} B", bytes as usize)
    } else if bytes < KB * KB {
        format!("{} KB", (bytes / KB).round() as usize)
    } else {
        format!("{:.1} MB", bytes / (KB * KB))
    }
}

/// A staged attachment an attachment chip can refer to. An image carries the
/// size of its hover preview: its own aspect ratio, contained in
/// [`IMAGE_TOOLTIP_MAX`] and never enlarged past its pixels.
pub(super) struct ChipAttachment {
    pub(super) image: Option<std::sync::Arc<gpui::Image>>,
    pub(super) preview: (f32, f32),
}

impl ChipAttachment {
    pub(super) fn new(image: Option<std::sync::Arc<gpui::Image>>) -> Self {
        let Some(picture) = &image else {
            return Self {
                image,
                preview: (0.0, 0.0),
            };
        };
        let (width, height) =
            ::image::ImageReader::new(std::io::Cursor::new(picture.bytes.as_slice()))
                .with_guessed_format()
                .ok()
                .and_then(|reader| reader.into_dimensions().ok())
                .filter(|(width, height)| *width > 0 && *height > 0)
                .unwrap_or((16, 10));
        let scale = (IMAGE_TOOLTIP_MAX.0 / width as f32)
            .min(IMAGE_TOOLTIP_MAX.1 / height as f32)
            .min(1.0);
        let preview = (
            (width as f32 * scale).max(48.0),
            (height as f32 * scale).max(32.0),
        );
        Self { image, preview }
    }
}

/// Spaces reserve room at the start of every chip for its icon well, which is
/// painted over them, so the text, caret and wrapping all treat the chip as
/// plain text. Non-breaking spaces, because the bundled Geist has no wider or
/// thinner ones.
pub(super) const CHIP_ICON_SLOT: &str = "\u{00A0}\u{00A0}\u{00A0}\u{00A0}\u{00A0}";
/// Breathing room after a chip's label.
pub(super) const CHIP_TRAILING_PAD: &str = "\u{00A0}\u{00A0}";
/// The icon sits at this size in its well.
const CHIP_ICON_SIZE: f32 = 14.0;

/// What a chip refers to; it picks the chip's icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipKind {
    Image,
    Skill,
    Command,
    File,
    Directory,
}

/// What a chip's icon is painted from: a monochrome glyph tinted like the
/// chip, or the file theme's own polychrome artwork, which tells formats and
/// languages apart.
pub(crate) enum ChipIcon {
    Glyph(&'static str),
    FileTheme(SharedString),
}

/// The icon of a chip. `path` names the file or folder for the file kinds.
pub(crate) fn chip_icon(
    kind: ChipKind,
    path: &str,
    appearance: crate::theme::Appearance,
) -> ChipIcon {
    use crate::file_icons::{FileIconIdentity, asset_path};
    match kind {
        ChipKind::Image => ChipIcon::Glyph(crate::icons::GALLERY),
        ChipKind::Skill => ChipIcon::Glyph(crate::icons::MAGIC_STICK_3),
        ChipKind::Command => ChipIcon::Glyph(crate::icons::COMMAND),
        ChipKind::File => ChipIcon::FileTheme(asset_path(FileIconIdentity::file(path), appearance)),
        ChipKind::Directory => ChipIcon::FileTheme(asset_path(
            FileIconIdentity::directory(path.trim_end_matches('/'), false),
            appearance,
        )),
    }
}

/// Paint a chip like the transcript's file badges: a soft rounded pill whose
/// first row starts with the icon on a small well. Callers center `chip` on
/// the line box: Geist's cap height sits exactly on its middle (ascent 1005,
/// descent 295, cap 710 per 1000 units), so a centered pill centers the label.
pub(crate) fn paint_chip(
    window: &mut Window,
    chip: Bounds<Pixels>,
    icon: Option<&ChipIcon>,
    theme: &Theme,
    cx: &App,
) {
    window.paint_quad(quad(
        chip,
        px(5.0),
        theme.ink(0.09),
        px(0.0),
        gpui::transparent_black(),
        BorderStyle::default(),
    ));
    let Some(icon) = icon else {
        return;
    };
    let well = chip.size.height - px(2.0);
    let well_bounds = Bounds::new(chip.origin + point(px(1.0), px(1.0)), size(well, well));
    window.paint_quad(quad(
        well_bounds,
        px(4.0),
        crate::file_icons::well_bg(theme),
        px(0.0),
        gpui::transparent_black(),
        BorderStyle::default(),
    ));
    // The icon follows the chip's size, so chips on a small line stay tidy.
    let side = px(CHIP_ICON_SIZE).min(well - px(4.0));
    let bounds = Bounds::new(
        well_bounds.origin + point((well - side) / 2.0, (well - side) / 2.0),
        size(side, side),
    );
    match icon {
        ChipIcon::Glyph(path) => {
            let _ = window.paint_svg(
                bounds,
                (*path).into(),
                None,
                gpui::TransformationMatrix::default(),
                theme.text_muted,
                cx,
            );
        }
        ChipIcon::FileTheme(path) => {
            if let Some(image) = crate::file_icons::raster(path, cx) {
                let _ = window.paint_image(bounds, Default::default(), image, 0, false);
            }
        }
    }
}

/// A sent message's text with its chips painted over their spans, for rows
/// that show one line of it (the queue). `text` is the projected display text
/// and `spans` its chips, as [`sent_mention_display`] returns them.
pub(crate) fn chip_text(
    text: SharedString,
    spans: Vec<SentMentionSpan>,
    theme: &Theme,
) -> gpui::AnyElement {
    if spans.is_empty() {
        return text.into_any_element();
    }
    let styled = gpui::StyledText::new(text);
    let layout = styled.layout().clone();
    let icons: Vec<ChipIcon> = spans
        .iter()
        .map(|span| chip_icon(span.kind, &span.path, theme.appearance))
        .collect();
    let theme = theme.clone();
    let underlay = gpui::canvas(
        |_, _, _| (),
        move |_, _, window, cx| {
            for (span, icon) in spans.iter().zip(&icons) {
                for (row, rect) in
                    crate::markdown::render::range_rects(&layout, &span.range, 0.0, 2.0)
                        .into_iter()
                        .enumerate()
                {
                    paint_chip(window, rect, (row == 0).then_some(icon), &theme, cx);
                }
            }
        },
    )
    .absolute()
    .size_full();
    div()
        .relative()
        .child(underlay)
        .child(styled)
        .into_any_element()
}
