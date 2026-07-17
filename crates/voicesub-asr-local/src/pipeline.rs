//! `stream-sub-translator` `local_asr_pipeline.py` parity — WebRTC VAD segments → ASR queue → decode worker.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::decode::{prepare_decode_window, DecodePass};
use crate::decode_pacing::{
    adaptive_partial_decode_interval_ms, DecodePacer, LastDecodeMs,
};
use crate::decode_timing::DecodeTimingBreakdown;
use crate::emit_policy::{dedupe_repeated_transcript, prefer_final_text};
use parakeet_rs::Transcriber;
use tracing::{debug, info};
use voicesub_partial_emit::{
    normalize_transcript_text, should_emit_partial, PartialEmitInput,
};

use crate::asr_segment_queue::{AsrSegmentQueue, AsrWorkItem, AsrWorkKind, RuntimeGeneration};
use crate::config::LocalAsrConfig;
use crate::hallucination_filter::HallucinationFilter;
use crate::inference::{InferenceEngine, InferenceError};
use crate::realtime_settings::ResolvedRealtimeSettings;
use crate::recognition_processing::RecognitionProcessor;
use crate::segment_enqueue::{clear_segment_audio_enqueue_state, slice_segment_audio_delta};
use crate::segment_state::SegmentStateController;
use crate::vad_engine::{f32_to_pcm_bytes, VadEngine, VadSegmentKind};
use crate::vad_tuning::vad_engine_from_config;

pub const SAMPLE_RATE: u32 = crate::capture::PARAKEET_SAMPLE_RATE;

#[derive(Debug, Clone, PartialEq)]
pub struct PipelineEmit {
    pub segment_id: String,
    pub revision: u64,
    pub text: String,
    pub is_final: bool,
    pub is_speech: bool,
}

pub struct RealtimePipeline {
    settings: ResolvedRealtimeSettings,
    vad: VadEngine,
    vad_enabled: bool,
    segment_state: SegmentStateController,
    segment_queue: Arc<AsrSegmentQueue>,
    runtime_generation: Arc<RuntimeGeneration>,
    queued_sample_len: HashMap<String, usize>,
    recognition: RecognitionProcessor,
    hallucination: HallucinationFilter,
    disabled_vad_audio: Vec<f32>,
    disabled_segment_id: String,
    decode_pacer: DecodePacer,
    last_decode_ms: Arc<LastDecodeMs>,
}

impl RealtimePipeline {
    pub fn new(
        config: &LocalAsrConfig,
        segment_queue: Arc<AsrSegmentQueue>,
        runtime_generation: Arc<RuntimeGeneration>,
    ) -> Self {
        Self::new_with_decode_feedback(
            config,
            segment_queue,
            runtime_generation,
            LastDecodeMs::new(),
        )
    }

    pub fn new_with_decode_feedback(
        config: &LocalAsrConfig,
        segment_queue: Arc<AsrSegmentQueue>,
        runtime_generation: Arc<RuntimeGeneration>,
        last_decode_ms: Arc<LastDecodeMs>,
    ) -> Self {
        let settings = ResolvedRealtimeSettings::from_config(config);
        Self {
            recognition: RecognitionProcessor::from(&config.recognition),
            hallucination: HallucinationFilter::new((&config.recognition).into()),
            segment_state: SegmentStateController::default(),
            vad: vad_engine_from_config(config, &settings),
            vad_enabled: config.vad.enabled,
            segment_queue,
            runtime_generation,
            queued_sample_len: HashMap::new(),
            settings,
            disabled_vad_audio: Vec::new(),
            disabled_segment_id: String::new(),
            decode_pacer: DecodePacer::default(),
            last_decode_ms,
        }
    }

    pub fn last_decode_ms_handle(&self) -> Arc<LastDecodeMs> {
        Arc::clone(&self.last_decode_ms)
    }

    pub fn chunk_window_ms(&self) -> u32 {
        self.settings.window_ms
    }

    pub fn push_samples(&mut self, samples: &[f32]) -> Result<(), InferenceError> {
        if samples.is_empty() {
            return Ok(());
        }
        if self.vad_enabled {
            self.push_samples_vad_enabled(samples)
        } else {
            self.push_samples_vad_disabled(samples);
            Ok(())
        }
    }

