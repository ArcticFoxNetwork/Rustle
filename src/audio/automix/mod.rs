//! Native, opt-in Automix planning primitives.
//!
//! This module intentionally contains deterministic data contracts and pure
//! planning/math. The audio thread remains the owner of playback execution;
//! analysis and scheduling identities are immutable snapshots that can be
//! discarded when their generation/group is stale.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::fs;
use std::hash::Hasher;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio::Source;
use serde::{Deserialize, Serialize};

pub const ANALYSIS_SCHEMA_VERSION: u32 = 2;
pub const ANALYZER_VERSION: &str = "rustle-native-v2";
pub const DEFAULT_ANALYSIS_MAX_SECONDS: u32 = 60;
pub const BASELINE_CROSSFADE_MS: u32 = 5_000;
pub const MANUAL_CROSSFADE_MS: u32 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub max_seconds: u32,
    pub schema_version: u32,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            max_seconds: DEFAULT_ANALYSIS_MAX_SECONDS,
            schema_version: ANALYSIS_SCHEMA_VERSION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackAnalysis {
    pub schema_version: u32,
    pub analyzer_version: String,
    pub content_id: String,
    pub duration: Duration,
    pub bpm: Option<f32>,
    pub bpm_confidence: f32,
    pub first_beat: Option<Duration>,
    pub beat_period: Option<Duration>,
    pub bar_beats: u8,
    pub lufs: Option<f32>,
    pub energy: f32,
    pub vocals_confidence: f32,
    pub key: Option<String>,
    pub cut_start: Duration,
    pub fade_out_start: Duration,
    pub recommended_exit: Duration,
    pub recommended_entry: Duration,
}

impl TrackAnalysis {
    pub fn fallback(content_id: impl Into<String>, duration: Duration) -> Self {
        let exit = duration.saturating_sub(Duration::from_secs(5));
        Self {
            schema_version: ANALYSIS_SCHEMA_VERSION,
            analyzer_version: ANALYZER_VERSION.to_string(),
            content_id: content_id.into(),
            duration,
            bpm: None,
            bpm_confidence: 0.0,
            first_beat: None,
            beat_period: None,
            bar_beats: 4,
            lufs: None,
            energy: 0.0,
            vocals_confidence: 0.0,
            key: None,
            cut_start: Duration::ZERO,
            fade_out_start: exit,
            recommended_exit: exit,
            recommended_entry: Duration::ZERO,
        }
    }

    pub fn is_compatible(&self, config: AnalysisConfig) -> bool {
        self.schema_version == config.schema_version
            && self.analyzer_version == ANALYZER_VERSION
            && !self.content_id.is_empty()
            && self.duration > Duration::ZERO
            && self.bpm_confidence.is_finite()
            && (0.0..=1.0).contains(&self.bpm_confidence)
    }
}

/// Deterministic FNV-1a cache key; avoids relying on randomized hash seeds.
pub fn cache_key(content_id: &str, config: AnalysisConfig) -> String {
    let mut hasher = Fnv64::default();
    hasher.write(content_id.as_bytes());
    hasher.write_u32(config.schema_version);
    hasher.write(ANALYZER_VERSION.as_bytes());
    hasher.write_u32(config.max_seconds);
    format!("{:016x}", hasher.finish())
}

pub fn content_identity(path: &Path, fallback: &str) -> String {
    match path.metadata() {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|value| value.as_nanos())
                .unwrap_or(0);
            format!(
                "file:{}:{}:{}",
                path.to_string_lossy(),
                metadata.len(),
                modified
            )
        }
        Err(_) => format!("track:{fallback}"),
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisCache {
    root: PathBuf,
    max_entries: usize,
}

impl AnalysisCache {
    pub fn new(root: impl Into<PathBuf>, max_entries: usize) -> Self {
        Self {
            root: root.into(),
            max_entries: max_entries.max(1),
        }
    }

    pub fn app_default() -> Self {
        Self::new(crate::utils::automix_cache_dir(), 512)
    }

    fn path_for(&self, content_id: &str, config: AnalysisConfig) -> PathBuf {
        self.root
            .join(format!("{}.json", cache_key(content_id, config)))
    }

