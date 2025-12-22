//! Audio Engine page component
//!
//! Rustle Audio Engine page with:
//! - 10-band parametric equalizer with vertical sliders
//! - EQ curve visualization using Canvas
//! - Preset selection
//! - Preamp control
//! - Professional spectrum analyzer (FFT-based)

use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{Space, column, container, pick_list, row, scrollable, text, toggler};
use iced::{Alignment, Background, Color, Element, Fill, Length, Padding, Point, Theme};

use crate::app::Message;
use crate::audio::{AudioAnalysisData, analyzer::FFT_SIZE};
use crate::features::{EqualizerPreset, Settings};
use crate::i18n::{Key, Locale};
use crate::ui::theme;
use crate::ui::widgets::vertical_slider;

/// Frequency labels for 10-band EQ
const FREQ_LABELS: [&str; 10] = [
    "32", "64", "125", "250", "500", "1K", "2K", "4K", "8K", "16K",
];

/// Frequency labels for spectrum analyzer (logarithmic scale)
const SPECTRUM_FREQ_LABELS: [(&str, f32); 10] = [
    ("32", 32.0),
    ("64", 64.0),
    ("125", 100.0),
    ("250", 200.0),
    ("500", 500.0),
    ("1k", 1000.0),
    ("2k", 2000.0),
    ("4k", 4000.0),
    ("8k", 8000.0),
    ("16k", 16000.0),
];

/// dB labels for spectrum analyzer
const SPECTRUM_DB_LABELS: [(i32, &str); 5] = [
    (12, "+12 dB"),
    (0, "0 dB"),
    (-12, "-12 dB"),
    (-24, "-24 dB"),
    (-48, "-48 dB"),
];

/// Audio Engine page view
pub fn view(
    settings: &Settings,
    locale: Locale,
    analysis_data: Option<&AudioAnalysisData>,
) -> Element<'static, Message> {
    // Header with just title
    let header = text(locale.get(Key::AudioEngineTitle).to_string())
        .size(28)
        .style(|theme| text::Style {
            color: Some(theme::settings_title(theme)),
        });

    let header_container = container(header)
        .width(Fill)
        .padding(Padding::new(40.0).right(60.0).bottom(20.0).left(60.0))
        .style(|theme| container::Style {
            background: Some(Background::Color(theme::background(theme))),
            ..Default::default()
        });

    // Get real-time audio levels and spectrum
    let (left_level, right_level, spectrum_db, sample_rate) = if let Some(data) = analysis_data {
        (
            data.left_rms(),
            data.right_rms(),
            data.spectrum_db(),
            data.sample_rate(),
        )
    } else {
        (0.0, 0.0, vec![-60.0; 128], 48000)
    };

    let decay = settings.playback.spectrum_decay;
    let bars_mode = settings.playback.spectrum_bars_mode;

    // Main content with equalizer and audio visualization
    let content = column![
        // Equalizer section
        equalizer_section(settings, locale),
        Space::new().height(40),
        // Audio visualization section
        audio_visualization_section(
            left_level,
            right_level,
            spectrum_db,
            sample_rate,
            decay,
            bars_mode,
            locale
        ),
    ]
    .spacing(0)
    .width(Fill);

    let scrollable_content = scrollable(
        container(content)
            .width(Fill)
            .padding(Padding::new(20.0).right(60.0).bottom(60.0).left(60.0)),
    )
    .width(Fill)
    .height(Fill);

    container(
        column![header_container, scrollable_content,]
            .width(Fill)
            .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(theme::main_content)
    .into()
}

/// Equalizer section with title
fn equalizer_section(settings: &Settings, locale: Locale) -> Element<'static, Message> {
    let eq_enabled = settings.playback.equalizer_enabled;
    let eq_preset = settings.playback.equalizer_preset;
    let eq_values = settings.playback.equalizer_values;
    let preamp = settings.playback.equalizer_preamp;

    // Section title row with toggle and preset
    let title_row = row![
        text(locale.get(Key::AudioEngineEqualizer).to_string())
            .size(18)
            .style(|theme| text::Style {
                color: Some(theme::settings_title(theme))
            }),
        Space::new().width(24),
        toggler(eq_enabled)
            .on_toggle(Message::UpdateEqualizerEnabled)
            .size(18),
        Space::new().width(8),
        text(if eq_enabled { "ON" } else { "OFF" })
            .size(13)
            .style(|theme| text::Style {
                color: Some(theme::settings_desc(theme))
            }),
        Space::new().width(Fill),
        preset_picker(eq_preset),
    ]
    .align_y(Alignment::Center)
    .width(Fill);

    // Equalizer content
    let eq_content: Element<'static, Message> = if eq_enabled {
        column![
            Space::new().height(24),
            // EQ Curve visualization
            eq_curve_canvas(eq_values),
            Space::new().height(24),
            // Sliders row with preamp
            sliders_with_preamp(eq_values, preamp),
        ]
        .spacing(0)
        .width(Fill)
        .into()
    } else {
        container(
            text(locale.get(Key::AudioEngineEqualizerDisabled).to_string())
                .size(14)
                .style(|theme| text::Style {
                    color: Some(theme::settings_desc(theme)),
                }),
        )
        .width(Fill)
        .height(100)
        .center_x(Fill)
        .center_y(100)
        .into()
    };

    column![title_row, eq_content,]
        .spacing(0)
        .width(Fill)
        .into()
}

