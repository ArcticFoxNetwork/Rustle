//! Right-click context menu — glass-morphism floating panel
//! Theme-aware: adapts to dark/light via theme functions in style closures

use iced::widget::{button, column, container, row, svg, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::app::{ContextMenuAction, ContextMenuState, Message};
use crate::i18n::{Key, Locale};
use crate::ui::{icons, theme};
use crate::utils::Source;

const W: f32 = 240.0;
const H: f32 = 36.0;
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 6.0;
const DIV_V: f32 = 4.0;
const DIV_X: f32 = 12.0;

pub fn view(menu: &ContextMenuState, locale: Locale, sw: f32, sh: f32) -> Element<'_, Message> {
    let source = menu.source;
    let is_liked = menu.is_liked;
    let can_show_folder = source != Source::Online;
    let can_download = source != Source::Local;
    let can_edit = source == Source::Local;

    let always_items: f32 = 7.0;
    let mut n = always_items;
    if can_show_folder {
        n += 1.0;
    }
    if can_download {
        n += 1.0;
    }
    if can_edit {
        n += 1.0;
    }
    let dv: f32 = 4.0;
    let mh = n * H + dv * (1.0 + DIV_V * 2.0) + PAD_Y * 2.0;

    let x = if menu.x + W > sw {
        (menu.x - W - 4.0).max(0.0)
    } else {
        menu.x.max(0.0)
    };
    let y = if menu.y + mh > sh {
        (menu.y - mh - 4.0).max(0.0)
    } else {
        menu.y.max(0.0)
    };

    let backdrop = iced::widget::mouse_area(
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                ..Default::default()
            }),
    )
    .on_press(Message::CloseContextMenu);

    use ContextMenuAction::*;
    let id = menu.song_id;
    macro_rules! ic {
        ($s:expr) => {
            svg::Handle::from_memory($s.as_bytes())
        };
    }

    let mut items: Vec<Element<'_, Message>> = Vec::new();
    items.push(item(
        ic!(icons::PLAY),
        locale.get(Key::ContextMenuPlayNow),
        Style::Normal,
        msg(PlayNow, id),
    ));
    items.push(item(
        ic!(icons::SKIP_NEXT),
        locale.get(Key::ContextMenuPlayNext),
        Style::Normal,
        msg(PlayNext, id),
    ));
    items.push(div());
    let (heart_icon, heart_label) = if is_liked {
        (
            ic!(icons::HEART),
            locale.get(Key::ContextMenuRemoveFavorites),
        )
    } else {
        (
            ic!(icons::HEART_OUTLINE),
            locale.get(Key::ContextMenuAddFavorites),
        )
    };
    items.push(item(
        heart_icon,
        heart_label,
        Style::Normal,
        msg(AddToFavorites, id),
    ));
    items.push(item(
        ic!(icons::PLUS),
        locale.get(Key::ContextMenuAddPlaylist),
        Style::Normal,
        msg(AddToPlaylist, id),
    ));
    items.push(div());
    items.push(item(
        ic!(icons::USER),
        locale.get(Key::ContextMenuViewArtist),
        Style::Normal,
        msg(ViewArtist, id),
    ));
    items.push(item(
        ic!(icons::DISC),
        locale.get(Key::ContextMenuViewAlbum),
        Style::Normal,
        msg(ViewAlbum, id),
    ));
    items.push(div());
    if can_show_folder {
        items.push(item(
            ic!(icons::FOLDER),
            locale.get(Key::ContextMenuShowInFolder),
            Style::Normal,
            msg(ShowInFolder, id),
        ));
    }
    if can_download {
        items.push(item(
            ic!(icons::DOWNLOAD),
            locale.get(Key::ContextMenuDownload),
            Style::Normal,
            msg(Download, id),
        ));
    }
    if can_edit {
        items.push(item(
            ic!(icons::EDIT),
            locale.get(Key::ContextMenuEditTags),
            Style::Accent,
            msg(EditSongTags, id),
        ));
    }
    items.push(div());
    items.push(item(
        ic!(icons::TRASH),
        locale.get(Key::ContextMenuRemoveFromList),
        Style::Danger,
        msg(RemoveFromList, id),
    ));

    let panel = container(
        column(items)
            .spacing(0)
            .padding(Padding::new(PAD_Y).left(PAD_X).right(PAD_X)),
    )
    .width(W)
    .style(|t| container::Style {
        background: Some(Background::Color(glass(t))),
        border: Border {
            radius: 12.0.into(),
            width: 1.0,
            color: glass_border(t),
        },
        ..Default::default()
    });

    let positioned = container(panel).padding(Padding::new(0.0).top(y).left(x));
    container(iced::widget::stack([backdrop.into(), positioned.into()]))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn msg(a: ContextMenuAction, id: i64) -> Message {
    Message::ContextMenuAction(a, id)
}

