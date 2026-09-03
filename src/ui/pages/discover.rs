//! Discovery-first home page.

use iced::widget::{Space, column, container, row, text};
use iced::{Color, Element, Fill, Length};

use crate::api::PRIVATE_RADAR_PLAYLIST_ID;
use crate::app::{ContentWidthTarget, DiscoverPageState, DiscoverViewMode, ImageState, Message};
use crate::i18n::{Key, Locale};
use crate::image::ImageKind;
use crate::ui::components::{feature_card, playlist_grid};
use crate::ui::responsive::{LayoutProfile, ResponsiveContext, TextRole, top_bar_height};
use crate::ui::theme;
use crate::ui::widgets::{self, section_header};

const DAILY_FEATURE_ID: u64 = 0;
const PERSONAL_FM_FEATURE_ID: u64 = u64::MAX;

fn personal_fm_action() -> Message {
    Message::Navigate(crate::ui::components::NavItem::Radio)
}

pub fn view<'a>(
    state: &'a DiscoverPageState,
    image_state: &'a ImageState,
    locale: Locale,
    active_personal_fm_cover: Option<&'a iced::widget::image::Handle>,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    match state.view_mode {
        DiscoverViewMode::Overview => view_overview(
            state,
            image_state,
            locale,
            active_personal_fm_cover,
            context,
        ),
        DiscoverViewMode::AllRecommended => view_all_playlists(
            state,
            image_state,
            locale,
            Key::DiscoverRecommended,
            &state.recommended_playlists,
            context,
        ),
        DiscoverViewMode::AllHot => view_all_playlists(
            state,
            image_state,
            locale,
            Key::DiscoverHot,
            &state.hot_playlists,
            context,
        ),
        DiscoverViewMode::AllOfficial => view_all_playlists(
            state,
            image_state,
            locale,
            Key::DiscoverOfficialPicks,
            &state.official_playlists,
            context,
        ),
    }
}

