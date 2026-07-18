//! SST `backend/core/vad.py` — WebRTC VAD + energy gate + adaptive ambient floor.

use std::collections::VecDeque;

use webrtc_vad::{SampleRate, Vad, VadMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadSegmentKind {
    Partial,
    Final,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VadSegment {
    pub kind: VadSegmentKind,
    pub audio: Vec<f32>,
    pub duration_ms: u32,
    pub voiced_ratio: f32,
    pub average_rms: f32,
}

#[derive(Debug, Clone)]
pub struct VadEngineConfig {
    pub sample_rate: u32,
    pub frame_duration_ms: u32,
    pub mode: u8,
    pub silence_hold_ms: u32,
    pub finalization_hold_ms: u32,
    pub min_speech_ms: u32,
    pub partial_emit_interval_ms: u32,
    pub max_segment_ms: u32,
    pub energy_gate_enabled: bool,
    pub min_rms_for_recognition: f32,
    pub min_voiced_ratio: f32,
    pub first_partial_min_speech_ms: u32,
    pub speech_attack_frames: u32,
    pub speech_preroll_frames: u32,
}

impl Default for VadEngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            frame_duration_ms: 30,
            mode: 2,
            silence_hold_ms: 180,
            finalization_hold_ms: 350,
            min_speech_ms: 180,
            partial_emit_interval_ms: 280,
            max_segment_ms: 5_500,
            energy_gate_enabled: false,
            min_rms_for_recognition: 0.0018,
            min_voiced_ratio: 0.0,
            first_partial_min_speech_ms: 180,
            speech_attack_frames: 2,
            speech_preroll_frames: 5,
        }
    }
}

pub struct VadEngine {
    config: VadEngineConfig,
    vad: Vad,
    frame_bytes: usize,
    pending_audio: Vec<u8>,
    speech_frames: Vec<Vec<u8>>,
    speech_rms_values: Vec<f32>,
    ambient_rms_values: VecDeque<f32>,
    silence_frames: u32,
    last_partial_frame_count: usize,
    segment_total_frames: u32,
    segment_voiced_frames: u32,
    segment_dropped_count: u64,
    ambient_rms_ema: f32,
    preroll: VecDeque<(Vec<u8>, f32)>,
    pending_attack: Vec<(Vec<u8>, f32)>,
    silence_hold_frames: u32,
    finalization_hold_frames: u32,
    min_speech_frames: u32,
    first_partial_min_speech_frames: u32,
    partial_interval_frames: u32,
    max_segment_frames: u32,
    speech_attack_frames: u32,
}

impl VadEngine {
    pub fn new(config: VadEngineConfig) -> Self {
        let sample_rate = match config.sample_rate {
            8_000 => SampleRate::Rate8kHz,
            32_000 => SampleRate::Rate32kHz,
            48_000 => SampleRate::Rate48kHz,
            _ => SampleRate::Rate16kHz,
        };
        let mode = vad_mode_from_u8(config.mode);
        let vad = Vad::new_with_rate_and_mode(sample_rate, mode);
        let frame_bytes =
            ((config.sample_rate as u64 * config.frame_duration_ms as u64 / 1000) * 2) as usize;
        let mut engine = Self {
            config,
            vad,
            frame_bytes,
            pending_audio: Vec::new(),
            speech_frames: Vec::new(),
            speech_rms_values: Vec::new(),
            ambient_rms_values: VecDeque::with_capacity(64),
            silence_frames: 0,
            last_partial_frame_count: 0,
            segment_total_frames: 0,
            segment_voiced_frames: 0,
            segment_dropped_count: 0,
            ambient_rms_ema: 0.0,
            preroll: VecDeque::new(),
            pending_attack: Vec::new(),
            silence_hold_frames: 1,
            finalization_hold_frames: 1,
            min_speech_frames: 0,
            first_partial_min_speech_frames: 0,
            partial_interval_frames: 1,
            max_segment_frames: 1,
            speech_attack_frames: 2,
        };
        engine.apply_config(engine.config.clone());
        engine
    }

    pub fn configure(&mut self, config: VadEngineConfig) {
        self.apply_config(config);
    }

    pub fn segment_dropped_count(&self) -> u64 {
        self.segment_dropped_count
    }

    pub fn in_speech(&self) -> bool {
        !self.speech_frames.is_empty()
    }