    pub fn process_decode_result(
        &mut self,
        item: AsrWorkItem,
        text: Option<String>,
    ) -> Vec<PipelineEmit> {
        if item.generation != self.runtime_generation.current() {
            return Vec::new();
        }
        let speech_active = item.kind == AsrWorkKind::Partial;
        let is_final = item.kind == AsrWorkKind::Final;

        let Some(raw) = text.filter(|value| !normalize_transcript_text(value).is_empty()) else {
            if is_final {
                self.clear_partial_tracking(&item.segment_id);
            }
            return Vec::new();
        };
        let mut text = dedupe_repeated_transcript(&raw);
        if is_final {
            let last = self.segment_state.get_last_partial_text(&item.segment_id);
            text = prefer_final_text(Some(last).filter(|s| !s.is_empty()), &text);
        }

        if !self.hallucination.accept_transcript(
            &text,
            item.duration_ms,
            is_final,
            speech_active || self.vad.in_speech(),
        ) {
            if is_final {
                self.clear_partial_tracking(&item.segment_id);
            }
            return Vec::new();
        }

        if !is_final && !self.should_emit_partial(&item.segment_id, &text) {
            return Vec::new();
        }

        if !is_final {
            self.segment_state
                .mark_partial_emitted(&item.segment_id, &text);
        } else {
            self.clear_partial_tracking(&item.segment_id);
        }

        vec![self.build_emit(
            &item.segment_id,
            item.revision,
            text,
            is_final,
            speech_active,
        )]
    }

    pub(crate) fn invalidate_runtime_generation(&mut self) {
        let _ = self.runtime_generation.bump();
        self.segment_queue.clear();
        self.queued_sample_len.clear();
        self.disabled_vad_audio.clear();
        self.vad.reset();
    }

    pub fn finalize_on_stop(&mut self, inference: &InferenceEngine) -> Result<Vec<PipelineEmit>, InferenceError> {
        let mut emits = Vec::new();
        if self.vad_enabled {
            for segment in self.vad.force_finalize() {
                self.enqueue_vad_capture_segment(segment, 0.0);
            }
        } else if !self.disabled_vad_audio.is_empty() {
            self.enqueue_disabled_final();
        }

        if !self.settings.streaming_decode
            && !self.vad_enabled
            && let Some(text) = self.decode_buffer_sync(inference, &self.disabled_vad_audio)?
        {
            let (segment_id, revision, _, _) = self.segment_state.assign_segment_tracking(None);
            emits.push(self.build_emit(&segment_id, revision, text, true, false));
        }

        Ok(emits)
    }

    fn push_samples_vad_enabled(&mut self, samples: &[f32]) -> Result<(), InferenceError> {
        let started = std::time::Instant::now();
        let pcm = f32_to_pcm_bytes(samples);
        let segments = self.vad.process_chunk(&pcm);
        let vad_ms = started.elapsed().as_secs_f32() * 1000.0;

        for segment in segments {
            self.enqueue_vad_capture_segment(segment, vad_ms);
        }

        Ok(())
    }

    fn push_samples_vad_disabled(&mut self, samples: &[f32]) {
        self.disabled_vad_audio.extend_from_slice(samples);
        if self.disabled_segment_id.is_empty() {
            let (segment_id, _, _, _) = self.segment_state.assign_segment_tracking(None);
            self.disabled_segment_id = segment_id;
        }
    }

