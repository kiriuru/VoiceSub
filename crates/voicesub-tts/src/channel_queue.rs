//! Per-channel speech queues (`speech`, `twitch`) owned by the TTS service.

use std::sync::Mutex;

use tracing::debug;

use crate::queue::{SpeechQueue, SpeechQueueItem, SpeechQueueState};

pub const CHANNEL_SPEECH: &str = "speech";
pub const CHANNEL_TWITCH: &str = "twitch";

#[derive(Debug, Default)]
pub struct DualChannelSpeechQueue {
    speech: Mutex<SpeechQueue>,
    twitch: Mutex<SpeechQueue>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChannelQueueError {
    #[error("unknown speech channel: {0}")]
    UnknownChannel(String),
}

impl DualChannelSpeechQueue {
    pub fn new() -> Self {
        Self::default()
    }

    fn queue_for(&self, channel: &str) -> Result<&Mutex<SpeechQueue>, ChannelQueueError> {
        match channel {
            CHANNEL_SPEECH => Ok(&self.speech),
            CHANNEL_TWITCH => Ok(&self.twitch),
            other => Err(ChannelQueueError::UnknownChannel(other.to_string())),
        }
    }

    pub fn enqueue(
        &self,
        channel: &str,
        item: SpeechQueueItem,
        max_items: u32,
    ) -> Result<usize, ChannelQueueError> {
        let queue = self.queue_for(channel)?;
        let mut guard = queue.lock().expect("speech queue lock");
        guard.enqueue_with_cap(item, max_items);
        Ok(guard.len())
    }

    pub fn begin_next(&self, channel: &str) -> Result<Option<SpeechQueueItem>, ChannelQueueError> {
        let queue = self.queue_for(channel)?;
        Ok(queue.lock().expect("speech queue lock").begin_next())
    }

    pub fn mark_finished(&self, channel: &str, item_id: &str) -> Result<(), ChannelQueueError> {
        let queue = self.queue_for(channel)?;
        queue
            .lock()
            .expect("speech queue lock")
            .mark_finished(item_id);
        Ok(())
    }

    pub fn clear(&self, channel: &str) -> Result<(), ChannelQueueError> {
        let queue = self.queue_for(channel)?;
        queue.lock().expect("speech queue lock").clear();
        debug!(target: "voicesub.tts", channel, "channel queue cleared");
        Ok(())
    }

    pub fn clear_all(&self) {
        self.speech.lock().expect("speech queue lock").clear();
        self.twitch.lock().expect("speech queue lock").clear();
        debug!(target: "voicesub.tts", "all channel queues cleared");
    }

    pub fn snapshot(&self, channel: &str) -> Result<Vec<SpeechQueueItem>, ChannelQueueError> {
        let queue = self.queue_for(channel)?;
        Ok(queue.lock().expect("speech queue lock").snapshot())
    }

    pub fn state(&self, channel: &str) -> Result<SpeechQueueState, ChannelQueueError> {
        let queue = self.queue_for(channel)?;
        Ok(queue.lock().expect("speech queue lock").state())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::SpeechQueueItem;

    fn item(id: &str) -> SpeechQueueItem {
        SpeechQueueItem {
            id: id.to_string(),
            text: "hello".to_string(),
            source: String::new(),
            lang: "en".to_string(),
        }
    }

    #[test]
    fn channels_are_independent() {
        let queues = DualChannelSpeechQueue::new();
        queues
            .enqueue(CHANNEL_SPEECH, item("s1"), 8)
            .expect("speech enqueue");
        queues
            .enqueue(CHANNEL_TWITCH, item("t1"), 8)
            .expect("twitch enqueue");
        assert_eq!(
            queues
                .begin_next(CHANNEL_SPEECH)
                .expect("speech begin")
                .expect("item")
                .id,
            "s1"
        );
        assert_eq!(
            queues
                .begin_next(CHANNEL_TWITCH)
                .expect("twitch begin")
                .expect("item")
                .id,
            "t1"
        );
    }
}
