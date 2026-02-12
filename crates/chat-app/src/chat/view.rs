use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_tokio_bridge::Tokio;

use crate::chat::events::{ConversationSelected, Stop, Submit};
use crate::chat::message::{
    Conversation, ConversationId, Message, MessageId, MessageStatus, Role, StreamSessionId,
    StreamTarget,
};
use crate::chat::{ChatSidebar, MessageInput, MessageList, StreamEventMapped, StreamEventPayload};
use crate::llm::{
    DEFAULT_OPENAI_MODEL, LlmProvider, ProviderConfig, ProviderEventStream, ProviderMessage,
    ProviderStreamHandle, ProviderWorker, StreamRequest, create_provider,
};
use crate::model_selector::{ModelSelected, ModelSelector, ModelSelectorSettingsClicked};
use crate::settings::{SettingsChanged, SettingsClose, SettingsState, SettingsView};

pub const STREAM_DEBOUNCE_MS: u64 = 50;

/// Coordinator-level stream metadata kept outside the domain model.
#[derive(Debug, Clone, Copy)]
struct ActiveStream {
    target: StreamTarget,
    assistant_message_id: MessageId,
}

/// Parent coordinator for sidebar/message list/input/provider orchestration.
pub struct ChatView {
    sidebar: Entity<ChatSidebar>,
    message_list: Entity<MessageList>,
    message_input: Entity<MessageInput>,
    model_selector: Entity<ModelSelector>,
    _settings_state: Entity<SettingsState>,
    settings_view: Entity<SettingsView>,
    settings_open: bool,
    provider: Option<Arc<dyn LlmProvider>>,
    current_model_id: String,
    conversations: HashMap<ConversationId, Conversation>,
    active_conversation_id: Option<ConversationId>,
    next_message_id: u64,
    next_stream_session_id: u64,
    active_stream: Option<ActiveStream>,
    stream_worker_task: Option<Task<Result<(), gpui_tokio_bridge::JoinError>>>,
    stream_reader_task: Option<Task<()>>,
    stream_debounce_task: Option<Task<()>>,
    pending_stream_chunk: String,
    provider_error: Option<String>,
}

