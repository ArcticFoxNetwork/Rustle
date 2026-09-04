//! Bottom player bar component

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{
    Space, button, column, container, opaque, responsive, rich_text, row, span, svg, text,
};
use iced::{Alignment, Color, Element, Fill, Length, Padding, Shadow, Vector, mouse};

use crate::api::ArtistSummary;
use crate::app::Message;
use crate::database::DbSong;
use crate::features::PlayMode;
use crate::ui::responsive::{ChromeRole, IconRole, ResponsiveContext, TextRole, UiTokens};
use crate::ui::theme::BOLD_WEIGHT;
use crate::ui::widgets::{self, ControlSize, PlayModeButtonSize, SliderSize};
use crate::ui::{icons, theme};
use crate::utils;

use super::playback_controls::play_mode_button;

// Visual dimensions are 1080P reference pixels resolved through `UiTokens`.
const CONTENT_HORIZONTAL_PADDING: f32 = 32.0;
const SECTION_SPACING: f32 = 16.0;
const CENTER_CONTROLS_WIDTH: f32 = 216.0;
const LEFT_MAX_WIDTH: f32 = 420.0;
const LEFT_MIN_WIDTH: f32 = 140.0;
const TIME_WIDTH: f32 = 84.0;
const HORIZONTAL_VOLUME_MAX_WIDTH: f32 = 100.0;
const HORIZONTAL_VOLUME_MIN_WIDTH: f32 = 50.0;
const RIGHT_HORIZONTAL_FIXED_WIDTH: f32 = 168.0;
const RIGHT_PREFERRED_WIDTH: f32 = RIGHT_HORIZONTAL_FIXED_WIDTH + HORIZONTAL_VOLUME_MAX_WIDTH;
const RIGHT_HORIZONTAL_MIN_WIDTH: f32 = RIGHT_HORIZONTAL_FIXED_WIDTH + HORIZONTAL_VOLUME_MIN_WIDTH;
const RIGHT_VERTICAL_WITH_TIME_WIDTH: f32 = 180.0;
const RIGHT_VERTICAL_MIN_WIDTH: f32 = 88.0;
const VERTICAL_VOLUME_SLIDER_HEIGHT: f32 = 96.0;

const COVER_EXPAND_CHEVRON: &str = r#"<svg viewBox="0 0 24 12" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M4 8.5L12 5L20 8.5"/>
</svg>"#;

