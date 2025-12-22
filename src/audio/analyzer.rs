//! Real-time audio spectrum analyzer
//!
//! Provides FFT-based frequency spectrum analysis with:
//! - 4096-point FFT for high frequency resolution (~11.7Hz per bin at 48kHz)
//! - Logarithmic frequency scale (20Hz - 20kHz)
//! - dB scale with decay smoothing
//! - RMS level metering

use rodio::Source;
use spectrum_analyzer::scaling::divide_by_N_sqrt;
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum};
use std::sync::{Arc, RwLock};

/// FFT size - 4096 gives ~11.7Hz resolution at 48kHz
pub const FFT_SIZE: usize = 4096;

/// Number of spectrum bars for visualization
pub const SPECTRUM_BARS: usize = 128;

/// Minimum frequency (Hz)
const MIN_FREQ: f32 = 20.0;

/// Maximum frequency (Hz)
const MAX_FREQ: f32 = 20000.0;

/// Audio analysis data shared between audio thread and UI
#[derive(Clone)]
pub struct AudioAnalysisData {
    inner: Arc<RwLock<AudioAnalysisInner>>,
}

struct AudioAnalysisInner {
    /// Left channel RMS level (0.0 to 1.0)
    left_rms: f32,
    /// Right channel RMS level (0.0 to 1.0)
    right_rms: f32,
    /// Spectrum magnitude in dB for each bar (smoothed with decay)
    spectrum_db: Vec<f32>,
    /// Peak hold values for each bar
    peak_db: Vec<f32>,
    /// Sample buffer for FFT (mono mixed)
    sample_buffer: Vec<f32>,
    /// Left channel samples for RMS
    left_samples: Vec<f32>,
    /// Right channel samples for RMS
    right_samples: Vec<f32>,
    /// Current channel index (for interleaved stereo)
    current_channel: usize,
    /// Number of channels
    channels: u16,
    /// Sample rate
    sample_rate: u32,
    /// Decay factor (0.0 = instant, 1.0 = no decay)
    decay: f32,
    /// Peak decay factor
    peak_decay: f32,
}

impl Default for AudioAnalysisInner {
    fn default() -> Self {
        Self {
            left_rms: 0.0,
            right_rms: 0.0,
            spectrum_db: vec![-60.0; SPECTRUM_BARS],
            peak_db: vec![-60.0; SPECTRUM_BARS],
            sample_buffer: Vec::with_capacity(FFT_SIZE),
            left_samples: Vec::with_capacity(FFT_SIZE / 2),
            right_samples: Vec::with_capacity(FFT_SIZE / 2),
            current_channel: 0,
            channels: 2,
            sample_rate: 48000,
            decay: 0.85,      // Smooth decay
            peak_decay: 0.98, // Slow peak fall
        }
    }
}

