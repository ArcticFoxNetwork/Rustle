//! Fade envelope - self-driving volume automation
//!
//! A pure audio processor that smoothly transitions volume without
//! knowing anything about playback state. It just does one thing:
//! smoothly interpolate volume from current to target.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use rodio::Source;

/// Shared fade control handle
///
/// Clone this to control fade from outside the audio thread.
/// All operations are lock-free using atomics.
#[derive(Clone)]
pub struct FadeControl {
    inner: Arc<FadeControlInner>,
}

struct FadeControlInner {
    /// Target volume (0.0 - 1.0), stored as u32 bits
    target_volume: AtomicU32,
    /// Fade duration in milliseconds
    fade_duration_ms: AtomicU32,
    /// Generation counter - incremented on each fade_to() call
    /// Used to detect new fade requests
    generation: AtomicU32,
}

impl FadeControl {
    /// Create a new fade control with initial volume
    pub fn new(initial_volume: f32) -> Self {
        Self {
            inner: Arc::new(FadeControlInner {
                target_volume: AtomicU32::new(initial_volume.to_bits()),
                fade_duration_ms: AtomicU32::new(0),
                generation: AtomicU32::new(0),
            }),
        }
    }

    /// Start a fade to target volume over duration
    ///
    /// This is the only API you need. Examples:
    /// - Fade in: `fade_to(1.0, Duration::from_millis(300))`
    /// - Fade out: `fade_to(0.0, Duration::from_millis(300))`
    /// - Instant: `fade_to(0.5, Duration::ZERO)`
    pub fn fade_to(&self, volume: f32, duration: Duration) {
        let volume = volume.clamp(0.0, 1.0);
        self.inner
            .target_volume
            .store(volume.to_bits(), Ordering::Release);
        self.inner
            .fade_duration_ms
            .store(duration.as_millis() as u32, Ordering::Release);
        self.inner.generation.fetch_add(1, Ordering::Release);
    }

    /// Set volume instantly (no fade)
    pub fn set_volume(&self, volume: f32) {
        self.fade_to(volume, Duration::ZERO);
    }

    /// Get current target volume
    pub fn target_volume(&self) -> f32 {
        f32::from_bits(self.inner.target_volume.load(Ordering::Acquire))
    }

    fn generation(&self) -> u32 {
        self.inner.generation.load(Ordering::Acquire)
    }

    fn fade_duration_ms(&self) -> u32 {
        self.inner.fade_duration_ms.load(Ordering::Acquire)
    }
}

impl Default for FadeControl {
    fn default() -> Self {
        Self::new(1.0)
    }
}

/// Fade envelope source wrapper
///
/// Wraps any audio source and applies smooth volume transitions.
/// Runs entirely in the audio thread - no external tick needed.
pub struct FadeEnvelope<S>
where
    S: Source<Item = f32>,
{
    source: S,
    control: FadeControl,
    /// Current volume (what we're outputting now)
    current_volume: f32,
    /// Volume at the start of current fade
    fade_start_volume: f32,
    /// Samples remaining in current fade
    fade_samples_remaining: u32,
    /// Total samples for current fade
    fade_samples_total: u32,
    /// Last seen generation (to detect new fade requests)
    last_generation: u32,
    /// Cached sample rate
    sample_rate: u32,
}

impl<S> FadeEnvelope<S>
where
    S: Source<Item = f32>,
{
    /// Create a new fade envelope wrapping a source
    pub fn new(source: S, control: FadeControl) -> Self {
        let sample_rate = source.sample_rate();
        let initial_volume = control.target_volume();
        let generation = control.generation();

        Self {
            source,
            control,
            current_volume: initial_volume,
            fade_start_volume: initial_volume,
            fade_samples_remaining: 0,
            fade_samples_total: 0,
            last_generation: generation,
            sample_rate,
        }
    }

    /// Check for new fade requests and start fade if needed
    fn check_fade_request(&mut self) {
        let current_gen = self.control.generation();
        if current_gen != self.last_generation {
            self.last_generation = current_gen;

            let duration_ms = self.control.fade_duration_ms();
            let target = self.control.target_volume();

            if duration_ms == 0 {
                // Instant change
                self.current_volume = target;
                self.fade_samples_remaining = 0;
            } else {
                // Start new fade
                self.fade_start_volume = self.current_volume;
                self.fade_samples_total =
                    (self.sample_rate as u64 * duration_ms as u64 / 1000) as u32;
                self.fade_samples_remaining = self.fade_samples_total;
            }
        }
    }

    /// Update current volume based on fade progress
    fn update_volume(&mut self) {
        if self.fade_samples_remaining > 0 {
            self.fade_samples_remaining -= 1;

            let target = self.control.target_volume();
            let progress =
                1.0 - (self.fade_samples_remaining as f32 / self.fade_samples_total as f32);

            // Smooth easing (ease-out cubic)
            let eased = 1.0 - (1.0 - progress).powi(3);
            self.current_volume =
                self.fade_start_volume + (target - self.fade_start_volume) * eased;

            // Snap to target when done
            if self.fade_samples_remaining == 0 {
                self.current_volume = target;
            }
        }
    }
}

impl<S> Iterator for FadeEnvelope<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        // Check for new fade requests (lock-free)
        self.check_fade_request();

        // Update volume based on fade progress
        self.update_volume();

        // Apply volume to sample
        self.source
            .next()
            .map(|sample| sample * self.current_volume)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.source.size_hint()
    }
}

impl<S> Source for FadeEnvelope<S>
where
    S: Source<Item = f32>,
{
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.source.try_seek(pos)
    }
}
