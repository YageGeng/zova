use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui::*;
use gpui_component::{ActiveTheme, Root, v_flex};
use gpui_tokio_bridge::Tokio;

use crate::chat::events::{ConversationSelected, Stop, Submit};
use crate::chat::message::{
    Conversation, ConversationId, Message, MessageId, MessageStatus, Role, StreamSessionId,
    StreamTarget,
};
use crate::chat::{
    ChatSidebar, MessageInput, MessageList, SidebarSettingsClicked, SidebarToggleClicked,
};
use crate::model_selector::{ModelSelected, ModelSelector, ModelSelectorSettingsClicked};
use crate::settings::{SettingsChanged, SettingsState, SettingsView};
use zova_llm::{
    DEFAULT_OPENAI_MODEL, LlmProvider, ProviderConfig, ProviderError, ProviderEventStream,
    ProviderMessage, ProviderStreamHandle, ProviderWorker, Role as ProviderRole,
    StreamEventMapped as ProviderStreamEventMapped,
    StreamEventPayload as ProviderStreamEventPayload, StreamRequest,
    StreamTarget as ProviderStreamTarget, create_provider,
};
use zova_storage::{MessageId as StorageMessageId, MessageRole as StorageMessageRole};

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
    settings_state: Entity<SettingsState>,
    settings_window: Option<WindowHandle<Root>>,
    provider: Option<Arc<dyn LlmProvider>>,
    current_model_id: String,
    conversations: HashMap<ConversationId, Conversation>,
    storage_message_ids: HashMap<ConversationId, HashMap<MessageId, StorageMessageId>>,
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

impl EventEmitter<SidebarToggleClicked> for ChatView {}

