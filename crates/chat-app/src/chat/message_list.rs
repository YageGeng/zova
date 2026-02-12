use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::ops::Range;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    text::TextView,
    v_flex, v_virtual_list,
};

use crate::chat::message::{Message, MessageId, MessageStatus, Role};
use crate::chat::scroll_manager::ScrollManager;

const DEFAULT_CONTENT_WIDTH: Pixels = px(680.);
const LIST_HORIZONTAL_PADDING: Pixels = px(16.);
const CONTENT_WIDTH_CHANGE_EPSILON: f32 = 1.0;
const USER_BUBBLE_MAX_WIDTH: Pixels = px(540.);
const USER_BUBBLE_PADDING_X: Pixels = px(14.);
const USER_BUBBLE_PADDING_Y: Pixels = px(10.);
const ASSISTANT_LABEL_HEIGHT: Pixels = px(16.);
const ASSISTANT_LABEL_GAP: Pixels = px(8.);
const STREAMING_INDICATOR_HEIGHT: Pixels = px(20.);
const STREAMING_INDICATOR_GAP: Pixels = px(8.);
const ERROR_ROW_HEIGHT: Pixels = px(20.);
const ERROR_ROW_GAP: Pixels = px(8.);
const ESTIMATED_TEXT_LINE_HEIGHT: Pixels = px(18.);
const ESTIMATED_CHAR_WIDTH: f32 = 7.0;
const MARKDOWN_SAFE_FALLBACK_THRESHOLD_BYTES: usize = 128 * 1024;

struct SizeCacheEntry {
    layout_hash: u64,
    height: Pixels,
    measured: bool,
}

pub struct MessageList {
    messages: Vec<Message>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_manager: ScrollManager,
    size_cache: HashMap<MessageId, SizeCacheEntry>,
    content_width: Option<Pixels>,
}

impl MessageList {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            messages: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scroll_manager: ScrollManager::new(),
            size_cache: HashMap::new(),
            content_width: None,
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn set_messages(&mut self, messages: Vec<Message>, cx: &mut Context<Self>) {
        let should_request_follow = messages.len() > self.messages.len()
            || messages
                .iter()
                .any(|message| matches!(message.status, MessageStatus::Streaming(_)));

        self.messages = messages;
        self.rebuild_item_sizes();

        if should_request_follow {
            self.scroll_manager.request_scroll_to_bottom_if_following();
        }

        cx.notify();
    }

    pub fn request_scroll_to_bottom(&mut self, cx: &mut Context<Self>) {
        self.scroll_manager.request_scroll_to_bottom();
        cx.notify();
    }

    pub fn reset_scroll_tracking(&mut self, cx: &mut Context<Self>) {
        self.scroll_manager.reset();
        cx.notify();
    }

    fn update_content_width(&mut self, cx: &mut Context<Self>) {
        let list_width = self.scroll_manager.bounds().size.width;
        if list_width <= Pixels::ZERO {
            return;
        }

        let next_content_width = max_pixels(px(1.), list_width - LIST_HORIZONTAL_PADDING * 2);
        let width_changed = self.content_width.is_none_or(|current| {
            (f32::from(current) - f32::from(next_content_width)).abs()
                > CONTENT_WIDTH_CHANGE_EPSILON
        });

        if width_changed {
            self.content_width = Some(next_content_width);

            // Mark cached measurements dirty so item heights can be recalculated for new width.
            for entry in self.size_cache.values_mut() {
                entry.measured = false;
            }

            self.rebuild_item_sizes();
            cx.notify();
        }
    }

    fn rebuild_item_sizes(&mut self) {
        let content_width = self.content_width.unwrap_or(DEFAULT_CONTENT_WIDTH);
        let mut active_ids = HashSet::with_capacity(self.messages.len());
        let mut sizes = Vec::with_capacity(self.messages.len());

        for message in &self.messages {
            let next_hash = layout_hash(message);
            let estimated_height = estimate_message_height(message, content_width);

            let entry = self.size_cache.entry(message.id).or_insert(SizeCacheEntry {
                layout_hash: next_hash,
                height: estimated_height,
                measured: false,
            });

            // Keep cache entries stable by message id and invalidate only on semantic content changes.
            if entry.layout_hash != next_hash {
                entry.layout_hash = next_hash;
                entry.height = estimated_height;
                entry.measured = false;
            } else if !entry.measured {
                entry.height = estimated_height;
            }

            sizes.push(size(px(0.), entry.height));
            active_ids.insert(message.id);
        }

        self.size_cache.retain(|id, _| active_ids.contains(id));
        self.item_sizes = Rc::new(sizes);
    }

