use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use reqwest::Client;
use serde_json::Value;
use voicesub_subtitle::TranslationEvent;

use crate::dispatcher::{
    ConfigGetter, DispatcherCallbacks, PublishFn, RelevanceFn, TranslationDispatcher,
};
use crate::engine::TranslationEngine;
use crate::providers::build_translation_http_client;
use crate::runtime_coordinator::{
    summarize_translation_diagnostics, translation_diagnostics_error,
};

pub struct TranslationRuntimeController {
    config_getter: ConfigGetter,
    publish: PublishFn,
    is_relevant: RelevanceFn,
    client: Client,
    cache_dir: Option<PathBuf>,
    engine: Option<TranslationEngine>,
    dispatcher: Option<Arc<TranslationDispatcher>>,
}

impl TranslationRuntimeController {
    pub fn new(
        config_getter: ConfigGetter,
        publish: PublishFn,
        is_relevant: RelevanceFn,
        cache_dir: Option<PathBuf>,
    ) -> Self {
        let client = build_translation_http_client();
        Self {
            config_getter,
            publish,
            is_relevant,
            client: client.clone(),
            cache_dir: cache_dir.clone(),
            engine: Some(TranslationEngine::new(client, cache_dir)),
            dispatcher: None,
        }
    }

    pub async fn start(&mut self, callbacks: DispatcherCallbacks) {
        let translation = self.translation_config();
        if let Some(engine) = self.engine.as_mut() {
            engine.apply_live_settings(&translation);
        }
        if self.dispatcher.is_none() {
            let engine = self.engine.take().unwrap_or_else(|| {
                TranslationEngine::new(self.client.clone(), self.cache_dir.clone())
            });
            let dispatcher = TranslationDispatcher::with_callbacks(
                engine,
                self.config_getter.clone(),
                self.publish.clone(),
                self.is_relevant.clone(),
                callbacks,
            );
            dispatcher.start().await;
            self.dispatcher = Some(dispatcher);
        }
    }

    pub async fn stop(&mut self) {
        if let Some(dispatcher) = self.dispatcher.take() {
            dispatcher.stop().await;
        }
        if self.engine.is_none() {
            self.engine = Some(TranslationEngine::new(
                self.client.clone(),
                self.cache_dir.clone(),
            ));
        }
    }

    pub fn apply_live_settings(&mut self) {
        let translation = self.translation_config();
        if let Some(dispatcher) = &self.dispatcher {
            if let Ok(mut engine) = dispatcher.engine_handle().try_lock() {
                engine.apply_live_settings(&translation);
            }
            return;
        }
        if let Some(engine) = self.engine.as_mut() {
            engine.apply_live_settings(&translation);
        }
    }

    pub async fn submit_final(
        &mut self,
        sequence: u64,
        source_text: &str,
        source_lang: &str,
        preview_lineage_key: Option<&str>,
    ) {
        if self.dispatcher.is_none() {
            self.start(DispatcherCallbacks::default()).await;
        }
        if let Some(dispatcher) = &self.dispatcher {
            dispatcher
                .submit_final(sequence, source_text, source_lang, preview_lineage_key)
                .await;
        }
    }

    pub fn diagnostics_snapshot(&self) -> Value {
        let config = (self.config_getter)();
        let translation = config.get("translation").cloned().unwrap_or(Value::Null);
        let readiness = if let Some(dispatcher) = &self.dispatcher {
            let handle = dispatcher.engine_handle();

            match handle.try_lock() {
                Ok(guard) => guard.summarize_readiness(&translation),
                Err(err) => translation_diagnostics_error(err.to_string()),
            }
        } else if let Some(engine) = &self.engine {
            engine.summarize_readiness(&translation)
        } else {
            translation_diagnostics_error("translation engine unavailable")
        };
        let metrics = self
            .dispatcher
            .as_ref()
            .map(|d| d.metrics_snapshot())
            .unwrap_or_else(|| Value::Object(Default::default()));
        summarize_translation_diagnostics(&translation, readiness, &metrics)
    }

    fn translation_config(&self) -> Value {
        let config = (self.config_getter)();
        config.get("translation").cloned().unwrap_or(Value::Null)
    }
}

pub fn arc_publish<F, Fut>(handler: F) -> PublishFn
where
    F: Fn(TranslationEvent) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    Arc::new(move |event| Box::pin(handler(event)))
}

pub fn arc_relevance<F, Fut>(handler: F) -> RelevanceFn
where
    F: Fn(u64) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = bool> + Send + 'static,
{
    Arc::new(move |sequence| Box::pin(handler(sequence)))
}