impl ChatView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| ChatSidebar::new(window, cx));
        let message_list = cx.new(MessageList::new);
        let message_input = cx.new(|cx| MessageInput::new(window, cx));
        let settings_state = SettingsState::new(cx);
        let initial_settings = settings_state.read(cx).settings();

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
            selector.set_models(initial_settings.configured_models(), cx);
            selector.set_model_id(current_model_id.clone(), cx);
        });

        let mut this = Self {
            sidebar: sidebar.clone(),
            message_list: message_list.clone(),
            message_input: message_input.clone(),
            model_selector: model_selector.clone(),
            settings_state: settings_state.clone(),
            settings_window: None,
            provider,
            current_model_id,
            conversations,
            storage_message_ids: HashMap::new(),
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

        cx.subscribe(&sidebar, |this, _, _event: &SidebarSettingsClicked, cx| {
            this.open_settings(cx);
        })
        .detach();

        cx.subscribe(&sidebar, |_, _, _event: &SidebarToggleClicked, cx| {
            cx.emit(SidebarToggleClicked);
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

        this
    }

    pub fn sidebar(&self) -> &Entity<ChatSidebar> {
        &self.sidebar
    }

    pub fn model_selector(&self) -> &Entity<ModelSelector> {
        &self.model_selector
    }

    pub fn resolved_provider_id(&self, cx: &App) -> String {
        let configured_provider_id = self.settings_state.read(cx).settings().provider_id.clone();

        if configured_provider_id.trim().is_empty() {
            "openai".to_string()
        } else {
            configured_provider_id.trim().to_string()
        }
    }

    pub fn create_conversation(&mut self, cx: &mut Context<Self>) {
        let _ = self
            .sidebar
            .update(cx, |sidebar, cx| sidebar.create_conversation(cx));
    }

    pub fn open_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.open_settings(cx);
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(settings_window) = self.settings_window.as_ref()
            && settings_window
                .update(cx, |_, window, _| {
                    window.activate_window();
                })
                .is_ok()
        {
            return;
        }

        self.settings_window = None;

        let settings_state = self.settings_state.clone();
        let settings_bounds = Bounds::centered(None, size(px(860.), px(760.)), cx);
        let settings_window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(settings_bounds)),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.), px(14.))),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                let settings_view = cx.new(|cx| SettingsView::new(&settings_state, window, cx));
                cx.new(|cx| Root::new(settings_view, window, cx))
            },
        );

        match settings_window {
            Ok(settings_window) => {
                self.settings_window = Some(settings_window);
            }
            Err(error) => {
                tracing::error!("failed to open settings window: {}", error);
            }
        }
    }

    fn handle_settings_changed(&mut self, event: &SettingsChanged, cx: &mut Context<Self>) {
        if self.active_stream.is_some() {
            self.cancel_active_stream(cx);
        }

        event.settings.apply_theme(None, cx);
        cx.refresh_windows();

        let model_id = event.settings.default_model_name();
        let available_models = event.settings.configured_models();

        match Self::create_provider_from_settings(&event.settings) {
            Ok((provider, _)) => {
                self.provider = provider.clone();
                self.current_model_id = model_id.clone();
                self.provider_error = None;

                self.model_selector.update(cx, |selector, cx| {
                    selector.set_models(available_models.clone(), cx);
                    selector.set_model_id(model_id.clone(), cx);
                });

                tracing::info!("reloaded provider adapter with new settings");
            }
            Err(error) => {
                self.provider = None;
                self.provider_error = Some(format!("{}", error));
                self.current_model_id = model_id.clone();
                self.model_selector.update(cx, |selector, cx| {
                    selector.set_models(available_models.clone(), cx);
                    selector.set_model_id(model_id.clone(), cx);
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
        let settings = settings_state.read(cx).settings();
        let default_model_from_settings = settings.default_model_name();

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

        let (provider, environment_model_id, provider_error) = Self::provider_from_environment();
        if provider.is_some() {
            (provider, environment_model_id, provider_error)
        } else {
            (provider, default_model_from_settings, provider_error)
        }
    }

    fn create_provider_from_settings(
        settings: &crate::settings::state::ProviderSettings,
    ) -> Result<(Option<Arc<dyn LlmProvider>>, String), ProviderError> {
        let config = settings.to_provider_config();
        let model_id = settings.default_model_name();

        let Some(config) = config else {
            return Ok((None, model_id));
        };

        match create_provider(config) {
            Ok(provider) => Ok((Some(provider), model_id)),
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

        let model_id = std::env::var("OPENAI_MODEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string());

        let endpoint = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| crate::settings::state::DEFAULT_ENDPOINT.to_string());

        let config = ProviderConfig::new("openai", api_key, endpoint);

        match create_provider(config) {
            Ok(provider) => (Some(provider), model_id, None),
            Err(error) => {
                tracing::error!("failed to initialize provider adapter: {error}");
                (None, model_id, Some(format!("Provider error: {}", error)))
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
        self.hydrate_conversation_messages(conversation_id, cx);
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

        // Persist user/assistant inserts after transition acceptance to keep stream lifecycle ordering unchanged.
        self.persist_inserted_message(
            active_conversation_id,
            user_message_id,
            Role::User,
            event.content.clone(),
            cx,
        );
        self.persist_inserted_message(
            active_conversation_id,
            assistant_message_id,
            Role::Assistant,
            String::new(),
            cx,
        );

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

        let configured_max_tokens = self
            .settings_state
            .read(cx)
            .settings()
            .model_max_tokens(&self.current_model_id);

        let mut request = StreamRequest::new(
            Self::chat_target_to_provider(event.target),
            self.current_model_id.clone(),
            request_messages,
        );
        if let Some(max_tokens) = configured_max_tokens {
            request = request.with_max_tokens(max_tokens);
        }

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

    fn handle_stream_event(&mut self, event: ProviderStreamEventMapped, cx: &mut Context<Self>) {
        // Provider events carry zova-llm typed IDs; normalize them to chat-domain IDs
        // before stale-session checks so stream isolation logic stays consistent.
        let event_target = Self::provider_target_to_chat(event.target);

        if !self.stream_event_is_current(event_target) {
            // Strict target equality prevents chunk leakage across conversation/session boundaries.
            return;
        }

        match event.payload {
            ProviderStreamEventPayload::Delta(chunk)
            | ProviderStreamEventPayload::ReasoningDelta(chunk) => {
                self.pending_stream_chunk.push_str(&chunk);
                self.schedule_debounced_stream_flush(cx);
            }
            ProviderStreamEventPayload::Done => {
                self.flush_pending_stream_chunk(cx);
                self.finish_stream_with_done(event_target, cx);
            }
            ProviderStreamEventPayload::Error(message) => {
                self.flush_pending_stream_chunk(cx);
                self.finish_stream_with_error(event_target, message, cx);
            }
        }
    }

    fn handle_stream_reader_closed(
        &mut self,
        target: ProviderStreamTarget,
        cx: &mut Context<Self>,
    ) {
        let target = Self::provider_target_to_chat(target);
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

        let mut persisted_assistant_content = None;

        if let Some(message) = conversation
            .messages
            .iter_mut()
            .find(|message| message.id == active_stream.assistant_message_id)
        {
            message.content.push_str(&chunk);
            persisted_assistant_content = Some(message.content.clone());
        }

        if let Some(content) = persisted_assistant_content {
            self.persist_updated_message(
                active_stream.target.conversation_id,
                active_stream.assistant_message_id,
                content,
                cx,
            );
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

        let mut persisted_assistant_content = None;

        if let Some(conversation) = self.conversations.get_mut(&target.conversation_id) {
            let _ = conversation.apply_stream_transition(transition);

            if let Some(message) = conversation
                .messages
                .iter_mut()
                .find(|message| message.id == active_stream.assistant_message_id)
            {
                message.status = final_status;
                persisted_assistant_content = Some(message.content.clone());
            }
        }

        if let Some(content) = persisted_assistant_content {
            self.persist_updated_message(
                target.conversation_id,
                active_stream.assistant_message_id,
                content,
                cx,
            );
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
        let title = self
            .sidebar
            .read(cx)
            .load_conversation(conversation_id)
            .map(|record| record.title)
            .unwrap_or_else(|| format!("Conversation {}", conversation_id.0));

        if let Some(conversation) = self.conversations.get_mut(&conversation_id) {
            conversation.title = title;
            return;
        }

        self.conversations
            .insert(conversation_id, Conversation::new(conversation_id, title));
    }

    fn hydrate_conversation_messages(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) {
        let persisted_messages = self
            .sidebar
            .read(cx)
            .list_persisted_messages(conversation_id);
        let mut hydrated_messages = Vec::with_capacity(persisted_messages.len());
        let mut storage_message_ids = HashMap::with_capacity(persisted_messages.len());

        // Keep a deterministic in-memory<->storage ID bridge so stream updates can scope writes.
        for persisted_message in persisted_messages {
            let message_id = self.alloc_message_id();
            storage_message_ids.insert(message_id, persisted_message.id);
            hydrated_messages.push(Message::new(
                message_id,
                storage_role_to_chat(persisted_message.role),
                persisted_message.content,
                MessageStatus::Done,
            ));
        }

        if let Some(conversation) = self.conversations.get_mut(&conversation_id) {
            conversation.messages = hydrated_messages;
        }
        self.storage_message_ids
            .insert(conversation_id, storage_message_ids);
    }

    fn persist_inserted_message(
        &mut self,
        conversation_id: ConversationId,
        message_id: MessageId,
        role: Role,
        content: String,
        cx: &mut Context<Self>,
    ) {
        let persisted_message =
            self.sidebar
                .read(cx)
                .append_persisted_message(conversation_id, role, content);
        if let Some(persisted_message) = persisted_message {
            self.storage_message_ids
                .entry(conversation_id)
                .or_default()
                .insert(message_id, persisted_message.id);
        }
    }

    fn persist_updated_message(
        &mut self,
        conversation_id: ConversationId,
        message_id: MessageId,
        content: String,
        cx: &mut Context<Self>,
    ) {
        let Some(storage_message_id) = self
            .storage_message_ids
            .get(&conversation_id)
            .and_then(|message_ids| message_ids.get(&message_id))
            .copied()
        else {
            tracing::warn!(
                "missing persisted message mapping for conversation={conversation_id:?}, message={message_id:?}"
            );
            return;
        };

        let _ = self.sidebar.read(cx).update_persisted_message_content(
            conversation_id,
            storage_message_id,
            content,
        );
    }

    fn build_provider_messages(conversation: &Conversation) -> Vec<ProviderMessage> {
        conversation
            .messages
            .iter()
            .filter(|message| !message.content.trim().is_empty())
            .filter(|message| !matches!(message.status, MessageStatus::Streaming(_)))
            .map(|message| {
                // Keep role mapping explicit at the crate boundary so llm types stay
                // decoupled from chat domain enums.
                ProviderMessage::new(
                    Self::chat_role_to_provider(message.role),
                    message.content.clone(),
                )
            })
            .collect()
    }

    fn chat_role_to_provider(role: Role) -> ProviderRole {
        match role {
            Role::System => ProviderRole::System,
            Role::User => ProviderRole::User,
            Role::Assistant => ProviderRole::Assistant,
        }
    }

    fn chat_target_to_provider(target: StreamTarget) -> ProviderStreamTarget {
        // Preserve numeric identity while translating between domain-specific typed wrappers.
        ProviderStreamTarget::new(
            zova_llm::ConversationId::new(target.conversation_id.0),
            zova_llm::StreamSessionId::new(target.session_id.0),
        )
    }

    fn provider_target_to_chat(target: ProviderStreamTarget) -> StreamTarget {
        // Convert provider routing keys back into chat routing keys for state transitions.
        StreamTarget::new(
            ConversationId::new(target.conversation_id.0),
            StreamSessionId::new(target.session_id.0),
        )
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
        let persisted_error_text = error_text.clone();

        if let Some(conversation) = self.conversations.get_mut(&conversation_id) {
            conversation.messages.push(Message::new(
                message_id,
                Role::Assistant,
                error_text,
                MessageStatus::Error("Provider not configured".to_string()),
            ));
        }

        self.persist_inserted_message(
            conversation_id,
            message_id,
            Role::Assistant,
            persisted_error_text,
            cx,
        );

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

fn storage_role_to_chat(role: StorageMessageRole) -> Role {
    match role {
        StorageMessageRole::System => Role::System,
        StorageMessageRole::User => Role::User,
        StorageMessageRole::Assistant => Role::Assistant,
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
                    .border_t_1()
                    .border_color(theme.border)
                    .child(self.message_input.clone()),
            )
    }
}
