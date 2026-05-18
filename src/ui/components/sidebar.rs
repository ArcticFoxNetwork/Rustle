//! Left sidebar navigation component
//! Dark gray panel with logo, menu, library section, and user profile

use iced::widget::{Space, button, column, container, mouse_area, row, scrollable, svg, text};
use iced::{Alignment, Color, Element, Fill, Padding};

use crate::app::{Message, Route, SidebarId};
use crate::i18n::{Key, Locale};
use crate::ui::animation::HoverAnimations;
use crate::ui::components::importing_card::{self, ImportingPlaylist};
use crate::ui::components::player_bar::PLAYER_BAR_HEIGHT;
use crate::ui::theme::{self, BOLD_WEIGHT, MEDIUM_WEIGHT};

/// Navigation menu items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavItem {
    Home,
    Discover,
    Radio,
    Settings,
    AudioEngine,
}

impl NavItem {
    pub fn i18n_key(&self) -> Key {
        match self {
            NavItem::Home => Key::NavHome,
            NavItem::Discover => Key::NavDiscover,
            NavItem::Radio => Key::NavRadio,
            NavItem::Settings => Key::NavSettings,
            NavItem::AudioEngine => Key::NavAudioEngine,
        }
    }

    pub fn icon_svg(&self) -> &'static str {
        match self {
            NavItem::Home => crate::ui::icons::HOME,
            NavItem::Discover => crate::ui::icons::BROWSE,
            NavItem::Radio => crate::ui::icons::RADIO,
            NavItem::Settings => crate::ui::icons::SETTINGS,
            NavItem::AudioEngine => crate::ui::icons::EQUALIZER,
        }
    }
}

/// Library section items (local only now)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryItem {
    RecentlyPlayed,
}

impl LibraryItem {
    pub fn i18n_key(&self) -> Key {
        match self {
            LibraryItem::RecentlyPlayed => Key::LibraryRecentlyPlayed,
        }
    }

    pub fn icon_svg(&self) -> &'static str {
        match self {
            LibraryItem::RecentlyPlayed => crate::ui::icons::CLOCK,
        }
    }
}

