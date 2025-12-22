//! Audio processing chain
//!
//! Unified audio processing pipeline that combines:
//! - Preamp (gain control before EQ)
//! - 10-band parametric equalizer
//! - Real-time audio analyzer for visualization
//!
//! This module is designed to be shared between AudioPlayer and UI,
//! allowing real-time parameter updates without tight coupling.

use rodio::Source;
use std::sync::{Arc, RwLock};

use super::analyzer::{AnalyzingSource, AudioAnalysisData};
use super::equalizer::{Equalizer, EqualizerParams};

/// Shared audio processing chain parameters
///
/// This struct holds all audio processing parameters and can be cloned
/// to share between AudioPlayer and UI. All parameters are thread-safe
/// and can be updated in real-time.
#[derive(Clone)]
pub struct AudioProcessingChain {
    inner: Arc<RwLock<ChainInner>>,
    /// Equalizer parameters (has its own Arc<RwLock>)
    eq_params: EqualizerParams,
    /// Audio analysis data for visualization
    analysis: AudioAnalysisData,
}

struct ChainInner {
    /// Preamp gain in dB (-12 to +12)
    preamp_db: f32,
    /// Current sample rate
    sample_rate: u32,
}

impl Default for ChainInner {
    fn default() -> Self {
        Self {
            preamp_db: 0.0,
            sample_rate: 44100,
        }
    }
}

impl AudioProcessingChain {
    /// Create a new audio processing chain
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ChainInner::default())),
            eq_params: EqualizerParams::new(44100),
            analysis: AudioAnalysisData::new(),
        }
    }

    // ========================================================================
    // Preamp controls
    // ========================================================================

    /// Set preamp gain in dB (-12 to +12)
    pub fn set_preamp(&self, db: f32) {
        if let Ok(mut inner) = self.inner.write() {
            inner.preamp_db = db.clamp(-12.0, 12.0);
        }
    }

    // ========================================================================
    // Equalizer controls
    // ========================================================================

    /// Enable or disable the equalizer
    pub fn set_equalizer_enabled(&self, enabled: bool) {
        self.eq_params.set_enabled(enabled);
    }

    /// Set all 10 band gains at once (in dB, typically -12 to +12)
    pub fn set_equalizer_gains(&self, gains: [f32; 10]) {
        self.eq_params.set_gains(gains);
    }

    // ========================================================================
    // Analysis data access
    // ========================================================================

    /// Get audio analysis data for visualization
    pub fn analysis(&self) -> &AudioAnalysisData {
        &self.analysis
    }

    /// Reset analysis data (call when playback stops)
    pub fn reset_analysis(&self) {
        self.analysis.reset();
    }

    /// Force EQ coefficients refresh
    /// This marks the EQ parameters as dirty, forcing a recalculation
    /// on the next audio sample. Useful when switching tracks to ensure
    /// the audio processing chain is properly initialized.
    pub fn refresh_eq_coefficients(&self) {
        self.eq_params.mark_dirty();
    }

    // ========================================================================
    // Chain configuration
    // ========================================================================

    /// Update sample rate (called when audio format changes)
    pub fn set_sample_rate(&self, sample_rate: u32) {
        if let Ok(mut inner) = self.inner.write() {
            inner.sample_rate = sample_rate;
        }
        self.eq_params.set_sample_rate(sample_rate);
    }

    // ========================================================================
    // Source processing
    // ========================================================================

    /// Apply the processing chain to an audio source
    ///
    /// Processing order:
    /// 1. Preamp (gain adjustment)
    /// 2. Equalizer (10-band parametric EQ)
    /// 3. Analyzer (for visualization, doesn't modify audio)
    pub fn apply<S>(&self, source: S) -> ProcessedSource<S>
    where
        S: Source<Item = f32>,
    {
        // Update sample rate from source
        self.set_sample_rate(source.sample_rate());

        ProcessedSource::new(source, self.clone())
    }
}

impl Default for AudioProcessingChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Audio source with processing chain applied
///
/// This wraps the source and applies preamp, EQ, and analysis in sequence.
pub struct ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    /// Inner source with EQ and analyzer applied
    inner: AnalyzingSource<Equalizer<PreampSource<S>>>,
}

impl<S> ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    fn new(source: S, chain: AudioProcessingChain) -> Self {
        // Build processing chain: Source -> Preamp -> EQ -> Analyzer
        let preamp_source = PreampSource::new(source, chain.inner.clone());
        let eq_source = Equalizer::new(preamp_source, chain.eq_params.clone());
        let analyzed = AnalyzingSource::new(eq_source, chain.analysis.clone());

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

/// Preamp source wrapper that applies gain before other processing
struct PreampSource<S>
where
    S: Source<Item = f32>,
{
    source: S,
    chain_inner: Arc<RwLock<ChainInner>>,
}

impl<S> PreampSource<S>
where
    S: Source<Item = f32>,
{
    fn new(source: S, chain_inner: Arc<RwLock<ChainInner>>) -> Self {
        Self {
            source,
            chain_inner,
        }
    }

    fn preamp_linear(&self) -> f32 {
        let db = self.chain_inner.read().map(|i| i.preamp_db).unwrap_or(0.0);
        if db.abs() < 0.01 {
            1.0
        } else {
            10.0_f32.powf(db / 20.0)
        }
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
            // Apply gain with soft clipping to prevent harsh distortion
            let amplified = sample * gain;
            Some(soft_clip(amplified))
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