#[derive(Debug, Clone, Copy, PartialEq)]
enum VolumeLayout {
    Horizontal { width: f32 },
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PlayerBarLayout {
    left_width: f32,
    right_width: f32,
    show_quality: bool,
    show_time: bool,
    volume: VolumeLayout,
}

impl PlayerBarLayout {
    fn for_width(total_width: f32, tokens: UiTokens) -> Self {
        let fixed_chrome = tokens.space(CONTENT_HORIZONTAL_PADDING + SECTION_SPACING * 2.0);
        let center_controls_width = tokens.size(CENTER_CONTROLS_WIDTH);
        let left_max_width = tokens.size(LEFT_MAX_WIDTH);
        let left_min_width = tokens.size(LEFT_MIN_WIDTH);
        let right_preferred_width = tokens.size(RIGHT_PREFERRED_WIDTH);
        let right_vertical_min_width = tokens.size(RIGHT_VERTICAL_MIN_WIDTH);
        let right_horizontal_min_width = tokens.size(RIGHT_HORIZONTAL_MIN_WIDTH);
        let right_vertical_with_time_width = tokens.size(RIGHT_VERTICAL_WITH_TIME_WIDTH);
        let horizontal_volume_min_width = tokens.size(HORIZONTAL_VOLUME_MIN_WIDTH);
        let horizontal_volume_max_width = tokens.size(HORIZONTAL_VOLUME_MAX_WIDTH);
        let right_horizontal_fixed_width = tokens.size(RIGHT_HORIZONTAL_FIXED_WIDTH);
        let side_width = (total_width - fixed_chrome - center_controls_width).max(0.0);

        let (left_width, right_width) = if side_width >= left_min_width + right_preferred_width {
            (
                (side_width - right_preferred_width).min(left_max_width),
                right_preferred_width,
            )
        } else if side_width >= left_min_width + right_vertical_min_width {
            (left_min_width, side_width - left_min_width)
        } else {
            let right_width = side_width.min(right_vertical_min_width);
            (side_width - right_width, right_width)
        };

        let (show_time, volume) = if right_width >= right_horizontal_min_width {
            (
                true,
                VolumeLayout::Horizontal {
                    width: (right_width - right_horizontal_fixed_width)
                        .clamp(horizontal_volume_min_width, horizontal_volume_max_width),
                },
            )
        } else {
            (
                right_width >= right_vertical_with_time_width,
                VolumeLayout::Vertical,
            )
        };

        Self {
            left_width,
            right_width,
            show_quality: left_width >= left_max_width,
            show_time,
            volume,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ArtistTarget {
    Id(u64),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtistLink {
    name: String,
    target: ArtistTarget,
}

fn artist_links(artist_text: &str, structured_artists: &[ArtistSummary]) -> Vec<ArtistLink> {
    let mut links = Vec::new();

    if !structured_artists.is_empty() {
        for artist in structured_artists {
            let name = artist.name.trim();
            if name.is_empty() {
                continue;
            }

            links.push(ArtistLink {
                name: name.to_string(),
                target: if artist.id == 0 {
                    ArtistTarget::Name(name.to_string())
                } else {
                    ArtistTarget::Id(artist.id)
                },
            });
        }
    } else {
        for name in artist_text
            .split('/')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            if links.iter().any(|link: &ArtistLink| link.name == name) {
                continue;
            }

            links.push(ArtistLink {
                name: name.to_string(),
                target: ArtistTarget::Name(name.to_string()),
            });
        }
    }

    if links.is_empty() {
        let name = artist_text.trim();
        if !name.is_empty() {
            links.push(ArtistLink {
                name: name.to_string(),
                target: ArtistTarget::Name(name.to_string()),
            });
        }
    }

    links
}

/// Build the player bar
pub fn view(
    context: ResponsiveContext,
    current_song: Option<&DbSong>,
    current_artists: &[ArtistSummary],
    is_playing: bool,
    position: f32, // 0.0 to 1.0
    duration_secs: f32,
    volume: f32, // 0.0 to 1.0
    play_mode: PlayMode,
    current_favorite: Option<(u64, bool)>,
    progress_colors: Option<[Color; 3]>,
    is_buffering: bool,             // Whether streaming is buffering
    download_progress: Option<f32>, // Download progress 0.0 to 1.0 (None if not streaming)
    is_fm_mode: bool,               // Whether in Personal FM mode
    is_first_song: bool,            // Whether at first song in queue
    current_song_cover: Option<&iced::widget::image::Handle>,
    current_quality: Option<&crate::app::ResolvedAudioQuality>,
) -> Element<'static, Message> {
    let current_time = utils::format_time(position * duration_secs);
    let total_time = utils::format_time(duration_secs);

    let current_song = current_song.cloned();
    let current_artists = current_artists.to_vec();
    let current_song_cover = current_song_cover.cloned();
    let current_quality = current_quality.cloned();
    let tokens = context.tokens;
    let bar_height = tokens.chrome(ChromeRole::PlayerBar);

    let body = responsive(move |size| {
        build_body(
            current_song.as_ref(),
            &current_artists,
            is_playing,
            volume,
            play_mode,
            current_favorite,
            is_buffering,
            is_fm_mode,
            is_first_song,
            current_song_cover.as_ref(),
            current_quality.as_ref(),
            &current_time,
            &total_time,
            tokens,
            PlayerBarLayout::for_width(size.width, tokens),
        )
    })
    .width(Fill)
    .height(Fill);

    let progress_bar_height = tokens.size(8.0);
    let top_progress = container(widgets::progress_slider::view(
        position,
        download_progress,
        SliderSize::Edge,
        progress_colors,
        tokens,
        Message::SeekPreview,
        Message::SeekRelease,
    ))
    .width(Fill)
    .height(progress_bar_height)
    .style(|theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(theme::player_bar_bg(theme))),
        ..Default::default()
    });