/// Audio visualization section with spectrum analyzer
fn audio_visualization_section(
    left_level: f32,
    right_level: f32,
    spectrum_db: Vec<f32>,
    sample_rate: u32,
    decay: f32,
    bars_mode: bool,
    locale: Locale,
) -> Element<'static, Message> {
    use iced::widget::slider;

    // Separator line
    let separator = container(Space::new().width(Fill).height(1)).style(|theme| container::Style {
        background: Some(Background::Color(theme::divider(theme))),
        ..Default::default()
    });

    // Section title
    let title = text(locale.get(Key::AudioEngineSpectrum).to_string())
        .size(18)
        .style(|theme| text::Style {
            color: Some(theme::settings_title(theme)),
        });

    // Mode dropdown (bars/line) - like equalizer preset picker
    let mode_picker = spectrum_mode_picker(bars_mode);

    // Spectrum analyzer canvas (main visualization)
    let spectrum_height = 280.0;
    let spectrum = Canvas::new(SpectrumAnalyzer {
        spectrum_db,
        sample_rate,
        decay,
        bars_mode,
    })
    .width(Fill)
    .height(Length::Fixed(spectrum_height));

    // Volume meters on the left (same height as spectrum graph area)
    let meter_height = spectrum_height - 50.0; // Account for top/bottom margins
    let meters = column![
        Space::new().height(10.0), // Align with graph top margin
        row![
            volume_meter_view("L", left_level, meter_height),
            Space::new().width(8),
            volume_meter_view("R", right_level, meter_height),
        ]
        .align_y(Alignment::Start),
    ]
    .align_x(Alignment::Center);

    // Decay slider
    let decay_label = text("Decay").size(11).style(|theme| text::Style {
        color: Some(theme::settings_desc(theme)),
    });
    let decay_slider = slider(0.0..=0.95, decay, Message::UpdateSpectrumDecay)
        .step(0.01)
        .width(Length::Fixed(120.0));

    column![
        separator,
        Space::new().height(24),
        row![title, Space::new().width(Fill), mode_picker,]
            .align_y(Alignment::Center)
            .width(Fill),
        Space::new().height(16),
        // Volume meters on left, spectrum in center (dB labels on right side of spectrum)
        row![meters, Space::new().width(16), spectrum,]
            .align_y(Alignment::Start)
            .width(Fill),
        Space::new().height(12),
        row![decay_label, Space::new().width(8), decay_slider,].align_y(Alignment::Center),
    ]
    .spacing(0)
    .width(Fill)
    .into()
}

/// Volume meter view for a single channel
fn volume_meter_view(label: &'static str, level: f32, height: f32) -> Element<'static, Message> {
    column![
        text(label).size(12).style(|theme| text::Style {
            color: Some(theme::settings_desc(theme))
        }),
        Space::new().height(8),
        Canvas::new(VolumeMeter { level })
            .width(Length::Fixed(24.0))
            .height(Length::Fixed(height)),
    ]
    .spacing(0)
    .align_x(Alignment::Center)
    .into()
}

/// Preset picker dropdown
fn preset_picker(current: EqualizerPreset) -> Element<'static, Message> {
    let presets: Vec<EqualizerPreset> = EqualizerPreset::all().to_vec();

    pick_list(presets, Some(current), Message::UpdateEqualizerPreset)
        .text_size(14)
        .padding([8, 16])
        .style(theme::settings_pick_list)
        .menu_style(theme::settings_pick_list_menu)
        .into()
}

/// Spectrum display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectrumMode {
    Bars,
    Line,
}