    pub fn reset(&mut self) {
        self.pending_audio.clear();
        self.speech_frames.clear();
        self.speech_rms_values.clear();
        self.silence_frames = 0;
        self.last_partial_frame_count = 0;
        self.segment_total_frames = 0;
        self.segment_voiced_frames = 0;
        self.pending_attack.clear();
        self.preroll.clear();
    }

    pub fn process_chunk(&mut self, audio_chunk: &[u8]) -> Vec<VadSegment> {
        if audio_chunk.is_empty() {
            return Vec::new();
        }

        let mut segments = Vec::new();
        self.pending_audio.extend_from_slice(audio_chunk);
        let total_len = self.pending_audio.len();
        if total_len < self.frame_bytes {
            return segments;
        }
        let usable = total_len - (total_len % self.frame_bytes);
        let chunk = self.pending_audio[..usable].to_vec();

        for start in (0..usable).step_by(self.frame_bytes) {
            if start + self.frame_bytes > chunk.len() {
                break;
            }
            let frame = chunk[start..start + self.frame_bytes].to_vec();
            let frame_rms = frame_rms(&frame);
            let is_speech = self.is_webrtc_speech(&frame);
            let mut admitted_speech = is_speech
                && (!self.config.energy_gate_enabled
                    || frame_rms >= self.config.min_rms_for_recognition);
            if admitted_speech && self.last_partial_frame_count == 0 {
                let adaptive_threshold = self.adaptive_pre_partial_rms_threshold();
                if adaptive_threshold > 0.0 && frame_rms < adaptive_threshold {
                    admitted_speech = false;
                }
            }

            if admitted_speech {
                if self.speech_frames.is_empty() {
                    self.pending_attack.push((frame, frame_rms));
                    if self.pending_attack.len() < self.speech_attack_frames as usize {
                        continue;
                    }
                    self.flush_speech_onset_from_preroll_and_attack();
                    self.try_grow_partial_or_finalize_max_segment(&mut segments);
                } else {
                    self.append_voiced_frame(frame, frame_rms, &mut segments);
                }
            } else if !self.speech_frames.is_empty() {
                self.remember_ambient_rms(frame_rms);
                self.segment_total_frames += 1;
                self.silence_frames += 1;
                if self.silence_frames >= self.silence_hold_frames
                    && self.speech_frames.len() >= self.min_speech_frames as usize
                    && self.last_partial_frame_count != self.speech_frames.len()
                    && let Some(partial) = self.build_segment(VadSegmentKind::Partial)
                {
                    segments.push(partial);
                    self.last_partial_frame_count = self.speech_frames.len();
                }
                if self.silence_frames >= self.finalization_hold_frames {
                    if let Some(final_segment) = self.build_segment(VadSegmentKind::Final) {
                        segments.push(final_segment);
                    } else {
                        self.segment_dropped_count += 1;
                    }
                    self.reset();
                }
            } else {
                self.pending_attack.clear();
                self.remember_ambient_rms(frame_rms);
                if self.config.speech_preroll_frames > 0 {
                    self.preroll.push_back((frame, frame_rms));
                    while self.preroll.len() > self.config.speech_preroll_frames as usize {
                        self.preroll.pop_front();
                    }
                }
            }
        }

        if usable > 0 && self.pending_audio.len() >= usable {
            self.pending_audio.drain(..usable);
        }
        segments
    }

    pub fn force_finalize(&mut self) -> Vec<VadSegment> {
        let mut segments = Vec::new();
        if self.speech_frames.is_empty() {
            return segments;
        }
        if let Some(final_segment) = self.build_segment(VadSegmentKind::Final) {
            segments.push(final_segment);
        }
        self.reset();
        segments
    }

