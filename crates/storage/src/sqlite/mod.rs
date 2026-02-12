use std::future::Future;
use std::num::ParseIntError;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use snafu::{OptionExt, ResultExt};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Connection, FromRow, SqliteConnection, SqlitePool};

use super::error::{
    ConflictSnafu, InvariantViolationSnafu, NotFoundSnafu, SqliteQuerySnafu,
    SqliteRuntimeInitSnafu, SqliteThreadSpawnSnafu,
};
use super::error::{
    CreateSqliteDirectorySnafu, SqliteConnectOptionsSnafu, SqliteConnectSnafu, SqliteMigrateSnafu,
    SqlitePragmaSnafu, StorageResult,
};
use super::ids::{AgentEventId, BranchId, MediaRefId, MessageId, SessionId};
use super::types::{
    AgentEventRecord, DEFAULT_SESSION_TITLE, HistoryForkOutcome, HistoryForkRequest,
    MediaRefRecord, MessageIdRemap, MessagePatch, MessageRecord, MessageRole, NewAgentEvent,
    NewMediaRef, NewMessage, NewSession, SessionPatch, SessionRecord,
};
use super::{AgentEventStore, MediaStore, MessageStore, SessionStore};

pub const LEGACY_CONVERSATIONS_TSV_RELATIVE_PATH: &str = ".zova/conversations.tsv";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportWarning {
    pub line_number: usize,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportReport {
    pub source_path: String,
    pub source_missing: bool,
    pub imported_sessions: usize,
    pub skipped_rows: usize,
    pub warnings: Vec<LegacyImportWarning>,
    pub already_migrated: bool,
}

#[derive(Debug, Clone)]
struct LegacyConversationRow {
    legacy_id: u64,
    updated_at_unix_seconds: u64,
    title: String,
}

#[derive(Debug, Clone)]
pub struct SqliteStorage {
    pool: SqlitePool,
    database_url: String,
}

impl SqliteStorage {
    pub async fn open(database_location: &str) -> StorageResult<Self> {
        ensure_database_directory(database_location)?;

        let database_url = normalize_database_url(database_location);
        let connect_options = SqliteConnectOptions::from_str(&database_url)
            .context(SqliteConnectOptionsSnafu {
                stage: "sqlite-open-parse-url",
                database_url: database_url.clone(),
            })?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_millis(5_000));

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await
            .context(SqliteConnectSnafu {
                stage: "sqlite-open-connect",
                database_url: database_url.clone(),
            })?;

        // Explicit PRAGMA writes make bootstrap behavior deterministic for QA checks.
        let _: String = sqlx::query_scalar("PRAGMA journal_mode = WAL;")
            .fetch_one(&pool)
            .await
            .context(SqlitePragmaSnafu {
                stage: "sqlite-open-pragma-journal-mode",
                pragma: "journal_mode",
            })?;
        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await
            .context(SqlitePragmaSnafu {
                stage: "sqlite-open-pragma-foreign-keys",
                pragma: "foreign_keys",
            })?;
        sqlx::query("PRAGMA busy_timeout = 5000;")
            .execute(&pool)
            .await
            .context(SqlitePragmaSnafu {
                stage: "sqlite-open-pragma-busy-timeout",
                pragma: "busy_timeout",
            })?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .context(SqliteMigrateSnafu {
                stage: "sqlite-open-migrate",
            })?;

        Ok(Self { pool, database_url })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn import_legacy_conversations_from_default_path(
        &self,
    ) -> StorageResult<LegacyImportReport> {
        self.import_legacy_conversations_from_path(Path::new(
            LEGACY_CONVERSATIONS_TSV_RELATIVE_PATH,
        ))
    }

    pub fn import_legacy_conversations_from_path(
        &self,
        legacy_tsv_path: &Path,
    ) -> StorageResult<LegacyImportReport> {
        let source_path = legacy_tsv_path.display().to_string();
        let source_text = match std::fs::read_to_string(legacy_tsv_path) {
            Ok(contents) => Some(contents),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(super::error::StorageError::ReadLegacyConversationTsv {
                    stage: "legacy-import-read-source",
                    path: source_path,
                    source,
                });
            }
        };

        let Some(source_text) = source_text else {
            return Ok(LegacyImportReport {
                source_path,
                source_missing: true,
                imported_sessions: 0,
                skipped_rows: 0,
                warnings: Vec::new(),
                already_migrated: false,
            });
        };

        let (legacy_rows, warnings) = parse_legacy_conversation_rows(&source_text);
        let imported_candidates = legacy_rows.len();
        let database_url = self.database_url.clone();
        let (imported_sessions, already_migrated) = self.run_db_call("legacy-session-import", async move {
            let mut connection = connect_store_connection(&database_url, "legacy-import-connect").await?;
            let existing_sessions = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions")
                .fetch_one(&mut connection)
                .await
                .context(SqliteQuerySnafu {
                    stage: "legacy-import-count-existing-sessions",
                })?;

            // Migration is idempotent: once sqlite has session rows, import is treated as applied.
            if existing_sessions > 0 {
                return Ok((0_usize, true));
            }

            if imported_candidates == 0 {
                return Ok((0_usize, false));
            }

            let mut tx = connection.begin().await.context(SqliteQuerySnafu {
                stage: "legacy-import-begin",
            })?;

            // Insert sessions and initial branches atomically so active_branch invariants remain consistent.
            for row in legacy_rows {
                let session_id = SessionId::new_v7();
                let branch_id = BranchId::new_v7();
                let updated_at = u64_to_i64(row.updated_at_unix_seconds, "legacy-import-updated-at")?;

                sqlx::query(
                    "INSERT INTO sessions (id, title, active_branch_id, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, NULL)",
                )
                .bind(session_id.to_string())
                .bind(row.title)
                .bind(branch_id.to_string())
                .bind(updated_at)
                .bind(updated_at)
                .execute(&mut *tx)
                .await
                .context(SqliteQuerySnafu {
                    stage: "legacy-import-insert-session",
                })?;

                sqlx::query(
                    "INSERT INTO branches (id, session_id, parent_branch_id, created_at, deleted_at) VALUES (?, ?, NULL, ?, NULL)",
                )
                .bind(branch_id.to_string())
                .bind(session_id.to_string())
                .bind(updated_at)
                .execute(&mut *tx)
                .await
                .context(SqliteQuerySnafu {
                    stage: "legacy-import-insert-branch",
                })?;
            }

            tx.commit().await.context(SqliteQuerySnafu {
                stage: "legacy-import-commit",
            })?;

            Ok((imported_candidates, false))
        })?;

        Ok(LegacyImportReport {
            source_path,
            source_missing: false,
            imported_sessions,
            skipped_rows: warnings.len(),
            warnings,
            already_migrated,
        })
    }

    fn run_db_call<T, F>(&self, stage: &'static str, op: F) -> StorageResult<T>
    where
        T: Send + 'static,
        F: Future<Output = StorageResult<T>> + Send + 'static,
    {
        // Store traits are sync, so each call executes on a dedicated worker thread
        // with its own current-thread runtime to avoid nested-runtime blocking panics.
        let worker = std::thread::Builder::new()
            .name(format!("sqlite-store-{stage}"))
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context(SqliteRuntimeInitSnafu {
                        stage: "sqlite-store-runtime-build",
                    })?;
                runtime.block_on(op)
            })
            .context(SqliteThreadSpawnSnafu {
                stage: "sqlite-store-spawn-worker",
            })?;

        match worker.join() {
            Ok(result) => result,
            Err(_) => InvariantViolationSnafu {
                stage,
                details: "sqlite storage worker thread panicked".to_string(),
            }
            .fail(),
        }
    }
}

