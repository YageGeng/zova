use crate::chat::message::{ConversationId, StreamTarget, StreamTransition};

/// Emitted when sidebar selection changes the active conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConversationSelected {
    pub conversation_id: ConversationId,
}

/// Emitted when the user submits a prompt to generate a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submit {
    pub target: StreamTarget,
    pub content: String,
}

/// Emitted when user requests cancellation of an active stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stop {
    pub target: StreamTarget,
}

/// Emitted when active model selection changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelChanged {
    pub model_id: String,
}

/// Provider-agnostic stream payload mapped into chat domain language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEventPayload {
    Delta(String),
    ReasoningDelta(String),
    Done,
    Error(String),
}

/// Emitted after provider stream events are mapped into domain events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEventMapped {
    pub target: StreamTarget,
    pub payload: StreamEventPayload,
}

impl Submit {
    /// Creates a submit event with explicit stream target.
    pub fn new(target: StreamTarget, content: impl Into<String>) -> Self {
        Self {
            target,
            content: content.into(),
        }
    }

    /// Returns stream state transition to start the session.
    pub fn start_transition(&self) -> StreamTransition {
        StreamTransition::Start(self.target)
    }
}

impl Stop {
    /// Returns stream state transition for user-triggered cancellation.
    pub fn into_transition(self) -> StreamTransition {
        StreamTransition::Cancel(self.target)
    }
}

impl StreamEventMapped {
    /// Maps terminal payloads to stream state transitions.
    ///
    /// Delta payloads intentionally return `None` because they mutate content buffers,
    /// not the stream lifecycle state.
    pub fn into_transition(self) -> Option<StreamTransition> {
        match self.payload {
            StreamEventPayload::Delta(_) | StreamEventPayload::ReasoningDelta(_) => None,
            StreamEventPayload::Done => Some(StreamTransition::Complete(self.target)),
            StreamEventPayload::Error(message) => Some(StreamTransition::Fail {
                target: self.target,
                message,
            }),
        }
    }
}