impl std::fmt::Display for SpectrumMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpectrumMode::Bars => write!(f, "柱状"),
            SpectrumMode::Line => write!(f, "曲线"),
        }
    }
}

/// Spectrum mode picker dropdown
fn spectrum_mode_picker(bars_mode: bool) -> Element<'static, Message> {
    let modes = vec![SpectrumMode::Bars, SpectrumMode::Line];
    let current = if bars_mode {
        SpectrumMode::Bars
    } else {
        SpectrumMode::Line
    };

    pick_list(modes, Some(current), |mode| {
        Message::UpdateSpectrumBarsMode(mode == SpectrumMode::Bars)
    })
    .text_size(14)
    .padding([8, 16])
    .style(theme::settings_pick_list)
    .menu_style(theme::settings_pick_list_menu)
    .into()
}

/// EQ curve visualization using Canvas
fn eq_curve_canvas(eq_values: [f32; 10]) -> Element<'static, Message> {
    Canvas::new(EqCurve { values: eq_values })
        .width(Fill)
        .height(Length::Fixed(120.0))
        .into()
}

// ============================================================================
// Canvas Programs
// ============================================================================

/// Canvas program for drawing EQ curve
struct EqCurve {
    values: [f32; 10],
}

impl canvas::Program<Message> for EqCurve {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height;

        // Draw horizontal grid lines (subtle)
        let grid_color = Color::from_rgba(1.0, 1.0, 1.0, 0.08);
        for i in 0..=4 {
            let y = height * (i as f32 / 4.0);
            let line = Path::line(Point::new(0.0, y), Point::new(width, y));
            frame.stroke(
                &line,
                Stroke::default().with_color(grid_color).with_width(1.0),
            );
        }

        // Draw center line (0 dB) slightly brighter
        let center_y = height / 2.0;
        let center_line = Path::line(Point::new(0.0, center_y), Point::new(width, center_y));
        frame.stroke(
            &center_line,
            Stroke::default()
                .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.15))
                .with_width(1.0),
        );

        // Calculate curve points with smooth interpolation
        let points = self.calculate_curve_points(width, height);

        // Draw gradient fill under the curve
        self.draw_gradient_fill(&mut frame, &points, width, height);

        // Draw the main curve line
        if points.len() >= 2 {
            let curve = Path::new(|builder| {
                builder.move_to(points[0]);
                for point in points.iter().skip(1) {
                    builder.line_to(*point);
                }
            });

            frame.stroke(
                &curve,
                Stroke::default()
                    .with_color(theme::ACCENT_PINK)
                    .with_width(2.0),
            );
        }

        vec![frame.into_geometry()]
    }
}

impl EqCurve {
    /// Calculate smooth curve points using catmull-rom spline interpolation
    fn calculate_curve_points(&self, width: f32, height: f32) -> Vec<Point> {
        let mut points = Vec::new();
        let num_samples = 200;
        let center_y = height / 2.0;
        let max_db = 12.0;

        // Preamp offset at the start (left side)
        let preamp_width = 60.0;
        let eq_start_x = preamp_width + 20.0;
        let eq_width = width - eq_start_x;

        for i in 0..=num_samples {
            let t = i as f32 / num_samples as f32;
            let x = eq_start_x + t * eq_width;

            let eq_pos = t * 9.0;
            let band_index = (eq_pos.floor() as usize).min(8);
            let band_frac = eq_pos - band_index as f32;

            let v0 = if band_index > 0 {
                self.values[band_index - 1]
            } else {
                self.values[0]
            };
            let v1 = self.values[band_index];
            let v2 = self.values[(band_index + 1).min(9)];
            let v3 = self.values[(band_index + 2).min(9)];

            let value = catmull_rom(v0, v1, v2, v3, band_frac);
            let normalized = value / max_db;
            let y = center_y - normalized * (height / 2.0 - 10.0);

            points.push(Point::new(x, y.clamp(5.0, height - 5.0)));
        }

        points
    }