    /// `stream-sub-translator` `local_asr_pipeline.run_capture_loop` — enqueue every VAD segment.
    ///
    /// SST parity for `streaming_decode`:
    /// - **ON:** enqueue PCM **delta** only (`slice_segment_audio_delta`); worker rebuilds
    ///   cumulative audio. Parakeet TDT still re-transcribes the full cumulative buffer
    ///   (no native streaming decoder state in ONNX).
    /// - **OFF:** enqueue the **full** VAD buffer on every partial and still emit partials
    ///   (batch re-recognize whole phrase). Must NOT skip partials — that diverges from SST.
    fn enqueue_vad_capture_segment(&mut self, segment: crate::vad_engine::VadSegment, vad_ms: f32) {
        let is_final = segment.kind == VadSegmentKind::Final;

        let (segment_id, revision, started_now, previous) =
            self.segment_state.assign_segment_tracking(None);
        if let Some(prev) = previous {
            self.clear_partial_tracking(&prev);
        }
        if started_now {
            clear_segment_audio_enqueue_state(&mut self.queued_sample_len, Some(&segment_id));
            self.decode_pacer.reset_for_new_segment();
        }

        // P1: pace partial ORT jobs as the cumulative buffer (and last decode) grows.
        // Finals always enqueue. Do not advance delta cursor when pacing skips.
        if !is_final {
            let interval_ms = adaptive_partial_decode_interval_ms(
                self.settings.decode_interval_ms,
                segment.duration_ms,
                self.last_decode_ms.load(),
            );
            if !self.decode_pacer.allow_partial(interval_ms) {
                debug!(
                    target: "voicesub.asr_local.pipeline",
                    segment_id = %segment_id,
                    duration_ms = segment.duration_ms,
                    interval_ms,
                    last_decode_ms = self.last_decode_ms.load(),
                    "partial decode paced (skip enqueue)"
                );
                return;
            }
            if interval_ms > self.settings.decode_interval_ms.saturating_mul(11) / 10 {
                info!(
                    target: "voicesub.asr_local.decode_pacing",
                    audio_ms = segment.duration_ms,
                    interval_ms,
                    last_decode_ms = self.last_decode_ms.load(),
                    base_ms = self.settings.decode_interval_ms,
                    "stretched partial decode interval"
                );
            }
        }

        let prepared = segment.audio;
        let (enqueue_audio, skip_enqueue, audio_is_delta) = if self.settings.streaming_decode {
            let (delta, skip) = slice_segment_audio_delta(
                &prepared,
                &segment_id,
                started_now,
                &mut self.queued_sample_len,
            );
            if !is_final && skip {
                return;
            }
            (delta, skip, true)
        } else {
            (prepared, false, false)
        };

        let kind = if is_final {
            AsrWorkKind::Final
        } else {
            AsrWorkKind::Partial
        };
        let mut item = AsrWorkItem::new(
            kind,
            enqueue_audio,
            segment.duration_ms,
            self.runtime_generation.current(),
            segment_id.clone(),
            revision,
        );
        item.vad_ms = vad_ms;
        item.audio_is_delta = audio_is_delta;
        self.segment_queue.push(item);

        debug!(
            target: "voicesub.asr_local.pipeline",
            kind = ?kind,
            segment_id = %segment_id,
            revision,
            audio_is_delta,
            skip_enqueue,
            duration_ms = segment.duration_ms,
            asr_queue_depth = self.segment_queue.queue_depth(),
            partial_jobs_coalesced = self.segment_queue.partial_jobs_coalesced(),
            partial_jobs_dropped = self.segment_queue.partial_jobs_dropped(),
            "vad segment enqueued"
        );

        if is_final {
            clear_segment_audio_enqueue_state(&mut self.queued_sample_len, Some(&segment_id));
            self.segment_state.clear_active_segment();
            self.decode_pacer.reset_for_new_segment();
        }
    }

    fn should_emit_partial(&self, segment_id: &str, text: &str) -> bool {
        let partial = &self.settings.partial_emit;
        should_emit_partial(PartialEmitInput {
            new_text: text,
            previous_text: self.segment_state.get_last_partial_text(segment_id),
            mode: &partial.partial_emit_mode,
            min_new_words: partial.partial_min_new_words,
            min_delta_chars: partial.partial_min_delta_chars,
            coalescing_ms: partial.partial_coalescing_ms,
            previous_emit: self.segment_state.last_partial_emit_at(segment_id),
            now: std::time::Instant::now(),
        })
    }

    fn enqueue_disabled_final(&mut self) {
        let (segment_id, revision, _, _) =
            self.segment_state.assign_segment_tracking(Some(&self.disabled_segment_id));
        let prepared = std::mem::take(&mut self.disabled_vad_audio);
        let duration_ms = segment_duration_ms(prepared.len());
        let item = AsrWorkItem::new(
            AsrWorkKind::Final,
            prepared,
            duration_ms,
            self.runtime_generation.current(),
            segment_id.clone(),
            revision,
        );
        self.segment_queue.push(item);
        clear_segment_audio_enqueue_state(&mut self.queued_sample_len, Some(&segment_id));
        self.disabled_segment_id.clear();
        self.segment_state.clear_active_segment();
    }