    fn apply_config(&mut self, config: VadEngineConfig) {
        let mode = vad_mode_from_u8(config.mode);
        self.vad.set_mode(mode);
        self.config = config;
        self.speech_attack_frames = self.config.speech_attack_frames.max(1);
        self.silence_hold_frames = ceil_frames(
            self.config
                .silence_hold_ms
                .max(self.config.frame_duration_ms),
            self.config.frame_duration_ms,
        )
        .max(1);
        self.finalization_hold_frames = ceil_frames(
            self.config
                .finalization_hold_ms
                .max(self.config.silence_hold_ms),
            self.config.frame_duration_ms,
        )
        .max(1);
        self.min_speech_frames =
            ceil_frames(self.config.min_speech_ms, self.config.frame_duration_ms);
        self.first_partial_min_speech_frames = ceil_frames(
            self.config
                .first_partial_min_speech_ms
                .max(self.config.min_speech_ms),
            self.config.frame_duration_ms,
        );
        self.partial_interval_frames = ceil_frames(
            self.config
                .partial_emit_interval_ms
                .max(self.config.frame_duration_ms),
            self.config.frame_duration_ms,
        )
        .max(1);
        self.max_segment_frames = ceil_frames(
            self.config
                .max_segment_ms
                .max(self.config.frame_duration_ms),
            self.config.frame_duration_ms,
        )
        .max(1);
        let pr_n = self.config.speech_preroll_frames;
        while self.preroll.len() > pr_n as usize {
            self.preroll.pop_front();
        }
    }

    fn is_webrtc_speech(&mut self, frame: &[u8]) -> bool {
        if frame.len() != self.frame_bytes {
            return false;
        }
        let mut samples = [0i16; 960];
        let count = frame.len() / 2;
        if count > samples.len() {
            return false;
        }
        for (idx, chunk) in frame.chunks_exact(2).enumerate() {
            samples[idx] = i16::from_le_bytes([chunk[0], chunk[1]]);
        }
        self.vad
            .is_voice_segment(&samples[..count])
            .unwrap_or(false)
    }

    fn flush_speech_onset_from_preroll_and_attack(&mut self) {
        for (frame, rms) in self.preroll.drain(..) {
            self.speech_frames.push(frame);
            self.speech_rms_values.push(rms);
            self.segment_total_frames += 1;
        }
        for (frame, rms) in self.pending_attack.drain(..) {
            self.speech_frames.push(frame);
            self.speech_rms_values.push(rms);
            self.segment_total_frames += 1;
            self.segment_voiced_frames += 1;
        }
        self.silence_frames = 0;
    }

    fn append_voiced_frame(
        &mut self,
        frame: Vec<u8>,
        frame_rms: f32,
        segments: &mut Vec<VadSegment>,
    ) {
        self.speech_frames.push(frame);
        self.speech_rms_values.push(frame_rms);
        self.silence_frames = 0;
        self.segment_total_frames += 1;
        self.segment_voiced_frames += 1;
        self.try_grow_partial_or_finalize_max_segment(segments);
    }

    fn try_grow_partial_or_finalize_max_segment(&mut self, segments: &mut Vec<VadSegment>) {
        let frames_since_last_partial = self
            .speech_frames
            .len()
            .saturating_sub(self.last_partial_frame_count);
        let target_partial_frames = if self.last_partial_frame_count == 0 {
            self.first_partial_min_speech_frames as usize
        } else {
            self.partial_interval_frames as usize
        };
        if self.speech_frames.len() >= self.min_speech_frames as usize
            && frames_since_last_partial >= target_partial_frames
            && let Some(partial) = self.build_segment(VadSegmentKind::Partial)
        {
            segments.push(partial);
            self.last_partial_frame_count = self.speech_frames.len();
        }
        if self.speech_frames.len() >= self.max_segment_frames as usize {
            if let Some(final_segment) = self.build_segment(VadSegmentKind::Final) {
                segments.push(final_segment);
            } else {
                self.segment_dropped_count += 1;
            }
            self.reset();
        }
    }

    fn build_segment(&self, kind: VadSegmentKind) -> Option<VadSegment> {
        if self.speech_frames.is_empty() {
            return None;
        }
        if !self.segment_passes_admission(kind == VadSegmentKind::Partial) {
            return None;
        }
        let audio = pcm_bytes_to_f32(
            &self
                .speech_frames
                .iter()
                .flat_map(|frame| frame.iter().copied())
                .collect::<Vec<_>>(),
        );
        Some(VadSegment {
            kind,
            audio,
            duration_ms: self.duration_ms(self.speech_frames.len()),
            voiced_ratio: self.segment_voiced_ratio(),
            average_rms: self.segment_average_rms(),
        })
    }

    fn segment_passes_admission(&self, for_partial: bool) -> bool {
        if self.speech_frames.len() < self.min_speech_frames as usize {
            return false;
        }
        if for_partial
            && self.last_partial_frame_count == 0
            && self.speech_frames.len() < self.first_partial_min_speech_frames as usize
        {
            return false;
        }
        if self.config.min_voiced_ratio > 0.0
            && self.segment_total_frames > 0
            && self.segment_voiced_ratio() < self.config.min_voiced_ratio
        {
            return false;
        }
        true
    }

