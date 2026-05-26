//! Single image-rendering entry-point for the entire app.
//!
//! Every cover, avatar, banner, and thumbnail flows through one of the two
//! functions below.  There is no other image widget in the codebase.
//!
//! # Quick reference
//!
//! | Situation | Use |
//! |---|---|
//! | Standard cover with `CoverSize` | `cover(handle, kind, size)` |
//! | Custom pixel size / radius | `custom(handle, kind, px, radius)` |
//! | Circular avatar | `circle(handle, kind, px)` |
//!
//! All functions accept `Option<&image::Handle>` from `ImageState`; resolving,
//! downloading, and local-path registration happen in the update layer.

use iced::Element;
use iced::widget::{container, image, svg};

use crate::image::{CoverSize, ImageKind};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render a cover image at a standard `CoverSize`.
pub fn cover(
    handle: Option<&image::Handle>,
    kind: ImageKind,
    size: CoverSize,
) -> Element<'static, crate::app::Message> {
    shaped(handle, kind, size.px(), size.radius())
}

/// Render with a custom pixel size and border radius.
pub fn custom(
    handle: Option<&image::Handle>,
    kind: ImageKind,
    px: f32,
    radius: f32,
) -> Element<'static, crate::app::Message> {
    shaped(handle, kind, px, radius)
}

/// Render a circular image (avatar, artist portrait).
pub fn circle(
    handle: Option<&image::Handle>,
    kind: ImageKind,
    px: f32,
) -> Element<'static, crate::app::Message> {
    shaped(handle, kind, px, px / 2.0)
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

fn shaped(
    handle: Option<&image::Handle>,
    kind: ImageKind,
    px: f32,
    radius: f32,
) -> Element<'static, crate::app::Message> {
    if let Some(handle) = handle {
        container(
            image(handle.clone())
                .width(px)
                .height(px)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(radius),
        )
        .width(px)
        .height(px)
        .into()
    } else {
        let svg_data = placeholder_svg(kind);
        let icon = (px * 0.4).min(64.0);
        container(
            svg(svg::Handle::from_memory(svg_data.as_bytes()))
                .width(icon)
                .height(icon),
        )
        .width(px)
        .height(px)
        .center_x(px)
        .center_y(px)
        .style(move |t| iced::widget::container::Style {
            background: Some(iced::Background::Color(
                crate::ui::theme::surface_container(t),
            )),
            border: iced::Border {
                radius: radius.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    }
}

fn placeholder_svg(kind: ImageKind) -> &'static str {
    match kind {
        ImageKind::SongCover
        | ImageKind::LocalSongCover
        | ImageKind::PlaylistCover
        | ImageKind::LocalPlaylistCover
        | ImageKind::AlbumCover
        | ImageKind::Banner => crate::ui::icons::MUSIC,
        ImageKind::ArtistCover | ImageKind::UserAvatar => crate::ui::icons::USER,
    }
}
