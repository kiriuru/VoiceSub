//! SST `segment_queue.py` — coalesce partials, prioritize finals, bounded backlog.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrWorkKind {
    Partial,
    Final,
}

#[derive(Debug, Clone)]
pub struct AsrWorkItem {
    pub kind: AsrWorkKind,
    pub audio: Vec<f32>,
    pub duration_ms: u32,
    pub generation: u64,
    pub segment_id: String,
    pub revision: u64,
    pub vad_ms: f32,
    pub created_at: Instant,
    pub audio_is_delta: bool,
}

impl AsrWorkItem {
    pub fn new(
        kind: AsrWorkKind,
        audio: Vec<f32>,
        duration_ms: u32,
        generation: u64,
        segment_id: String,
        revision: u64,
    ) -> Self {
        Self {
            kind,
            audio,
            duration_ms,
            generation,
            segment_id,
            revision,
            vad_ms: 0.0,
            created_at: Instant::now(),
            audio_is_delta: false,
        }
    }
}

#[derive(Default)]
struct QueueInner {
    items: VecDeque<AsrWorkItem>,
    partial_jobs_dropped: u64,
    partial_jobs_coalesced: u64,
    finals_prioritized_count: u64,
    wake_counter: u64,
}

pub struct AsrSegmentQueue {
    inner: Mutex<QueueInner>,
    notify: Condvar,
    maxsize: usize,
}

impl AsrSegmentQueue {
    pub fn new(maxsize: usize) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(QueueInner::default()),
            notify: Condvar::new(),
            maxsize: maxsize.max(1),
        })
    }

    pub fn push(&self, mut item: AsrWorkItem) {
        if item.created_at.elapsed().is_zero() {
            item.created_at = Instant::now();
        }
        let mut guard = self.inner.lock().expect("segment queue lock");
        if item.kind == AsrWorkKind::Partial && !item.segment_id.is_empty() {
            // Merge pruned delta PCM into the newer job so backpressure never drops audio.
            Self::prune_redundant_partials_locked(&mut guard, &mut item);
        } else if item.kind == AsrWorkKind::Final
            && !item.segment_id.is_empty()
            && !(item.audio_is_delta && item.audio.is_empty())
        {
            Self::prune_redundant_partials_locked(&mut guard, &mut item);
        }
        if guard.items.len() >= self.maxsize {
            let dropped_existing_partial = Self::drop_oldest_partial_locked(&mut guard);
            if !dropped_existing_partial {
                if item.kind == AsrWorkKind::Partial {
                    guard.partial_jobs_dropped += 1;
                    self.notify.notify_all();
                    return;
                }
                guard.items.pop_front();
            }
        }
        guard.items.push_back(item);
        self.notify.notify_one();
    }

    pub fn pop(&self, timeout_ms: u64) -> Option<AsrWorkItem> {
        let mut guard = self.inner.lock().expect("segment queue lock");
        let wake_counter = guard.wake_counter;
        let deadline = Instant::now() + std::time::Duration::from_millis(timeout_ms);
        while guard.items.is_empty() {
            if guard.wake_counter != wake_counter {
                return None;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            guard = self
                .notify
                .wait_timeout(guard, remaining)
                .expect("segment queue wait")
                .0;
        }
        Self::pop_next_locked(&mut guard)
    }

    pub(crate) fn clear(&self) {
        let mut guard = self.inner.lock().expect("segment queue lock");
        guard.items.clear();
        guard.wake_counter += 1;
        self.notify.notify_all();
    }

    pub fn wake(&self) {
        let mut guard = self.inner.lock().expect("segment queue lock");
        guard.wake_counter += 1;
        self.notify.notify_all();
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("segment queue lock").items.len()
    }

    pub fn partial_jobs_dropped(&self) -> u64 {
        self.inner
            .lock()
            .expect("segment queue lock")
            .partial_jobs_dropped
    }

    pub fn partial_jobs_coalesced(&self) -> u64 {
        self.inner
            .lock()
            .expect("segment queue lock")
            .partial_jobs_coalesced
    }

    pub fn queue_depth(&self) -> usize {
        self.len()
    }

    fn prune_redundant_partials_locked(guard: &mut QueueInner, item: &mut AsrWorkItem) {
        if guard.items.is_empty() {
            return;
        }
        let mut removed_count = 0u64;
        let mut merged_delta = Vec::<f32>::new();
        let merge_deltas = item.audio_is_delta;
        let retained: VecDeque<AsrWorkItem> = guard
            .items
            .drain(..)
            .filter(|existing| {
                if existing.segment_id == item.segment_id && existing.kind == AsrWorkKind::Partial {
                    if merge_deltas && existing.audio_is_delta && !existing.audio.is_empty() {
                        merged_delta.extend_from_slice(&existing.audio);
                    }
                    removed_count += 1;
                    false
                } else {
                    true
                }
            })
            .collect();
        guard.items = retained;
        if !merged_delta.is_empty() {
            merged_delta.extend_from_slice(&item.audio);
            item.audio = merged_delta;
        }
        if removed_count > 0 {
            guard.partial_jobs_coalesced += removed_count;
        }
    }

    fn drop_oldest_partial_locked(guard: &mut QueueInner) -> bool {
        let mut removed = false;
        let retained: VecDeque<AsrWorkItem> = guard
            .items
            .drain(..)
            .filter(|existing| {
                if !removed && existing.kind == AsrWorkKind::Partial {
                    removed = true;
                    guard.partial_jobs_dropped += 1;
                    false
                } else {
                    true
                }
            })
            .collect();
        guard.items = retained;
        removed
    }

    fn is_deferred_empty_delta_final(item: &AsrWorkItem, items: &VecDeque<AsrWorkItem>) -> bool {
        if item.kind != AsrWorkKind::Final || !item.audio_is_delta || !item.audio.is_empty() {
            return false;
        }
        let segment_id = item.segment_id.trim();
        if segment_id.is_empty() {
            return false;
        }
        items.iter().any(|existing| {
            existing.kind == AsrWorkKind::Partial && existing.segment_id == segment_id
        })
    }

    fn pop_next_locked(guard: &mut QueueInner) -> Option<AsrWorkItem> {
        if guard.items.is_empty() {
            return None;
        }
        if let Some(index) = guard.items.iter().position(|item| {
            item.kind == AsrWorkKind::Final
                && !Self::is_deferred_empty_delta_final(item, &guard.items)
        }) {
            let item = guard.items.remove(index).expect("final index");
            guard.finals_prioritized_count += 1;
            return Some(item);
        }
        guard.items.pop_front()
    }
}

