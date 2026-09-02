//! Shared cover card used by user and artist detail pages.

use iced::widget::{Space, button, column, text};
use iced::{Color, Element};

use crate::app::Message;
use crate::image::ImageKind;
use crate::ui::components::cover_image;
use crate::ui::theme;

/// Width shared by the cover cards on detail pages.
pub const CARD_WIDTH: f32 = 200.0;
/// Horizontal gap shared by the detail-page card grids.
pub const CARD_SPACING: f32 = 20.0;

const COVER_RADIUS: f32 = 16.0;
const COVER_TEXT_SPACING: f32 = 10.0;

/// Render a clickable cover card with a title and secondary label.
///
/// The caller supplies the already-resolved image handle. Image downloads and
/// cache registration remain in the app image pipeline, so both user playlists
/// and artist albums use the exact same rendering and loading path.
pub fn view<'a>(
    name: String,
    subtitle: String,
    cover_handle: Option<&'a iced::widget::image::Handle>,
    image_kind: ImageKind,
    on_press: Message,
) -> Element<'a, Message> {
    let cover = cover_image::custom(cover_handle, image_kind, CARD_WIDTH, COVER_RADIUS);

    button(
        column![
            cover,
            Space::new().height(COVER_TEXT_SPACING),
            text(name)
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_primary(theme)),
                }),
            text(subtitle)
                .size(theme::TEXT_SIZE_BODY)
                .style(|theme| iced::widget::text::Style {
                    color: Some(theme::text_secondary(theme)),
                }),
        ]
        .width(CARD_WIDTH),
    )
    .padding(0)
    .style(|_theme, _status| iced::widget::button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    })
    .on_press(on_press)
    .into()
}