impl SessionStore for SqliteStorage {
    fn create_session(&self, input: NewSession) -> StorageResult<SessionRecord> {
        let database_url = self.database_url.clone();
        self.run_db_call("session-create", async move {
            let mut connection = connect_store_connection(&database_url, "session-create-connect").await?;
            let title = input.title;
            let mut tx = connection
                .begin()
                .await
                .context(SqliteQuerySnafu {
                    stage: "session-create-begin",
                })?;

            let session_id = SessionId::new_v7();
            let branch_id = BranchId::new_v7();
            let now = unix_timestamp_seconds();

            sqlx::query(
                "INSERT INTO sessions (id, title, active_branch_id, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, NULL)",
            )
            .bind(session_id.to_string())
            .bind(title.clone())
            .bind(branch_id.to_string())
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .context(SqliteQuerySnafu {
                stage: "session-create-insert-session",
            })?;

            sqlx::query(
                "INSERT INTO branches (id, session_id, parent_branch_id, created_at, deleted_at) VALUES (?, ?, NULL, ?, NULL)",
            )
            .bind(branch_id.to_string())
            .bind(session_id.to_string())
            .bind(now)
            .execute(&mut *tx)
            .await
            .context(SqliteQuerySnafu {
                stage: "session-create-insert-branch",
            })?;

            tx.commit().await.context(SqliteQuerySnafu {
                stage: "session-create-commit",
            })?;

            Ok(SessionRecord {
                id: session_id,
                title,
                active_branch_id: branch_id,
                updated_at_unix_seconds: i64_to_u64(now, "session-create-updated-at")?,
                deleted_at_unix_seconds: None,
            })
        })
    }

    fn list_sessions(&self, include_deleted: bool) -> StorageResult<Vec<SessionRecord>> {
        let database_url = self.database_url.clone();
        self.run_db_call("session-list", async move {
            let mut connection = connect_store_connection(&database_url, "session-list-connect").await?;
            let rows = if include_deleted {
                sqlx::query_as::<_, SessionRow>(
                    "SELECT id, title, active_branch_id, updated_at, deleted_at FROM sessions ORDER BY updated_at DESC, id DESC",
                )
                .fetch_all(&mut connection)
                .await
            } else {
                sqlx::query_as::<_, SessionRow>(
                    "SELECT id, title, active_branch_id, updated_at, deleted_at FROM sessions WHERE deleted_at IS NULL ORDER BY updated_at DESC, id DESC",
                )
                .fetch_all(&mut connection)
                .await
            }
            .context(SqliteQuerySnafu {
                stage: "session-list-query",
            })?;

            rows.into_iter().map(session_row_to_record).collect()
        })
    }

