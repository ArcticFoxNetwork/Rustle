//! Discovery-first home page.

use iced::widget::{Space, column, container, row, text};
use iced::{Color, Element, Fill, Length};

use crate::api::PRIVATE_RADAR_PLAYLIST_ID;
use crate::app::{ContentWidthTarget, DiscoverPageState, DiscoverViewMode, ImageState, Message};
use crate::i18n::{Key, Locale};
use crate::image::ImageKind;
use crate::ui::components::{feature_card, playlist_grid};
use crate::ui::theme;
use crate::ui::widgets::{self, section_header};

const DAILY_FEATURE_ID: u64 = 0;
const PERSONAL_FM_FEATURE_ID: u64 = u64::MAX;
const FEATURE_ROW_BREAKPOINT: f32 = 760.0;
const TOP_OVERLAY_SAFE_AREA: f32 = theme::TOP_BAR_HEIGHT + 4.0;

fn personal_fm_action() -> Message {
    Message::Navigate(crate::ui::components::NavItem::Radio)
}

pub fn view<'a>(
    state: &'a DiscoverPageState,
    image_state: &'a ImageState,
    locale: Locale,
    active_personal_fm_cover: Option<&'a iced::widget::image::Handle>,
) -> Element<'a, Message> {
    match state.view_mode {
        DiscoverViewMode::Overview => {
            view_overview(state, image_state, locale, active_personal_fm_cover)
        }
        DiscoverViewMode::AllRecommended => view_all_playlists(
            state,
            image_state,
            locale,
            Key::DiscoverRecommended,
            &state.recommended_playlists,
        ),
        DiscoverViewMode::AllHot => view_all_playlists(
            state,
            image_state,
            locale,
            Key::DiscoverHot,
            &state.hot_playlists,
        ),
        DiscoverViewMode::AllOfficial => view_all_playlists(
            state,
            image_state,
            locale,
            Key::DiscoverOfficialPicks,
            &state.official_playlists,
        ),
    }
}

fn view_overview<'a>(
    state: &'a DiscoverPageState,
    image_state: &'a ImageState,
    locale: Locale,
    active_personal_fm_cover: Option<&'a iced::widget::image::Handle>,
) -> Element<'a, Message> {
    let content_width = state.content_width;
    let feature_row = personal_feature_row(
        state,
        image_state,
        locale,
        content_width,
        active_personal_fm_cover,
    );

    let content = column![
        Space::new().height(TOP_OVERLAY_SAFE_AREA),
        feature_row,
        Space::new().height(40),
        section_header::view(
            locale.get(Key::DiscoverRecommended),
            locale.get(Key::DiscoverSeeAll),
            Some(Message::SeeAllRecommended),
        ),
        Space::new().height(16),
        playlist_grid::view_single_row(
            &state.recommended_playlists,
            image_state,
            &state.card_animations,
            content_width,
        ),
        Space::new().height(40),
        section_header::view(
            locale.get(Key::DiscoverHot),
            locale.get(Key::DiscoverSeeAll),
            Some(Message::SeeAllHot),
        ),
        Space::new().height(16),
        playlist_grid::view_single_row(
            &state.hot_playlists,
            image_state,
            &state.card_animations,
            content_width,
        ),
        Space::new().height(40),
        section_header::view(
            locale.get(Key::DiscoverOfficialPicks),
            locale.get(Key::DiscoverSeeAll),
            Some(Message::SeeAllOfficial),
        ),
        Space::new().height(16),
        playlist_grid::view_single_row(
            &state.official_playlists,
            image_state,
            &state.card_animations,
            content_width,
        ),
        Space::new().height(40),
    ]
    .padding(32);

    container(widgets::measured_scrollable(
        content,
        "discover_scroll",
        |size| Message::ContentWidthResized(ContentWidthTarget::Discover, size),
        Message::SmoothScroll,
    ))
    .width(Fill)
    .height(Fill)
    .style(theme::main_content)
    .into()
}