    /// Draw gradient fill from curve to bottom with fade effect
    fn draw_gradient_fill(&self, frame: &mut Frame, points: &[Point], _width: f32, height: f32) {
        if points.len() < 2 {
            return;
        }

        let num_strips = 12;
        for strip in 0..num_strips {
            let strip_ratio = strip as f32 / num_strips as f32;
            let next_ratio = (strip + 1) as f32 / num_strips as f32;
            let alpha = 0.20 * (1.0 - strip_ratio).powf(1.5);

            if alpha < 0.005 {
                continue;
            }

            let fill_path = Path::new(|builder| {
                let mut started = false;
                for point in points.iter() {
                    let y_at_curve = point.y;
                    let y_top = y_at_curve + (height - y_at_curve) * strip_ratio;
                    if !started {
                        builder.move_to(Point::new(point.x, y_top));
                        started = true;
                    } else {
                        builder.line_to(Point::new(point.x, y_top));
                    }
                }
                for point in points.iter().rev() {
                    let y_at_curve = point.y;
                    let y_bottom = y_at_curve + (height - y_at_curve) * next_ratio;
                    builder.line_to(Point::new(point.x, y_bottom));
                }
                builder.close();
            });

            let fill_color = Color::from_rgba(0.95, 0.3, 0.5, alpha);
            frame.fill(&fill_path, fill_color);
        }
    }
}

/// Catmull-Rom spline interpolation
fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Volume meter canvas program
struct VolumeMeter {
    level: f32, // 0.0 to 1.0
}

impl canvas::Program<Message> for VolumeMeter {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height;

        // Background track
        let bg_rect = Path::new(|builder| {
            builder.move_to(Point::new(0.0, 0.0));
            builder.line_to(Point::new(width, 0.0));
            builder.line_to(Point::new(width, height));
            builder.line_to(Point::new(0.0, height));
            builder.close();
        });
        frame.fill(&bg_rect, Color::from_rgba(1.0, 1.0, 1.0, 0.1));

        // Level indicator (from bottom up)
        // Draw level with gradient colors (green -> yellow -> red)
        let num_segments = 20;
        let segment_height = height / num_segments as f32;

        for i in 0..num_segments {
            let seg_bottom = height - (i as f32 * segment_height);
            let seg_top = seg_bottom - segment_height;
            let seg_level = i as f32 / num_segments as f32;

            // Only draw if this segment is within the level
            if seg_level > self.level {
                continue;
            }

            // Color based on level: green (low) -> yellow (mid) -> red (high)
            let color = if seg_level < 0.6 {
                // Green zone
                theme::spectrum_green()
            } else if seg_level < 0.85 {
                // Yellow zone
                theme::spectrum_yellow()
            } else {
                // Red zone
                theme::spectrum_red()
            };

            let seg_rect = Path::new(|builder| {
                builder.move_to(Point::new(2.0, seg_top + 1.0));
                builder.line_to(Point::new(width - 2.0, seg_top + 1.0));
                builder.line_to(Point::new(width - 2.0, seg_bottom - 1.0));
                builder.line_to(Point::new(2.0, seg_bottom - 1.0));
                builder.close();
            });
            frame.fill(&seg_rect, color);
        }

        vec![frame.into_geometry()]
    }
}

/// Professional spectrum analyzer canvas (FFT-based)
struct SpectrumAnalyzer {
    spectrum_db: Vec<f32>,
    sample_rate: u32,
    #[allow(dead_code)]
    decay: f32,
    bars_mode: bool,
}

impl canvas::Program<Message> for SpectrumAnalyzer {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height;

        // Layout constants - dB labels on left side
        let left_margin = 50.0; // Space for dB labels on left
        let right_margin = 10.0;
        let top_margin = 10.0;
        let bottom_margin = 40.0; // Space for frequency labels and info
        let graph_width = width - left_margin - right_margin;
        let graph_height = height - top_margin - bottom_margin;

        // dB range: +12 to -60
        let db_max = 12.0_f32;
        let db_min = -60.0_f32;
        let db_range = db_max - db_min;

        // Background
        let bg_rect = Path::new(|builder| {
            builder.move_to(Point::new(left_margin, top_margin));
            builder.line_to(Point::new(left_margin + graph_width, top_margin));
            builder.line_to(Point::new(
                left_margin + graph_width,
                top_margin + graph_height,
            ));
            builder.line_to(Point::new(left_margin, top_margin + graph_height));
            builder.close();
        });
        frame.fill(&bg_rect, Color::from_rgba(0.0, 0.0, 0.0, 0.3));

        // Draw horizontal grid lines (dB levels)
        let grid_color = Color::from_rgba(1.0, 1.0, 1.0, 0.08);
        let zero_db_color = Color::from_rgba(1.0, 1.0, 1.0, 0.2);

