//! Live Local ASR → subtitle pipeline via the shared browser speech ingest path.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use voicesub_asr_local::{LocalAsrModuleService, PipelineEmit, RuntimeSessionError};
use voicesub_browser::IngestedAsrUpdate;

use crate::browser_speech_source::OrderedBrowserSpeechIngest;

const LOCAL_ASR_SESSION_ID: &str = "local-asr";

pub fn pipeline_emit_to_ingested(emit: PipelineEmit, generation_id: u64) -> IngestedAsrUpdate {
    let (partial, final_text) = if emit.is_final {
        (String::new(), emit.text)
    } else {
        (emit.text, String::new())
    };
    IngestedAsrUpdate {
        partial,
        final_text,
        is_final: emit.is_final,
        source_lang: None,
        generation_id,
        session_id: Some(LOCAL_ASR_SESSION_ID.into()),
        client_segment_id: Some(emit.segment_id),
        forced_final: false,
        worker_message_sequence: None,
    }
}

pub struct SharedLocalAsrSpeechSource {
    local_asr: Arc<LocalAsrModuleService>,
    ordered_ingest: Arc<OrderedBrowserSpeechIngest>,
    generation: AtomicU64,
}

impl SharedLocalAsrSpeechSource {
    pub fn new(
        local_asr: Arc<LocalAsrModuleService>,
        ordered_ingest: Arc<OrderedBrowserSpeechIngest>,
    ) -> Arc<Self> {
        Arc::new(Self {
            local_asr,
            ordered_ingest,
            generation: AtomicU64::new(0),
        })
    }

    pub fn is_running(&self) -> bool {
        self.local_asr.runtime_capture_running()
    }

    pub fn start(&self) -> Result<(), String> {
        if self.is_running() {
            return Ok(());
        }
        if !self.local_asr.inference_snapshot().model_loaded {
            self.local_asr.load_model().map_err(|err| err.to_string())?;
        }
        let generation_id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let ingest = Arc::clone(&self.ordered_ingest);
        self.local_asr
            .start_runtime_capture(Arc::new(move |emit| {
                ingest.enqueue(pipeline_emit_to_ingested(emit, generation_id));
            }))
            .map_err(runtime_session_error_message)
    }

    pub fn stop(&self) -> Result<(), String> {
        if self.is_running() {
            self.local_asr
                .stop_runtime_capture()
                .map_err(runtime_session_error_message)?;
        }
        // SST runtime stop always unloads Parakeet — independent of keep_model_loaded.
        self.local_asr.unload_model_after_runtime_stop();
        Ok(())
    }
}

fn runtime_session_error_message(err: RuntimeSessionError) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_partial_emit_to_ingested_update() {
        let update = pipeline_emit_to_ingested(
            PipelineEmit {
                segment_id: "seg-1".into(),
                revision: 1,
                text: "hello".into(),
                is_final: false,
                is_speech: true,
            },
            3,
        );
        assert_eq!(update.partial, "hello");
        assert!(update.final_text.is_empty());
        assert!(!update.is_final);
        assert_eq!(update.generation_id, 3);
        assert_eq!(update.client_segment_id.as_deref(), Some("seg-1"));
    }

    #[test]
    fn maps_final_emit_to_ingested_update() {
        let update = pipeline_emit_to_ingested(
            PipelineEmit {
                segment_id: "seg-2".into(),
                revision: 2,
                text: "done".into(),
                is_final: true,
                is_speech: false,
            },
            5,
        );
        assert!(update.partial.is_empty());
        assert_eq!(update.final_text, "done");
        assert!(update.is_final);
    }
}
