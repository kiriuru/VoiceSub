use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
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

#[derive(Debug, Error)]
pub enum TestBenchError {
    #[error("test bench already running")]
    AlreadyRunning,
    #[error("test bench is not running")]
    NotRunning,
    #[error("capture error: {0}")]
    Capture(#[from] CaptureError),
    #[error("inference error: {0}")]
    Inference(#[from] InferenceError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestBenchPhase {
    Idle,
    Listening,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestBenchSnapshot {
    pub running: bool,
    pub phase: TestBenchPhase,
    pub execution_provider: String,
    pub model_label: String,
    pub duration_ms: u64,
    pub device_id: String,
    pub device_label: String,
    pub sample_count: u64,
    pub sample_rate: u32,
    /// Live partial text for the active segment.
    pub partial_transcript: String,
    /// Finalized lines joined for display.
    pub transcript: String,
    pub finalized_segments: u32,
    pub is_speech: bool,
    pub decode_count: u32,
    pub capture_ms: u64,
    pub last_decode_ms: u64,
    /// P0 outside-vs-parakeet breakdown for the latest decode.
    pub last_decode_timing: Option<crate::decode_timing::DecodeTimingBreakdown>,
    pub error: Option<String>,
}

impl Default for TestBenchSnapshot {
    fn default() -> Self {
        Self {
            running: false,
            phase: TestBenchPhase::Idle,
            execution_provider: String::new(),
            model_label: String::new(),
            duration_ms: 0,
            device_id: String::new(),
            device_label: String::new(),
            sample_count: 0,
            sample_rate: SAMPLE_RATE,
            partial_transcript: String::new(),
            transcript: String::new(),
            finalized_segments: 0,
            is_speech: false,
            decode_count: 0,
            capture_ms: 0,
            last_decode_ms: 0,
            last_decode_timing: None,
            error: None,
        }
    }
}

struct TestBenchInner {
    snapshot: TestBenchSnapshot,
    worker: Option<JoinHandle<()>>,
    stop: PipelineStopFlag,
    mic: Option<MicStream>,
    telemetry: Arc<EmitTelemetry>,
}

pub struct TestBench {
    inner: Arc<Mutex<TestBenchInner>>,
}

impl TestBench {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TestBenchInner {
                snapshot: TestBenchSnapshot::default(),
                worker: None,
                stop: PipelineStopFlag::new(),
                mic: None,
                telemetry: EmitTelemetry::new_shared(),
            })),
        }
    }

    pub fn emit_telemetry(&self) -> EmitTelemetrySnapshot {
        self.inner.lock().telemetry.snapshot()
    }

    pub fn snapshot(&self) -> TestBenchSnapshot {
        self.inner.lock().snapshot.clone()
    }

    pub fn start(
        &self,
        inference: Arc<InferenceEngine>,
        config: LocalAsrConfig,
        duration_ms: u64,
        execution_provider: String,
        device_id: String,
        device_label: String,
    ) -> Result<(), TestBenchError> {
        let mut guard = self.inner.lock();
        if guard.snapshot.running {
            return Err(TestBenchError::AlreadyRunning);
        }
        let (prev_worker, prev_mic) = take_worker_and_mic(&mut guard);
        drop(guard);
        join_worker_outside_lock(prev_worker, prev_mic);
        let mut guard = self.inner.lock();

        guard.stop = PipelineStopFlag::new();
        let stop_flag = guard.stop.clone_flag();
        guard.telemetry.reset();
        let telemetry = Arc::clone(&guard.telemetry);
        guard.snapshot = TestBenchSnapshot {
            running: true,
            phase: TestBenchPhase::Listening,
            execution_provider: execution_provider.clone(),
            model_label: crate::model_family::model_display_label(
                &config.model.family,
                &config.model.variant,
            ),
            duration_ms,
            device_id: device_id.clone(),
            device_label,
            sample_rate: SAMPLE_RATE,
            ..TestBenchSnapshot::default()
        };

        let (mic, rx, sample_rate, resolved_label) = capture::start_mic_stream(&device_id)?;
        guard.mic = Some(mic);
        guard.snapshot.device_label = resolved_label;
        guard.snapshot.sample_rate = sample_rate;

        let worker_state = Arc::clone(&self.inner);
        let handle = thread::spawn(move || {
            run_streaming_test_bench(
                worker_state,
                inference,
                config,
                duration_ms,
                execution_provider,
                device_id,
                rx,
                stop_flag,
                telemetry,
            );
        });
        guard.worker = Some(handle);
        Ok(())
    }

    pub fn stop(&self) -> Result<TestBenchSnapshot, TestBenchError> {
        let (worker, mic) = {
            let mut guard = self.inner.lock();
            if !guard.snapshot.running {
                return Err(TestBenchError::NotRunning);
            }
            guard.stop.signal();
            take_worker_and_mic(&mut guard)
        };

        join_worker_outside_lock(worker, mic);

        let mut guard = self.inner.lock();
        guard.snapshot.running = false;
        if guard.snapshot.phase == TestBenchPhase::Listening {
            guard.snapshot.phase = TestBenchPhase::Done;
        }
        Ok(guard.snapshot.clone())
    }
}