    pub fn load(
        &self,
        content_id: &str,
        config: AnalysisConfig,
    ) -> Result<Option<TrackAnalysis>, String> {
        let path = self.path_for(content_id, config);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("read Automix cache {:?}: {error}", path)),
        };
        let analysis: TrackAnalysis = match serde_json::from_slice(&bytes) {
            Ok(analysis) => analysis,
            Err(_) => return Ok(None),
        };
        if analysis.content_id != content_id || !analysis.is_compatible(config) {
            return Ok(None);
        }
        Ok(Some(analysis))
    }

    pub fn store(&self, analysis: &TrackAnalysis, config: AnalysisConfig) -> Result<(), String> {
        if !analysis.is_compatible(config) {
            return Err("incompatible Automix analysis".to_string());
        }
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("create Automix cache {:?}: {error}", self.root))?;
        let path = self.path_for(&analysis.content_id, config);
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec(analysis)
            .map_err(|error| format!("serialize Automix analysis: {error}"))?;
        fs::write(&temp, bytes)
            .map_err(|error| format!("write Automix cache {:?}: {error}", temp))?;
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| format!("replace Automix cache {:?}: {error}", path))?;
        }
        fs::rename(&temp, &path)
            .map_err(|error| format!("finalize Automix cache {:?}: {error}", path))?;
        self.prune()?;
        Ok(())
    }

    pub fn analyze_file_if_missing<F>(
        &self,
        path: &Path,
        content_id: &str,
        config: AnalysisConfig,
        is_cancelled: F,
    ) -> Result<bool, String>
    where
        F: Fn() -> bool,
    {
        if is_cancelled() {
            return Ok(false);
        }
        if self.load(content_id, config)?.is_some() {
            return Ok(false);
        }
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("create Automix cache {:?}: {error}", self.root))?;
        let claim = self
            .path_for(content_id, config)
            .with_extension("analysis.lock");
        let claim_file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&claim)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
            Err(error) => return Err(format!("claim Automix analysis {:?}: {error}", claim)),
        };
        let result = analyze_file(path, content_id.to_string(), config).and_then(|analysis| {
            if is_cancelled() {
                Ok(false)
            } else {
                self.store(&analysis, config).map(|_| true)
            }
        });
        drop(claim_file);
        let _ = fs::remove_file(claim);
        result
    }

    fn prune(&self) -> Result<(), String> {
        let mut entries: Vec<_> = fs::read_dir(&self.root)
            .map_err(|error| format!("scan Automix cache {:?}: {error}", self.root))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort_by(|left, right| {
            modified(left.path().as_path()).cmp(&modified(right.path().as_path()))
        });
        let remove_count = entries.len().saturating_sub(self.max_entries);
        for entry in entries.into_iter().take(remove_count) {
            let _ = fs::remove_file(entry.path());
        }
        Ok(())
    }
}

fn modified(path: &Path) -> std::time::SystemTime {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::UNIX_EPOCH)
}

#[derive(Default)]
struct Fnv64(u64);

