//! Shared partial emit policy — SST `partial_emit_coordinator` port.
//!
//! Used by browser runtime (`voicesub-runtime`) and local Parakeet (`voicesub-asr-local`).

mod coordinator;
mod policy;
mod settings;

pub use coordinator::PartialEmitCoordinator;
pub use policy::{
    normalize_transcript_text, should_emit_partial, split_words, PartialEmitInput,
};
pub use settings::{PartialEmitSettings, partial_emit_settings_from_config};
