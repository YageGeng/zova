use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, IndexPath, Sizable, ThemeMode, ThemeRegistry,
    button::{Button, ButtonVariants},
    h_flex,
    input::InputState,
    scroll::ScrollableElement,
    select::SelectState,
    v_flex,
};

use crate::settings::state::{
    ModelSettings, ProviderProfileSettings, ProviderSettings, SettingsState,
};

mod provider;
mod theme;

struct ModelInputRow {
    model_name_input: Entity<InputState>,
    max_completion_tokens_input: Entity<InputState>,
    max_output_tokens_input: Entity<InputState>,
    max_tokens_input: Entity<InputState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsCategory {
    Provider,
    Theme,
}

const SETTINGS_TRAFFIC_LIGHT_SAFE_TOP: f32 = 44.0;

pub struct SettingsView {
    state: Entity<SettingsState>,
    provider_input: Entity<InputState>,
    api_key_input: Entity<InputState>,
    endpoint_input: Entity<InputState>,
    model_rows: Vec<ModelInputRow>,
    provider_profiles: Vec<ProviderProfileSettings>,
    active_provider_index: usize,
    expanded_provider_index: Option<usize>,
    theme_preset_select: Entity<SelectState<Vec<SharedString>>>,
    theme_mode: ThemeMode,
    active_category: SettingsCategory,
    error_message: Option<String>,
}

impl SettingsView {
    fn theme_names(cx: &App) -> Vec<SharedString> {
        ThemeRegistry::global(cx)
            .sorted_themes()
            .iter()
            .map(|theme| theme.name.clone())
            .collect()
    }

    fn selected_theme_index(
        theme_names: &[SharedString],
        selected_theme_name: &str,
    ) -> Option<IndexPath> {
        if selected_theme_name.trim().is_empty() {
            return None;
        }

        theme_names
            .iter()
            .position(|theme_name| theme_name.as_ref() == selected_theme_name.trim())
            .map(|index| IndexPath::default().row(index))
    }

    fn provider_profile_label(profile: &ProviderProfileSettings, index: usize) -> String {
        let provider_id = profile.provider_id.trim();
        if provider_id.is_empty() {
            format!("Provider #{}", index + 1)
        } else {
            provider_id.to_string()
        }
    }

    fn active_provider_profile(&self) -> Option<&ProviderProfileSettings> {
        self.provider_profiles.get(self.active_provider_index)
    }

    fn active_provider_profile_mut(&mut self) -> Option<&mut ProviderProfileSettings> {
        self.provider_profiles.get_mut(self.active_provider_index)
    }

    fn next_provider_key(&self) -> String {
        let mut index = self.provider_profiles.len() + 1;
        loop {
            let candidate = format!("provider-{index}");
            if !self
                .provider_profiles
                .iter()
                .any(|profile| profile.provider_key == candidate)
            {
                return candidate;
            }
            index += 1;
        }
    }

    fn new_model_row(
        model: &ModelSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ModelInputRow {
        let model_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Model Name (e.g., gpt-4o-mini)"));
        model_name_input.update(cx, |input_state, cx| {
            input_state.set_value(model.model_name.clone(), window, cx);
        });

        let max_completion_tokens_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("max_completion_tokens (optional)"));
        max_completion_tokens_input.update(cx, |input_state, cx| {
            input_state.set_value(
                model
                    .max_completion_tokens
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            );
        });

        let max_output_tokens_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("max_output_tokens (optional)"));
        max_output_tokens_input.update(cx, |input_state, cx| {
            input_state.set_value(
                model
                    .max_output_tokens
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            );
        });

        let max_tokens_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("max_tokens (optional)"));
        max_tokens_input.update(cx, |input_state, cx| {
            input_state.set_value(
                model
                    .max_tokens
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            );
        });