impl Hasher for Fnv64 {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.0 == 0 {
            self.0 = 0xcbf29ce484222325;
        }
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

impl Fnv64 {
    fn write_u32(&mut self, value: u32) {
        self.write(&value.to_le_bytes());
    }
}

/// Analyze a bounded in-memory sample window. This is deliberately modest:
/// it supplies stable energy/BPM hints and leaves advanced feature extraction
/// optional rather than blocking playback.
pub fn analyze_samples(
    content_id: impl Into<String>,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    duration: Duration,
    config: AnalysisConfig,
) -> TrackAnalysis {
    let max_samples = usize::try_from(config.max_seconds)
        .unwrap_or(usize::MAX)
        .saturating_mul(sample_rate as usize)
        .saturating_mul(channels.max(1) as usize);
    let window = &samples[..samples.len().min(max_samples)];
    let channel_count = channels.max(1) as usize;
    let mono: Vec<f32> = window
        .chunks(channel_count)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len().max(1) as f32)
        .collect();
    let mean_square = if mono.is_empty() {
        0.0
    } else {
        mono.iter().map(|sample| sample * sample).sum::<f32>() / mono.len() as f32
    };
    let energy = mean_square.sqrt().clamp(0.0, 1.0);
    let frame_samples = (sample_rate as usize / 50).max(1);
    let frame_energy: Vec<f32> = mono
        .chunks(frame_samples)
        .map(|frame| {
            (frame.iter().map(|sample| sample * sample).sum::<f32>() / frame.len().max(1) as f32)
                .sqrt()
        })
        .collect();
    let onset: Vec<f32> = frame_energy
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).max(0.0))
        .collect();
    let (bpm, confidence, first_beat) = estimate_tempo(&onset);
    let beat_period = bpm.map(|value| Duration::from_secs_f32(60.0 / value));
    let audible_threshold = (energy * 0.12).max(0.002);
    let cut_frame = frame_energy
        .iter()
        .position(|value| *value >= audible_threshold)
        .unwrap_or(0);
    let last_audible_frame = frame_energy
        .iter()
        .rposition(|value| *value >= audible_threshold)
        .unwrap_or_else(|| frame_energy.len().saturating_sub(1));
    let frame_duration = Duration::from_millis(20);
    let cut_start = frame_duration.saturating_mul(cut_frame as u32);
    let last_audible = frame_duration.saturating_mul(last_audible_frame as u32);
    let fade_out_start = last_audible.saturating_sub(Duration::from_secs(4));
    let exit = fade_out_start.min(duration.saturating_sub(Duration::from_millis(100)));
    let crossings = mono
        .windows(2)
        .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0))
        .count();
    let crossing_rate = crossings as f32 / mono.len().max(1) as f32;
    let vocals_confidence =
        ((crossing_rate - 0.02) / 0.18).clamp(0.0, 1.0) * (energy * 4.0).clamp(0.0, 1.0);
    TrackAnalysis {
        schema_version: config.schema_version,
        analyzer_version: ANALYZER_VERSION.to_string(),
        content_id: content_id.into(),
        duration,
        bpm,
        bpm_confidence: confidence,
        first_beat,
        beat_period,
        bar_beats: 4,
        lufs: if mean_square > 0.0 {
            Some(-0.691 + 10.0 * mean_square.log10())
        } else {
            None
        },
        energy,
        vocals_confidence,
        key: estimate_key(&mono, sample_rate, energy),
        cut_start,
        fade_out_start,
        recommended_exit: exit,
        recommended_entry: cut_start,
    }
}

fn estimate_tempo(onset: &[f32]) -> (Option<f32>, f32, Option<Duration>) {
    if onset.len() < 40 || onset.iter().copied().sum::<f32>() <= f32::EPSILON {
        return (None, 0.0, None);
    }
    let mut best_lag = 0usize;
    let mut best_score = 0.0f32;
    let normalization = onset.iter().map(|value| value * value).sum::<f32>();
    for lag in 17..=75 {
        if lag >= onset.len() {
            break;
        }
        let score = onset
            .iter()
            .skip(lag)
            .zip(onset.iter())
            .map(|(a, b)| a * b)
            .sum::<f32>();
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }
    if best_lag == 0 || best_score <= f32::EPSILON {
        return (None, 0.0, None);
    }
    let bpm = 60_000.0 / (best_lag as f32 * 20.0);
    let confidence = (best_score / normalization.max(best_score)).clamp(0.0, 1.0);
    let first = onset
        .iter()
        .take(best_lag)
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, _)| Duration::from_millis((index as u64 + 1) * 20));
    (Some(bpm.clamp(40.0, 240.0)), confidence, first)
}

fn estimate_key(samples: &[f32], sample_rate: u32, energy: f32) -> Option<String> {
    if sample_rate == 0 || samples.is_empty() || energy < 0.005 {
        return None;
    }
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let limit = samples.len().min(sample_rate as usize * 10);
    let stride = 4usize;
    let mut best = (0usize, 0.0f64);
    for pitch in 0..12 {
        let frequency = 130.812_782_65_f64 * 2.0_f64.powf(pitch as f64 / 12.0);
        let omega = std::f64::consts::TAU * frequency / sample_rate as f64;
        let mut re = 0.0f64;
        let mut im = 0.0f64;
        for (index, sample) in samples[..limit].iter().step_by(stride).enumerate() {
            let phase = omega * (index * stride) as f64;
            re += f64::from(*sample) * phase.cos();
            im -= f64::from(*sample) * phase.sin();
        }
        let magnitude = re * re + im * im;
        if magnitude > best.1 {
            best = (pitch, magnitude);
        }
    }
    (best.1 > 0.0).then(|| NAMES[best.0].to_string())
}

