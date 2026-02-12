use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, IndexPath, Sizable, ThemeMode, ThemeRegistry,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    select::{Select, SelectState},
    v_flex,
};

use crate::settings::state::{ProviderSettings, SettingsState};

pub struct SettingsView {
    state: Entity<SettingsState>,
    provider_input: Entity<InputState>,
    api_key_input: Entity<InputState>,
    base_url_input: Entity<InputState>,
    model_input: Entity<InputState>,
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

    pub fn new(state: &Entity<SettingsState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = state.read(cx).settings().clone();

        let provider_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Provider ID (e.g., openai)"));
        provider_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.provider_id.clone(), window, cx);
        });

        let api_key_input = cx.new(|cx| InputState::new(window, cx).placeholder("API Key"));
        api_key_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.api_key.clone(), window, cx);
        });

        let base_url_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Endpoint (e.g., https://api.openai.com/v1)")
        });
        base_url_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.base_url.clone(), window, cx);
        });

        let model_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Default Model (e.g., gpt-4o-mini)"));
        model_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.default_model.clone(), window, cx);
        });

        let theme_names = Self::theme_names(cx);
        let selected_theme_index = Self::selected_theme_index(&theme_names, &settings.theme_name);
        let theme_preset_select = cx.new(|cx| {
            SelectState::new(theme_names, selected_theme_index, window, cx).searchable(true)
        });

        Self {
            state: state.clone(),
            provider_input,
            api_key_input,
            base_url_input,
            model_input,
            theme_preset_select,
            theme_mode: settings.theme_mode,
            error_message: None,
        }
    }

    pub fn reload_from_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.state.read(cx).settings().clone();

        self.provider_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.provider_id.clone(), window, cx);
        });
        self.api_key_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.api_key.clone(), window, cx);
        });
        self.base_url_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.base_url.clone(), window, cx);
        });
        self.model_input.update(cx, |input_state, cx| {
            input_state.set_value(settings.default_model.clone(), window, cx);
        });
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
        let base_url = self.base_url_input.read(cx).value().to_string();
        let default_model = self.model_input.read(cx).value().to_string();
        let theme_name = self
            .theme_preset_select
            .read(cx)
            .selected_value()
            .map(|theme_name| theme_name.to_string())
            .unwrap_or_default();

        let new_settings = ProviderSettings {
            provider_id: provider_id.trim().to_string(),
            api_key: api_key.trim().to_string(),
            base_url: if base_url.trim().is_empty() {
                crate::settings::state::DEFAULT_BASE_URL.to_string()
            } else {
                base_url.trim().to_string()
            },
            default_model: if default_model.trim().is_empty() {
                crate::llm::DEFAULT_OPENAI_MODEL.to_string()
            } else {
                default_model.trim().to_string()
            },
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
            Err(e) => {
                self.error_message = Some(format!("Failed to save settings: {e}"));
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

        v_flex()
            .id("settings-view")
            .w(px(400.))
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
                            .child(Input::new(&self.base_url_input).w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child("Default Model"),
                            )
                            .child(Input::new(&self.model_input).w_full()),
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