    fn get_session(&self, session_id: SessionId) -> StorageResult<Option<SessionRecord>> {
        let database_url = self.database_url.clone();
        self.run_db_call("session-get", async move {
            let mut connection = connect_store_connection(&database_url, "session-get-connect").await?;
            let row = sqlx::query_as::<_, SessionRow>(
                "SELECT id, title, active_branch_id, updated_at, deleted_at FROM sessions WHERE id = ?",
            )
            .bind(session_id.to_string())
            .fetch_optional(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "session-get-query",
            })?;

            row.map(session_row_to_record).transpose()
        })
    }

    fn update_session(
        &self,
        session_id: SessionId,
        patch: SessionPatch,
    ) -> StorageResult<SessionRecord> {
        let database_url = self.database_url.clone();
        self.run_db_call("session-update", async move {
            let mut connection = connect_store_connection(&database_url, "session-update-connect").await?;
            let now = unix_timestamp_seconds();
            let update_result = sqlx::query(
                "UPDATE sessions SET title = COALESCE(?, title), updated_at = ? WHERE id = ?",
            )
            .bind(patch.title)
            .bind(now)
            .bind(session_id.to_string())
            .execute(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "session-update-apply",
            })?;

            if update_result.rows_affected() == 0 {
                return NotFoundSnafu {
                    stage: "session-update-missing",
                    entity: "session",
                    id: session_id.to_string(),
                }
                .fail();
            }

            let row = sqlx::query_as::<_, SessionRow>(
                "SELECT id, title, active_branch_id, updated_at, deleted_at FROM sessions WHERE id = ?",
            )
            .bind(session_id.to_string())
            .fetch_optional(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "session-update-load",
            })?
            .context(NotFoundSnafu {
                stage: "session-update-load-missing",
                entity: "session",
                id: session_id.to_string(),
            })?;

            session_row_to_record(row)
        })
    }

    fn soft_delete_session(&self, session_id: SessionId) -> StorageResult<()> {
        let database_url = self.database_url.clone();
        self.run_db_call("session-soft-delete", async move {
            let mut connection =
                connect_store_connection(&database_url, "session-soft-delete-connect").await?;
            let now = unix_timestamp_seconds();
            let result = sqlx::query(
                "UPDATE sessions SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(now)
            .bind(now)
            .bind(session_id.to_string())
            .execute(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "session-soft-delete-apply",
            })?;

            if result.rows_affected() == 0 {
                let exists = session_exists(&mut connection, session_id).await?;
                if !exists {
                    return NotFoundSnafu {
                        stage: "session-soft-delete-missing",
                        entity: "session",
                        id: session_id.to_string(),
                    }
                    .fail();
                }
            }

            Ok(())
        })
    }

    fn restore_session(&self, session_id: SessionId) -> StorageResult<()> {
        let database_url = self.database_url.clone();
        self.run_db_call("session-restore", async move {
            let mut connection = connect_store_connection(&database_url, "session-restore-connect").await?;
            let now = unix_timestamp_seconds();
            let result = sqlx::query(
                "UPDATE sessions SET deleted_at = NULL, updated_at = ? WHERE id = ? AND deleted_at IS NOT NULL",
            )
            .bind(now)
            .bind(session_id.to_string())
            .execute(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "session-restore-apply",
            })?;

            if result.rows_affected() == 0 {
                let exists = session_exists(&mut connection, session_id).await?;
                if !exists {
                    return NotFoundSnafu {
                        stage: "session-restore-missing",
                        entity: "session",
                        id: session_id.to_string(),
                    }
                    .fail();
                }
            }

            Ok(())
        })
    }
}

