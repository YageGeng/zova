use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl Model {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
        }
    }

    pub fn from_id(id: impl Into<String>) -> Self {
        let id = id.into();
        Self::new(id.clone(), id)
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCatalogSource {
    ProviderApi,
    CacheFresh,
    CacheStaleFallback,
    StaticFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalog {
    pub models: Vec<Model>,
    pub source: ModelCatalogSource,
    pub warning: Option<String>,
}

impl ModelCatalog {
    pub fn from_provider_api(models: Vec<Model>) -> Self {
        Self {
            models,
            source: ModelCatalogSource::ProviderApi,
            warning: None,
        }
    }

    pub fn from_cache_fresh(models: Vec<Model>) -> Self {
        Self {
            models,
            source: ModelCatalogSource::CacheFresh,
            warning: None,
        }
    }

    pub fn from_cache_stale(models: Vec<Model>, warning: String) -> Self {
        Self {
            models,
            source: ModelCatalogSource::CacheStaleFallback,
            warning: Some(warning),
        }
    }

    pub fn from_static_fallback(models: Vec<Model>, warning: String) -> Self {
        Self {
            models,
            source: ModelCatalogSource::StaticFallback,
            warning: Some(warning),
        }
    }
}

struct CacheEntry {
    models: Vec<Model>,
    fetched_at: Instant,
}

pub struct ModelCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
}

impl ModelCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(60 * 60))
    }

    pub async fn get_fresh(&self, provider_id: &str) -> Option<Vec<Model>> {
        let entries = self.entries.read().await;
        entries.get(provider_id).and_then(|entry| {
            if entry.fetched_at.elapsed() < self.ttl {
                Some(entry.models.clone())
            } else {
                None
            }
        })
    }

    pub async fn get_any(&self, provider_id: &str) -> Option<Vec<Model>> {
        let entries = self.entries.read().await;
        entries.get(provider_id).map(|entry| entry.models.clone())
    }

    pub async fn set(&self, provider_id: &str, models: Vec<Model>) {
        let mut entries = self.entries.write().await;
        entries.insert(
            provider_id.to_string(),
            CacheEntry {
                models,
                fetched_at: Instant::now(),
            },
        );
    }
}

static MODEL_CACHE: OnceLock<Arc<ModelCache>> = OnceLock::new();

pub fn get_model_cache() -> Arc<ModelCache> {
    MODEL_CACHE
        .get_or_init(|| Arc::new(ModelCache::with_default_ttl()))
        .clone()
}

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";

pub fn default_openai_models() -> Vec<Model> {
    vec![
        Model::from_id("gpt-4o-mini").with_description("Balanced cost/performance default"),
        Model::from_id("gpt-4o").with_description("High quality general model"),
        Model::from_id("gpt-4.1").with_description("Reasoning-forward GPT-4.1"),
        Model::from_id("o3").with_description("Advanced reasoning model"),
    ]
}
