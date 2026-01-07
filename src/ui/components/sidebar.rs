//! Left sidebar navigation component
//! Dark gray panel with logo, menu, library section, and user profile

use iced::widget::{Space, button, column, container, mouse_area, row, scrollable, svg, text};
use iced::{Alignment, Color, Element, Fill, Padding};

use crate::app::{Message, SidebarId};
use crate::i18n::{Key, Locale};
use crate::ui::animation::HoverAnimations;
use crate::ui::components::importing_card::{self, ImportingPlaylist};
use crate::ui::theme::{self, MEDIUM_WEIGHT};

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
    active_nav: NavItem,
    locale: Locale,
    is_logged_in: bool,
    user_info: Option<&crate::app::UserInfo>,
    importing_playlist: Option<&ImportingPlaylist>,
    playlists: &[crate::database::DbPlaylist],
    user_playlists: &[crate::api::SongList],
    current_playlist_id: Option<i64>, // Local playlist ID
    sidebar_animations: &HoverAnimations<SidebarId>,
    viewing_recently_played: bool,
    sidebar_width: f32,
) -> Element<'static, Message> {
    // Logo section
    let logo = row![
        // Pink music icon
        container(
            svg(svg::Handle::from_memory(
                crate::ui::icons::MUSIC_LOGO.as_bytes()
            ))
            .width(24)
            .height(24)
            .style(|_theme, _status| svg::Style {
                color: Some(theme::ACCENT_PINK),
            })
        ),
        Space::new().width(10),
        text(locale.get(Key::AppName))
            .size(22)
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme))
            })
    ]
    .align_y(Alignment::Center)
    .padding(Padding::new(20.0).bottom(30.0));

    // Main navigation menu with hover animations
    let nav_items = [NavItem::Home, NavItem::Discover, NavItem::Radio];
    let nav_menu = column(nav_items.into_iter().enumerate().map(|(idx, item)| {
        let is_active = item == active_nav && current_playlist_id.is_none();
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
        .size(12)
        .color(theme::TEXT_MUTED)
        .width(Fill);

    // Recently played button - use same animated style as nav buttons
    let recently_played_progress = sidebar_animations.get_progress(&SidebarId::Library(0));
    let recently_played = sidebar_button_animated(
        crate::ui::icons::CLOCK,
        locale.get(Key::LibraryRecentlyPlayed).to_string(),
        viewing_recently_played, // active when viewing recently played
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

    // User profile card at bottom - clickable login prompt with hover animation
    let user_hover_progress = sidebar_animations.get_progress(&SidebarId::UserCard);

    let not_logged_in = locale.get(Key::NotLoggedIn).to_string();
    let click_to_login = locale.get(Key::ClickToLogin).to_string();

    let user_card_content = if is_logged_in {
        if let Some(info) = user_info {
            // Use pre-loaded avatar handle for instant rendering
            let avatar = if let Some(handle) = &info.avatar_handle {
                container(
                    iced::widget::image(handle.clone())
                        .width(Fill)
                        .height(Fill)
                        .content_fit(iced::ContentFit::Cover),
                )
                .width(36)
                .height(36)
                .style(|_theme| iced::widget::container::Style {
                    border: iced::Border {
                        radius: 18.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                })
            } else {
                container(
                    svg(svg::Handle::from_memory(crate::ui::icons::USER.as_bytes()))
                        .width(18)
                        .height(18)
                        .style(|theme, _status| svg::Style {
                            color: Some(theme::text_secondary(theme)),
                        }),
                )
                .width(36)
                .height(36)
                .center_x(36)
                .center_y(36)
                .style(|theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(theme::border_color(theme))),
                    border: iced::Border {
                        radius: 18.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            };

            let vip_text = if info.vip_type > 0 {
                row![text("VIP").size(10).style(|_theme| text::Style {
                    color: Some(theme::ACCENT_PINK),
                }),]
            } else {
                row![]
            };

            button(
                row![
                    avatar,
                    Space::new().width(12),
                    column![
                        text(info.nickname.clone())
                            .size(14)
                            .style(|theme| text::Style {
                                color: Some(theme::text_primary(theme))
                            })
                            .font(iced::Font {
                                weight: MEDIUM_WEIGHT,
                                ..Default::default()
                            }),
                        Space::new().height(2),
                        vip_text,
                    ],
                    Space::new().width(Fill),
                    // Arrow indicator with animated color
                    svg(svg::Handle::from_memory(
                        crate::ui::icons::CHEVRON_RIGHT.as_bytes()
                    ))
                    .width(16)
                    .height(16)
                    .style(move |theme, _status| svg::Style {
                        color: Some(theme::animated_brightness(theme, user_hover_progress)),
                    }),
                ]
                .align_y(Alignment::Center)
                .padding(Padding::new(10.0)),
            )
        } else {
            // Should not happen if is_logged_in is true, but fallback
            button(
                row![
                    // Avatar placeholder
                    container(
                        svg(svg::Handle::from_memory(crate::ui::icons::USER.as_bytes()))
                            .width(18)
                            .height(18)
                            .style(|theme, _status| svg::Style {
                                color: Some(theme::text_secondary(theme)),
                            })
                    )
                    .width(36)
                    .height(36)
                    .center_x(36)
                    .center_y(36)
                    .style(|theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(theme::border_color(theme))),
                        border: iced::Border {
                            radius: 18.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                    Space::new().width(12),
                    column![
                        text(not_logged_in)
                            .size(14)
                            .style(|theme| text::Style {
                                color: Some(theme::text_primary(theme))
                            })
                            .font(iced::Font {
                                weight: MEDIUM_WEIGHT,
                                ..Default::default()
                            }),
                        Space::new().height(2),
                        text(click_to_login).size(12).color(theme::ACCENT_PINK),
                    ],
                    Space::new().width(Fill),
                    // Arrow indicator with animated color
                    svg(svg::Handle::from_memory(
                        crate::ui::icons::CHEVRON_RIGHT.as_bytes()
                    ))
                    .width(16)
                    .height(16)
                    .style(move |theme, _status| svg::Style {
                        color: Some(theme::animated_brightness(theme, user_hover_progress)),
                    }),
                ]
                .align_y(Alignment::Center)
                .padding(Padding::new(10.0)),
            )
        }
    } else {
        button(
            row![
                // Avatar placeholder
                container(
                    svg(svg::Handle::from_memory(crate::ui::icons::USER.as_bytes()))
                        .width(18)
                        .height(18)
                        .style(|theme, _status| svg::Style {
                            color: Some(theme::text_secondary(theme)),
                        })
                )
                .width(36)
                .height(36)
                .center_x(36)
                .center_y(36)
                .style(|theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(theme::border_color(theme))),
                    border: iced::Border {
                        radius: 18.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                Space::new().width(12),
                column![
                    text(not_logged_in)
                        .size(14)
                        .style(|theme| text::Style {
                            color: Some(theme::text_primary(theme))
                        })
                        .font(iced::Font {
                            weight: MEDIUM_WEIGHT,
                            ..Default::default()
                        }),
                    Space::new().height(2),
                    text(click_to_login).size(12).color(theme::ACCENT_PINK),
                ],
                Space::new().width(Fill),
                // Arrow indicator with animated color
                svg(svg::Handle::from_memory(
                    crate::ui::icons::CHEVRON_RIGHT.as_bytes()
                ))
                .width(16)
                .height(16)
                .style(move |theme, _status| svg::Style {
                    color: Some(theme::animated_brightness(theme, user_hover_progress)),
                }),
            ]
            .align_y(Alignment::Center)
            .padding(Padding::new(10.0)),
        )
    }
    .width(Fill)
    .padding(0)
    .style(move |theme, _status| iced::widget::button::Style {
        background: Some(iced::Background::Color(theme::hover_bg_alpha(
            theme,
            0.08 * user_hover_progress,
        ))),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(if is_logged_in {
        Message::OpenSettings
    } else {
        Message::ToggleLoginPopup
    });

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
        let is_active = current_playlist_id == Some(id);
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
            .size(12)
            .color(theme::TEXT_MUTED)
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
            // NCM playlist IDs are stored as negative in PlaylistView
            // So we compare current_playlist_id with -(id as i64)
            let is_active = current_playlist_id == Some(-(id as i64));
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

    // Main content: fixed header (logo + nav) + scrollable area + fixed user card
    let content = column![
        logo,
        nav_menu,
        Space::new().height(24),
        scrollable_content,
        Space::new().height(8),
        user_card,
    ]
    .padding(16)
    .width(sidebar_width);

    // Wrap entire sidebar in mouse_area to clear hover when leaving sidebar
    let sidebar_container = container(content)
        .width(sidebar_width)
        .height(Fill)
        .style(theme::sidebar);

    mouse_area(sidebar_container)
        .on_exit(Message::HoverSidebar(None))
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
        .width(20)
        .height(20)
        .style(move |theme, _status| svg::Style {
            color: Some(if is_active {
                theme::text_primary(theme)
            } else {
                theme::animated_brightness(theme, hover_progress)
            }),
        });

    let label_text = text(label).size(14).style(move |theme| text::Style {
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
