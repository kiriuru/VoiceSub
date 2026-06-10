//! Port of SST `BrowserAsrOperationalFsm` (L2 operational lifecycle).

use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserOperationalPhase {
    Idle,
    WorkerSocketIdle,
    WorkerLive,
    IngestPartial,
    IngestFinal,
    DegradedHint,
}

impl BrowserOperationalPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::WorkerSocketIdle => "worker_socket_idle",
            Self::WorkerLive => "worker_live",
            Self::IngestPartial => "ingest_partial",
            Self::IngestFinal => "ingest_final",
            Self::DegradedHint => "degraded_hint",
        }
    }
}

#[derive(Debug)]
pub struct BrowserAsrOperationalFsm {
    phase: BrowserOperationalPhase,
}

impl Default for BrowserAsrOperationalFsm {
    fn default() -> Self {
        Self {
            phase: BrowserOperationalPhase::Idle,
        }
    }
}

impl BrowserAsrOperationalFsm {
    pub fn phase(&self) -> BrowserOperationalPhase {
        self.phase
    }

    pub fn reset(&mut self) {
        self.phase = BrowserOperationalPhase::Idle;
    }

    pub fn note_worker_connected(&mut self) {
        self.transition(
            BrowserOperationalPhase::WorkerSocketIdle,
            "worker_connected",
            true,
        );
    }

    pub fn note_worker_disconnected(&mut self) {
        self.transition(BrowserOperationalPhase::Idle, "worker_disconnected", true);
    }

    pub fn note_status_aggregate(&mut self, worker_connected: bool, degraded_reason: Option<&str>) {
        if !worker_connected {
            return;
        }
        let target = if degraded_reason.is_some() {
            BrowserOperationalPhase::DegradedHint
        } else {
            BrowserOperationalPhase::WorkerLive
        };
        self.transition(target, "status_aggregate", false);
    }

    pub fn note_ingest(&mut self, is_final: bool) {
        let target = if is_final {
            BrowserOperationalPhase::IngestFinal
        } else {
            BrowserOperationalPhase::IngestPartial
        };
        self.transition(target, "ingest", true);
    }

    fn transition(&mut self, new_phase: BrowserOperationalPhase, reason: &str, log_always: bool) {
        let old = self.phase;
        if new_phase == old && !log_always {
            return;
        }
        self.phase = new_phase;
        debug!(
            from = old.as_str(),
            to = new_phase.as_str(),
            reason,
            "browser_asr_fsm_transition"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_connect_and_ingest_final() {
        let mut fsm = BrowserAsrOperationalFsm::default();
        assert_eq!(fsm.phase(), BrowserOperationalPhase::Idle);
        fsm.note_worker_connected();
        assert_eq!(fsm.phase(), BrowserOperationalPhase::WorkerSocketIdle);
        fsm.note_ingest(true);
        assert_eq!(fsm.phase(), BrowserOperationalPhase::IngestFinal);
        fsm.note_worker_disconnected();
        assert_eq!(fsm.phase(), BrowserOperationalPhase::Idle);
    }
}
