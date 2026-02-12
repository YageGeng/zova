/// Event contracts for chat module wiring.
pub mod events;
/// Domain entities and deterministic stream state boundaries.
pub mod message;
pub mod message_input;
pub mod message_list;
pub mod scroll_manager;
pub mod sidebar;
pub mod view;

pub use events::{
    ConversationSelected, ModelChanged, Stop, StreamEventMapped, StreamEventPayload, Submit,
};
pub use message::{
    Conversation, ConversationId, Message, MessageId, MessageStatus, Role, StreamSessionId,
    StreamState, StreamTarget, StreamTransition, StreamTransitionRejection, StreamTransitionResult,
};
pub use message_input::MessageInput;
pub use message_list::MessageList;
pub use scroll_manager::ScrollManager;
pub use sidebar::{ChatSidebar, SidebarSettingsClicked, SidebarToggleClicked};
pub use view::ChatView;
