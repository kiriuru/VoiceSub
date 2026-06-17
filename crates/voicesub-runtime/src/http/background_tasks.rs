//! Tracks long-lived runtime background tasks for diagnostics.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};

#[derive(Debug, Default)]
pub struct BackgroundTaskRegistry {
    http_server: AtomicBool,
    runtime_heartbeat: AtomicBool,
    startup_check: AtomicBool,
}

impl BackgroundTaskRegistry {
    pub fn set_http_server(&self, active: bool) {
        self.http_server.store(active, Ordering::Relaxed);
    }

    pub fn set_runtime_heartbeat(&self, active: bool) {
        self.runtime_heartbeat.store(active, Ordering::Relaxed);
    }

    pub fn set_startup_check(&self, active: bool) {
        self.startup_check.store(active, Ordering::Relaxed);
    }

    pub fn active_count(&self) -> u32 {
        [
            self.http_server.load(Ordering::Relaxed),
            self.runtime_heartbeat.load(Ordering::Relaxed),
            self.startup_check.load(Ordering::Relaxed),
        ]
        .iter()
        .filter(|active| **active)
        .count() as u32
    }

    pub fn snapshot(&self) -> Value {
        json!({
            "http_server": self.http_server.load(Ordering::Relaxed),
            "runtime_heartbeat": self.runtime_heartbeat.load(Ordering::Relaxed),
            "startup_check": self.startup_check.load(Ordering::Relaxed),
            "active_count": self.active_count(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reflects_active_tasks() {
        let registry = BackgroundTaskRegistry::default();
        registry.set_http_server(true);
        registry.set_runtime_heartbeat(true);
        let snap = registry.snapshot();
        assert_eq!(snap["active_count"], 2);
        assert_eq!(snap["http_server"], true);
    }
}
