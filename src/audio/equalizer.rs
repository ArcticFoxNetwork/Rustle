//! 10-band parametric equalizer using biquad filters
//!
//! Standard 10-band EQ frequencies:
//! 31Hz, 62Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz

use rodio::Source;
use std::sync::{Arc, RwLock};

/// Standard 10-band equalizer center frequencies in Hz
pub const EQ_FREQUENCIES: [f32; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// Biquad filter coefficients
#[derive(Clone, Copy, Default)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// Biquad filter state for one channel
#[derive(Clone, Copy, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    fn process(&mut self, coeffs: &BiquadCoeffs, input: f32) -> f32 {
        let output = coeffs.b0 * input + coeffs.b1 * self.x1 + coeffs.b2 * self.x2
            - coeffs.a1 * self.y1
            - coeffs.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// Calculate peaking EQ filter coefficients
/// gain_db: gain in decibels (-12 to +12 typical)
/// freq: center frequency in Hz
/// sample_rate: audio sample rate
/// q: quality factor (bandwidth), typically 1.0-2.0 for EQ
fn calc_peaking_eq(freq: f32, gain_db: f32, sample_rate: f32, q: f32) -> BiquadCoeffs {
    if gain_db.abs() < 0.01 {
        // Unity gain - bypass filter
        return BiquadCoeffs {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        };
    }

    let a = 10.0_f32.powf(gain_db / 40.0);
    let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let sin_omega = omega.sin();
    let cos_omega = omega.cos();
    let alpha = sin_omega / (2.0 * q);

    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_omega;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_omega;
    let a2 = 1.0 - alpha / a;

    // Normalize coefficients
    BiquadCoeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Shared equalizer parameters that can be updated in real-time
#[derive(Clone)]
pub struct EqualizerParams {
    inner: Arc<RwLock<EqualizerParamsInner>>,
}

struct EqualizerParamsInner {
    enabled: bool,
    gains: [f32; 10],
    sample_rate: u32,
    coeffs_dirty: bool,
}

impl EqualizerParams {
    /// Create new equalizer parameters
    pub fn new(sample_rate: u32) -> Self {
        Self {
            inner: Arc::new(RwLock::new(EqualizerParamsInner {
                enabled: false,
                gains: [0.0; 10],
                sample_rate,
                coeffs_dirty: true,
            })),
        }
    }

    /// Enable or disable the equalizer
    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut inner) = self.inner.write() {
            inner.enabled = enabled;
            inner.coeffs_dirty = true;
        }
    }

    /// Check if equalizer is enabled
    pub fn is_enabled(&self) -> bool {
        self.inner.read().map(|i| i.enabled).unwrap_or(false)
    }

    /// Set all 10 band gains at once (in dB, typically -12 to +12)
    pub fn set_gains(&self, gains: [f32; 10]) {
        if let Ok(mut inner) = self.inner.write() {
            inner.gains = gains;
            inner.coeffs_dirty = true;
        }
    }

    /// Set a single band gain
    #[allow(dead_code)]
    pub fn set_band_gain(&self, band: usize, gain_db: f32) {
        if band < 10 {
            if let Ok(mut inner) = self.inner.write() {
                inner.gains[band] = gain_db.clamp(-12.0, 12.0);
                inner.coeffs_dirty = true;
            }
        }
    }

    /// Get current gains
    #[allow(dead_code)]
    pub fn get_gains(&self) -> [f32; 10] {
        self.inner.read().map(|i| i.gains).unwrap_or([0.0; 10])
    }

    /// Update sample rate (call when audio format changes)
    pub fn set_sample_rate(&self, sample_rate: u32) {
        if let Ok(mut inner) = self.inner.write() {
            if inner.sample_rate != sample_rate {
                inner.sample_rate = sample_rate;
                inner.coeffs_dirty = true;
            }
        }
    }

    /// 标记系数为脏，强制下次采样时重新计算
    /// 切换曲目时使用，确保 EQ 正确初始化
    pub fn mark_dirty(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.coeffs_dirty = true;
        }
    }
}

/// 10-band equalizer Source wrapper
pub struct Equalizer<S>
where
    S: Source<Item = f32>,
{
    source: S,
    params: EqualizerParams,
    // Filter coefficients for each band
    coeffs: [BiquadCoeffs; 10],
    // Filter state for each band, per channel (stereo = 2 channels)
    states: [[BiquadState; 10]; 2],
    // Current channel being processed (for interleaved stereo)
    current_channel: usize,
    channels: u16,
}

impl<S> Equalizer<S>
where
    S: Source<Item = f32>,
{
    /// Create a new equalizer wrapping the given source
    pub fn new(source: S, params: EqualizerParams) -> Self {
        let channels = source.channels();
        let sample_rate = source.sample_rate();

        // Update params with actual sample rate
        params.set_sample_rate(sample_rate);

        // 强制重新计算系数，新实例的系数是默认值
        params.mark_dirty();

        let mut eq = Self {
            source,
            params,
            coeffs: [BiquadCoeffs::default(); 10],
            states: [[BiquadState::default(); 10]; 2],
            current_channel: 0,
            channels,
        };

        eq.update_coefficients();
        eq
    }

    /// Update filter coefficients from current parameters
    fn update_coefficients(&mut self) {
        let (enabled, gains, sample_rate, coeffs_dirty) = {
            let inner = self.params.inner.read().unwrap();
            (
                inner.enabled,
                inner.gains,
                inner.sample_rate,
                inner.coeffs_dirty,
            )
        };

        if !coeffs_dirty {
            return;
        }

        // Mark as clean
        if let Ok(mut inner) = self.params.inner.write() {
            inner.coeffs_dirty = false;
        }

        if !enabled {
            // Set all filters to unity gain (bypass)
            for coeff in &mut self.coeffs {
                *coeff = BiquadCoeffs {
                    b0: 1.0,
                    b1: 0.0,
                    b2: 0.0,
                    a1: 0.0,
                    a2: 0.0,
                };
            }
            return;
        }

        // Q factor for each band - wider at low frequencies, narrower at high
        let q_values: [f32; 10] = [0.7, 0.8, 1.0, 1.2, 1.4, 1.4, 1.4, 1.2, 1.0, 0.8];

        for (i, &freq) in EQ_FREQUENCIES.iter().enumerate() {
            self.coeffs[i] = calc_peaking_eq(freq, gains[i], sample_rate as f32, q_values[i]);
        }
    }
}

impl<S> Iterator for Equalizer<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        // Check if coefficients need updating
        if self
            .params
            .inner
            .read()
            .map(|i| i.coeffs_dirty)
            .unwrap_or(false)
        {
            self.update_coefficients();
        }

        let sample = self.source.next()?;

        // Check if EQ is enabled
        let enabled = self.params.is_enabled();
        if !enabled {
            // Bypass - return original sample
            self.current_channel = (self.current_channel + 1) % self.channels as usize;
            return Some(sample);
        }

        // Process through all 10 bands in series
        let channel = self.current_channel.min(1); // Clamp to stereo
        let mut output = sample;

        for (i, coeff) in self.coeffs.iter().enumerate() {
            output = self.states[channel][i].process(coeff, output);
        }

        // Soft clip to prevent harsh distortion
        output = soft_clip(output);

        self.current_channel = (self.current_channel + 1) % self.channels as usize;
        Some(output)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.source.size_hint()
    }
}

impl<S> Source for Equalizer<S>
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
        // Reset filter states when seeking to avoid audio artifacts
        for channel_states in &mut self.states {
            for state in channel_states {
                state.reset();
            }
        }
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
