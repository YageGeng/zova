use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::*;
use gpui_component::{
    ActiveTheme, IconName, Sizable, VirtualListScrollHandle,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    list::ListItem,
    v_flex, v_virtual_list,
};

use crate::chat::events::ConversationSelected;
use crate::chat::message::ConversationId;
use crate::database::{ConversationRecord, ConversationStore, DEFAULT_CONVERSATION_TITLE};

const GROUP_HEADER_HEIGHT: f32 = 26.0;
const CONVERSATION_ROW_HEIGHT: f32 = 40.0;
const DAY_SECONDS: u64 = 60 * 60 * 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConversationAgeGroup {
    Today,
    Yesterday,
    Older,
}

#[derive(Debug, Clone)]
enum SidebarListItem {
    GroupHeader(&'static str),
    Conversation(ConversationRecord),
}

pub struct ChatSidebar {
    search_input: Entity<InputState>,
    search_query: String,
    conversations: Vec<ConversationRecord>,
    selected_conversation: Option<ConversationId>,
    flat_items: Vec<SidebarListItem>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
    conversation_store: ConversationStore,
}

impl EventEmitter<ConversationSelected> for ChatSidebar {}

impl ChatSidebar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search conversations..."));
        let conversation_store = ConversationStore::default();
        let conversations = match conversation_store.list_conversations() {
            Ok(conversations) => conversations,
            Err(error) => {
                tracing::error!("failed to load conversations from store: {error}");
                Vec::new()
            }
        };

        cx.subscribe_in(
            &search_input,
            window,
            |this, _, _event: &InputEvent, _window, cx| {
                this.search_query = this.search_input.read(cx).value().to_string();
                this.rebuild_flat_items();
                cx.notify();
            },
        )
        .detach();

