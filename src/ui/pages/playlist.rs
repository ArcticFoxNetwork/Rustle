//! Playlist detail page
//!
//! Shows playlist info with gradient background extracted from cover,
//! and song list with hover effects.
//!
//! Uses virtual list for efficient rendering of large playlists.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use iced::widget::{
    Space, button, column, container, mouse_area, row, scrollable, svg, text, text_input,
};
use iced::{Alignment, Color, Element, Fill, Padding};

use crate::app::{ImageState, Message};
use crate::i18n::{Key, Locale};
use crate::ui::components::playlist_view::{self, PlaylistColumns, SongItem};
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::widgets::VirtualListState;
use crate::ui::{icons, theme};
use crate::utils::ColorPalette;

/// Playlist data for display
#[derive(Debug, Clone)]
pub struct PlaylistView {
    pub kind: DetailPageKind,
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub profile_stats: Option<String>,
    pub artist_tab: ArtistPageTab,
    pub artist_albums: Vec<crate::api::AlbumSummary>,
    pub user_playlists: Vec<crate::api::PlaylistSummary>,
    pub cover_path: Option<String>,
    pub owner: String,
    pub owner_artist_id: Option<u64>,
    pub owner_avatar_path: Option<String>,
    /// Creator user ID (for NCM playlists, 0 for local)
    pub creator_id: u64,
    pub song_count: u32,
    pub total_duration: String,
    pub like_count: String,
    pub songs: Vec<PlaylistSongView>,
    /// Extracted color palette from cover
    pub palette: ColorPalette,
    /// Whether this is a local playlist (no like count, no download)
    pub is_local: bool,
    /// Whether the current user has subscribed to this playlist
    pub is_subscribed: bool,
    /// Root folder backing this local library playlist, if any.
    pub watched_folder_path: Option<String>,
    /// Whether the local library root is actively monitored.
    pub watch_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailPageKind {
    Playlist,
    Album,
    User,
    Artist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArtistPageTab {
    #[default]
    TopSongs,
    Albums,
}

/// Song item in playlist (alias for SongItem)
pub type PlaylistSongView = SongItem;

/// Build the playlist detail page
pub fn view<'a>(
    playlist: &'a PlaylistView,
    image_state: &'a ImageState,
    song_animations: &'a crate::ui::animation::HoverAnimations<i64>,
    icon_animations: &crate::ui::animation::HoverAnimations<crate::app::IconId>,
    search_animation: &crate::ui::animation::SingleHoverAnimation,
    search_expanded: bool,
    search_query: &str,
    liked_songs: Option<&'a HashSet<u64>>,
    locale: Locale,
    scroll_state: Rc<RefCell<VirtualListState>>,
    current_user_id: Option<u64>,
    current_playing_id: Option<i64>,
    description_expanded: bool,
) -> Element<'a, Message> {
    let palette = playlist.palette.clone();
    let header = build_header(playlist, image_state, locale, description_expanded);
    let controls = build_controls(
        playlist,
        icon_animations,
        search_animation,
        search_expanded,
        search_query,
        locale,
        current_user_id,
    );

    // Filter songs by index so the full playlist is not cloned on every view rebuild.
    let filtered_indices = playlist_view::filter_song_indices(&playlist.songs, search_query);

    // Content with gradient that extends through controls
    let header_and_controls = column![header, controls,].spacing(0).width(Fill);

    // Wrap header+controls in gradient container
    // Use the extracted palette colors directly for a more vibrant look
    let primary = palette.primary;
    // The skeleton page uses `ColorPalette::default()` until its cover is
    // available. Do not paint that placeholder palette into the gradient:
    // while the cover is loading, the page should blend into the theme's
    // actual background instead of flashing the default pink/purple color.
    let has_background_cover = playlist_page_cover_handle(playlist, image_state).is_some();
    // Apply slight boost for brighter gradient
    let top_r = (primary.r * 1.1 + 0.05).min(1.0);
    let top_g = (primary.g * 1.05 + 0.03).min(1.0);
    let top_b = (primary.b * 1.08 + 0.04).min(1.0);