fn take_worker_and_mic(guard: &mut TestBenchInner) -> (Option<JoinHandle<()>>, Option<MicStream>) {
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

impl Default for TestBench {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::too_many_arguments)]
fn run_streaming_test_bench(
    shared: Arc<Mutex<TestBenchInner>>,
    inference: Arc<InferenceEngine>,
    config: LocalAsrConfig,
    duration_ms: u64,
    execution_provider: String,
    device_id: String,
    rx: std::sync::mpsc::Receiver<Vec<f32>>,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    telemetry: Arc<EmitTelemetry>,
) {
    let started = Instant::now();
    let max_duration = if duration_ms == 0 {
        None
    } else {
        Some(Duration::from_millis(duration_ms))
    };

    let queue = AsrSegmentQueue::new(64);
    let generation = Arc::new(RuntimeGeneration::new(1));
    let mut pipeline = RealtimePipeline::new(&config, Arc::clone(&queue), Arc::clone(&generation));
    let last_decode_ms = pipeline.last_decode_ms_handle();
    let mut emit_policy = RealtimeEmitPolicy::default();
    let decode_worker = AsyncDecodeWorker::spawn_with_feedback(
        inference.clone(),
        config.recognition,
        queue,
        generation,
        pipeline.chunk_window_ms(),
        last_decode_ms,
    );
    let mut finalized_lines: Vec<String> = Vec::new();
    let mut partial = String::new();
    let mut decode_count = 0u32;
    let mut last_decode_ms = 0u64;
    let mut last_decode_timing = None;

    // Same emit path as live runtime (§4.8 D2): policy → dedupe → telemetry → UI.
    let mut apply_emits = |emits: Vec<PipelineEmit>, segment_speech_ms: u32| {
        for emit in emits {
            let previous = emit_policy.last_partial(&emit.segment_id).to_string();
            for mut emit in emit_policy.apply(vec![emit]) {
                emit.text = dedupe_repeated_transcript(&emit.text);
                if emit.text.is_empty() {
                    continue;
                }
                telemetry.record_emit(&emit, &previous, segment_speech_ms);
                if emit.is_final {
                    finalized_lines.push(emit.text.clone());
                    partial.clear();
                    update_snapshot(&shared, |snap| {
                        snap.finalized_segments += 1;
                        snap.partial_transcript.clear();
                        snap.transcript = finalized_lines.join("\n");
                    });
                } else {
                    partial = emit.text.clone();
                    decode_count += 1;
                    update_snapshot(&shared, |snap| {
                        snap.partial_transcript = partial.clone();
                        snap.is_speech = emit.is_speech;
                        snap.decode_count = decode_count;
                    });
                }
            }
        }
    };

    loop {
        if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        if max_duration.is_some_and(|limit| started.elapsed() >= limit) {
            break;
        }

        for (item, decoded) in decode_worker.drain_results() {
            match decoded {
                Ok(text) => {
                    if let Some(timing) = inference.snapshot().last_decode_timing {
                        last_decode_ms = timing.total_ms();
                        last_decode_timing = Some(timing.clone());
                        update_snapshot(&shared, |snap| {
                            snap.last_decode_ms = timing.total_ms();
                            snap.last_decode_timing = Some(timing);
                        });
                    }
                    let duration_ms = item.duration_ms;
                    apply_emits(pipeline.process_decode_result(item, text), duration_ms);
                }
                Err(err) => {
                    if matches!(err, InferenceError::ProfilingBudgetReached) {
                        info!(
                            target: "voicesub.asr_local.test",
                            "ORT profiling budget reached — stopping test bench and flushing profile"
                        );
                        update_snapshot(&shared, |snap| {
                            snap.running = false;
                            snap.phase = TestBenchPhase::Done;
                            snap.error = None;
                        });
                        return;
                    }
                    warn!(
                        target: "voicesub.asr_local.test",
                        error = %err,
                        "async streaming decode failed"
                    );
                    update_snapshot(&shared, |snap| {
                        snap.running = false;
                        snap.phase = TestBenchPhase::Error;
                        snap.error = Some(err.to_string());
                    });
                    return;
                }
            }
        }

        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(samples) => {
                let sample_count = samples.len() as u64;
                update_snapshot(&shared, |snap| {
                    snap.sample_count = snap.sample_count.saturating_add(sample_count);
                    snap.capture_ms = started.elapsed().as_millis() as u64;
                });

                if let Err(err) = pipeline.push_samples(&samples) {
                    warn!(
                        target: "voicesub.asr_local.test",
                        error = %err,
                        "streaming decode failed"
                    );
                    update_snapshot(&shared, |snap| {
                        snap.running = false;
                        snap.phase = TestBenchPhase::Error;
                        snap.error = Some(err.to_string());
                    });
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    for (item, decoded) in decode_worker.drain_results() {
        if let Ok(text) = decoded {
            let duration_ms = item.duration_ms;
            apply_emits(pipeline.process_decode_result(item, text), duration_ms);
        }
    }

    match pipeline.finalize_on_stop(&inference) {
        Ok(emits) => apply_emits(emits, 0),
        Err(err) => {
            update_snapshot(&shared, |snap| {
                snap.running = false;
                snap.phase = TestBenchPhase::Error;
                snap.error = Some(err.to_string());
            });
            return;
        }
    }

    for (item, decoded) in decode_worker.drain_results() {
        if let Ok(text) = decoded {
            let duration_ms = item.duration_ms;
            apply_emits(pipeline.process_decode_result(item, text), duration_ms);
        }
    }

    let transcript = if finalized_lines.is_empty() {
        partial.clone()
    } else if partial.is_empty() {
        finalized_lines.join("\n")
    } else {
        format!("{}\n{}", finalized_lines.join("\n"), partial)
    };

    info!(
        target: "voicesub.asr_local.test",
        provider = %execution_provider,
        device_id = %device_id,
        segments = finalized_lines.len(),
        decode_count,
        chars = transcript.len(),
        "local asr streaming test bench complete"
    );

    update_snapshot(&shared, |snap| {
        snap.running = false;
        snap.phase = TestBenchPhase::Done;
        snap.partial_transcript = partial;
        snap.transcript = transcript;
        snap.finalized_segments = finalized_lines.len() as u32;
        snap.decode_count = decode_count;
        snap.capture_ms = started.elapsed().as_millis() as u64;
        snap.last_decode_ms = last_decode_ms;
        snap.last_decode_timing = last_decode_timing.clone();
        snap.error = None;
    });
}

fn update_snapshot(
    shared: &Arc<Mutex<TestBenchInner>>,
    update: impl FnOnce(&mut TestBenchSnapshot),
) {
    let mut guard = shared.lock();
    update(&mut guard.snapshot);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_defaults_idle() {
        let bench = TestBench::new();
        let snap = bench.snapshot();
        assert_eq!(snap.phase, TestBenchPhase::Idle);
        assert!(!snap.running);
    }
}