        ModelInputRow {
            model_name_input,
            max_completion_tokens_input,
            max_output_tokens_input,
            max_tokens_input,
        }
    }

    fn build_model_rows(
        provider: &ProviderProfileSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<ModelInputRow> {
        let models = if provider.models.is_empty() {
            vec![ModelSettings::default()]
        } else {
            provider.models.clone()
        };

        models
            .iter()
            .map(|model| Self::new_model_row(model, window, cx))
            .collect()
    }

    fn load_active_provider_into_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(provider) = self.active_provider_profile().cloned() {
            self.provider_input.update(cx, |input_state, cx| {
                input_state.set_value(provider.provider_id.clone(), window, cx);
            });
            self.api_key_input.update(cx, |input_state, cx| {
                input_state.set_value(provider.api_key.clone(), window, cx);
            });
            self.endpoint_input.update(cx, |input_state, cx| {
                input_state.set_value(provider.endpoint.clone(), window, cx);
            });
            self.model_rows = Self::build_model_rows(&provider, window, cx);
        }
    }

    fn parse_optional_u64(
        value: &str,
        field_name: &str,
        model_name: &str,
    ) -> Result<Option<u64>, String> {
        let trimmed_value = value.trim();
        if trimmed_value.is_empty() {
            return Ok(None);
        }

        let parsed_value = trimmed_value.parse::<u64>().map_err(|_| {
            format!("Model '{model_name}' field '{field_name}' must be an unsigned integer")
        })?;

        Ok(Some(parsed_value))
    }

    fn collect_models(&self, cx: &App) -> Result<Vec<ModelSettings>, String> {
        if self.model_rows.is_empty() {
            return Err("At least one model is required".to_string());
        }

        let mut models = Vec::with_capacity(self.model_rows.len());
        for (index, row) in self.model_rows.iter().enumerate() {
            let model_name = row.model_name_input.read(cx).value().to_string();
            let trimmed_model_name = model_name.trim();
            if trimmed_model_name.is_empty() {
                return Err(format!("Model #{} requires a model name", index + 1));
            }

            let max_completion_tokens =
                row.max_completion_tokens_input.read(cx).value().to_string();
            let max_output_tokens = row.max_output_tokens_input.read(cx).value().to_string();
            let max_tokens = row.max_tokens_input.read(cx).value().to_string();

            models.push(ModelSettings {
                model_name: trimmed_model_name.to_string(),
                max_completion_tokens: Self::parse_optional_u64(
                    &max_completion_tokens,
                    "max_completion_tokens",
                    trimmed_model_name,
                )?,
                max_output_tokens: Self::parse_optional_u64(
                    &max_output_tokens,
                    "max_output_tokens",
                    trimmed_model_name,
                )?,
                max_tokens: Self::parse_optional_u64(
                    &max_tokens,
                    "max_tokens",
                    trimmed_model_name,
                )?,
            });
        }

        Ok(models)
    }

    fn apply_inputs_to_active_provider(&mut self, cx: &App) -> Result<(), String> {
        let models = self.collect_models(cx)?;
        let provider_id = self.provider_input.read(cx).value().trim().to_string();
        let api_key = self.api_key_input.read(cx).value().trim().to_string();
        let endpoint = self.endpoint_input.read(cx).value().trim().to_string();

        if let Some(provider) = self.active_provider_profile_mut() {
            provider.provider_id = provider_id;
            provider.api_key = api_key;
            provider.endpoint = if endpoint.is_empty() {
                crate::settings::state::DEFAULT_ENDPOINT.to_string()
            } else {
                endpoint
            };
            provider.models = models;
        }

        Ok(())
    }

    fn apply_inputs_to_expanded_provider(&mut self, cx: &App) -> Result<(), String> {
        if self.expanded_provider_index.is_some() {
            self.apply_inputs_to_active_provider(cx)?;
        }
        Ok(())
    }

    pub fn new(state: &Entity<SettingsState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = state.read(cx).settings();

        let provider_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Provider ID (e.g., openai)"));
        let api_key_input = cx.new(|cx| InputState::new(window, cx).placeholder("API Key"));
        let endpoint_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Endpoint (e.g., https://api.openai.com/v1)")
        });

        let mut provider_profiles = settings.providers().to_vec();
        if provider_profiles.is_empty() {
            provider_profiles.push(ProviderProfileSettings::default());
        }
        let active_provider_index = settings.active_provider_index();

        let default_provider = provider_profiles
            .get(active_provider_index)
            .cloned()
            .unwrap_or_else(ProviderProfileSettings::default);

        provider_input.update(cx, |input_state, cx| {
            input_state.set_value(default_provider.provider_id.clone(), window, cx);
        });
        api_key_input.update(cx, |input_state, cx| {
            input_state.set_value(default_provider.api_key.clone(), window, cx);
        });
        endpoint_input.update(cx, |input_state, cx| {
            input_state.set_value(default_provider.endpoint.clone(), window, cx);
        });

        let model_rows = Self::build_model_rows(&default_provider, window, cx);

        let theme_names = Self::theme_names(cx);
        let selected_theme_index = Self::selected_theme_index(&theme_names, &settings.theme_name);
        let theme_preset_select = cx.new(|cx| {
            SelectState::new(theme_names, selected_theme_index, window, cx).searchable(true)
        });

        Self {
            state: state.clone(),
            provider_input,
            api_key_input,
            endpoint_input,
            model_rows,
            provider_profiles,
            active_provider_index,
            expanded_provider_index: None,
            theme_preset_select,
            theme_mode: settings.theme_mode,
            active_category: SettingsCategory::Provider,
            error_message: None,
        }
    }

    pub fn reload_from_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.state.read(cx).settings();
        self.provider_profiles = settings.providers().to_vec();
        if self.provider_profiles.is_empty() {
            self.provider_profiles
                .push(ProviderProfileSettings::default());
        }
        self.active_provider_index = settings.active_provider_index();
        self.expanded_provider_index = None;

        self.load_active_provider_into_inputs(window, cx);

        self.theme_preset_select.update(cx, |select_state, cx| {
            let theme_names = Self::theme_names(cx);
            let selected_theme_index =
                Self::selected_theme_index(&theme_names, &settings.theme_name);
            select_state.set_items(theme_names, window, cx);
            select_state.set_selected_index(selected_theme_index, window, cx);
        });
        self.theme_mode = settings.theme_mode;
        self.error_message = None;
    }

    fn add_provider_profile(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.apply_inputs_to_expanded_provider(cx) {
            self.error_message = Some(error);
            cx.notify();
            return;
        }

        let provider = ProviderProfileSettings {
            provider_key: self.next_provider_key(),
            ..ProviderProfileSettings::default()
        };
        self.provider_profiles.push(provider);
        self.active_provider_index = self.provider_profiles.len().saturating_sub(1);
        self.expanded_provider_index = None;
        self.error_message = None;
        cx.notify();
    }

    fn remove_provider_profile(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.provider_profiles.len() {
            return;
        }

        if let Err(error) = self.apply_inputs_to_expanded_provider(cx) {
            self.error_message = Some(error);
            cx.notify();
            return;
        }

        if self.provider_profiles.len() <= 1 {
            if let Some(provider) = self.provider_profiles.get_mut(0) {
                let provider_key = provider.provider_key.clone();
                *provider = ProviderProfileSettings::default();
                provider.provider_key = provider_key;
            }
            self.active_provider_index = 0;
        } else {
            self.provider_profiles.remove(index);

            if self.active_provider_index > index {
                self.active_provider_index = self.active_provider_index.saturating_sub(1);
            } else if self.active_provider_index >= self.provider_profiles.len() {
                self.active_provider_index = self.provider_profiles.len().saturating_sub(1);
            }
        }

        self.expanded_provider_index = None;
        self.error_message = None;
        cx.notify();
    }

    fn select_provider_profile(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.provider_profiles.len() {
            return;
        }

        if self.expanded_provider_index == Some(index) {
            if let Err(error) = self.apply_inputs_to_active_provider(cx) {
                self.error_message = Some(error);
                cx.notify();
                return;
            }

            self.expanded_provider_index = None;
            self.error_message = None;
            cx.notify();
            return;
        }

        if let Err(error) = self.apply_inputs_to_expanded_provider(cx) {
            self.error_message = Some(error);
            cx.notify();
            return;
        }

        self.active_provider_index = index;
        self.load_active_provider_into_inputs(window, cx);
        self.expanded_provider_index = Some(index);
        self.error_message = None;
        cx.notify();
    }

    fn add_model_row(
        &mut self,
        _event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model_rows
            .push(Self::new_model_row(&ModelSettings::default(), window, cx));
        self.error_message = None;
        cx.notify();
    }

    fn remove_model_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_rows.len() <= 1 {
            self.model_rows[0] = Self::new_model_row(&ModelSettings::default(), window, cx);
        } else if index < self.model_rows.len() {
            self.model_rows.remove(index);
        }

        self.error_message = None;
        cx.notify();
    }

    fn select_light_mode(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_mode = ThemeMode::Light;
        cx.notify();
    }

    fn select_dark_mode(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_mode = ThemeMode::Dark;
        cx.notify();
    }

    fn save_settings(
        &mut self,
        _event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.apply_inputs_to_expanded_provider(cx) {
            self.error_message = Some(error);
            cx.notify();
            return;
        }

        let Some(active_provider) = self.active_provider_profile() else {
            self.error_message = Some("At least one provider is required".to_string());
            cx.notify();
            return;
        };

        let theme_name = self
            .theme_preset_select
            .read(cx)
            .selected_value()
            .map(|theme_name| theme_name.to_string())
            .unwrap_or_default();

        let new_settings = ProviderSettings {
            active_provider_key: active_provider.provider_key.clone(),
            providers: self.provider_profiles.clone(),
            provider_id: String::new(),
            api_key: String::new(),
            endpoint: String::new(),
            models: Vec::new(),
            theme_mode: self.theme_mode,
            theme_name: theme_name.trim().to_string(),
        };

        match self
            .state
            .update(cx, |state, cx| state.update_settings(new_settings, cx))
        {
            Ok(()) => {
                self.error_message = None;
                window.remove_window();
                cx.notify();
            }
            Err(error) => {
                self.error_message = Some(format!("Failed to save settings: {error}"));
                cx.notify();
            }
        }
    }

    fn cancel(&mut self, _event: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.error_message = None;
        window.remove_window();
        cx.notify();
    }

    fn select_provider_category(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_category = SettingsCategory::Provider;
        cx.notify();
    }

    fn select_theme_category(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_category = SettingsCategory::Theme;
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let provider_selected = self.active_category == SettingsCategory::Provider;
        let theme_selected = self.active_category == SettingsCategory::Theme;
        let category_content = match self.active_category {
            SettingsCategory::Provider => provider::render(self, cx),
            SettingsCategory::Theme => theme::render(self, cx),
        };
        let theme = cx.theme();

        v_flex()
            .id("settings-view")
            .size_full()
            .min_h_0()
            .bg(theme.background)
            .child(
                h_flex()
                    .size_full()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        v_flex()
                            .id("settings-category-list")
                            .w(px(200.))
                            .h_full()
                            .flex_shrink_0()
                            .pt(px(SETTINGS_TRAFFIC_LIGHT_SAFE_TOP))
                            .px_3()
                            .pb_3()
                            .gap_2()
                            .bg(theme.muted.opacity(0.35))
                            .border_r_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.muted_foreground)
                                    .child("Categories"),
                            )
                            .child(
                                Button::new("settings-category-provider")
                                    .small()
                                    .when(provider_selected, |button| button.primary())
                                    .when(!provider_selected, |button| button.ghost())
                                    .child("Provider")
                                    .on_click(cx.listener(Self::select_provider_category)),
                            )
                            .child(
                                Button::new("settings-category-theme")
                                    .small()
                                    .when(theme_selected, |button| button.primary())
                                    .when(!theme_selected, |button| button.ghost())
                                    .child("Theme")
                                    .on_click(cx.listener(Self::select_theme_category)),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-category-content")
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .pt(px(SETTINGS_TRAFFIC_LIGHT_SAFE_TOP))
                            .child(category_content),
                    ),
            )
    }
}
