//! Carousel banner component for NCM homepage
//!
//! Displays rotating banners from NCM API with auto-advance and manual navigation.

use iced::widget::{Space, button, canvas, column, container, row, svg, text};
use iced::{
    Alignment, Background, Color, Element, Fill, Padding, Point, Rectangle, Renderer, Size, Theme,
    mouse,
};
use std::path::PathBuf;

use crate::api::BannersInfo;
use crate::app::Message;
use crate::i18n::{Key, Locale};
use crate::ui::theme::{self, BOLD_WEIGHT};

const BANNER_HEIGHT: f32 = 280.0;
const INDICATOR_SIZE: f32 = 8.0;
const INDICATOR_SPACING: f32 = 8.0;

struct BannerDrawer<'a> {
    current_image: Option<&'a (PathBuf, u32, u32)>,
    last_image: Option<&'a (PathBuf, u32, u32)>,
    progress: f32,
    direction: i32,
}

impl<'a, Message> canvas::Program<Message> for BannerDrawer<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Helper to draw image or fallback
        let draw_banner =
            |frame: &mut canvas::Frame, image_data: Option<&(PathBuf, u32, u32)>, offset_x: f32| {
                if let Some((path, width, height)) = image_data {
                    let image = canvas::Image::new(path);

                    // Calculate ContentFit::Cover
                    let img_w = *width as f32;
                    let img_h = *height as f32;

                    if img_w > 0.0 && img_h > 0.0 {
                        let target_w = bounds.width;
                        let target_h = bounds.height;

                        let scale_w = target_w / img_w;
                        let scale_h = target_h / img_h;

                        // User complained about content not being visible on wide screens (Cover crops too much).
                        // Switch to Contain (fit entirely)
                        let scale = scale_w.min(scale_h);

                        let final_w = img_w * scale;
                        let final_h = img_h * scale;

                        let x = offset_x + (target_w - final_w) / 2.0;
                        let y = (target_h - final_h) / 2.0;

                        // Draw background to fill gaps - use a neutral color that works for both themes
                        if final_w < target_w || final_h < target_h {
                            frame.fill_rectangle(
                                Point::new(offset_x, 0.0),
                                Size::new(target_w, target_h),
                                Color::from_rgb(0.15, 0.1, 0.2), // Subtle purple tint
                            );
                        }

                        frame.draw_image(
                            Rectangle::new(Point::new(x, y), Size::new(final_w, final_h)),
                            image,
                        );
                    }
                } else {
                    // Fallback color - use theme-aware banner placeholder
                    frame.fill_rectangle(
                        Point::new(offset_x, 0.0),
                        bounds.size(),
                        Color::from_rgb(0.15, 0.1, 0.2), // Subtle purple tint
                    );
                }
            };

        if self.progress >= 1.0 {
            // Not animating (or finished), just draw current
            draw_banner(&mut frame, self.current_image, 0.0);
        } else {
            // Animating
            let width = bounds.width;
            // Ease out cubic for smoother feel
            let eased_progress = 1.0 - (1.0 - self.progress).powi(3);

            let (last_offset, current_offset) = if self.direction > 0 {
                // Next: Last moves Left (-width), Current moves in from Right (width -> 0)
                (-width * eased_progress, width * (1.0 - eased_progress))
            } else {
                // Prev: Last moves Right (width), Current moves in from Left (-width -> 0)
                (width * eased_progress, -width * (1.0 - eased_progress))
            };

            // Draw last first (bottom)
            draw_banner(&mut frame, self.last_image, last_offset);
            // Draw current
            draw_banner(&mut frame, self.current_image, current_offset);
        }

        vec![frame.into_geometry()]
    }
}

