//! Audio processing chain
//!
//! Unified audio processing pipeline that combines:
//! - Preamp (gain control before EQ)
//! - 10-band parametric equalizer
//! - Fade envelope
//! - Real-time audio analyzer for visualization
//!

use rodio::Source;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use super::analyzer::{AnalyzingSource, AudioAnalysisData};
use super::equalizer::{Equalizer, EqualizerParams};
use super::fade::{FadeControl, FadeEnvelope};

struct AudioProcessingChainShared {
    preamp_linear_bits: Arc<AtomicU32>,
    eq_params: EqualizerParams,
    analysis_enabled: AtomicBool,
    analysis_decay_bits: AtomicU32,
    active_analysis: RwLock<AudioAnalysisData>,
    idle_analysis: AudioAnalysisData,
}

/// Shared audio processing chain configuration.
///
/// Only persistent DSP configuration lives here. Per-playback runtime state
/// such as fades and analyzer buffers live in `PlaybackProcessingRuntime`.
#[derive(Clone)]
pub struct AudioProcessingChain {
    shared: Arc<AudioProcessingChainShared>,
}

/// Per-playback processing runtime state.
///
/// Each sink gets its own runtime so current playback and preloaded sinks do
/// not share fade or analyzer state.
#[derive(Clone)]
pub struct PlaybackProcessingRuntime {
    fade_control: FadeControl,
    analysis: AudioAnalysisData,
}

impl AudioProcessingChain {
    /// Create a new audio processing chain
    pub fn new() -> Self {
        let idle_analysis = AudioAnalysisData::new();
        idle_analysis.set_enabled(false);

        Self {
            shared: Arc::new(AudioProcessingChainShared {
                preamp_linear_bits: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
                eq_params: EqualizerParams::new(44100),
                analysis_enabled: AtomicBool::new(false),
                analysis_decay_bits: AtomicU32::new(0.85_f32.to_bits()),
                active_analysis: RwLock::new(idle_analysis.clone()),
                idle_analysis,
            }),
        }
    }

    // Preamp controls
    // ========================================================================

    /// Set preamp gain in dB (-12 to +12)
    pub fn set_preamp(&self, db: f32) {
        let db = db.clamp(-12.0, 12.0);
        let linear = if db.abs() < 0.01 {
            1.0
        } else {
            10.0_f32.powf(db / 20.0)
        };
        self.shared
            .preamp_linear_bits
            .store(linear.to_bits(), Ordering::Release);
    }

    // ========================================================================
    // Equalizer controls
    // ========================================================================

    /// Enable or disable the equalizer
    pub fn set_equalizer_enabled(&self, enabled: bool) {
        self.shared.eq_params.set_enabled(enabled);
    }

    /// Set all 10 band gains at once (in dB, typically -12 to +12)
    pub fn set_equalizer_gains(&self, gains: [f32; 10]) {
        self.shared.eq_params.set_gains(gains);
    }

    // ========================================================================
    // Analysis data access
    // ========================================================================

    /// Get audio analysis data for visualization
    pub fn analysis(&self) -> AudioAnalysisData {
        self.shared
            .active_analysis
            .read()
            .map(|analysis| analysis.clone())
            .unwrap_or_else(|_| self.shared.idle_analysis.clone())
    }

    pub fn set_analysis_enabled(&self, enabled: bool) {
        self.shared
            .analysis_enabled
            .store(enabled, Ordering::Release);
        self.analysis().set_enabled(enabled);
    }

    pub fn set_analysis_decay(&self, decay: f32) {
        let decay = decay.clamp(0.0, 0.99);
        self.shared
            .analysis_decay_bits
            .store(decay.to_bits(), Ordering::Release);
        self.analysis().set_decay(decay);
    }

    /// Force EQ coefficients refresh
    /// This marks the EQ parameters as dirty, forcing a recalculation
    /// on the next audio sample. Useful when switching tracks to ensure
    /// the audio processing chain is properly initialized.
    pub fn refresh_eq_coefficients(&self) {
        self.shared.eq_params.mark_dirty();
    }

    // ========================================================================
    // Chain configuration
    // ========================================================================

    /// Update sample rate (called when audio format changes)
    pub fn set_sample_rate(&self, sample_rate: u32) {
        self.shared.eq_params.set_sample_rate(sample_rate);
    }

    // ========================================================================
    // Runtime lifecycle
    // ========================================================================

    pub fn create_runtime(&self) -> PlaybackProcessingRuntime {
        PlaybackProcessingRuntime::new(self.analysis_decay())
    }

    pub fn activate_runtime(&self, runtime: Option<&PlaybackProcessingRuntime>) {
        let analysis = if let Some(runtime) = runtime {
            runtime.prepare_analysis(self.analysis_enabled(), self.analysis_decay());
            runtime.analysis.clone()
        } else {
            self.shared.idle_analysis.reset();
            self.shared.idle_analysis.set_enabled(false);
            self.shared.idle_analysis.clone()
        };

        if let Ok(mut active_analysis) = self.shared.active_analysis.write() {
            *active_analysis = analysis;
        }
    }

    fn analysis_enabled(&self) -> bool {
        self.shared.analysis_enabled.load(Ordering::Acquire)
    }

    fn analysis_decay(&self) -> f32 {
        f32::from_bits(self.shared.analysis_decay_bits.load(Ordering::Acquire))
    }

    // ========================================================================
    // Source processing
    // ========================================================================