    let gradient_section = container(header_and_controls)
        .width(Fill)
        .style(move |theme| {
            // Get the bottom color based on theme (black for dark, white for light)
            let bottom_color = theme::background(theme);
            if !has_background_cover {
                return iced::widget::container::Style {
                    background: Some(iced::Background::Color(bottom_color)),
                    ..Default::default()
                };
            }
            let is_light = !theme::is_dark_theme(theme);

            // For light mode: make colors brighter and less saturated
            let (adj_r, adj_g, adj_b) = if is_light {
                // Lighten and desaturate for light mode
                let avg = (top_r + top_g + top_b) / 3.0;
                let desat = 0.4; // Desaturation factor
                let lighten = 0.3; // Lighten factor
                (
                    ((top_r * (1.0 - desat) + avg * desat) + lighten).min(1.0),
                    ((top_g * (1.0 - desat) + avg * desat) + lighten).min(1.0),
                    ((top_b * (1.0 - desat) + avg * desat) + lighten).min(1.0),
                )
            } else {
                (top_r, top_g, top_b)
            };

            iced::widget::container::Style {
                background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                    // Top to bottom gradient with palette colors
                    iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI))
                        .add_stop(0.0, Color::from_rgb(adj_r, adj_g, adj_b))
                        .add_stop(
                            0.55,
                            Color::from_rgb(
                                adj_r * 0.6 + bottom_color.r * 0.4,
                                adj_g * 0.55 + bottom_color.g * 0.4,
                                adj_b * 0.58 + bottom_color.b * 0.4,
                            ),
                        )
                        .add_stop(1.0, bottom_color),
                ))),
                ..Default::default()
            }
        });

    // Build song list header using the reusable component
    let columns = if playlist.is_local {
        PlaylistColumns::local()
    } else {
        PlaylistColumns::online()
    };
    let song_list_header = playlist_view::build_header(locale, columns);

    // Use virtual list for song rows
    let song_list = playlist_view::build_list(
        &playlist.songs,
        filtered_indices,
        image_state,
        song_animations,
        liked_songs,
        columns,
        scroll_state,
        current_playing_id,
    );

    let content = column![gradient_section, song_list_header, song_list,]
        .spacing(0)
        .width(Fill);

    content.into()
}