impl MessageStore for SqliteStorage {
    fn append_message(
        &self,
        session_id: SessionId,
        input: NewMessage,
    ) -> StorageResult<MessageRecord> {
        let database_url = self.database_url.clone();
        self.run_db_call("message-append", async move {
            let mut connection = connect_store_connection(&database_url, "message-append-connect").await?;
            let active_branch_id =
                load_active_branch_id(&mut connection, session_id, "message-append-load-active").await?;
            let next_seq = sqlx::query_scalar::<_, i64>(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM messages WHERE session_id = ? AND branch_id = ?",
            )
            .bind(session_id.to_string())
            .bind(active_branch_id.to_string())
            .fetch_one(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-append-next-seq",
            })?;

            let now = unix_timestamp_seconds();
            let message_id = MessageId::new_v7();
            let role_text = role_to_sql(input.role);

            sqlx::query(
                "INSERT INTO messages (id, session_id, branch_id, seq, role, content, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)",
            )
            .bind(message_id.to_string())
            .bind(session_id.to_string())
            .bind(active_branch_id.to_string())
            .bind(next_seq)
            .bind(role_text)
            .bind(input.content.clone())
            .bind(now)
            .bind(now)
            .execute(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-append-insert",
            })?;

            Ok(MessageRecord {
                id: message_id,
                session_id,
                branch_id: active_branch_id,
                seq: i64_to_u64(next_seq, "message-append-seq")?,
                role: input.role,
                content: input.content,
                deleted_at_unix_seconds: None,
            })
        })
    }

    fn list_messages(&self, session_id: SessionId) -> StorageResult<Vec<MessageRecord>> {
        let database_url = self.database_url.clone();
        self.run_db_call("message-list", async move {
            let mut connection = connect_store_connection(&database_url, "message-list-connect").await?;
            let active_branch_id =
                load_active_branch_id(&mut connection, session_id, "message-list-load-active").await?;
            let rows = sqlx::query_as::<_, MessageRow>(
                "SELECT id, session_id, branch_id, seq, role, content, deleted_at FROM messages WHERE session_id = ? AND branch_id = ? AND deleted_at IS NULL ORDER BY seq ASC, id ASC",
            )
            .bind(session_id.to_string())
            .bind(active_branch_id.to_string())
            .fetch_all(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-list-query",
            })?;

            rows.into_iter().map(message_row_to_record).collect()
        })
    }

    fn get_message(
        &self,
        session_id: SessionId,
        message_id: MessageId,
    ) -> StorageResult<Option<MessageRecord>> {
        let database_url = self.database_url.clone();
        self.run_db_call("message-get", async move {
            let mut connection = connect_store_connection(&database_url, "message-get-connect").await?;
            let row = sqlx::query_as::<_, MessageRow>(
                "SELECT id, session_id, branch_id, seq, role, content, deleted_at FROM messages WHERE session_id = ? AND id = ? AND deleted_at IS NULL",
            )
            .bind(session_id.to_string())
            .bind(message_id.to_string())
            .fetch_optional(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-get-query",
            })?;

            row.map(message_row_to_record).transpose()
        })
    }

    fn update_message(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        patch: MessagePatch,
    ) -> StorageResult<MessageRecord> {
        let database_url = self.database_url.clone();
        self.run_db_call("message-update", async move {
            let mut connection = connect_store_connection(&database_url, "message-update-connect").await?;
            let now = unix_timestamp_seconds();
            let update_result = sqlx::query(
                "UPDATE messages SET content = COALESCE(?, content), updated_at = ? WHERE session_id = ? AND id = ? AND deleted_at IS NULL",
            )
            .bind(patch.content)
            .bind(now)
            .bind(session_id.to_string())
            .bind(message_id.to_string())
            .execute(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-update-apply",
            })?;

            if update_result.rows_affected() == 0 {
                return NotFoundSnafu {
                    stage: "message-update-missing",
                    entity: "message",
                    id: message_id.to_string(),
                }
                .fail();
            }

            let row = sqlx::query_as::<_, MessageRow>(
                "SELECT id, session_id, branch_id, seq, role, content, deleted_at FROM messages WHERE session_id = ? AND id = ? AND deleted_at IS NULL",
            )
            .bind(session_id.to_string())
            .bind(message_id.to_string())
            .fetch_optional(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-update-load",
            })?
            .context(NotFoundSnafu {
                stage: "message-update-load-missing",
                entity: "message",
                id: message_id.to_string(),
            })?;

            message_row_to_record(row)
        })
    }

    fn fork_from_history(
        &self,
        session_id: SessionId,
        request: HistoryForkRequest,
    ) -> StorageResult<HistoryForkOutcome> {
        let database_url = self.database_url.clone();
        self.run_db_call("message-fork-from-history", async move {
            let mut connection = connect_store_connection(&database_url, "message-fork-connect").await?;
            let mut tx = connection.begin().await.context(SqliteQuerySnafu {
                stage: "message-fork-begin",
            })?;

            let active_branch_id = load_active_branch_id_in_tx(
                &mut tx,
                session_id,
                "message-fork-load-active-branch",
            )
            .await?;

            let source = sqlx::query_as::<_, ForkSourceRow>(
                "SELECT seq FROM messages WHERE session_id = ? AND branch_id = ? AND id = ? AND deleted_at IS NULL",
            )
            .bind(session_id.to_string())
            .bind(active_branch_id.to_string())
            .bind(request.source_message_id.to_string())
            .fetch_optional(&mut *tx)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-fork-load-source",
            })?
            .context(NotFoundSnafu {
                stage: "message-fork-source-missing",
                entity: "message",
                id: request.source_message_id.to_string(),
            })?;

            let now = unix_timestamp_seconds();
            let new_branch_id = BranchId::new_v7();

            sqlx::query(
                "INSERT INTO branches (id, session_id, parent_branch_id, created_at, deleted_at) VALUES (?, ?, ?, ?, NULL)",
            )
            .bind(new_branch_id.to_string())
            .bind(session_id.to_string())
            .bind(active_branch_id.to_string())
            .bind(now)
            .execute(&mut *tx)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-fork-insert-branch",
            })?;

            let prefix_rows = sqlx::query_as::<_, ForkPrefixRow>(
                "SELECT id, seq, role, content FROM messages WHERE session_id = ? AND branch_id = ? AND deleted_at IS NULL AND seq <= ? ORDER BY seq ASC, id ASC",
            )
            .bind(session_id.to_string())
            .bind(active_branch_id.to_string())
            .bind(source.seq)
            .fetch_all(&mut *tx)
            .await
            .context(SqliteQuerySnafu {
                stage: "message-fork-load-prefix",
            })?;

            if prefix_rows.is_empty() {
                return InvariantViolationSnafu {
                    stage: "message-fork-empty-prefix",
                    details: "fork prefix query unexpectedly returned zero rows".to_string(),
                }
                .fail();
            }

            let mut remaps = Vec::with_capacity(prefix_rows.len());

            // Keep all copy/write/swap steps in one transaction so branch visibility never
            // observes a partial state where both branches appear active.
            for prefix in prefix_rows {
                let old_message_id = MessageId::parse(&prefix.id)?;
                let new_message_id = MessageId::new_v7();
                let replacement_content = if old_message_id == request.source_message_id {
                    request.replacement_content.as_str()
                } else {
                    prefix.content.as_str()
                };

                sqlx::query(
                    "INSERT INTO messages (id, session_id, branch_id, seq, role, content, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)",
                )
                .bind(new_message_id.to_string())
                .bind(session_id.to_string())
                .bind(new_branch_id.to_string())
                .bind(prefix.seq)
                .bind(prefix.role)
                .bind(replacement_content)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await
                .context(SqliteQuerySnafu {
                    stage: "message-fork-copy-prefix",
                })?;

                remaps.push(MessageIdRemap {
                    old_message_id,
                    new_message_id,
                });
            }

            sqlx::query("UPDATE sessions SET active_branch_id = ?, updated_at = ? WHERE id = ?")
                .bind(new_branch_id.to_string())
                .bind(now)
                .bind(session_id.to_string())
                .execute(&mut *tx)
                .await
                .context(SqliteQuerySnafu {
                    stage: "message-fork-update-session-active-branch",
                })?;

            sqlx::query("UPDATE branches SET deleted_at = ? WHERE session_id = ? AND id = ?")
                .bind(now)
                .bind(session_id.to_string())
                .bind(active_branch_id.to_string())
                .execute(&mut *tx)
                .await
                .context(SqliteQuerySnafu {
                    stage: "message-fork-soft-delete-old-branch",
                })?;

            tx.commit().await.context(SqliteQuerySnafu {
                stage: "message-fork-commit",
            })?;

            Ok(HistoryForkOutcome {
                new_branch_id,
                message_id_remaps: remaps,
            })
        })
    }
}