    fn decode_buffer_sync(
        &self,
        inference: &InferenceEngine,
        source: &[f32],
    ) -> Result<Option<String>, InferenceError> {
        decode_segment_audio(
            inference,
            &self.recognition,
            source,
            self.settings.window_ms,
            DecodePass::Final,
        )
    }

    fn build_emit(
        &self,
        segment_id: &str,
        revision: u64,
        text: String,
        is_final: bool,
        is_speech: bool,
    ) -> PipelineEmit {
        PipelineEmit {
            segment_id: segment_id.to_string(),
            revision,
            text,
            is_final,
            is_speech,
        }
    }

    fn clear_partial_tracking(&mut self, segment_id: &str) {
        self.segment_state
            .clear_partial_tracking_for_segment(Some(segment_id));
    }

    #[cfg(test)]
    fn prime_active_segment(&mut self, segment_id: &str) {
        self.segment_state
            .assign_segment_tracking(Some(segment_id));
    }

    #[cfg(test)]
    fn enqueue_vad_segment_for_test(&mut self, segment: crate::vad_engine::VadSegment) {
        self.decode_pacer.allow_next();
        self.enqueue_vad_capture_segment(segment, 0.0);
    }
}

/// Run ONNX decode off the capture thread with SST chunk-window context.
pub fn decode_segment_audio(
    inference: &InferenceEngine,
    recognition: &RecognitionProcessor,
    source: &[f32],
    chunk_window_ms: u32,
    pass: DecodePass,
) -> Result<Option<String>, InferenceError> {
    if source.is_empty() {
        return Ok(None);
    }
    let prepare_started = Instant::now();
    let prepared = prepare_decode_window(source, chunk_window_ms, pass);
    let prepare_us = prepare_started.elapsed().as_micros() as u64;
    let mut window = prepared.samples;
    let audio_samples = window.len();

    let preprocess_started = Instant::now();
    recognition.preprocess_for_decode(&mut window, prepared.preemphasis_prev_start);
    let preprocess_us = preprocess_started.elapsed().as_micros() as u64;

    let parakeet_started = Instant::now();
    let text = inference.with_tdt_model(|model| {
        model
            .transcribe_samples(window, SAMPLE_RATE, 1, None)
            .map(|result| normalize_transcript_text(&result.text))
            .map_err(|err| InferenceError::Runtime(err.to_string()))
    })?;
    let parakeet_us = parakeet_started.elapsed().as_micros() as u64;
    inference.record_decode_timing(DecodeTimingBreakdown::from_parts(
        audio_samples,
        prepare_us,
        preprocess_us,
        parakeet_us,
    ));
    if text.is_empty() {
        Ok(None)
    } else {
        debug!(
            target: "voicesub.asr_local.pipeline",
            chars = text.len(),
            samples = source.len(),
            chunk_window_ms,
            pass = ?pass,
            "streaming decode"
        );
        Ok(Some(text))
    }
}

fn segment_duration_ms(samples: usize) -> u32 {
    ((samples as u64 * 1000) / SAMPLE_RATE as u64) as u32
}

pub struct PipelineStopFlag(Arc<AtomicBool>);

impl PipelineStopFlag {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn signal(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn clone_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
    }
}

impl Default for PipelineStopFlag {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr_segment_queue::AsrSegmentQueue;
    use crate::decode::select_partial_window;
    use crate::emit_policy::prefer_final_text;

    fn select_decode_window(cumulative: &[f32], chunk_window_ms: u32, sample_rate: u32) -> Vec<f32> {
        select_partial_window(cumulative, chunk_window_ms, sample_rate).1
    }

    #[test]
    fn streaming_decode_off_still_enqueues_partials_with_full_buffer() {
        // SST OFF: every partial re-transcribes the whole phrase — partials are NOT skipped.
        let queue = AsrSegmentQueue::new(64);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let mut config = LocalAsrConfig::default();
        config.realtime.streaming_decode = false;
        let mut pipeline = RealtimePipeline::new(&config, Arc::clone(&queue), generation);
        pipeline.enqueue_vad_segment_for_test(crate::vad_engine::VadSegment {
            kind: VadSegmentKind::Partial,
            audio: vec![0.1; 3_200],
            duration_ms: 200,
            voiced_ratio: 1.0,
            average_rms: 0.1,
        });
        assert_eq!(queue.queue_depth(), 1);
        let item = queue.pop(0).expect("partial job");
        assert_eq!(item.kind, AsrWorkKind::Partial);
        assert!(!item.audio_is_delta);
        assert_eq!(item.audio.len(), 3_200);
    }