pub fn analyze_file(
    path: &Path,
    content_id: impl Into<String>,
    config: AnalysisConfig,
) -> Result<TrackAnalysis, String> {
    let file = fs::File::open(path).map_err(|error| format!("open {:?}: {error}", path))?;
    let byte_len = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let mut decoder = rodio::Decoder::builder()
        .with_data(BufReader::new(file))
        .with_byte_len(byte_len)
        .with_seekable(true)
        .build()
        .map_err(|error| format!("decode {:?}: {error}", path))?;
    let duration = decoder.total_duration().unwrap_or(Duration::ZERO);
    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels();
    let max_samples = usize::try_from(config.max_seconds)
        .unwrap_or(usize::MAX)
        .saturating_mul(sample_rate as usize)
        .saturating_mul(channels.max(1) as usize);
    let split_samples = max_samples / 2;
    let mut head: Vec<f32> = decoder.by_ref().take(split_samples.max(1)).collect();
    let mut tail = Vec::new();
    let analyzed_head_duration = Duration::from_secs_f64(
        head.len() as f64 / sample_rate.max(1) as f64 / channels.max(1) as f64,
    );
    if duration > analyzed_head_duration.saturating_mul(2) {
        let tail_duration = Duration::from_secs_f64(
            split_samples as f64 / sample_rate.max(1) as f64 / channels.max(1) as f64,
        );
        let tail_start = duration.saturating_sub(tail_duration);
        if decoder.try_seek(tail_start).is_ok() {
            tail = decoder.by_ref().take(split_samples.max(1)).collect();
        }
    } else {
        head.extend(
            decoder
                .by_ref()
                .take(max_samples.saturating_sub(head.len())),
        );
    }
    let mut samples = head;
    samples.extend_from_slice(&tail);
    let mut analysis = analyze_samples(
        content_id,
        &samples,
        sample_rate,
        channels,
        duration,
        config,
    );
    if !tail.is_empty() {
        let tail_frames = tail.len() / channels.max(1) as usize;
        let tail_duration = Duration::from_secs_f64(tail_frames as f64 / sample_rate.max(1) as f64);
        let tail_start = duration.saturating_sub(tail_duration);
        let tail_fade =
            detect_tail_fade_start(&tail, sample_rate, channels, analysis.energy, tail_start);
        analysis.fade_out_start = tail_fade;
        analysis.recommended_exit =
            tail_fade.min(duration.saturating_sub(Duration::from_millis(100)));
    }
    Ok(analysis)
}