impl MediaStore for SqliteStorage {
    fn attach_media(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        input: NewMediaRef,
    ) -> StorageResult<MediaRefRecord> {
        let database_url = self.database_url.clone();
        self.run_db_call("media-attach", async move {
            let mut connection = connect_store_connection(&database_url, "media-attach-connect").await?;
            ensure_message_in_session(
                &mut connection,
                session_id,
                message_id,
                "media-attach-ensure-message",
            )
            .await?;
            validate_media_uri(&input.uri, "media-attach-validate-uri")?;

            let media_ref_id = MediaRefId::new_v7();
            let now = unix_timestamp_seconds();
            let size_bytes = u64_to_i64(input.size_bytes, "media-attach-size-bytes")?;
            let duration_ms = input
                .duration_ms
                .map(|value| u64_to_i64(value, "media-attach-duration-ms"))
                .transpose()?;
            let width_px = input.width_px.map(i64::from);
            let height_px = input.height_px.map(i64::from);

            sqlx::query(
                "INSERT INTO media_refs (id, session_id, message_id, uri, mime_type, size_bytes, duration_ms, width_px, height_px, sha256_hex, created_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
            )
            .bind(media_ref_id.to_string())
            .bind(session_id.to_string())
            .bind(message_id.to_string())
            .bind(input.uri.clone())
            .bind(input.mime_type.clone())
            .bind(size_bytes)
            .bind(duration_ms)
            .bind(width_px)
            .bind(height_px)
            .bind(input.sha256_hex.clone())
            .bind(now)
            .execute(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "media-attach-insert",
            })?;

            Ok(MediaRefRecord {
                id: media_ref_id,
                session_id,
                message_id,
                uri: input.uri,
                mime_type: input.mime_type,
                size_bytes: input.size_bytes,
                duration_ms: input.duration_ms,
                width_px: input.width_px,
                height_px: input.height_px,
                sha256_hex: input.sha256_hex,
                deleted_at_unix_seconds: None,
            })
        })
    }

    fn list_media(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        include_deleted: bool,
    ) -> StorageResult<Vec<MediaRefRecord>> {
        let database_url = self.database_url.clone();
        self.run_db_call("media-list", async move {
            let mut connection = connect_store_connection(&database_url, "media-list-connect").await?;
            ensure_message_in_session(
                &mut connection,
                session_id,
                message_id,
                "media-list-ensure-message",
            )
            .await?;

            let rows = if include_deleted {
                sqlx::query_as::<_, MediaRefRow>(
                    "SELECT id, session_id, message_id, uri, mime_type, size_bytes, duration_ms, width_px, height_px, sha256_hex, deleted_at FROM media_refs WHERE session_id = ? AND message_id = ? ORDER BY created_at ASC, id ASC",
                )
                .bind(session_id.to_string())
                .bind(message_id.to_string())
                .fetch_all(&mut connection)
                .await
            } else {
                sqlx::query_as::<_, MediaRefRow>(
                    "SELECT id, session_id, message_id, uri, mime_type, size_bytes, duration_ms, width_px, height_px, sha256_hex, deleted_at FROM media_refs WHERE session_id = ? AND message_id = ? AND deleted_at IS NULL ORDER BY created_at ASC, id ASC",
                )
                .bind(session_id.to_string())
                .bind(message_id.to_string())
                .fetch_all(&mut connection)
                .await
            }
            .context(SqliteQuerySnafu {
                stage: "media-list-query",
            })?;

            rows.into_iter().map(media_ref_row_to_record).collect()
        })
    }

    fn soft_delete_media(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        media_ref_id: MediaRefId,
    ) -> StorageResult<()> {
        let database_url = self.database_url.clone();
        self.run_db_call("media-soft-delete", async move {
            let mut connection =
                connect_store_connection(&database_url, "media-soft-delete-connect").await?;
            ensure_message_in_session(
                &mut connection,
                session_id,
                message_id,
                "media-soft-delete-ensure-message",
            )
            .await?;

            let now = unix_timestamp_seconds();
            let result = sqlx::query(
                "UPDATE media_refs SET deleted_at = ? WHERE session_id = ? AND message_id = ? AND id = ? AND deleted_at IS NULL",
            )
            .bind(now)
            .bind(session_id.to_string())
            .bind(message_id.to_string())
            .bind(media_ref_id.to_string())
            .execute(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "media-soft-delete-apply",
            })?;

            if result.rows_affected() == 0 {
                let exists = media_ref_exists_in_scope(&mut connection, session_id, message_id, media_ref_id).await?;
                if !exists {
                    return NotFoundSnafu {
                        stage: "media-soft-delete-missing",
                        entity: "media_ref",
                        id: media_ref_id.to_string(),
                    }
                    .fail();
                }
            }

            Ok(())
        })
    }
}

