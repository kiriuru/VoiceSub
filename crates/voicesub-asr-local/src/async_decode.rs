//! Background ONNX decode worker — Parakeet TDT cumulative batch redecode.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use parakeet_rs::Transcriber;
use voicesub_partial_emit::normalize_transcript_text;

use crate::asr_segment_queue::{AsrSegmentQueue, AsrWorkItem, AsrWorkKind, RuntimeGeneration};
use crate::config::LocalAsrRecognitionConfig;
use crate::decode::{DecodePass, prepare_decode_window};
use crate::decode_pacing::LastDecodeMs;
use crate::decode_timing::DecodeTimingBreakdown;
use crate::inference::{InferenceEngine, InferenceError};
use crate::pipeline::SAMPLE_RATE;
use crate::recognition_processing::RecognitionProcessor;

pub struct AsyncDecodeWorker {
    queue: Arc<AsrSegmentQueue>,
    stop: Arc<AtomicBool>,
    result_rx: Receiver<(AsrWorkItem, Result<Option<String>, InferenceError>)>,
    handle: Option<JoinHandle<()>>,
}

impl AsyncDecodeWorker {
    pub fn spawn_with_feedback(
        inference: Arc<InferenceEngine>,
        recognition: LocalAsrRecognitionConfig,
        queue: Arc<AsrSegmentQueue>,
        runtime_generation: Arc<RuntimeGeneration>,
        chunk_window_ms: u32,
        last_decode_ms: Arc<LastDecodeMs>,
    ) -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_queue = Arc::clone(&queue);
        let worker_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            run_decode_worker(
                inference,
                recognition,
                worker_queue,
                runtime_generation,
                chunk_window_ms,
                worker_stop,
                result_tx,
                last_decode_ms,
            )
        });
        Self {
            queue,
            stop,
            result_rx,
            handle: Some(handle),
        }
    }

    pub fn drain_results(
        &self,
    ) -> impl Iterator<Item = (AsrWorkItem, Result<Option<String>, InferenceError>)> + '_ {
        std::iter::from_fn(|| self.result_rx.try_recv().ok())
    }
}

impl Drop for AsyncDecodeWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.queue.wake();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct WorkerState {
    cumulative_audio: HashMap<String, Vec<f32>>,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            cumulative_audio: HashMap::new(),
        }
    }

    fn clear_segment(&mut self, segment_id: &str) {
        self.cumulative_audio.remove(segment_id);
    }
}

fn run_decode_worker(
    inference: Arc<InferenceEngine>,
    recognition: LocalAsrRecognitionConfig,
    queue: Arc<AsrSegmentQueue>,
    runtime_generation: Arc<RuntimeGeneration>,
    chunk_window_ms: u32,
    stop: Arc<AtomicBool>,
    result_tx: Sender<(AsrWorkItem, Result<Option<String>, InferenceError>)>,
    last_decode_ms: Arc<LastDecodeMs>,
) {
    let processor = RecognitionProcessor::from(&recognition);
    let mut state = WorkerState::new();

    loop {
        if stop.load(Ordering::SeqCst) && queue.len() == 0 {
            break;
        }
        let Some(item) = queue.pop(250) else {
            continue;
        };
        if item.generation != runtime_generation.current() {
            continue;
        }

        let is_final = item.kind == AsrWorkKind::Final;
        let pass = if is_final {
            DecodePass::Final
        } else {
            DecodePass::Partial
        };

        let decode_source = {
            let entry = state
                .cumulative_audio
                .entry(item.segment_id.clone())
                .or_default();
            if item.audio_is_delta {
                entry.extend_from_slice(&item.audio);
            } else if !item.audio.is_empty() {
                *entry = item.audio.clone();
            }
            entry.as_slice()
        };

        let decoded = decode_batch_tdt(
            &inference,
            &processor,
            decode_source,
            chunk_window_ms,
            pass,
            &last_decode_ms,
        );

        if result_tx.send((item.clone(), decoded)).is_err() {
            break;
        }

        if is_final {
            state.clear_segment(&item.segment_id);
        }
    }
}

fn decode_batch_tdt(
    inference: &InferenceEngine,
    processor: &RecognitionProcessor,
    source: &[f32],
    chunk_window_ms: u32,
    pass: DecodePass,
    last_decode_ms: &LastDecodeMs,
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
    processor.preprocess_for_decode(&mut window, prepared.preemphasis_prev_start);
    let preprocess_us = preprocess_started.elapsed().as_micros() as u64;

    let parakeet_started = Instant::now();
    let text = inference.with_tdt_model(|model| {
        model
            .transcribe_samples(window, SAMPLE_RATE, 1, None)
            .map(|result| normalize_transcript_text(&result.text))
            .map_err(|err| InferenceError::Runtime(err.to_string()))
    })?;
    let parakeet_us = parakeet_started.elapsed().as_micros() as u64;
    let timing = DecodeTimingBreakdown::from_parts(
        audio_samples,
        prepare_us,
        preprocess_us,
        parakeet_us,
    );
    last_decode_ms.store(timing.total_ms());
    inference.record_decode_timing(timing);
    Ok(non_empty_text(text))
}

fn non_empty_text(text: String) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr_segment_queue::{AsrWorkItem, AsrWorkKind};

    #[test]
    fn worker_ignores_stale_generation_jobs() {
        let queue = AsrSegmentQueue::new(8);
        let generation = Arc::new(RuntimeGeneration::new(1));
        let inference = Arc::new(InferenceEngine::new());
        let worker = AsyncDecodeWorker::spawn_with_feedback(
            inference,
            LocalAsrRecognitionConfig::default(),
            Arc::clone(&queue),
            Arc::clone(&generation),
            640,
            crate::decode_pacing::LastDecodeMs::new(),
        );
        generation.bump();
        queue.push(AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.1; 1600],
            100,
            1,
            "segment-1".into(),
            1,
        ));
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(worker.drain_results().next().is_none());
    }
}
