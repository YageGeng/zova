use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    v_flex,
};

use super::SettingsView;

pub(super) fn render(view: &mut SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let theme = cx.theme();
    let can_remove_rows = view.model_rows.len() > 1;

    v_flex()
        .id("settings-provider-category")
        .gap_4()
        .p_4()
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
                        .child(Input::new(&view.provider_input).w_full()),
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
                        .child(Input::new(&view.api_key_input).w_full()),
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
                        .child(Input::new(&view.endpoint_input).w_full()),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .child(div().text_sm().text_color(theme.foreground).child("Models"))
                                .child(
                                    Button::new("settings-add-model")
                                        .ghost()
                                        .small()
                                        .child("Add Model")
                                        .on_click(cx.listener(SettingsView::add_model_row)),
                                ),
                        )
                        .children(view.model_rows.iter().enumerate().map(|(index, row)| {
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
                                                    Input::new(&max_output_tokens_input).w_full(),
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
                ),
        )
        .when_some(view.error_message.clone(), |el, error| {
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
                        .on_click(cx.listener(SettingsView::cancel)),
                )
                .child(
                    Button::new("settings-save")
                        .primary()
                        .small()
                        .child("Save")
                        .on_click(cx.listener(SettingsView::save_settings)),
                ),
        )
        .into_any_element()
}