impl AgentEventStore for SqliteStorage {
    fn append_agent_event(
        &self,
        session_id: SessionId,
        input: NewAgentEvent,
    ) -> StorageResult<AgentEventRecord> {
        let database_url = self.database_url.clone();
        self.run_db_call("agent-event-append", async move {
            let mut connection =
                connect_store_connection(&database_url, "agent-event-append-connect").await?;

            if let Some(message_id) = input.message_id {
                ensure_message_in_session(
                    &mut connection,
                    session_id,
                    message_id,
                    "agent-event-append-ensure-message",
                )
                .await?;
            } else {
                ensure_session_in_scope(&mut connection, session_id, "agent-event-append-ensure-session")
                    .await?;
            }

            let payload_is_valid = sqlx::query_scalar::<_, i64>("SELECT json_valid(?)")
                .bind(input.payload_json.as_str())
                .fetch_one(&mut connection)
                .await
                .context(SqliteQuerySnafu {
                    stage: "agent-event-append-validate-json",
                })?
                == 1;
            if !payload_is_valid {
                return ConflictSnafu {
                    stage: "agent-event-append-invalid-json",
                    entity: "agent_event",
                    details: "payload_json must be valid canonical JSON text".to_string(),
                }
                .fail();
            }

            let event_id = AgentEventId::new_v7();
            let now = unix_timestamp_seconds();
            sqlx::query(
                "INSERT INTO agent_events (id, session_id, message_id, event_type, payload_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(event_id.to_string())
            .bind(session_id.to_string())
            .bind(input.message_id.map(|value| value.to_string()))
            .bind(input.event_type.clone())
            .bind(input.payload_json.clone())
            .bind(now)
            .execute(&mut connection)
            .await
            .context(SqliteQuerySnafu {
                stage: "agent-event-append-insert",
            })?;

            Ok(AgentEventRecord {
                id: event_id,
                session_id,
                message_id: input.message_id,
                event_type: input.event_type,
                payload_json: input.payload_json,
                created_at_unix_seconds: i64_to_u64(now, "agent-event-append-created-at")?,
            })
        })
    }

    fn list_agent_events(
        &self,
        session_id: SessionId,
        message_id: Option<MessageId>,
    ) -> StorageResult<Vec<AgentEventRecord>> {
        let database_url = self.database_url.clone();
        self.run_db_call("agent-event-list", async move {
            let mut connection = connect_store_connection(&database_url, "agent-event-list-connect").await?;

            if let Some(scoped_message_id) = message_id {
                ensure_message_in_session(
                    &mut connection,
                    session_id,
                    scoped_message_id,
                    "agent-event-list-ensure-message",
                )
                .await?;

                let rows = sqlx::query_as::<_, AgentEventRow>(
                    "SELECT id, session_id, message_id, event_type, payload_json, created_at FROM agent_events WHERE session_id = ? AND message_id = ? ORDER BY created_at ASC, id ASC",
                )
                .bind(session_id.to_string())
                .bind(scoped_message_id.to_string())
                .fetch_all(&mut connection)
                .await
                .context(SqliteQuerySnafu {
                    stage: "agent-event-list-query-message",
                })?;
                rows.into_iter().map(agent_event_row_to_record).collect()
            } else {
                ensure_session_in_scope(&mut connection, session_id, "agent-event-list-ensure-session")
                    .await?;

                let rows = sqlx::query_as::<_, AgentEventRow>(
                    "SELECT id, session_id, message_id, event_type, payload_json, created_at FROM agent_events WHERE session_id = ? ORDER BY created_at ASC, id ASC",
                )
                .bind(session_id.to_string())
                .fetch_all(&mut connection)
                .await
                .context(SqliteQuerySnafu {
                    stage: "agent-event-list-query-session",
                })?;
                rows.into_iter().map(agent_event_row_to_record).collect()
            }
        })
    }
}

#[derive(Debug, FromRow)]
struct SessionRow {
    id: String,
    title: String,
    active_branch_id: Option<String>,
    updated_at: i64,
    deleted_at: Option<i64>,
}

#[derive(Debug, FromRow)]
struct MessageRow {
    id: String,
    session_id: String,
    branch_id: String,
    seq: i64,
    role: String,
    content: String,
    deleted_at: Option<i64>,
}

#[derive(Debug, FromRow)]
struct ForkSourceRow {
    seq: i64,
}

#[derive(Debug, FromRow)]
struct ForkPrefixRow {
    id: String,
    seq: i64,
    role: String,
    content: String,
}

#[derive(Debug, FromRow)]
struct MediaRefRow {
    id: String,
    session_id: String,
    message_id: String,
    uri: String,
    mime_type: String,
    size_bytes: i64,
    duration_ms: Option<i64>,
    width_px: Option<i64>,
    height_px: Option<i64>,
    sha256_hex: Option<String>,
    deleted_at: Option<i64>,
}

#[derive(Debug, FromRow)]
struct AgentEventRow {
    id: String,
    session_id: String,
    message_id: Option<String>,
    event_type: String,
    payload_json: String,
    created_at: i64,
}

fn session_row_to_record(row: SessionRow) -> StorageResult<SessionRecord> {
    Ok(SessionRecord {
        id: SessionId::parse(&row.id)?,
        title: row.title,
        active_branch_id: BranchId::parse(&row.active_branch_id.context(
            InvariantViolationSnafu {
                stage: "session-row-active-branch-missing",
                details: "session row is missing active_branch_id".to_string(),
            },
        )?)?,
        updated_at_unix_seconds: i64_to_u64(row.updated_at, "session-row-updated-at")?,
        deleted_at_unix_seconds: row
            .deleted_at
            .map(|value| i64_to_u64(value, "session-row-deleted-at"))
            .transpose()?,
    })
}

fn message_row_to_record(row: MessageRow) -> StorageResult<MessageRecord> {
    Ok(MessageRecord {
        id: MessageId::parse(&row.id)?,
        session_id: SessionId::parse(&row.session_id)?,
        branch_id: BranchId::parse(&row.branch_id)?,
        seq: i64_to_u64(row.seq, "message-row-seq")?,
        role: role_from_sql(&row.role)?,
        content: row.content,
        deleted_at_unix_seconds: row
            .deleted_at
            .map(|value| i64_to_u64(value, "message-row-deleted-at"))
            .transpose()?,
    })
}

fn media_ref_row_to_record(row: MediaRefRow) -> StorageResult<MediaRefRecord> {
    Ok(MediaRefRecord {
        id: MediaRefId::parse(&row.id)?,
        session_id: SessionId::parse(&row.session_id)?,
        message_id: MessageId::parse(&row.message_id)?,
        uri: row.uri,
        mime_type: row.mime_type,
        size_bytes: i64_to_u64(row.size_bytes, "media-row-size-bytes")?,
        duration_ms: row
            .duration_ms
            .map(|value| i64_to_u64(value, "media-row-duration-ms"))
            .transpose()?,
        width_px: row
            .width_px
            .map(|value| i64_to_u32(value, "media-row-width-px"))
            .transpose()?,
        height_px: row
            .height_px
            .map(|value| i64_to_u32(value, "media-row-height-px"))
            .transpose()?,
        sha256_hex: row.sha256_hex,
        deleted_at_unix_seconds: row
            .deleted_at
            .map(|value| i64_to_u64(value, "media-row-deleted-at"))
            .transpose()?,
    })
}

fn agent_event_row_to_record(row: AgentEventRow) -> StorageResult<AgentEventRecord> {
    Ok(AgentEventRecord {
        id: AgentEventId::parse(&row.id)?,
        session_id: SessionId::parse(&row.session_id)?,
        message_id: row
            .message_id
            .as_deref()
            .map(MessageId::parse)
            .transpose()?,
        event_type: row.event_type,
        payload_json: row.payload_json,
        created_at_unix_seconds: i64_to_u64(row.created_at, "agent-event-row-created-at")?,
    })
}

async fn connect_store_connection(
    database_url: &str,
    stage: &'static str,
) -> StorageResult<SqliteConnection> {
    let mut connection =
        SqliteConnection::connect(database_url)
            .await
            .context(SqliteConnectSnafu {
                stage,
                database_url: database_url.to_string(),
            })?;

    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&mut connection)
        .await
        .context(SqlitePragmaSnafu {
            stage: "sqlite-store-pragma-foreign-keys",
            pragma: "foreign_keys",
        })?;
    sqlx::query("PRAGMA busy_timeout = 5000;")
        .execute(&mut connection)
        .await
        .context(SqlitePragmaSnafu {
            stage: "sqlite-store-pragma-busy-timeout",
            pragma: "busy_timeout",
        })?;