/// Build the carousel banner component
pub fn view<'a>(
    banners: &'a [BannersInfo],
    banner_images: &'a std::collections::HashMap<usize, (PathBuf, u32, u32)>,
    current_index: usize,
    last_index: usize,
    animation: &'a iced::animation::Animation<bool>,
    direction: i32,
    locale: Locale,
    is_logged_in: bool,
) -> Element<'a, Message> {
    if banners.is_empty() {
        // Show placeholder when no banners loaded
        return view_placeholder(locale);
    }

    let now = iced::time::Instant::now();
    let progress = animation.interpolate(0.0_f32, 1.0_f32, now);

    let current_image = banner_images.get(&current_index);
    let last_image = banner_images.get(&last_index);

    // Banner content using Canvas for animation
    let banner_content: Element<'_, Message> = canvas(BannerDrawer {
        current_image,
        last_image,
        progress,
        direction,
    })
    .width(Fill)
    .height(BANNER_HEIGHT)
    .into();

    // Navigation arrows
    let left_arrow = button(
        svg(svg::Handle::from_memory(
            crate::ui::icons::CHEVRON_LEFT.as_bytes(),
        ))
        .width(24)
        .height(24)
        .style(|_theme, _status| svg::Style {
            color: Some(Color::WHITE),
        }),
    )
    .padding(12)
    .style(theme::carousel_nav_button)
    .on_press(Message::CarouselNavigate(-1));

    let right_arrow = button(
        svg(svg::Handle::from_memory(
            crate::ui::icons::CHEVRON_RIGHT.as_bytes(),
        ))
        .width(24)
        .height(24)
        .style(|_theme, _status| svg::Style {
            color: Some(Color::WHITE),
        }),
    )
    .padding(12)
    .style(theme::carousel_nav_button)
    .on_press(Message::CarouselNavigate(1));

    // Play and favorite buttons
    let play_button = button(
        row![
            svg(svg::Handle::from_memory(crate::ui::icons::PLAY.as_bytes()))
                .width(16)
                .height(16)
                .style(|_theme, _status| svg::Style {
                    color: Some(Color::BLACK),
                }),
            text("Play").size(14).color(Color::BLACK).font(iced::Font {
                weight: BOLD_WEIGHT,
                ..Default::default()
            }),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(8.0).left(20).right(20)) // Pill shape padding
    .height(40)
    .style(theme::banner_play_button)
    .on_press(Message::BannerPlay(current_index));

    // Plus button (replacing favorite/heart to match design)
    let plus_button: Element<'_, Message> = button(
        svg(svg::Handle::from_memory(crate::ui::icons::PLUS.as_bytes()))
            .width(20)
            .height(20)
            .style(|_theme, _status| svg::Style {
                color: Some(Color::WHITE),
            }),
    )
    .padding(0)
    .width(40)
    .height(40)
    .style(theme::glass_icon_button)
    .on_press(Message::ToggleBannerFavorite(current_index))
    .into();

    // Page indicators (dots)
    let indicators: Element<'_, Message> = row(banners
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let is_active = i == current_index;
            container(Space::new().width(INDICATOR_SIZE).height(INDICATOR_SIZE))
                .style(move |theme| container::Style {
                    background: Some(
                        if is_active {
                            Color::WHITE
                        } else {
                            theme::indicator_inactive(theme)
                        }
                        .into(),
                    ),
                    border: iced::Border {
                        radius: (INDICATOR_SIZE / 2.0).into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        })
        .collect::<Vec<_>>())
    .spacing(INDICATOR_SPACING)
    .align_y(Alignment::Center)
    .into();

    // Buttons row (left side)
    let buttons_row = row![
        play_button,
        Space::new().width(12),
        if is_logged_in {
            plus_button
        } else {
            Space::new().width(40).height(40).into()
        },
    ]
    .align_y(Alignment::Center);

    // Bottom row with buttons on left and indicators on right
    let bottom_row = row![buttons_row, Space::new().width(Fill), indicators,]
        .align_y(Alignment::Center)
        .padding(Padding::new(0.0).left(32.0).right(32.0));

    // Gradient overlay (Bottom to 1/4 up)
    let gradient_overlay = container(
        column![
            Space::new().height(Fill), // Push content to bottom
            bottom_row,
        ]
        .padding(Padding::new(24.0).bottom(32.0)),
    )
    .width(Fill)
    .height(Fill)
    .style(|_theme| container::Style {
        background: Some(iced::Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI)) // Top to Bottom
                .add_stop(0.0, Color::TRANSPARENT)
                .add_stop(0.5, Color::TRANSPARENT)
                .add_stop(1.0, theme::banner_gradient_bottom()),
        ))),
        ..Default::default()
    });

    // Navigation overlay (arrows)
    let nav_overlay = row![
        container(left_arrow)
            .height(BANNER_HEIGHT)
            .align_y(Alignment::Center)
            .padding(Padding::new(8.0)),
        Space::new().width(Fill),
        container(right_arrow)
            .height(BANNER_HEIGHT)
            .align_y(Alignment::Center)
            .padding(Padding::new(8.0)),
    ]
    .width(Fill)
    .height(BANNER_HEIGHT);

    // Stack all layers
    let stacked = iced::widget::stack![banner_content, gradient_overlay, nav_overlay]
        .width(Fill)
        .height(BANNER_HEIGHT);

    // Container without click handling - only play button triggers playback
    container(stacked)
        .width(Fill)
        .height(BANNER_HEIGHT)
        .style(theme::hero_banner)
        .into()
}

/// Placeholder view when no banners are loaded
fn view_placeholder(locale: Locale) -> Element<'static, Message> {
    let illustration = container(Space::new().width(Fill).height(BANNER_HEIGHT))
        .width(Fill)
        .height(BANNER_HEIGHT)
        .style(move |theme| container::Style {
            background: Some(Background::Color(theme::banner_placeholder(theme))),
            ..Default::default()
        });

    let hero_title = locale.get(Key::HeroTitle);
    let hero_subtitle = locale.get(Key::HeroSubtitle);

    let overlay_content = column![
        text(hero_title)
            .size(36)
            .font(iced::Font {
                weight: BOLD_WEIGHT,
                ..Default::default()
            })
            .style(|theme| text::Style {
                color: Some(theme::text_primary(theme))
            }),
        text(hero_subtitle).size(14).color(theme::TEXT_SECONDARY),
        Space::new().height(20),
        text(locale.get(Key::Loading).to_string())
            .size(14)
            .color(theme::TEXT_SECONDARY),
    ]
    .spacing(8)
    .padding(Padding::new(32.0));

    container(iced::widget::stack![
        illustration,
        container(overlay_content)
            .width(Fill)
            .height(BANNER_HEIGHT)
            .align_y(iced::alignment::Vertical::Bottom)
            .align_x(iced::alignment::Horizontal::Left),
    ])
    .width(Fill)
    .height(BANNER_HEIGHT)
    .style(theme::hero_banner)
    .into()
}
