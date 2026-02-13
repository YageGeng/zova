use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable, ThemeMode,
    button::{Button, ButtonVariants},
    h_flex,
    select::Select,
    v_flex,
};

use super::SettingsView;

pub(super) fn render(view: &mut SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let theme = cx.theme();

    v_flex()
        .id("settings-theme-category")
        .gap_4()
        .p_4()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child("Theme Settings"),
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
                                .when(view.theme_mode == ThemeMode::Light, |button| {
                                    button.primary()
                                })
                                .when(view.theme_mode != ThemeMode::Light, |button| button.ghost())
                                .child("Light")
                                .on_click(cx.listener(SettingsView::select_light_mode)),
                        )
                        .child(
                            Button::new("settings-theme-dark")
                                .small()
                                .when(view.theme_mode == ThemeMode::Dark, |button| {
                                    button.primary()
                                })
                                .when(view.theme_mode != ThemeMode::Dark, |button| button.ghost())
                                .child("Dark")
                                .on_click(cx.listener(SettingsView::select_dark_mode)),
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
                    Select::new(&view.theme_preset_select)
                        .w_full()
                        .placeholder("Follow mode")
                        .search_placeholder("Search theme preset")
                        .cleanable(true),
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
