// src/app/update/lyrics.rs
//! Lyrics page message handlers
//!
//! Architecture: Async-first loading to prevent UI blocking
//! - Background colors: extracted asynchronously
//! - Local/cached lyrics: loaded asynchronously
//! - Online lyrics: fetched asynchronously (already was)

use iced::Task;

use crate::app::lyrics_cache_manager::DisplayFetchAction;
use crate::app::message::Message;
use crate::app::state::App;
use crate::ui::effects::background::color_to_array;

impl App {
    pub(super) fn clear_lyrics_page_cache(&mut self) {
        self.ui.lyrics.displayed_song_id = None;
        self.ui.lyrics.pending_song_id = None;
        self.ui.lyrics.lines.clear();
        self.ui.lyrics.cached_engine_lines = None;
        self.ui.lyrics.cached_shaped_lines = None;
        self.ui.lyrics.current_line_idx = None;
        self.ui.lyrics.last_update = None;
        self.ui.lyrics.shaped_content_width = 0.0;
        self.ui.lyrics.shaped_font_size = 0.0;
        self.ui.lyrics.shape_generation = self.ui.lyrics.shape_generation.wrapping_add(1);
        self.ui.lyrics.is_loading = false;
        self.ui.lyrics.load_error = None;

        if let Some(engine_cell) = &self.ui.lyrics.engine {
            let mut engine = engine_cell.borrow_mut();
            engine.reset_for_new_lyrics();
            engine.set_cached_shaped_lines(Vec::new());
        }
    }

