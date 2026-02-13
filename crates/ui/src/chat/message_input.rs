use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    v_flex,
};

use crate::chat::events::{Stop, Submit};
use crate::chat::message::{ConversationId, StreamSessionId, StreamTarget};

const DEFAULT_STREAM_TARGET: StreamTarget =
    StreamTarget::new(ConversationId::new(0), StreamSessionId::new(0));

pub struct MessageInput {
    input_state: Entity<InputState>,
    stream_target: StreamTarget,
    is_streaming: bool,
    pending_newline: bool,
}

impl EventEmitter<Submit> for MessageInput {}
impl EventEmitter<Stop> for MessageInput {}

impl MessageInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Type your message...")
                .clean_on_escape()
                .auto_grow(3, 10)
        });

        cx.subscribe_in(
            &input_state,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary } = event {
                    if *secondary {
                        this.pending_newline = false;
                        return;
                    }

                    if this.pending_newline {
                        // Shift+Enter inserts a newline manually and then still emits PressEnter.
                        // Consume that synthetic enter so it never triggers submit.
                        this.pending_newline = false;
                    } else {
                        this.trim_trailing_newline(window, cx);
                        this.handle_submit(window, cx);
                    }
                }
            },
        )
        .detach();

        Self {
            input_state,
            stream_target: DEFAULT_STREAM_TARGET,
            is_streaming: false,
            pending_newline: false,
        }
    }

    pub fn set_stream_target(&mut self, target: StreamTarget, cx: &mut Context<Self>) {
        self.stream_target = target;
        cx.notify();
    }

    pub fn stream_target(&self) -> StreamTarget {
        self.stream_target
    }

    pub fn set_streaming(&mut self, streaming: bool, cx: &mut Context<Self>) {
        self.is_streaming = streaming;
        if !streaming {
            self.pending_newline = false;
        }
        cx.notify();
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.pending_newline = false;
    }

    fn handle_shift_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_streaming {
            return;
        }

        self.pending_newline = true;
        self.input_state.update(cx, |state, cx| {
            state.insert("\n", window, cx);
        });
        cx.notify();
    }

    fn trim_trailing_newline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            if let Some(trimmed) = value.strip_suffix('\n') {
                state.set_value(trimmed.to_string(), window, cx);
            }
        });
    }

    fn handle_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_streaming {
            return;
        }

        let content = self.input_state.read(cx).value().to_string();
        if content.trim().is_empty() {
            return;
        }

        cx.emit(Submit::new(self.stream_target, content));
        self.clear(window, cx);
    }

    fn handle_stop(&mut self, cx: &mut Context<Self>) {
        if !self.is_streaming {
            return;
        }

        cx.emit(Stop {
            target: self.stream_target,
        });

        // Reset immediately after emitting stop so the input is editable again.
        self.is_streaming = false;
        self.pending_newline = false;
        cx.notify();
    }
}

impl Render for MessageInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let is_streaming = self.is_streaming;
        let action = if is_streaming {
            Button::new("stop")
                .small()
                .danger()
                .icon(IconName::CircleX)
                .child("Stop")
                .on_click(cx.listener(|this, _, _window, cx| {
                    this.handle_stop(cx);
                }))
                .into_any_element()
        } else {
            Button::new("send")
                .small()
                .primary()
                .icon(IconName::ArrowUp)
                .child("Send")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.handle_submit(window, cx);
                }))
                .into_any_element()
        };

        v_flex()
            .bg(theme.background)
            .gap_2()
            .p_3()
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "enter" && event.keystroke.modifiers.shift {
                            this.handle_shift_enter(window, cx);
                        }
                    }))
                    .child(
                        Input::new(&self.input_state)
                            .w_full()
                            .disabled(is_streaming),
                    ),
            )
            .child(div().w_full().flex().justify_end().child(action))
    }
}