    #[test]
    fn streaming_decode_on_enqueues_audio_delta() {
        let queue = AsrSegmentQueue::new(64);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let mut config = LocalAsrConfig::default();
        config.realtime.streaming_decode = true;
        let mut pipeline = RealtimePipeline::new(&config, Arc::clone(&queue), generation);
        let audio_a = vec![0.1; 1_600];
        let mut audio_b = audio_a.clone();
        audio_b.extend(std::iter::repeat_n(0.2, 1_600));
        pipeline.enqueue_vad_segment_for_test(crate::vad_engine::VadSegment {
            kind: VadSegmentKind::Partial,
            audio: audio_a,
            duration_ms: 100,
            voiced_ratio: 1.0,
            average_rms: 0.1,
        });
        let first = queue.pop(0).expect("first partial");
        assert!(first.audio_is_delta);
        assert_eq!(first.audio.len(), 1_600);

        pipeline.enqueue_vad_segment_for_test(crate::vad_engine::VadSegment {
            kind: VadSegmentKind::Partial,
            audio: audio_b,
            duration_ms: 200,
            voiced_ratio: 1.0,
            average_rms: 0.1,
        });
        let second = queue.pop(0).expect("second partial delta");
        assert!(second.audio_is_delta);
        // Only the suffix not yet queued is enqueued.
        assert_eq!(second.audio.len(), 1_600);
        assert!(second.audio.iter().all(|s| (*s - 0.2).abs() < f32::EPSILON));
    }

    #[test]
    fn adaptive_pacing_skips_rapid_partials_without_advancing_delta() {
        let queue = AsrSegmentQueue::new(64);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let mut config = LocalAsrConfig::default();
        config.realtime.streaming_decode = true;
        config.realtime.decode_interval_ms = Some(500);
        let mut pipeline = RealtimePipeline::new(&config, Arc::clone(&queue), generation);

        let audio_a = vec![0.1; 1_600];
        let mut audio_b = audio_a.clone();
        audio_b.extend(std::iter::repeat_n(0.2, 1_600));

        // First partial of a segment is allowed immediately.
        pipeline.enqueue_vad_capture_segment(
            crate::vad_engine::VadSegment {
                kind: VadSegmentKind::Partial,
                audio: audio_a.clone(),
                duration_ms: 100,
                voiced_ratio: 1.0,
                average_rms: 0.1,
            },
            0.0,
        );
        assert_eq!(queue.queue_depth(), 1);
        let _ = queue.pop(0);

        // Immediate second partial must be paced (do not advance delta cursor).
        pipeline.enqueue_vad_capture_segment(
            crate::vad_engine::VadSegment {
                kind: VadSegmentKind::Partial,
                audio: audio_b.clone(),
                duration_ms: 2_500,
                voiced_ratio: 1.0,
                average_rms: 0.1,
            },
            0.0,
        );
        assert_eq!(queue.queue_depth(), 0);

        // Force allow and confirm full remaining delta is still available.
        pipeline.decode_pacer.allow_next();
        pipeline.enqueue_vad_capture_segment(
            crate::vad_engine::VadSegment {
                kind: VadSegmentKind::Partial,
                audio: audio_b,
                duration_ms: 2_500,
                voiced_ratio: 1.0,
                average_rms: 0.1,
            },
            0.0,
        );
        let item = queue.pop(0).expect("paced partial eventually enqueued");
        assert_eq!(item.audio.len(), 1_600);
        assert!(item.audio.iter().all(|s| (*s - 0.2).abs() < f32::EPSILON));
    }

    #[test]
    fn prefer_final_text_keeps_longer_partial() {
        assert_eq!(
            prefer_final_text(Some("hello world"), "yeah"),
            "hello world"
        );
        assert_eq!(
            prefer_final_text(Some("hello"), "hello world"),
            "hello world"
        );
    }

    #[test]
    fn optional_tail_window_selects_left_context_for_long_audio() {
        let sample_rate = 16_000;
        let cumulative = vec![0.0; sample_rate as usize * 10];
        let window = select_decode_window(&cumulative, 640, sample_rate);
        assert!(window.len() < cumulative.len());
        assert!(window.len() >= crate::segment_enqueue::ms_to_samples(1800, sample_rate));
    }