fn detect_tail_fade_start(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    track_energy: f32,
    tail_start: Duration,
) -> Duration {
    let channel_count = channels.max(1) as usize;
    let mono: Vec<f32> = samples
        .chunks(channel_count)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len().max(1) as f32)
        .collect();
    let frame_samples = (sample_rate as usize / 50).max(1);
    let threshold = (track_energy * 0.12).max(0.002);
    let last_audible = mono
        .chunks(frame_samples)
        .enumerate()
        .filter_map(|(index, frame)| {
            let rms = (frame.iter().map(|sample| sample * sample).sum::<f32>()
                / frame.len().max(1) as f32)
                .sqrt();
            (rms >= threshold).then_some(index)
        })
        .next_back()
        .unwrap_or(0);
    tail_start
        .saturating_add(Duration::from_millis(last_audible as u64 * 20))
        .saturating_sub(Duration::from_secs(4))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionFallback {
    AnalysisUnavailable,
    LowConfidence,
    InvalidBounds,
    SchedulerUnderrun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    Manual,
    BaselineNatural,
    Automix,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionDirective {
    pub kind: TransitionKind,
    pub duration: Duration,
    pub entry: Duration,
    pub expected_exit: Option<Duration>,
    pub group: ScheduleGroup,
    pub automation: AdvancedAutomation,
}

impl TransitionDirective {
    pub fn manual(group: ScheduleGroup) -> Self {
        Self {
            kind: TransitionKind::Manual,
            duration: Duration::from_millis(MANUAL_CROSSFADE_MS.into()),
            entry: Duration::ZERO,
            expected_exit: None,
            group,
            automation: AdvancedAutomation::default(),
        }
    }

    pub fn baseline_natural(group: ScheduleGroup) -> Self {
        Self {
            kind: TransitionKind::BaselineNatural,
            duration: Duration::from_millis(BASELINE_CROSSFADE_MS.into()),
            entry: Duration::ZERO,
            expected_exit: None,
            group,
            automation: AdvancedAutomation::default(),
        }
    }

    pub fn automix(
        group: ScheduleGroup,
        plan: &TransitionPlan,
        automation: AdvancedAutomation,
    ) -> Self {
        Self {
            kind: TransitionKind::Automix,
            duration: plan.duration,
            entry: plan.entry,
            expected_exit: Some(plan.exit),
            group,
            automation,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionPlan {
    pub exit: Duration,
    pub entry: Duration,
    pub duration: Duration,
    pub beat_aligned: bool,
    pub aggressive_outro: bool,
    pub fallback: Option<TransitionFallback>,
}

impl TransitionPlan {
    pub fn baseline(current: &TrackAnalysis, next: &TrackAnalysis, duration: Duration) -> Self {
        let max_duration = current.duration.min(next.duration).min(duration);
        Self {
            exit: current.duration.saturating_sub(max_duration),
            entry: Duration::ZERO,
            duration: max_duration,
            beat_aligned: false,
            aggressive_outro: false,
            fallback: None,
        }
    }
}

pub fn plan_transition(
    current: &TrackAnalysis,
    next: &TrackAnalysis,
    default_duration: Duration,
) -> Result<TransitionPlan, TransitionFallback> {
    if current.duration.is_zero() || next.duration.is_zero() {
        return Err(TransitionFallback::InvalidBounds);
    }
    if current.bpm_confidence < 0.6 || next.bpm_confidence < 0.6 {
        return Err(TransitionFallback::LowConfidence);
    }
    let mut duration = default_duration
        .min(current.duration)
        .min(next.duration)
        .max(Duration::from_millis(100));
    let mut exit = current
        .recommended_exit
        .min(current.duration.saturating_sub(duration));
    let mut entry = next
        .recommended_entry
        .min(next.duration.saturating_sub(duration));
    let mut beat_aligned = false;
    if let (Some(current_period), Some(current_first), Some(next_period), Some(next_first)) = (
        current.beat_period,
        current.first_beat,
        next.beat_period,
        next.first_beat,
    ) && current_period > Duration::ZERO
        && next_period > Duration::ZERO
    {
        exit = snap_to_beat(exit, current_first, current_period)
            .min(current.duration.saturating_sub(duration));
        entry = snap_to_beat(entry, next_first, next_period)
            .min(next.duration.saturating_sub(duration));
        duration = duration.min(current.duration.saturating_sub(exit));
        beat_aligned = true;
    }
    if duration < Duration::from_millis(100) {
        return Err(TransitionFallback::InvalidBounds);
    }
    let aggressive_outro = current.vocals_confidence < 0.3 && current.energy < 0.45;
    if aggressive_outro {
        let advance = current
            .beat_period
            .unwrap_or(Duration::from_millis(500))
            .saturating_mul(2);
        exit = exit.saturating_sub(advance);
        duration = duration
            .min(Duration::from_secs(3))
            .max(Duration::from_millis(300));
    }
    Ok(TransitionPlan {
        exit,
        entry,
        duration,
        beat_aligned,
        aggressive_outro,
        fallback: None,
    })
}

fn snap_to_beat(value: Duration, first: Duration, period: Duration) -> Duration {
    if value <= first {
        return first;
    }
    let steps = value.saturating_sub(first).as_secs_f64() / period.as_secs_f64();
    first + period.mul_f64(steps.round().max(0.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScheduleGroup(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledAction {
    pub group: ScheduleGroup,
    pub at: Duration,
    pub underrun: bool,
}

#[derive(Debug, Default)]
pub struct AudioClockScheduler {
    horizon: Duration,
    actions: VecDeque<ScheduledAction>,
}

impl AudioClockScheduler {
    pub fn new(horizon: Duration) -> Self {
        Self {
            horizon,
            ..Self::default()
        }
    }

    pub fn schedule(&mut self, group: ScheduleGroup, at: Duration) {
        self.actions.retain(|action| action.group != group);
        self.actions.push_back(ScheduledAction {
            group,
            at,
            underrun: false,
        });
    }

    pub fn cancel(&mut self, group: ScheduleGroup) {
        self.actions.retain(|action| action.group != group);
    }

    pub fn poll(&mut self, now: Duration) -> Option<ScheduledAction> {
        let action = self.actions.front()?;
        if action.at > now {
            return None;
        }
        let mut action = self.actions.pop_front()?;
        action.underrun = now > action.at.saturating_add(self.horizon);
        Some(action)
    }
}

pub const SCHEDULER_HORIZON_MS: u64 = 1_500;
pub const SCHEDULER_POLL_MS: u64 = 75;
pub const AUTOMIX_PLANNING_WINDOW_SECS: u64 = 45;

pub fn equal_power_gains(progress: f32) -> (f32, f32) {
    let p = progress.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (p.cos(), p.sin())
}

pub fn clamp_rate_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(0.97, 1.03)
    } else {
        1.0
    }
}

pub fn loudness_gain_db(delta_db: f32) -> f32 {
    if delta_db.is_finite() {
        delta_db.clamp(-9.0, 9.0)
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdvancedAutomation {
    pub rate: f32,
    pub gain_db: f32,
    pub bass_swap: bool,
}

impl Default for AdvancedAutomation {
    fn default() -> Self {
        Self {
            rate: 1.0,
            gain_db: 0.0,
            bass_swap: false,
        }
    }
}

pub fn automation_for_transition(
    current: &TrackAnalysis,
    next: &TrackAnalysis,
) -> AdvancedAutomation {
    let rate = match (current.bpm, next.bpm) {
        (Some(a), Some(b))
            if a > 0.0
                && b > 0.0
                && current.bpm_confidence >= 0.4
                && next.bpm_confidence >= 0.4
                && (0.97..=1.03).contains(&(a / b)) =>
        {
            a / b
        }
        _ => 1.0,
    };
    let gain_db = match (current.lufs, next.lufs) {
        (Some(a), Some(b)) => loudness_gain_db(a - b),
        _ => 0.0,
    };
    AdvancedAutomation {
        rate,
        gain_db,
        bass_swap: current.energy > 0.2
            && next.energy > 0.2
            && matches!((current.bpm, next.bpm), (Some(a), Some(b)) if a > 0.0 && ((a - b).abs() / a) < 0.06),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable_and_versioned() {
        let config = AnalysisConfig::default();
        assert_eq!(cache_key("song-1", config), cache_key("song-1", config));
        assert_ne!(cache_key("song-1", config), cache_key("song-2", config));
    }

    #[test]
    fn cache_round_trip_and_corruption_are_non_fatal() {
        let root = std::env::temp_dir().join(format!(
            "rustle-automix-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = AnalysisCache::new(&root, 2);
        let config = AnalysisConfig::default();
        let analysis = TrackAnalysis::fallback("song", Duration::from_secs(30));
        cache.store(&analysis, config).unwrap();
        assert_eq!(cache.load("song", config).unwrap(), Some(analysis));

        let path = cache.path_for("song", config);
        fs::write(&path, b"not-json").unwrap();
        assert_eq!(cache.load("song", config).unwrap(), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn analysis_is_bounded_and_has_fallback_for_silence() {
        let config = AnalysisConfig {
            max_seconds: 1,
            ..AnalysisConfig::default()
        };
        let analysis = analyze_samples(
            "silence",
            &[0.0; 48_000 * 2],
            48_000,
            1,
            Duration::from_secs(10),
            config,
        );
        assert_eq!(analysis.energy, 0.0);
        assert_eq!(analysis.bpm_confidence, 0.0);
        assert!(analysis.is_compatible(config));
    }

    #[test]
    fn periodic_onsets_produce_usable_bpm_confidence() {
        let sample_rate = 1_000;
        let mut samples = vec![0.0; sample_rate as usize * 10];
        for beat in (0..samples.len()).step_by(500) {
            for sample in samples.iter_mut().skip(beat).take(20) {
                *sample = 0.8;
            }
        }
        let analysis = analyze_samples(
            "click",
            &samples,
            sample_rate,
            1,
            Duration::from_secs(10),
            AnalysisConfig::default(),
        );
        assert!(analysis.bpm.is_some_and(|bpm| (bpm - 120.0).abs() < 1.0));
        assert!(analysis.bpm_confidence >= 0.6);
    }

    #[test]
    fn tail_fade_detection_uses_absolute_track_time() {
        let mut tail = vec![0.4; 5_000];
        tail.extend(vec![0.0; 5_000]);
        let fade = detect_tail_fade_start(&tail, 1_000, 1, 0.4, Duration::from_secs(90));
        assert!(fade >= Duration::from_secs(90));
        assert!(fade < Duration::from_secs(96));
    }

    #[test]
    fn planner_requires_confidence_and_bounds_duration() {
        let mut current = TrackAnalysis::fallback("a", Duration::from_secs(2));
        let next = TrackAnalysis::fallback("b", Duration::from_secs(3));
        assert_eq!(
            plan_transition(&current, &next, Duration::from_secs(5)),
            Err(TransitionFallback::LowConfidence)
        );
        current.bpm = Some(120.0);
        current.bpm_confidence = 0.9;
        current.beat_period = Some(Duration::from_millis(500));
        current.first_beat = Some(Duration::from_millis(100));
        let mut next = next;
        next.bpm = Some(120.0);
        next.bpm_confidence = 0.9;
        next.beat_period = Some(Duration::from_millis(500));
        next.first_beat = Some(Duration::from_millis(100));
        let plan = plan_transition(&current, &next, Duration::from_secs(5)).unwrap();
        assert!(plan.duration <= Duration::from_secs(2));
        assert!(plan.beat_aligned);
    }

    #[test]
    fn equal_power_and_advanced_automation_are_clamped() {
        let (outgoing, incoming) = equal_power_gains(0.5);
        assert!((outgoing * outgoing + incoming * incoming - 1.0).abs() < 1e-5);
        assert_eq!(clamp_rate_ratio(f32::NAN), 1.0);
        assert_eq!(clamp_rate_ratio(2.0), 1.03);
        assert_eq!(loudness_gain_db(-20.0), -9.0);
        let mut current = TrackAnalysis::fallback("a", Duration::from_secs(30));
        let mut next = TrackAnalysis::fallback("b", Duration::from_secs(30));
        current.bpm = Some(100.0);
        next.bpm = Some(140.0);
        current.bpm_confidence = 0.9;
        next.bpm_confidence = 0.9;
        assert_eq!(automation_for_transition(&current, &next).rate, 1.0);
    }

    #[test]
    fn scheduler_cancels_stale_groups() {
        let mut scheduler = AudioClockScheduler::new(Duration::from_secs(1));
        let group = ScheduleGroup(1);
        scheduler.schedule(group, Duration::from_secs(1));
        scheduler.cancel(group);
        assert!(scheduler.poll(Duration::from_secs(2)).is_none());
    }

    #[test]
    fn scheduler_uses_audio_deadline_and_marks_underruns() {
        let mut scheduler = AudioClockScheduler::new(Duration::from_millis(1_500));
        let on_time = ScheduleGroup(1);
        scheduler.schedule(on_time, Duration::from_secs(10));
        assert!(scheduler.poll(Duration::from_millis(9_999)).is_none());
        let action = scheduler.poll(Duration::from_millis(10_075)).unwrap();
        assert_eq!(action.group, on_time);
        assert!(!action.underrun);

        let late = ScheduleGroup(2);
        scheduler.schedule(late, Duration::from_secs(20));
        let action = scheduler.poll(Duration::from_millis(21_501)).unwrap();
        assert_eq!(action.group, late);
        assert!(action.underrun);
    }

    #[test]
    fn aggressive_outro_advances_quiet_non_vocal_exit() {
        let mut current = TrackAnalysis::fallback("a", Duration::from_secs(30));
        let mut next = TrackAnalysis::fallback("b", Duration::from_secs(30));
        for analysis in [&mut current, &mut next] {
            analysis.bpm = Some(120.0);
            analysis.bpm_confidence = 0.9;
            analysis.first_beat = Some(Duration::ZERO);
            analysis.beat_period = Some(Duration::from_millis(500));
            analysis.energy = 0.8;
        }
        current.energy = 0.2;
        current.vocals_confidence = 0.1;
        let plan = plan_transition(&current, &next, Duration::from_secs(5)).unwrap();
        assert!(plan.aggressive_outro);
        assert!(plan.duration <= Duration::from_secs(3));
        assert!(plan.exit < current.recommended_exit);
    }

    #[test]
    fn transition_directives_keep_manual_and_natural_policies_separate() {
        let group = ScheduleGroup(7);
        assert_eq!(
            TransitionDirective::manual(group).duration,
            Duration::from_millis(MANUAL_CROSSFADE_MS.into())
        );
        assert_eq!(
            TransitionDirective::baseline_natural(group).duration,
            Duration::from_millis(BASELINE_CROSSFADE_MS.into())
        );
    }
}