    let main_content = container(body)
        .width(Fill)
        .height(bar_height)
        .padding(Padding::new(0.0).top(progress_bar_height))
        .align_y(Alignment::Center)
        .style(move |theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::player_bar_bg(theme))),
            ..Default::default()
        });

    // Draw the progress slider after the player bar body so its hover handle
    // can extend below the rail without being covered by the body background.
    let bar = iced::widget::stack![main_content, top_progress]
        .width(Fill)
        .height(bar_height);

    // Use opaque to block events from reaching underlying widgets without swallowing
    // interactions inside the player bar itself.
    opaque(bar)
}

#[allow(clippy::too_many_arguments)]
fn build_body(
    current_song: Option<&DbSong>,
    current_artists: &[ArtistSummary],
    is_playing: bool,
    volume: f32,
    play_mode: PlayMode,
    current_favorite: Option<(u64, bool)>,
    is_buffering: bool,
    is_fm_mode: bool,
    is_first_song: bool,
    current_song_cover: Option<&iced::widget::image::Handle>,
    current_quality: Option<&crate::app::ResolvedAudioQuality>,
    current_time: &str,
    total_time: &str,
    tokens: UiTokens,
    layout: PlayerBarLayout,
) -> Element<'static, Message> {
    let song_info = build_song_info(
        current_song,
        current_artists,
        current_song_cover,
        current_quality,
        layout.show_quality,
        tokens,
    );

    let left_section = container(song_info)
        .width(layout.left_width)
        .align_y(Alignment::Center)
        .clip(true);

    // The responsive allocator protects this lane before either side is
    // allowed to consume it.
    let mode_button = play_mode_button(play_mode, PlayModeButtonSize::Small, tokens, is_fm_mode);
    let prev_action = (!is_first_song || !is_fm_mode).then_some(Message::PrevSong);
    let favorite_state = current_favorite.map(|(_, liked)| liked);
    let favorite_action = current_favorite.map(|(song_id, _)| Message::ToggleFavorite(song_id));
    let controls = widgets::playback_controls::view_player_bar(
        is_playing,
        is_buffering,
        ControlSize::Small,
        tokens,
        is_fm_mode && is_first_song,
        mode_button,
        prev_action,
        Message::TogglePlayback,
        Message::NextSong,
        favorite_state,
        favorite_action,
    );

    let center_section = container(controls)
        .width(Length::Fill)
        .align_x(Alignment::Center);

    let right_section = build_right_section(
        current_time,
        total_time,
        volume,
        layout.right_width,
        layout.show_time,
        layout.volume,
        tokens,
    );

    row![left_section, center_section, right_section]
        .spacing(tokens.space(SECTION_SPACING))
        .align_y(Alignment::Center)
        .padding(
            Padding::new(0.0)
                .left(tokens.space(16.0))
                .right(tokens.space(16.0)),
        )
        .width(Fill)
        .height(Fill)
        .into()
}