    /// Handle lyrics page related messages
    pub fn handle_lyrics(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::OpenLyricsPage => {
                // Only open if there's a song playing
                if let Some(song) = self.playback.current_song.clone() {
                    self.ui.lyrics.is_open = true;
                    self.ui.lyrics.animation.start();

                    let lyrics_need_load = self.should_load_lyrics_for_song(song.id);

                    if lyrics_need_load {
                        tracing::debug!("Loading lyrics for song: {} (id={})", song.title, song.id);
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
                self.ui.lyrics.animation.stop();
                Some(Task::none())
            }

            &Message::LyricsScroll(delta) => {
                self.handle_lyrics_scroll(delta);
                Some(Task::none())
            }

            Message::LyricsViewportResized(size) => {
                if (self.ui.lyrics.viewport_width - size.width).abs() < 0.5
                    && (self.ui.lyrics.viewport_height - size.height).abs() < 0.5
                {
                    return Some(Task::none());
                }

                self.ui.lyrics.viewport_width = size.width.max(100.0);
                self.ui.lyrics.viewport_height = size.height.max(100.0);
                self.ui.lyrics.viewport_initialized = true;

                if let Some(engine_cell) = &self.ui.lyrics.engine {
                    let mut engine = engine_cell.borrow_mut();
                    engine
                        .line_animations_mut()
                        .set_viewport_height(self.ui.lyrics.viewport_height);
                    engine.invalidate_layout();
                }

                Some(self.request_lyrics_shaping_for_current_viewport())
            }

            Message::WindowResized(size) => {
                self.ui.lyrics.viewport_width = (size.width * 0.6 - 60.0).max(100.0);
                self.ui.lyrics.viewport_height = size.height;
                self.ui.lyrics.viewport_initialized = true;

                if let Some(engine_cell) = &self.ui.lyrics.engine {
                    let mut engine = engine_cell.borrow_mut();
                    engine
                        .line_animations_mut()
                        .set_viewport_height(size.height);

                    // Force re-layout by invalidating cached dimensions
                    engine.invalidate_layout();
                }

                // Fallback widths before Sensor reports the rendered content size.
                const GRID_PADDING: f32 = 64.0;
                const DETAIL_GRID_PADDING: f32 = 96.0;
                let available_width = (size.width - self.ui.sidebar_width).max(200.0);
                self.ui.discover.content_width = (available_width - GRID_PADDING).max(200.0);
                self.ui.playlist_page.content_width =
                    (available_width - DETAIL_GRID_PADDING).max(200.0);
                self.ui.search.content_width = (available_width - GRID_PADDING).max(200.0);

                Some(self.request_lyrics_shaping_for_current_viewport())
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

                if let (Some(engine_cell), Some(shaped_lines)) = (
                    &self.ui.lyrics.engine,
                    self.ui.lyrics.cached_shaped_lines.as_ref(),
                ) {
                    let mut engine = engine_cell.borrow_mut();
                    engine.set_cached_shaped_lines_with_metrics(
                        shaped_lines.as_ref().clone(),
                        self.ui.lyrics.shaped_content_width,
                        self.ui.lyrics.shaped_font_size,
                    );
                }

                Some(self.request_lyrics_shaping_for_current_viewport())
            }

            Message::FetchLyricsOnline(song_id, ncm_id) => {
                let song_id = *song_id;
                let ncm_id = *ncm_id;

                if self.ui.lyrics.pending_song_id != Some(song_id) {
                    return Some(Task::none());
                }

                match self
                    .playback
                    .lyrics_cache_manager
                    .register_display_fetch(song_id, ncm_id)
                {
                    DisplayFetchAction::UseCache => {
                        Some(self.resume_display_lyrics_load_from_cache(song_id))
                    }
                    DisplayFetchAction::AwaitExisting => Some(Task::none()),
                    DisplayFetchAction::StartFetch => {
                        if let Some(client) = self.core.ncm_client.clone() {
                            Some(Task::perform(
                                async move {
                                    match crate::features::lyrics::fetch_lyrics(&client, ncm_id)
                                        .await
                                    {
                                        Ok(lines) => {
                                            let ui_lines =
                                                crate::features::lyrics::to_ui_lyrics(lines);
                                            Message::LyricsLoaded(song_id, ui_lines)
                                        }
                                        Err(e) => Message::LyricsLoadFailed(song_id, e.to_string()),
                                    }
                                },
                                |msg| msg,
                            ))
                        } else {
                            self.playback
                                .lyrics_cache_manager
                                .finish_warmup(song_id, Err("No NCM client".to_string()));
                            Some(Task::done(Message::LyricsLoadFailed(
                                song_id,
                                "No NCM client".to_string(),
                            )))
                        }
                    }
                }
            }

            Message::WarmLyricsCache(song_id, ncm_id) => {
                let song_id = *song_id;
                let ncm_id = *ncm_id;

                if !self
                    .playback
                    .lyrics_cache_manager
                    .begin_warmup(song_id, ncm_id)
                {
                    return Some(Task::none());
                }

                if let Some(client) = self.core.ncm_client.clone() {
                    Some(Task::perform(
                        async move {
                            crate::features::lyrics::fetch_lyrics(&client, ncm_id)
                                .await
                                .map(|_| ())
                                .map_err(|err| err.to_string())
                        },
                        move |result| Message::LyricsWarmupFinished(song_id, result),
                    ))
                } else {
                    Some(Task::done(Message::LyricsWarmupFinished(
                        song_id,
                        Err("No NCM client".to_string()),
                    )))
                }
            }

            Message::LyricsWarmupFinished(song_id, result) => {
                match result {
                    Ok(()) => tracing::debug!("Lyrics warmup completed for song {}", song_id),
                    Err(error) => {
                        tracing::debug!("Lyrics warmup failed for song {}: {}", song_id, error)
                    }
                }
                self.playback
                    .lyrics_cache_manager
                    .finish_warmup(*song_id, result.clone());

                if self.ui.lyrics.pending_song_id == Some(*song_id) {
                    return Some(match result {
                        Ok(()) => self.resume_display_lyrics_load_from_cache(*song_id),
                        Err(error) => {
                            Task::done(Message::LyricsLoadFailed(*song_id, error.clone()))
                        }
                    });
                }
                Some(Task::none())
            }

            Message::LyricsLoaded(song_id, lines) => {
                if self.ui.lyrics.pending_song_id == Some(*song_id) {
                    self.note_lyrics_cache_ready_if_available(*song_id);
                    self.apply_lyrics_lines(*song_id, lines.clone());
                    tracing::info!(
                        "Loaded {} online lyrics lines for song {}",
                        lines.len(),
                        song_id
                    );

                    return Some(Self::prepare_engine_lines_task(*song_id, lines.clone()));
                }
                Some(Task::none())
            }

            Message::LyricsLoadFailed(song_id, error) => {
                if self.ui.lyrics.pending_song_id == Some(*song_id) {
                    if *song_id < 0 {
                        self.playback
                            .lyrics_cache_manager
                            .finish_warmup(*song_id, Err(error.clone()));
                    }
                    self.apply_lyrics_error(*song_id, error.clone());
                    tracing::warn!("Failed to load lyrics for song {}: {}", song_id, error);
                }
                Some(Task::none())
            }

            Message::LocalLyricsReady(song_id, lines) => {
                if self.ui.lyrics.pending_song_id == Some(*song_id) {
                    self.note_lyrics_cache_ready_if_available(*song_id);
                    self.apply_lyrics_lines(*song_id, lines.clone());
                    tracing::info!(
                        "Loaded {} local/cached lyrics lines for song {}",
                        lines.len(),
                        song_id
                    );

                    return Some(Self::prepare_engine_lines_task(*song_id, lines.clone()));
                }
                Some(Task::none())
            }

            // Handle pre-computed engine lines
            Message::LyricsEngineLinesReady(song_id, engine_lines) => {
                if self.ui.lyrics.displayed_song_id == Some(*song_id) {
                    self.ui.lyrics.cached_engine_lines = Some(engine_lines.clone());
                    tracing::info!(
                        "Engine lines ready for song {}: {} lines",
                        song_id,
                        engine_lines.len()
                    );

                    return Some(self.request_lyrics_shaping_for_current_viewport());
                }
                Some(Task::none())
            }

            // Handle pre-computed shaped lines (Single Source of Truth for text layout)
            Message::LyricsShapedLinesReady(
                song_id,
                generation,
                shaped_lines,
                pre_generated_bitmaps,
                content_width,
                font_size,
            ) => {
                if self.ui.lyrics.displayed_song_id == Some(*song_id)
                    && self.ui.lyrics.shape_generation == *generation
                {
                    self.ui.lyrics.cached_shaped_lines = Some(shaped_lines.clone());
                    self.ui.lyrics.shaped_content_width = *content_width;
                    self.ui.lyrics.shaped_font_size = *font_size;
                    tracing::info!(
                        "Shaped lines ready for song {}: {} lines",
                        song_id,
                        shaped_lines.len()
                    );

                    // Update engine with pre-computed shaped lines
                    if let Some(engine_cell) = &self.ui.lyrics.engine {
                        let mut engine = engine_cell.borrow_mut();
                        engine.set_cached_shaped_lines_with_metrics(
                            shaped_lines.as_ref().clone(),
                            *content_width,
                            *font_size,
                        );
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
                if self.playback.current_song.as_ref().map(|s| s.id) == Some(*song_id) {
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
                if self.playback.current_song.as_ref().map(|s| s.id) == Some(*song_id) {
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

    fn current_lyrics_shape_metrics(&self) -> Option<(f32, f32)> {
        if !self.ui.lyrics.viewport_initialized {
            return None;
        }

        let viewport_width = self.ui.lyrics.viewport_width.max(100.0);
        let viewport_height = self.ui.lyrics.viewport_height.max(100.0);
        let content_width = viewport_width * 0.9;
        let font_size = crate::features::lyrics::engine::FontSizeConfig::default()
            .calculate_font_size(viewport_width, viewport_height);

        Some((content_width, font_size))
    }

    fn request_lyrics_shaping_for_current_viewport(&mut self) -> Task<Message> {
        let Some(lines_for_shaping) = self.ui.lyrics.cached_engine_lines.clone() else {
            return Task::none();
        };
        let Some(font_system) = self.ui.lyrics.shared_font_system.clone() else {
            return Task::none();
        };
        let Some(song_id) = self.ui.lyrics.displayed_song_id else {
            return Task::none();
        };
        let Some((content_width, font_size)) = self.current_lyrics_shape_metrics() else {
            return Task::none();
        };

        let cache_matches_current = self
            .ui
            .lyrics
            .cached_shaped_lines
            .as_ref()
            .map(|lines| lines.len() == lines_for_shaping.len())
            .unwrap_or(false)
            && (self.ui.lyrics.shaped_content_width - content_width).abs() <= 1.0
            && (self.ui.lyrics.shaped_font_size - font_size).abs() <= 0.1;

        if cache_matches_current {
            return Task::none();
        }

        self.ui.lyrics.shape_generation = self.ui.lyrics.shape_generation.wrapping_add(1);
        let generation = self.ui.lyrics.shape_generation;

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    use crate::features::lyrics::engine::{
                        CachedShapedLine, SdfPreGenerator, TextShaper,
                    };

                    let trans_height_ratio = 0.5;
                    let roman_height_ratio = 0.5;
                    let bg_font_size_ratio = 0.7;

                    let text_shaper = TextShaper::new(font_system.clone());

                    let shaped_lines: Vec<CachedShapedLine> = lines_for_shaping
                        .iter()
                        .map(|line| {
                            let main_font_size = if line.is_bg {
                                font_size * bg_font_size_ratio
                            } else {
                                font_size
                            };
                            let trans_font_size = (main_font_size * trans_height_ratio).max(10.0);
                            let roman_font_size = (main_font_size * roman_height_ratio).max(10.0);

                            let main_shaped = text_shaper.shape_line(
                                &line.text,
                                &line.words,
                                main_font_size,
                                content_width,
                            );
                            let mut total_height = main_shaped.height;

                            let translation_shaped = if let Some(ref translated) = line.translated {
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

                            let romanized_shaped = if let Some(ref romanized) = line.romanized {
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
                                main_font_size,
                                translation: translation_shaped,
                                translation_font_size: trans_font_size,
                                romanized: romanized_shaped,
                                romanized_font_size: roman_font_size,
                                total_height,
                            }
                        })
                        .collect();

                    let start = std::time::Instant::now();
                    let sdf_pre_gen = SdfPreGenerator::new(font_system);

                    let cache_keys: Vec<cosmic_text::CacheKey> = shaped_lines
                        .iter()
                        .flat_map(|line| {
                            let main_keys = line.main.glyphs.iter().map(|g| g.cache_key);
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
                        generation,
                        std::sync::Arc::new(shaped_lines),
                        pre_generated_bitmaps,
                        content_width,
                        font_size,
                    )
                })
                .await
                .ok()
            },
            |result| {
                if let Some((
                    song_id,
                    generation,
                    shaped_lines,
                    pre_generated_bitmaps,
                    content_width,
                    font_size,
                )) = result
                {
                    Message::LyricsShapedLinesReady(
                        song_id,
                        generation,
                        shaped_lines,
                        pre_generated_bitmaps,
                        content_width,
                        font_size,
                    )
                } else {
                    Message::Noop
                }
            },
        )
    }

    fn should_load_lyrics_for_song(&self, song_id: i64) -> bool {
        (self.ui.lyrics.displayed_song_id != Some(song_id)
            && self.ui.lyrics.pending_song_id != Some(song_id))
            || self.ui.lyrics.load_error.is_some()
    }

    fn prepare_display_lyrics_load(&mut self, song_id: i64) {
        self.ui.lyrics.displayed_song_id = None;
        self.ui.lyrics.pending_song_id = Some(song_id);
        self.ui.lyrics.lines.clear();
        self.ui.lyrics.cached_engine_lines = None;
        self.ui.lyrics.cached_shaped_lines = None;
        self.ui.lyrics.shaped_content_width = 0.0;
        self.ui.lyrics.shaped_font_size = 0.0;
        self.ui.lyrics.shape_generation = self.ui.lyrics.shape_generation.wrapping_add(1);
        self.ui.lyrics.current_line_idx = None;
        self.ui.lyrics.is_loading = true;
        self.ui.lyrics.load_error = None;

        if let Some(engine_cell) = &self.ui.lyrics.engine {
            let mut engine = engine_cell.borrow_mut();
            engine.reset_for_new_lyrics();
            engine.set_cached_shaped_lines(Vec::new());
        }
    }

    fn apply_lyrics_lines(&mut self, song_id: i64, lines: Vec<crate::ui::pages::LyricLine>) {
        self.ui.lyrics.displayed_song_id = Some(song_id);
        self.ui.lyrics.pending_song_id = None;
        self.ui.lyrics.lines = lines;
        self.ui.lyrics.cached_engine_lines = None;
        self.ui.lyrics.cached_shaped_lines = None;
        self.ui.lyrics.shaped_content_width = 0.0;
        self.ui.lyrics.shaped_font_size = 0.0;
        self.ui.lyrics.is_loading = false;
        self.ui.lyrics.load_error = None;
        self.ui.lyrics.current_line_idx = None;

        // Clear engine's cached data for re-layout, but keep the engine instance
        // (engine is pre-created at app startup to avoid FontSystem::new() delay)
        if let Some(engine_cell) = &self.ui.lyrics.engine {
            let mut engine = engine_cell.borrow_mut();
            engine.reset_for_new_lyrics();
            engine.set_cached_shaped_lines(Vec::new());
        }
    }

    fn apply_lyrics_error(&mut self, song_id: i64, error: String) {
        if self.ui.lyrics.pending_song_id != Some(song_id) {
            return;
        }

        self.ui.lyrics.displayed_song_id = None;
        self.ui.lyrics.pending_song_id = None;
        self.ui.lyrics.lines.clear();
        self.ui.lyrics.cached_engine_lines = None;
        self.ui.lyrics.cached_shaped_lines = None;
        self.ui.lyrics.shaped_content_width = 0.0;
        self.ui.lyrics.shaped_font_size = 0.0;
        self.ui.lyrics.shape_generation = self.ui.lyrics.shape_generation.wrapping_add(1);
        self.ui.lyrics.is_loading = false;
        self.ui.lyrics.load_error = Some(error);
        self.ui.lyrics.current_line_idx = None;

        if let Some(engine_cell) = &self.ui.lyrics.engine {
            let mut engine = engine_cell.borrow_mut();
            engine.set_cached_shaped_lines(Vec::new());
        }
    }

    fn note_lyrics_cache_ready_if_available(&mut self, song_id: i64) {
        if song_id < 0 {
            let ncm_id = (-song_id) as u64;
            if crate::features::lyrics::load_cached_lyrics(ncm_id).is_some() {
                self.playback
                    .lyrics_cache_manager
                    .mark_ready(song_id, ncm_id);
            }
        }
    }

    fn resume_display_lyrics_load_from_cache(&mut self, song_id: i64) -> Task<Message> {
        let ncm_id = if song_id < 0 { (-song_id) as u64 } else { 0 };

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    if song_id < 0 {
                        crate::features::lyrics::load_cached_lyrics(ncm_id)
                            .map(|cached_lines| crate::features::lyrics::to_ui_lyrics(cached_lines))
                    } else {
                        None
                    }
                })
                .await
                .ok()
                .flatten()
            },
            move |lines| match lines {
                Some(lines) => Message::LocalLyricsReady(song_id, lines),
                None => Message::LyricsLoadFailed(
                    song_id,
                    "Cached lyrics unavailable after warmup".to_string(),
                ),
            },
        )
    }

    fn prepare_engine_lines_task(
        song_id: i64,
        lines_for_task: Vec<crate::ui::pages::LyricLine>,
    ) -> Task<Message> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let engine_lines: Vec<crate::features::lyrics::engine::LyricLineData> =
                        lines_for_task
                            .iter()
                            .map(|line| {
                                let word_count = line.words.len();
                                crate::features::lyrics::engine::LyricLineData {
                                    text: line.text.clone(),
                                    words: line
                                        .words
                                        .iter()
                                        .enumerate()
                                        .map(|(i, w)| crate::features::lyrics::engine::WordData {
                                            text: w.word.clone(),
                                            start_ms: w.start_ms,
                                            end_ms: w.end_ms,
                                            emphasize: false,
                                            is_last_word: i == word_count.saturating_sub(1),
                                        })
                                        .collect(),
                                    translated: line.translated.clone(),
                                    romanized: line.romanized.clone(),
                                    start_ms: line.start_ms,
                                    end_ms: line.end_ms,
                                    is_duet: line.is_duet,
                                    is_bg: line.is_background,
                                }
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
        )
    }

    /// Check if lyrics page should be fully closed (animation complete)
    pub fn check_lyrics_page_close(&mut self) {
        let progress = self.ui.lyrics.animation.progress();
        if progress < 0.01 && !self.ui.lyrics.animation.is_animating() && self.ui.lyrics.is_open {
            self.ui.lyrics.is_open = false;
            self.clear_lyrics_page_cache();
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

        let runtime = self.playback_runtime();
        let position_ms = if runtime.has_loaded_audio && runtime.info.duration.as_secs_f32() > 0.0 {
            (runtime.info.position.as_secs_f32() * 1000.0) as u64
        } else {
            0
        };

        let new_current_line =
            crate::ui::pages::find_current_line(&self.ui.lyrics.lines, position_ms);

        if new_current_line != self.ui.lyrics.current_line_idx {
            self.ui.lyrics.current_line_idx = new_current_line;
        }

        self.update_lyrics_engine(delta_secs);

        Task::none()
    }

    /// Update lyrics engine with current state
    fn update_lyrics_engine(&mut self, delta_secs: f32) {
        // Engine is now pre-created at app startup, so just check if lines changed
        let just_initialized = false;

        let engine_lines = self.get_or_create_engine_lines();

        let content_width = self.ui.lyrics.viewport_width * 0.9;
        let font_size = crate::features::lyrics::engine::FontSizeConfig::default()
            .calculate_font_size(
                self.ui.lyrics.viewport_width,
                self.ui.lyrics.viewport_height,
            );
        let viewport_height = self.ui.lyrics.viewport_height;

        let runtime = self.playback_runtime();
        let time_ms = if runtime.has_loaded_audio && runtime.info.duration.as_secs_f32() > 0.0 {
            runtime.info.position.as_secs_f64() * 1000.0
        } else {
            self.playback
                .saved_state
                .as_ref()
                .map(|s| s.position_secs * 1000.0)
                .unwrap_or(0.0)
        };

        let is_playing = matches!(
            self.playback_runtime().info.status,
            crate::audio::PlaybackStatus::Playing
        );

        if let Some(engine_cell) = &self.ui.lyrics.engine {
            let mut engine = engine_cell.borrow_mut();

            engine.update(delta_secs);

            engine.set_viewport_info(
                &engine_lines,
                content_width,
                font_size,
                viewport_height,
                self.ui.lyrics.viewport_width,
            );

            if is_playing {
                engine.resume();
            } else {
                engine.pause();
            }

            engine.set_current_time(time_ms, &engine_lines, just_initialized);
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
                let line_data = crate::features::lyrics::engine::LyricLineData {
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
                                emphasize: false,
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
                };
                line_data
            })
            .collect();

        let arc = std::sync::Arc::new(engine_lines);
        self.ui.lyrics.cached_engine_lines = Some(arc.clone());
        arc
    }

    /// Handle user scroll event on lyrics
    pub fn handle_lyrics_scroll(&mut self, delta: f32) {
        tracing::debug!("Lyrics scroll: delta={}", delta);

        if let Some(engine_cell) = &self.ui.lyrics.engine {
            engine_cell.borrow_mut().handle_wheel(delta);
        }
    }

    // ============ ASYNC LOADING METHODS ============

    /// 异步加载歌词（本地、缓存或在线）
    /// 歌词加载的主入口
    pub fn load_lyrics_async(&mut self, song: &crate::database::DbSong) -> Task<Message> {
        tracing::info!(
            "load_lyrics_async called for song: {} (id={})",
            song.title,
            song.id
        );
        self.prepare_display_lyrics_load(song.id);

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
            |result| match result {
                Some((song_id, lines, needs_online)) => {
                    if needs_online {
                        let ncm_id = (-song_id) as u64;
                        Message::FetchLyricsOnline(song_id, ncm_id)
                    } else if !lines.is_empty() {
                        Message::LocalLyricsReady(song_id, lines)
                    } else {
                        Message::LyricsLoadFailed(song_id, "No lyrics found".to_string())
                    }
                }
                None => Message::Noop,
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

    /// 只更新歌词页面背景（封面下载完成后调用）
    /// 不重新加载歌词
    pub fn update_lyrics_background(&mut self, song: &crate::database::DbSong) -> Task<Message> {
        self.update_background_async(song)
    }
}