/// Build the playlist header
fn build_header(
    playlist: &PlaylistView,
    image_state: &ImageState,
    locale: Locale,
    description_expanded: bool,
) -> Element<'static, Message> {
    let s = crate::image::CoverSize::Large;
    let cover_handle = playlist_header_cover_handle(playlist, image_state);
    let cover: Element<'static, Message> = crate::ui::components::cover_image::cover(
        cover_handle,
        crate::image::ImageKind::PlaylistCover,
        s,
    );

    // Playlist type label - larger font
    let type_label_text = match playlist.kind {
        DetailPageKind::Playlist => locale.get(Key::PlaylistTypeLabel).to_string(),
        DetailPageKind::Album => "专辑".to_string(),
        DetailPageKind::User => "用户".to_string(),
        DetailPageKind::Artist => "歌手".to_string(),
    };
    let type_label = text(type_label_text)
        .size(theme::TEXT_SIZE_BODY)
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        });

    // Playlist title - larger font for big screens
    // Use Inter or system sans-serif with bold weight
    let title = text(playlist.name.clone())
        .size(theme::TEXT_SIZE_DISPLAY_LARGE)
        .line_height(iced::widget::text::LineHeight::Relative(1.0))
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT));

    // Description — clamped to 2 lines by default, expandable when long
    let description: Element<'static, Message> = if let Some(desc) = &playlist.description {
        let desc_text = desc.clone();
        // Rough estimate: count newlines + approximate line wraps by char count
        let line_count = desc_text.lines().count()
            + desc_text
                .lines()
                .map(|l| l.chars().count().saturating_sub(1) / 55)
                .sum::<usize>();
        let is_long = line_count > 2;

        let desc_widget = text(desc_text)
            .size(theme::TEXT_SIZE_BODY_LARGE)
            .style(|theme| text::Style {
                color: Some(theme::text_secondary(theme)),
            });

        if is_long {
            if description_expanded {
                // Expanded: scrollable description with "收起" button
                let scrollable_desc = scrollable(
                    container(desc_widget)
                        .width(Fill)
                        .padding(Padding::new(4.0).left(0.0)),
                )
                .direction(scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new()
                        .width(4)
                        .scroller_width(4),
                ))
                .height(150);

                let collapse_btn = button(
                    text(locale.get(Key::CollapseDescription))
                        .size(theme::TEXT_SIZE_CAPTION)
                        .style(|_theme| text::Style {
                            color: Some(theme::ACCENT_PINK),
                        }),
                )
                .style(theme::transparent_btn)
                .padding(Padding::new(2.0).left(0.0))
                .on_press(Message::ToggleDescriptionExpand);

                column![scrollable_desc, collapse_btn]
                    .spacing(2)
                    .width(Fill)
                    .into()
            } else {
                // Collapsed: clamped to 2 lines with "展开" button
                let clamped_desc = container(desc_widget)
                    .max_height(theme::TEXT_SIZE_BODY_LARGE * 1.5 * 2.0 + 4.0)
                    .clip(true)
                    .width(Fill);

                let expand_btn = button(
                    text(locale.get(Key::ExpandDescription))
                        .size(theme::TEXT_SIZE_CAPTION)
                        .style(|_theme| text::Style {
                            color: Some(theme::ACCENT_PINK),
                        }),
                )
                .style(theme::transparent_btn)
                .padding(Padding::new(2.0).left(0.0))
                .on_press(Message::ToggleDescriptionExpand);

                column![clamped_desc, expand_btn]
                    .spacing(2)
                    .width(Fill)
                    .into()
            }
        } else {
            desc_widget.into()
        }
    } else {
        text("").size(theme::TEXT_SIZE_BODY_LARGE).into()
    };

    // Owner avatar - use real avatar if available, otherwise show first letter
    let owner_name = playlist.owner.clone();
    let owner_avatar: Element<'static, Message> =
        if let Some(handle) = playlist_owner_avatar_handle(playlist, image_state) {
            crate::ui::components::cover_image::circle(
                Some(handle),
                crate::image::ImageKind::UserAvatar,
                24.0,
            )
        } else {
            build_owner_avatar_placeholder(&owner_name)
        };

    // Owner and stats - better spacing and brighter colors
    let song_count = playlist.song_count;
    let duration = playlist.total_duration.clone();
    let is_local = playlist.is_local;
    let like_count = playlist.like_count.clone();
    let owner_artist_id = playlist.owner_artist_id;

    // Build stats row - use proper dot separator with spacing
    let owner_label = text(owner_name.clone())
        .size(theme::TEXT_SIZE_BODY)
        .style(|theme| text::Style {
            color: Some(theme::text_primary(theme)),
        })
        .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT));

    let owner_action =
        if playlist.kind == DetailPageKind::Playlist && !is_local && playlist.creator_id != 0 {
            Some(Message::OpenUser(playlist.creator_id))
        } else {
            owner_artist_id.map(Message::OpenArtist)
        };

    let owner_info: Element<'static, Message> = if let Some(action) = owner_action {
        container(
            button(
                row![owner_avatar, Space::new().width(8), owner_label].align_y(Alignment::Center),
            )
            .padding(Padding::new(4.0).left(0.0).right(8.0))
            .style(|_theme, _status| button::Style {
                background: Some(iced::Background::Color(Color::TRANSPARENT)),
                border: iced::Border {
                    radius: 999.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .on_press(action),
        )
        .into()
    } else {
        row![owner_avatar, Space::new().width(8), owner_label]
            .align_y(Alignment::Center)
            .into()
    };

    let mut stats_items: Vec<Element<'static, Message>> = vec![owner_info];

    // Only show like count for non-local playlists
    if !is_local && !like_count.is_empty() {
        stats_items.push(Space::new().width(6).into());
        stats_items.push(
            text("·")
                .size(theme::TEXT_SIZE_BODY)
                .style(|theme| text::Style {
                    color: Some(theme::header_text(theme)),
                })
                .into(),
        );
        stats_items.push(Space::new().width(6).into());
        stats_items.push(
            text(format!("{} likes", like_count))
                .size(theme::TEXT_SIZE_BODY)
                .style(|theme| text::Style {
                    color: Some(theme::text_secondary(theme)),
                })
                .into(),
        );
    }

    // Song count and duration - brighter, with proper spacing
    stats_items.push(Space::new().width(6).into());
    stats_items.push(
        text("·")
            .size(theme::TEXT_SIZE_BODY)
            .style(|theme| text::Style {
                color: Some(theme::header_text(theme)),
            })
            .into(),
    );
    stats_items.push(Space::new().width(6).into());
    stats_items.push(
        text(
            locale
                .get(Key::PlaylistSongCount)
                .replace("{}", &song_count.to_string()),
        )
        .size(theme::TEXT_SIZE_BODY)
        .style(|theme| text::Style {
            color: Some(theme::text_secondary(theme)),
        })
        .into(),
    );
    stats_items.push(Space::new().width(6).into());
    stats_items.push(
        text("·")
            .size(theme::TEXT_SIZE_BODY)
            .style(|theme| text::Style {
                color: Some(theme::header_text(theme)),
            })
            .into(),
    );
    stats_items.push(Space::new().width(6).into());
    stats_items.push(
        text(duration)
            .size(theme::TEXT_SIZE_BODY)
            .style(|theme| text::Style {
                color: Some(theme::text_secondary(theme)),
            })
            .into(),
    );

    let stats = row(stats_items).align_y(Alignment::Center);

    // Info column - description closer to title, farther from stats
    let info = column![
        type_label,
        Space::new().height(12),
        title,
        Space::new().height(6),
        description,
        Space::new().height(12),
        stats,
    ]
    .spacing(0);

    // Header row - align to bottom of cover
    row![cover, Space::new().width(28), info,]
        .align_y(Alignment::End)
        .padding(Padding::new(36.0).top(60.0).bottom(12.0))
        .into()
}

fn playlist_page_cover_handle<'a>(
    playlist: &PlaylistView,
    image_state: &'a ImageState,
) -> Option<&'a iced::widget::image::Handle> {
    if playlist.is_local {
        return u64::try_from(playlist.id)
            .ok()
            .and_then(|id| image_state.get(crate::image::ImageKind::LocalPlaylistCover, id));
    }

    match playlist.kind {
        DetailPageKind::Playlist => playlist
            .id
            .checked_neg()
            .and_then(|id| u64::try_from(id).ok())
            .and_then(|id| image_state.get(crate::image::ImageKind::PlaylistCover, id)),
        DetailPageKind::Album => playlist
            .id
            .checked_sub(i64::MIN / 4)
            .and_then(|id| u64::try_from(id).ok())
            .and_then(|id| image_state.get(crate::image::ImageKind::AlbumCover, id)),
        DetailPageKind::Artist => playlist
            .id
            .checked_sub(i64::MIN)
            .and_then(|id| u64::try_from(id).ok())
            .and_then(|id| image_state.get(crate::image::ImageKind::ArtistCover, id)),
        DetailPageKind::User => playlist
            .owner_artist_id
            .and_then(|id| image_state.get(crate::image::ImageKind::ArtistCover, id)),
    }
}

