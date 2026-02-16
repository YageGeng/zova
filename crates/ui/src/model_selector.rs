use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use zova_llm::{Model, default_openai_models};

use crate::chat::events::ModelChanged;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModelGroup {
    pub provider_key: String,
    pub provider_id: String,
    pub models: Vec<Model>,
}

pub struct ModelSelector {
    current_provider_key: String,
    current_model_id: String,
    is_open: bool,
    available_groups: Vec<ProviderModelGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSelected {
    pub provider_key: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSelectorSettingsClicked;

impl EventEmitter<ModelSelected> for ModelSelector {}
impl EventEmitter<ModelSelectorSettingsClicked> for ModelSelector {}

impl ModelSelector {
    pub fn new(
        current_provider_key: impl Into<String>,
        current_model_id: impl Into<String>,
    ) -> Self {
        Self {
            current_provider_key: current_provider_key.into(),
            current_model_id: current_model_id.into(),
            is_open: false,
            available_groups: vec![ProviderModelGroup {
                provider_key: "provider-1".to_string(),
                provider_id: "openai".to_string(),
                models: default_openai_models(),
            }],
        }
    }

    pub fn set_selection(
        &mut self,
        provider_key: impl Into<String>,
        model_id: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.current_provider_key = provider_key.into();
        self.current_model_id = model_id.into();
        self.ensure_valid_selection();
        cx.notify();
    }

    pub fn set_provider_model_groups(
        &mut self,
        groups: Vec<ProviderModelGroup>,
        cx: &mut Context<Self>,
    ) {
        self.available_groups = if groups.is_empty() {
            vec![ProviderModelGroup {
                provider_key: "provider-1".to_string(),
                provider_id: "openai".to_string(),
                models: default_openai_models(),
            }]
        } else {
            groups
        };

        self.ensure_valid_selection();
        cx.notify();
    }

    fn ensure_valid_selection(&mut self) {
        let has_current_selection = self.available_groups.iter().any(|group| {
            group.provider_key == self.current_provider_key
                && group
                    .models
                    .iter()
                    .any(|model| model.id == self.current_model_id)
        });

        if has_current_selection {
            return;
        }

        if let Some((provider_key, model_id)) = self.available_groups.iter().find_map(|group| {
            group
                .models
                .first()
                .map(|model| (group.provider_key.clone(), model.id.clone()))
        }) {
            self.current_provider_key = provider_key;
            self.current_model_id = model_id;
        }
    }

    fn toggle_open(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_open = !self.is_open;
        cx.notify();
    }

    fn select_model(
        &mut self,
        provider_key: String,
        model_id: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.current_provider_key = provider_key.clone();
        self.current_model_id = model_id.clone();
        self.is_open = false;
        cx.emit(ModelSelected {
            provider_key,
            model_id,
        });
        cx.emit(ModelChanged {
            provider_key: self.current_provider_key.clone(),
            model_id: self.current_model_id.clone(),
        });
        cx.notify();
    }

    fn open_settings(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_open = false;
        cx.emit(ModelSelectorSettingsClicked);
        cx.notify();
    }

    fn current_model_display_name(&self) -> String {
        self.available_groups
            .iter()
            .find(|group| group.provider_key == self.current_provider_key)
            .and_then(|group| {
                group
                    .models
                    .iter()
                    .find(|model| model.id == self.current_model_id)
                    .map(|model| model.name.clone())
            })
            .unwrap_or_else(|| self.current_model_id.clone())
    }
}

impl Render for ModelSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let display_name = self.current_model_display_name();
        let is_open = self.is_open;
        let current_provider_key = self.current_provider_key.clone();
        let current_model_id = self.current_model_id.clone();

        let mut dropdown_items = Vec::new();
        for group in self.available_groups.clone() {
            dropdown_items.push(
                h_flex()
                    .id(ElementId::Name(
                        format!("model-group-header-{}", group.provider_key).into(),
                    ))
                    .px_3()
                    .py_2()
                    .bg(theme.muted.opacity(0.35))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child(group.provider_id.clone()),
                    )
                    .into_any_element(),
            );

            for model in group.models {
                let provider_key = group.provider_key.clone();
                let model_id = model.id.clone();
                let is_selected =
                    provider_key == current_provider_key && model_id == current_model_id;

                dropdown_items.push(
                    h_flex()
                        .id(ElementId::Name(
                            format!("model-option-{}-{}", provider_key, model.id.clone()).into(),
                        ))
                        .px_3()
                        .py_2()
                        .gap_2()
                        .items_center()
                        .cursor_pointer()
                        .when(is_selected, |element| {
                            element.bg(theme.primary.opacity(0.1))
                        })
                        .when(!is_selected, |element| {
                            element.hover(|element| element.bg(theme.muted.opacity(0.5)))
                        })
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.select_model(provider_key.clone(), model_id.clone(), window, cx);
                        }))
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(theme.foreground)
                                .child(model.name.clone()),
                        )
                        .into_any_element(),
                );
            }
        }

        h_flex()
            .id("model-selector")
            .relative()
            .child(
                Button::new("model-selector-button")
                    .ghost()
                    .small()
                    .child(display_name)
                    .when(is_open, |button| button.selected(true))
                    .on_click(cx.listener(Self::toggle_open)),
            )
            .when(is_open, |element| {
                element.child(
                    v_flex()
                        .id("model-selector-dropdown")
                        .absolute()
                        .top(px(32.))
                        .right_0()
                        .w(px(360.))
                        .max_h(px(460.))
                        .overflow_y_scroll()
                        .bg(theme.popover)
                        .rounded_md()
                        .shadow_md()
                        .border_1()
                        .border_color(theme.border)
                        .py_1()
                        .child(
                            h_flex()
                                .px_3()
                                .py_2()
                                .border_b_1()
                                .border_color(theme.border)
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Select Model"),
                                )
                                .child(
                                    Button::new("model-selector-settings")
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Settings)
                                        .child("Settings")
                                        .on_click(cx.listener(Self::open_settings)),
                                ),
                        )
                        .children(dropdown_items),
                )
            })
    }
}

impl EventEmitter<ModelChanged> for ModelSelector {}