    fn measure_visible_items(
        &mut self,
        visible_range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.messages.is_empty() {
            return;
        }

        let content_width = self.content_width.unwrap_or(DEFAULT_CONTENT_WIDTH);
        let available_space = size(
            AvailableSpace::Definite(content_width),
            AvailableSpace::MinContent,
        );
        let mut updated = false;

        for index in visible_range {
            let Some(message) = self.messages.get(index).cloned() else {
                continue;
            };

            let next_hash = layout_hash(&message);
            let estimated_height = estimate_message_height(&message, content_width);

            {
                let entry = self.size_cache.entry(message.id).or_insert(SizeCacheEntry {
                    layout_hash: next_hash,
                    height: estimated_height,
                    measured: false,
                });

                if entry.layout_hash != next_hash {
                    entry.layout_hash = next_hash;
                    entry.height = estimated_height;
                    entry.measured = false;
                }
            }

            let mut row = self.render_message_row(&message, index, cx);
            let measured_height = row.layout_as_root(available_space, window, cx).height;
            let Some(entry) = self.size_cache.get_mut(&message.id) else {
                continue;
            };
            let height_changed = !entry.measured || pixels_changed(entry.height, measured_height);
            if height_changed {
                entry.height = measured_height;
                updated = true;
            }
            entry.measured = true;
        }

        if updated {
            self.rebuild_item_sizes();
            cx.notify();
        }
    }

    fn render_message_row(
        &self,
        message: &Message,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();

        if message.role == Role::User {
            let content = if message.content.is_empty() {
                " ".to_string()
            } else {
                message.content.clone()
            };

            return v_flex()
                .w_full()
                .items_end()
                .child(
                    div()
                        .max_w(USER_BUBBLE_MAX_WIDTH)
                        .px(USER_BUBBLE_PADDING_X)
                        .py(USER_BUBBLE_PADDING_Y)
                        .rounded_lg()
                        .bg(theme.accent)
                        .text_color(theme.accent_foreground)
                        .child(Label::new(content).text_sm()),
                )
                .into_any_element();
        }

        let speaker_label = if message.role == Role::System {
            "System"
        } else {
            "Assistant"
        };

        let content = self.render_assistant_content(message, index);
        let error_message = if let MessageStatus::Error(error) = &message.status {
            Some(error.clone())
        } else {
            None
        };

        v_flex()
            .w_full()
            .gap_2()
            .child(
                Label::new(speaker_label)
                    .text_xs()
                    .text_color(theme.foreground.opacity(0.5)),
            )
            .child(content)
            .when(
                matches!(message.status, MessageStatus::Streaming(_)),
                |column| {
                    column.child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(div().size(px(8.)).rounded_full().bg(theme.primary))
                            .child(
                                Label::new("Streaming")
                                    .text_xs()
                                    .text_color(theme.foreground.opacity(0.65)),
                            ),
                    )
                },
            )
            .when_some(error_message, |column, error| {
                column.child(
                    Label::new(format!("Error: {error}"))
                        .text_xs()
                        .text_color(theme.danger),
                )
            })
            .into_any_element()
    }

    fn render_assistant_content(&self, message: &Message, index: usize) -> AnyElement {
        if message.content.trim().is_empty() {
            let empty_label = if matches!(message.status, MessageStatus::Streaming(_)) {
                "Waiting for response..."
            } else {
                "(empty response)"
            };

            return Label::new(empty_label).text_sm().into_any_element();
        }

        if message.content.len() > MARKDOWN_SAFE_FALLBACK_THRESHOLD_BYTES {
            // Keep markdown rendering predictable by falling back to plain text for oversized payloads.
            return Label::new(message.content.clone())
                .text_sm()
                .into_any_element();
        }

        let markdown_id = ElementId::Name(SharedString::from(format!(
            "assistant-markdown-{}-{index}",
            message.id.0
        )));

        TextView::markdown(markdown_id, message.content.clone())
            .code_block_actions(|code_block, _window, _cx| {
                let code = code_block.code().to_string();
                let mut hasher = DefaultHasher::new();
                hasher.write(code.as_bytes());
                let copy_button_id = format!("copy-code-{}", hasher.finish());

                h_flex().w_full().justify_end().child(
                    Button::new(copy_button_id)
                        .ghost()
                        .small()
                        .icon(IconName::Copy)
                        .child("Copy")
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                        }),
                )
            })
            .selectable(true)
            .into_any_element()
    }
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_content_width(cx);
        self.scroll_manager.update_follow_state();
        self.scroll_manager.apply_pending_scroll();

        v_flex().size_full().min_h_0().child(
            v_virtual_list(
                cx.entity().clone(),
                "message-list",
                self.item_sizes.clone(),
                |this, visible_range, window, cx| {
                    // Measure only visible rows so long histories keep O(visible) layout work.
                    this.update_content_width(cx);
                    this.measure_visible_items(visible_range.clone(), window, cx);
                    visible_range
                        .filter_map(|index| {
                            this.messages
                                .get(index)
                                .cloned()
                                .map(|message| this.render_message_row(&message, index, cx))
                        })
                        .collect::<Vec<_>>()
                },
            )
            .size_full()
            .px_4()
            .py_3()
            .gap_4()
            .track_scroll(self.scroll_manager.handle()),
        )
    }
}