enum Style {
    Normal,
    Accent,
    Danger,
}

impl Style {
    fn is_accent(&self) -> bool {
        matches!(self, Style::Accent)
    }
    fn is_danger(&self) -> bool {
        matches!(self, Style::Danger)
    }
}

fn item<'a>(
    icon: iced::widget::svg::Handle,
    label: &str,
    style: Style,
    msg: Message,
) -> Element<'a, Message> {
    let lb = label.to_string();
    let is_accent = style.is_accent();
    let is_danger = style.is_danger();
    button(
        container(
            row![
                svg(icon)
                    .width(16)
                    .height(16)
                    .style(move |t, _| svg::Style {
                        color: Some(if is_accent {
                            purple()
                        } else if is_danger {
                            theme::ACCENT_PINK
                        } else {
                            theme::text_muted(t)
                        }),
                    }),
                iced::widget::Space::new().width(12.0),
                text(lb).size(13.0).style(move |t| text::Style {
                    color: Some(if is_accent {
                        accent_text()
                    } else if is_danger {
                        theme::ACCENT_PINK
                    } else {
                        theme::text_primary(t)
                    }),
                }),
            ]
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(H))
    .style(move |t, s| {
        let hover = matches!(s, button::Status::Hovered | button::Status::Pressed);
        let bg = if hover {
            if is_accent {
                Some(Background::Color(purple_bg()))
            } else if is_danger {
                Some(Background::Color(red_bg()))
            } else {
                Some(Background::Color(theme::surface_hover(t)))
            }
        } else {
            None
        };
        button::Style {
            background: bg,
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(msg)
    .into()
}

fn purple() -> Color {
    Color::from_rgba(0.659, 0.333, 0.969, 1.0)
}
fn accent_text() -> Color {
    Color::from_rgba(0.847, 0.706, 0.996, 1.0)
}
fn purple_bg() -> Color {
    Color::from_rgba(0.659, 0.333, 0.969, 0.12)
}
fn red_bg() -> Color {
    Color::from_rgba(1.0, 0.08, 0.29, 0.10)
}

fn div<'a>() -> Element<'a, Message> {
    container(
        container(iced::widget::Space::new().width(Length::Fill).height(1.0))
            .height(Length::Fixed(1.0))
            .style(|t| container::Style {
                background: Some(Background::Color(glass_border(t))),
                ..Default::default()
            }),
    )
    .padding(Padding::new(DIV_V).left(DIV_X).right(DIV_X))
    .into()
}

// ── Theme-aware helpers ────────────────────────────

fn glass(t: &iced::Theme) -> Color {
    let s = theme::surface(t);
    Color::from_rgba(s.r, s.g, s.b, 0.92)
}

fn glass_border(t: &iced::Theme) -> Color {
    if theme::is_dark_theme(t) {
        Color::from_rgba(1.0, 1.0, 1.0, 0.07)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.08)
    }
}
