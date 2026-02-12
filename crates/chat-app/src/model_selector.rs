use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::chat::events::ModelChanged;
use crate::llm::{Model, default_openai_models};

pub struct ModelSelector {
    current_model_id: String,
    is_open: bool,
    available_models: Vec<Model>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSelected {
    pub model_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSelectorSettingsClicked;

impl EventEmitter<ModelSelected> for ModelSelector {}
impl EventEmitter<ModelSelectorSettingsClicked> for ModelSelector {}

impl ModelSelector {
    pub fn new(current_model_id: impl Into<String>) -> Self {
        Self {
            current_model_id: current_model_id.into(),
            is_open: false,
            available_models: default_openai_models(),
        }
    }

    pub fn set_model_id(&mut self, model_id: impl Into<String>, cx: &mut Context<Self>) {
        self.current_model_id = model_id.into();
        cx.notify();
    }

    pub fn set_models(&mut self, models: Vec<Model>, cx: &mut Context<Self>) {
        self.available_models = if models.is_empty() {
            default_openai_models()
        } else {
            models
        };

        if !self
            .available_models
            .iter()
            .any(|model| model.id == self.current_model_id)
            && let Some(first_model) = self.available_models.first()
        {
            self.current_model_id = first_model.id.clone();
        }

        cx.notify();
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

    fn select_model(&mut self, model_id: String, _window: &mut Window, cx: &mut Context<Self>) {
        self.current_model_id = model_id.clone();
        self.is_open = false;
        cx.emit(ModelSelected { model_id });
        cx.emit(ModelChanged {
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
        self.available_models
            .iter()
            .find(|model| model.id == self.current_model_id)
            .map(|model| model.name.clone())
            .unwrap_or_else(|| self.current_model_id.clone())
    }
}

impl Render for ModelSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let display_name = self.current_model_display_name();
        let is_open = self.is_open;

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
                        .w(px(320.))
                        .max_h(px(420.))
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
                        .children(self.available_models.iter().map(|model| {
                            let model_id = model.id.clone();
                            let is_selected = model_id == self.current_model_id;

                            h_flex()
                                .id(ElementId::Name(format!("model-option-{model_id}").into()))
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
                                    this.select_model(model_id.clone(), window, cx);
                                }))
                                .child(
                                    h_flex()
                                        .flex_1()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.foreground)
                                                .child(model.name.clone()),
                                        )
                                        .when_some(
                                            model.description.clone(),
                                            |element, description| {
                                                element.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child(description),
                                                )
                                            },
                                        ),
                                )
                                .when(is_selected, |element| {
                                    element.child(
                                        h_flex()
                                            .gap_1()
                                            .items_center()
                                            .child(
                                                Icon::new(IconName::Check)
                                                    .size(px(16.))
                                                    .text_color(theme.primary),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.primary)
                                                    .child("Selected"),
                                            ),
                                    )
                                })
                                .into_any_element()
                        })),
                )
            })
    }
}

impl EventEmitter<ModelChanged> for ModelSelector {}
