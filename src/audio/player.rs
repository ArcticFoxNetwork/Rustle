//! Audio player using rodio with preloading support
//!
//! This player supports preloading up to 3 tracks (previous, current, next)
//! for seamless track switching without loading delays.
//!
//! The player is decoupled from audio processing - it receives an
//! `AudioProcessingChain` reference and applies it to all audio sources.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source, mixer::Mixer};

use super::chain::AudioProcessingChain;

/// Cached audio devices to avoid repeated enumeration (which triggers Jack/ALSA warnings)
static AUDIO_DEVICES_CACHE: OnceLock<Vec<AudioDevice>> = OnceLock::new();

/// Playback status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackStatus {
    Stopped,
    Playing,
    Paused,
    Pausing, // Currently fading out before pause
}

/// Fade state for smooth transitions
#[derive(Debug, Clone)]
enum FadeState {
    None,
    FadingIn {
        start_time: Instant,
        duration: Duration,
        start_volume: f32,
        target_volume: f32,
    },
    FadingOut {
        start_time: Instant,
        duration: Duration,
        start_volume: f32,
        target_volume: f32,
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
    original_volume: f32,
    track_gain: f32,
    fade_state: FadeState,
    // Current device name (None = default)
    device_name: Option<String>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            duration: Duration::ZERO,
            volume: 1.0,
            paused_position: None,
            original_volume: 1.0,
            track_gain: 1.0,
            fade_state: FadeState::None,
            device_name: None,
        }
    }
}

/// A preloaded track ready for instant playback
struct PreloadedTrack {
    sink: Sink,
    path: PathBuf,
    duration: Duration,
}

impl PreloadedTrack {
    /// Create a new preloaded track (sink starts paused)
    fn new(
        mixer: &Mixer,
        path: PathBuf,
        volume: f32,
        chain: &AudioProcessingChain,
    ) -> Result<Self, String> {
        let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader).map_err(|e| format!("Failed to decode audio: {}", e))?;
        let duration = source.total_duration().unwrap_or(Duration::ZERO);

        // Apply the processing chain (preamp -> EQ -> analyzer)
        let processed = chain.apply(source);

        let sink = Sink::connect_new(mixer);
        sink.append(processed);
        sink.set_volume(volume);
        sink.pause(); // Start paused, ready for instant playback

        Ok(Self {
            sink,
            path,
            duration,
        })
    }

    /// Start playing this track
    fn play(&self) {
        self.sink.play();
    }

    /// Stop and consume this track
    fn stop(self) {
        self.sink.stop();
    }
}

/// Audio player with preloading support for seamless track switching
pub struct AudioPlayer {
    _stream: OutputStream,
    mixer: Arc<Mixer>,
    // Current playing track
    current_sink: Option<Sink>,
    current_path: Option<PathBuf>,
    // Preloaded tracks for instant switching
    preloaded_prev: Option<PreloadedTrack>,
    preloaded_next: Option<PreloadedTrack>,
    // Shared state
    state: Arc<Mutex<PlayerState>>,
    // Audio processing chain (preamp, EQ, analyzer)
    chain: AudioProcessingChain,
}

impl AudioPlayer {
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
        let mixer = stream.mixer().clone();

        let mut state = PlayerState::default();
        state.device_name = device_name.map(|s| s.to_string());

