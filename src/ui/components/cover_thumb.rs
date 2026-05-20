//! Cover thumbnail component — render a cover image from a pre-loaded handle,
//! or a music-icon placeholder.

use iced::widget::{container, image, svg};
use iced::Element;

pub use size::CoverSize;

mod size {
    pub enum CoverSize {
        Tiny, Picker, Small, Medium, Large,
    }
    impl CoverSize {
        pub fn px(&self) -> f32 {
            match self { CoverSize::Tiny => 40.0, CoverSize::Picker => 40.0, CoverSize::Small => 48.0, CoverSize::Medium => 56.0, CoverSize::Large => 200.0 }
        }
        pub fn radius(&self) -> f32 {
            match self { CoverSize::Tiny => 4.0, CoverSize::Picker => 8.0, CoverSize::Small => 4.0, CoverSize::Medium => 8.0, CoverSize::Large => 12.0 }
        }
    }
}

/// Render a cover image from a pre-loaded handle, or a music-icon placeholder.
pub fn thumb(handle: Option<&image::Handle>, s: f32, r: f32) -> Element<'static, crate::app::Message> {
    if let Some(handle) = handle {
        container(
            image(handle.clone())
                .width(s).height(s)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(r),
        )
        .width(s).height(s).into()
    } else {
        let icon = (s * 0.4).min(64.0);
        container(svg(svg::Handle::from_memory(crate::ui::icons::MUSIC.as_bytes())).width(icon).height(icon))
            .width(s).height(s).center_x(s).center_y(s)
            .style(move |t| iced::widget::container::Style {
                background: Some(iced::Background::Color(crate::ui::theme::surface_container(t))),
                border: iced::Border { radius: r.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
    }
}

/// Resolve a song's cover Handle from the local cache directory.
/// Returns `None` if the cover file is not cached (placeholder will be shown).
pub fn resolve_song_cover(song_id: i64) -> Option<image::Handle> {
    let ncm_id = if song_id < 0 { (-song_id) as u64 } else { song_id as u64 };
    crate::utils::find_song_cover(ncm_id).map(image::Handle::from_path)
}

/// Resolve a playlist's cover Handle from the local cache directory.
pub fn resolve_playlist_cover(playlist_id: u64) -> Option<image::Handle> {
    crate::utils::find_playlist_cover(playlist_id).map(image::Handle::from_path)
}
