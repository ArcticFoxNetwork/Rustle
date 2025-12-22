// src/app/update/lyrics.rs
//! Lyrics page message handlers
//!
//! Architecture: Async-first loading to prevent UI blocking
//! - Background colors: extracted asynchronously
//! - Local/cached lyrics: loaded asynchronously
//! - Online lyrics: fetched asynchronously (already was)

use iced::Task;
use iced::time::Instant;

use crate::app::message::Message;
use crate::app::state::App;
use crate::ui::effects::background::color_to_array;

impl App {
    /// Handle lyrics page related messages
    pub fn handle_lyrics(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::OpenLyricsPage => {
                // Only open if there's a song playing
                if let Some(song) = self.library.current_song.clone() {
                    self.ui.lyrics.is_open = true;
                    self.ui.lyrics.animation.start(Instant::now());

                    // 智能加载歌词：
                    // 检查当前歌词是否属于当前歌曲
                    // 注意：loading_song_id 表示"正在加载或已加载的歌曲ID"
                    // 如果 loading_song_id 不匹配当前歌曲，需要重新加载
                    let lyrics_need_load = self.ui.lyrics.loading_song_id != Some(song.id);

                    if lyrics_need_load {
                        tracing::debug!("Loading lyrics for song: {} (id={})", song.title, song.id);
                        // Use async loading
                        return Some(self.load_lyrics_async(&song));
                    } else {
                        tracing::debug!(
                            "Lyrics already loaded for song: {} (id={})",
                            song.title,
                            song.id
                        );
                        // Still need to update background if cover changed
                        return Some(self.update_background_async(&song));
                    }
                }
                Some(Task::none())
            }

            Message::CloseLyricsPage => {
                // Start close animation, actual close happens when animation completes
                self.ui.lyrics.animation.stop(Instant::now());
                Some(Task::none())
            }

            &Message::LyricsScroll(delta) => {
                self.handle_lyrics_scroll(delta);
                Some(Task::none())
            }

            Message::WindowResized(size) => {
                self.ui.lyrics.viewport_width = (size.width * 0.6 - 60.0).max(100.0);
                self.ui.lyrics.viewport_height = size.height;

                if let Some(engine_cell) = &self.ui.lyrics.engine {
                    let mut engine = engine_cell.borrow_mut();
                    engine
                        .line_animations_mut()
                        .set_viewport_height(size.height);

                    // Force re-layout by invalidating cached dimensions
                    engine.invalidate_layout();
                }

                // Update discover page content width
                // Content width = window width - sidebar (240) - padding (64)
                const SIDEBAR_WIDTH: f32 = 240.0;
                const CONTENT_PADDING: f32 = 64.0; // 32px on each side
                self.ui.discover.content_width =
                    (size.width - SIDEBAR_WIDTH - CONTENT_PADDING).max(200.0);

                Some(Task::none())
            }

            // Handle async FontSystem initialization
            Message::LyricsFontSystemReady(font_system) => {
                tracing::info!("FontSystem ready for lyrics");
                self.ui.lyrics.shared_font_system = Some(font_system.clone());

                // Create LyricsEngine with the shared font system
                if self.ui.lyrics.engine.is_none() {
                    self.ui.lyrics.engine = Some(std::cell::RefCell::new(
                        crate::features::lyrics::engine::LyricsEngine::new_with_font_system(
                            crate::features::lyrics::engine::LyricsEngineConfig::default(),
                            font_system.clone(),
                        ),
                    ));
                    tracing::info!("LyricsEngine created with shared FontSystem");
                }

                Some(Task::none())
            }

            Message::PreloadLyrics(song_id, ncm_id, _song_name, _singer, _album) => {
                let song_id = *song_id;
                let ncm_id = *ncm_id;

                if self.ui.lyrics.loading_song_id == Some(song_id) && self.ui.lyrics.is_loading {
                    return Some(Task::none());
                }

                self.ui.lyrics.loading_song_id = Some(song_id);
                self.ui.lyrics.is_loading = true;

                if let Some(client) = self.core.ncm_client.clone() {
                    Some(Task::perform(
                        async move {
                            match crate::features::lyrics::fetch_lyrics(&client, ncm_id).await {
                                Ok(lines) => {
                                    let ui_lines = crate::features::lyrics::to_ui_lyrics(lines);
                                    Message::LyricsLoaded(song_id, ui_lines)
                                }
                                Err(e) => Message::LyricsLoadFailed(song_id, e.to_string()),
                            }
                        },
                        |msg| msg,
                    ))
                } else {
                    self.ui.lyrics.is_loading = false;
                    Some(Task::none())
                }
            }

            Message::LyricsLoaded(song_id, lines) => {
                if self.ui.lyrics.loading_song_id == Some(*song_id) {
                    // Apply lyrics lines (fast, just stores data)
                    self.apply_lyrics_lines(lines.clone());
                    tracing::info!(
                        "Loaded {} online lyrics lines for song {}",
                        lines.len(),
                        song_id
                    );

                    // Trigger async engine line preparation
                    let lines_for_task = lines.clone();
                    let song_id = *song_id;
                    return Some(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                // Pre-compute engine lines in background thread
                                let engine_lines: Vec<
                                    crate::features::lyrics::engine::LyricLineData,
                                > = lines_for_task
                                    .iter()
                                    .map(|line| {
                                        let word_count = line.words.len();
                                        let mut line_data =
                                            crate::features::lyrics::engine::LyricLineData {
                                                text: line.text.clone(),
                                                words: line
                                                    .words
                                                    .iter()
                                                    .enumerate()
                                                    .map(|(i, w)| {
                                                        crate::features::lyrics::engine::WordData {
                                                            text: w.word.clone(),
                                                            start_ms: w.start_ms,
                                                            end_ms: w.end_ms,
                                                            roman_word: None,
                                                            emphasize: false,
                                                            x_start: 0.0,
                                                            x_end: 0.0,
                                                            is_last_word: i
                                                                == word_count.saturating_sub(1),
                                                        }
                                                    })
                                                    .collect(),
                                                translated: line.translated.clone(),
                                                romanized: line.romanized.clone(),
                                                start_ms: line.start_ms,
                                                end_ms: line.end_ms,
                                                is_duet: line.is_duet,
                                                is_bg: line.is_background,
                                                mask_animation: None,
                                            };
                                        line_data.compute_mask_animation();
                                        line_data
                                    })
                                    .collect();
                                (song_id, std::sync::Arc::new(engine_lines))
                            })
                            .await
                            .ok()
                        },
                        |result| {
                            if let Some((song_id, engine_lines)) = result {
                                Message::LyricsEngineLinesReady(song_id, engine_lines)
                            } else {
                                Message::Noop
                            }
                        },
                    ));
                }
                Some(Task::none())
            }

            Message::LyricsLoadFailed(song_id, error) => {
                if self.ui.lyrics.loading_song_id == Some(*song_id) {
                    // Clear old lyrics when loading fails (e.g., no lyrics found)
                    self.ui.lyrics.lines.clear();
                    self.ui.lyrics.cached_engine_lines = None;
                    self.ui.lyrics.cached_shaped_lines = None;
                    self.ui.lyrics.is_loading = false;
                    self.ui.lyrics.load_error = Some(error.clone());
                    self.ui.lyrics.current_line_idx = None;

                    // Clear engine's cached data
                    if let Some(engine_cell) = &self.ui.lyrics.engine {
                        let mut engine = engine_cell.borrow_mut();
                        engine.set_cached_shaped_lines(Vec::new());
                    }

                    tracing::warn!("Failed to load lyrics for song {}: {}", song_id, error);
                }
                Some(Task::none())
            }

            // NEW: Handle async local/cached lyrics
            Message::LocalLyricsReady(song_id, lines) => {
                if self.ui.lyrics.loading_song_id == Some(*song_id) {
                    self.apply_lyrics_lines(lines.clone());
                    tracing::info!(
                        "Loaded {} local/cached lyrics lines for song {}",
                        lines.len(),
                        song_id
                    );

                    // Trigger async engine line preparation (same as LyricsLoaded)
                    let lines_for_task = lines.clone();
                    let song_id = *song_id;
                    return Some(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let engine_lines: Vec<
                                    crate::features::lyrics::engine::LyricLineData,
                                > = lines_for_task
                                    .iter()
                                    .map(|line| {
                                        let word_count = line.words.len();
                                        let mut line_data =
                                            crate::features::lyrics::engine::LyricLineData {
                                                text: line.text.clone(),
                                                words: line
                                                    .words
                                                    .iter()
                                                    .enumerate()
                                                    .map(|(i, w)| {
                                                        crate::features::lyrics::engine::WordData {
                                                            text: w.word.clone(),
                                                            start_ms: w.start_ms,
                                                            end_ms: w.end_ms,
                                                            roman_word: None,
                                                            emphasize: false,
                                                            x_start: 0.0,
                                                            x_end: 0.0,
                                                            is_last_word: i
                                                                == word_count.saturating_sub(1),
                                                        }
                                                    })
                                                    .collect(),
                                                translated: line.translated.clone(),
                                                romanized: line.romanized.clone(),
                                                start_ms: line.start_ms,
                                                end_ms: line.end_ms,
                                                is_duet: line.is_duet,
                                                is_bg: line.is_background,
                                                mask_animation: None,
                                            };
                                        line_data.compute_mask_animation();
                                        line_data
                                    })
                                    .collect();
                                (song_id, std::sync::Arc::new(engine_lines))
                            })
                            .await
                            .ok()
                        },
                        |result| {
                            if let Some((song_id, engine_lines)) = result {
                                Message::LyricsEngineLinesReady(song_id, engine_lines)
                            } else {
                                Message::Noop
                            }
                        },
                    ));
                }
                Some(Task::none())
            }

            // Handle pre-computed engine lines
            Message::LyricsEngineLinesReady(song_id, engine_lines) => {
                if self.ui.lyrics.loading_song_id == Some(*song_id) {
                    self.ui.lyrics.cached_engine_lines = Some(engine_lines.clone());
                    tracing::info!(
                        "Engine lines ready for song {}: {} lines",
                        song_id,
                        engine_lines.len()
                    );

                    // Check if font system is ready
                    let Some(font_system) = self.ui.lyrics.shared_font_system.clone() else {
                        tracing::warn!(
                            "FontSystem not ready, skipping text shaping for song {}",
                            song_id
                        );
                        return Some(Task::none());
                    };

                    // Trigger async text shaping in background thread
                    // This is the key optimization: text shaping is CPU-intensive and should not block the main thread
                    let lines_for_shaping = engine_lines.clone();
                    let song_id = *song_id;
                    let viewport_width = self.ui.lyrics.viewport_width;
                    let viewport_height = self.ui.lyrics.viewport_height;

                    return Some(Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                use crate::features::lyrics::engine::{
                                    CachedShapedLine, SdfPreGenerator, TextShaper,
                                };

                                // Calculate font sizes (same as in LyricsEngine::calculate_line_heights)
                                let content_width = viewport_width * 0.9;
                                let font_size = (viewport_height * 0.055).clamp(24.0, 72.0);
                                let trans_height_ratio = 0.7;
                                let roman_height_ratio = 0.6;
                                let trans_font_size = (font_size * trans_height_ratio).max(10.0);
                                let roman_font_size = (font_size * roman_height_ratio).max(10.0);

                                // Create text shaper with shared font system
                                let text_shaper = TextShaper::new(font_system.clone());

                                // Shape all lines
                                let shaped_lines: Vec<CachedShapedLine> = lines_for_shaping
                                    .iter()
                                    .map(|line| {
                                        // Shape main lyrics
                                        let main_shaped = text_shaper.shape_line(
                                            &line.text,
                                            &line.words,
                                            font_size,
                                            content_width,
                                        );
                                        let mut total_height = main_shaped.height;

                                        // Shape translation line if present
                                        let translation_shaped =
                                            if let Some(ref translated) = line.translated {
                                                if !translated.is_empty() {
                                                    let shaped = text_shaper.shape_simple(
                                                        translated,
                                                        trans_font_size,
                                                        content_width,
                                                    );
                                                    total_height += shaped.height;
                                                    Some(shaped)
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                        // Shape romanized line if present
                                        let romanized_shaped =
                                            if let Some(ref romanized) = line.romanized {
                                                if !romanized.is_empty() {
                                                    let shaped = text_shaper.shape_simple(
                                                        romanized,
                                                        roman_font_size,
                                                        content_width,
                                                    );
                                                    total_height += shaped.height;
                                                    Some(shaped)
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                        CachedShapedLine {
                                            main: main_shaped,
                                            translation: translation_shaped,
                                            romanized: romanized_shaped,
                                            total_height,
                                        }
                                    })
                                    .collect();

                                // Pre-generate SDF glyphs in background thread
                                let start = std::time::Instant::now();
                                let sdf_pre_gen = SdfPreGenerator::new(font_system);

                                // Collect all cache keys from shaped lines
                                let cache_keys: Vec<cosmic_text::CacheKey> = shaped_lines
                                    .iter()
                                    .flat_map(|line| {
                                        let main_keys =
                                            line.main.glyphs.iter().map(|g| g.cache_key);
                                        let trans_keys = line
                                            .translation
                                            .iter()
                                            .flat_map(|t| t.glyphs.iter().map(|g| g.cache_key));
                                        let roman_keys = line
                                            .romanized
                                            .iter()
                                            .flat_map(|r| r.glyphs.iter().map(|g| g.cache_key));
                                        main_keys.chain(trans_keys).chain(roman_keys)
                                    })
                                    .collect();

                                // Pre-generate all SDF glyphs
                                let generated = sdf_pre_gen.generate_all(&cache_keys);
                                let pre_generated_bitmaps = sdf_pre_gen.take_all();

                                tracing::info!(
                                    "Pre-generated {} SDF glyphs in {:?} (total keys: {})",
                                    generated,
                                    start.elapsed(),
                                    cache_keys.len()
                                );

                                (
                                    song_id,
                                    std::sync::Arc::new(shaped_lines),
                                    pre_generated_bitmaps,
                                )
                            })
                            .await
                            .ok()
                        },
                        |result| {
                            if let Some((song_id, shaped_lines, pre_generated_bitmaps)) = result {
                                Message::LyricsShapedLinesReady(
                                    song_id,
                                    shaped_lines,
                                    pre_generated_bitmaps,
                                )
                            } else {
                                Message::Noop
                            }
                        },
                    ));
                }
                Some(Task::none())
            }

            // Handle pre-computed shaped lines (Single Source of Truth for text layout)
            Message::LyricsShapedLinesReady(song_id, shaped_lines, pre_generated_bitmaps) => {
                if self.ui.lyrics.loading_song_id == Some(*song_id) {
                    self.ui.lyrics.cached_shaped_lines = Some(shaped_lines.clone());
                    tracing::info!(
                        "Shaped lines ready for song {}: {} lines",
                        song_id,
                        shaped_lines.len()
                    );

                    // Update engine with pre-computed shaped lines
                    if let Some(engine_cell) = &self.ui.lyrics.engine {
                        let mut engine = engine_cell.borrow_mut();
                        engine.set_cached_shaped_lines(shaped_lines.as_ref().clone());
                    }

                    // Import pre-generated MSDF bitmaps to global cache
                    // The GPU pipeline will use these during first render
                    if !pre_generated_bitmaps.is_empty() {
                        crate::features::lyrics::engine::sdf_cache::import_to_global_cache(
                            pre_generated_bitmaps.clone(),
                        );
                        tracing::info!(
                            "Imported {} pre-generated MSDF bitmaps to global cache for song {}",
                            pre_generated_bitmaps.len(),
                            song_id
                        );
                    }
                }
                Some(Task::none())
            }

            // NEW: Handle async background colors
            Message::LyricsBackgroundReady(song_id, primary, secondary, tertiary) => {
                // Only apply if this is still the current song
                if self.library.current_song.as_ref().map(|s| s.id) == Some(*song_id) {
                    self.ui
                        .lyrics
                        .bg_shader
                        .set_colors(*primary, *secondary, *tertiary);

                    // Convert to iced Color for bg_colors
                    self.ui.lyrics.bg_colors = crate::utils::DominantColors {
                        primary: iced::Color::from_rgba(
                            primary[0], primary[1], primary[2], primary[3],
                        ),
                        secondary: iced::Color::from_rgba(
                            secondary[0],
                            secondary[1],
                            secondary[2],
                            secondary[3],
                        ),
                        tertiary: iced::Color::from_rgba(
                            tertiary[0],
                            tertiary[1],
                            tertiary[2],
                            tertiary[3],
                        ),
                        brightness: (primary[0] * 0.299 + primary[1] * 0.587 + primary[2] * 0.114),
                    };

                    tracing::debug!("Applied background colors for song {}", song_id);
                }
                Some(Task::none())
            }

            // NEW: Handle async cover image loading for textured background
            Message::LyricsCoverImageReady(song_id, image_data, width, height) => {
                if self.library.current_song.as_ref().map(|s| s.id) == Some(*song_id) {
                    // Convert raw bytes back to DynamicImage
                    if let Some(img) =
                        image::RgbImage::from_raw(*width, *height, image_data.clone())
                    {
                        let dynamic_img = image::DynamicImage::ImageRgb8(img);
                        self.ui
                            .lyrics
                            .textured_bg_shader
                            .set_album_image(dynamic_img, None);
                        tracing::debug!(
                            "Applied cover image for song {} ({}x{})",
                            song_id,
                            width,
                            height
                        );
                    }
                }
                Some(Task::none())
            }

            _ => None,
        }
    }

    /// Apply lyrics lines to state (shared by online and local loading)
    fn apply_lyrics_lines(&mut self, lines: Vec<crate::ui::pages::LyricLine>) {
        self.ui.lyrics.lines = lines;
        self.ui.lyrics.cached_engine_lines = None;
        self.ui.lyrics.cached_shaped_lines = None; // Clear shaped lines cache
        self.ui.lyrics.is_loading = false;
        self.ui.lyrics.load_error = None;
        self.ui.lyrics.current_line_idx = None;

        // Clear engine's cached data for re-layout, but keep the engine instance
        // (engine is pre-created at app startup to avoid FontSystem::new() delay)
        if let Some(engine_cell) = &self.ui.lyrics.engine {
            let mut engine = engine_cell.borrow_mut();
            // Clear cached shaped lines to force re-calculation
            engine.set_cached_shaped_lines(Vec::new());
        }
    }

    /// Check if lyrics page should be fully closed (animation complete)
    pub fn check_lyrics_page_close(&mut self) {
        let now = Instant::now();
        let progress = self.ui.lyrics.animation.progress(now);
        if progress < 0.01 && !self.ui.lyrics.animation.is_animating(now) && self.ui.lyrics.is_open
        {
            self.ui.lyrics.is_open = false;
        }
    }

    /// Update lyrics line animations based on current playback position
    pub fn update_lyrics_animations(&mut self) -> Task<Message> {
        let now = std::time::Instant::now();
        let delta_secs = if let Some(last) = self.ui.lyrics.last_update {
            let delta = now.duration_since(last).as_secs_f32();
            delta.clamp(0.001, 0.1)
        } else {
            0.016
        };
        self.ui.lyrics.last_update = Some(now);

        if let Some(start_time) = self.ui.lyrics.shader_start_time {
            let elapsed_ms = now.duration_since(start_time).as_secs_f32() * 1000.0;
            let shader_time = elapsed_ms / 10000.0;
            self.ui.lyrics.bg_shader.set_time(elapsed_ms);
            self.ui.lyrics.textured_bg_shader.set_time(shader_time);
            self.ui
                .lyrics
                .textured_bg_shader
                .update(delta_secs * 1000.0);
        }

        if self.ui.lyrics.lines.is_empty() {
            return Task::none();
        }

        let position_ms = if let Some(player) = &self.core.audio {
            let info = player.get_info();
            if info.duration.as_secs_f32() > 0.0 {
                (info.position.as_secs_f32() * 1000.0) as u64
            } else {
                0
            }
        } else {
            0
        };

        let new_current_line =
            crate::ui::pages::find_current_line(&self.ui.lyrics.lines, position_ms);

        if new_current_line != self.ui.lyrics.current_line_idx {
            self.ui.lyrics.current_line_idx = new_current_line;
        }

        self.update_scroll_bounce_back(delta_secs);
        self.update_lyrics_engine(delta_secs);

        Task::none()
    }

    /// Update lyrics engine with current state
    fn update_lyrics_engine(&mut self, delta_secs: f32) {
        // Engine is now pre-created at app startup, so just check if lines changed
        let just_initialized = false;

        let engine_lines = self.get_or_create_engine_lines();

        let user_scrolling = self.ui.lyrics.user_scrolling;
        let manual_scroll_offset = self.ui.lyrics.manual_scroll_offset;
        let content_width = self.ui.lyrics.viewport_width * 0.9;
        let font_size = (self.ui.lyrics.viewport_height * 0.055).clamp(24.0, 72.0);
        let viewport_height = self.ui.lyrics.viewport_height;

        let time_ms = if let Some(player) = &self.core.audio {
            let info = player.get_info();
            if info.duration.as_secs_f32() > 0.0 {
                info.position.as_secs_f64() * 1000.0
            } else {
                self.library
                    .playback_state
                    .as_ref()
                    .map(|s| s.position_secs * 1000.0)
                    .unwrap_or(0.0)
            }
        } else {
            self.library
                .playback_state
                .as_ref()
                .map(|s| s.position_secs * 1000.0)
                .unwrap_or(0.0)
        };

        let is_playing = self
            .core
            .audio
            .as_ref()
            .map(|player| {
                let info = player.get_info();
                info.status == crate::audio::PlaybackStatus::Playing
            })
            .unwrap_or(false);

        if let Some(engine_cell) = &self.ui.lyrics.engine {
            let mut engine = engine_cell.borrow_mut();

            engine.update(delta_secs);

            if user_scrolling {
                engine.handle_wheel(manual_scroll_offset);
            }

            engine.set_viewport_info(&engine_lines, content_width, font_size, viewport_height);

            if is_playing {
                engine.resume();
            } else {
                engine.pause();
            }

            engine.set_current_time(time_ms, &engine_lines, just_initialized);
        }

        if user_scrolling {
            self.ui.lyrics.manual_scroll_offset = 0.0;
        }
    }

    /// Get or create cached engine lines
    fn get_or_create_engine_lines(
        &mut self,
    ) -> std::sync::Arc<Vec<crate::features::lyrics::engine::LyricLineData>> {
        let cache_valid = self
            .ui
            .lyrics
            .cached_engine_lines
            .as_ref()
            .map(|cached| cached.len() == self.ui.lyrics.lines.len())
            .unwrap_or(false);

        if cache_valid {
            return self.ui.lyrics.cached_engine_lines.clone().unwrap();
        }

        let engine_lines: Vec<crate::features::lyrics::engine::LyricLineData> = self
            .ui
            .lyrics
            .lines
            .iter()
            .map(|line| {
                let mut line_data = crate::features::lyrics::engine::LyricLineData {
                    text: line.text.clone(),
                    words: {
                        let word_count = line.words.len();
                        line.words
                            .iter()
                            .enumerate()
                            .map(|(i, w)| crate::features::lyrics::engine::WordData {
                                text: w.word.clone(),
                                start_ms: w.start_ms,
                                end_ms: w.end_ms,
                                roman_word: None,
                                emphasize: false,
                                x_start: 0.0,
                                x_end: 0.0,
                                is_last_word: i == word_count.saturating_sub(1),
                            })
                            .collect()
                    },
                    translated: line.translated.clone(),
                    romanized: line.romanized.clone(),
                    start_ms: line.start_ms,
                    end_ms: line.end_ms,
                    is_duet: line.is_duet,
                    is_bg: line.is_background,
                    mask_animation: None,
                };
                line_data.compute_mask_animation();
                line_data
            })
            .collect();

        let arc = std::sync::Arc::new(engine_lines);
        self.ui.lyrics.cached_engine_lines = Some(arc.clone());
        arc
    }

    /// Handle scroll bounce-back after user inactivity
    fn update_scroll_bounce_back(&mut self, delta_secs: f32) {
        const BOUNCE_BACK_DELAY_SECS: f32 = 3.0;
        const BOUNCE_BACK_SPEED: f32 = 8.0;

        if let Some(last_scroll) = self.ui.lyrics.last_scroll_time {
            let elapsed = std::time::Instant::now()
                .duration_since(last_scroll)
                .as_secs_f32();

            if elapsed > BOUNCE_BACK_DELAY_SECS {
                let lerp_factor = 1.0 - (-BOUNCE_BACK_SPEED * delta_secs).exp();
                self.ui.lyrics.manual_scroll_offset *= 1.0 - lerp_factor;

                if self.ui.lyrics.manual_scroll_offset.abs() < 1.0 {
                    self.ui.lyrics.manual_scroll_offset = 0.0;
                    self.ui.lyrics.user_scrolling = false;
                    self.ui.lyrics.last_scroll_time = None;
                }
            }
        }
    }

    /// Handle user scroll event on lyrics
    pub fn handle_lyrics_scroll(&mut self, delta: f32) {
        tracing::debug!("Lyrics scroll: delta={}", delta);
        self.ui.lyrics.user_scrolling = true;
        self.ui.lyrics.last_scroll_time = Some(std::time::Instant::now());
        self.ui.lyrics.manual_scroll_offset += delta;
    }

    // ============ ASYNC LOADING METHODS ============

    /// Load lyrics asynchronously (local, cached, or online)
    /// This is the main entry point for lyrics loading
    pub fn load_lyrics_async(&mut self, song: &crate::database::DbSong) -> Task<Message> {
        tracing::info!(
            "load_lyrics_async called for song: {} (id={})",
            song.title,
            song.id
        );

        // Clear current state immediately (non-blocking)
        self.ui.lyrics.lines.clear();
        self.ui.lyrics.cached_engine_lines = None;
        self.ui.lyrics.cached_shaped_lines = None;
        self.ui.lyrics.current_line_idx = None;
        self.ui.lyrics.load_error = None;
        self.ui.lyrics.loading_song_id = Some(song.id);
        self.ui.lyrics.is_loading = true;

        // Clear engine's cached data for re-layout, but keep the engine instance
        if let Some(engine_cell) = &self.ui.lyrics.engine {
            let mut engine = engine_cell.borrow_mut();
            engine.set_cached_shaped_lines(Vec::new());
        }

        let song_id = song.id;
        let file_path = song.file_path.clone();
        let is_ncm = song.id < 0;
        let ncm_id = if is_ncm { (-song.id) as u64 } else { 0 };

        // Also start background color extraction
        let bg_task = self.update_background_async(song);

        // Create async task for lyrics loading
        // CRITICAL: Use spawn_blocking for synchronous I/O operations
        let lyrics_task = Task::perform(
            async move {
                // Use spawn_blocking to move sync I/O to blocking thread pool
                tokio::task::spawn_blocking(move || {
                    // Priority 1: Local lyrics file or embedded
                    if !file_path.is_empty() {
                        let audio_path = std::path::Path::new(&file_path);
                        if let Some(lrc_lines) =
                            crate::features::media::lyrics::find_lyrics(audio_path)
                        {
                            let ui_lines =
                                crate::features::media::lyrics::to_ui_lyric_lines(lrc_lines);
                            return Some((song_id, ui_lines, false)); // false = no online fetch needed
                        }
                    }

                    // Priority 2: Cached online lyrics (for NCM songs)
                    if is_ncm {
                        if let Some(cached_lines) =
                            crate::features::lyrics::load_cached_lyrics(ncm_id)
                        {
                            let ui_lines = crate::features::lyrics::to_ui_lyrics(cached_lines);
                            return Some((song_id, ui_lines, false));
                        }
                        // Need online fetch
                        return Some((song_id, Vec::new(), true)); // true = need online fetch
                    }

                    // No lyrics found for local song
                    Some((song_id, Vec::new(), false))
                })
                .await
                .ok()
                .flatten()
            },
            |result| {
                match result {
                    Some((song_id, lines, needs_online)) => {
                        if needs_online {
                            // Trigger online fetch via PreloadLyrics
                            let ncm_id = (-song_id) as u64;
                            Message::PreloadLyrics(
                                song_id,
                                ncm_id,
                                String::new(),
                                String::new(),
                                String::new(),
                            )
                        } else if !lines.is_empty() {
                            Message::LocalLyricsReady(song_id, lines)
                        } else {
                            // No lyrics found, just mark as not loading
                            Message::LyricsLoadFailed(song_id, "No lyrics found".to_string())
                        }
                    }
                    None => Message::Noop,
                }
            },
        );

        Task::batch([bg_task, lyrics_task])
    }

    /// Update background asynchronously (color extraction + texture)
    fn update_background_async(&mut self, song: &crate::database::DbSong) -> Task<Message> {
        let song_id = song.id;
        let cover_path = song.cover_path.clone();

        // Reset shader time if needed
        if self.ui.lyrics.shader_start_time.is_none() {
            self.ui.lyrics.shader_start_time = Some(std::time::Instant::now());
        }

        // If no cover, just clear and return
        let Some(path) = cover_path else {
            self.ui.lyrics.textured_bg_shader.clear_cover();
            return Task::none();
        };

        // Skip if cover is a URL (not downloaded yet)
        if path.starts_with("http://") || path.starts_with("https://") {
            tracing::debug!("Cover is URL, waiting for download: {}", path);
            return Task::none();
        }

        // Check if we already have this image cached (fast path)
        let path_obj = std::path::Path::new(&path);
        if self.ui.lyrics.textured_bg_shader.is_same_image(path_obj) {
            tracing::debug!("Cover image already cached for song {}", song_id);
            return Task::none();
        }

        // Load both image and colors asynchronously
        let path_for_image = path.clone();
        let path_for_colors = path.clone();

        // Task 1: Load cover image for textured background
        let image_task = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || match image::open(&path_for_image) {
                    Ok(img) => {
                        let rgb = img.to_rgb8();
                        let (width, height) = rgb.dimensions();
                        let data = rgb.into_raw();
                        Some((song_id, data, width, height))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load cover image: {}", e);
                        None
                    }
                })
                .await
                .ok()
                .flatten()
            },
            |result| match result {
                Some((song_id, data, width, height)) => {
                    Message::LyricsCoverImageReady(song_id, data, width, height)
                }
                None => Message::Noop,
            },
        );

        // Task 2: Extract colors
        let colors_task = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    if let Some(colors) =
                        crate::utils::DominantColors::from_image_path(&path_for_colors)
                    {
                        let primary = color_to_array(colors.primary);
                        let secondary = color_to_array(colors.secondary);
                        let tertiary = color_to_array(colors.tertiary);
                        Some((song_id, primary, secondary, tertiary))
                    } else {
                        None
                    }
                })
                .await
                .ok()
                .flatten()
            },
            |result| match result {
                Some((song_id, primary, secondary, tertiary)) => {
                    Message::LyricsBackgroundReady(song_id, primary, secondary, tertiary)
                }
                None => Message::Noop,
            },
        );

        Task::batch([image_task, colors_task])
    }

    /// 更新歌词页面背景（切歌时调用）
    ///
    /// 返回一个 Task 用于异步加载歌词和背景
    pub fn update_lyrics_background(&mut self, song: &crate::database::DbSong) -> Task<Message> {
        // Use the new async loading
        self.load_lyrics_async(song)
    }

    /// 只更新歌词页面背景（封面下载完成后调用）
    /// 不重新加载歌词
    pub fn update_lyrics_background_only(
        &mut self,
        song: &crate::database::DbSong,
    ) -> Task<Message> {
        self.update_background_async(song)
    }
}