        Ok(Self {
            _stream: stream,
            mixer: Arc::new(mixer),
            current_sink: None,
            current_path: None,
            preloaded_prev: None,
            preloaded_next: None,
            state: Arc::new(Mutex::new(state)),
            chain,
        })
    }

    /// Create output stream for a specific device by name
    fn create_stream_for_device(device_name: &str) -> Result<OutputStream, String> {
        let host = rodio::cpal::default_host();

        // Find the device by name
        let device = host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate devices: {}", e))?
            .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
            .ok_or_else(|| format!("Device not found: {}", device_name))?;

        // Get default config for the device
        let config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get device config: {}", e))?;

        // Build stream with the device
        OutputStreamBuilder::from_device(device)
            .map_err(|e| format!("Failed to create stream builder: {}", e))?
            .with_sample_rate(config.sample_rate().0)
            .open_stream()
            .map_err(|e| format!("Failed to open stream: {}", e))
    }

    /// Switch to a different audio output device
    /// Returns the path of the track that was playing (if any) so it can be resumed
    pub fn switch_device(
        &mut self,
        device_name: Option<&str>,
    ) -> Result<Option<(PathBuf, Duration, bool)>, String> {
        // Save current playback state
        let playback_state = self.current_path.clone().map(|path| {
            let info = self.get_info();
            let was_playing = info.status == PlaybackStatus::Playing;
            let position = info.position;
            (path, position, was_playing)
        });

        // Stop everything
        self.stop();
        self.clear_preloads();

        // Create new stream with the specified device
        let stream = if let Some(name) = device_name {
            Self::create_stream_for_device(name)?
        } else {
            OutputStreamBuilder::open_default_stream()
                .map_err(|e| format!("Failed to create audio output: {}", e))?
        };

        let mixer = stream.mixer().clone();

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            state.device_name = device_name.map(|s| s.to_string());
        }

        self._stream = stream;
        self.mixer = Arc::new(mixer);

        tracing::info!("Switched audio device to: {:?}", device_name);
        Ok(playback_state)
    }

    /// Get current volume with track gain applied
    fn get_effective_volume(&self) -> f32 {
        let state = self.state.lock().unwrap();
        state.volume * state.track_gain
    }

    /// Play a file (clears preloaded tracks)
    pub fn play(&mut self, path: PathBuf) -> Result<(), String> {
        self.play_with_fade(path, false)
    }

    /// Play a file with fade in option
    pub fn play_with_fade(&mut self, path: PathBuf, fade_in: bool) -> Result<(), String> {
        // Stop current playback and clear preloads
        self.stop();
        self.clear_preloads();

        // Open file and decode
        let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader).map_err(|e| format!("Failed to decode audio: {}", e))?;
        let duration = source.total_duration().unwrap_or(Duration::ZERO);

        // Apply the processing chain (preamp -> EQ -> analyzer)
        let processed = self.chain.apply(source);

        // Create sink and start playing
        let sink = Sink::connect_new(&self.mixer);
        sink.append(processed);

        let volume = self.get_effective_volume();
        sink.set_volume(if fade_in { 0.0 } else { volume });

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Playing;
            state.duration = duration;
            state.paused_position = None;
            state.original_volume = state.volume;
        }

        self.current_sink = Some(sink);
        self.current_path = Some(path);

        if fade_in {
            self.start_fade_in(300);
        }

        tracing::info!("Playing audio, duration: {:?}", duration);
        Ok(())
    }

    /// Preload the previous track for instant backward switching
    pub fn preload_prev(&mut self, path: PathBuf) {
        // Don't preload if it's the same as current
        if self.current_path.as_ref() == Some(&path) {
            return;
        }
        // Don't preload if already preloaded
        if self.preloaded_prev.as_ref().map(|t| &t.path) == Some(&path) {
            return;
        }

        let volume = self.get_effective_volume();
        match PreloadedTrack::new(&self.mixer, path.clone(), volume, &self.chain) {
            Ok(track) => {
                // Stop old preloaded track if exists
                if let Some(old) = self.preloaded_prev.take() {
                    old.stop();
                }
                tracing::debug!("Preloaded prev track: {:?}", path);
                self.preloaded_prev = Some(track);
            }
            Err(e) => {
                tracing::warn!("Failed to preload prev track: {}", e);
            }
        }
    }

    /// Preload the next track for instant forward switching
    pub fn preload_next(&mut self, path: PathBuf) {
        // Don't preload if it's the same as current
        if self.current_path.as_ref() == Some(&path) {
            return;
        }
        // Don't preload if already preloaded
        if self.preloaded_next.as_ref().map(|t| &t.path) == Some(&path) {
            return;
        }

        let volume = self.get_effective_volume();
        match PreloadedTrack::new(&self.mixer, path.clone(), volume, &self.chain) {
            Ok(track) => {
                // Stop old preloaded track if exists
                if let Some(old) = self.preloaded_next.take() {
                    old.stop();
                }
                tracing::debug!("Preloaded next track: {:?}", path);
                self.preloaded_next = Some(track);
            }
            Err(e) => {
                tracing::warn!("Failed to preload next track: {}", e);
            }
        }
    }

    /// Switch to the preloaded next track instantly
    /// Returns the path of the new current track, or None if no preload available
    pub fn switch_to_next(&mut self) -> Option<PathBuf> {
        let preloaded = self.preloaded_next.take()?;

        // Stop current playback
        if let Some(sink) = self.current_sink.take() {
            sink.stop();
        }

        // Move current to prev preload slot (if we want to keep it)
        // For now, just clear prev since we're moving forward
        self.clear_prev_preload();

        // Reset analysis data before starting new track
        self.chain.reset_analysis();

        // Force EQ coefficients refresh to ensure audio processing is active
        self.chain.refresh_eq_coefficients();

        // Start playing preloaded track
        preloaded.play();
        let path = preloaded.path.clone();
        let duration = preloaded.duration;

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Playing;
            state.duration = duration;
            state.paused_position = None;
        }

        self.current_sink = Some(preloaded.sink);
        self.current_path = Some(path.clone());

        tracing::info!("Switched to preloaded next track: {:?}", path);
        Some(path)
    }

    /// Switch to the preloaded previous track instantly
    /// Returns the path of the new current track, or None if no preload available
    pub fn switch_to_prev(&mut self) -> Option<PathBuf> {
        let preloaded = self.preloaded_prev.take()?;

        // Stop current playback
        if let Some(sink) = self.current_sink.take() {
            sink.stop();
        }

        // Clear next preload since we're moving backward
        self.clear_next_preload();

        // Reset analysis data before starting new track
        self.chain.reset_analysis();

        // Force EQ coefficients refresh to ensure audio processing is active
        self.chain.refresh_eq_coefficients();

        // Start playing preloaded track
        preloaded.play();
        let path = preloaded.path.clone();
        let duration = preloaded.duration;

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            state.status = PlaybackStatus::Playing;
            state.duration = duration;
            state.paused_position = None;
        }

        self.current_sink = Some(preloaded.sink);
        self.current_path = Some(path.clone());

        tracing::info!("Switched to preloaded prev track: {:?}", path);
        Some(path)
    }

    /// Get the path of preloaded next track
    pub fn preloaded_next_path(&self) -> Option<&PathBuf> {
        self.preloaded_next.as_ref().map(|t| &t.path)
    }

    /// Get the path of preloaded prev track
    pub fn preloaded_prev_path(&self) -> Option<&PathBuf> {
        self.preloaded_prev.as_ref().map(|t| &t.path)
    }

    /// Clear all preloaded tracks
    fn clear_preloads(&mut self) {
        self.clear_prev_preload();
        self.clear_next_preload();
    }

    /// Clear preloaded previous track
    fn clear_prev_preload(&mut self) {
        if let Some(track) = self.preloaded_prev.take() {
            track.stop();
        }
    }

    /// Clear preloaded next track
    fn clear_next_preload(&mut self) {
        if let Some(track) = self.preloaded_next.take() {
            track.stop();
        }
    }

    /// Update volume on preloaded tracks when volume changes
    fn update_preload_volumes(&self) {
        let volume = self.get_effective_volume();
        if let Some(ref track) = self.preloaded_prev {
            track.sink.set_volume(volume);
        }
        if let Some(ref track) = self.preloaded_next {
            track.sink.set_volume(volume);
        }
    }

    /// Pause playback
    pub fn pause(&mut self) {
        self.pause_with_fade(false);
    }

    /// Pause playback with optional fade out
    pub fn pause_with_fade(&mut self, fade_out: bool) {
        if let Some(sink) = self.current_sink.as_ref() {
            if fade_out {
                self.stop_fade();
                self.start_fade_out(300);
                let mut state = self.state.lock().unwrap();
                state.status = PlaybackStatus::Pausing;
            } else {
                let current_pos = sink.get_pos();
                sink.pause();
                let mut state = self.state.lock().unwrap();
                state.status = PlaybackStatus::Paused;
                state.paused_position = Some(current_pos);
            }
        }
    }

    /// Resume playback
    pub fn resume(&mut self) {
        self.resume_with_fade(false);
    }

    /// Resume playback with optional fade in
    pub fn resume_with_fade(&mut self, fade_in: bool) {
        if let Some(sink) = &self.current_sink {
            self.stop_fade();

            let target_volume = self.get_effective_volume();

            if fade_in {
                sink.set_volume(0.0);
            } else {
                sink.set_volume(target_volume);
            }

            sink.play();
            {
                let mut state = self.state.lock().unwrap();
                state.status = PlaybackStatus::Playing;
                state.paused_position = None;
            }

            if fade_in {
                self.start_fade_in(300);
            }
        }
    }

    /// Stop playback (keeps preloaded tracks)
    pub fn stop(&mut self) {
        if let Some(sink) = self.current_sink.take() {
            sink.stop();
        }
        // Reset analysis data when stopping
        self.chain.reset_analysis();
        let mut state = self.state.lock().unwrap();
        state.status = PlaybackStatus::Stopped;
    }

    /// Set volume (0.0 to 1.0)
    pub fn set_volume(&mut self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        {
            let mut state = self.state.lock().unwrap();
            state.original_volume = volume;
            state.volume = volume;
        }

        // Apply volume to current sink
        if let Some(sink) = &self.current_sink {
            let effective_volume = self.get_effective_volume();
            sink.set_volume(effective_volume);
        }

        // Also update preloaded tracks
        self.update_preload_volumes();
    }

    /// Seek to position
    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        // First try direct seek
        if let Some(sink) = &mut self.current_sink {
            match sink.try_seek(position) {
                Ok(_) => {
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
        tracing::info!("Attempting reload workaround for seek");

        let path = self.current_path.clone().ok_or("No current path")?;
        let volume = self.get_effective_volume();
        let was_playing = {
            let state = self.state.lock().unwrap();
            state.status == PlaybackStatus::Playing
        };

        // Stop and remove old sink
        if let Some(old_sink) = self.current_sink.take() {
            old_sink.stop();
        }

        // Open and decode file
        let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader).map_err(|e| format!("Failed to decode: {}", e))?;
        let duration = source.total_duration();

        // Apply the processing chain
        let processed = self.chain.apply(source);

        // Create new sink
        let new_sink = Sink::connect_new(&self.mixer);
        new_sink.append(processed);
        new_sink.set_volume(volume);

        // Try to seek in the new sink
        let seek_failed = if let Err(seek_err) = new_sink.try_seek(position) {
            tracing::warn!("Seek after reload also failed: {:?}", seek_err);
            // For AAC and some other formats, seeking may not be supported
            // In this case, we need to handle it gracefully
            true
        } else {
            false
        };

        // Ensure playback state is correct
        if !was_playing {
            new_sink.pause();
        }

        {
            let mut state = self.state.lock().unwrap();
            state.duration = duration.unwrap_or(Duration::ZERO);
            // Ensure status is correct after reload
            state.status = if was_playing {
                PlaybackStatus::Playing
            } else {
                PlaybackStatus::Paused
            };
            state.paused_position = None;
        }

        self.current_sink = Some(new_sink);

        // Return error if seek failed so caller can handle it
        // (e.g., show a message to user or skip to next song)
        if seek_failed {
            Err("Seek not supported for this format".to_string())
        } else {
            Ok(())
        }
    }

    /// Update cached position when paused
    pub fn update_paused_position(&self, position: Duration) {
        if !self.is_playing() {
            let mut state = self.state.lock().unwrap();
            state.paused_position = Some(position);
        }
    }

    /// Start fade in effect
    pub fn start_fade_in(&self, duration_ms: u32) {
        if let Some(sink) = self.current_sink.as_ref() {
            let mut state = self.state.lock().unwrap();
            let target_volume = state.original_volume * state.track_gain;
            state.fade_state = FadeState::FadingIn {
                start_time: Instant::now(),
                duration: Duration::from_millis(duration_ms as u64),
                start_volume: 0.0,
                target_volume,
            };
            sink.set_volume(0.0);
        }
    }

    /// Start fade out effect
    pub fn start_fade_out(&self, duration_ms: u32) {
        if self.current_sink.is_some() {
            let mut state = self.state.lock().unwrap();
            let current_volume = state.volume * state.track_gain;
            state.fade_state = FadeState::FadingOut {
                start_time: Instant::now(),
                duration: Duration::from_millis(duration_ms as u64),
                start_volume: current_volume,
                target_volume: 0.0,
            };
        }
    }

    /// Update fade state (call regularly from PlaybackTick)
    pub fn update_fade(&self) {
        if let Some(sink) = self.current_sink.as_ref() {
            let mut state = self.state.lock().unwrap();
            let fade_state = state.fade_state.clone();

            match fade_state {
                FadeState::FadingIn {
                    start_time,
                    duration,
                    start_volume,
                    target_volume,
                } => {
                    let elapsed = start_time.elapsed();
                    if elapsed >= duration {
                        sink.set_volume(target_volume);
                        state.fade_state = FadeState::None;
                    } else {
                        let progress = elapsed.as_secs_f32() / duration.as_secs_f32();
                        let current_volume =
                            start_volume + (target_volume - start_volume) * progress;
                        sink.set_volume(current_volume);
                    }
                }
                FadeState::FadingOut {
                    start_time,
                    duration,
                    start_volume,
                    target_volume,
                } => {
                    let elapsed = start_time.elapsed();
                    if elapsed >= duration {
                        sink.set_volume(target_volume);
                        state.fade_state = FadeState::None;

                        if state.status == PlaybackStatus::Pausing {
                            let current_pos = sink.get_pos();
                            sink.pause();
                            state.status = PlaybackStatus::Paused;
                            state.paused_position = Some(current_pos);
                        }
                    } else {
                        let progress = elapsed.as_secs_f32() / duration.as_secs_f32();
                        let current_volume =
                            start_volume + (target_volume - start_volume) * progress;
                        sink.set_volume(current_volume);
                    }
                }
                FadeState::None => {}
            }
        }
    }

    /// Stop any ongoing fade
    pub fn stop_fade(&self) {
        let mut state = self.state.lock().unwrap();
        state.fade_state = FadeState::None;
    }

    /// Set track gain for normalization
    pub fn set_track_gain(&self, gain: f32) {
        let mut state = self.state.lock().unwrap();
        state.track_gain = gain;
        drop(state);

        if let Some(sink) = &self.current_sink {
            let effective_volume = self.get_effective_volume();
            sink.set_volume(effective_volume);
        }
        self.update_preload_volumes();
    }

    /// Get current playback info
    pub fn get_info(&self) -> PlaybackInfo {
        let state = self.state.lock().unwrap();

        let position = if let Some(sink) = &self.current_sink {
            if state.status == PlaybackStatus::Paused {
                state.paused_position.unwrap_or_else(|| sink.get_pos())
            } else {
                sink.get_pos()
            }
        } else {
            Duration::ZERO
        };

        let status = if let Some(sink) = &self.current_sink {
            if sink.empty() && state.status == PlaybackStatus::Playing {
                PlaybackStatus::Stopped
            } else {
                state.status
            }
        } else {
            state.status
        };

        PlaybackInfo {
            status,
            position,
            duration: state.duration,
            volume: state.volume,
        }
    }

    /// Check if currently playing
    pub fn is_playing(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.status == PlaybackStatus::Playing || state.status == PlaybackStatus::Pausing
    }

    /// Check if playback finished
    ///
    /// Returns true if:
    /// 1. Sink is empty (normal completion)
    /// 2. Position >= duration (for formats where sink.empty() may not work correctly)
    /// 3. No sink loaded
    pub fn is_finished(&self) -> bool {
        if let Some(sink) = &self.current_sink {
            // Check if sink is empty (normal completion)
            if sink.empty() {
                return true;
            }

            // Additional check: position >= duration
            // This handles cases where sink.empty() doesn't work correctly
            // (e.g., after failed seek in AAC files)
            let state = self.state.lock().unwrap();
            if state.status == PlaybackStatus::Playing && state.duration.as_secs_f32() > 0.0 {
                let position = sink.get_pos();
                // Consider finished if position is within 500ms of duration
                // This accounts for timing inaccuracies
                if position.as_secs_f32() >= state.duration.as_secs_f32() - 0.5 {
                    return true;
                }
            }

            false
        } else {
            true
        }
    }

    /// Check if player has no loaded audio
    pub fn is_empty(&self) -> bool {
        self.current_sink.is_none()
    }
}