    fn duration_ms(&self, frame_count: usize) -> u32 {
        frame_count as u32 * self.config.frame_duration_ms
    }

    fn segment_voiced_ratio(&self) -> f32 {
        if self.segment_total_frames == 0 {
            return 0.0;
        }
        self.segment_voiced_frames as f32 / self.segment_total_frames as f32
    }

    fn segment_average_rms(&self) -> f32 {
        if self.speech_rms_values.is_empty() {
            return 0.0;
        }
        self.speech_rms_values.iter().sum::<f32>() / self.speech_rms_values.len() as f32
    }

    fn remember_ambient_rms(&mut self, frame_rms: f32) {
        if frame_rms <= 0.0 {
            return;
        }
        if self.ambient_rms_values.len() >= 64 {
            self.ambient_rms_values.pop_front();
        }
        self.ambient_rms_values.push_back(frame_rms);
        if self.ambient_rms_ema <= 0.0 {
            self.ambient_rms_ema = frame_rms;
        } else {
            self.ambient_rms_ema = 0.88 * self.ambient_rms_ema + 0.12 * frame_rms;
        }
    }

    fn ambient_rms_floor(&self) -> f32 {
        let median = median(
            self.ambient_rms_values
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let ema = self.ambient_rms_ema;
        if median > 0.0 && ema > 0.0 {
            return median.max(ema * 0.98);
        }
        if median > 0.0 {
            return median;
        }
        if ema > 0.0 { ema } else { 0.0 }
    }

    fn adaptive_pre_partial_rms_threshold(&self) -> f32 {
        let ambient_floor = self.ambient_rms_floor();
        let static_floor = if self.config.energy_gate_enabled {
            self.config.min_rms_for_recognition
        } else {
            0.0
        };
        if ambient_floor <= 0.0 {
            return static_floor;
        }
        let multiplier = if self.config.mode <= 1 { 2.15 } else { 1.85 };
        let padding = if self.config.mode <= 1 {
            0.00065
        } else {
            0.00045
        };
        static_floor
            .max(ambient_floor * multiplier)
            .max(ambient_floor + padding)
            .min(0.0028)
    }
}

pub fn f32_to_pcm_bytes(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        out.extend_from_slice(&((clamped * 32767.0) as i16).to_le_bytes());
    }
    out
}

pub fn pcm_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
        .collect()
}

pub fn frame_rms(frame: &[u8]) -> f32 {
    let samples = pcm_bytes_to_f32(frame);
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32 + 1e-12).sqrt()
}

fn ceil_frames(ms: u32, frame_duration_ms: u32) -> u32 {
    if frame_duration_ms == 0 {
        return 0;
    }
    ms.div_ceil(frame_duration_ms)
}

fn median(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted[sorted.len() / 2]
}

fn vad_mode_from_u8(mode: u8) -> VadMode {
    match mode {
        0 => VadMode::Quality,
        1 => VadMode::LowBitrate,
        3 => VadMode::VeryAggressive,
        _ => VadMode::Aggressive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_finalize_after_speech_chunks() {
        let mut vad = VadEngine::new(VadEngineConfig {
            speech_attack_frames: 1,
            speech_preroll_frames: 0,
            energy_gate_enabled: false,
            ..VadEngineConfig::default()
        });
        let frame = sine_pcm(300.0, 480);
        for _ in 0..12 {
            let _ = vad.process_chunk(&frame);
        }
        let finals = vad.force_finalize();
        assert!(!finals.is_empty());
        assert_eq!(finals[0].kind, VadSegmentKind::Final);
    }

    fn sine_pcm(freq: f32, samples: usize) -> Vec<u8> {
        use std::f32::consts::PI;
        let wave = (0..samples)
            .map(|idx| (2.0 * PI * freq * idx as f32 / 16_000.0).sin() * 0.35)
            .collect::<Vec<_>>();
        f32_to_pcm_bytes(&wave)
    }

    #[test]
    fn silence_produces_no_segments() {
        let mut vad = VadEngine::new(VadEngineConfig::default());
        let silence = vec![0u8; 960];
        for _ in 0..50 {
            assert!(vad.process_chunk(&silence).is_empty());
        }
    }
}
