//! Song info / editor dialog — glass-morphism modal, theme-aware

use iced::border::Radius;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Border, Color, Element, Length};

use crate::app::{Message, SongEditDialogState};
use crate::i18n::{Key, Locale};
use crate::ui::{theme, widgets};

const COVER_SIZE: f32 = 192.0;

// ── Public ─────────────────────────────────────────

/// Content-only edit body (used by unified overlay — header/footer from modal_section).
pub fn view_edit_body(e: &SongEditDialogState, locale: Locale) -> Element<'_, Message> {
    let id = e.song_id;
    row![
        edit_cover_block(&e.cover_path, locale.get(Key::SongEditCoverReplace), id),
        spacer(32.0),
        column![
            inp(locale.get(Key::SongEditLabelTitle), &e.title, id, "title"),
            inp(
                locale.get(Key::SongEditLabelArtist),
                &e.artist,
                id,
                "artist"
            ),
            inp(locale.get(Key::SongEditLabelAlbum), &e.album, id, "album"),
            spacer(2.0),
            row![
                inp2(
                    "年份",
                    &e.year.map_or_else(String::new, |y| y.to_string()),
                    id,
                    "year"
                ),
                spacer(16.0),
                inp2(
                    "曲目号",
                    &e.track_number.map_or_else(String::new, |n| n.to_string()),
                    id,
                    "track_number"
                ),
            ],
            spacer(2.0),
            inp("流派", &e.genre, id, "genre"),
        ]
        .width(Length::Fill),
    ]
    .padding(24)
    .into()
}

// ── Cover ──────────────────────────────────────────

fn edit_cover_block<'a>(
    path: &Option<std::path::PathBuf>,
    replace_label: &str,
    song_id: i64,
) -> Element<'a, Message> {
    let handle = path
        .as_ref()
        .filter(|p| p.exists())
        .map(|p| iced::widget::image::Handle::from_path(p.clone()));
    let img = crate::ui::components::cover_image::custom(
        handle.as_ref(),
        crate::image::ImageKind::SongCover,
        COVER_SIZE,
        12.0,
    );
    let label = replace_label.to_string();
    let replace_button = button(text(label).size(11.0))
        .style(dim_btn)
        .padding([4, 12])
        .on_press(Message::PickSongEditCover(song_id));
    let replace_button =
        widgets::hover_surface(replace_button).style(|theme, progress| container::Style {
            background: Some(iced::Background::Color(theme::lerp_color(
                Color::TRANSPARENT,
                theme::surface_hover(theme),
                progress,
            ))),
            border: r(8.0),
            ..Default::default()
        });

    column![
        container(img)
            .width(COVER_SIZE)
            .height(COVER_SIZE)
            .style(cover_border),
        spacer(6.0),
        replace_button,
    ]
    .into()
}

// ── Fields ─────────────────────────────────────────

fn inp<'a>(label: &str, value: &str, sid: i64, field: &'static str) -> Element<'a, Message> {
    let l = label.to_string();
    let v = value.to_string();
    column![
        text(l).size(11.0).style(st(theme::text_secondary)),
        spacer(4.0),
        text_input("", &v)
            .on_input(move |x| msg_f(sid, field, x))
            .padding([8, 10])
            .style(inp_s),
        spacer(10.0),
    ]
    .width(Length::Fill)
    .into()
}

fn inp2<'a>(label: &str, value: &str, sid: i64, field: &'static str) -> Element<'a, Message> {
    let l = label.to_string();
    let v = value.to_string();
    column![
        text(l).size(11.0).style(st(theme::text_secondary)),
        spacer(4.0),
        text_input("", &v)
            .on_input(move |x| msg_f(sid, field, x))
            .width(Length::Fixed(90.0))
            .padding([8, 10])
            .style(inp_s),
    ]
    .into()
}

fn msg_f(sid: i64, field: &str, v: String) -> Message {
    Message::SongEditFieldChanged {
        song_id: sid,
        field: field.into(),
        value: v,
    }
}

// ── Buttons ────────────────────────────────────────

fn dim_btn(t: &iced::Theme, s: button::Status) -> button::Style {
    let hover = matches!(s, button::Status::Hovered | button::Status::Pressed);
    let tc = if hover {
        theme::text_primary(t)
    } else {
        theme::text_secondary(t)
    };
    button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        text_color: tc,
        ..Default::default()
    }
}

fn inp_s(t: &iced::Theme, s: text_input::Status) -> text_input::Style {
    let purple = Color::from_rgba(0.659, 0.333, 0.969, 1.0);
    let c = match s {
        text_input::Status::Focused { .. } => Color::from_rgba(0.659, 0.333, 0.969, 0.50),
        text_input::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 0.20),
        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.10),
    };
    text_input::Style {
        background: iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05)),
        border: Border {
            color: c,
            width: 1.0,
            radius: Radius::new(8.0),
        },
        icon: theme::TEXT_MUTED,
        placeholder: theme::TEXT_MUTED,
        value: theme::text_primary(t),
        selection: purple,
    }
}

// ── Theme-aware color helpers ──────────────────────

fn glass_border(t: &iced::Theme) -> Color {
    if theme::is_dark_theme(t) {
        Color::from_rgba(1.0, 1.0, 1.0, 0.10)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.10)
    }
}

fn r(v: f32) -> Border {
    Border {
        radius: Radius::new(v),
        ..Default::default()
    }
}
fn spacer<'a>(w: f32) -> Element<'a, Message> {
    iced::widget::Space::new().width(w).height(w).into()
}

fn st(f: fn(&iced::Theme) -> Color) -> impl Fn(&iced::Theme) -> text::Style {
    move |t| text::Style { color: Some(f(t)) }
}

fn cover_border(t: &iced::Theme) -> container::Style {
    container::Style {
        border: Border {
            radius: Radius::new(12.0),
            width: 1.0,
            color: glass_border(t),
        },
        ..Default::default()
    }
}
