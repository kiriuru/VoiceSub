//! Translation dispatcher, engine, and provider registry (SST port).

mod cache;
mod dispatcher;
mod engine;
mod preview_lineage;
mod providers;
mod readiness;
mod runtime;
mod runtime_coordinator;

pub use dispatcher::{
    ConfigGetter, DispatcherCallbacks, MetricsCallback, PublishFn, RelevanceFn, StructuredLogFn,
    TranslationDispatcher,
};
pub use engine::{
    PreparedLine, PreparedRequest, TranslateTargetOptions, TranslationBatch, TranslationEngine,
};
pub use preview_lineage::TranslationPreviewLineage;
pub use providers::{
    DEFAULT_HTTP_CONNECT_TIMEOUT_SECONDS, DEFAULT_HTTP_KEEPALIVE_EXPIRY_SECONDS,
    DEFAULT_HTTP_KEEPALIVE_LIMIT, DEFAULT_REQUEST_TIMEOUT_SECONDS, MAX_HTTP_REQUEST_TIMEOUT_SECONDS,
    GoogleCloudTranslationV3Provider, ProviderError, ProviderInfo, SUPPORTED_PROVIDERS,
    SharedHttpClient, StubTranslationProvider, TranslateRequest, TranslationProvider,
    build_default_registry, build_translation_http_client, effective_request_timeout,
};
pub use readiness::summarize_readiness;
pub use runtime::{TranslationRuntimeController, arc_publish, arc_relevance};
pub use runtime_coordinator::{summarize_translation_diagnostics, translation_diagnostics_error};