/// Build the sidebar component
pub fn view(
    current_route: &Route,
    locale: Locale,
    is_logged_in: bool,
    user_info: Option<&crate::app::UserInfo>,
    importing_playlist: Option<&ImportingPlaylist>,
    playlists: &[crate::database::DbPlaylist],
    user_playlists: &[crate::api::SongList],
    sidebar_animations: &HoverAnimations<SidebarId>,
    sidebar_width: f32,
) -> Element<'static, Message> {
    // Logo section
    let logo = row![
        // Pink music icon
        container(
            svg(svg::Handle::from_memory(
                crate::ui::icons::MUSIC_LOGO.as_bytes()
            ))
            .width(30)
            .height(30)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::ACCENT_PINK),
            })
        ),
        Space::new().width(12),
        text(locale.get(Key::AppName))
            .size(theme::TEXT_SIZE_TITLE_LARGE - 2.0)
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme))
            })
            .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT))
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(24.0).bottom(34.0));

    // Main navigation menu with hover animations
    let nav_items = [NavItem::Home, NavItem::Discover, NavItem::Radio];
    let nav_menu = column(nav_items.into_iter().enumerate().map(|(idx, item)| {
        let is_active = matches!(current_route.nav_item(), Some(active) if active == item);
        let hover_progress = sidebar_animations.get_progress(&SidebarId::Nav(idx));
        sidebar_button_animated(
            item.icon_svg(),
            locale.get(item.i18n_key()).to_string(),
            is_active,
            hover_progress,
            SidebarId::Nav(idx),
            Message::Navigate(item),
        )
    }))
    .spacing(4);

    // Library section header
    let library_header = text(locale.get(Key::LibraryTitle))
        .size(theme::TEXT_SIZE_LABEL)
        .style(|theme| text::Style {
            color: Some(theme::text_muted(theme)),
        })
        .width(Fill);

    // Recently played button - use same animated style as nav buttons
    let recently_played_progress = sidebar_animations.get_progress(&SidebarId::Library(0));
    let recently_played = sidebar_button_animated(
        crate::ui::icons::CLOCK,
        locale.get(Key::LibraryRecentlyPlayed).to_string(),
        matches!(current_route, Route::RecentlyPlayed),
        recently_played_progress,
        SidebarId::Library(0),
        Message::LibrarySelect(LibraryItem::RecentlyPlayed),
    );

    // Import local playlist button - use same animated style as nav buttons
    let import_progress = sidebar_animations.get_progress(&SidebarId::Library(1));
    let import_playlist_btn = sidebar_button_animated(
        crate::ui::icons::PLUS,
        locale.get(Key::ImportLocalPlaylist).to_string(),
        false, // not active
        import_progress,
        SidebarId::Library(1),
        Message::ImportLocalPlaylist,
    );

    // User profile card at bottom
    let user_hover_progress = sidebar_animations.get_progress(&SidebarId::UserCard);

    let not_logged_in = locale.get(Key::NotLoggedIn).to_string();
    let click_to_login = locale.get(Key::ClickToLogin).to_string();
    let avatar_size = 40.0;
    let avatar_radius = avatar_size / 2.0;
    let avatar_icon_size = 20.0;

    let avatar_elem = if is_logged_in {
        if let Some(info) = user_info {
            if let Some(handle) = &info.avatar_handle {
                container(
                    iced::widget::image(handle.clone())
                        .width(Fill)
                        .height(Fill)
                        .content_fit(iced::ContentFit::Cover)
                        .border_radius(avatar_radius),
                )
                .width(avatar_size)
                .height(avatar_size)
                .into()
            } else {
                avatar_placeholder(avatar_size, avatar_radius, avatar_icon_size)
            }
        } else {
            avatar_placeholder(avatar_size, avatar_radius, avatar_icon_size)
        }
    } else {
        avatar_placeholder(avatar_size, avatar_radius, avatar_icon_size)
    };

    let text_col = if is_logged_in {
        if let Some(info) = user_info {
            if info.vip_type > 0 {
                column![
                    text(info.nickname.clone())
                        .size(theme::TEXT_SIZE_BODY_LARGE)
                        .style(|theme| text::Style {
                            color: Some(theme::text_primary(theme))
                        })
                        .font(iced::Font::DEFAULT.weight(MEDIUM_WEIGHT)),
                    Space::new().height(2),
                    row![text("VIP").size(theme::TEXT_SIZE_CAPTION).style(|_theme| {
                        text::Style {
                            color: Some(theme::ACCENT_PINK),
                        }
                    }),],
                ]
                .into()
            } else {
                column![
                    text(info.nickname.clone())
                        .size(theme::TEXT_SIZE_BODY_LARGE)
                        .style(|theme| text::Style {
                            color: Some(theme::text_primary(theme))
                        })
                        .font(iced::Font::DEFAULT.weight(MEDIUM_WEIGHT)),
                ]
                .into()
            }
        } else {
            column![
                text(not_logged_in)
                    .size(theme::TEXT_SIZE_BODY_LARGE)
                    .style(|theme| text::Style {
                        color: Some(theme::text_primary(theme))
                    })
                    .font(iced::Font::DEFAULT.weight(MEDIUM_WEIGHT)),
                Space::new().height(2),
                text(click_to_login)
                    .size(theme::TEXT_SIZE_LABEL)
                    .color(theme::ACCENT_PINK),
            ]
            .into()
        }
    } else {
        column![
            text(not_logged_in)
                .size(theme::TEXT_SIZE_BODY_LARGE)
                .style(|theme| text::Style {
                    color: Some(theme::text_primary(theme))
                })
                .font(iced::Font::DEFAULT.weight(MEDIUM_WEIGHT)),
            Space::new().height(2),
            text(click_to_login)
                .size(theme::TEXT_SIZE_LABEL)
                .color(theme::ACCENT_PINK),
        ]
        .into()
    };

    let user_card_content = user_card_row(
        avatar_elem,
        text_col,
        user_hover_progress,
        if is_logged_in {
            Message::OpenSettings
        } else {
            Message::ToggleLoginPopup
        },
    );

    let user_card: Element<'static, Message> = mouse_area(user_card_content)
        .on_enter(Message::HoverSidebar(Some(SidebarId::UserCard)))
        .on_exit(Message::HoverSidebar(None))
        .into();

    // Build library section with proper spacing (same as nav_menu)
    let mut library_items: Vec<Element<'static, Message>> = vec![recently_played];

    // Show importing playlist if any
    if let Some(playlist) = importing_playlist {
        library_items.push(importing_card::view(playlist));
    }

    // Show local playlists with hover animations
    for playlist in playlists {
        let name = playlist.name.clone();
        let id = playlist.id;
        let is_active = matches!(current_route, Route::Playlist(current_id) if *current_id == id);
        let hover_progress = sidebar_animations.get_progress(&SidebarId::Playlist(id));
        library_items.push(sidebar_button_animated(
            crate::ui::icons::MUSIC,
            name,
            is_active,
            hover_progress,
            SidebarId::Playlist(id),
            Message::OpenPlaylist(id),
        ));
    }

    library_items.push(import_playlist_btn);

    // Library section with spacing matching nav_menu
    let library_section = column(library_items).spacing(4);

    // Build scrollable content (only library and cloud playlists, not logo/nav)
    let mut scrollable_items: Vec<Element<'static, Message>> = vec![
        container(library_header)
            .padding(Padding::new(0.0).left(14.0).bottom(8.0))
            .into(),
        library_section.into(),
    ];

    // Only show cloud playlists section if logged in
    if is_logged_in {
        let cloud_header = text(locale.get(Key::CloudPlaylistsTitle))
            .size(theme::TEXT_SIZE_LABEL)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .width(Fill);

        scrollable_items.push(Space::new().height(20).into());
        scrollable_items.push(
            container(cloud_header)
                .padding(Padding::new(0.0).left(14.0).bottom(8.0))
                .into(),
        );

        // User playlists
        let mut cloud_playlist_items: Vec<Element<'static, Message>> = Vec::new();
        for playlist in user_playlists {
            let name = playlist.name.clone();
            let id = playlist.id;
            let is_active =
                matches!(current_route, Route::NcmPlaylist(current_id) if *current_id == id);
            let hover_progress = sidebar_animations.get_progress(&SidebarId::UserPlaylist(id));

            cloud_playlist_items.push(sidebar_button_animated(
                crate::ui::icons::MUSIC,
                name,
                is_active,
                hover_progress,
                SidebarId::UserPlaylist(id),
                Message::OpenNcmPlaylist(id),
            ));
        }

        scrollable_items.push(column(cloud_playlist_items).spacing(4).into());
    }

    // Scrollable area for library and cloud playlists only (hidden scrollbar)
    let scrollable_content =
        scrollable(column(scrollable_items))
            .height(Fill)
            .style(|_theme, _status| scrollable::Style {
                container: iced::widget::container::Style::default(),
                vertical_rail: scrollable::Rail {
                    background: None,
                    border: iced::Border::default(),
                    scroller: scrollable::Scroller {
                        // Hidden scrollbar - transparent
                        background: iced::Background::Color(Color::TRANSPARENT),
                        border: iced::Border::default(),
                    },
                },
                horizontal_rail: scrollable::Rail {
                    background: None,
                    border: iced::Border::default(),
                    scroller: scrollable::Scroller {
                        background: iced::Background::Color(Color::TRANSPARENT),
                        border: iced::Border::default(),
                    },
                },
                gap: None,
                auto_scroll: scrollable::AutoScroll {
                    background: iced::Background::Color(Color::TRANSPARENT),
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    icon: Color::TRANSPARENT,
                },
            });

    let top_content = column![logo, nav_menu, Space::new().height(24), scrollable_content,]
        .padding(16)
        .width(sidebar_width)
        .height(Fill);

    let user_footer_border = container(Space::new().height(0))
        .width(Fill)
        .height(1)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::player_bar_border(theme))),
            ..Default::default()
        });

    let user_footer_content = container(user_card)
        .width(Fill)
        .height(PLAYER_BAR_HEIGHT - 1.0)
        .padding(Padding::new(8.0).left(16.0).right(16.0))
        .align_y(Alignment::Center)
        .style(|theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::player_bar_bg(theme))),
            ..Default::default()
        });

    let content = column![top_content, user_footer_border, user_footer_content]
        .width(sidebar_width)
        .height(Fill);

    // Wrap entire sidebar in mouse_area to clear hover when leaving sidebar
    let sidebar_container = container(content)
        .width(sidebar_width)
        .height(Fill)
        .style(theme::sidebar);

    mouse_area(sidebar_container)
        .on_exit(Message::HoverSidebar(None))
        .into()
}