fn personal_feature_row<'a>(
    state: &'a DiscoverPageState,
    image_state: &'a ImageState,
    locale: Locale,
    content_width: f32,
    active_personal_fm_cover: Option<&'a iced::widget::image::Handle>,
) -> Element<'a, Message> {
    let day = chrono::Local::now().format("%d").to_string();
    let daily = feature_card::view(
        locale.get(Key::DiscoverDailyRecommend).to_string(),
        locale.get(Key::DiscoverDailyRecommendDesc).to_string(),
        Some(day),
        crate::ui::icons::CALENDAR,
        state
            .daily_recommend_preview
            .as_ref()
            .and_then(|track| image_state.get(ImageKind::SongCover, track.id)),
        (
            Color::from_rgb(0.92, 0.25, 0.43),
            Color::from_rgb(0.48, 0.18, 0.62),
        ),
        feature_width(content_width),
        state.card_animations.get_progress(&DAILY_FEATURE_ID),
        Message::OpenNcmPlaylist(DAILY_FEATURE_ID),
        Message::PlayDiscoverPlaylist(DAILY_FEATURE_ID),
        Message::HoverDiscoverPlaylist(Some(DAILY_FEATURE_ID)),
        Message::HoverDiscoverPlaylist(None),
    );

    let radar = state.private_radar.as_ref();
    let radar_title = radar
        .map(|playlist| playlist.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| locale.get(Key::DiscoverPrivateRadar).to_string());
    let radar_subtitle = locale.get(Key::DiscoverPrivateRadarDesc).to_string();
    let radar = feature_card::view(
        radar_title,
        radar_subtitle,
        None,
        crate::ui::icons::BROWSE,
        image_state.get(ImageKind::PlaylistCover, PRIVATE_RADAR_PLAYLIST_ID),
        (
            Color::from_rgb(0.18, 0.35, 0.63),
            Color::from_rgb(0.42, 0.18, 0.55),
        ),
        feature_width(content_width),
        state
            .card_animations
            .get_progress(&PRIVATE_RADAR_PLAYLIST_ID),
        Message::OpenNcmPlaylist(PRIVATE_RADAR_PLAYLIST_ID),
        Message::PlayDiscoverPlaylist(PRIVATE_RADAR_PLAYLIST_ID),
        Message::HoverDiscoverPlaylist(Some(PRIVATE_RADAR_PLAYLIST_ID)),
        Message::HoverDiscoverPlaylist(None),
    );

    let personal_fm_action = personal_fm_action();
    let personal_fm = feature_card::view(
        locale.get(Key::DiscoverPersonalFm).to_string(),
        locale.get(Key::DiscoverPersonalFmDesc).to_string(),
        None,
        crate::ui::icons::RADIO,
        active_personal_fm_cover.or_else(|| {
            state
                .personal_fm_preview
                .as_ref()
                .and_then(|track| image_state.get(ImageKind::SongCover, track.id))
        }),
        (
            Color::from_rgb(0.16, 0.46, 0.59),
            Color::from_rgb(0.29, 0.19, 0.55),
        ),
        feature_width(content_width),
        state.card_animations.get_progress(&PERSONAL_FM_FEATURE_ID),
        personal_fm_action.clone(),
        personal_fm_action,
        Message::HoverDiscoverPlaylist(Some(PERSONAL_FM_FEATURE_ID)),
        Message::HoverDiscoverPlaylist(None),
    );

    if content_width >= FEATURE_ROW_BREAKPOINT {
        row![daily, radar, personal_fm]
            .spacing(18)
            .width(Fill)
            .into()
    } else {
        column![daily, radar, personal_fm]
            .spacing(16)
            .width(Fill)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::personal_fm_action;
    use crate::app::Message;
    use crate::ui::components::NavItem;

    #[test]
    fn personal_fm_card_reuses_sidebar_navigation_action() {
        assert!(matches!(
            personal_fm_action(),
            Message::Navigate(NavItem::Radio)
        ));
    }
}

fn feature_width(content_width: f32) -> Length {
    if content_width >= FEATURE_ROW_BREAKPOINT {
        Length::FillPortion(1)
    } else {
        Length::Fill
    }
}

fn view_all_playlists<'a>(
    state: &'a DiscoverPageState,
    image_state: &'a ImageState,
    locale: Locale,
    title: Key,
    playlists: &'a [crate::api::PlaylistSummary],
) -> Element<'a, Message> {
    let content = column![
        Space::new().height(TOP_OVERLAY_SAFE_AREA),
        text(locale.get(title)).size(theme::TEXT_SIZE_TITLE),
        Space::new().height(24),
        playlist_grid::view(
            playlists,
            image_state,
            &state.card_animations,
            None,
            state.content_width,
        ),
        Space::new().height(40),
    ]
    .padding(32);

    container(widgets::measured_scrollable(
        content,
        "discover_scroll",
        |size| Message::ContentWidthResized(ContentWidthTarget::Discover, size),
        Message::SmoothScroll,
    ))
    .width(Fill)
    .height(Fill)
    .style(theme::main_content)
    .into()
}