/// Audio device info with internal name and display name
#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,        // Internal name for selection
    pub description: String, // User-friendly display name
}

/// Get list of available audio output devices
/// Uses cpal directly for consistent device names with AudioPlayer
/// Results are cached to avoid repeated enumeration (which triggers Jack/ALSA warnings)
pub fn get_audio_devices() -> Vec<AudioDevice> {
    AUDIO_DEVICES_CACHE
        .get_or_init(|| {
            // Use cpal directly - this ensures device names match what AudioPlayer expects
            let devices = get_cpal_devices();
            if !devices.is_empty() {
                return devices;
            }

            // Fallback to PulseAudio device listing (for display purposes)
            let pa_devices = get_pulseaudio_devices();
            if !pa_devices.is_empty() {
                return pa_devices;
            }

            // Last resort: ALSA devices
            get_alsa_devices()
        })
        .clone()
}

/// Get devices from PulseAudio/PipeWire using pactl command
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

/// Get devices from ALSA using aplay command
fn get_alsa_devices() -> Vec<AudioDevice> {
    let mut devices = Vec::new();

    // Try aplay -l to list ALSA devices
    if let Ok(output) = std::process::Command::new("aplay").args(["-l"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Parse output like: "card 0: PCH [HDA Intel PCH], device 0: ALC892 Analog [ALC892 Analog]"
            for line in stdout.lines() {
                if line.starts_with("card ") {
                    if let Some((card_info, device_info)) = line.split_once(", device ") {
                        // Extract card number
                        let card_num = card_info
                            .trim_start_matches("card ")
                            .split(':')
                            .next()
                            .unwrap_or("0")
                            .trim();

                        // Extract device number
                        let device_num = device_info.split(':').next().unwrap_or("0").trim();

                        // Extract description from brackets
                        let description = if let Some(start) = line.find('[') {
                            if let Some(end) = line.rfind(']') {
                                line[start + 1..end].to_string()
                            } else {
                                line.to_string()
                            }
                        } else {
                            line.to_string()
                        };

                        // ALSA device name format: hw:CARD,DEVICE
                        let name = format!("hw:{},{}", card_num, device_num);

                        devices.push(AudioDevice { name, description });
                    }
                }
            }
        }
    }

    // If aplay failed, try using cpal directly
    if devices.is_empty() {
        devices = get_cpal_devices();
    }

    devices
}

/// Get devices directly from cpal (last resort fallback)
fn get_cpal_devices() -> Vec<AudioDevice> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};

    let host = rodio::cpal::default_host();
    let mut devices = Vec::new();

    if let Ok(output_devices) = host.output_devices() {
        for device in output_devices {
            if let Ok(name) = device.name() {
                let name_lower = name.to_lowercase();

                // Skip problematic backends
                if name_lower.contains("jack")
                    || name_lower.contains("oss")
                    || name_lower.contains("/dev/dsp")
                    || name == "default"
                    || name == "pipewire"
                    || name == "pulse"
                {
                    continue;
                }

                // Check if device has valid output config
                if device.default_output_config().is_ok() {
                    devices.push(AudioDevice {
                        name: name.clone(),
                        description: name, // Use name as description for cpal devices
                    });
                }
            }
        }
    }

    devices
}
