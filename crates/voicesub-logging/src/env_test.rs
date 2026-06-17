//! Shared env isolation for diagnostic / session logging tests.

use std::sync::{Mutex, MutexGuard};

use crate::set_config_full_logging_enabled;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Serializes tests that read or mutate process environment / logging flags.
pub fn lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn clear_env_vars(keys: &[&str]) {
    for key in keys {
        unsafe {
            std::env::remove_var(key);
        }
    }
}

/// Compact backbone default — deep diagnostics off, no trace env overrides.
pub fn reset_compact_logging_env() {
    set_config_full_logging_enabled(false);
    clear_env_vars(DEEP_DIAGNOSTICS_KEYS);
}

pub const DEEP_DIAGNOSTICS_KEYS: &[&str] = &["VOICESUB_DEEP_DIAGNOSTICS", "SST_DEEP_DIAGNOSTICS"];

pub const RUNTIME_VERBOSE_KEYS: &[&str] = &[
    "VOICESUB_DEEP_DIAGNOSTICS",
    "SST_DEEP_DIAGNOSTICS",
    "VOICESUB_TRACE_RUNTIME_EVENTS_VERBOSE",
    "SST_TRACE_RUNTIME_EVENTS_VERBOSE",
];
