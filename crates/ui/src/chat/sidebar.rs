use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, VirtualListScrollHandle,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    list::ListItem,
    v_flex, v_virtual_list,
};

use crate::chat::events::ConversationSelected;
use crate::chat::message::{ConversationId, Role};
use crate::database::{ConversationRecord, DEFAULT_CONVERSATION_TITLE};
use zova_storage::{
    MessageId as StorageMessageId, MessagePatch, MessageRecord as StorageMessageRecord,
    MessageRole as StorageMessageRole, MessageStore, NewMessage, NewSession, SessionId,
    SessionStore, SqliteStorage,
};

const GROUP_HEADER_HEIGHT: f32 = 26.0;
const CONVERSATION_ROW_HEIGHT: f32 = 40.0;
const DAY_SECONDS: u64 = 60 * 60 * 24;
const DEFAULT_STORAGE_DB_RELATIVE_PATH: &str = ".zova/storage.db";

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
    storage: Option<Arc<SqliteStorage>>,
    conversation_to_session: HashMap<ConversationId, SessionId>,
    session_to_conversation: HashMap<SessionId, ConversationId>,
    next_conversation_id: u64,
}

impl EventEmitter<ConversationSelected> for ChatSidebar {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarSettingsClicked;

impl EventEmitter<SidebarSettingsClicked> for ChatSidebar {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarToggleClicked;

impl EventEmitter<SidebarToggleClicked> for ChatSidebar {}

impl ChatSidebar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search conversations..."));
        let storage = Self::open_storage();

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
            conversations: Vec::new(),
            selected_conversation: None,
            flat_items: Vec::new(),
            item_sizes: Rc::new(Vec::new()),
            scroll_handle: VirtualListScrollHandle::new(),
            storage,
            conversation_to_session: HashMap::new(),
            session_to_conversation: HashMap::new(),
            next_conversation_id: 1,
        };
        sidebar.refresh_from_store();
        sidebar
    }

    pub fn selected_conversation(&self) -> Option<ConversationId> {
        self.selected_conversation
    }

    pub fn conversations(&self) -> &[ConversationRecord] {
        &self.conversations
    }

    pub fn session_id_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Option<SessionId> {
        self.conversation_to_session.get(&conversation_id).copied()
    }

    pub fn reload_from_persistence(&mut self, cx: &mut Context<Self>) {
        self.refresh_from_store();
        cx.notify();
    }

    pub fn create_conversation(&mut self, cx: &mut Context<Self>) -> Option<ConversationId> {
        let Some(storage) = self.storage.as_ref() else {
            tracing::error!("cannot create conversation because storage is unavailable");
            return None;
        };

        let created = match storage.create_session(NewSession {
            title: DEFAULT_CONVERSATION_TITLE.to_string(),
        }) {
            Ok(created) => created,
            Err(error) => {
                tracing::error!("failed to create conversation in store: {error}");
                return None;
            }
        };

        // Keep conversation/session IDs stable for this process so stream targets stay deterministic.
        let conversation_id = if let Some(existing) = self.session_to_conversation.get(&created.id)
        {
            *existing
        } else {
            let allocated = self.alloc_conversation_id();
            self.session_to_conversation.insert(created.id, allocated);
            self.conversation_to_session.insert(allocated, created.id);
            allocated
        };

        self.refresh_from_store();
        self.select_conversation(conversation_id, cx);
        Some(conversation_id)
    }

    pub fn load_conversation(&self, conversation_id: ConversationId) -> Option<ConversationRecord> {
        let storage = self.storage.as_ref()?;
        let session_id = self.session_id_for_conversation(conversation_id)?;

        match storage.get_session(session_id) {
            Ok(Some(session)) => Some(ConversationRecord::new(
                conversation_id,
                session.title,
                session.updated_at_unix_seconds,
            )),
            Ok(None) => None,
            Err(error) => {
                tracing::error!("failed to load conversation {conversation_id:?}: {error}");
                None
            }
        }
    }

    pub fn list_persisted_messages(
        &self,
        conversation_id: ConversationId,
    ) -> Vec<StorageMessageRecord> {
        let Some(storage) = self.storage.as_ref() else {
            return Vec::new();
        };
        let Some(session_id) = self.session_id_for_conversation(conversation_id) else {
            tracing::warn!("missing session mapping for conversation {conversation_id:?}");
            return Vec::new();
        };

        match storage.list_messages(session_id) {
            Ok(messages) => messages,
            Err(error) => {
                tracing::error!(
                    "failed to list persisted messages for {conversation_id:?}: {error}"
                );
                Vec::new()
            }
        }
    }

    pub fn append_persisted_message(
        &self,
        conversation_id: ConversationId,
        role: Role,
        content: String,
    ) -> Option<StorageMessageRecord> {
        let storage = self.storage.as_ref()?;
        let Some(session_id) = self.session_id_for_conversation(conversation_id) else {
            tracing::warn!("missing session mapping for conversation {conversation_id:?}");
            return None;
        };

        match storage.append_message(
            session_id,
            NewMessage {
                role: chat_role_to_storage(role),
                content,
            },
        ) {
            Ok(message) => Some(message),
            Err(error) => {
                tracing::error!(
                    "failed to append persisted message for {conversation_id:?}: {error}"
                );
                None
            }
        }
    }

    pub fn update_persisted_message_content(
        &self,
        conversation_id: ConversationId,
        message_id: StorageMessageId,
        content: String,
    ) -> Option<StorageMessageRecord> {
        let storage = self.storage.as_ref()?;
        let Some(session_id) = self.session_id_for_conversation(conversation_id) else {
            tracing::warn!("missing session mapping for conversation {conversation_id:?}");
            return None;
        };

        // All message mutations stay scoped by (session_id, msg_id) to prevent cross-session writes.
        match storage.update_message(
            session_id,
            message_id,
            MessagePatch {
                content: Some(content),
            },
        ) {
            Ok(message) => Some(message),
            Err(error) => {
                tracing::error!(
                    "failed to update persisted message {message_id} for {conversation_id:?}: {error}"
                );
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
        let Some(storage) = self.storage.as_ref() else {
            tracing::error!("storage unavailable while refreshing sidebar");
            self.conversations.clear();
            self.conversation_to_session.clear();
            self.session_to_conversation.clear();
            self.selected_conversation = None;
            self.rebuild_flat_items();
            return;
        };

        match storage.list_sessions(false) {
            Ok(sessions) => {
                let mut conversations = Vec::with_capacity(sessions.len());
                let mut conversation_to_session = HashMap::with_capacity(sessions.len());
                let mut session_to_conversation = HashMap::with_capacity(sessions.len());

                for session in sessions {
                    let conversation_id =
                        if let Some(existing) = self.session_to_conversation.get(&session.id) {
                            *existing
                        } else {
                            self.alloc_conversation_id()
                        };

                    conversation_to_session.insert(conversation_id, session.id);
                    session_to_conversation.insert(session.id, conversation_id);
                    conversations.push(ConversationRecord::new(
                        conversation_id,
                        session.title,
                        session.updated_at_unix_seconds,
                    ));
                }

                self.conversations = conversations;
                self.conversation_to_session = conversation_to_session;
                self.session_to_conversation = session_to_conversation;

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
                self.conversation_to_session.clear();
                self.session_to_conversation.clear();
                self.selected_conversation = None;
                self.rebuild_flat_items();
            }
        }
    }

    fn alloc_conversation_id(&mut self) -> ConversationId {
        let next = ConversationId::new(self.next_conversation_id);
        self.next_conversation_id = self.next_conversation_id.saturating_add(1);
        next
    }

    fn open_storage() -> Option<Arc<SqliteStorage>> {
        // Sidebar constructor is sync, so storage bootstrap runs in a local current-thread runtime.
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::error!("failed to initialize runtime for sqlite storage: {error}");
                return None;
            }
        };

        let storage = match runtime.block_on(SqliteStorage::open(DEFAULT_STORAGE_DB_RELATIVE_PATH))
        {
            Ok(storage) => storage,
            Err(error) => {
                tracing::error!("failed to open sqlite storage: {error}");
                return None;
            }
        };

        match storage.import_legacy_conversations_from_default_path() {
            Ok(report) if report.source_missing => {
                tracing::debug!("legacy conversation TSV not found; skipping import");
            }
            Ok(report) if report.imported_sessions > 0 || report.skipped_rows > 0 => {
                tracing::info!(
                    "legacy import complete: imported_sessions={}, skipped_rows={}",
                    report.imported_sessions,
                    report.skipped_rows
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::error!("failed to import legacy conversations into sqlite: {error}");
            }
        }

        Some(Arc::new(storage))
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
            px(0.),
        );
        append_group(
            &mut flat_items,
            &mut item_sizes,
            "Yesterday",
            yesterday_items,
            px(0.),
        );
        append_group(
            &mut flat_items,
            &mut item_sizes,
            "Older",
            older_items,
            px(0.),
        );

        self.flat_items = flat_items;
        self.item_sizes = Rc::new(item_sizes);
    }

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .px_3()
            .pt(px(8.))
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

    fn render_footer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .min_w_0()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .id("sidebar-user-center")
                    .size(px(32.))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::CircleUser)
                            .size(px(18.))
                            .text_color(theme.foreground),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("sidebar-settings")
                            .ghost()
                            .small()
                            .icon(IconName::Settings)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(SidebarSettingsClicked);
                            })),
                    )
                    .child(
                        Button::new("sidebar-toggle")
                            .ghost()
                            .small()
                            .icon(IconName::PanelLeftClose)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(SidebarToggleClicked);
                            })),
                    ),
            )
    }
}

impl Render for ChatSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .bg(theme.background)
            .child(self.render_toolbar(cx))
            .child(self.render_history_list(cx))
            .child(self.render_footer(cx))
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

fn chat_role_to_storage(role: Role) -> StorageMessageRole {
    match role {
        Role::System => StorageMessageRole::System,
        Role::User => StorageMessageRole::User,
        Role::Assistant => StorageMessageRole::Assistant,
    }
}
