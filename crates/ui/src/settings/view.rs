use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    select::{Select, SelectState},
    v_flex, ActiveTheme, IndexPath, Sizable, ThemeMode, ThemeRegistry,
};

use crate::settings::state::{ModelSettings, ProviderSettings, SettingsState};

struct ModelInputRow {
    model_name_input: Entity<InputState>,
    max_completion_tokens_input: Entity<InputState>,
    max_output_tokens_input: Entity<InputState>,
    max_tokens_input: Entity<InputState>,
}

pub struct SettingsView {
    state: Entity<SettingsState>,
    provider_input: Entity<InputState>,
    api_key_input: Entity<InputState>,
    endpoint_input: Entity<InputState>,
    model_rows: Vec<ModelInputRow>,
    theme_preset_select: Entity<SelectState<Vec<SharedString>>>,
    theme_mode: ThemeMode,
    error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsClose;

impl EventEmitter<SettingsClose> for SettingsView {}

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
        settings: &ProviderSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<ModelInputRow> {
        let models = if settings.models.is_empty() {
            vec![ModelSettings::default()]
        } else {
            settings.models.clone()
        };

        models
            .iter()
            .map(|model| Self::new_model_row(model, window, cx))
            .collect()
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

    pub fn new(state: &Entity<SettingsState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = state.read(cx).settings();

        let provider_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Provider ID (e.g., openai)"));
        provider_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.provider_id.clone(), window, cx);
        });

        let api_key_input = cx.new(|cx| InputState::new(window, cx).placeholder("API Key"));
        api_key_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.api_key.clone(), window, cx);
        });

        let endpoint_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Endpoint (e.g., https://api.openai.com/v1)")
        });
        endpoint_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.endpoint.clone(), window, cx);
        });

        let model_rows = Self::build_model_rows(&settings, window, cx);

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
            theme_preset_select,
            theme_mode: settings.theme_mode,
            error_message: None,
        }
    }

    pub fn reload_from_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.state.read(cx).settings();

        self.provider_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.provider_id.clone(), window, cx);
        });
        self.api_key_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.api_key.clone(), window, cx);
        });
        self.endpoint_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.endpoint.clone(), window, cx);
        });
        self.model_rows = Self::build_model_rows(&settings, window, cx);
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let provider_id = self.provider_input.read(cx).value().to_string();
        let api_key = self.api_key_input.read(cx).value().to_string();
        let endpoint = self.endpoint_input.read(cx).value().to_string();
        let theme_name = self
            .theme_preset_select
            .read(cx)
            .selected_value()
            .map(|theme_name| theme_name.to_string())
            .unwrap_or_default();

        let models = match self.collect_models(cx) {
            Ok(models) => models,
            Err(error) => {
                self.error_message = Some(error);
                cx.notify();
                return;
            }
        };

        let new_settings = ProviderSettings {
            provider_id: provider_id.trim().to_string(),
            api_key: api_key.trim().to_string(),
            endpoint: if endpoint.trim().is_empty() {
                crate::settings::state::DEFAULT_ENDPOINT.to_string()
            } else {
                endpoint.trim().to_string()
            },
            models,
            theme_mode: self.theme_mode,
            theme_name: theme_name.trim().to_string(),
        };

        match self
            .state
            .update(cx, |state, cx| state.update_settings(new_settings, cx))
        {
            Ok(()) => {
                self.error_message = None;
                cx.emit(SettingsClose);
                cx.notify();
            }
            Err(error) => {
                self.error_message = Some(format!("Failed to save settings: {error}"));
                cx.notify();
            }
        }
    }

    fn cancel(&mut self, _event: &gpui::ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.error_message = None;
        cx.emit(SettingsClose);
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let can_remove_rows = self.model_rows.len() > 1;

        v_flex()
            .id("settings-view")
            .w(px(700.))
            .gap_4()
            .p_4()
            .bg(theme.popover)
            .rounded_lg()
            .shadow_lg()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child("Provider Settings"),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child("Provider"),
                            )
                            .child(Input::new(&self.provider_input).w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child("API Key"),
                            )
                            .child(Input::new(&self.api_key_input).w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child("Endpoint"),
                            )
                            .child(Input::new(&self.endpoint_input).w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.foreground)
                                            .child("Models"),
                                    )
                                    .child(
                                        Button::new("settings-add-model")
                                            .ghost()
                                            .small()
                                            .child("Add Model")
                                            .on_click(cx.listener(Self::add_model_row)),
                                    ),
                            )
                            .children(self.model_rows.iter().enumerate().map(|(index, row)| {
                                let model_name_input = row.model_name_input.clone();
                                let max_completion_tokens_input =
                                    row.max_completion_tokens_input.clone();
                                let max_output_tokens_input = row.max_output_tokens_input.clone();
                                let max_tokens_input = row.max_tokens_input.clone();

                                v_flex()
                                    .id(("settings-model-row", index))
                                    .gap_2()
                                    .p_3()
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded_md()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child(format!("Model #{}", index + 1)),
                                            )
                                            .when(can_remove_rows, |row_header| {
                                                row_header.child(
                                                    Button::new(("settings-remove-model", index))
                                                        .ghost()
                                                        .xsmall()
                                                        .child("Remove")
                                                        .on_click(cx.listener(
                                                            move |this, _event, window, cx| {
                                                                this.remove_model_row(
                                                                    index, window, cx,
                                                                );
                                                            },
                                                        )),
                                                )
                                            }),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.foreground)
                                                    .child("model_name"),
                                            )
                                            .child(Input::new(&model_name_input).w_full()),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_2()
                                            .child(
                                                v_flex()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.foreground)
                                                            .child("max_completion_tokens"),
                                                    )
                                                    .child(
                                                        Input::new(&max_completion_tokens_input)
                                                            .w_full(),
                                                    ),
                                            )
                                            .child(
                                                v_flex()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.foreground)
                                                            .child("max_output_tokens"),
                                                    )
                                                    .child(
                                                        Input::new(&max_output_tokens_input)
                                                            .w_full(),
                                                    ),
                                            )
                                            .child(
                                                v_flex()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.foreground)
                                                            .child("max_tokens"),
                                                    )
                                                    .child(Input::new(&max_tokens_input).w_full()),
                                            ),
                                    )
                                    .into_any_element()
                            })),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child("Theme Mode"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("settings-theme-light")
                                            .small()
                                            .when(self.theme_mode == ThemeMode::Light, |button| {
                                                button.primary()
                                            })
                                            .when(self.theme_mode != ThemeMode::Light, |button| {
                                                button.ghost()
                                            })
                                            .child("Light")
                                            .on_click(cx.listener(Self::select_light_mode)),
                                    )
                                    .child(
                                        Button::new("settings-theme-dark")
                                            .small()
                                            .when(self.theme_mode == ThemeMode::Dark, |button| {
                                                button.primary()
                                            })
                                            .when(self.theme_mode != ThemeMode::Dark, |button| {
                                                button.ghost()
                                            })
                                            .child("Dark")
                                            .on_click(cx.listener(Self::select_dark_mode)),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child("Theme Preset"),
                            )
                            .child(
                                Select::new(&self.theme_preset_select)
                                    .w_full()
                                    .placeholder("Follow mode")
                                    .search_placeholder("Search theme preset")
                                    .cleanable(true),
                            ),
                    ),
            )
            .when_some(self.error_message.clone(), |el, error| {
                el.child(div().text_sm().text_color(theme.danger).child(error))
            })
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("settings-cancel")
                            .ghost()
                            .small()
                            .child("Cancel")
                            .on_click(cx.listener(Self::cancel)),
                    )
                    .child(
                        Button::new("settings-save")
                            .primary()
                            .small()
                            .child("Save")
                            .on_click(cx.listener(Self::save_settings)),
                    ),
            )
    }
}
