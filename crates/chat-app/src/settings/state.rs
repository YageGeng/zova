use std::path::PathBuf;

use gpui::*;
use gpui_component::{Theme, ThemeMode, ThemeRegistry};

/// Default provider ID when none is specified.
pub const DEFAULT_PROVIDER_ID: &str = "openai";

/// Default base URL for OpenAI API.
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// Settings that persist across app restarts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSettings {
    /// Provider identifier (e.g., "openai")
    pub provider_id: String,
    /// API key for the provider
    pub api_key: String,
    /// Base URL for the provider API
    pub base_url: String,
    /// Default model ID to use
    pub default_model: String,
    pub theme_mode: ThemeMode,
    pub theme_name: String,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            provider_id: DEFAULT_PROVIDER_ID.to_string(),
            api_key: String::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
            default_model: crate::llm::DEFAULT_OPENAI_MODEL.to_string(),
            theme_mode: ThemeMode::Light,
            theme_name: String::new(),
        }
    }
}

impl ProviderSettings {
    /// Creates provider config from these settings.
    /// Returns None if API key is empty.
    pub fn to_provider_config(&self) -> Option<crate::llm::ProviderConfig> {
        if self.api_key.trim().is_empty() {
            return None;
        }

        Some(crate::llm::ProviderConfig::new(
            &self.provider_id,
            &self.api_key,
            &self.base_url,
            Some(self.default_model.clone()),
        ))
    }

    /// Returns true if the settings are valid (have non-empty API key).
    pub fn is_valid(&self) -> bool {
        !self.api_key.trim().is_empty()
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

/// Settings persistence layer using a simple line-based format.
pub struct SettingsStore {
    settings: ProviderSettings,
    config_path: PathBuf,
}

impl SettingsStore {
    /// Returns the default config file path in the user's home directory.
    pub fn default_config_path() -> PathBuf {
        // Use .chat-app directory in current working directory for MVP
        PathBuf::from(".chat-app").join("settings.conf")
    }

    /// Creates a new settings store with the given config path.
    pub fn new(config_path: PathBuf) -> Self {
        let settings = Self::load_from_disk(&config_path);
        Self {
            settings,
            config_path,
        }
    }

    /// Loads settings with default path.
    pub fn load() -> Self {
        Self::new(Self::default_config_path())
    }

    /// Returns current settings.
    pub fn settings(&self) -> &ProviderSettings {
        &self.settings
    }

    /// Updates settings and persists to disk.
    pub fn update(&mut self, settings: ProviderSettings) -> Result<(), SettingsError> {
        self.persist(&settings)?;
        self.settings = settings;
        Ok(())
    }

    /// Loads settings from disk or returns defaults.
    fn load_from_disk(path: &PathBuf) -> ProviderSettings {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => {
                tracing::info!("settings file not found at {:?}, using defaults", path);
                return ProviderSettings::default();
            }
        };

        Self::parse_settings(&content)
    }

    /// Parses settings from content using key=value format.
    fn parse_settings(content: &str) -> ProviderSettings {
        let mut settings = ProviderSettings::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "provider_id" => settings.provider_id = value.to_string(),
                    "api_key" => settings.api_key = value.to_string(),
                    "base_url" => settings.base_url = value.to_string(),
                    "default_model" => settings.default_model = value.to_string(),
                    "theme_mode" => settings.theme_mode = parse_theme_mode(value),
                    "theme_name" => settings.theme_name = value.to_string(),
                    _ => {}
                }
            }
        }

        settings
    }

    /// Formats settings for persistence.
    fn format_settings(settings: &ProviderSettings) -> String {
        format!(
            "# Chat App Settings\n\
             provider_id={}\n\
             api_key={}\n\
             base_url={}\n\
             default_model={}\n\
             theme_mode={}\n\
             theme_name={}\n",
            settings.provider_id,
            settings.api_key,
            settings.base_url,
            settings.default_model,
            settings.theme_mode.name(),
            settings.theme_name
        )
    }

    /// Persists settings to disk.
    fn persist(&self, settings: &ProviderSettings) -> Result<(), SettingsError> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SettingsError::CreateDir {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let content = Self::format_settings(settings);

        std::fs::write(&self.config_path, content).map_err(|e| SettingsError::WriteFile {
            path: self.config_path.clone(),
            source: e,
        })?;

        tracing::info!("saved settings to {:?}", self.config_path);
        Ok(())
    }
}

/// Errors that can occur during settings operations.
#[derive(Debug)]
pub enum SettingsError {
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsError::CreateDir { path, source } => {
                write!(
                    f,
                    "failed to create config directory at {:?}: {}",
                    path, source
                )
            }
            SettingsError::WriteFile { path, source } => {
                write!(f, "failed to write settings file to {:?}: {}", path, source)
            }
        }
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SettingsError::CreateDir { source, .. } => Some(source),
            SettingsError::WriteFile { source, .. } => Some(source),
        }
    }
}

fn parse_theme_mode(value: &str) -> ThemeMode {
    if value.trim().eq_ignore_ascii_case("dark") {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    }
}

/// GPUI entity that holds settings state and emits change events.
pub struct SettingsState {
    store: SettingsStore,
}

/// Emitted when settings change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsChanged {
    pub settings: ProviderSettings,
}

impl EventEmitter<SettingsChanged> for SettingsState {}

impl SettingsState {
    /// Creates a new settings state entity.
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            store: SettingsStore::load(),
        })
    }

    /// Returns current settings.
    pub fn settings(&self) -> &ProviderSettings {
        self.store.settings()
    }

    /// Updates settings and notifies subscribers.
    pub fn update_settings(
        &mut self,
        settings: ProviderSettings,
        cx: &mut Context<Self>,
    ) -> Result<(), SettingsError> {
        self.store.update(settings.clone())?;
        cx.emit(SettingsChanged {
            settings: settings.clone(),
        });
        cx.notify();
        Ok(())
    }
}
