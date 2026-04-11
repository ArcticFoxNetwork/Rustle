//! Audio player - playback control and audio output
//!
//! Provides:
//! - Play/pause/seek/volume control
//! - Streaming buffer playback
//! - Audio processing chain integration
//! - Pre-decoded Sink creation for seamless track switching

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

use super::chain::{AudioProcessingChain, PlaybackProcessingRuntime};
use super::streaming::StreamingBuffer;

/// Cached audio devices to avoid repeated enumeration (which triggers Jack/ALSA warnings)
static AUDIO_DEVICES_CACHE: OnceLock<Vec<AudioDevice>> = OnceLock::new();

/// Playback status
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackStatus {
    Stopped,
    Playing,
    Paused,
    /// Buffering - waiting for more data before playback can continue.
    /// Used when playback catches up with download, or when seeking to unbuffered position.
    Buffering {
        position: Duration,
    },
}

/// Current playback info
#[derive(Debug, Clone)]
pub struct PlaybackInfo {
    pub status: PlaybackStatus,
    pub position: Duration,
    pub duration: Duration,
    pub volume: f32,
}

impl Default for PlaybackInfo {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            position: Duration::ZERO,
            duration: Duration::ZERO,
            volume: 1.0,
        }
    }
}

/// Audio player state (shared between threads)
struct PlayerState {
    status: PlaybackStatus,
    duration: Duration,
    volume: f32,
    paused_position: Option<Duration>,
    current_track_gain: f32,
    device_name: Option<String>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            duration: Duration::ZERO,
            volume: 1.0,
            paused_position: None,
            current_track_gain: 1.0,
            device_name: None,
        }
    }
}

/// Audio player - simplified, focused on playback control
/// Preloading is managed externally by PreloadManager
pub struct AudioPlayer {
    _stream: OutputStream,
    current_sink: Option<Sink>,
    current_path: Option<PathBuf>,
    state: Arc<Mutex<PlayerState>>,
    chain: AudioProcessingChain,
    current_runtime: Option<PlaybackProcessingRuntime>,
    is_streaming: bool,
}

impl AudioPlayer {
    const FADE_DURATION: Duration = Duration::from_millis(300);
    const FADE_STEPS: u32 = 12;

    /// Create a new audio player with default output device
    pub fn new(chain: AudioProcessingChain) -> Result<Self, String> {
        Self::with_device(None, chain)
    }

    /// Create a new audio player with specified output device
    pub fn with_device(
        device_name: Option<&str>,
        chain: AudioProcessingChain,
    ) -> Result<Self, String> {
        let stream = if let Some(name) = device_name {
            Self::create_stream_for_device(name)?
        } else {
            OutputStreamBuilder::open_default_stream()
                .map_err(|e| format!("Failed to create audio output: {}", e))?
        };

        let mut state = PlayerState::default();
        state.device_name = device_name.map(|s| s.to_string());

        Ok(Self {
            _stream: stream,
            current_sink: None,
            current_path: None,
            state: Arc::new(Mutex::new(state)),
            chain,
            current_runtime: None,
            is_streaming: false,
        })
    }

    /// Create output stream for a specific device by name
    fn create_stream_for_device(device_name: &str) -> Result<OutputStream, String> {
        let host = rodio::cpal::default_host();

        let device = host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate devices: {}", e))?
            .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
            .ok_or_else(|| format!("Device not found: {}", device_name))?;

        let config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get device config: {}", e))?;

        OutputStreamBuilder::from_device(device)
            .map_err(|e| format!("Failed to create stream builder: {}", e))?
            .with_sample_rate(config.sample_rate().0)
            .open_stream()
            .map_err(|e| format!("Failed to open stream: {}", e))
    }

    /// Switch to a different audio output device
    pub fn switch_device(
        &mut self,
        device_name: Option<&str>,
    ) -> Result<Option<(PathBuf, Duration, bool)>, String> {
        let playback_state = self.current_path.clone().map(|path| {
            let info = self.get_info();
            let was_playing = info.status == PlaybackStatus::Playing;
            let position = info.position;
            (path, position, was_playing)
        });

        self.stop();

        let stream = if let Some(name) = device_name {
            Self::create_stream_for_device(name)?
        } else {
            OutputStreamBuilder::open_default_stream()
                .map_err(|e| format!("Failed to create audio output: {}", e))?
        };

        {
            let mut state = self.state.lock().unwrap();
            state.device_name = device_name.map(|s| s.to_string());
        }

        self._stream = stream;

        tracing::info!("Switched audio device to: {:?}", device_name);
        Ok(playback_state)
    }