fn playlist_header_cover_handle<'a>(
    playlist: &PlaylistView,
    image_state: &'a ImageState,
) -> Option<&'a iced::widget::image::Handle> {
    playlist_page_cover_handle(playlist, image_state).or_else(|| {
        playlist.songs.first().and_then(|song| {
            let (kind, id) = crate::image::song_cover_key(song.id)?;
            image_state.get(kind, id)
        })
    })
}

fn playlist_owner_avatar_handle<'a>(
    playlist: &PlaylistView,
    image_state: &'a ImageState,
) -> Option<&'a iced::widget::image::Handle> {
    if playlist.kind == DetailPageKind::Playlist && playlist.creator_id != 0 {
        return image_state.get(crate::image::ImageKind::UserAvatar, playlist.creator_id);
    }
    playlist
        .owner_artist_id
        .and_then(|id| image_state.get(crate::image::ImageKind::ArtistCover, id))
}

/// Build the control buttons (play, like, download, etc.)
pub(crate) fn build_controls<'a>(
    playlist: &PlaylistView,
    icon_animations: &crate::ui::animation::HoverAnimations<crate::app::IconId>,
    search_animation: &crate::ui::animation::SingleHoverAnimation,
    search_expanded: bool,
    search_query: &str,
    locale: Locale,
    current_user_id: Option<u64>,
) -> Element<'a, Message> {
    use crate::app::IconId;

    let is_local = playlist.is_local;
    let is_artist = playlist.kind == DetailPageKind::Artist;
    let is_album = playlist.kind == DetailPageKind::Album;
    let is_user = playlist.kind == DetailPageKind::User;
    let playlist_id = playlist.id;
    let is_own_playlist = current_user_id == Some(playlist.creator_id);
    let is_subscribed = playlist.is_subscribed;

    // Helper to get icon color based on animation (using gray levels instead of opacity)
    let get_icon_color = |icon_id: IconId| -> Color {
        let base = 0.5_f32; // Default dimmed (gray)
        let bright = 1.0_f32; // Hover bright (white)
        let value = icon_animations.interpolate_f32(&icon_id, base, bright);
        Color::from_rgb(value, value, value)
    };

    // Play button with hover scale animation
    // Base sizes
    let base_btn_size = 52.0_f32;
    let base_icon_size = 22.0_f32;
    let container_size = 56.0_f32; // Fixed outer container, slightly larger than max button size

    // Scale factor: 1.0 -> 1.06 on hover (3px growth: 52 -> 55)
    let scale = icon_animations.interpolate_f32(&IconId::PlayButton, 1.0, 1.06);
    let btn_size = base_btn_size * scale;
    let icon_size = base_icon_size * scale;
    let btn_radius = btn_size / 2.0;

    // Color: lighter pink -> slightly lighter on hover
    let progress = icon_animations.get_progress(&IconId::PlayButton);
    let play_bg = Color::from_rgb(
        1.0,
        0.412 + (0.494 - 0.412) * progress,
        0.706 + (0.753 - 0.706) * progress,
    );

    // Build from inside out:
    // 1. SVG icon
    // 2. Inner container with rounded pink background (scales with animation)
    // 3. Fixed outer container to prevent layout shift
    // 4. mouse_area for hover + click
    let inner_padding = (btn_size - icon_size) / 2.0;
    let offset = 2.0 * scale; // Triangle visual offset, scales with button

    let play_btn = mouse_area(
        container(
            button(
                container(
                    svg(svg::Handle::from_memory(icons::PLAY.as_bytes()))
                        .width(icon_size)
                        .height(icon_size)
                        .style(|_theme, _status| svg::Style {
                            color: Some(theme::BLACK),
                        }),
                )
                .padding(Padding {
                    top: inner_padding,
                    bottom: inner_padding,
                    left: inner_padding + offset,
                    right: inner_padding - offset,
                }),
            )
            .padding(0)
            .width(btn_size)
            .height(btn_size)
            .style(move |_theme, _status| button::Style {
                background: Some(iced::Background::Color(play_bg)),
                border: iced::Border {
                    radius: btn_radius.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .on_press(Message::PlayPlaylist(playlist_id)),
        )
        .width(container_size)
        .height(container_size)
        .center_x(container_size)
        .center_y(container_size),
    )
    .on_enter(Message::HoverIcon(Some(IconId::PlayButton)))
    .on_exit(Message::HoverIcon(None));

    // Sort button with animated color
    let sort_color = get_icon_color(IconId::Sort);
    let sort_btn = mouse_area(
        button(
            row![
                text(locale.get(Key::PlaylistCustomSort))
                    .size(theme::TEXT_SIZE_BODY)
                    .color(sort_color),
                Space::new().width(6),
                svg(svg::Handle::from_memory(icons::LIST.as_bytes()))
                    .width(18)
                    .height(18)
                    .style(move |_theme, _status| svg::Style {
                        color: Some(sort_color),
                    })
            ]
            .align_y(Alignment::Center),
        )
        .style(theme::transparent_btn)
        .on_press(Message::PlayHero),
    )
    .on_enter(Message::HoverIcon(Some(IconId::Sort)))
    .on_exit(Message::HoverIcon(None));

    // Build controls row
    let mut control_items: Vec<Element<'a, Message>> =
        vec![play_btn.into(), Space::new().width(24).into()];

    if is_artist || is_user || is_album {
        // Artist page keeps only play and search controls.
    } else if is_local && playlist_id != -1 {
        // For local playlists (but not recently played), show edit button with animated color
        let edit_color = get_icon_color(IconId::Edit);
        let edit_btn = mouse_area(
            button(
                svg(svg::Handle::from_memory(icons::EDIT.as_bytes()))
                    .width(24)
                    .height(24)
                    .style(move |_theme, _status| svg::Style {
                        color: Some(edit_color),
                    }),
            )
            .style(theme::transparent_btn)
            .on_press(Message::EditPlaylist(playlist_id)),
        )
        .on_enter(Message::HoverIcon(Some(IconId::Edit)))
        .on_exit(Message::HoverIcon(None));

        control_items.push(edit_btn.into());

        // Delete button for local playlists
        control_items.push(Space::new().width(8).into());
        let delete_color = get_icon_color(IconId::Delete);
        let delete_btn = mouse_area(
            button(
                svg(svg::Handle::from_memory(icons::TRASH.as_bytes()))
                    .width(22)
                    .height(22)
                    .style(move |_theme, _status| svg::Style {
                        color: Some(delete_color),
                    }),
            )
            .style(theme::transparent_btn)
            .on_press(Message::RequestDeletePlaylist(playlist_id)),
        )
        .on_enter(Message::HoverIcon(Some(IconId::Delete)))
        .on_exit(Message::HoverIcon(None));

        control_items.push(delete_btn.into());
    } else if !is_local {
        // For cloud playlists, show like button only if not own playlist
        if !is_own_playlist {
            let like_color = if is_subscribed {
                // Subscribed: show pink color
                theme::ACCENT_PINK
            } else {
                get_icon_color(IconId::Like)
            };
            let heart_icon = if is_subscribed {
                icons::HEART
            } else {
                icons::HEART_OUTLINE
            };
            let like_btn = mouse_area(
                button(
                    svg(svg::Handle::from_memory(heart_icon.as_bytes()))
                        .width(26)
                        .height(26)
                        .style(move |_theme, _status| svg::Style {
                            color: Some(like_color),
                        }),
                )
                .style(theme::transparent_btn)
                .on_press(Message::TogglePlaylistSubscribe(playlist_id)),
            )
            .on_enter(Message::HoverIcon(Some(IconId::Like)))
            .on_exit(Message::HoverIcon(None));

            control_items.push(like_btn.into());
            control_items.push(Space::new().width(16).into());
        }

        let download_color = get_icon_color(IconId::Download);
        let download_btn = mouse_area(
            button(
                svg(svg::Handle::from_memory(icons::DOWNLOAD.as_bytes()))
                    .width(24)
                    .height(24)
                    .style(move |_theme, _status| svg::Style {
                        color: Some(download_color),
                    }),
            )
            .style(theme::transparent_btn)
            .on_press(Message::RequestDownloadPlaylist(
                playlist_id,
                playlist.name.clone(),
                playlist.song_count,
            )),
        )
        .on_enter(Message::HoverIcon(Some(IconId::Download)))
        .on_exit(Message::HoverIcon(None));

        control_items.push(download_btn.into());
    }

    control_items.push(Space::new().width(Fill).into());

    // Animated search component - expands from right to left
    let search_progress = search_animation.progress();
    let search_color = get_icon_color(IconId::Search);

    // Animation: width goes from 36 (just icon) to 250 (full input)
    let min_width = 36.0_f32;
    let max_width = 250.0_f32;
    let current_width = min_width + (max_width - min_width) * search_progress;

    // Input opacity: fade in as it expands
    let input_opacity = search_progress;

    let search_query_owned = search_query.to_string();

    let search_component: Element<'a, Message> = if search_expanded || search_progress > 0.01 {
        // Expanded or animating - show input with search icon
        let search_icon = button(
            svg(svg::Handle::from_memory(icons::SEARCH.as_bytes()))
                .width(18)
                .height(18)
                .style(move |_theme, _status| svg::Style {
                    color: Some(search_color),
                }),
        )
        .style(theme::transparent_btn)
        .padding(0)
        .on_press(Message::TogglePlaylistSearch);

        // Text input - only show when animation is far enough
        let input_element: Element<'a, Message> = if search_progress > 0.3 {
            text_input("", &search_query_owned)
                .id(iced::widget::Id::new("playlist_search_input"))
                .on_input(Message::PlaylistSearchChanged)
                .on_submit(Message::PlaylistSearchSubmit)
                .padding(Padding::new(8.0).left(0.0).right(8.0))
                .size(theme::TEXT_SIZE_BODY)
                .width(Fill)
                .style(move |_theme, _status| text_input::Style {
                    background: iced::Background::Color(Color::TRANSPARENT),
                    border: iced::Border::default(),
                    icon: Color::from_rgba(1.0, 1.0, 1.0, input_opacity),
                    placeholder: Color::from_rgba(1.0, 1.0, 1.0, 0.5 * input_opacity),
                    value: Color::from_rgba(1.0, 1.0, 1.0, input_opacity),
                    selection: theme::ACCENT_PINK,
                })
                .into()
        } else {
            Space::new().width(Fill).into()
        };

        let search_row =
            row![search_icon, Space::new().width(8), input_element,].align_y(Alignment::Center);

        // Container with animated width and rounded background
        let bg_alpha = 0.15 * search_progress;
        let search_container = container(search_row)
            .width(current_width)
            .height(36)
            .padding(Padding::new(0.0).left(8.0).right(4.0))
            .center_y(36)
            .style(move |_theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    1.0, 1.0, 1.0, bg_alpha,
                ))),
                border: iced::Border {
                    radius: 18.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        // Wrap in mouse_area to detect when mouse leaves - triggers blur check
        mouse_area(search_container)
            .on_exit(Message::PlaylistSearchBlur)
            .into()
    } else {
        // Collapsed - just show search button
        mouse_area(
            button(
                svg(svg::Handle::from_memory(icons::SEARCH.as_bytes()))
                    .width(20)
                    .height(20)
                    .style(move |_theme, _status| svg::Style {
                        color: Some(search_color),
                    }),
            )
            .style(theme::transparent_btn)
            .on_press(Message::TogglePlaylistSearch),
        )
        .on_enter(Message::HoverIcon(Some(IconId::Search)))
        .on_exit(Message::HoverIcon(None))
        .into()
    };

    control_items.push(search_component);

    if !is_artist && !is_album {
        control_items.push(Space::new().width(20).into());
        control_items.push(sort_btn.into());
    }

    let controls = row(control_items)
        .align_y(Alignment::Center)
        .padding(Padding::new(16.0).left(36.0).right(36.0));

    // No background - gradient continues from header
    container(controls).width(Fill).into()
}

/// Build owner avatar placeholder (first letter on pink background)
fn build_owner_avatar_placeholder(owner_name: &str) -> Element<'static, Message> {
    let first_char = owner_name.chars().next().unwrap_or('R');
    container(
        text(first_char.to_string())
            .size(theme::TEXT_SIZE_MICRO)
            .color(theme::BLACK)
            .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT)),
    )
    .width(24)
    .height(24)
    .center_x(24)
    .center_y(24)
    .style(|_theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::ACCENT_PINK_HOVER)),
        border: iced::Border {
            radius: 12.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}