        let mut sidebar = Self {
            search_input,
            search_query: String::new(),
            conversations,
            selected_conversation: None,
            flat_items: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scroll_handle: VirtualListScrollHandle::new(),
            conversation_store,
        };
        sidebar.rebuild_flat_items();
        sidebar
    }

    pub fn selected_conversation(&self) -> Option<ConversationId> {
        self.selected_conversation
    }

    pub fn conversations(&self) -> &[ConversationRecord] {
        &self.conversations
    }

    pub fn reload_from_persistence(&mut self, cx: &mut Context<Self>) {
        self.refresh_from_store();
        cx.notify();
    }

    pub fn create_conversation(&mut self, cx: &mut Context<Self>) -> Option<ConversationId> {
        let created = match self
            .conversation_store
            .create_conversation(DEFAULT_CONVERSATION_TITLE)
        {
            Ok(created) => created,
            Err(error) => {
                tracing::error!("failed to create conversation in store: {error}");
                return None;
            }
        };

        self.refresh_from_store();
        self.select_conversation(created.id, cx);
        Some(created.id)
    }

    pub fn load_conversation(&self, conversation_id: ConversationId) -> Option<ConversationRecord> {
        match self.conversation_store.load_conversation(conversation_id) {
            Ok(conversation) => conversation,
            Err(error) => {
                tracing::error!("failed to load conversation {conversation_id:?}: {error}");
                None
            }
        }
    }

    pub fn select_conversation(&mut self, conversation_id: ConversationId, cx: &mut Context<Self>) {
        self.selected_conversation = Some(conversation_id);
        cx.emit(ConversationSelected { conversation_id });
        cx.notify();
    }

    fn refresh_from_store(&mut self) {
        match self.conversation_store.list_conversations() {
            Ok(conversations) => {
                self.conversations = conversations;

                if self.selected_conversation.is_some_and(|selected| {
                    !self
                        .conversations
                        .iter()
                        .any(|conversation| conversation.id == selected)
                }) {
                    self.selected_conversation = None;
                }

                self.rebuild_flat_items();
            }
            Err(error) => {
                tracing::error!("failed to refresh conversations from store: {error}");
                self.conversations.clear();
                self.selected_conversation = None;
                self.rebuild_flat_items();
            }
        }
    }

    fn rebuild_flat_items(&mut self) {
        let normalized_query = self.search_query.trim().to_ascii_lowercase();
        let now_unix_seconds = unix_now_seconds();

        let mut today_items = Vec::new();
        let mut yesterday_items = Vec::new();
        let mut older_items = Vec::new();

        // Keep ordering deterministic by preserving the repository order within each group.
        for conversation in self.conversations.iter().cloned() {
            if !matches_query(&conversation, &normalized_query) {
                continue;
            }

            match classify_group(conversation.updated_at_unix_seconds, now_unix_seconds) {
                ConversationAgeGroup::Today => today_items.push(conversation),
                ConversationAgeGroup::Yesterday => yesterday_items.push(conversation),
                ConversationAgeGroup::Older => older_items.push(conversation),
            }
        }

        let mut flat_items = Vec::new();
        let mut item_sizes = Vec::new();

        append_group(
            &mut flat_items,
            &mut item_sizes,
            "Today",
            today_items,
            px(240.),
        );
        append_group(
            &mut flat_items,
            &mut item_sizes,
            "Yesterday",
            yesterday_items,
            px(240.),
        );
        append_group(
            &mut flat_items,
            &mut item_sizes,
            "Older",
            older_items,
            px(240.),
        );

        self.flat_items = flat_items;
        self.item_sizes = Rc::new(item_sizes);
    }

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .gap_2()
            .px_3()
            .pb_2()
            .child(Input::new(&self.search_input).w_full().small())
            .child(
                Button::new("new")
                    .small()
                    .primary()
                    .icon(IconName::Plus)
                    .child("New")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.create_conversation(cx);
                    })),
            )
    }

    fn render_empty_state(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let message = if self.conversations.is_empty() {
            "No conversations yet"
        } else {
            "No conversations match your search"
        };

        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .px_4()
            .child(
                Label::new(message)
                    .text_sm()
                    .text_color(theme.foreground.opacity(0.55)),
            )
            .into_any_element()
    }

    fn render_history_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.flat_items.is_empty() {
            return self.render_empty_state(cx);
        }

        let selected = self.selected_conversation;
        let item_sizes = self.item_sizes.clone();
        let items = self.flat_items.clone();

        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                v_virtual_list(
                    cx.entity().clone(),
                    "conversation-list",
                    item_sizes,
                    move |_this, visible_range, _scroll_handle, cx| {
                        let theme = cx.theme();

                        visible_range
                            .map(|index| match &items[index] {
                                SidebarListItem::GroupHeader(name) => div()
                                    .w_full()
                                    .h(px(GROUP_HEADER_HEIGHT))
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .child(
                                        Label::new(*name)
                                            .text_xs()
                                            .text_color(theme.foreground.opacity(0.5)),
                                    )
                                    .into_any_element(),
                                SidebarListItem::Conversation(conversation) => {
                                    let conversation_id = conversation.id;
                                    let title = conversation.title.clone();
                                    let is_selected = selected == Some(conversation_id);

                                    div()
                                        .w_full()
                                        .h(px(CONVERSATION_ROW_HEIGHT))
                                        .px_2()
                                        .child(
                                            ListItem::new(("conversation", index))
                                                .w_full()
                                                .h_full()
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .selected(is_selected)
                                                .on_click(cx.listener(
                                                    move |this, _event: &ClickEvent, _window, cx| {
                                                        this.select_conversation(conversation_id, cx);
                                                    },
                                                ))
                                                .child(
                                                    h_flex().w_full().items_center().child(
                                                        div().flex_1().min_w_0().truncate().child(
                                                            Label::new(title.clone()).text_sm(),
                                                        ),
                                                    ),
                                                ),
                                        )
                                        .into_any_element()
                                }
                            })
                            .collect()
                    },
                )
                .w_full()
                .flex_1()
                .track_scroll(&self.scroll_handle),
            )
            .into_any_element()
    }
}

impl Render for ChatSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.background)
            .pt(px(44.))
            .child(self.render_toolbar(cx))
            .child(self.render_history_list(cx))
    }
}

fn append_group(
    flat_items: &mut Vec<SidebarListItem>,
    item_sizes: &mut Vec<Size<Pixels>>,
    title: &'static str,
    conversations: Vec<ConversationRecord>,
    item_width: Pixels,
) {
    if conversations.is_empty() {
        return;
    }

    flat_items.push(SidebarListItem::GroupHeader(title));
    item_sizes.push(size(item_width, px(GROUP_HEADER_HEIGHT)));

    for conversation in conversations {
        flat_items.push(SidebarListItem::Conversation(conversation));
        item_sizes.push(size(item_width, px(CONVERSATION_ROW_HEIGHT)));
    }
}

fn matches_query(conversation: &ConversationRecord, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    conversation.title.to_ascii_lowercase().contains(query)
}

fn classify_group(updated_at_unix_seconds: u64, now_unix_seconds: u64) -> ConversationAgeGroup {
    let age_seconds = now_unix_seconds.saturating_sub(updated_at_unix_seconds);

    // Use elapsed-time buckets to avoid timezone dependencies in MVP persistence.
    if age_seconds < DAY_SECONDS {
        ConversationAgeGroup::Today
    } else if age_seconds < DAY_SECONDS * 2 {
        ConversationAgeGroup::Yesterday
    } else {
        ConversationAgeGroup::Older
    }
}

fn unix_now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}