    /// Get current sink volume controlled by the user setting.
    fn get_sink_volume(&self) -> f32 {
        self.state.lock().unwrap().volume
    }

    fn decode_local_file(path: &Path) -> Result<Decoder<BufReader<File>>, String> {
        let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let reader = BufReader::new(file);

        Decoder::builder()
            .with_data(reader)
            .with_byte_len(file_len)
            .with_seekable(true)
            .build()
            .map_err(|e| format!("Failed to decode audio: {}", e))
    }

    fn prepare_runtime(&self, fade_in: bool) -> PlaybackProcessingRuntime {
        let runtime = self.chain.create_runtime();
        runtime.set_fade_volume(if fade_in { 0.0 } else { 1.0 });
        runtime
    }

    fn fade_sink_volume_blocking(sink: &Sink, from: f32, to: f32, duration: Duration) {
        if duration.is_zero() || (from - to).abs() < 0.001 {
            sink.set_volume(to);
            return;
        }

        let step_duration =
            Duration::from_secs_f32(duration.as_secs_f32() / Self::FADE_STEPS as f32);

        for step in 1..=Self::FADE_STEPS {
            let progress = step as f32 / Self::FADE_STEPS as f32;
            let eased = 1.0 - (1.0 - progress).powi(3);
            sink.set_volume(from + (to - from) * eased);

            if step < Self::FADE_STEPS {
                thread::sleep(step_duration);
            }
        }
    }

    fn try_seek_on_start(sink: &Sink, position: Duration, path: &Path) -> Option<String> {
        if position.is_zero() {
            return None;
        }

        if let Err(err) = sink.try_seek(position) {
            tracing::warn!(
                "Failed to seek to {:?} while starting {:?}: {}",
                position,
                path,
                err
            );
            return Some("Seek not supported for this format".to_string());
        }

        None
    }

    /// Play a file with fade in option
    pub fn play_with_fade(
        &mut self,
        path: PathBuf,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<(), String> {
        self.stop();
        self.chain.refresh_eq_coefficients();
        let runtime = self.prepare_runtime(fade_in);
        self.chain.activate_runtime(Some(&runtime));

        let source = Self::decode_local_file(&path)?;
        let duration = source.total_duration().unwrap_or(Duration::ZERO);

        let processed = self.chain.apply(source, track_gain, runtime.clone());

        let sink = Sink::connect_new(self._stream.mixer());
        sink.append(processed);

        let volume = self.get_sink_volume();
        sink.set_volume(volume);

        if fade_in {
            runtime.fade_to(1.0, Self::FADE_DURATION);
        }

        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Playing;
            state.duration = duration;
            state.paused_position = None;
            state.current_track_gain = track_gain;
        }

        self.current_sink = Some(sink);
        self.current_path = Some(path.clone());
        self.current_runtime = Some(runtime);
        self.is_streaming = false;

        tracing::info!("Playing audio, duration: {:?}", duration);
        Ok(())
    }

    /// Create a preload sink for external use (by PreloadManager)
    /// Returns (Sink, Duration) - sink is paused and ready for playback
    pub fn create_preload_sink(
        &self,
        path: &Path,
        track_gain: f32,
    ) -> Result<(Sink, Duration, PlaybackProcessingRuntime), String> {
        let source = Self::decode_local_file(path)?;
        let duration = source.total_duration().unwrap_or(Duration::ZERO);

        let runtime = self.prepare_runtime(false);
        let processed = self.chain.apply(source, track_gain, runtime.clone());

        let sink = Sink::connect_new(self._stream.mixer());
        sink.append(processed);
        sink.set_volume(self.get_sink_volume());
        sink.pause(); // Start paused

        Ok((sink, duration, runtime))
    }