pub struct RuntimeGeneration {
    value: AtomicU64,
}

impl RuntimeGeneration {
    pub fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }

    pub fn current(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    pub fn bump(&self) -> u64 {
        self.value.fetch_add(1, Ordering::SeqCst) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_partials_for_same_segment() {
        let queue = AsrSegmentQueue::new(64);
        queue.push(AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.1; 100],
            100,
            1,
            "seg-1".into(),
            1,
        ));
        queue.push(AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.2; 200],
            200,
            1,
            "seg-1".into(),
            2,
        ));
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.partial_jobs_coalesced(), 1);
    }

    #[test]
    fn coalesced_delta_partials_merge_audio() {
        let queue = AsrSegmentQueue::new(64);
        let mut first = AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.1; 100],
            100,
            1,
            "seg-1".into(),
            1,
        );
        first.audio_is_delta = true;
        queue.push(first);
        let mut second = AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.2; 50],
            150,
            1,
            "seg-1".into(),
            2,
        );
        second.audio_is_delta = true;
        queue.push(second);
        assert_eq!(queue.len(), 1);
        let item = queue.pop(0).expect("merged partial");
        assert!(item.audio_is_delta);
        assert_eq!(item.audio.len(), 150);
        assert!(
            item.audio[..100]
                .iter()
                .all(|s| (*s - 0.1).abs() < f32::EPSILON)
        );
        assert!(
            item.audio[100..]
                .iter()
                .all(|s| (*s - 0.2).abs() < f32::EPSILON)
        );
    }

    #[test]
    fn prioritizes_final_over_partial() {
        let queue = AsrSegmentQueue::new(64);
        queue.push(AsrWorkItem::new(
            AsrWorkKind::Partial,
            vec![0.1; 100],
            100,
            1,
            "seg-1".into(),
            1,
        ));
        queue.push(AsrWorkItem::new(
            AsrWorkKind::Final,
            vec![0.2; 200],
            200,
            1,
            "seg-1".into(),
            2,
        ));
        let item = queue.pop(100).expect("item");
        assert_eq!(item.kind, AsrWorkKind::Final);
    }
}