fn build_song_info(
    current_song: Option<&DbSong>,
    current_artists: &[ArtistSummary],
    current_song_cover: Option<&iced::widget::image::Handle>,
    current_quality: Option<&crate::app::ResolvedAudioQuality>,
    show_quality: bool,
    tokens: UiTokens,
) -> Element<'static, Message> {
    let Some(song) = current_song else {
        let placeholder = column![
            text("No song playing")
                .size(tokens.text(TextRole::Body))
                .width(Fill)
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme))
                }),
            text("Select a song to play")
                .size(tokens.text(TextRole::Caption))
                .width(Fill)
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme))
                }),
        ]
        .spacing(tokens.space(2.0))
        .width(Fill);

        return row![
            container(
                svg(svg::Handle::from_memory(icons::MUSIC.as_bytes()))
                    .width(tokens.icon(IconRole::Large))
                    .height(tokens.icon(IconRole::Large))
                    .style(|theme, _status| svg::Style {
                        color: Some(theme::opaque_color(theme::icon_muted(theme))),
                    })
                    .opacity(0.4_f32),
            )
            .width(tokens.size(56.0))
            .height(tokens.size(56.0))
            .center_x(tokens.size(56.0))
            .center_y(tokens.size(56.0))
            .style(move |theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme::surface_container(theme))),
                border: iced::Border {
                    radius: tokens.size(4.0).into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            Space::new().width(tokens.space(12.0)),
            placeholder
        ]
        .align_y(Alignment::Center)
        .width(Fill)
        .clip(true)
        .into();
    };

    let song = song.clone();

    // Cover - clickable to open lyrics page
    let cover_px = tokens.size(56.0);
    let cover_radius = tokens.size(8.0);
    let cover_content: Element<'static, Message> = crate::ui::components::cover_image::custom(
        current_song_cover,
        crate::image::ImageKind::SongCover,
        cover_px,
        cover_radius,
        tokens,
    );

    let expand_overlay = widgets::hover_surface(Space::new().width(cover_px).height(cover_px))
        .style(move |_theme, progress| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                0.0,
                0.0,
                0.0,
                0.58 * progress,
            ))),
            border: iced::Border {
                radius: cover_radius.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .svg_overlay(
            svg::Handle::from_memory(COVER_EXPAND_CHEVRON.as_bytes()),
            iced::Size::new(tokens.size(24.0), tokens.size(12.0)),
            Color::WHITE,
        );

    let cover_btn = button(iced::widget::stack![cover_content, expand_overlay])
        .padding(0)
        .style(|_theme, _status| button::Style {
            background: Some(iced::Background::Color(Color::TRANSPARENT)),
            ..Default::default()
        })
        .on_press(Message::OpenLyricsPage);

    let title_btn = button(
        text(song.title.clone())
            .size(tokens.text(TextRole::Body))
            .width(Fill)
            .height(tokens.size(20.0))
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme)),
            })
            .font(iced::Font::DEFAULT.weight(BOLD_WEIGHT)),
    )
    .padding(0)
    .width(Fill)
    .clip(true)
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    })
    .on_press(Message::OpenLyricsPage);

    let links = artist_links(&song.artist, current_artists);
    let mut artist_spans = Vec::with_capacity(links.len().saturating_mul(2));
    for (index, link) in links.into_iter().enumerate() {
        if index > 0 {
            artist_spans.push(span(" / "));
        }
        artist_spans.push(span(link.name).link(link.target));
    }

    // Keep quality in the same inline flow as the artists. A separate `Fill`
    // artist widget would push a short name and the quality label to opposite
    // ends of the metadata lane.
    if show_quality && let Some(quality) = current_quality {
        artist_spans.push(span(format!("  {}", quality.actual.short_name())).color(theme::ACCENT));
    }

    let artist_line: Element<'static, Message> = rich_text(artist_spans)
        .size(tokens.text(TextRole::Caption))
        .width(Fill)
        .height(tokens.size(18.0))
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
        .style(|theme| text::Style {
            color: Some(theme::text_secondary(theme)),
        })
        .on_link_click(|target| match target {
            ArtistTarget::Id(id) => Message::OpenArtist(id),
            ArtistTarget::Name(name) => Message::OpenArtistByName(name),
        })
        .into();

    let song_details = column![title_btn, artist_line]
        .spacing(tokens.space(2.0))
        .width(Fill)
        .clip(true);

    row![
        cover_btn,
        Space::new().width(tokens.space(12.0)),
        song_details
    ]
    .align_y(Alignment::Center)
    .width(Fill)
    .clip(true)
    .into()
}