impl ChatView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| ChatSidebar::new(window, cx));
        let message_list = cx.new(MessageList::new);
        let message_input = cx.new(|cx| MessageInput::new(window, cx));
        let settings_state = SettingsState::new(cx);

        let mut conversations = HashMap::new();
        for record in sidebar.read(cx).conversations().iter().cloned() {
            conversations.insert(record.id, Conversation::new(record.id, record.title));
        }

        let mut initial_conversation_id = sidebar
            .read(cx)
            .conversations()
            .first()
            .map(|record| record.id);

        if initial_conversation_id.is_none() {
            initial_conversation_id =
                sidebar.update(cx, |sidebar, cx| sidebar.create_conversation(cx));
            if let Some(created_id) = initial_conversation_id {
                if let Some(record) = sidebar.read(cx).load_conversation(created_id) {
                    conversations.insert(
                        created_id,
                        Conversation::new(record.id, record.title.clone()),
                    );
                } else {
                    conversations.insert(
                        created_id,
                        Conversation::new(created_id, format!("Conversation {}", created_id.0)),
                    );
                }
            }
        }

        if let Some(conversation_id) = initial_conversation_id {
            sidebar.update(cx, |sidebar, cx| {
                sidebar.select_conversation(conversation_id, cx);
            });
        }

        // Initialize provider from persisted settings with environment fallback
        let (provider, current_model_id, provider_error) =
            Self::initialize_provider(&settings_state, cx);

        let model_selector = cx.new(|_| ModelSelector::new(&current_model_id));
        model_selector.update(cx, |selector, cx| {
            selector.set_provider(provider.clone(), cx);
        });

        let settings_view = cx.new(|cx| SettingsView::new(&settings_state, window, cx));

        let mut this = Self {
            sidebar: sidebar.clone(),
            message_list: message_list.clone(),
            message_input: message_input.clone(),
            model_selector: model_selector.clone(),
            _settings_state: settings_state.clone(),
            settings_view: settings_view.clone(),
            settings_open: false,
            provider,
            current_model_id,
            conversations,
            active_conversation_id: None,
            next_message_id: 1,
            next_stream_session_id: 1,
            active_stream: None,
            stream_worker_task: None,
            stream_reader_task: None,
            stream_debounce_task: None,
            pending_stream_chunk: String::new(),
            provider_error,
        };

        if let Some(conversation_id) = initial_conversation_id {
            this.activate_conversation(conversation_id, cx);
        }

        cx.subscribe(&sidebar, |this, _, event: &ConversationSelected, cx| {
            this.handle_conversation_selected(*event, cx);
        })
        .detach();

        cx.subscribe(&message_input, |this, _, event: &Submit, cx| {
            this.handle_submit(event.clone(), cx);
        })
        .detach();

        cx.subscribe(&message_input, |this, _, event: &Stop, cx| {
            this.handle_stop(*event, cx);
        })
        .detach();

        cx.subscribe(&model_selector, |this, _, event: &ModelSelected, cx| {
            this.handle_model_selected(event.clone(), cx);
        })
        .detach();

        cx.subscribe(
            &model_selector,
            |this, _, _event: &ModelSelectorSettingsClicked, cx| {
                this.open_settings(cx);
            },
        )
        .detach();

        cx.subscribe(&settings_state, |this, _, event: &SettingsChanged, cx| {
            this.handle_settings_changed(event, cx);
        })
        .detach();

        cx.subscribe(&settings_view, |this, _, _event: &SettingsClose, cx| {
            this.close_settings(cx);
        })
        .detach();

        this
    }

    pub fn sidebar(&self) -> &Entity<ChatSidebar> {
        &self.sidebar
    }

    pub fn create_conversation(&mut self, cx: &mut Context<Self>) {
        let _ = self
            .sidebar
            .update(cx, |sidebar, cx| sidebar.create_conversation(cx));
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings_open {
            return;
        }
        self.settings_open = true;
        cx.notify();
    }

    fn open_settings_click(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings(cx);
    }

    fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = false;
        cx.notify();
    }

    fn handle_settings_changed(&mut self, event: &SettingsChanged, cx: &mut Context<Self>) {
        if self.active_stream.is_some() {
            self.cancel_active_stream(cx);
        }

        event.settings.apply_theme(None, cx);
        cx.refresh_windows();

        match Self::create_provider_from_settings(&event.settings) {
            Ok((provider, model_id)) => {
                self.provider = provider.clone();
                self.current_model_id = model_id.clone();
                self.provider_error = None;

                self.model_selector.update(cx, |selector, cx| {
                    selector.set_provider(provider, cx);
                    selector.set_model_id(model_id, cx);
                });

                tracing::info!("reloaded provider adapter with new settings");
            }
            Err(error) => {
                self.provider = None;
                self.provider_error = Some(format!("{}", error));
                self.model_selector.update(cx, |selector, cx| {
                    selector.set_provider(None, cx);
                });
                tracing::error!("failed to reload provider adapter: {}", error);
            }
        }

        cx.notify();
    }

    fn handle_model_selected(&mut self, event: ModelSelected, cx: &mut Context<Self>) {
        self.current_model_id = event.model_id;
        cx.notify();
    }

    fn initialize_provider(
        settings_state: &Entity<SettingsState>,
        cx: &mut Context<Self>,
    ) -> (Option<Arc<dyn LlmProvider>>, String, Option<String>) {
        let settings = settings_state.read(cx).settings().clone();

        if settings.is_valid() {
            match Self::create_provider_from_settings(&settings) {
                Ok((provider, model_id)) => {
                    tracing::info!("initialized provider from persisted settings");
                    return (provider, model_id, None);
                }
                Err(e) => {
                    tracing::warn!(
                        "failed to create provider from persisted settings, falling back: {}",
                        e
                    );
                }
            }
        }

        Self::provider_from_environment()
    }

    fn create_provider_from_settings(
        settings: &crate::settings::state::ProviderSettings,
    ) -> Result<(Option<Arc<dyn LlmProvider>>, String), crate::llm::ProviderError> {
        let config = settings.to_provider_config();

        let Some(config) = config else {
            return Ok((None, DEFAULT_OPENAI_MODEL.to_string()));
        };

        match create_provider(config) {
            Ok(provider) => {
                let model_id = provider.default_model().to_string();
                Ok((Some(provider), model_id))
            }
            Err(error) => Err(error),
        }
    }

    fn provider_from_environment() -> (Option<Arc<dyn LlmProvider>>, String, Option<String>) {
        let api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let Some(api_key) = api_key else {
            return (None, DEFAULT_OPENAI_MODEL.to_string(), None);
        };

        let default_model = std::env::var("OPENAI_MODEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        let config = ProviderConfig::new("openai", api_key, base_url, default_model);

        match create_provider(config) {
            Ok(provider) => {
                let model_id = provider.default_model().to_string();
                (Some(provider), model_id, None)
            }
            Err(error) => {
                tracing::error!("failed to initialize provider adapter: {error}");
                (
                    None,
                    DEFAULT_OPENAI_MODEL.to_string(),
                    Some(format!("Provider error: {}", error)),
                )
            }
        }
    }

    fn handle_conversation_selected(
        &mut self,
        event: ConversationSelected,
        cx: &mut Context<Self>,
    ) {
        if self.active_conversation_id == Some(event.conversation_id) {
            return;
        }

        if self.active_stream.is_some() {
            // MVP isolation rule: selecting another conversation cancels active streaming immediately.
            self.cancel_active_stream(cx);
        }

        self.activate_conversation(event.conversation_id, cx);
    }

    fn activate_conversation(&mut self, conversation_id: ConversationId, cx: &mut Context<Self>) {
        self.ensure_conversation_exists(conversation_id, cx);
        self.active_conversation_id = Some(conversation_id);

        self.message_input.update(cx, |input, cx| {
            input.set_streaming(false, cx);
        });

        self.update_input_stream_target(cx);
        self.sync_active_conversation_messages(cx, true);
        cx.notify();
    }

    fn handle_submit(&mut self, event: Submit, cx: &mut Context<Self>) {
        let Some(active_conversation_id) = self.active_conversation_id else {
            return;
        };

        if event.target.conversation_id != active_conversation_id {
            return;
        }

        if self.active_stream.is_some() {
            // Single-stream MVP: ignore additional submits while one stream is active.
            return;
        }

        if self.provider.is_none() {
            self.push_provider_not_configured_error(active_conversation_id, cx);
            return;
        }

        self.ensure_conversation_exists(active_conversation_id, cx);

        let user_message_id = self.alloc_message_id();
        let assistant_message_id = self.alloc_message_id();

        let request_messages = {
            let Some(conversation) = self.conversations.get_mut(&active_conversation_id) else {
                return;
            };

            if conversation
                .apply_stream_transition(event.start_transition())
                .is_err()
            {
                return;
            }

            conversation.messages.push(Message::new(
                user_message_id,
                Role::User,
                event.content.clone(),
                MessageStatus::Done,
            ));

            conversation.messages.push(Message::assistant_streaming(
                assistant_message_id,
                event.target.session_id,
            ));

            Self::build_provider_messages(conversation)
        };

        self.active_stream = Some(ActiveStream {
            target: event.target,
            assistant_message_id,
        });

        self.pending_stream_chunk.clear();
        self.stream_debounce_task = None;

        self.message_input.update(cx, |input, cx| {
            input.set_streaming(true, cx);
        });

        self.sync_active_conversation_messages(cx, false);

        // Reserve the next session id immediately so follow-up submissions never reuse a target.
        self.next_stream_session_id = self.next_stream_session_id.saturating_add(1);

        let request = StreamRequest::new(
            event.target,
            self.current_model_id.clone(),
            request_messages,
        );
        let stream_result = self
            .provider
            .as_ref()
            .expect("provider checked above")
            .stream_chat(request);

        match stream_result {
            Ok(handle) => self.spawn_stream_pipeline(handle, cx),
            Err(error) => {
                self.finish_stream_with_error(event.target, error.to_string(), cx);
            }
        }
    }

    fn spawn_stream_pipeline(&mut self, handle: ProviderStreamHandle, cx: &mut Context<Self>) {
        self.spawn_stream_worker(handle.worker, cx);
        self.spawn_stream_reader(handle.stream, cx);
    }

    fn spawn_stream_worker(&mut self, worker: ProviderWorker, cx: &mut Context<Self>) {
        self.stream_worker_task = Some(Tokio::spawn(cx, worker));
    }

    fn handle_stop(&mut self, event: Stop, cx: &mut Context<Self>) {
        let Some(active_stream) = self.active_stream else {
            return;
        };

        if active_stream.target != event.target {
            // Ignore stale stop events that do not match the in-flight stream target.
            return;
        }

        self.cancel_active_stream(cx);
    }

    fn spawn_stream_reader(&mut self, mut stream: ProviderEventStream, cx: &mut Context<Self>) {
        let stream_target = stream.target();

        self.stream_reader_task = Some(cx.spawn(async move |this, cx| {
            while let Some(event) = stream.recv().await {
                let _ = this.update(cx, |this, cx| {
                    this.handle_stream_event(event, cx);
                });
            }

            let _ = this.update(cx, |this, cx| {
                this.handle_stream_reader_closed(stream_target, cx);
            });
        }));
    }

    fn handle_stream_event(&mut self, event: StreamEventMapped, cx: &mut Context<Self>) {
        if !self.stream_event_is_current(event.target) {
            // Strict target equality prevents chunk leakage across conversation/session boundaries.
            return;
        }

        match event.payload {
            StreamEventPayload::Delta(chunk) | StreamEventPayload::ReasoningDelta(chunk) => {
                self.pending_stream_chunk.push_str(&chunk);
                self.schedule_debounced_stream_flush(cx);
            }
            StreamEventPayload::Done => {
                self.flush_pending_stream_chunk(cx);
                self.finish_stream_with_done(event.target, cx);
            }
            StreamEventPayload::Error(message) => {
                self.flush_pending_stream_chunk(cx);
                self.finish_stream_with_error(event.target, message, cx);
            }
        }
    }

    fn handle_stream_reader_closed(&mut self, target: StreamTarget, cx: &mut Context<Self>) {
        self.stream_worker_task = None;
        self.stream_reader_task = None;

        if self.stream_event_is_current(target) {
            self.finish_stream_with_error(
                target,
                "provider stream ended before a terminal event".to_string(),
                cx,
            );
        }
    }

    fn schedule_debounced_stream_flush(&mut self, cx: &mut Context<Self>) {
        if self.stream_debounce_task.is_some() {
            return;
        }

        self.stream_debounce_task = Some(cx.spawn(async move |this, cx| {
            // Debounce token bursts into a single UI mutation roughly every 50ms.
            cx.background_executor()
                .timer(Duration::from_millis(STREAM_DEBOUNCE_MS))
                .await;

            let _ = this.update(cx, |this, cx| {
                this.flush_pending_stream_chunk(cx);
                this.stream_debounce_task = None;
            });
        }));
    }

    fn flush_pending_stream_chunk(&mut self, cx: &mut Context<Self>) {
        if self.pending_stream_chunk.is_empty() {
            return;
        }

        let Some(active_stream) = self.active_stream else {
            self.pending_stream_chunk.clear();
            return;
        };

        if !self.stream_event_is_current(active_stream.target) {
            self.pending_stream_chunk.clear();
            return;
        }

        let chunk = std::mem::take(&mut self.pending_stream_chunk);
        let Some(conversation) = self
            .conversations
            .get_mut(&active_stream.target.conversation_id)
        else {
            return;
        };

        if let Some(message) = conversation
            .messages
            .iter_mut()
            .find(|message| message.id == active_stream.assistant_message_id)
        {
            message.content.push_str(&chunk);
        }

        if self.active_conversation_id == Some(active_stream.target.conversation_id) {
            self.sync_active_conversation_messages(cx, false);
        }
    }

    fn finish_stream_with_done(&mut self, target: StreamTarget, cx: &mut Context<Self>) {
        let Some(active_stream) = self.active_stream else {
            return;
        };

        if active_stream.target != target {
            return;
        }

        self.finalize_stream(
            target,
            MessageStatus::Done,
            crate::chat::StreamTransition::Complete(target),
            cx,
        );
    }

    fn finish_stream_with_error(
        &mut self,
        target: StreamTarget,
        message: String,
        cx: &mut Context<Self>,
    ) {
        let Some(active_stream) = self.active_stream else {
            return;
        };

        if active_stream.target != target {
            return;
        }

        self.finalize_stream(
            target,
            MessageStatus::Error(message.clone()),
            crate::chat::StreamTransition::Fail { target, message },
            cx,
        );
    }

    fn cancel_active_stream(&mut self, cx: &mut Context<Self>) {
        let Some(active_stream) = self.active_stream else {
            return;
        };

        // Dropping the task cancels the stream reader and drops ProviderEventStream,
        // which in turn signals cancellation to the provider worker.
        self.stream_worker_task = None;
        self.stream_reader_task = None;

        self.finalize_stream(
            active_stream.target,
            MessageStatus::Cancelled,
            crate::chat::StreamTransition::Cancel(active_stream.target),
            cx,
        );
    }

    fn finalize_stream(
        &mut self,
        target: StreamTarget,
        final_status: MessageStatus,
        transition: crate::chat::StreamTransition,
        cx: &mut Context<Self>,
    ) {
        let Some(active_stream) = self.active_stream else {
            return;
        };

        if active_stream.target != target {
            return;
        }

        self.pending_stream_chunk.clear();
        self.stream_debounce_task = None;
        self.stream_worker_task = None;

        if let Some(conversation) = self.conversations.get_mut(&target.conversation_id) {
            let _ = conversation.apply_stream_transition(transition);

            if let Some(message) = conversation
                .messages
                .iter_mut()
                .find(|message| message.id == active_stream.assistant_message_id)
            {
                message.status = final_status;
            }
        }

        self.active_stream = None;
        self.message_input.update(cx, |input, cx| {
            input.set_streaming(false, cx);
        });

        self.update_input_stream_target(cx);

        if self.active_conversation_id == Some(target.conversation_id) {
            self.sync_active_conversation_messages(cx, false);
        }

        cx.notify();
    }

    fn update_input_stream_target(&mut self, cx: &mut Context<Self>) {
        let Some(conversation_id) = self.active_conversation_id else {
            return;
        };

        let target = StreamTarget::new(
            conversation_id,
            StreamSessionId::new(self.next_stream_session_id),
        );

        self.message_input.update(cx, |input, cx| {
            input.set_stream_target(target, cx);
        });
    }

    fn sync_active_conversation_messages(&mut self, cx: &mut Context<Self>, reset_scroll: bool) {
        let messages = self
            .active_conversation_id
            .and_then(|conversation_id| self.conversations.get(&conversation_id))
            .map(|conversation| conversation.messages.clone())
            .unwrap_or_default();

        self.message_list.update(cx, |list, cx| {
            if reset_scroll {
                list.reset_scroll_tracking(cx);
            }
            list.set_messages(messages, cx);
        });
    }

    fn ensure_conversation_exists(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) {
        if self.conversations.contains_key(&conversation_id) {
            return;
        }

        let title = self
            .sidebar
            .read(cx)
            .load_conversation(conversation_id)
            .map(|record| record.title)
            .unwrap_or_else(|| format!("Conversation {}", conversation_id.0));

        self.conversations
            .insert(conversation_id, Conversation::new(conversation_id, title));
    }

    fn build_provider_messages(conversation: &Conversation) -> Vec<ProviderMessage> {
        conversation
            .messages
            .iter()
            .filter(|message| !message.content.trim().is_empty())
            .filter(|message| !matches!(message.status, MessageStatus::Streaming(_)))
            .map(|message| ProviderMessage::new(message.role, message.content.clone()))
            .collect()
    }

    fn push_provider_not_configured_error(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) {
        self.ensure_conversation_exists(conversation_id, cx);
        let message_id = self.alloc_message_id();

        let error_text = if let Some(ref error) = self.provider_error {
            format!(
                "Provider configuration error: {}. Please check settings.",
                error
            )
        } else {
            "Provider is not configured. Please set API key in settings.".to_string()
        };

        if let Some(conversation) = self.conversations.get_mut(&conversation_id) {
            conversation.messages.push(Message::new(
                message_id,
                Role::Assistant,
                error_text,
                MessageStatus::Error("Provider not configured".to_string()),
            ));
        }

        self.sync_active_conversation_messages(cx, false);
        cx.notify();
    }

    fn stream_event_is_current(&self, target: StreamTarget) -> bool {
        self.active_stream
            .is_some_and(|active_stream| active_stream.target == target)
            && self
                .conversations
                .get(&target.conversation_id)
                .is_some_and(|conversation| conversation.stream_state.accepts_stream_event(target))
    }

    fn alloc_message_id(&mut self) -> MessageId {
        let id = MessageId::new(self.next_message_id);
        self.next_message_id = self.next_message_id.saturating_add(1);
        id
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .id("chat-view")
            .relative()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .bg(theme.background)
            .child(
                h_flex()
                    .id("chat-view-header")
                    .h(px(48.))
                    .px_4()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child("Chat"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(self.model_selector.clone())
                            .child(
                                Button::new("chat-view-settings")
                                    .ghost()
                                    .small()
                                    .child("Settings")
                                    .on_click(cx.listener(Self::open_settings_click)),
                            ),
                    ),
            )
            .child(
                div()
                    .id("chat-view-message-list")
                    .flex_1()
                    .min_h_0()
                    .child(self.message_list.clone()),
            )
            .child(
                div()
                    .id("chat-view-message-input")
                    .flex_shrink_0()
                    .w_full()
                    .child(self.message_input.clone()),
            )
            .when(self.settings_open, |el| {
                el.child(
                    div()
                        .id("settings-overlay")
                        .absolute()
                        .inset_0()
                        .bg(theme.background.opacity(0.8))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(self.settings_view.clone()),
                )
            })
    }
}