    Ok(connection)
}

async fn session_exists(
    connection: &mut SqliteConnection,
    session_id: SessionId,
) -> StorageResult<bool> {
    let existing = sqlx::query_scalar::<_, i64>("SELECT 1 FROM sessions WHERE id = ? LIMIT 1")
        .bind(session_id.to_string())
        .fetch_optional(&mut *connection)
        .await
        .context(SqliteQuerySnafu {
            stage: "session-exists-query",
        })?;

    Ok(existing.is_some())
}

async fn ensure_session_in_scope(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    stage: &'static str,
) -> StorageResult<()> {
    let exists = session_exists(connection, session_id).await?;
    if !exists {
        return NotFoundSnafu {
            stage,
            entity: "session",
            id: session_id.to_string(),
        }
        .fail();
    }

    Ok(())
}

async fn ensure_message_in_session(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    message_id: MessageId,
    stage: &'static str,
) -> StorageResult<()> {
    let exists = message_exists_in_scope(connection, session_id, message_id).await?;
    if !exists {
        return NotFoundSnafu {
            stage,
            entity: "message",
            id: message_id.to_string(),
        }
        .fail();
    }

    Ok(())
}

async fn message_exists_in_scope(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    message_id: MessageId,
) -> StorageResult<bool> {
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM messages WHERE session_id = ? AND id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(session_id.to_string())
    .bind(message_id.to_string())
    .fetch_optional(&mut *connection)
    .await
    .context(SqliteQuerySnafu {
        stage: "message-exists-in-scope-query",
    })?;

    Ok(existing.is_some())
}

async fn media_ref_exists_in_scope(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    message_id: MessageId,
    media_ref_id: MediaRefId,
) -> StorageResult<bool> {
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM media_refs WHERE session_id = ? AND message_id = ? AND id = ? LIMIT 1",
    )
    .bind(session_id.to_string())
    .bind(message_id.to_string())
    .bind(media_ref_id.to_string())
    .fetch_optional(&mut *connection)
    .await
    .context(SqliteQuerySnafu {
        stage: "media-ref-exists-in-scope-query",
    })?;

    Ok(existing.is_some())
}