fn view_overview<'a>(
    state: &'a DiscoverPageState,
    image_state: &'a ImageState,
    locale: Locale,
    active_personal_fm_cover: Option<&'a iced::widget::image::Handle>,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let tokens = context.tokens;
    let content_width = state.content_width;
    let feature_row = personal_feature_row(
        state,
        image_state,
        locale,
        active_personal_fm_cover,
        context,
    );

    let content = column![
        Space::new().height(top_bar_height(&context) + tokens.space(4.0)),
        feature_row,
        Space::new().height(tokens.space(40.0)),
        section_header::view(
            locale.get(Key::DiscoverRecommended),
            locale.get(Key::DiscoverSeeAll),
            Some(Message::SeeAllRecommended),
        ),
        Space::new().height(tokens.space(16.0)),
        playlist_grid::view_single_row_with_context(
            &state.recommended_playlists,
            image_state,
            &state.card_animations,
            content_width,
            context,
        ),
        Space::new().height(tokens.space(40.0)),
        section_header::view(
            locale.get(Key::DiscoverHot),
            locale.get(Key::DiscoverSeeAll),
            Some(Message::SeeAllHot),
        ),
        Space::new().height(tokens.space(16.0)),
        playlist_grid::view_single_row_with_context(
            &state.hot_playlists,
            image_state,
            &state.card_animations,
            content_width,
            context,
        ),
        Space::new().height(tokens.space(40.0)),
        section_header::view(
            locale.get(Key::DiscoverOfficialPicks),
            locale.get(Key::DiscoverSeeAll),
            Some(Message::SeeAllOfficial),
        ),
        Space::new().height(tokens.space(16.0)),
        playlist_grid::view_single_row_with_context(
            &state.official_playlists,
            image_state,
            &state.card_animations,
            content_width,
            context,
        ),
        Space::new().height(tokens.space(40.0)),
    ]
    .padding(tokens.space(32.0));

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
    active_personal_fm_cover: Option<&'a iced::widget::image::Handle>,
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let tokens = context.tokens;
    let day = chrono::Local::now().format("%d").to_string();
    let daily = feature_card::view_with_context(
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
        feature_width(context),
        state.card_animations.get_progress(&DAILY_FEATURE_ID),
        Message::OpenNcmPlaylist(DAILY_FEATURE_ID),
        Message::PlayDiscoverPlaylist(DAILY_FEATURE_ID),
        Message::HoverDiscoverPlaylist(Some(DAILY_FEATURE_ID)),
        Message::HoverDiscoverPlaylist(None),
        context,
    );

    let radar = state.private_radar.as_ref();
    let radar_title = radar
        .map(|playlist| playlist.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| locale.get(Key::DiscoverPrivateRadar).to_string());
    let radar_subtitle = locale.get(Key::DiscoverPrivateRadarDesc).to_string();
    let radar = feature_card::view_with_context(
        radar_title,
        radar_subtitle,
        None,
        crate::ui::icons::BROWSE,
        image_state.get(ImageKind::PlaylistCover, PRIVATE_RADAR_PLAYLIST_ID),
        (
            Color::from_rgb(0.18, 0.35, 0.63),
            Color::from_rgb(0.42, 0.18, 0.55),
        ),
        feature_width(context),
        state
            .card_animations
            .get_progress(&PRIVATE_RADAR_PLAYLIST_ID),
        Message::OpenNcmPlaylist(PRIVATE_RADAR_PLAYLIST_ID),
        Message::PlayDiscoverPlaylist(PRIVATE_RADAR_PLAYLIST_ID),
        Message::HoverDiscoverPlaylist(Some(PRIVATE_RADAR_PLAYLIST_ID)),
        Message::HoverDiscoverPlaylist(None),
        context,
    );

    let personal_fm_action = personal_fm_action();
    let personal_fm = feature_card::view_with_context(
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
        feature_width(context),
        state.card_animations.get_progress(&PERSONAL_FM_FEATURE_ID),
        personal_fm_action.clone(),
        personal_fm_action,
        Message::HoverDiscoverPlaylist(Some(PERSONAL_FM_FEATURE_ID)),
        Message::HoverDiscoverPlaylist(None),
        context,
    );

    let gap = tokens.space(18.0);
    match context.profile {
        LayoutProfile::Expanded | LayoutProfile::Standard => row![daily, radar, personal_fm]
            .spacing(gap)
            .width(Fill)
            .into(),
        LayoutProfile::Compact => column![
            row![daily, radar].spacing(gap).width(Fill),
            row![personal_fm].width(Fill),
        ]
        .spacing(gap)
        .width(Fill)
        .into(),
        LayoutProfile::Tablet | LayoutProfile::Narrow => column![daily, radar, personal_fm]
            .spacing(tokens.space(16.0))
            .width(Fill)
            .into(),
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

fn feature_width(context: ResponsiveContext) -> Length {
    match context.profile {
        LayoutProfile::Expanded | LayoutProfile::Standard | LayoutProfile::Compact => {
            Length::FillPortion(1)
        }
        LayoutProfile::Tablet | LayoutProfile::Narrow => Length::Fill,
    }
}

fn view_all_playlists<'a>(
    state: &'a DiscoverPageState,
    image_state: &'a ImageState,
    locale: Locale,
    title: Key,
    playlists: &'a [crate::api::PlaylistSummary],
    context: ResponsiveContext,
) -> Element<'a, Message> {
    let tokens = context.tokens;
    let content = column![
        Space::new().height(top_bar_height(&context) + tokens.space(4.0)),
        text(locale.get(title)).size(tokens.text(TextRole::Title)),
        Space::new().height(tokens.space(24.0)),
        playlist_grid::view_with_context(
            playlists,
            image_state,
            &state.card_animations,
            None,
            state.content_width,
            context,
        ),
        Space::new().height(tokens.space(40.0)),
    ]
    .padding(tokens.space(32.0));

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