fn build_right_section(
    current_time: &str,
    total_time: &str,
    volume: f32,
    right_width: f32,
    show_time: bool,
    volume_layout: VolumeLayout,
    tokens: UiTokens,
) -> Element<'static, Message> {
    let volume_area: Element<'static, Message> = match volume_layout {
        VolumeLayout::Horizontal { width } => {
            let volume_icon = volume_icon(tokens);
            iced::widget::mouse_area(
                row![
                    volume_icon,
                    Space::new().width(tokens.space(8.0)),
                    widgets::progress_slider::volume_slider(
                        volume,
                        width,
                        tokens,
                        Message::SetVolume,
                    )
                ]
                .align_y(Alignment::Center)
                .width(Length::Shrink),
            )
            .on_scroll(move |delta| Message::SetVolume(volume_after_scroll(volume, delta)))
            .into()
        }
        VolumeLayout::Vertical => {
            let anchor = iced::widget::mouse_area(
                widgets::hover_surface(
                    container(volume_icon(tokens))
                        .width(tokens.target(crate::ui::responsive::TargetRole::Icon))
                        .height(tokens.target(crate::ui::responsive::TargetRole::Icon))
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center),
                )
                .style(move |theme, progress| iced::widget::container::Style {
                    background: Some(iced::Background::Color(theme::hover_bg_alpha(
                        theme,
                        0.12 * progress,
                    ))),
                    border: iced::Border {
                        radius: (tokens.target(crate::ui::responsive::TargetRole::Icon) / 2.0)
                            .into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            )
            .on_scroll(move |delta| Message::SetVolume(volume_after_scroll(volume, delta)))
            .interaction(mouse::Interaction::Pointer);

            let popup = iced::widget::mouse_area(
                container(widgets::progress_slider::vertical_volume_slider(
                    volume,
                    tokens.size(VERTICAL_VOLUME_SLIDER_HEIGHT),
                    tokens,
                    Message::SetVolume,
                ))
                .padding(
                    Padding::new(tokens.space(10.0))
                        .left(tokens.space(12.0))
                        .right(tokens.space(12.0)),
                )
                .style(move |theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(theme::surface_elevated(theme))),
                    border: iced::Border {
                        radius: tokens.size(12.0).into(),
                        width: tokens.size(1.0),
                        color: theme::border_color(theme),
                    },
                    shadow: Shadow {
                        color: theme::shadow_color(theme),
                        offset: Vector::new(0.0, tokens.size(4.0)),
                        blur_radius: tokens.size(12.0),
                    },
                    ..Default::default()
                }),
            )
            .on_scroll(move |delta| Message::SetVolume(volume_after_scroll(volume, delta)));

            widgets::hover_popup(anchor, popup, tokens.space(8.0)).into()
        }
    };

    let queue_btn = button(
        svg(svg::Handle::from_memory(icons::QUEUE.as_bytes()))
            .width(tokens.icon(IconRole::Medium))
            .height(tokens.icon(IconRole::Medium))
            .style(|theme, _status| svg::Style {
                color: Some(theme::text_secondary(theme)),
            }),
    )
    .padding(tokens.space(8.0))
    .style(|_theme, _status| button::Style {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    })
    .on_press(Message::ToggleQueue);
    let queue_btn = widgets::hover_surface(queue_btn).style(move |theme, progress| {
        iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::hover_bg_alpha(
                theme,
                0.12 * progress,
            ))),
            border: iced::Border {
                radius: tokens.size(18.0).into(),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    let controls: Element<'static, Message> = if show_time {
        let time = container(
            text(format!("{current_time} / {total_time}"))
                .size(tokens.text(TextRole::Caption))
                .width(Fill)
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .align_x(Alignment::End)
                .style(|theme| text::Style {
                    color: Some(theme::text_muted(theme)),
                }),
        )
        .width(tokens.size(TIME_WIDTH));

        row![
            time,
            Space::new().width(tokens.space(8.0)),
            volume_area,
            Space::new().width(tokens.space(12.0)),
            queue_btn,
        ]
        .align_y(Alignment::Center)
        .width(Length::Shrink)
        .into()
    } else {
        row![
            volume_area,
            Space::new().width(tokens.space(12.0)),
            queue_btn
        ]
        .align_y(Alignment::Center)
        .width(Length::Shrink)
        .into()
    };

    container(controls)
        .width(right_width)
        .align_x(Alignment::End)
        .align_y(Alignment::Center)
        .clip(true)
        .into()
}

fn volume_icon(tokens: UiTokens) -> iced::widget::Svg<'static, iced::Theme> {
    svg(svg::Handle::from_memory(icons::VOLUME.as_bytes()))
        .width(tokens.icon(IconRole::Medium))
        .height(tokens.icon(IconRole::Medium))
        .style(|theme, _status| svg::Style {
            color: Some(theme::text_secondary(theme)),
        })
}

