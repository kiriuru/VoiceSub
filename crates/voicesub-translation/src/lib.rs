//! Translation dispatcher, engine, and provider registry (SST port).

mod cache;
mod dispatcher;
mod engine;
mod readiness;
mod runtime_coordinator;
mod preview_lineage;
mod providers;
mod runtime;

pub use dispatcher::{
    ConfigGetter, DispatcherCallbacks, MetricsCallback, PublishFn, RelevanceFn, StructuredLogFn,
    TranslationDispatcher,
};
pub use engine::{
    PreparedLine, PreparedRequest, TranslateTargetOptions, TranslationBatch, TranslationEngine,
};
pub use readiness::summarize_readiness;
pub use runtime_coordinator::{
    summarize_translation_diagnostics, translation_diagnostics_error,
};
pub use preview_lineage::TranslationPreviewLineage;
pub use providers::{
    build_default_registry, build_translation_http_client, GoogleCloudTranslationV3Provider,
    ProviderError, ProviderInfo, SharedHttpClient, StubTranslationProvider, TranslateRequest,
    TranslationProvider, DEFAULT_HTTP_CONNECT_TIMEOUT_SECONDS,
    DEFAULT_HTTP_KEEPALIVE_EXPIRY_SECONDS, DEFAULT_HTTP_KEEPALIVE_LIMIT, DEFAULT_HTTP_TOTAL_LIMIT,
    DEFAULT_REQUEST_TIMEOUT_SECONDS, SUPPORTED_PROVIDERS,
};
pub use runtime::{arc_publish, arc_relevance, TranslationRuntimeController};