impl AudioAnalysisData {
    /// Create new audio analysis data
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(AudioAnalysisInner::default())),
        }
    }

    /// Get left channel RMS level (0.0 to 1.0)
    pub fn left_rms(&self) -> f32 {
        self.inner.read().map(|i| i.left_rms).unwrap_or(0.0)
    }

    /// Get right channel RMS level (0.0 to 1.0)
    pub fn right_rms(&self) -> f32 {
        self.inner.read().map(|i| i.right_rms).unwrap_or(0.0)
    }

    /// Get spectrum data in dB (SPECTRUM_BARS values, -60 to +12 dB range)
    pub fn spectrum_db(&self) -> Vec<f32> {
        self.inner
            .read()
            .map(|i| i.spectrum_db.clone())
            .unwrap_or_else(|_| vec![-60.0; SPECTRUM_BARS])
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.inner.read().map(|i| i.sample_rate).unwrap_or(48000)
    }

    /// Set decay factor (0.0 = instant, 0.99 = very slow)
    pub fn set_decay(&self, decay: f32) {
        if let Ok(mut inner) = self.inner.write() {
            inner.decay = decay.clamp(0.0, 0.99);
        }
    }

    /// Reset analysis data (call when playback stops)
    pub fn reset(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.left_rms = 0.0;
            inner.right_rms = 0.0;
            inner.spectrum_db.fill(-60.0);
            inner.peak_db.fill(-60.0);
            inner.sample_buffer.clear();
            inner.left_samples.clear();
            inner.right_samples.clear();
        }
    }

    /// Update sample rate and channels
    fn configure(&self, sample_rate: u32, channels: u16) {
        if let Ok(mut inner) = self.inner.write() {
            inner.sample_rate = sample_rate;
            inner.channels = channels;
            inner.current_channel = 0;
        }
    }

    /// Process a single sample
    fn process_sample(&self, sample: f32) {
        if let Ok(mut inner) = self.inner.write() {
            let channel = inner.current_channel;
            let channels = inner.channels as usize;

            // Accumulate samples for RMS
            if channel == 0 {
                inner.left_samples.push(sample);
                // Also add to FFT buffer (mono)
                inner.sample_buffer.push(sample);
            } else if channel == 1 {
                inner.right_samples.push(sample);
                // Mix into FFT buffer for stereo
                if let Some(last) = inner.sample_buffer.last_mut() {
                    *last = (*last + sample) * 0.5;
                }
            }

            // Update channel counter
            inner.current_channel = (channel + 1) % channels;

            // Check if we have enough samples for FFT
            if inner.sample_buffer.len() >= FFT_SIZE {
                Self::perform_fft(&mut inner);
            }
        }
    }

    /// Perform FFT analysis and update spectrum
    fn perform_fft(inner: &mut AudioAnalysisInner) {
        // Calculate RMS first
        if !inner.left_samples.is_empty() {
            let sum_sq: f32 = inner.left_samples.iter().map(|s| s * s).sum();
            let rms = (sum_sq / inner.left_samples.len() as f32).sqrt();
            inner.left_rms = inner.left_rms * 0.7 + rms.min(1.0) * 0.3;
        }
        if !inner.right_samples.is_empty() {
            let sum_sq: f32 = inner.right_samples.iter().map(|s| s * s).sum();
            let rms = (sum_sq / inner.right_samples.len() as f32).sqrt();
            inner.right_rms = inner.right_rms * 0.7 + rms.min(1.0) * 0.3;
        }

        // Apply Hann window to samples
        let samples: Vec<f32> = inner.sample_buffer[..FFT_SIZE].to_vec();
        let windowed = hann_window(&samples);

        // Perform FFT
        if let Ok(spectrum) = samples_fft_to_spectrum(
            &windowed,
            inner.sample_rate,
            FrequencyLimit::Range(MIN_FREQ, MAX_FREQ),
            Some(&divide_by_N_sqrt),
        ) {
            // Map FFT bins to logarithmic frequency bars
            let freq_data = spectrum.data();
            let decay = inner.decay;
            let peak_decay = inner.peak_decay;

            for bar_idx in 0..SPECTRUM_BARS {
                // Calculate frequency range for this bar (logarithmic scale)
                let t0 = bar_idx as f32 / SPECTRUM_BARS as f32;
                let t1 = (bar_idx + 1) as f32 / SPECTRUM_BARS as f32;
                let freq_low = MIN_FREQ * (MAX_FREQ / MIN_FREQ).powf(t0);
                let freq_high = MIN_FREQ * (MAX_FREQ / MIN_FREQ).powf(t1);

                // Find max magnitude in this frequency range
                let mut max_mag: f32 = 0.0;
                for (freq, mag) in freq_data.iter() {
                    let f = freq.val();
                    if f >= freq_low && f < freq_high {
                        max_mag = max_mag.max(mag.val());
                    }
                }

                // Convert to dB (with floor at -60dB)
                let db = if max_mag > 0.0 {
                    (20.0 * max_mag.log10()).clamp(-60.0, 12.0)
                } else {
                    -60.0
                };

                // Apply decay smoothing
                let current = inner.spectrum_db[bar_idx];
                inner.spectrum_db[bar_idx] = if db > current {
                    // Attack: fast rise
                    current * 0.3 + db * 0.7
                } else {
                    // Decay: smooth fall
                    current * decay + db * (1.0 - decay)
                };

                // Update peak hold
                if db > inner.peak_db[bar_idx] {
                    inner.peak_db[bar_idx] = db;
                } else {
                    inner.peak_db[bar_idx] =
                        inner.peak_db[bar_idx] * peak_decay + (-60.0) * (1.0 - peak_decay);
                }
            }
        }

        // Keep overlap for smoother updates (50% overlap)
        let overlap = FFT_SIZE / 2;
        inner.sample_buffer.drain(0..overlap);
        inner.left_samples.clear();
        inner.right_samples.clear();
    }
}

impl Default for AudioAnalysisData {
    fn default() -> Self {
        Self::new()
    }
}

/// Audio analyzer source wrapper
pub struct AnalyzingSource<S>
where
    S: Source<Item = f32>,
{
    source: S,
    analysis: AudioAnalysisData,
}

impl<S> AnalyzingSource<S>
where
    S: Source<Item = f32>,
{
    /// Create a new analyzing source
    pub fn new(source: S, analysis: AudioAnalysisData) -> Self {
        analysis.configure(source.sample_rate(), source.channels());
        Self { source, analysis }
    }
}

impl<S> Iterator for AnalyzingSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.source.next()?;
        self.analysis.process_sample(sample);
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.source.size_hint()
    }
}

impl<S> Source for AnalyzingSource<S>
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
        // Reset analysis buffers when seeking
        self.analysis.reset();
        self.source.try_seek(pos)
    }
}