async fn load_active_branch_id(
    connection: &mut SqliteConnection,
    session_id: SessionId,
    stage: &'static str,
) -> StorageResult<BranchId> {
    let active_branch_id = sqlx::query_scalar::<_, Option<String>>(
        "SELECT active_branch_id FROM sessions WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(session_id.to_string())
    .fetch_optional(&mut *connection)
    .await
    .context(SqliteQuerySnafu { stage })?
    .context(NotFoundSnafu {
        stage,
        entity: "session",
        id: session_id.to_string(),
    })?
    .context(InvariantViolationSnafu {
        stage,
        details: format!("session '{}' has NULL active_branch_id", session_id),
    })?;

    BranchId::parse(&active_branch_id)
}

async fn load_active_branch_id_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: SessionId,
    stage: &'static str,
) -> StorageResult<BranchId> {
    let active_branch_id = sqlx::query_scalar::<_, Option<String>>(
        "SELECT active_branch_id FROM sessions WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(session_id.to_string())
    .fetch_optional(&mut **tx)
    .await
    .context(SqliteQuerySnafu { stage })?
    .context(NotFoundSnafu {
        stage,
        entity: "session",
        id: session_id.to_string(),
    })?
    .context(InvariantViolationSnafu {
        stage,
        details: format!("session '{}' has NULL active_branch_id", session_id),
    })?;

    BranchId::parse(&active_branch_id)
}

fn role_to_sql(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    }
}

fn role_from_sql(raw: &str) -> StorageResult<MessageRole> {
    match raw {
        "system" => Ok(MessageRole::System),
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        _ => InvariantViolationSnafu {
            stage: "message-role-from-sql",
            details: format!("unknown message role '{raw}'"),
        }
        .fail(),
    }
}

fn unix_timestamp_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0_i64, |duration| duration.as_secs() as i64)
}

fn i64_to_u64(value: i64, stage: &'static str) -> StorageResult<u64> {
    value
        .try_into()
        .map_err(|_| super::error::StorageError::InvariantViolation {
            stage,
            details: format!("negative sqlite integer '{value}' cannot map to u64"),
        })
}

fn i64_to_u32(value: i64, stage: &'static str) -> StorageResult<u32> {
    value
        .try_into()
        .map_err(|_| super::error::StorageError::InvariantViolation {
            stage,
            details: format!("sqlite integer '{value}' cannot map to u32"),
        })
}

fn u64_to_i64(value: u64, stage: &'static str) -> StorageResult<i64> {
    value
        .try_into()
        .map_err(|_| super::error::StorageError::InvariantViolation {
            stage,
            details: format!("u64 '{value}' cannot map to sqlite i64"),
        })
}

fn validate_media_uri(uri: &str, stage: &'static str) -> StorageResult<()> {
    let uri_lower = uri.to_ascii_lowercase();
    let is_blob_like = uri_lower.starts_with("data:") || uri_lower.contains(";base64,");
    if is_blob_like {
        return ConflictSnafu {
            stage,
            entity: "media_ref",
            details: "blob payloads are not allowed; store URI/path references only".to_string(),
        }
        .fail();
    }

    Ok(())
}

fn parse_legacy_conversation_rows(
    store: &str,
) -> (Vec<LegacyConversationRow>, Vec<LegacyImportWarning>) {
    let mut rows = Vec::new();
    let mut warnings = Vec::new();

    for (index, line) in store.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        match parse_legacy_conversation_row(line) {
            Ok(parsed) => rows.push(parsed),
            Err(reason) => warnings.push(LegacyImportWarning {
                line_number: index + 1,
                reason,
            }),
        }
    }

    // Align with legacy ordering expectations: newest timestamp first, then highest legacy id.
    rows.sort_by(|left, right| {
        right
            .updated_at_unix_seconds
            .cmp(&left.updated_at_unix_seconds)
            .then_with(|| right.legacy_id.cmp(&left.legacy_id))
    });

    (rows, warnings)
}

fn parse_legacy_conversation_row(line: &str) -> Result<LegacyConversationRow, &'static str> {
    let mut fields = line.splitn(3, '\t');
    let raw_id = fields.next().ok_or("missing-id")?;
    let raw_updated_at = fields.next().ok_or("missing-updated-at")?;
    let raw_title = fields.next().ok_or("missing-title")?;

    let legacy_id = parse_legacy_u64(raw_id).map_err(|_| "invalid-id")?;
    let updated_at_unix_seconds =
        parse_legacy_u64(raw_updated_at).map_err(|_| "invalid-updated-at")?;
    let decoded_title = decode_legacy_title(raw_title);

    // Legacy create behavior defaults empty/whitespace-only titles to "New Conversation".
    let title = if decoded_title.trim().is_empty() {
        DEFAULT_SESSION_TITLE.to_string()
    } else {
        decoded_title
    };

    Ok(LegacyConversationRow {
        legacy_id,
        updated_at_unix_seconds,
        title,
    })
}

fn parse_legacy_u64(raw: &str) -> Result<u64, ParseIntError> {
    raw.parse::<u64>()
}

fn decode_legacy_title(encoded_title: &str) -> String {
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

fn ensure_database_directory(database_location: &str) -> StorageResult<()> {
    if database_location.starts_with("sqlite:") || database_location == ":memory:" {
        return Ok(());
    }

    let path = Path::new(database_location);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).context(CreateSqliteDirectorySnafu {
            stage: "sqlite-open-create-directory",
            path: parent.display().to_string(),
        })?;
    }

    Ok(())
}

fn normalize_database_url(database_location: &str) -> String {
    if database_location.starts_with("sqlite:") {
        return database_location.to_string();
    }

    if database_location == ":memory:" {
        return "sqlite::memory:".to_string();
    }

    format!("sqlite://{database_location}")
}
