//! Live runtime mic capture → `RealtimePipeline` (SST `local_asr_pipeline` ingest).

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use parking_lot::Mutex;
use thiserror::Error;
use tracing::{info, warn};

use crate::asr_segment_queue::{AsrSegmentQueue, RuntimeGeneration};
use crate::async_decode::AsyncDecodeWorker;
use crate::capture::{self, CaptureError, MicStream};
use crate::config::LocalAsrConfig;
use crate::emit_policy::{RealtimeEmitPolicy, dedupe_repeated_transcript};
use crate::emit_telemetry::{EmitTelemetry, EmitTelemetrySnapshot};
use crate::inference::{InferenceEngine, InferenceError};
use crate::pipeline::{PipelineEmit, PipelineStopFlag, RealtimePipeline, SAMPLE_RATE};

pub type RuntimeEmitCallback = Arc<dyn Fn(PipelineEmit) + Send + Sync>;

#[derive(Debug, Error)]
pub enum RuntimeSessionError {
    #[error("local ASR runtime capture already running")]
    AlreadyRunning,
    #[error("local ASR runtime capture is not running")]
    NotRunning,
    #[error("capture error: {0}")]
    Capture(#[from] CaptureError),
    #[error("inference error: {0}")]
    Inference(#[from] InferenceError),
    #[error("{0}")]
    Precondition(String),
}

struct RuntimeSessionInner {
    running: bool,
    worker: Option<JoinHandle<()>>,
    stop: PipelineStopFlag,
    mic: Option<MicStream>,
    telemetry: Arc<EmitTelemetry>,
}

pub struct LocalAsrRuntimeSession {
    inner: Arc<Mutex<RuntimeSessionInner>>,
}

impl LocalAsrRuntimeSession {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RuntimeSessionInner {
                running: false,
                worker: None,
                stop: PipelineStopFlag::new(),
                mic: None,
                telemetry: EmitTelemetry::new_shared(),
            })),
        }
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().running
    }

    pub fn emit_telemetry(&self) -> EmitTelemetrySnapshot {
        self.inner.lock().telemetry.snapshot()
    }

    pub fn start(
        &self,
        inference: Arc<InferenceEngine>,
        config: LocalAsrConfig,
        on_emit: RuntimeEmitCallback,
    ) -> Result<(), RuntimeSessionError> {
        let mut guard = self.inner.lock();
        if guard.running {
            return Err(RuntimeSessionError::AlreadyRunning);
        }
        let (prev_worker, prev_mic) = take_worker_and_mic(&mut guard);
        drop(guard);
        join_worker_outside_lock(prev_worker, prev_mic);
        let mut guard = self.inner.lock();

        guard.stop = PipelineStopFlag::new();
        let stop_flag = guard.stop.clone_flag();
        guard.telemetry.reset();
        let telemetry = Arc::clone(&guard.telemetry);
        let device_id = config.microphone.device_id.clone();
        let (mic, rx, _sample_rate, _label) = capture::start_mic_stream(&device_id)?;
        guard.mic = Some(mic);
        guard.running = true;

        let worker_state = Arc::clone(&self.inner);
        let handle = thread::spawn(move || {
            run_runtime_capture_loop(
                worker_state,
                inference,
                config,
                rx,
                stop_flag,
                on_emit,
                telemetry,
            );
        });
        guard.worker = Some(handle);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), RuntimeSessionError> {
        let (worker, mic) = {
            let mut guard = self.inner.lock();
            if !guard.running {
                return Err(RuntimeSessionError::NotRunning);
            }
            guard.stop.signal();
            take_worker_and_mic(&mut guard)
        };
        join_worker_outside_lock(worker, mic);
        let mut guard = self.inner.lock();
        guard.running = false;
        Ok(())
    }
}

impl Default for LocalAsrRuntimeSession {
    fn default() -> Self {
        Self::new()
    }
}

fn take_worker_and_mic(
    guard: &mut RuntimeSessionInner,
) -> (Option<JoinHandle<()>>, Option<MicStream>) {
    (guard.worker.take(), guard.mic.take())
}

fn join_worker_outside_lock(worker: Option<JoinHandle<()>>, mut mic: Option<MicStream>) {
    if let Some(mut mic_stream) = mic.take() {
        mic_stream.stop();
    }
    if let Some(handle) = worker {
        let _ = handle.join();
    }
}

