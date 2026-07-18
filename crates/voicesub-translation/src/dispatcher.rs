use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify, Semaphore};
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tracing::{debug, warn};
use voicesub_subtitle::{TranslationEvent, TranslationItem};

use crate::engine::{
    DEFAULT_TRANSLATION_RETRIES, PreparedLine, PreparedRequest, TranslateTargetOptions,
    TranslationEngine,
};
use crate::preview_lineage::TranslationPreviewLineage;

const DEFAULT_QUEUE_MAX: usize = 8;
const DEFAULT_TIMEOUT_MS: u64 = 10_000;
/// Local LLM (LM Studio / Ollama) JIT load often exceeds the global 10s default.
const LOCAL_LLM_MIN_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_MAX_CONCURRENT: usize = 2;

pub type ConfigGetter = Arc<dyn Fn() -> Value + Send + Sync>;
pub type PublishFn =
    Arc<dyn Fn(TranslationEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;
pub type RelevanceFn = Arc<dyn Fn(u64) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;
pub type MetricsCallback = Arc<dyn Fn(Value) + Send + Sync>;
pub type StructuredLogFn = Arc<dyn Fn(&str, &str, Value) + Send + Sync>;

#[derive(Clone, Default)]
pub struct DispatcherCallbacks {
    pub metrics_callback: Option<MetricsCallback>,
    pub structured_log: Option<StructuredLogFn>,
}

#[derive(Clone)]
struct QueuedJob {
    job_id: u64,
    sequence: u64,
    source_text: String,
    source_lang: String,
    preview_lineage_key: Option<String>,
    preview_generation: u64,
    submitted_at: Instant,
}

struct ActiveTask {
    job_id: u64,
    sequence: u64,
    source_lang: String,
    source_text_len: usize,
    cancel: Arc<AtomicBool>,
    cancel_notify: Arc<Notify>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

struct DispatcherInner {
    queue: VecDeque<QueuedJob>,
    active_jobs: usize,
    active_tasks: Vec<ActiveTask>,
    stopped: bool,
    next_job_id: u64,
    preview_lineage: TranslationPreviewLineage,
    worker: Option<JoinHandle<()>>,
    jobs_cancelled: u64,
    provider_semaphores: HashMap<String, (usize, Arc<Semaphore>)>,
    provider_mutexes: HashMap<String, Arc<Mutex<()>>>,
    provider_next_allowed_at: HashMap<String, f64>,
    last_logged_queue_depth: usize,
    last_runtime_reason: Option<String>,
    last_slot_id: Option<String>,
    last_provider: Option<String>,
    last_target_lang: Option<String>,
    last_timeout_ms: Option<u64>,
    last_provider_latency_ms: Option<f64>,
    queue_latency_ms: Option<f64>,
}

enum LineOutcome {
    Done,
    Timeout,
    Error,
    ProviderSkipped,
}

struct LineResult {
    item: TranslationItem,
    outcome: LineOutcome,
    provider_latency_ms: f64,
    reason: Option<String>,
    status_message: Option<String>,
    used_default_prompt: bool,
}

pub struct TranslationDispatcher {
    inner: Mutex<DispatcherInner>,
    engine: Arc<Mutex<TranslationEngine>>,
    config_getter: ConfigGetter,
    publish: PublishFn,
    is_relevant: RelevanceFn,
    callbacks: DispatcherCallbacks,
    notify: Notify,
    jobs_started: AtomicU64,
    stale_dropped: AtomicU64,
    provider_skipped: AtomicU64,
}

impl TranslationDispatcher {
    pub fn new(
        engine: TranslationEngine,
        config_getter: ConfigGetter,
        publish: PublishFn,
        is_relevant: RelevanceFn,
    ) -> Arc<Self> {
        Self::with_callbacks(
            engine,
            config_getter,
            publish,
            is_relevant,
            DispatcherCallbacks::default(),
        )
    }

    pub fn with_callbacks(
        engine: TranslationEngine,
        config_getter: ConfigGetter,
        publish: PublishFn,
        is_relevant: RelevanceFn,
        callbacks: DispatcherCallbacks,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(DispatcherInner {
                queue: VecDeque::new(),
                active_jobs: 0,
                active_tasks: Vec::new(),
                stopped: false,
                next_job_id: 0,
                preview_lineage: TranslationPreviewLineage::default(),
                worker: None,
                jobs_cancelled: 0,
                provider_semaphores: HashMap::new(),
                provider_mutexes: HashMap::new(),
                provider_next_allowed_at: HashMap::new(),
                last_logged_queue_depth: usize::MAX,
                last_runtime_reason: None,
                last_slot_id: None,
                last_provider: None,
                last_target_lang: None,
                last_timeout_ms: None,
                last_provider_latency_ms: None,
                queue_latency_ms: None,
            }),
            engine: Arc::new(Mutex::new(engine)),
            config_getter,
            publish,
            is_relevant,
            callbacks,
            notify: Notify::new(),
            jobs_started: AtomicU64::new(0),
            stale_dropped: AtomicU64::new(0),
            provider_skipped: AtomicU64::new(0),
        })
    }

    pub async fn start(self: &Arc<Self>) {
        let mut inner = self.inner.lock().await;
        inner.stopped = false;
        self.ensure_worker_locked(&mut inner);
    }

    pub fn engine_handle(self: &Arc<Self>) -> Arc<Mutex<TranslationEngine>> {
        Arc::clone(&self.engine)
    }

    pub async fn stop(self: &Arc<Self>) {
        let mut inner = self.inner.lock().await;
        inner.stopped = true;
        inner.queue.clear();
        let aborted = inner.active_tasks.len();
        for task in inner.active_tasks.drain(..) {
            task.cancel.store(true, Ordering::Release);
            task.cancel_notify.notify_waiters();
            if let Some(handle) = task.handle.lock().await.take() {
                handle.abort();
            }
        }
        inner.active_jobs = inner.active_jobs.saturating_sub(aborted);
        if let Some(worker) = inner.worker.take() {
            worker.abort();
        }
        self.notify.notify_waiters();
    }

    pub async fn cancel_older_than(self: &Arc<Self>, sequence: u64) {
        let (queued_jobs, active_tasks) = {
            let inner = self.inner.lock().await;
            let queued_jobs = inner
                .queue
                .iter()
                .filter(|job| job.sequence < sequence)
                .cloned()
                .collect::<Vec<_>>();
            let active_tasks = inner
                .active_tasks
                .iter()
                .filter(|task| task.sequence < sequence)
                .map(|task| {
                    (
                        task.job_id,
                        task.sequence,
                        task.source_lang.clone(),
                        task.source_text_len,
                    )
                })
                .collect::<Vec<_>>();
            (queued_jobs, active_tasks)
        };

        let mut queued_cancel_sequences = HashSet::new();
        for job in queued_jobs {
            if (self.is_relevant)(job.sequence).await {
                continue;
            }
            self.log_event(
                "translation_job_cancelled",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "source_lang": job.source_lang,
                    "source_text_len": job.source_text.len(),
                    "relevant": false,
                    "fresh": false,
                    "reason": "replaced_by_newer_sequence",
                }),
            )
            .await;
            queued_cancel_sequences.insert(job.sequence);
        }

        let mut active_cancel_sequences = HashSet::new();
        for (job_id, job_sequence, source_lang, source_text_len) in active_tasks {
            if (self.is_relevant)(job_sequence).await {
                continue;
            }
            self.log_event(
                "translation_job_cancelled",
                json!({
                    "job_id": job_id,
                    "sequence": job_sequence,
                    "source_lang": source_lang,
                    "source_text_len": source_text_len,
                    "relevant": false,
                    "fresh": false,
                    "reason": "active_job_replaced",
                }),
            )
            .await;
            active_cancel_sequences.insert(job_sequence);
        }

        if queued_cancel_sequences.is_empty() && active_cancel_sequences.is_empty() {
            return;
        }

        let mut cancel_sequences = queued_cancel_sequences;
        cancel_sequences.extend(active_cancel_sequences);

        let mut inner = self.inner.lock().await;
        let before = inner.queue.len();
        inner
            .queue
            .retain(|job| !cancel_sequences.contains(&job.sequence));
        let queued_removed = before.saturating_sub(inner.queue.len());
        inner.jobs_cancelled += queued_removed as u64;

        let mut aborted = 0u64;
        let (keep, remove): (Vec<_>, Vec<_>) = inner
            .active_tasks
            .drain(..)
            .partition(|task| !cancel_sequences.contains(&task.sequence));
        inner.active_tasks = keep;
        for task in remove {
            task.cancel.store(true, Ordering::Release);
            task.cancel_notify.notify_waiters();
            if let Some(handle) = task.handle.lock().await.take() {
                handle.abort();
            }
            aborted += 1;
        }
        inner.jobs_cancelled += aborted;
        inner.active_jobs = inner.active_jobs.saturating_sub(aborted as usize);

        if queued_removed > 0 {
            inner.last_runtime_reason = Some("cancelled:replaced_by_newer_sequence".into());
        }
        if aborted > 0 {
            inner.last_runtime_reason = Some("cancelled:active_job_replaced".into());
        }

        self.emit_metrics_locked(&mut inner);
        drop(inner);
        if aborted > 0 {
            self.notify.notify_waiters();
        }
    }

    pub async fn submit_final(
        self: &Arc<Self>,
        sequence: u64,
        source_text: &str,
        source_lang: &str,
        preview_lineage_key: Option<&str>,
    ) {
        self.cancel_older_than(sequence).await;

        let translation = self.translation_config();
        let mut inner = self.inner.lock().await;
        if inner.stopped {
            return;
        }
        inner.next_job_id += 1;
        let job_id = inner.next_job_id;
        let preview_generation = if let Some(key) = preview_lineage_key.filter(|k| !k.is_empty()) {
            inner.preview_lineage.supersede(Some(key))
        } else {
            0
        };
        let job = QueuedJob {
            job_id,
            sequence,
            source_text: source_text.to_string(),
            source_lang: source_lang.to_string(),
            preview_lineage_key: preview_lineage_key.map(str::to_string),
            preview_generation,
            submitted_at: Instant::now(),
        };
        drop(inner);
        self.enqueue_job(job, &translation).await;
        let mut inner = self.inner.lock().await;
        self.ensure_worker_locked(&mut inner);
        self.notify.notify_one();
    }

    async fn enqueue_job(self: &Arc<Self>, job: QueuedJob, translation: &Value) {
        let queue_max = Self::queue_max(translation);
        loop {
            {
                let mut inner = self.inner.lock().await;
                if inner.queue.len() < queue_max {
                    inner.queue.push_back(job);
                    self.emit_metrics_locked(&mut inner);
                    return;
                }
            }

            let sequences: Vec<u64> = {
                let inner = self.inner.lock().await;
                inner.queue.iter().map(|queued| queued.sequence).collect()
            };
            let mut irrelevant = HashSet::new();
            for sequence in sequences {
                if irrelevant.contains(&sequence) {
                    continue;
                }
                if !(self.is_relevant)(sequence).await {
                    irrelevant.insert(sequence);
                }
            }

            let dropped_job = {
                let mut inner = self.inner.lock().await;
                if inner.queue.len() < queue_max {
                    None
                } else {
                    let index = inner
                        .queue
                        .iter()
                        .position(|queued| irrelevant.contains(&queued.sequence))
                        .unwrap_or(0);
                    let dropped_job = inner
                        .queue
                        .remove(index)
                        .expect("queue overflow drop requires a queued job");
                    inner.jobs_cancelled += 1;
                    inner.last_runtime_reason = Some("cancelled:queue_overflow".into());
                    Some(dropped_job)
                }
            };
            let Some(dropped_job) = dropped_job else {
                continue;
            };
            let is_relevant = (self.is_relevant)(dropped_job.sequence).await;
            self.log_event(
                "translation_job_cancelled",
                json!({
                    "job_id": dropped_job.job_id,
                    "sequence": dropped_job.sequence,
                    "source_lang": dropped_job.source_lang,
                    "source_text_len": dropped_job.source_text.len(),
                    "relevant": is_relevant,
                    "fresh": is_relevant,
                    "reason": "queue_overflow",
                }),
            )
            .await;
            let mut inner = self.inner.lock().await;
            self.emit_metrics_locked(&mut inner);
        }
    }

    pub async fn refresh_provider_throttles(self: &Arc<Self>) {
        let mut inner = self.inner.lock().await;
        inner.provider_semaphores.clear();
        inner.provider_mutexes.clear();
        inner.provider_next_allowed_at.clear();
    }

    fn ensure_worker_locked(self: &Arc<Self>, inner: &mut DispatcherInner) {
        if inner.worker.is_some() {
            return;
        }
        let this = Arc::clone(self);
        inner.worker = Some(tokio::spawn(async move {
            this.worker_loop().await;
        }));
    }

    async fn worker_loop(self: Arc<Self>) {
        loop {
            let job = {
                let mut inner = self.inner.lock().await;
                if inner.stopped {
                    return;
                }
                if inner.active_jobs >= Self::max_concurrent_jobs(&self.config_getter)
                    || inner.queue.is_empty()
                {
                    None
                } else {
                    inner.active_jobs += 1;
                    inner.queue.pop_front()
                }
            };

            let Some(job) = job else {
                self.notify.notified().await;
                continue;
            };

            let queue_latency_ms = job.submitted_at.elapsed().as_secs_f64() * 1000.0;
            {
                let mut inner = self.inner.lock().await;
                inner.queue_latency_ms = Some(queue_latency_ms);
            }
            let this = Arc::clone(&self);
            let job_id = job.job_id;
            let job_for_task = job.clone();
            let job_cancelled = Arc::new(AtomicBool::new(false));
            let job_cancel_notify = Arc::new(Notify::new());
            {
                let mut inner = self.inner.lock().await;
                inner.active_tasks.push(ActiveTask {
                    job_id,
                    sequence: job.sequence,
                    source_lang: job.source_lang.clone(),
                    source_text_len: job.source_text.len(),
                    cancel: job_cancelled.clone(),
                    cancel_notify: job_cancel_notify.clone(),
                    handle: Mutex::new(None),
                });
            }
            let job_cancelled_run = job_cancelled.clone();
            let job_cancel_notify_run = job_cancel_notify.clone();
            let handle = tokio::spawn(async move {
                this.run_job(&job_for_task, job_cancelled_run, job_cancel_notify_run)
                    .await;
                let mut inner = this.inner.lock().await;
                let still_active = inner.active_tasks.iter().any(|task| task.job_id == job_id);
                if still_active {
                    inner.active_jobs = inner.active_jobs.saturating_sub(1);
                    inner.active_tasks.retain(|task| task.job_id != job_id);
                }
                this.emit_metrics_locked(&mut inner);
                this.notify.notify_one();
            });
            {
                let mut inner = self.inner.lock().await;
                if let Some(task) = inner
                    .active_tasks
                    .iter_mut()
                    .find(|task| task.job_id == job_id)
                {
                    *task.handle.lock().await = Some(handle);
                }
                self.jobs_started.fetch_add(1, Ordering::Relaxed);
                inner.last_runtime_reason = None;
                let timeout_ms = Self::timeout_ms(&self.translation_config());
                inner.last_timeout_ms = Some(timeout_ms);
                drop(inner);
                let is_relevant = (self.is_relevant)(job.sequence).await;
                self.log_event(
                    "translation_job_started",
                    json!({
                        "job_id": job.job_id,
                        "sequence": job.sequence,
                        "source_lang": job.source_lang,
                        "source_text_len": job.source_text.len(),
                        "queue_latency_ms": queue_latency_ms,
                        "timeout_ms": timeout_ms,
                        "relevant": is_relevant,
                        "fresh": is_relevant,
                    }),
                )
                .await;
                let mut inner = self.inner.lock().await;
                self.emit_metrics_locked(&mut inner);
            }
        }
    }

    async fn run_job(
        self: &Arc<Self>,
        job: &QueuedJob,
        job_cancelled: Arc<AtomicBool>,
        job_cancel_notify: Arc<Notify>,
    ) {
        let translation = self.translation_config();
        if !translation
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            self.log_event(
                "translation_publish_skipped",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "source_lang": job.source_lang,
                    "source_text_len": job.source_text.len(),
                    "relevant": (self.is_relevant)(job.sequence).await,
                    "fresh": (self.is_relevant)(job.sequence).await,
                    "reason": "translation_disabled",
                }),
            )
            .await;
            debug!(
                sequence = job.sequence,
                "translation disabled, skipping job"
            );
            return;
        }

        if translation
            .get("__test_fail_prepare")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let reason = translation
                .get("__test_fail_prepare_message")
                .and_then(|v| v.as_str())
                .unwrap_or("prepare_request exploded");
            let mut inner = self.inner.lock().await;
            inner.last_runtime_reason = Some(format!("job_error:{reason}"));
            drop(inner);
            self.log_event(
                "translation_job_error",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "source_lang": job.source_lang,
                    "source_text_len": job.source_text.len(),
                    "provider": translation.get("provider").and_then(|v| v.as_str()),
                    "target_languages": translation.get("target_languages"),
                    "relevant": (self.is_relevant)(job.sequence).await,
                    "fresh": (self.is_relevant)(job.sequence).await,
                    "error_type": "RuntimeError",
                    "reason": reason,
                }),
            )
            .await;
            let mut inner = self.inner.lock().await;
            self.emit_metrics_locked(&mut inner);
            return;
        }

        if !(self.is_relevant)(job.sequence).await {
            self.stale_dropped.fetch_add(1, Ordering::Relaxed);
            let mut inner = self.inner.lock().await;
            inner.last_runtime_reason = Some("stale:job_not_relevant".into());
            drop(inner);
            self.log_event(
                "translation_stale_dropped",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "source_lang": job.source_lang,
                    "source_text_len": job.source_text.len(),
                    "relevant": false,
                    "fresh": false,
                    "reason": "job_not_relevant",
                }),
            )
            .await;
            let mut inner = self.inner.lock().await;
            self.emit_metrics_locked(&mut inner);
            return;
        }

        if self.is_preview_superseded(job).await {
            self.stale_dropped.fetch_add(1, Ordering::Relaxed);
            let current_generation = {
                let inner = self.inner.lock().await;
                job.preview_lineage_key
                    .as_ref()
                    .map(|key| inner.preview_lineage.generation(key))
                    .unwrap_or(0)
            };
            let mut inner = self.inner.lock().await;
            inner.last_runtime_reason = Some("stale:preview_superseded".into());
            drop(inner);
            self.log_event(
                "translation_preview_superseded",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "preview_lineage_key": job.preview_lineage_key,
                    "job_generation": job.preview_generation,
                    "current_generation": current_generation,
                }),
            )
            .await;
            let mut inner = self.inner.lock().await;
            self.emit_metrics_locked(&mut inner);
            return;
        }

        let prepared = {
            let mut engine = self.engine.lock().await;
            engine.apply_live_settings(&translation);
            engine.prepare_request(&translation)
        };
        if prepared.lines.is_empty() {
            self.log_event(
                "translation_publish_skipped",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "source_lang": job.source_lang,
                    "source_text_len": job.source_text.len(),
                    "provider": prepared.provider_name,
                    "target_languages": prepared.target_languages,
                    "relevant": true,
                    "fresh": true,
                    "reason": "no_translation_lines",
                }),
            )
            .await;
            return;
        }

        for line in &prepared.lines {
            let timeout_ms = Self::line_timeout_ms(&translation, line.local_provider);
            self.log_event(
                "translation_line_started",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "source_lang": job.source_lang,
                    "source_text_len": job.source_text.len(),
                    "slot_id": line.slot_id,
                    "label": line.label,
                    "target_lang": line.target_lang,
                    "target_languages": prepared.target_languages,
                    "provider": line.provider_name,
                    "timeout_ms": timeout_ms,
                    "local_provider": line.local_provider,
                    "relevant": true,
                    "fresh": true,
                }),
            )
            .await;
        }

        let mut join_set = JoinSet::new();
        for line in prepared.lines.clone() {
            let this = Arc::clone(self);
            let job = job.clone();
            let timeout_ms = Self::line_timeout_ms(&translation, line.local_provider);
            let timeout_secs = timeout_ms as f64 / 1000.0;
            join_set.spawn(async move {
                this.translate_one_line(&job, &line, timeout_secs, timeout_ms)
                    .await
            });
        }

        let mut published: Vec<TranslationItem> = Vec::new();
        let mut final_status_message: Option<String> = None;
        let mut used_default_prompt = false;

        loop {
            let joined = tokio::select! {
                biased;
                () = Self::wait_for_job_cancel(
                    job_cancelled.clone(),
                    job_cancel_notify.clone(),
                ) => {
                    join_set.abort_all();
                    while join_set.join_next().await.is_some() {}
                    return;
                }
                joined = join_set.join_next() => joined,
            };
            let Some(joined) = joined else {
                break;
            };
            let result = match joined {
                Ok(result) => result,
                Err(_) => continue,
            };

            if matches!(result.outcome, LineOutcome::ProviderSkipped) {
                self.provider_skipped.fetch_add(1, Ordering::Relaxed);
                self.log_event(
                    "translation_provider_call_skipped",
                    json!({
                        "job_id": job.job_id,
                        "sequence": job.sequence,
                        "source_lang": job.source_lang,
                        "source_text_len": job.source_text.len(),
                        "slot_id": result.item.slot_id,
                        "reason": result.reason,
                        "preview_lineage_key": job.preview_lineage_key,
                        "job_generation": job.preview_generation,
                    }),
                )
                .await;
                continue;
            }

            let line_timeout_ms = Self::line_timeout_ms(&translation, result.item.local_provider);
            {
                let mut inner = self.inner.lock().await;
                inner.last_provider = Some(result.item.provider.clone());
                inner.last_slot_id = result.item.slot_id.clone();
                inner.last_target_lang = Some(result.item.target_lang.clone());
                inner.last_timeout_ms = Some(line_timeout_ms);
                inner.last_provider_latency_ms = Some(result.provider_latency_ms);
                if matches!(result.outcome, LineOutcome::Timeout | LineOutcome::Error) {
                    inner.last_runtime_reason =
                        result.reason.clone().or_else(|| result.item.error.clone());
                }
                self.emit_metrics_locked(&mut inner);
            }

            let event_name = match result.outcome {
                LineOutcome::Timeout => "translation_line_timeout",
                LineOutcome::Error => "translation_line_error",
                LineOutcome::Done => "translation_line_done",
                LineOutcome::ProviderSkipped => unreachable!(),
            };
            self.log_event(
                event_name,
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "source_lang": job.source_lang,
                    "source_text_len": job.source_text.len(),
                    "slot_id": result.item.slot_id,
                    "label": result.item.label,
                    "target_lang": result.item.target_lang,
                    "target_languages": prepared.target_languages,
                    "provider": result.item.provider,
                    "latency_ms": result.provider_latency_ms,
                    "queue_latency_ms": self
                        .inner
                        .lock()
                        .await
                        .queue_latency_ms,
                    "timeout_ms": line_timeout_ms,
                    "relevant": (self.is_relevant)(job.sequence).await,
                    "fresh": (self.is_relevant)(job.sequence).await,
                    "reason": result.reason,
                }),
            )
            .await;

            if self.is_stopped().await {
                return;
            }

            if !(self.is_relevant)(job.sequence).await {
                self.stale_dropped.fetch_add(1, Ordering::Relaxed);
                let mut inner = self.inner.lock().await;
                inner.last_runtime_reason = Some("stale:target_result_arrived_late".into());
                drop(inner);
                self.log_event(
                    "translation_stale_dropped",
                    json!({
                        "job_id": job.job_id,
                        "sequence": job.sequence,
                        "slot_id": result.item.slot_id,
                        "relevant": false,
                        "fresh": false,
                        "reason": "target_result_arrived_late",
                    }),
                )
                .await;
                let mut inner = self.inner.lock().await;
                self.emit_metrics_locked(&mut inner);
                continue;
            }

            if self.is_preview_superseded(job).await {
                self.stale_dropped.fetch_add(1, Ordering::Relaxed);
                let mut inner = self.inner.lock().await;
                inner.last_runtime_reason = Some("stale:preview_superseded".into());
                drop(inner);
                self.log_event(
                    "translation_preview_superseded",
                    json!({
                        "job_id": job.job_id,
                        "sequence": job.sequence,
                        "slot_id": result.item.slot_id,
                        "stage": "after_translate",
                    }),
                )
                .await;
                let mut inner = self.inner.lock().await;
                self.emit_metrics_locked(&mut inner);
                continue;
            }

            used_default_prompt |= result.used_default_prompt;
            let event = TranslationEvent {
                sequence: job.sequence,
                source_text: job.source_text.clone(),
                source_lang: job.source_lang.clone(),
                translations: vec![result.item.clone()],
                provider: result.item.provider.clone(),
                provider_group: result.item.provider_group.clone(),
                experimental: result.item.experimental,
                local_provider: result.item.local_provider,
                used_default_prompt: result.used_default_prompt,
                status_message: result.status_message.clone(),
                is_complete: false,
            };
            (self.publish)(event).await;
            published.push(result.item);
            if let Some(status) = result.status_message {
                final_status_message = Some(status);
            }

            self.log_event(
                "translation_publish_accepted",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "slot_id": published.last().and_then(|item| item.slot_id.clone()),
                    "relevant": true,
                    "fresh": true,
                    "reason": "target_result",
                }),
            )
            .await;
        }

        if self.is_preview_superseded(job).await {
            self.log_event(
                "translation_preview_superseded",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "stage": "completion",
                }),
            )
            .await;
            return;
        }

        let final_relevant = (self.is_relevant)(job.sequence).await;
        let stopped = self.is_stopped().await;
        if stopped || !final_relevant {
            self.log_event(
                "translation_publish_skipped",
                json!({
                    "job_id": job.job_id,
                    "sequence": job.sequence,
                    "source_lang": job.source_lang,
                    "source_text_len": job.source_text.len(),
                    "provider": prepared.provider_name,
                    "relevant": final_relevant && !stopped,
                    "fresh": final_relevant && !stopped,
                    "reason": if stopped {
                        "dispatcher_stopped"
                    } else {
                        "completion_not_relevant"
                    },
                }),
            )
            .await;
            return;
        }

        let completion_provider = completion_provider_name(&prepared, &published);
        let completion = TranslationEvent {
            sequence: job.sequence,
            source_text: job.source_text.clone(),
            source_lang: job.source_lang.clone(),
            translations: published,
            provider: completion_provider.clone(),
            provider_group: Some(prepared.provider_group.clone()),
            experimental: prepared.experimental,
            local_provider: prepared.local_provider,
            used_default_prompt,
            status_message: final_status_message.clone(),
            is_complete: true,
        };
        (self.publish)(completion).await;
        self.log_event(
            "translation_publish_accepted",
            json!({
                "job_id": job.job_id,
                "sequence": job.sequence,
                "provider": prepared.provider_name,
                "relevant": true,
                "fresh": true,
                "reason": "job_complete",
            }),
        )
        .await;
    }

    async fn translate_one_line(
        self: &Arc<Self>,
        job: &QueuedJob,
        line: &PreparedLine,
        timeout_secs: f64,
        timeout_ms: u64,
    ) -> LineResult {
        if !(self.is_relevant)(job.sequence).await {
            return LineResult {
                item: line_item_from_prepared(line, None),
                outcome: LineOutcome::ProviderSkipped,
                provider_latency_ms: 0.0,
                reason: Some("sequence_not_relevant_before_provider".into()),
                status_message: None,
                used_default_prompt: false,
            };
        }
        if self.is_preview_superseded(job).await {
            return LineResult {
                item: line_item_from_prepared(line, None),
                outcome: LineOutcome::ProviderSkipped,
                provider_latency_ms: 0.0,
                reason: Some("preview_superseded_before_provider".into()),
                status_message: None,
                used_default_prompt: false,
            };
        }

        let started_at = Instant::now();
        let line_for_task = line.clone();
        let source_text = job.source_text.clone();
        let source_lang = job.source_lang.clone();
        let engine = Arc::clone(&self.engine);
        let provider_name = line.provider_name.clone();
        let provider_mutex = self.provider_mutex(&line.provider_name).await;
        let provider_semaphore = if provider_mutex.is_none() {
            self.provider_semaphore(&line.provider_name).await
        } else {
            None
        };
        let this = Arc::clone(self);

        let timed = tokio::time::timeout(Duration::from_secs_f64(timeout_secs), async move {
            let provider_mutex = provider_mutex;
            let provider_semaphore = provider_semaphore;
            let _provider_guard = match &provider_mutex {
                Some(mutex) => Some(mutex.lock().await),
                None => None,
            };
            let _provider_permit = if _provider_guard.is_none() {
                match &provider_semaphore {
                    Some(sem) => sem.acquire().await.ok(),
                    None => None,
                }
            } else {
                None
            };
            this.provider_rate_wait(&provider_name).await;

            TranslationEngine::translate_target_concurrent(
                engine,
                &source_text,
                &source_lang,
                &line_for_task,
                TranslateTargetOptions {
                    slot_id: Some(line_for_task.slot_id.clone()),
                    label: Some(line_for_task.label.clone()),
                    budget_seconds: Some(timeout_secs),
                    retries: DEFAULT_TRANSLATION_RETRIES,
                },
            )
            .await
        })
        .await;

        match timed {
            Ok((item, diagnostics)) => {
                let status_message = diagnostics
                    .get("status_message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let used_default_prompt = diagnostics
                    .get("used_default_prompt")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let outcome = if item.success {
                    LineOutcome::Done
                } else {
                    LineOutcome::Error
                };
                let reason = if item.success {
                    Some("success".into())
                } else {
                    status_message.clone()
                };
                LineResult {
                    item,
                    outcome,
                    provider_latency_ms: started_at.elapsed().as_secs_f64() * 1000.0,
                    reason,
                    status_message,
                    used_default_prompt,
                }
            }
            Err(_) => {
                warn!(
                    sequence = job.sequence,
                    slot = %line.slot_id,
                    "translation line timeout"
                );
                LineResult {
                    item: line_item_from_prepared(
                        line,
                        Some(format!("Translation timed out after {timeout_ms} ms.")),
                    ),
                    outcome: LineOutcome::Timeout,
                    provider_latency_ms: started_at.elapsed().as_secs_f64() * 1000.0,
                    reason: Some(format!("timeout_after_{timeout_ms}_ms")),
                    status_message: Some("Translation target timed out.".into()),
                    used_default_prompt: false,
                }
            }
        }
    }

    async fn provider_mutex(self: &Arc<Self>, provider_name: &str) -> Option<Arc<Mutex<()>>> {
        let (max_concurrent, _) =
            Self::provider_limit_for(&self.translation_config(), provider_name);
        if max_concurrent != Some(1) {
            return None;
        }
        let mut inner = self.inner.lock().await;
        Some(
            inner
                .provider_mutexes
                .entry(provider_name.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone(),
        )
    }

    async fn provider_semaphore(self: &Arc<Self>, provider_name: &str) -> Option<Arc<Semaphore>> {
        let (max_concurrent, _) =
            Self::provider_limit_for(&self.translation_config(), provider_name);
        let max_concurrent = max_concurrent?;
        if max_concurrent <= 1 {
            return None;
        }
        let mut inner = self.inner.lock().await;
        if let Some((capacity, semaphore)) = inner.provider_semaphores.get(provider_name)
            && *capacity == max_concurrent
        {
            return Some(semaphore.clone());
        }
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        inner.provider_semaphores.insert(
            provider_name.to_string(),
            (max_concurrent, semaphore.clone()),
        );
        Some(semaphore)
    }

    async fn provider_rate_wait(self: &Arc<Self>, provider_name: &str) {
        let (_, min_interval_ms) =
            Self::provider_limit_for(&self.translation_config(), provider_name);
        if min_interval_ms == 0 {
            return;
        }
        let now = monotonic_now();
        let delay = {
            let mut inner = self.inner.lock().await;
            let allowed_at = inner
                .provider_next_allowed_at
                .get(provider_name)
                .copied()
                .unwrap_or(0.0);
            let delay = (allowed_at - now).max(0.0);
            inner.provider_next_allowed_at.insert(
                provider_name.to_string(),
                allowed_at.max(now) + min_interval_ms as f64 / 1000.0,
            );
            delay
        };
        if delay > 0.0 {
            tokio::time::sleep(Duration::from_secs_f64(delay)).await;
        }
    }

    async fn is_preview_superseded(self: &Arc<Self>, job: &QueuedJob) -> bool {
        let Some(ref key) = job.preview_lineage_key else {
            return false;
        };
        if job.preview_generation == 0 {
            return false;
        }
        let inner = self.inner.lock().await;
        // Any mismatch (newer generation, or missing/reset counter) means this job is stale.
        inner.preview_lineage.generation(key) != job.preview_generation
    }

    /// SST parity: `_run_job` catches `CancelledError` and cancels line tasks.
    async fn wait_for_job_cancel(job_cancelled: Arc<AtomicBool>, notify: Arc<Notify>) {
        loop {
            let notified = notify.notified();
            if job_cancelled.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }

    async fn is_stopped(&self) -> bool {
        self.inner.lock().await.stopped
    }

    fn translation_config(&self) -> Value {
        let config = (self.config_getter)();
        config.get("translation").cloned().unwrap_or(Value::Null)
    }

    async fn log_event(&self, event: &str, mut fields: Value) {
        let Some(ref logger) = self.callbacks.structured_log else {
            return;
        };
        let inner = self.inner.lock().await;
        if let Some(obj) = fields.as_object_mut() {
            obj.entry("queue_depth")
                .or_insert(json!(inner.queue.len() + inner.active_jobs));
            obj.entry("cancelled_count")
                .or_insert(json!(inner.jobs_cancelled));
            obj.entry("stale_drop_count")
                .or_insert(json!(self.stale_dropped.load(Ordering::Relaxed)));
        }
        logger("translation_dispatcher", event, fields);
    }

    fn emit_metrics_locked(self: &Arc<Self>, inner: &mut DispatcherInner) {
        let queue_depth = inner.queue.len() + inner.active_jobs;
        if let Some(ref logger) = self.callbacks.structured_log
            && inner.last_logged_queue_depth != queue_depth
        {
            inner.last_logged_queue_depth = queue_depth;
            logger(
                "translation_dispatcher",
                "translation_queue_depth_changed",
                json!({ "queue_depth": queue_depth }),
            );
        }
        if let Some(ref callback) = self.callbacks.metrics_callback {
            callback(self.metrics_from_inner(inner));
        }
    }

    fn metrics_from_inner(&self, inner: &DispatcherInner) -> Value {
        json!({
            "translation_jobs_started": self.jobs_started.load(Ordering::Relaxed),
            "translation_stale_results_dropped": self.stale_dropped.load(Ordering::Relaxed),
            "translation_jobs_cancelled": inner.jobs_cancelled,
            "translation_provider_skipped_before_call": self.provider_skipped.load(Ordering::Relaxed),
            "translation_queue_depth": inner.queue.len() + inner.active_jobs,
            "translation_queue_latency_ms": inner.queue_latency_ms,
            "translation_provider_latency_ms": inner
                .last_provider_latency_ms
                .map(|v| json!(v))
                .unwrap_or(Value::Null),
            "translation_last_slot_id": inner.last_slot_id,
            "translation_last_target_lang": inner.last_target_lang,
            "translation_last_provider": inner.last_provider,
            "translation_last_timeout_ms": inner.last_timeout_ms,
            "translation_last_runtime_reason": inner.last_runtime_reason,
        })
    }

    fn max_concurrent_jobs(config_getter: &ConfigGetter) -> usize {
        let config = (config_getter)();
        config
            .get("translation")
            .and_then(|v| v.get("max_concurrent_jobs"))
            .and_then(|v| v.as_u64())
            .map(|v| v.clamp(1, 8) as usize)
            .unwrap_or(DEFAULT_MAX_CONCURRENT)
    }

    fn timeout_ms(translation: &Value) -> u64 {
        translation
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .map(|v| v.clamp(1_000, MAX_TIMEOUT_MS))
            .unwrap_or(DEFAULT_TIMEOUT_MS)
    }

    fn line_timeout_ms(translation: &Value, local_provider: bool) -> u64 {
        let configured = Self::timeout_ms(translation);
        if local_provider {
            configured.clamp(LOCAL_LLM_MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
        } else {
            configured
        }
    }

    fn queue_max(translation: &Value) -> usize {
        translation
            .get("queue_max_size")
            .and_then(|v| v.as_u64())
            .map(|v| v.clamp(1, 64) as usize)
            .unwrap_or(DEFAULT_QUEUE_MAX)
    }

    fn provider_limit_for(translation: &Value, provider_name: &str) -> (Option<usize>, u64) {
        let limits = translation
            .get("provider_limits")
            .and_then(|v| v.as_object());
        let Some(limits) = limits else {
            return (None, 0);
        };
        let provider_cfg = limits.get(provider_name).and_then(|v| v.as_object());
        let Some(provider_cfg) = provider_cfg else {
            return (None, 0);
        };
        let max_concurrent = provider_cfg
            .get("max_concurrent_targets")
            .and_then(parse_limit_usize)
            .map(|v| v.clamp(1, 16));
        let min_interval_ms = provider_cfg
            .get("min_interval_ms")
            .and_then(parse_limit_u64)
            .unwrap_or(0)
            .clamp(0, 60_000);
        (max_concurrent, min_interval_ms)
    }

    pub fn metrics_snapshot(&self) -> Value {
        self.inner
            .try_lock()
            .map(|inner| self.metrics_from_inner(&inner))
            .unwrap_or_else(|_| json!({}))
    }
}

fn line_item_from_prepared(line: &PreparedLine, error: Option<String>) -> TranslationItem {
    TranslationItem {
        target_lang: line.target_lang.clone(),
        text: String::new(),
        provider: line.provider_name.clone(),
        slot_id: Some(line.slot_id.clone()),
        label: Some(line.label.clone()),
        provider_group: Some(line.provider_group.clone()),
        experimental: line.experimental,
        local_provider: line.local_provider,
        success: false,
        error,
        cached: false,
    }
}

fn completion_provider_name(prepared: &PreparedRequest, published: &[TranslationItem]) -> String {
    let providers: HashSet<_> = published
        .iter()
        .map(|item| item.provider.as_str())
        .filter(|name| !name.is_empty())
        .collect();
    if providers.len() <= 1 {
        prepared.provider_name.clone()
    } else {
        "mixed".into()
    }
}

fn parse_limit_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().map(|v| v.max(0) as u64))
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

fn parse_limit_usize(value: &Value) -> Option<usize> {
    parse_limit_u64(value).map(|v| v as usize)
}

fn monotonic_now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_secs_f64()
}