fn layout_hash(message: &Message) -> u64 {
    let mut hasher = DefaultHasher::new();

    hasher.write_u64(message.id.0);

    let role_tag = match message.role {
        Role::System => 0,
        Role::User => 1,
        Role::Assistant => 2,
    };
    hasher.write_u8(role_tag);

    match &message.status {
        MessageStatus::Pending => hasher.write_u8(0),
        MessageStatus::Streaming(session_id) => {
            hasher.write_u8(1);
            hasher.write_u64(session_id.0);
        }
        MessageStatus::Done => hasher.write_u8(2),
        MessageStatus::Error(error) => {
            hasher.write_u8(3);
            hasher.write(error.as_bytes());
        }
        MessageStatus::Cancelled => hasher.write_u8(4),
    }

    hasher.write(message.content.as_bytes());
    hasher.finish()
}

fn estimate_message_height(message: &Message, content_width: Pixels) -> Pixels {
    match message.role {
        Role::User => {
            let bubble_width = min_pixels(content_width, USER_BUBBLE_MAX_WIDTH);
            let text_width = max_pixels(px(1.), bubble_width - USER_BUBBLE_PADDING_X * 2);
            let text_height = estimate_text_height(&message.content, text_width);
            text_height + USER_BUBBLE_PADDING_Y * 2
        }
        Role::System | Role::Assistant => {
            let text_height = if message.content.is_empty() {
                ESTIMATED_TEXT_LINE_HEIGHT
            } else {
                estimate_text_height(&message.content, content_width)
            };

            let mut total_height = ASSISTANT_LABEL_HEIGHT + ASSISTANT_LABEL_GAP + text_height;
            if matches!(message.status, MessageStatus::Streaming(_)) {
                total_height += STREAMING_INDICATOR_GAP + STREAMING_INDICATOR_HEIGHT;
            }
            if matches!(message.status, MessageStatus::Error(_)) {
                total_height += ERROR_ROW_GAP + ERROR_ROW_HEIGHT;
            }

            total_height
        }
    }
}

fn estimate_text_height(content: &str, width: Pixels) -> Pixels {
    if content.is_empty() {
        return ESTIMATED_TEXT_LINE_HEIGHT;
    }

    let width_as_f32 = f32::from(width);
    let chars_per_line = (width_as_f32 / ESTIMATED_CHAR_WIDTH).floor().max(1.0) as usize;

    let mut line_count = 0usize;
    for line in content.lines() {
        let char_count = line.chars().count().max(1);
        line_count += char_count.div_ceil(chars_per_line);
    }

    // Account for the trailing empty line when content ends with a newline.
    if content.ends_with('\n') {
        line_count += 1;
    }

    ESTIMATED_TEXT_LINE_HEIGHT * line_count.max(1)
}

fn max_pixels(a: Pixels, b: Pixels) -> Pixels {
    if f32::from(a) >= f32::from(b) { a } else { b }
}

fn min_pixels(a: Pixels, b: Pixels) -> Pixels {
    if f32::from(a) <= f32::from(b) { a } else { b }
}

fn pixels_changed(a: Pixels, b: Pixels) -> bool {
    (f32::from(a) - f32::from(b)).abs() > 0.5
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualizationMetric {
    pub message_id: MessageId,
    pub estimated_height: f32,
    pub layout_hash: u64,
}

pub fn virtualization_metrics(
    messages: &[Message],
    content_width: f32,
) -> Vec<VirtualizationMetric> {
    let bounded_width = px(content_width.max(1.0));

    messages
        .iter()
        .map(|message| VirtualizationMetric {
            message_id: message.id,
            estimated_height: f32::from(estimate_message_height(message, bounded_width)),
            layout_hash: layout_hash(message),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::message::StreamSessionId;

    #[::core::prelude::v1::test]
    fn large_history_fixture_keeps_row_metrics_deterministic() {
        let mut messages = (0..2_000)
            .map(|index| {
                let role = if index % 2 == 0 {
                    Role::User
                } else {
                    Role::Assistant
                };
                let status = if index == 1_999 {
                    MessageStatus::Streaming(StreamSessionId::new(42))
                } else {
                    MessageStatus::Done
                };

                Message::new(
                    MessageId::new(index as u64 + 1),
                    role,
                    format!("message-{index}: virtualization fixture payload"),
                    status,
                )
            })
            .collect::<Vec<_>>();

        let content_width = px(680.);
        let heights_before = messages
            .iter()
            .map(|message| estimate_message_height(message, content_width))
            .collect::<Vec<_>>();
        let hashes_before = messages.iter().map(layout_hash).collect::<Vec<_>>();

        assert_eq!(heights_before.len(), 2_000);
        assert!(heights_before.iter().all(|height| *height > Pixels::ZERO));

        if let Some(last_message) = messages.last_mut() {
            // Tail-only mutation should invalidate only the final row hash.
            last_message.content.push_str(" [finalized]");
            last_message.status = MessageStatus::Done;
        }

        let heights_after = messages
            .iter()
            .map(|message| estimate_message_height(message, content_width))
            .collect::<Vec<_>>();
        let hashes_after = messages.iter().map(layout_hash).collect::<Vec<_>>();

        assert_eq!(heights_after.len(), 2_000);
        assert!(heights_after.iter().all(|height| *height > Pixels::ZERO));
        assert_eq!(hashes_before[..1_999], hashes_after[..1_999]);
        assert_ne!(hashes_before[1_999], hashes_after[1_999]);
    }
}