    #[test]
    fn disabled_vad_does_not_panic_on_empty_push() {
        let queue = AsrSegmentQueue::new(64);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let mut config = LocalAsrConfig::default();
        config.vad.enabled = false;
        let mut pipeline = RealtimePipeline::new(&config, queue, generation);
        assert!(pipeline.push_samples(&[]).is_ok());
    }

    #[test]
    fn prepare_decode_window_keeps_full_buffer_for_partials() {
        let sample_rate = 16_000;
        let cumulative = vec![1.0; sample_rate as usize * 10];
        let prepared = prepare_decode_window(&cumulative, 640, DecodePass::Partial);
        assert_eq!(prepared.samples.len(), cumulative.len());
    }

    #[test]
    fn partial_survives_while_final_decode_is_pending() {
        let queue = AsrSegmentQueue::new(64);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let config = LocalAsrConfig::default();
        let mut pipeline = RealtimePipeline::new(&config, queue, generation);
        pipeline.prime_active_segment("segment-1");
        let partial_item = AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.1; 100],
            100,
            1,
            "segment-1".into(),
            1,
        );
        assert!(!pipeline
            .process_decode_result(partial_item, Some("hello".into()))
            .is_empty());
        let late_partial = AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.1; 150],
            150,
            1,
            "segment-1".into(),
            3,
        );
        assert!(!pipeline
            .process_decode_result(late_partial, Some("hello world".into()))
            .is_empty());
        let final_item = AsrWorkItem::new(
            AsrWorkKind::Final,
            vec![0.1; 200],
            200,
            1,
            "segment-1".into(),
            2,
        );
        assert!(!pipeline
            .process_decode_result(final_item, Some("hello world".into()))
            .is_empty());
    }

    #[test]
    fn stale_partial_after_final_still_emits() {
        let queue = AsrSegmentQueue::new(64);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let config = LocalAsrConfig::default();
        let mut pipeline = RealtimePipeline::new(&config, queue, generation);
        let final_item = AsrWorkItem::new(
            AsrWorkKind::Final,
            vec![0.1; 100],
            100,
            1,
            "segment-1".into(),
            2,
        );
        assert!(!pipeline
            .process_decode_result(final_item, Some("done".into()))
            .is_empty());
        let stale_partial = AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.1; 100],
            100,
            1,
            "segment-1".into(),
            1,
        );
        assert!(!pipeline
            .process_decode_result(stale_partial, Some("done".into()))
            .is_empty());
    }

    #[test]
    fn process_decode_grows_partial_like_typed_text() {
        let queue = AsrSegmentQueue::new(64);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let config = LocalAsrConfig::default();
        let mut pipeline = RealtimePipeline::new(&config, queue, generation);
        pipeline.prime_active_segment("seg-grow");
        let first = pipeline.process_decode_result(
            AsrWorkItem::new(AsrWorkKind::Partial, vec![0.1; 100], 100, 1, "seg-grow".into(), 1),
            Some("hello".into()),
        );
        assert_eq!(first.len(), 1);
        assert!(!first[0].is_final);
        assert_eq!(first[0].text, "hello");
        let second = pipeline.process_decode_result(
            AsrWorkItem::new(AsrWorkKind::Partial, vec![0.1; 200], 200, 1, "seg-grow".into(), 2),
            Some("hello world".into()),
        );
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].text, "hello world");
        let final_emit = pipeline.process_decode_result(
            AsrWorkItem::new(AsrWorkKind::Final, vec![0.1; 200], 200, 1, "seg-grow".into(), 3),
            Some(
                "hello world hello world".into(),
            ),
        );
        assert_eq!(final_emit.len(), 1);
        assert!(final_emit[0].is_final);
        assert_eq!(final_emit[0].text, "hello world");
    }

    #[test]
    fn stale_generation_results_are_ignored() {
        let queue = AsrSegmentQueue::new(64);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let config = LocalAsrConfig::default();
        let mut pipeline = RealtimePipeline::new(&config, queue, generation.clone());
        pipeline.invalidate_runtime_generation();
        let item = AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.1; 100],
            100,
            1,
            "segment-1".into(),
            1,
        );
        assert!(pipeline
            .process_decode_result(item, Some("hello".into()))
            .is_empty());
    }
}
