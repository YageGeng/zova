use super::ids::{AgentEventId, BranchId, MediaRefId, MessageId, SessionId};

/// Default session title used when legacy rows have empty titles.
pub const DEFAULT_SESSION_TITLE: &str = "New Conversation";

/// Storage-local message role, intentionally decoupled from UI-layer role enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: SessionId,
    pub title: String,
    pub active_branch_id: BranchId,
    pub updated_at_unix_seconds: u64,
    pub deleted_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSession {
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionPatch {
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRecord {
    pub id: MessageId,
    pub session_id: SessionId,
    pub branch_id: BranchId,
    pub seq: u64,
    pub role: MessageRole,
    pub content: String,
    pub deleted_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessagePatch {
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryForkRequest {
    pub source_message_id: MessageId,
    pub replacement_content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageIdRemap {
    pub old_message_id: MessageId,
    pub new_message_id: MessageId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryForkOutcome {
    pub new_branch_id: BranchId,
    // Copy-on-write history edits can mint new message IDs, so callers need deterministic remaps.
    pub message_id_remaps: Vec<MessageIdRemap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRefRecord {
    pub id: MediaRefId,
    pub session_id: SessionId,
    pub message_id: MessageId,
    pub uri: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub duration_ms: Option<u64>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    pub sha256_hex: Option<String>,
    pub deleted_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMediaRef {
    pub uri: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub duration_ms: Option<u64>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    pub sha256_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentEventRecord {
    pub id: AgentEventId,
    pub session_id: SessionId,
    pub message_id: Option<MessageId>,
    pub event_type: String,
    pub payload_json: String,
    pub created_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgentEvent {
    pub message_id: Option<MessageId>,
    pub event_type: String,
    pub payload_json: String,
}
