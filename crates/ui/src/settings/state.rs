use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use figment::{
    Figment,
    providers::{Format, Json, Serialized},
};
use gpui::*;
use gpui_component::{Theme, ThemeMode, ThemeRegistry};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use snafu::{ResultExt, Snafu};
use zova_llm::{DEFAULT_OPENAI_MODEL, Model, ProviderConfig};

pub const DEFAULT_PROVIDER_ID: &str = "openai";
pub const DEFAULT_ENDPOINT: &str = "https://api.openai.com/v1";
pub const SETTINGS_DIRECTORY_NAME: &str = "zova";
pub const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSettings {
    pub model_name: String,
    #[serde(default)]
    pub max_completion_tokens: Option<u64>,
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            model_name: DEFAULT_OPENAI_MODEL.to_string(),
            max_completion_tokens: None,
            max_output_tokens: None,
            max_tokens: None,
        }
    }
}

impl ModelSettings {
    fn normalized(mut self) -> Option<Self> {
        self.model_name = self.model_name.trim().to_string();
        if self.model_name.is_empty() {
            return None;
        }

        Some(self)
    }

    pub fn as_selector_model(&self) -> Model {
        let mut model = Model::from_id(self.model_name.clone());
        if let Some(description) = self.token_limits_description() {
            model = model.with_description(description);
        }
        model
    }

    fn token_limits_description(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(value) = self.max_completion_tokens {
            parts.push(format!("max_completion_tokens={value}"));
        }
        if let Some(value) = self.max_output_tokens {
            parts.push(format!("max_output_tokens={value}"));
        }
        if let Some(value) = self.max_tokens {
            parts.push(format!("max_tokens={value}"));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSettings {
    #[serde(default = "default_provider_id")]
    pub provider_id: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_models")]
    pub models: Vec<ModelSettings>,
    #[serde(
        default = "default_theme_mode",
        serialize_with = "serialize_theme_mode",
        deserialize_with = "deserialize_theme_mode"
    )]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub theme_name: String,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            provider_id: default_provider_id(),
            api_key: String::new(),
            endpoint: default_endpoint(),
            models: default_models(),
            theme_mode: default_theme_mode(),
            theme_name: String::new(),
        }
    }
}

impl ProviderSettings {
    pub fn to_provider_config(&self) -> Option<ProviderConfig> {
        if self.api_key.trim().is_empty() {
            return None;
        }

        Some(ProviderConfig::new(
            &self.provider_id,
            &self.api_key,
            &self.endpoint,
        ))
    }

    pub fn is_valid(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    pub fn default_model_name(&self) -> String {
        self.models
            .iter()
            .find_map(|model| {
                let name = model.model_name.trim();
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            })
            .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string())
    }

    pub fn configured_models(&self) -> Vec<Model> {
        let models = self
            .models
            .iter()
            .filter_map(|model| model.clone().normalized())
            .map(|model| model.as_selector_model())
            .collect::<Vec<_>>();

        if models.is_empty() {
            vec![ModelSettings::default().as_selector_model()]
        } else {
            models
        }
    }

    pub fn model_max_tokens(&self, model_name: &str) -> Option<u64> {
        self.models
            .iter()
            .find(|model| model.model_name == model_name)
            .and_then(|model| model.max_tokens)
    }

    pub fn normalized(mut self) -> Self {
        self.provider_id = if self.provider_id.trim().is_empty() {
            default_provider_id()
        } else {
            self.provider_id.trim().to_string()
        };
        self.api_key = self.api_key.trim().to_string();
        self.endpoint = if self.endpoint.trim().is_empty() {
            default_endpoint()
        } else {
            self.endpoint.trim().to_string()
        };
        self.theme_name = self.theme_name.trim().to_string();

        // Keep provider->models relationship valid by removing blank rows.
        self.models = self
            .models
            .into_iter()
            .filter_map(ModelSettings::normalized)
            .collect();
        if self.models.is_empty() {
            self.models.push(ModelSettings::default());
        }

        self
    }

    pub fn apply_theme(&self, window: Option<&mut Window>, cx: &mut App) {
        if let Some(theme_config) = ThemeRegistry::global(cx)
            .themes()
            .get(&SharedString::from(self.theme_name.trim().to_string()))
            .cloned()
        {
            let mode = theme_config.mode;
            let theme = Theme::global_mut(cx);
            if mode.is_dark() {
                theme.dark_theme = theme_config;
            } else {
                theme.light_theme = theme_config;
            }
            Theme::change(mode, window, cx);
            return;
        }

        Theme::change(self.theme_mode, window, cx);
    }
}

pub struct SettingsStore {
    settings: Arc<ArcSwap<ProviderSettings>>,
    config_path: PathBuf,
}