        for &(db, label) in &SPECTRUM_DB_LABELS {
            let y = top_margin + ((db_max - db as f32) / db_range) * graph_height;
            let line = Path::line(
                Point::new(left_margin, y),
                Point::new(left_margin + graph_width, y),
            );
            let color = if db == 0 { zero_db_color } else { grid_color };
            frame.stroke(&line, Stroke::default().with_color(color).with_width(1.0));

            // dB label on left side
            let text = Text {
                content: label.to_string(),
                position: Point::new(left_margin - 8.0, y),
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.5),
                size: iced::Pixels(10.0),
                align_x: iced::alignment::Horizontal::Right.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Text::default()
            };
            frame.fill_text(text);
        }

        // Draw vertical grid lines (frequency markers)
        for &(label, freq) in &SPECTRUM_FREQ_LABELS {
            let x = left_margin + freq_to_x(freq, graph_width);
            let line = Path::line(
                Point::new(x, top_margin),
                Point::new(x, top_margin + graph_height),
            );
            frame.stroke(
                &line,
                Stroke::default().with_color(grid_color).with_width(1.0),
            );

            // Frequency label
            let text = Text {
                content: label.to_string(),
                position: Point::new(x, top_margin + graph_height + 12.0),
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.5),
                size: iced::Pixels(10.0),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Top,
                ..Text::default()
            };
            frame.fill_text(text);
        }

        // Draw spectrum visualization
        let num_bars = self.spectrum_db.len();
        if num_bars > 0 {
            if self.bars_mode {
                // Bar mode
                let bar_width = (graph_width / num_bars as f32).max(1.0) - 1.0;

                for (i, &db) in self.spectrum_db.iter().enumerate() {
                    let t = i as f32 / num_bars as f32;
                    let x = left_margin + t * graph_width;
                    let normalized = ((db - db_min) / db_range).clamp(0.0, 1.0);
                    let bar_height = normalized * graph_height;
                    let y = top_margin + graph_height - bar_height;

                    if bar_height < 1.0 {
                        continue;
                    }

                    let color = db_to_color(db);
                    let bar = Path::new(|builder| {
                        builder.move_to(Point::new(x, top_margin + graph_height));
                        builder.line_to(Point::new(x, y));
                        builder.line_to(Point::new(x + bar_width, y));
                        builder.line_to(Point::new(x + bar_width, top_margin + graph_height));
                        builder.close();
                    });
                    frame.fill(&bar, color);
                }
            } else {
                // Line/curve mode with gradient fill
                if num_bars >= 2 {
                    // Draw filled area under curve
                    let fill_path = Path::new(|builder| {
                        builder.move_to(Point::new(left_margin, top_margin + graph_height));
                        for (i, &db) in self.spectrum_db.iter().enumerate() {
                            let t = i as f32 / num_bars as f32;
                            let x = left_margin + t * graph_width;
                            let normalized = ((db - db_min) / db_range).clamp(0.0, 1.0);
                            let y = top_margin + graph_height - normalized * graph_height;
                            builder.line_to(Point::new(x, y));
                        }
                        builder.line_to(Point::new(
                            left_margin + graph_width,
                            top_margin + graph_height,
                        ));
                        builder.close();
                    });
                    frame.fill(&fill_path, Color::from_rgba(0.9, 0.3, 0.5, 0.3));

                    // Draw curve line
                    let curve = Path::new(|builder| {
                        let mut started = false;
                        for (i, &db) in self.spectrum_db.iter().enumerate() {
                            let t = i as f32 / num_bars as f32;
                            let x = left_margin + t * graph_width;
                            let normalized = ((db - db_min) / db_range).clamp(0.0, 1.0);
                            let y = top_margin + graph_height - normalized * graph_height;

                            if !started {
                                builder.move_to(Point::new(x, y));
                                started = true;
                            } else {
                                builder.line_to(Point::new(x, y));
                            }
                        }
                    });
                    frame.stroke(
                        &curve,
                        Stroke::default()
                            .with_color(theme::ACCENT_PINK)
                            .with_width(2.0),
                    );
                }
            }
        }

        // Draw info text at bottom right (inside graph area)
        let info_text = format!("FFT: {}  SR: {}Hz", FFT_SIZE, self.sample_rate);
        let info = Text {
            content: info_text,
            position: Point::new(
                left_margin + graph_width - 4.0,
                top_margin + graph_height - 4.0,
            ),
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
            size: iced::Pixels(10.0),
            align_x: iced::alignment::Horizontal::Right.into(),
            align_y: iced::alignment::Vertical::Bottom,
            ..Text::default()
        };
        frame.fill_text(info);

        vec![frame.into_geometry()]
    }
}