/// Circular avatar placeholder with a centered user icon
fn avatar_placeholder(size: f32, radius: f32, icon_size: f32) -> Element<'static, Message> {
    container(
        svg(svg::Handle::from_memory(crate::ui::icons::USER.as_bytes()))
            .width(icon_size)
            .height(icon_size)
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_secondary(theme)),
            }),
    )
    .width(size)
    .height(size)
    .center_x(size)
    .center_y(size)
    .style(move |theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::border_color(theme))),
        border: iced::Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Build a user card row with avatar, text column, and chevron indicator
fn user_card_row(
    avatar: Element<'static, Message>,
    text_column: Element<'static, Message>,
    hover_progress: f32,
    on_press: Message,
) -> Element<'static, Message> {
    button(
        row![
            avatar,
            Space::new().width(12),
            text_column,
            Space::new().width(Fill),
            svg(svg::Handle::from_memory(
                crate::ui::icons::CHEVRON_RIGHT.as_bytes()
            ))
            .width(16)
            .height(16)
            .style(move |theme, _status| svg::Style {
                color: Some(theme::animated_brightness(theme, hover_progress)),
            }),
        ]
        .align_y(Alignment::Center)
        .padding(Padding::new(10.0)),
    )
    .width(Fill)
    .padding(0)
    .style(move |_theme, _status| iced::widget::button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(on_press)
    .into()
}