fn drain_decode_results(
    pipeline: &mut RealtimePipeline,
    decode_worker: &AsyncDecodeWorker,
    emit_policy: &mut RealtimeEmitPolicy,
    on_emit: &RuntimeEmitCallback,
    telemetry: &EmitTelemetry,
) {
    for (item, decoded) in decode_worker.drain_results() {
        match decoded {
            Ok(text) => {
                let duration_ms = item.duration_ms;
                let emits = pipeline.process_decode_result(item, text);
                forward_emits(emit_policy, emits, on_emit, telemetry, duration_ms);
            }
            Err(err) => {
                if matches!(
                    err,
                    crate::inference::InferenceError::ProfilingBudgetReached
                ) {
                    info!(
                        target: "voicesub.asr_local.runtime",
                        "ORT profiling budget reached — profile flushed; further decodes skipped until reload"
                    );
                    continue;
                }
                warn!(
                    target: "voicesub.asr_local.runtime",
                    error = %err,
                    "async streaming decode failed"
                );
            }
        }
    }
}

fn forward_emits(
    emit_policy: &mut RealtimeEmitPolicy,
    emits: Vec<PipelineEmit>,
    on_emit: &RuntimeEmitCallback,
    telemetry: &EmitTelemetry,
    segment_speech_ms: u32,
) {
    for emit in emits {
        let previous = emit_policy.last_partial(&emit.segment_id).to_string();
        for mut emit in emit_policy.apply(vec![emit]) {
            emit.text = dedupe_repeated_transcript(&emit.text);
            if emit.text.is_empty() {
                continue;
            }
            telemetry.record_emit(&emit, &previous, segment_speech_ms);
            on_emit(emit);
        }
    }
}

fn run_runtime_capture_loop(
    shared: Arc<Mutex<RuntimeSessionInner>>,
    inference: Arc<InferenceEngine>,
    config: LocalAsrConfig,
    rx: std::sync::mpsc::Receiver<Vec<f32>>,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    on_emit: RuntimeEmitCallback,
    telemetry: Arc<EmitTelemetry>,
) {
    let queue = AsrSegmentQueue::new(64);
    let generation = Arc::new(RuntimeGeneration::new(1));
    let mut pipeline = RealtimePipeline::new(&config, Arc::clone(&queue), Arc::clone(&generation));
    let last_decode_ms = pipeline.last_decode_ms_handle();
    let mut emit_policy = RealtimeEmitPolicy::default();
    let decode_worker = AsyncDecodeWorker::spawn_with_feedback(
        inference.clone(),
        config.recognition.clone(),
        queue,
        generation,
        pipeline.chunk_window_ms(),
        last_decode_ms,
    );

    loop {
        if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        drain_decode_results(
            &mut pipeline,
            &decode_worker,
            &mut emit_policy,
            &on_emit,
            &telemetry,
        );

        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(samples) => {
                if let Err(err) = pipeline.push_samples(&samples) {
                    warn!(
                        target: "voicesub.asr_local.runtime",
                        error = %err,
                        "streaming pipeline failed"
                    );
                    shared.lock().running = false;
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    drain_decode_results(
        &mut pipeline,
        &decode_worker,
        &mut emit_policy,
        &on_emit,
        &telemetry,
    );

    match pipeline.finalize_on_stop(&inference) {
        Ok(emits) => forward_emits(&mut emit_policy, emits, &on_emit, &telemetry, 0),
        Err(err) => {
            warn!(
                target: "voicesub.asr_local.runtime",
                error = %err,
                "finalize on stop failed"
            );
        }
    }

    drain_decode_results(
        &mut pipeline,
        &decode_worker,
        &mut emit_policy,
        &on_emit,
        &telemetry,
    );

    pipeline.invalidate_runtime_generation();

    info!(
        target: "voicesub.asr_local.runtime",
        device_id = %config.microphone.device_id,
        sample_rate = SAMPLE_RATE,
        "local asr runtime capture stopped"
    );
    shared.lock().running = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_session_defaults_idle() {
        let session = LocalAsrRuntimeSession::new();
        assert!(!session.is_running());
    }
}
