pub mod error;
pub mod ids;
pub mod sqlite;
pub mod types;

pub use error::{StorageError, StorageResult};
pub use ids::{AgentEventId, BranchId, MediaRefId, MessageId, SessionId};
pub use sqlite::SqliteStorage;
pub use types::{
    AgentEventRecord, DEFAULT_SESSION_TITLE, HistoryForkOutcome, HistoryForkRequest,
    MediaRefRecord, MessageIdRemap, MessagePatch, MessageRecord, MessageRole, NewAgentEvent,
    NewMediaRef, NewMessage, NewSession, SessionPatch, SessionRecord,
};

pub trait SessionStore: Send + Sync {
    fn create_session(&self, input: NewSession) -> StorageResult<SessionRecord>;
    fn list_sessions(&self, include_deleted: bool) -> StorageResult<Vec<SessionRecord>>;
    fn get_session(&self, session_id: SessionId) -> StorageResult<Option<SessionRecord>>;
    fn update_session(
        &self,
        session_id: SessionId,
        patch: SessionPatch,
    ) -> StorageResult<SessionRecord>;
    fn soft_delete_session(&self, session_id: SessionId) -> StorageResult<()>;
    fn restore_session(&self, session_id: SessionId) -> StorageResult<()>;
}

pub trait MessageStore: Send + Sync {
    fn append_message(
        &self,
        session_id: SessionId,
        input: NewMessage,
    ) -> StorageResult<MessageRecord>;
    fn list_messages(&self, session_id: SessionId) -> StorageResult<Vec<MessageRecord>>;
    fn get_message(
        &self,
        session_id: SessionId,
        message_id: MessageId,
    ) -> StorageResult<Option<MessageRecord>>;
    fn update_message(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        patch: MessagePatch,
    ) -> StorageResult<MessageRecord>;
    fn fork_from_history(
        &self,
        session_id: SessionId,
        request: HistoryForkRequest,
    ) -> StorageResult<HistoryForkOutcome>;
}

pub trait MediaStore: Send + Sync {
    fn attach_media(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        input: NewMediaRef,
    ) -> StorageResult<MediaRefRecord>;
    fn list_media(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        include_deleted: bool,
    ) -> StorageResult<Vec<MediaRefRecord>>;
    fn soft_delete_media(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        media_ref_id: MediaRefId,
    ) -> StorageResult<()>;
}

pub trait AgentEventStore: Send + Sync {
    fn append_agent_event(
        &self,
        session_id: SessionId,
        input: NewAgentEvent,
    ) -> StorageResult<AgentEventRecord>;
    fn list_agent_events(
        &self,
        session_id: SessionId,
        message_id: Option<MessageId>,
    ) -> StorageResult<Vec<AgentEventRecord>>;
}

pub trait Storage: SessionStore + MessageStore + MediaStore + AgentEventStore {}

impl<T> Storage for T where T: SessionStore + MessageStore + MediaStore + AgentEventStore {}
