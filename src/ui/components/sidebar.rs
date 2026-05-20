//! Left sidebar navigation component
//! Dark gray panel with logo, menu, library section, and user profile

use iced::widget::{Space, button, column, container, mouse_area, row, scrollable, svg, text};
use iced::{Alignment, Color, Element, Fill, Padding};

use crate::app::{Message, Route, SidebarId};
use crate::i18n::{Key, Locale};
use crate::ui::animation::HoverAnimations;
use crate::ui::components::importing_card::{self, ImportingPlaylist};
use crate::ui::theme::{self, BOLD_WEIGHT};

/// Navigation menu items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavItem {
    Home,
    Discover,
    Radio,
    Downloads,
    Settings,
    AudioEngine,
}

impl NavItem {
    pub fn i18n_key(&self) -> Key {
        match self {
            NavItem::Home => Key::NavHome,
            NavItem::Discover => Key::NavDiscover,
            NavItem::Radio => Key::NavRadio,
            NavItem::Downloads => Key::NavDownloads,
            NavItem::Settings => Key::NavSettings,
            NavItem::AudioEngine => Key::NavAudioEngine,
        }
    }

    pub fn icon_svg(&self) -> &'static str {
        match self {
            NavItem::Home => crate::ui::icons::HOME,
            NavItem::Discover => crate::ui::icons::BROWSE,
            NavItem::Radio => crate::ui::icons::RADIO,
            NavItem::Downloads => crate::ui::icons::DOWNLOAD,
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
    let nav_items = [
        NavItem::Home,
        NavItem::Discover,
        NavItem::Radio,
        NavItem::Downloads,
    ];
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
        let s = crate::ui::components::cover_thumb::CoverSize::Tiny;
        let cover_handle = playlist.cover_path.as_deref()
            .filter(|p| !p.is_empty() && std::path::Path::new(p).exists())
            .map(|p| iced::widget::image::Handle::from_path(std::path::PathBuf::from(p)));
        let cover_el = Some(crate::ui::components::cover_thumb::thumb(
            cover_handle.as_ref(), s.px(), s.radius(),
        ));
        library_items.push(sidebar_button_animated_opt_cover(
            crate::ui::icons::MUSIC,
            cover_el,
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
        // Split user playlists into owned and collected
        let (owned_playlists, collected_playlists): (Vec<_>, Vec<_>) = user_playlists
            .iter()
            .partition(|pl| !pl.subscribed);

        // Helper to render a playlist button
        let render_playlist_btn =
            |playlist: &crate::api::SongList| -> Element<'static, Message> {
                let name = playlist.name.clone();
                let id = playlist.id;
                let is_active =
                    matches!(current_route, Route::NcmPlaylist(current_id) if *current_id == id);
                let hover_progress = sidebar_animations.get_progress(&SidebarId::UserPlaylist(id));

                let s = crate::ui::components::cover_thumb::CoverSize::Tiny;
                let cover_handle = crate::utils::find_playlist_cover(id)
                    .map(iced::widget::image::Handle::from_path);
                let cover_el = crate::ui::components::cover_thumb::thumb(
                    cover_handle.as_ref(), s.px(), s.radius(),
                );
                sidebar_button_animated_opt_cover(
                    crate::ui::icons::MUSIC,
                    Some(cover_el),
                    name,
                    is_active,
                    hover_progress,
                    SidebarId::UserPlaylist(id),
                    Message::OpenNcmPlaylist(id),
                )
            };

        scrollable_items.push(Space::new().height(20).into());

        // "My Playlists" section (owned)
        let my_header = text(locale.get(Key::CloudPlaylistsTitle))
            .size(theme::TEXT_SIZE_LABEL)
            .style(|theme| text::Style {
                color: Some(theme::text_muted(theme)),
            })
            .width(Fill);
        scrollable_items.push(
            container(my_header)
                .padding(Padding::new(0.0).left(14.0).bottom(8.0))
                .into(),
        );
        let my_items: Vec<Element<'static, Message>> =
            owned_playlists.into_iter().map(render_playlist_btn).collect();
        scrollable_items.push(column(my_items).spacing(4).into());

        // "Collected Playlists" section
        if !collected_playlists.is_empty() {
            scrollable_items.push(Space::new().height(20).into());
            let collected_header = text(locale.get(Key::CollectedPlaylistsTitle))
                .size(theme::TEXT_SIZE_LABEL)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme)),
                })
                .width(Fill);
            scrollable_items.push(
                container(collected_header)
                    .padding(Padding::new(0.0).left(14.0).bottom(8.0))
                    .into(),
            );
            let collected_items: Vec<Element<'static, Message>> =
                collected_playlists.into_iter().map(render_playlist_btn).collect();
            scrollable_items.push(column(collected_items).spacing(4).into());
        }
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

    let content = container(top_content).width(sidebar_width).height(Fill);

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
    sidebar_button_animated_opt_cover(icon_svg, None, label, is_active, hover_progress, sidebar_id, on_press)
}

fn sidebar_button_animated_opt_cover(
    fallback_svg: &'static str,
    cover_icon: Option<Element<'static, Message>>,
    label: String,
    is_active: bool,
    hover_progress: f32,
    sidebar_id: SidebarId,
    on_press: Message,
) -> Element<'static, Message> {
    let icon: Element<'static, Message> = match cover_icon {
        Some(el) => el,
        None => svg(svg::Handle::from_memory(fallback_svg.as_bytes()))
            .width(24)
            .height(24)
            .style(move |theme, _status| svg::Style {
                color: Some(if is_active {
                    theme::text_primary(theme)
                } else {
                    theme::animated_brightness(theme, hover_progress)
                }),
            })
            .into(),
    };

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