impl SettingsStore {
    pub fn default_config_dir() -> PathBuf {
        dirs::config_dir()
            .map(|path| path.join(SETTINGS_DIRECTORY_NAME))
            .unwrap_or_else(|| PathBuf::from(".zova"))
    }

    pub fn default_config_path() -> PathBuf {
        Self::default_config_dir().join(SETTINGS_FILE_NAME)
    }

    pub fn new(config_path: PathBuf) -> Self {
        let settings = Self::load_from_disk(&config_path);
        Self {
            settings: Arc::new(ArcSwap::from_pointee(settings)),
            config_path,
        }
    }

    pub fn load() -> Self {
        Self::new(Self::default_config_path())
    }

    pub fn settings(&self) -> Arc<ProviderSettings> {
        self.settings.load_full()
    }

    pub fn update(&self, settings: ProviderSettings) -> Result<(), SettingsError> {
        let normalized_settings = settings.normalized();
        self.persist(&normalized_settings)?;
        self.settings.store(Arc::new(normalized_settings));
        Ok(())
    }

    fn load_from_disk(path: &PathBuf) -> ProviderSettings {
        if !path.exists() {
            tracing::info!("settings file not found at {:?}, using defaults", path);
            return ProviderSettings::default();
        }

        let figment = Figment::from(Serialized::defaults(ProviderSettings::default()))
            .merge(Json::file(path));

        match figment.extract::<ProviderSettings>() {
            Ok(settings) => settings.normalized(),
            Err(error) => {
                tracing::warn!(
                    "failed to parse settings from {:?}: {}. using defaults",
                    path,
                    error
                );
                ProviderSettings::default()
            }
        }
    }

    fn persist(&self, settings: &ProviderSettings) -> Result<(), SettingsError> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent).context(CreateDirSnafu {
                stage: "create-settings-directory",
                path: parent.to_path_buf(),
            })?;
        }

        let content = serde_json::to_string_pretty(settings).context(SerializeConfigSnafu {
            stage: "serialize-settings-json",
        })?;

        let temp_path = self.config_path.with_extension("json.tmp");
        std::fs::write(&temp_path, content).context(WriteFileSnafu {
            stage: "write-temporary-settings-file",
            path: temp_path.clone(),
        })?;

        std::fs::rename(&temp_path, &self.config_path).context(RenameTempFileSnafu {
            stage: "rename-temporary-settings-file",
            from: temp_path,
            to: self.config_path.clone(),
        })?;

        tracing::info!("saved settings to {:?}", self.config_path);
        Ok(())
    }
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum SettingsError {
    #[snafu(display("failed to create settings directory at {path:?} on `{stage}`: {source}"))]
    CreateDir {
        stage: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("failed to serialize settings on `{stage}`: {source}"))]
    SerializeConfig {
        stage: &'static str,
        source: serde_json::Error,
    },
    #[snafu(display("failed to write settings file at {path:?} on `{stage}`: {source}"))]
    WriteFile {
        stage: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display(
        "failed to replace settings file from {from:?} to {to:?} on `{stage}`: {source}"
    ))]
    RenameTempFile {
        stage: &'static str,
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
}

pub struct SettingsState {
    store: SettingsStore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsChanged {
    pub settings: ProviderSettings,
}

impl EventEmitter<SettingsChanged> for SettingsState {}

impl SettingsState {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            store: SettingsStore::load(),
        })
    }

    pub fn settings(&self) -> Arc<ProviderSettings> {
        self.store.settings()
    }

    pub fn update_settings(
        &mut self,
        settings: ProviderSettings,
        cx: &mut Context<Self>,
    ) -> Result<(), SettingsError> {
        let normalized_settings = settings.normalized();
        self.store.update(normalized_settings.clone())?;
        cx.emit(SettingsChanged {
            settings: normalized_settings,
        });
        cx.notify();
        Ok(())
    }
}

fn default_provider_id() -> String {
    DEFAULT_PROVIDER_ID.to_string()
}

fn default_endpoint() -> String {
    DEFAULT_ENDPOINT.to_string()
}

fn default_models() -> Vec<ModelSettings> {
    vec![ModelSettings::default()]
}

fn default_theme_mode() -> ThemeMode {
    ThemeMode::Light
}

fn serialize_theme_mode<S>(value: &ThemeMode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(value.name())
}

fn deserialize_theme_mode<'de, D>(deserializer: D) -> Result<ThemeMode, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(parse_theme_mode(&value))
}

fn parse_theme_mode(value: &str) -> ThemeMode {
    if value.trim().eq_ignore_ascii_case("dark") {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    }
}