    /// Apply the processing chain to an audio source using the provided
    /// per-playback runtime state.
    pub fn apply<S>(
        &self,
        source: S,
        track_gain: f32,
        runtime: PlaybackProcessingRuntime,
    ) -> ProcessedSource<S>
    where
        S: Source<Item = f32>,
    {
        // Update sample rate from source
        self.set_sample_rate(source.sample_rate());

        ProcessedSource::new(source, self.shared.clone(), runtime, track_gain)
    }
}

impl Default for AudioProcessingChain {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackProcessingRuntime {
    fn new(analysis_decay: f32) -> Self {
        let analysis = AudioAnalysisData::new();
        analysis.set_enabled(false);
        analysis.set_decay(analysis_decay);

        Self {
            fade_control: FadeControl::new(1.0),
            analysis,
        }
    }

    pub fn fade_to(&self, volume: f32, duration: std::time::Duration) {
        self.fade_control.fade_to(volume, duration);
    }

    pub fn set_fade_volume(&self, volume: f32) {
        self.fade_control.set_volume(volume);
    }

    fn prepare_analysis(&self, enabled: bool, decay: f32) {
        self.analysis.set_decay(decay);
        self.analysis.reset();
        self.analysis.set_enabled(enabled);
    }
}

/// Audio source with processing chain applied
///
/// This wraps the source and applies track gain, preamp, EQ, fade, and analysis in sequence.
pub struct ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    /// Inner source with full processing chain applied
    inner:
        AnalyzingSource<FadeEnvelope<SoftClipSource<Equalizer<PreampSource<TrackGainSource<S>>>>>>,
}

impl<S> ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    fn new(
        source: S,
        shared: Arc<AudioProcessingChainShared>,
        runtime: PlaybackProcessingRuntime,
        track_gain: f32,
    ) -> Self {
        // Build processing chain: Source -> TrackGain -> Preamp -> EQ -> SoftClip -> Fade -> Analyzer
        let track_gain_source = TrackGainSource::new(source, track_gain);
        let preamp_source = PreampSource::new(track_gain_source, shared.preamp_linear_bits.clone());
        let eq_source = Equalizer::new(preamp_source, shared.eq_params.clone());
        let clip_source = SoftClipSource::new(eq_source);
        let fade_source = FadeEnvelope::new(clip_source, runtime.fade_control.clone());
        let analyzed = AnalyzingSource::new(fade_source, runtime.analysis.clone());

        Self { inner: analyzed }
    }
}

impl<S> Iterator for ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Source for ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: std::time::Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(pos)
    }
}

/// Per-track gain applied from normalization metadata or waveform analysis.
struct TrackGainSource<S>
where
    S: Source<Item = f32>,
{
    source: S,
    gain: f32,
}

impl<S> TrackGainSource<S>
where
    S: Source<Item = f32>,
{
    fn new(source: S, gain: f32) -> Self {
        Self {
            source,
            gain: gain.max(0.0),
        }
    }
}

impl<S> Iterator for TrackGainSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.source.next()?;
        let gain = self.gain;

        if (gain - 1.0).abs() < 0.001 {
            Some(sample)
        } else {
            Some(sample * gain)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.source.size_hint()
    }
}

impl<S> Source for TrackGainSource<S>
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

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.source.total_duration()
    }

    fn try_seek(&mut self, pos: std::time::Duration) -> Result<(), rodio::source::SeekError> {
        self.source.try_seek(pos)
    }
}

/// Shared preamp wrapper driven by UI settings.
struct PreampSource<S>
where
    S: Source<Item = f32>,
{
    source: S,
    preamp_linear_bits: Arc<AtomicU32>,
}

impl<S> PreampSource<S>
where
    S: Source<Item = f32>,
{
    fn new(source: S, preamp_linear_bits: Arc<AtomicU32>) -> Self {
        Self {
            source,
            preamp_linear_bits,
        }
    }

    fn preamp_linear(&self) -> f32 {
        f32::from_bits(self.preamp_linear_bits.load(Ordering::Acquire))
    }
}

impl<S> Iterator for PreampSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.source.next()?;
        let gain = self.preamp_linear();

        if (gain - 1.0).abs() < 0.001 {
            Some(sample)
        } else {
            Some(sample * gain)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.source.size_hint()
    }
}

impl<S> Source for PreampSource<S>
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

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.source.total_duration()
    }

    fn try_seek(&mut self, pos: std::time::Duration) -> Result<(), rodio::source::SeekError> {
        self.source.try_seek(pos)
    }
}

/// Final soft-clip stage applied once after gain and EQ.
struct SoftClipSource<S>
where
    S: Source<Item = f32>,
{
    source: S,
}

impl<S> SoftClipSource<S>
where
    S: Source<Item = f32>,
{
    fn new(source: S) -> Self {
        Self { source }
    }
}

impl<S> Iterator for SoftClipSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.source.next()?;
        if sample.abs() < 0.9 {
            Some(sample)
        } else {
            Some(soft_clip(sample))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.source.size_hint()
    }
}

impl<S> Source for SoftClipSource<S>
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

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.source.total_duration()
    }

    fn try_seek(&mut self, pos: std::time::Duration) -> Result<(), rodio::source::SeekError> {
        self.source.try_seek(pos)
    }
}

/// Soft clipping function to prevent harsh digital clipping
fn soft_clip(x: f32) -> f32 {
    if x.abs() < 0.9 {
        x
    } else if x > 0.0 {
        0.9 + 0.1 * ((x - 0.9) / 0.1).tanh()
    } else {
        -0.9 - 0.1 * ((-x - 0.9) / 0.1).tanh()
    }
}