/// Convert frequency to x position (hybrid log scale with compressed low end)
fn freq_to_x(freq: f32, width: f32) -> f32 {
    let min_freq = 32.0_f32;
    let max_freq = 16000.0_f32;

    // First apply logarithmic mapping
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();
    let log_freq = freq.clamp(min_freq, max_freq).ln();
    let t = (log_freq - log_min) / (log_max - log_min);

    // Then apply power function to compress low frequencies further
    // power < 1 compresses low end (where t is small)
    let power = 1.0_f32;
    let x = t.powf(power);

    x * width
}

/// Convert dB value to color (gradient from dark blue to pink/red)
fn db_to_color(db: f32) -> Color {
    // Normalize to 0-1 range (-60dB to +12dB)
    let t = ((db + 60.0) / 72.0).clamp(0.0, 1.0);

    if t < 0.5 {
        // Low levels: dark blue to cyan
        let s = t * 2.0;
        Color::from_rgba(0.1 + s * 0.2, 0.2 + s * 0.4, 0.4 + s * 0.3, 0.7)
    } else if t < 0.8 {
        // Mid levels: cyan to pink
        let s = (t - 0.5) / 0.3;
        Color::from_rgba(0.3 + s * 0.6, 0.6 - s * 0.2, 0.7 - s * 0.2, 0.8)
    } else {
        // High levels: pink to red (hot)
        let s = (t - 0.8) / 0.2;
        Color::from_rgba(0.9 + s * 0.05, 0.4 - s * 0.2, 0.5 - s * 0.3, 0.9)
    }
}

// ============================================================================
// Slider Components
// ============================================================================

/// Format dB value for display
fn format_db(value: f32) -> String {
    if value >= 0.0 {
        format!("+{:.1}", value)
    } else {
        format!("{:.1}", value)
    }
}

/// Sliders row with preamp and separator
fn sliders_with_preamp(eq_values: [f32; 10], preamp: f32) -> Element<'static, Message> {
    // Preamp slider with value display
    let preamp_slider = column![
        text("PREAMP").size(11).style(|theme| text::Style {
            color: Some(theme::settings_desc(theme))
        }),
        Space::new().height(4),
        text(format_db(preamp))
            .size(10)
            .style(move |theme| text::Style {
                color: Some(if preamp != 0.0 {
                    theme::ACCENT_PINK
                } else {
                    theme::settings_value(theme)
                })
            }),
        Space::new().height(4),
        vertical_slider(-12.0..=12.0, preamp, Message::UpdateEqualizerPreamp)
            .step(0.5)
            .width(40)
            .height(180),
    ]
    .spacing(0)
    .align_x(Alignment::Center)
    .width(Length::Fixed(60.0));

    // Separator line
    let separator = container(Space::new().width(1).height(200)).style(|theme| container::Style {
        background: Some(Background::Color(theme::divider(theme))),
        ..Default::default()
    });

    // EQ band sliders
    let band_sliders: Vec<Element<'static, Message>> = (0..10)
        .map(|i| {
            let value = eq_values[i];
            let freq = FREQ_LABELS[i];
            vertical_slider_band(i, value, freq, eq_values)
        })
        .collect();

    row![
        preamp_slider,
        Space::new().width(16),
        separator,
        Space::new().width(16),
        row(band_sliders).spacing(0).width(Fill),
    ]
    .align_y(Alignment::End)
    .width(Fill)
    .into()
}

/// Single vertical slider band with label and value display
fn vertical_slider_band(
    index: usize,
    value: f32,
    freq_label: &'static str,
    eq_values: [f32; 10],
) -> Element<'static, Message> {
    column![
        // Value display above slider
        text(format_db(value))
            .size(10)
            .style(move |theme| text::Style {
                color: Some(if value != 0.0 {
                    theme::ACCENT_PINK
                } else {
                    theme::settings_value(theme)
                })
            })
            .align_x(Alignment::Center)
            .width(Fill),
        Space::new().height(4),
        // Vertical slider
        vertical_slider(-12.0..=12.0, value, move |v| {
            let mut new_values = eq_values;
            new_values[index] = v;
            Message::UpdateEqualizerValues(new_values)
        })
        .step(0.5)
        .width(40)
        .height(180),
        Space::new().height(8),
        // Frequency label
        text(freq_label)
            .size(12)
            .style(|theme| text::Style {
                color: Some(theme::settings_desc(theme))
            })
            .align_x(Alignment::Center)
            .width(Fill),
    ]
    .spacing(0)
    .align_x(Alignment::Center)
    .width(Fill)
    .into()
}
