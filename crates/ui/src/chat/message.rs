/// Stable identifier for one conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConversationId(pub u64);

impl ConversationId {
    /// Creates a typed conversation identifier.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

/// Stable identifier for one message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MessageId(pub u64);

impl MessageId {
    /// Creates a typed message identifier.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

/// Identifier for one streaming generation session.
///
/// This must change on every submit/retry so stale chunks can be rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamSessionId(pub u64);

impl StreamSessionId {
    /// Creates a typed stream session identifier.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

/// Stream routing key used for stale-chunk rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamTarget {
    pub conversation_id: ConversationId,
    pub session_id: StreamSessionId,
}

impl StreamTarget {
    /// Builds a full stream target from conversation and session IDs.
    pub const fn new(conversation_id: ConversationId, session_id: StreamSessionId) -> Self {
        Self {
            conversation_id,
            session_id,
        }
    }
}

/// Chat speaker role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Lifecycle status for one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageStatus {
    Pending,
    Streaming(StreamSessionId),
    Done,
    Error(String),
    Cancelled,
}

/// Core immutable message model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: MessageId,
    pub role: Role,
    pub content: String,
    pub status: MessageStatus,
}

impl Message {
    /// Creates a message with explicit status.
    pub fn new(
        id: MessageId,
        role: Role,
        content: impl Into<String>,
        status: MessageStatus,
    ) -> Self {
        Self {
            id,
            role,
            content: content.into(),
            status,
        }
    }

    /// Creates a pending user message before stream starts.
    pub fn user_pending(id: MessageId, content: impl Into<String>) -> Self {
        Self::new(id, Role::User, content, MessageStatus::Pending)
    }

    /// Creates an assistant placeholder while streaming.
    pub fn assistant_streaming(id: MessageId, session_id: StreamSessionId) -> Self {
        Self::new(
            id,
            Role::Assistant,
            String::new(),
            MessageStatus::Streaming(session_id),
        )
    }
}

/// Conversation aggregate root for chat state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub id: ConversationId,
    pub title: String,
    pub messages: Vec<Message>,
    pub stream_state: StreamState,
}

impl Conversation {
    /// Creates an empty conversation in idle state.
    pub fn new(id: ConversationId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            messages: Vec::new(),
            stream_state: StreamState::Idle,
        }
    }

    /// Applies a deterministic stream transition.
    pub fn apply_stream_transition(
        &mut self,
        transition: StreamTransition,
    ) -> StreamTransitionResult {
        let next_state = self.stream_state.apply(transition)?;
        self.stream_state = next_state.clone();
        Ok(next_state)
    }
}

/// Stream state boundary for conversation orchestration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StreamState {
    #[default]
    Idle,
    Streaming(StreamTarget),
    Done(StreamTarget),
    Error {
        target: StreamTarget,
        message: String,
    },
    Cancelled(StreamTarget),
}

/// State transition input for stream lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamTransition {
    Start(StreamTarget),
    Complete(StreamTarget),
    Fail {
        target: StreamTarget,
        message: String,
    },
    Cancel(StreamTarget),
    ResetToIdle,
}

/// Rejection reason for illegal stream transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamTransitionRejection {
    AlreadyStreaming {
        active: StreamTarget,
        attempted: StreamTarget,
    },
    NoActiveStream,
    SessionMismatch {
        active: StreamTarget,
        attempted: StreamTarget,
    },
}

/// Result type for stream transition application.
pub type StreamTransitionResult = Result<StreamState, StreamTransitionRejection>;

impl StreamState {
    /// Returns active streaming target if and only if state is `Streaming`.
    pub fn active_target(&self) -> Option<StreamTarget> {
        match self {
            Self::Streaming(target) => Some(*target),
            Self::Idle | Self::Done(_) | Self::Error { .. } | Self::Cancelled(_) => None,
        }
    }

    /// Returns true when incoming stream data matches the active session.
    pub fn accepts_stream_event(&self, target: StreamTarget) -> bool {
        matches!(self, Self::Streaming(active) if *active == target)
    }

    /// Applies one transition deterministically.
    ///
    /// Non-streaming states may start a new session directly. Any terminal transition
    /// (`Complete`/`Fail`/`Cancel`) must match the currently active session exactly.
    pub fn apply(&self, transition: StreamTransition) -> StreamTransitionResult {
        match transition {
            StreamTransition::Start(target) => self.apply_start(target),
            StreamTransition::Complete(target) => self.apply_complete(target),
            StreamTransition::Fail { target, message } => self.apply_fail(target, message),
            StreamTransition::Cancel(target) => self.apply_cancel(target),
            StreamTransition::ResetToIdle => Ok(Self::Idle),
        }
    }

    fn apply_start(&self, target: StreamTarget) -> StreamTransitionResult {
        match self {
            Self::Streaming(active) if *active != target => {
                Err(StreamTransitionRejection::AlreadyStreaming {
                    active: *active,
                    attempted: target,
                })
            }
            Self::Streaming(_) => Ok(self.clone()),
            Self::Idle | Self::Done(_) | Self::Error { .. } | Self::Cancelled(_) => {
                Ok(Self::Streaming(target))
            }
        }
    }

    fn apply_complete(&self, target: StreamTarget) -> StreamTransitionResult {
        match self {
            Self::Streaming(active) if *active == target => Ok(Self::Done(target)),
            Self::Streaming(active) => Err(StreamTransitionRejection::SessionMismatch {
                active: *active,
                attempted: target,
            }),
            Self::Idle | Self::Done(_) | Self::Error { .. } | Self::Cancelled(_) => {
                Err(StreamTransitionRejection::NoActiveStream)
            }
        }
    }

    fn apply_fail(&self, target: StreamTarget, message: String) -> StreamTransitionResult {
        match self {
            Self::Streaming(active) if *active == target => Ok(Self::Error { target, message }),
            Self::Streaming(active) => Err(StreamTransitionRejection::SessionMismatch {
                active: *active,
                attempted: target,
            }),
            Self::Idle | Self::Done(_) | Self::Error { .. } | Self::Cancelled(_) => {
                Err(StreamTransitionRejection::NoActiveStream)
            }
        }
    }

    fn apply_cancel(&self, target: StreamTarget) -> StreamTransitionResult {
        match self {
            Self::Streaming(active) if *active == target => Ok(Self::Cancelled(target)),
            Self::Streaming(active) => Err(StreamTransitionRejection::SessionMismatch {
                active: *active,
                attempted: target,
            }),
            Self::Idle | Self::Done(_) | Self::Error { .. } | Self::Cancelled(_) => {
                Err(StreamTransitionRejection::NoActiveStream)
            }
        }
    }
}