/// Create an animated sidebar button with hover transition
/// Used for both navigation items and playlist items
fn sidebar_button_animated(
    icon_svg: &'static str,
    label: String,
    is_active: bool,
    hover_progress: f32,
    sidebar_id: SidebarId,
    on_press: Message,
) -> Element<'static, Message> {
    let icon = svg(svg::Handle::from_memory(icon_svg.as_bytes()))
        .width(22)
        .height(22)
        .style(move |theme, _status| svg::Style {
            color: Some(if is_active {
                theme::text_primary(theme)
            } else {
                theme::animated_brightness(theme, hover_progress)
            }),
        });

    let label_text = text(label)
        .size(theme::TEXT_SIZE_BODY_LARGE)
        .style(move |theme| text::Style {
            color: Some(if is_active {
                theme::text_primary(theme)
            } else {
                theme::animated_brightness(theme, hover_progress)
            }),
        });

    let content = row![icon, Space::new().width(12), label_text]
        .align_y(Alignment::Center)
        .padding(Padding::new(12.0).left(14.0).right(14.0));

    // Use button for proper click feedback and cursor
    let btn = button(content)
        .width(Fill)
        .padding(0)
        .style(move |theme, _status| {
            let bg_alpha = if is_active {
                0.12
            } else {
                0.12 * hover_progress
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(theme::hover_bg_alpha(
                    theme, bg_alpha,
                ))),
                border: iced::Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                text_color: theme::text_primary(theme),
                ..Default::default()
            }
        })
        .on_press(on_press.clone());

    // Add hover events if not active
    // Each button needs on_exit to clear hover when mouse leaves
    if is_active {
        btn.into()
    } else {
        mouse_area(btn)
            .on_enter(Message::HoverSidebar(Some(sidebar_id)))
            .on_exit(Message::HoverSidebar(None))
            .into()
    }
}
