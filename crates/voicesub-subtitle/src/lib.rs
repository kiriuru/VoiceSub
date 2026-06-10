//! Subtitle lifecycle FSM and router (SST `SubtitleRouter` port).

mod lifecycle;
mod overlay_broadcaster;
mod presentation;
mod router;
mod router_actor;
mod style;
mod trace;
mod types;

pub use overlay_broadcaster::OverlayBroadcaster;
pub use router::{ConfigGetter, PublishCallback, SubtitleRouter};
pub use style::subtitle_style_presets;
pub use trace::{structured_log_from_runtime_logger, StructuredLogFn, SubtitleLog};
pub use types::{
    LifecycleState, SubtitleLineItem, SubtitlePayloadEvent, TranscriptEvent, TranscriptKind,
    TranscriptSegment, TranslationEvent, TranslationItem,
};
