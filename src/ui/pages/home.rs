//! Home page - "Home" view
//! Main content area with carousel banner and trending songs from NCM API

use iced::widget::{column, container, scrollable, Space};
use iced::{Element, Fill, Padding};

use crate::app::HomePageState;
use crate::app::Message;
use crate::i18n::Locale;
use crate::ui::components;
use crate::ui::theme;

/// Build the home page view with NCM data
pub fn view<'a>(
    _search_query: &'a str,
    home_state: &'a HomePageState,
    locale: Locale,
    is_logged_in: bool,
) -> Element<'a, Message> {
    // Main scrollable content
    let content = column![
        // Carousel banner from NCM API
        components::carousel_banner::view(
            &home_state.banners,
            &home_state.banner_images,
            home_state.current_banner,
            home_state.last_banner,
            &home_state.carousel_animation,
            home_state.carousel_direction,
            locale,
            is_logged_in,
        ),
        Space::new().height(32),
        // Trending songs section (飙升榜)
        components::trending_list::view(
            &home_state.trending_songs,
            &home_state.song_covers,
            &home_state.song_hover_animations,
            locale,
            is_logged_in,
        ),
        Space::new().height(40),
    ]
    .padding(Padding::new(24.0).top(50.0));

    let scrollable_content = scrollable(content)
        .width(Fill)
        .height(Fill)
        .id(iced::widget::Id::new("home_scroll"))
        .style(theme::dark_scrollable);

    // Compose page
    container(scrollable_content)
        .width(Fill)
        .height(Fill)
        .style(theme::main_content)
        .into()
}