fn volume_after_scroll(volume: f32, delta: mouse::ScrollDelta) -> f32 {
    let delta_y = match delta {
        mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => y,
    };

    (volume + delta_y.signum() * 0.02).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{
        ArtistLink, ArtistTarget, HORIZONTAL_VOLUME_MAX_WIDTH, HORIZONTAL_VOLUME_MIN_WIDTH,
        PlayerBarLayout, RIGHT_PREFERRED_WIDTH, VolumeLayout, artist_links,
    };
    use crate::api::ArtistSummary;
    use crate::ui::responsive::UiTokens;

    #[test]
    fn player_bar_shrinks_left_before_right() {
        let tokens = UiTokens::default();
        let wide = PlayerBarLayout::for_width(1_100.0, tokens);
        let narrower = PlayerBarLayout::for_width(820.0, tokens);

        assert_eq!(wide.right_width, RIGHT_PREFERRED_WIDTH);
        assert_eq!(narrower.right_width, RIGHT_PREFERRED_WIDTH);
        assert!(wide.show_quality);
        assert!(narrower.left_width < wide.left_width);
        assert!(matches!(
            narrower.volume,
            VolumeLayout::Horizontal { width } if width == HORIZONTAL_VOLUME_MAX_WIDTH
        ));
    }

    #[test]
    fn player_bar_shrinks_horizontal_volume_then_switches_vertical() {
        let tokens = UiTokens::default();
        let shrinking = PlayerBarLayout::for_width(660.0, tokens);
        let threshold = PlayerBarLayout::for_width(638.0, tokens);
        let vertical = PlayerBarLayout::for_width(620.0, tokens);

        assert!(matches!(
            shrinking.volume,
            VolumeLayout::Horizontal { width }
                if width > HORIZONTAL_VOLUME_MIN_WIDTH
                    && width < HORIZONTAL_VOLUME_MAX_WIDTH
        ));
        assert!(matches!(
            threshold.volume,
            VolumeLayout::Horizontal { width } if width == HORIZONTAL_VOLUME_MIN_WIDTH
        ));
        assert_eq!(vertical.volume, VolumeLayout::Vertical);
        assert!(vertical.show_time);
    }

    #[test]
    fn player_bar_hides_quality_before_compressing_right_controls() {
        let layout = PlayerBarLayout::for_width(820.0, UiTokens::default());

        assert!(!layout.show_quality);
        assert_eq!(layout.right_width, RIGHT_PREFERRED_WIDTH);
    }

    #[test]
    fn structured_artists_keep_individual_ids() {
        let artists = vec![
            ArtistSummary {
                id: 12,
                name: "Artist A".to_string(),
                image_url: String::new(),
            },
            ArtistSummary {
                id: 34,
                name: "Artist B".to_string(),
                image_url: String::new(),
            },
        ];

        assert_eq!(
            artist_links("Artist A / Artist B", &artists),
            vec![
                ArtistLink {
                    name: "Artist A".to_string(),
                    target: ArtistTarget::Id(12),
                },
                ArtistLink {
                    name: "Artist B".to_string(),
                    target: ArtistTarget::Id(34),
                },
            ]
        );
    }

    #[test]
    fn structured_artists_with_the_same_name_keep_distinct_ids() {
        let artists = vec![
            ArtistSummary {
                id: 12,
                name: "Shared Name".to_string(),
                image_url: String::new(),
            },
            ArtistSummary {
                id: 34,
                name: "Shared Name".to_string(),
                image_url: String::new(),
            },
        ];

        assert_eq!(
            artist_links("Shared Name / Shared Name", &artists),
            vec![
                ArtistLink {
                    name: "Shared Name".to_string(),
                    target: ArtistTarget::Id(12),
                },
                ArtistLink {
                    name: "Shared Name".to_string(),
                    target: ArtistTarget::Id(34),
                },
            ]
        );
    }

    #[test]
    fn string_artists_are_split_trimmed_and_deduplicated() {
        assert_eq!(
            artist_links(" Artist A / Artist B / Artist A ", &[]),
            vec![
                ArtistLink {
                    name: "Artist A".to_string(),
                    target: ArtistTarget::Name("Artist A".to_string()),
                },
                ArtistLink {
                    name: "Artist B".to_string(),
                    target: ArtistTarget::Name("Artist B".to_string()),
                },
            ]
        );
    }
}
