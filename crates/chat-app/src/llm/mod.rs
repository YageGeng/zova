use std::sync::Arc;

mod model;
mod provider;
mod rig_adapter;

pub use model::{
    DEFAULT_OPENAI_MODEL, Model, ModelCache, ModelCatalog, ModelCatalogSource,
    default_openai_models, get_model_cache,
};
pub use provider::{
    LlmProvider, ProviderConfig, ProviderError, ProviderEventStream, ProviderMessage,
    ProviderResult, ProviderStreamHandle, ProviderWorker, StreamRequest,
};
pub use rig_adapter::{RIG_OPENAI_PROVIDER_ID, RigProviderAdapter};

pub fn create_provider(mut config: ProviderConfig) -> ProviderResult<Arc<dyn LlmProvider>> {
    if config.provider_id.trim().is_empty() {
        config.provider_id = RIG_OPENAI_PROVIDER_ID.to_string();
    }

    match config.provider_id.as_str() {
        "openai" | "rig-openai" => {
            config.provider_id = RIG_OPENAI_PROVIDER_ID.to_string();
            Ok(Arc::new(RigProviderAdapter::new(config)?))
        }
        _ => Err(ProviderError::UnsupportedProvider {
            stage: "create-provider",
            provider_id: config.provider_id,
        }),
    }
}

pub async fn fetch_models_for_provider(provider: &dyn LlmProvider) -> ProviderResult<ModelCatalog> {
    provider.fetch_models().await
}