    /// Load a local file into paused state at a target position.
    pub fn load_paused(
        &mut self,
        path: PathBuf,
        position: Duration,
        track_gain: f32,
    ) -> Result<(), String> {
        self.stop();
        self.chain.refresh_eq_coefficients();
        let runtime = self.prepare_runtime(false);
        self.chain.activate_runtime(Some(&runtime));

        let source = Self::decode_local_file(&path)?;
        let duration = source.total_duration().unwrap_or(Duration::ZERO);
        let processed = self.chain.apply(source, track_gain, runtime.clone());

        let sink = Sink::connect_new(self._stream.mixer());
        sink.append(processed);
        sink.set_volume(self.get_sink_volume());
        sink.pause();

        let _ = Self::try_seek_on_start(&sink, position, &path);

        let paused_position = sink.get_pos();

        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Paused;
            state.duration = duration;
            state.paused_position = Some(paused_position);
            state.current_track_gain = track_gain;
        }

        self.current_sink = Some(sink);
        self.current_path = Some(path.clone());
        self.current_runtime = Some(runtime);
        self.is_streaming = false;

        tracing::info!(
            "Loaded paused audio at {:?}, duration: {:?}",
            paused_position,
            duration
        );
        Ok(())
    }

    /// Load a streaming source into paused state at a target position.
    pub fn load_streaming_paused(
        &mut self,
        buffer: StreamingBuffer,
        duration: Duration,
        cache_path: Option<PathBuf>,
        position: Duration,
        track_gain: f32,
    ) -> Result<Option<String>, String> {
        self.stop();
        self.chain.refresh_eq_coefficients();
        let runtime = self.prepare_runtime(false);
        self.chain.activate_runtime(Some(&runtime));

        let start = std::time::Instant::now();
        let byte_len = loop {
            let size = buffer.shared().total_size();
            if size > 0 {
                break size;
            }
            if buffer.shared().is_complete() {
                let downloaded = buffer.shared().downloaded();
                tracing::info!(
                    "LoadPausedStreaming: download complete before total_size set, using downloaded: {}",
                    downloaded
                );
                break downloaded;
            }
            if start.elapsed() > Duration::from_secs(5) {
                tracing::warn!(
                    "LoadPausedStreaming: timeout waiting for content-length, seek may not work properly"
                );
                break 0;
            }
            std::thread::sleep(Duration::from_millis(50));
        };

        tracing::info!(
            "load_streaming_paused: byte_len={} (waited {:?}), downloaded={}, complete={}, cache_path={:?}",
            byte_len,
            start.elapsed(),
            buffer.shared().downloaded(),
            buffer.shared().is_complete(),
            cache_path
        );

        let source = Decoder::builder()
            .with_data(buffer)
            .with_byte_len(byte_len)
            .with_seekable(byte_len > 0)
            .build()
            .map_err(|e| format!("Failed to decode streaming audio: {}", e))?;

        let processed = self.chain.apply(source, track_gain, runtime.clone());

        let sink = Sink::connect_new(self._stream.mixer());
        sink.append(processed);
        sink.set_volume(self.get_sink_volume());
        sink.pause();

        let seek_error = cache_path
            .as_deref()
            .map(|path| Self::try_seek_on_start(&sink, position, path))
            .unwrap_or_else(|| {
                if position.is_zero() {
                    None
                } else if let Err(err) = sink.try_seek(position) {
                    tracing::warn!(
                        "Failed to seek streaming source to {:?} while loading paused: {}",
                        position,
                        err
                    );
                    Some("Seek not supported for this format".to_string())
                } else {
                    None
                }
            });

        let paused_position = sink.get_pos();

        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Paused;
            state.duration = duration;
            state.paused_position = Some(paused_position);
            state.current_track_gain = track_gain;
        }

        self.current_sink = Some(sink);
        self.current_path = cache_path;
        self.current_runtime = Some(runtime);
        self.is_streaming = true;

        tracing::info!(
            "Loaded paused streaming audio at {:?}, duration: {:?}",
            paused_position,
            duration
        );
        Ok(seek_error)
    }

    /// Play a local file from a target position.
    ///
    /// Returns `Ok(Some(error))` when playback started but the initial seek failed.
    pub fn play_from_position_with_fade(
        &mut self,
        path: PathBuf,
        position: Duration,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<Option<String>, String> {
        self.stop();
        self.chain.refresh_eq_coefficients();
        let runtime = self.prepare_runtime(fade_in);
        self.chain.activate_runtime(Some(&runtime));

        let source = Self::decode_local_file(&path)?;
        let duration = source.total_duration().unwrap_or(Duration::ZERO);
        let processed = self.chain.apply(source, track_gain, runtime.clone());

        let sink = Sink::connect_new(self._stream.mixer());
        sink.append(processed);

        let volume = self.get_sink_volume();
        sink.set_volume(volume);
        let seek_error = Self::try_seek_on_start(&sink, position, &path);

        if fade_in {
            runtime.fade_to(1.0, Self::FADE_DURATION);
        }

        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Playing;
            state.duration = duration;
            state.paused_position = None;
            state.current_track_gain = track_gain;
        }

        self.current_sink = Some(sink);
        self.current_path = Some(path.clone());
        self.current_runtime = Some(runtime);
        self.is_streaming = false;

        tracing::info!(
            "Playing audio from position {:?}, duration: {:?}",
            position,
            duration
        );
        Ok(seek_error)
    }

    /// Create a preload sink from StreamingBuffer for NCM songs
    ///
    /// This allows streaming playback where the buffer continues downloading
    /// in the background while playback proceeds. The StreamingBuffer's read()
    /// method blocks when data is not yet available.
    ///
    /// Returns (Sink, Duration) - sink is paused and ready for playback
    pub fn create_preload_sink_streaming(
        &self,
        buffer: StreamingBuffer,
        duration: Duration,
        track_gain: f32,
    ) -> Result<(Sink, Duration, PlaybackProcessingRuntime), String> {
        // Wait for total_size to be set (from Content-Length header)
        // This is critical for FLAC and other formats that need byte_len for seeking
        let start = std::time::Instant::now();
        let byte_len = loop {
            let size = buffer.shared().total_size();
            if size > 0 {
                break size;
            }
            // If download is already complete, use downloaded size
            if buffer.shared().is_complete() {
                let downloaded = buffer.shared().downloaded();
                tracing::info!(
                    "Preload: download complete before total_size set, using downloaded: {}",
                    downloaded
                );
                break downloaded;
            }
            // Timeout after 5 seconds
            if start.elapsed() > Duration::from_secs(5) {
                tracing::warn!(
                    "Preload: timeout waiting for content-length, seek may not work properly"
                );
                break 0;
            }
            // Wait a bit before checking again
            std::thread::sleep(Duration::from_millis(50));
        };

        tracing::debug!(
            "create_preload_sink_streaming: byte_len={} (waited {:?})",
            byte_len,
            start.elapsed()
        );

        // Use Decoder::builder() to properly set byte_len and seekable
        // This is required for FLAC and other formats that need byte_len for seeking
        let source = Decoder::builder()
            .with_data(buffer)
            .with_byte_len(byte_len)
            .with_seekable(byte_len > 0)
            .build()
            .map_err(|e| format!("Failed to decode streaming audio: {}", e))?;

        // Use provided duration since streaming buffer may not know total duration
        let actual_duration = source.total_duration().unwrap_or(duration);

        let runtime = self.prepare_runtime(false);
        let processed = self.chain.apply(source, track_gain, runtime.clone());

        let sink = Sink::connect_new(self._stream.mixer());
        sink.append(processed);
        sink.set_volume(self.get_sink_volume());
        sink.pause(); // Start paused

        Ok((sink, actual_duration, runtime))
    }

    /// Play a preloaded sink (from PreloadManager)
    ///
    pub fn play_preloaded_sink(
        &mut self,
        sink: Sink,
        duration: Duration,
        path: PathBuf,
        is_streaming: bool,
        fade_in: bool,
        track_gain: f32,
        runtime: PlaybackProcessingRuntime,
    ) -> Result<(), String> {
        self.stop();
        self.chain.activate_runtime(Some(&runtime));

        sink.set_volume(self.get_sink_volume());
        if fade_in {
            runtime.set_fade_volume(0.0);
            sink.play();
            runtime.fade_to(1.0, Self::FADE_DURATION);
        } else {
            runtime.set_fade_volume(1.0);
            sink.play();
        }

        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Playing;
            state.duration = duration;
            state.paused_position = None;
            state.current_track_gain = track_gain;
        }

        self.current_sink = Some(sink);
        self.current_path = Some(path.clone());
        self.current_runtime = Some(runtime);
        self.is_streaming = is_streaming;

        tracing::info!(
            "Playing preloaded audio, duration: {:?}, streaming: {}",
            duration,
            is_streaming
        );
        Ok(())
    }

    /// Play from a streaming buffer (for network streaming without file I/O)
    ///
    /// The StreamingBuffer blocks on read() when data is not yet available,
    /// allowing seamless playback while downloading continues in background.
    pub fn play_streaming(
        &mut self,
        buffer: StreamingBuffer,
        duration: Duration,
        cache_path: Option<PathBuf>,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<(), String> {
        self.stop();
        self.chain.refresh_eq_coefficients();
        let runtime = self.prepare_runtime(fade_in);
        self.chain.activate_runtime(Some(&runtime));

        // Wait for total_size to be set (from Content-Length header)
        // This is critical for FLAC and other formats that need byte_len for seeking
        let start = std::time::Instant::now();
        let byte_len = loop {
            let size = buffer.shared().total_size();
            if size > 0 {
                break size;
            }
            // If download is already complete, use downloaded size
            if buffer.shared().is_complete() {
                let downloaded = buffer.shared().downloaded();
                tracing::info!(
                    "Download complete before total_size set, using downloaded: {}",
                    downloaded
                );
                break downloaded;
            }
            // Timeout after 5 seconds
            if start.elapsed() > Duration::from_secs(5) {
                tracing::warn!("Timeout waiting for content-length, seek may not work properly");
                break 0;
            }
            // Wait a bit before checking again
            std::thread::sleep(Duration::from_millis(50));
        };

        tracing::info!(
            "play_streaming: byte_len={} (waited {:?}), downloaded={}, complete={}, cache_path={:?}",
            byte_len,
            start.elapsed(),
            buffer.shared().downloaded(),
            buffer.shared().is_complete(),
            cache_path
        );

        // Use Decoder::builder() to properly set byte_len and seekable
        // This is required for FLAC and other formats that need byte_len for seeking
        let source = Decoder::builder()
            .with_data(buffer)
            .with_byte_len(byte_len)
            .with_seekable(byte_len > 0)
            .build()
            .map_err(|e| format!("Failed to decode streaming audio: {}", e))?;

        let processed = self.chain.apply(source, track_gain, runtime.clone());

        let sink = Sink::connect_new(self._stream.mixer());
        sink.append(processed);

        let volume = self.get_sink_volume();
        sink.set_volume(volume);

        if fade_in {
            runtime.fade_to(1.0, Self::FADE_DURATION);
        }

        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Playing;
            state.duration = duration;
            state.paused_position = None;
            state.current_track_gain = track_gain;
        }

        self.current_sink = Some(sink);
        // Store cache path for seek fallback (when streaming seek fails, we can reload from file)
        self.current_path = cache_path;
        self.current_runtime = Some(runtime);
        self.is_streaming = true;

        tracing::info!("Playing streaming audio, duration: {:?}", duration);
        Ok(())
    }

    /// Pause playback
    pub fn pause(&mut self) {
        self.pause_with_fade(false);
    }

    /// Pause playback with optional fade out
    pub fn pause_with_fade(&mut self, fade_out: bool) {
        if let Some(sink) = self.current_sink.as_ref() {
            if fade_out {
                let current_volume = sink.volume();
                Self::fade_sink_volume_blocking(sink, current_volume, 0.0, Self::FADE_DURATION);
            }

            let current_pos = sink.get_pos();
            sink.pause();
            sink.set_volume(self.get_sink_volume());

            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Paused;
            state.paused_position = Some(current_pos);
        }
    }

    /// Resume playback
    pub fn resume(&mut self) {
        self.resume_with_fade(false);
    }

    /// Resume playback with optional fade in
    pub fn resume_with_fade(&mut self, fade_in: bool) {
        if let Some(sink) = &self.current_sink {
            let target_volume = self.get_sink_volume();

            if fade_in {
                sink.set_volume(0.0);
                sink.play();
                Self::fade_sink_volume_blocking(sink, 0.0, target_volume, Self::FADE_DURATION);
            } else {
                sink.set_volume(target_volume);
                sink.play();
            }

            {
                let mut state = self.state.lock().unwrap();
                state.status = PlaybackStatus::Playing;
                state.paused_position = None;
            }
        }
    }

    pub fn pause_sink(&self) {
        if let Some(sink) = &self.current_sink {
            sink.pause();
        }
    }

    pub fn play_sink(&self) {
        if let Some(sink) = &self.current_sink {
            sink.play();
        }
    }

    /// Stop playback
    pub fn stop(&mut self) {
        if let Some(sink) = self.current_sink.take() {
            sink.stop();
        }
        self.current_runtime = None;
        self.chain.activate_runtime(None);
        let mut state = self.state.lock().unwrap();
        state.status = PlaybackStatus::Stopped;
        state.current_track_gain = 1.0;
    }

    /// Set volume (0.0 to 1.0)
    pub fn set_volume(&mut self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        {
            let mut state = self.state.lock().unwrap();
            state.volume = volume;
        }

        if let Some(sink) = &self.current_sink {
            sink.set_volume(self.get_sink_volume());
        }
    }

    /// Seek to position
    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        if let Some(sink) = &mut self.current_sink {
            match sink.try_seek(position) {
                Ok(_) => {
                    let new_position = sink.get_pos();
                    let mut state = self.state.lock().unwrap();
                    if matches!(state.status, PlaybackStatus::Paused) {
                        state.paused_position = Some(new_position);
                    }
                    tracing::debug!("Seek to {:?} successful", position);
                    return Ok(());
                }
                Err(e) => {
                    tracing::debug!("Direct seek failed: {:?}, will try reload", e);
                }
            }
        } else {
            return Err("No audio loaded".to_string());
        }

        // Direct seek failed, try reloading the file
        let path = match self.current_path.clone() {
            Some(p) => p,
            None => {
                return Err("Seek failed: end of stream (streaming playback)".to_string());
            }
        };

        tracing::info!("Attempting reload workaround for seek to {:?}", position);
        let (volume, was_playing, track_gain) = {
            let state = self.state.lock().unwrap();
            (
                state.volume,
                state.status == PlaybackStatus::Playing,
                state.current_track_gain,
            )
        };

        if let Some(old_sink) = self.current_sink.take() {
            old_sink.stop();
        }

        let source = Self::decode_local_file(&path)?;
        let duration = source.total_duration();

        let runtime = self
            .current_runtime
            .clone()
            .unwrap_or_else(|| self.prepare_runtime(false));
        self.chain.activate_runtime(Some(&runtime));
        let processed = self.chain.apply(source, track_gain, runtime.clone());

        let new_sink = Sink::connect_new(self._stream.mixer());
        new_sink.append(processed);
        new_sink.set_volume(volume);

        let seek_failed = if let Err(seek_err) = new_sink.try_seek(position) {
            tracing::warn!("Seek after reload also failed: {:?}", seek_err);
            true
        } else {
            false
        };

        if !was_playing {
            new_sink.pause();
        }

        let new_position = new_sink.get_pos();

        {
            let mut state = self.state.lock().unwrap();
            state.duration = duration.unwrap_or(Duration::ZERO);
            state.status = if was_playing {
                PlaybackStatus::Playing
            } else {
                PlaybackStatus::Paused
            };
            state.paused_position = if was_playing {
                None
            } else {
                Some(new_position)
            };
            state.current_track_gain = track_gain;
        }

        self.current_sink = Some(new_sink);
        self.current_runtime = Some(runtime);

        if seek_failed {
            Err("Seek not supported for this format".to_string())
        } else {
            Ok(())
        }
    }

    /// Get current playback info
    pub fn get_info(&self) -> PlaybackInfo {
        let state = self.state.lock().unwrap();

        let position = if let Some(sink) = &self.current_sink {
            if matches!(state.status, PlaybackStatus::Paused) {
                state.paused_position.unwrap_or_else(|| sink.get_pos())
            } else {
                sink.get_pos()
            }
        } else {
            Duration::ZERO
        };

        // Don't change status based on sink.empty() - it's unreliable
        // The is_finished() method handles proper finish detection
        let status = state.status.clone();

        PlaybackInfo {
            status,
            position,
            duration: state.duration,
            volume: state.volume,
        }
    }

    /// Check if playback finished
    pub fn is_finished(&self) -> bool {
        if let Some(sink) = &self.current_sink {
            let state = self.state.lock().unwrap();
            let position = sink.get_pos();
            let duration = state.duration;

            // Don't consider finished if we just started or if paused/stopped
            if state.status != PlaybackStatus::Playing {
                return false;
            }

            // Need valid duration to determine if finished
            if duration.as_secs_f32() <= 0.0 {
                return false;
            }

            // Don't consider finished if position is very early
            if position.as_secs_f32() < 5.0 {
                return false;
            }

            // Check if we've reached near the end of the track
            // Use a small tolerance (0.5s) to account for timing variations
            if position.as_secs_f32() >= duration.as_secs_f32() - 0.5 {
                tracing::debug!(
                    "is_finished: reached end at {:.1}s / {:.1}s",
                    position.as_secs_f32(),
                    duration.as_secs_f32()
                );
                return true;
            }

            // Also check if sink is empty AND we're very close to the end (95%)
            // Also check if sink is empty and we're past 95% of duration
            if sink.empty() && position.as_secs_f32() > duration.as_secs_f32() * 0.95 {
                tracing::debug!(
                    "is_finished: sink empty near end at {:.1}s / {:.1}s",
                    position.as_secs_f32(),
                    duration.as_secs_f32()
                );
                return true;
            }

            false
        } else {
            false
        }
    }

    /// Check if current playback is streaming
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }
}

