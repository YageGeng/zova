use std::cmp::Ordering;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use snafu::{OptionExt, ResultExt, Snafu};

use crate::chat::ConversationId;

const DEFAULT_STORE_RELATIVE_PATH: &str = ".zova/conversations.tsv";
pub const DEFAULT_CONVERSATION_TITLE: &str = "New Conversation";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationRecord {
    pub id: ConversationId,
    pub title: String,
    pub updated_at_unix_seconds: u64,
}

impl ConversationRecord {
    pub fn new(id: ConversationId, title: impl Into<String>, updated_at_unix_seconds: u64) -> Self {
        Self {
            id,
            title: title.into(),
            updated_at_unix_seconds,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConversationStore {
    path: PathBuf,
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new(PathBuf::from(DEFAULT_STORE_RELATIVE_PATH))
    }
}

impl ConversationStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn create_conversation(
        &self,
        title: impl Into<String>,
    ) -> ConversationStoreResult<ConversationRecord> {
        let mut conversations = self.list_conversations()?;
        let next_id = conversations
            .iter()
            .map(|conversation| conversation.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        let mut title = title.into();
        if title.trim().is_empty() {
            title = DEFAULT_CONVERSATION_TITLE.to_string();
        }

        let created = ConversationRecord::new(
            ConversationId::new(next_id),
            title,
            current_unix_timestamp_seconds(),
        );
        conversations.push(created.clone());
        conversations.sort_by(sort_by_recent_desc);
        self.write_conversations(&conversations)?;
        Ok(created)
    }

    pub fn list_conversations(&self) -> ConversationStoreResult<Vec<ConversationRecord>> {
        let store = self.read_store_text()?;
        let mut conversations = Vec::new();

        for (index, line) in store.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let parsed = parse_line(line, index + 1)?;
            conversations.push(parsed);
        }

        conversations.sort_by(sort_by_recent_desc);
        Ok(conversations)
    }

    pub fn load_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> ConversationStoreResult<Option<ConversationRecord>> {
        Ok(self
            .list_conversations()?
            .into_iter()
            .find(|conversation| conversation.id == conversation_id))
    }

    fn read_store_text(&self) -> ConversationStoreResult<String> {
        if !self.path.exists() {
            return Ok(String::new());
        }

        std::fs::read_to_string(&self.path).context(ReadStoreSnafu {
            stage: "read-store",
            path: display_path(&self.path),
        })
    }

    fn write_conversations(
        &self,
        conversations: &[ConversationRecord],
    ) -> ConversationStoreResult<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).context(CreateStoreDirectorySnafu {
                stage: "create-store-directory",
                path: display_path(parent),
            })?;
        }

        let mut serialized = String::new();

        // Escape title text to keep a TSV payload deterministic and line-safe.
        for conversation in conversations {
            serialized.push_str(&conversation.id.0.to_string());
            serialized.push('\t');
            serialized.push_str(&conversation.updated_at_unix_seconds.to_string());
            serialized.push('\t');
            serialized.push_str(&encode_title(&conversation.title));
            serialized.push('\n');
        }

        std::fs::write(&self.path, serialized).context(WriteStoreSnafu {
            stage: "write-store",
            path: display_path(&self.path),
        })
    }
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum ConversationStoreError {
    #[snafu(display("failed to create conversation store directory at {path}"))]
    CreateStoreDirectory {
        stage: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[snafu(display("failed to read conversation store from {path}"))]
    ReadStore {
        stage: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[snafu(display("failed to write conversation store to {path}"))]
    WriteStore {
        stage: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[snafu(display("failed to parse conversation store line {line_number}: {line}"))]
    ParseStoreLine {
        stage: &'static str,
        line_number: usize,
        line: String,
    },
    #[snafu(display("failed to parse conversation id '{raw}'"))]
    ParseConversationId {
        stage: &'static str,
        raw: String,
        source: ParseIntError,
    },
    #[snafu(display("failed to parse conversation timestamp '{raw}'"))]
    ParseUpdatedAt {
        stage: &'static str,
        raw: String,
        source: ParseIntError,
    },
}

pub type ConversationStoreResult<T> = Result<T, ConversationStoreError>;

fn parse_line(line: &str, line_number: usize) -> ConversationStoreResult<ConversationRecord> {
    let mut fields = line.splitn(3, '\t');
    let raw_id = fields.next().context(ParseStoreLineSnafu {
        stage: "parse-store-line-id",
        line_number,
        line: line.to_string(),
    })?;
    let raw_updated_at = fields.next().context(ParseStoreLineSnafu {
        stage: "parse-store-line-updated-at",
        line_number,
        line: line.to_string(),
    })?;
    let raw_title = fields.next().context(ParseStoreLineSnafu {
        stage: "parse-store-line-title",
        line_number,
        line: line.to_string(),
    })?;

    let id = raw_id.parse::<u64>().context(ParseConversationIdSnafu {
        stage: "parse-conversation-id",
        raw: raw_id.to_string(),
    })?;
    let updated_at_unix_seconds = raw_updated_at.parse::<u64>().context(ParseUpdatedAtSnafu {
        stage: "parse-conversation-updated-at",
        raw: raw_updated_at.to_string(),
    })?;

    Ok(ConversationRecord::new(
        ConversationId::new(id),
        decode_title(raw_title),
        updated_at_unix_seconds,
    ))
}

fn sort_by_recent_desc(left: &ConversationRecord, right: &ConversationRecord) -> Ordering {
    right
        .updated_at_unix_seconds
        .cmp(&left.updated_at_unix_seconds)
        .then_with(|| right.id.0.cmp(&left.id.0))
}

fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn encode_title(title: &str) -> String {
    let mut encoded = String::with_capacity(title.len());

    for character in title.chars() {
        match character {
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\t' => encoded.push_str("\\t"),
            '\r' => encoded.push_str("\\r"),
            _ => encoded.push(character),
        }
    }

    encoded
}

fn decode_title(encoded_title: &str) -> String {
    let mut decoded = String::with_capacity(encoded_title.len());
    let mut characters = encoded_title.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }

        match characters.next() {
            Some('n') => decoded.push('\n'),
            Some('t') => decoded.push('\t'),
            Some('r') => decoded.push('\r'),
            Some('\\') => decoded.push('\\'),
            Some(other) => {
                decoded.push('\\');
                decoded.push(other);
            }
            None => decoded.push('\\'),
        }
    }

    decoded
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}
