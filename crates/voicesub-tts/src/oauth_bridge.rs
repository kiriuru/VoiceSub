use std::sync::Mutex;
use std::time::{Duration, Instant};

const PENDING_TTL: Duration = Duration::from_secs(600);

struct PendingOAuth {
    token: String,
    received_at: Instant,
}

#[derive(Default)]
pub struct TwitchOAuthBridge {
    pending: Mutex<Option<PendingOAuth>>,
}

impl TwitchOAuthBridge {
    pub fn store(&self, token: String) {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return;
        }
        let mut guard = self.pending.lock().expect("twitch oauth bridge lock");
        *guard = Some(PendingOAuth {
            token: trimmed.to_string(),
            received_at: Instant::now(),
        });
    }

    pub fn take(&self) -> Option<String> {
        let mut guard = self.pending.lock().expect("twitch oauth bridge lock");
        let entry = guard.take()?;
        if entry.received_at.elapsed() > PENDING_TTL {
            return None;
        }
        Some(entry.token)
    }
}