// ============ Audio Device Discovery ============

/// Audio device info with internal name and display name
#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
}

/// Get list of available audio output devices
pub fn get_audio_devices() -> Vec<AudioDevice> {
    AUDIO_DEVICES_CACHE
        .get_or_init(|| {
            let devices = get_cpal_devices();
            if !devices.is_empty() {
                return devices;
            }

            let pa_devices = get_pulseaudio_devices();
            if !pa_devices.is_empty() {
                return pa_devices;
            }

            get_alsa_devices()
        })
        .clone()
}

fn get_pulseaudio_devices() -> Vec<AudioDevice> {
    let mut devices = Vec::new();

    if let Ok(output) = std::process::Command::new("pactl")
        .args(["list", "sinks"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_name = String::new();

            for line in stdout.lines() {
                let line = line.trim();
                if line.starts_with("Name:") {
                    current_name = line.trim_start_matches("Name:").trim().to_string();
                } else if line.starts_with("Description:") && !current_name.is_empty() {
                    let description = line.trim_start_matches("Description:").trim().to_string();
                    devices.push(AudioDevice {
                        name: current_name.clone(),
                        description,
                    });
                    current_name.clear();
                }
            }
        }
    }

    devices
}

fn get_alsa_devices() -> Vec<AudioDevice> {
    let mut devices = Vec::new();

    if let Ok(output) = std::process::Command::new("aplay").args(["-l"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);

            for line in stdout.lines() {
                if line.starts_with("card ") {
                    if let Some((card_info, device_info)) = line.split_once(", device ") {
                        let card_num = card_info
                            .trim_start_matches("card ")
                            .split(':')
                            .next()
                            .unwrap_or("0")
                            .trim();

                        let device_num = device_info.split(':').next().unwrap_or("0").trim();

                        let description = if let Some(start) = line.find('[') {
                            if let Some(end) = line.rfind(']') {
                                line[start + 1..end].to_string()
                            } else {
                                line.to_string()
                            }
                        } else {
                            line.to_string()
                        };

                        let name = format!("hw:{},{}", card_num, device_num);

                        devices.push(AudioDevice { name, description });
                    }
                }
            }
        }
    }

    if devices.is_empty() {
        devices = get_cpal_devices();
    }

    devices
}

fn get_cpal_devices() -> Vec<AudioDevice> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};

    let host = rodio::cpal::default_host();
    let mut devices = Vec::new();

    if let Ok(output_devices) = host.output_devices() {
        for device in output_devices {
            if let Ok(name) = device.name() {
                let name_lower = name.to_lowercase();

                if name_lower.contains("jack")
                    || name_lower.contains("oss")
                    || name_lower.contains("/dev/dsp")
                    || name == "default"
                    || name == "pipewire"
                    || name == "pulse"
                {
                    continue;
                }

                if device.default_output_config().is_ok() {
                    devices.push(AudioDevice {
                        name: name.clone(),
                        description: name,
                    });
                }
            }
        }
    }

    devices
}
