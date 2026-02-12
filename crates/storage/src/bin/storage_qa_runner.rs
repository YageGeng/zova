use std::cmp::Ordering;
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use snafu::{OptionExt, ResultExt, Snafu};

use zova_storage::sqlite::LEGACY_CONVERSATIONS_TSV_RELATIVE_PATH;
use zova_storage::{
    AgentEventId, AgentEventStore, BranchId, DEFAULT_SESSION_TITLE, HistoryForkRequest, MediaRefId,
    MediaStore, MessageId, MessagePatch, MessageRole, MessageStore, NewAgentEvent, NewMediaRef,
    NewMessage, NewSession, SessionId, SessionPatch, SessionStore, SqliteStorage, StorageError,
};

#[derive(Debug, Clone)]
struct RunnerArgs {
    scenario: Scenario,
    db_path: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum Scenario {
    IdRoundtrip,
    IdInvalid,
    PrepNoop,
    SchemaInit,
    FkViolation,
    SessionCrud,
    HistoryBranchFork,
    CrossSessionGuard,
    MediaRefRoundtrip,
    MediaBlobGuard,
    AgentEventRoundtrip,
    MigrateTsvFixture,
    MigrateIdempotent,
    MigrateMalformedRow,
    All,
}

impl Scenario {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "id_roundtrip" => Some(Self::IdRoundtrip),
            "id_invalid" => Some(Self::IdInvalid),
            "prep_noop" => Some(Self::PrepNoop),
            "schema_init" => Some(Self::SchemaInit),
            "fk_violation" => Some(Self::FkViolation),
            "session_crud" => Some(Self::SessionCrud),
            "history_branch_fork" => Some(Self::HistoryBranchFork),
            "cross_session_guard" => Some(Self::CrossSessionGuard),
            "media_ref_roundtrip" => Some(Self::MediaRefRoundtrip),
            "media_blob_guard" => Some(Self::MediaBlobGuard),
            "agent_event_roundtrip" => Some(Self::AgentEventRoundtrip),
            "migrate_tsv_fixture" => Some(Self::MigrateTsvFixture),
            "migrate_idempotent" => Some(Self::MigrateIdempotent),
            "migrate_malformed_row" => Some(Self::MigrateMalformedRow),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::IdRoundtrip => "id_roundtrip",
            Self::IdInvalid => "id_invalid",
            Self::PrepNoop => "prep_noop",
            Self::SchemaInit => "schema_init",
            Self::FkViolation => "fk_violation",
            Self::SessionCrud => "session_crud",
            Self::HistoryBranchFork => "history_branch_fork",
            Self::CrossSessionGuard => "cross_session_guard",
            Self::MediaRefRoundtrip => "media_ref_roundtrip",
            Self::MediaBlobGuard => "media_blob_guard",
            Self::AgentEventRoundtrip => "agent_event_roundtrip",
            Self::MigrateTsvFixture => "migrate_tsv_fixture",
            Self::MigrateIdempotent => "migrate_idempotent",
            Self::MigrateMalformedRow => "migrate_malformed_row",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Snafu)]
enum RunnerError {
    #[snafu(display("missing required --scenario argument"))]
    MissingScenario { stage: &'static str },
    #[snafu(display("missing value for argument '{arg}'"))]
    MissingArgumentValue {
        stage: &'static str,
        arg: &'static str,
    },
    #[snafu(display("unknown scenario '{raw}'"))]
    UnknownScenario { stage: &'static str, raw: String },
    #[snafu(display("unknown argument '{raw}'"))]
    UnknownArgument { stage: &'static str, raw: String },
    #[snafu(display("storage validation failed: {source}"))]
    StorageValidation {
        stage: &'static str,
        source: StorageError,
    },
    #[snafu(display("missing required --db argument for scenario '{scenario}'"))]
    MissingDbPath {
        stage: &'static str,
        scenario: &'static str,
    },
    #[snafu(display("sqlite query failed: {source}"))]
    SqliteQuery {
        stage: &'static str,
        source: sqlx::Error,
    },
    #[snafu(display("scenario '{scenario}' failed: {reason}"))]
    ScenarioFailed {
        stage: &'static str,
        scenario: &'static str,
        reason: String,
    },
    #[snafu(display("file operation failed at '{path}': {source}"))]
    FileIo {
        stage: &'static str,
        path: String,
        source: std::io::Error,
    },
}

type RunnerResult<T> = Result<T, RunnerError>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        println!("runner_ok=false");
        eprintln!("runner_error={error}");
        std::process::exit(1);
    }
}

async fn run() -> RunnerResult<()> {
    let args = parse_args(env::args().skip(1))?;
    println!("scenario={}", args.scenario.name());
    if let Some(db_path) = args.db_path.as_deref() {
        println!("db_path={db_path}");
    }

    match args.scenario {
        Scenario::IdRoundtrip => run_id_roundtrip(),
        Scenario::IdInvalid => run_id_invalid(),
        Scenario::PrepNoop => run_prep_noop(),
        Scenario::SchemaInit => run_schema_init(require_db_path(&args, "schema_init")?).await,
        Scenario::FkViolation => run_fk_violation(require_db_path(&args, "fk_violation")?).await,
        Scenario::SessionCrud => run_session_crud(require_db_path(&args, "session_crud")?).await,
        Scenario::HistoryBranchFork => {
            run_history_branch_fork(require_db_path(&args, "history_branch_fork")?).await
        }
        Scenario::CrossSessionGuard => {
            run_cross_session_guard(require_db_path(&args, "cross_session_guard")?).await
        }
        Scenario::MediaRefRoundtrip => {
            run_media_ref_roundtrip(require_db_path(&args, "media_ref_roundtrip")?).await
        }
        Scenario::MediaBlobGuard => {
            run_media_blob_guard(require_db_path(&args, "media_blob_guard")?).await
        }
        Scenario::AgentEventRoundtrip => {
            run_agent_event_roundtrip(require_db_path(&args, "agent_event_roundtrip")?).await
        }
        Scenario::MigrateTsvFixture => {
            run_migrate_tsv_fixture(require_db_path(&args, "migrate_tsv_fixture")?).await
        }
        Scenario::MigrateIdempotent => {
            run_migrate_idempotent(require_db_path(&args, "migrate_idempotent")?).await
        }
        Scenario::MigrateMalformedRow => {
            run_migrate_malformed_row(require_db_path(&args, "migrate_malformed_row")?).await
        }
        Scenario::All => run_all(args.db_path.as_deref()).await,
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> RunnerResult<RunnerArgs> {
    let mut scenario = None;
    let mut db_path = None;
    let mut pending = args.into_iter();

    // The parser is intentionally strict to keep scenario execution deterministic in CI.
    while let Some(argument) = pending.next() {
        match argument.as_str() {
            "--scenario" => {
                let value = pending.next().context(MissingArgumentValueSnafu {
                    stage: "parse-args-scenario-value",
                    arg: "--scenario",
                })?;

                let parsed = Scenario::parse(&value).context(UnknownScenarioSnafu {
                    stage: "parse-args-scenario",
                    raw: value,
                })?;
                scenario = Some(parsed);
            }
            "--db" => {
                let value = pending.next().context(MissingArgumentValueSnafu {
                    stage: "parse-args-db-value",
                    arg: "--db",
                })?;
                db_path = Some(value);
            }
            _ => {
                return UnknownArgumentSnafu {
                    stage: "parse-args",
                    raw: argument,
                }
                .fail();
            }
        }
    }

    Ok(RunnerArgs {
        scenario: scenario.context(MissingScenarioSnafu {
            stage: "parse-args-scenario-required",
        })?,
        db_path,
    })
}

fn run_id_roundtrip() -> RunnerResult<()> {
    assert_id_roundtrip("session_id", SessionId::new_v7())?;
    assert_id_roundtrip("message_id", MessageId::new_v7())?;
    assert_id_roundtrip("branch_id", BranchId::new_v7())?;
    assert_id_roundtrip("media_ref_id", MediaRefId::new_v7())?;
    assert_id_roundtrip("agent_event_id", AgentEventId::new_v7())?;
    println!("id_roundtrip=true");
    println!("runner_ok=true");
    Ok(())
}

fn run_id_invalid() -> RunnerResult<()> {
    let invalid_input = "not-a-valid-uuid";
    let invalid_id_error = invalid_input_is_rejected::<SessionId>(invalid_input)
        && invalid_input_is_rejected::<MessageId>(invalid_input)
        && invalid_input_is_rejected::<BranchId>(invalid_input)
        && invalid_input_is_rejected::<MediaRefId>(invalid_input)
        && invalid_input_is_rejected::<AgentEventId>(invalid_input);

    println!("invalid_id_error={invalid_id_error}");
    if !invalid_id_error {
        return ScenarioFailedSnafu {
            stage: "scenario-id-invalid",
            scenario: "id_invalid",
            reason: "at least one ID wrapper accepted malformed UUID input".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

fn run_prep_noop() -> RunnerResult<()> {
    println!("prep_noop=true");
    println!("runner_ok=true");
    Ok(())
}

async fn run_all(db_path: Option<&str>) -> RunnerResult<()> {
    run_id_roundtrip()?;
    run_id_invalid()?;
    run_prep_noop()?;

    if let Some(path) = db_path {
        run_schema_init(path).await?;
        run_fk_violation(path).await?;
        run_session_crud(path).await?;
        run_history_branch_fork(path).await?;
        run_cross_session_guard(path).await?;
        run_media_ref_roundtrip(path).await?;
        run_media_blob_guard(path).await?;
        run_agent_event_roundtrip(path).await?;
    }

    println!("all_passed=true");
    Ok(())
}

async fn run_schema_init(db_path: &str) -> RunnerResult<()> {
    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-schema-init-open",
        })?;
    let pool = storage.pool();

    let discovered_tables = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('sessions', 'branches', 'messages', 'media_refs', 'agent_events')",
    )
    .fetch_all(pool)
    .await
    .context(SqliteQuerySnafu {
        stage: "scenario-schema-init-list-tables",
    })?;

    let required_tables = [
        "sessions",
        "branches",
        "messages",
        "media_refs",
        "agent_events",
    ];
    let available_tables: HashSet<String> = discovered_tables.into_iter().collect();
    let schema_ok = required_tables
        .iter()
        .all(|table_name| available_tables.contains(*table_name));

    let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode;")
        .fetch_one(pool)
        .await
        .context(SqliteQuerySnafu {
            stage: "scenario-schema-init-journal-mode",
        })?
        .to_lowercase();
    let foreign_keys = sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys;")
        .fetch_one(pool)
        .await
        .context(SqliteQuerySnafu {
            stage: "scenario-schema-init-foreign-keys",
        })?;
    let busy_timeout = sqlx::query_scalar::<_, i64>("PRAGMA busy_timeout;")
        .fetch_one(pool)
        .await
        .context(SqliteQuerySnafu {
            stage: "scenario-schema-init-busy-timeout",
        })?;

    println!("schema_ok={schema_ok}");
    println!("journal_mode={journal_mode}");
    println!("foreign_keys={foreign_keys}");
    println!("busy_timeout={busy_timeout}");

    if !schema_ok {
        return ScenarioFailedSnafu {
            stage: "scenario-schema-init-assert-schema",
            scenario: "schema_init",
            reason: "expected migration tables are missing".to_string(),
        }
        .fail();
    }

    if journal_mode != "wal" {
        return ScenarioFailedSnafu {
            stage: "scenario-schema-init-assert-journal-mode",
            scenario: "schema_init",
            reason: format!("expected journal_mode=wal but was {journal_mode}"),
        }
        .fail();
    }

    if foreign_keys != 1 {
        return ScenarioFailedSnafu {
            stage: "scenario-schema-init-assert-foreign-keys",
            scenario: "schema_init",
            reason: format!("expected foreign_keys=1 but was {foreign_keys}"),
        }
        .fail();
    }

    if busy_timeout != 5_000 {
        return ScenarioFailedSnafu {
            stage: "scenario-schema-init-assert-busy-timeout",
            scenario: "schema_init",
            reason: format!("expected busy_timeout=5000 but was {busy_timeout}"),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_fk_violation(db_path: &str) -> RunnerResult<()> {
    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-fk-violation-open",
        })?;
    let pool = storage.pool();

    let insert_result = sqlx::query(
        "INSERT INTO messages (id, session_id, branch_id, seq, role, content, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(MessageId::new_v7().to_string())
    .bind(SessionId::new_v7().to_string())
    .bind(BranchId::new_v7().to_string())
    .bind(1_i64)
    .bind("user")
    .bind("orphan-row")
    .bind(0_i64)
    .bind(0_i64)
    .bind(Option::<i64>::None)
    .execute(pool)
    .await;

    let fk_violation_blocked = match insert_result {
        Ok(_) => false,
        Err(error) => {
            if is_foreign_key_violation(&error) {
                true
            } else {
                return ScenarioFailedSnafu {
                    stage: "scenario-fk-violation-insert",
                    scenario: "fk_violation",
                    reason: format!("unexpected sqlite error while testing FK guard: {error}"),
                }
                .fail();
            }
        }
    };

    println!("fk_violation_blocked={fk_violation_blocked}");
    if !fk_violation_blocked {
        return ScenarioFailedSnafu {
            stage: "scenario-fk-violation-assert",
            scenario: "fk_violation",
            reason: "orphan message insert unexpectedly succeeded".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_session_crud(db_path: &str) -> RunnerResult<()> {
    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-open",
        })?;

    let created_a = storage
        .create_session(NewSession {
            title: "session-a".to_string(),
        })
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-create-a",
        })?;
    let created_b = storage
        .create_session(NewSession {
            title: "session-b".to_string(),
        })
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-create-b",
        })?;

    storage
        .get_session(created_a.id)
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-get-a",
        })?
        .context(ScenarioFailedSnafu {
            stage: "scenario-session-crud-get-a-missing",
            scenario: "session_crud",
            reason: "created session_a not found".to_string(),
        })?;

    storage
        .update_session(
            created_b.id,
            SessionPatch {
                title: Some("session-b-updated".to_string()),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-update-b",
        })?;

    storage
        .soft_delete_session(created_a.id)
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-soft-delete-a",
        })?;
    let active_after_delete = storage
        .list_sessions(false)
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-list-after-delete",
        })?;

    storage
        .restore_session(created_a.id)
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-restore-a",
        })?;
    let active_after_restore = storage
        .list_sessions(false)
        .context(StorageValidationSnafu {
            stage: "scenario-session-crud-list-after-restore",
        })?;

    let created = 2_i64;
    let soft_deleted = 1_i64;
    let restored = 1_i64;
    let list_order_ok = is_session_list_ordered(&active_after_restore);

    println!("created={created}");
    println!("soft_deleted={soft_deleted}");
    println!("restored={restored}");
    println!("active_after_delete_count={}", active_after_delete.len());
    println!("active_after_restore_count={}", active_after_restore.len());
    println!("list_order_ok={list_order_ok}");

    if active_after_delete.len() != 1 {
        return ScenarioFailedSnafu {
            stage: "scenario-session-crud-assert-delete-filter",
            scenario: "session_crud",
            reason: format!(
                "expected exactly one active session after soft-delete, got {}",
                active_after_delete.len()
            ),
        }
        .fail();
    }

    if active_after_restore.len() != 2 {
        return ScenarioFailedSnafu {
            stage: "scenario-session-crud-assert-restore",
            scenario: "session_crud",
            reason: format!(
                "expected two active sessions after restore, got {}",
                active_after_restore.len()
            ),
        }
        .fail();
    }

    if !list_order_ok {
        return ScenarioFailedSnafu {
            stage: "scenario-session-crud-assert-order",
            scenario: "session_crud",
            reason:
                "session listing order is not updated_at DESC with deterministic id DESC tie-break"
                    .to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_history_branch_fork(db_path: &str) -> RunnerResult<()> {
    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-history-branch-fork-open",
        })?;
    let pool = storage.pool();

    let session = storage
        .create_session(NewSession {
            title: "fork-session".to_string(),
        })
        .context(StorageValidationSnafu {
            stage: "scenario-history-branch-fork-create-session",
        })?;
    let old_branch_id = session.active_branch_id;

    let first = storage
        .append_message(
            session.id,
            NewMessage {
                role: MessageRole::User,
                content: "first".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-history-branch-fork-append-first",
        })?;
    let second = storage
        .append_message(
            session.id,
            NewMessage {
                role: MessageRole::Assistant,
                content: "second".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-history-branch-fork-append-second",
        })?;
    let _third = storage
        .append_message(
            session.id,
            NewMessage {
                role: MessageRole::User,
                content: "third".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-history-branch-fork-append-third",
        })?;

    let outcome = storage
        .fork_from_history(
            session.id,
            HistoryForkRequest {
                source_message_id: second.id,
                replacement_content: "second-edited".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-history-branch-fork-execute",
        })?;

    let active_messages = storage
        .list_messages(session.id)
        .context(StorageValidationSnafu {
            stage: "scenario-history-branch-fork-list-active",
        })?;

    let old_branch_visible_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM messages AS m JOIN branches AS b ON b.session_id = m.session_id AND b.id = m.branch_id WHERE m.session_id = ? AND m.branch_id = ? AND m.deleted_at IS NULL AND b.deleted_at IS NULL",
    )
    .bind(session.id.to_string())
    .bind(old_branch_id.to_string())
    .fetch_one(pool)
    .await
    .context(SqliteQuerySnafu {
        stage: "scenario-history-branch-fork-old-branch-visible-count",
    })?;

    let remap_deterministic = outcome
        .message_id_remaps
        .iter()
        .map(|remap| remap.old_message_id)
        .collect::<Vec<_>>()
        == vec![first.id, second.id];
    let replacement_ok = active_messages.len() == 2
        && active_messages
            .iter()
            .any(|message| message.id != second.id && message.content == "second-edited");
    let fork_created = outcome.new_branch_id != old_branch_id;

    println!("fork_created={fork_created}");
    println!("active_branch_visible_count={}", active_messages.len());
    println!("old_branch_visible_count={old_branch_visible_count}");
    println!("remap_deterministic={remap_deterministic}");
    println!("replacement_ok={replacement_ok}");

    if !fork_created {
        return ScenarioFailedSnafu {
            stage: "scenario-history-branch-fork-assert-created",
            scenario: "history_branch_fork",
            reason: "fork operation did not create a new active branch".to_string(),
        }
        .fail();
    }

    if old_branch_visible_count != 0 {
        return ScenarioFailedSnafu {
            stage: "scenario-history-branch-fork-assert-old-branch-hidden",
            scenario: "history_branch_fork",
            reason: format!("expected old_branch_visible_count=0, got {old_branch_visible_count}"),
        }
        .fail();
    }

    if !remap_deterministic {
        return ScenarioFailedSnafu {
            stage: "scenario-history-branch-fork-assert-remap",
            scenario: "history_branch_fork",
            reason: "message_id_remaps order/content is not deterministic".to_string(),
        }
        .fail();
    }

    if !replacement_ok {
        return ScenarioFailedSnafu {
            stage: "scenario-history-branch-fork-assert-replacement",
            scenario: "history_branch_fork",
            reason: "replacement message content missing from active branch history".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_cross_session_guard(db_path: &str) -> RunnerResult<()> {
    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-cross-session-guard-open",
        })?;

    let session_a = storage
        .create_session(NewSession {
            title: "guard-a".to_string(),
        })
        .context(StorageValidationSnafu {
            stage: "scenario-cross-session-guard-create-a",
        })?;
    let session_b = storage
        .create_session(NewSession {
            title: "guard-b".to_string(),
        })
        .context(StorageValidationSnafu {
            stage: "scenario-cross-session-guard-create-b",
        })?;

    let message = storage
        .append_message(
            session_a.id,
            NewMessage {
                role: MessageRole::User,
                content: "session-a-original".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-cross-session-guard-append",
        })?;

    let wrong_session_update = storage.update_message(
        session_b.id,
        message.id,
        MessagePatch {
            content: Some("cross-session-override".to_string()),
        },
    );

    let mutation_blocked = matches!(wrong_session_update, Err(StorageError::NotFound { .. }));
    let original_message = storage
        .get_message(session_a.id, message.id)
        .context(StorageValidationSnafu {
            stage: "scenario-cross-session-guard-get-original",
        })?
        .context(ScenarioFailedSnafu {
            stage: "scenario-cross-session-guard-get-original-missing",
            scenario: "cross_session_guard",
            reason: "original message unexpectedly missing".to_string(),
        })?;

    let cross_session_guard = mutation_blocked && original_message.content == "session-a-original";

    println!("cross_session_guard={cross_session_guard}");
    if !cross_session_guard {
        return ScenarioFailedSnafu {
            stage: "scenario-cross-session-guard-assert",
            scenario: "cross_session_guard",
            reason: "cross-session message mutation was not rejected".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_media_ref_roundtrip(db_path: &str) -> RunnerResult<()> {
    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-media-roundtrip-open",
        })?;

    let session = storage
        .create_session(NewSession {
            title: "media-roundtrip".to_string(),
        })
        .context(StorageValidationSnafu {
            stage: "scenario-media-roundtrip-create-session",
        })?;
    let message = storage
        .append_message(
            session.id,
            NewMessage {
                role: MessageRole::User,
                content: "media-target".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-media-roundtrip-append-message",
        })?;

    let attached = storage
        .attach_media(
            session.id,
            message.id,
            NewMediaRef {
                uri: "file:///tmp/sample-audio.mp3".to_string(),
                mime_type: "audio/mpeg".to_string(),
                size_bytes: 4_096,
                duration_ms: Some(3_500),
                width_px: None,
                height_px: None,
                sha256_hex: Some("abc123".to_string()),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-media-roundtrip-attach",
        })?;

    let listed =
        storage
            .list_media(session.id, message.id, false)
            .context(StorageValidationSnafu {
                stage: "scenario-media-roundtrip-list-active",
            })?;

    let stored_uri = listed
        .first()
        .map(|record| record.uri.clone())
        .unwrap_or_default();
    let metadata_ok = listed.len() == 1
        && listed[0].mime_type == "audio/mpeg"
        && listed[0].size_bytes == 4_096
        && listed[0].duration_ms == Some(3_500)
        && listed[0].sha256_hex.as_deref() == Some("abc123");

    storage
        .soft_delete_media(session.id, message.id, attached.id)
        .context(StorageValidationSnafu {
            stage: "scenario-media-roundtrip-soft-delete",
        })?;
    let active_after_delete =
        storage
            .list_media(session.id, message.id, false)
            .context(StorageValidationSnafu {
                stage: "scenario-media-roundtrip-list-after-delete",
            })?;
    let with_deleted =
        storage
            .list_media(session.id, message.id, true)
            .context(StorageValidationSnafu {
                stage: "scenario-media-roundtrip-list-with-deleted",
            })?;

    let media_roundtrip = metadata_ok
        && active_after_delete.is_empty()
        && with_deleted.len() == 1
        && with_deleted[0].deleted_at_unix_seconds.is_some();

    println!("stored_uri={stored_uri}");
    println!("media_roundtrip={media_roundtrip}");
    if !media_roundtrip {
        return ScenarioFailedSnafu {
            stage: "scenario-media-roundtrip-assert",
            scenario: "media_ref_roundtrip",
            reason: "media URI/metadata persistence or soft-delete behavior failed".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_media_blob_guard(db_path: &str) -> RunnerResult<()> {
    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-media-blob-guard-open",
        })?;

    let session = storage
        .create_session(NewSession {
            title: "media-blob-guard".to_string(),
        })
        .context(StorageValidationSnafu {
            stage: "scenario-media-blob-guard-create-session",
        })?;
    let message = storage
        .append_message(
            session.id,
            NewMessage {
                role: MessageRole::User,
                content: "blob-attempt".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-media-blob-guard-append-message",
        })?;

    let blob_attempt = storage.attach_media(
        session.id,
        message.id,
        NewMediaRef {
            uri: "data:audio/wav;base64,UklGRhQAAABXQVZF".to_string(),
            mime_type: "audio/wav".to_string(),
            size_bytes: 24,
            duration_ms: None,
            width_px: None,
            height_px: None,
            sha256_hex: None,
        },
    );
    let blob_guard = matches!(blob_attempt, Err(StorageError::Conflict { .. }));

    println!("blob_guard={blob_guard}");
    if !blob_guard {
        return ScenarioFailedSnafu {
            stage: "scenario-media-blob-guard-assert",
            scenario: "media_blob_guard",
            reason: "blob-like media URI input was not rejected".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_agent_event_roundtrip(db_path: &str) -> RunnerResult<()> {
    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-agent-event-roundtrip-open",
        })?;

    let session = storage
        .create_session(NewSession {
            title: "agent-event-roundtrip".to_string(),
        })
        .context(StorageValidationSnafu {
            stage: "scenario-agent-event-roundtrip-create-session",
        })?;
    let message = storage
        .append_message(
            session.id,
            NewMessage {
                role: MessageRole::User,
                content: "event-target".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-agent-event-roundtrip-append-message",
        })?;

    storage
        .append_agent_event(
            session.id,
            NewAgentEvent {
                message_id: None,
                event_type: "session.lifecycle".to_string(),
                payload_json: "{\"phase\":\"boot\"}".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-agent-event-roundtrip-append-session-event",
        })?;
    storage
        .append_agent_event(
            session.id,
            NewAgentEvent {
                message_id: Some(message.id),
                event_type: "message.tool".to_string(),
                payload_json: "{\"tool\":\"search\",\"ok\":true}".to_string(),
            },
        )
        .context(StorageValidationSnafu {
            stage: "scenario-agent-event-roundtrip-append-message-event",
        })?;

    let session_events =
        storage
            .list_agent_events(session.id, None)
            .context(StorageValidationSnafu {
                stage: "scenario-agent-event-roundtrip-list-session",
            })?;
    let message_events = storage
        .list_agent_events(session.id, Some(message.id))
        .context(StorageValidationSnafu {
            stage: "scenario-agent-event-roundtrip-list-message",
        })?;

    let message_filter_ok = message_events.len() == 1
        && message_events[0].message_id == Some(message.id)
        && message_events[0].event_type == "message.tool";
    let ordering_ok = session_events.len() == 2
        && session_events[0].event_type == "session.lifecycle"
        && session_events[1].event_type == "message.tool";
    let payload_ok = session_events
        .iter()
        .any(|event| event.payload_json == "{\"tool\":\"search\",\"ok\":true}");
    let agent_event_roundtrip = message_filter_ok && ordering_ok && payload_ok;

    println!("agent_event_roundtrip={agent_event_roundtrip}");
    if !agent_event_roundtrip {
        return ScenarioFailedSnafu {
            stage: "scenario-agent-event-roundtrip-assert",
            scenario: "agent_event_roundtrip",
            reason: "agent event append/list semantics or payload persistence mismatch".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_migrate_tsv_fixture(db_path: &str) -> RunnerResult<()> {
    reset_sqlite_files(db_path)?;
    let _fixture_guard = LegacyFixtureGuard::install(TASK6_VALID_TSV_FIXTURE)?;

    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-tsv-fixture-open",
        })?;
    let report = storage
        .import_legacy_conversations_from_default_path()
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-tsv-fixture-import",
        })?;

    let sessions = storage
        .list_sessions(false)
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-tsv-fixture-list",
        })?;
    let titles_in_order = sessions
        .iter()
        .map(|session| session.title.as_str())
        .collect::<Vec<_>>();
    let order_preserved = titles_in_order == vec!["Legacy New", "Legacy Mid", "Legacy Old"];

    let initial_branch_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM branches WHERE parent_branch_id IS NULL AND deleted_at IS NULL",
    )
    .fetch_one(storage.pool())
    .await
    .context(SqliteQuerySnafu {
        stage: "scenario-migrate-tsv-fixture-count-branches",
    })?;

    let active_branch_links = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sessions AS s JOIN branches AS b ON b.session_id = s.id AND b.id = s.active_branch_id WHERE s.deleted_at IS NULL AND b.deleted_at IS NULL",
    )
    .fetch_one(storage.pool())
    .await
    .context(SqliteQuerySnafu {
        stage: "scenario-migrate-tsv-fixture-active-branch-links",
    })?;

    let source_retained = legacy_fixture_path().exists();

    println!("imported_sessions={}", report.imported_sessions);
    println!("order_preserved={order_preserved}");
    println!("initial_branch_count={initial_branch_count}");
    println!("active_branch_links={active_branch_links}");
    println!("source_retained={source_retained}");

    if report.imported_sessions != 3 {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-tsv-fixture-assert-imported-count",
            scenario: "migrate_tsv_fixture",
            reason: format!(
                "expected imported_sessions=3, got {}",
                report.imported_sessions
            ),
        }
        .fail();
    }

    if report.skipped_rows != 0 {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-tsv-fixture-assert-no-skips",
            scenario: "migrate_tsv_fixture",
            reason: format!(
                "expected skipped_rows=0 for clean fixture, got {}",
                report.skipped_rows
            ),
        }
        .fail();
    }

    if !order_preserved {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-tsv-fixture-assert-order",
            scenario: "migrate_tsv_fixture",
            reason: "sqlite listing order does not match legacy updated_at ordering".to_string(),
        }
        .fail();
    }

    if initial_branch_count != 3 || active_branch_links != 3 {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-tsv-fixture-assert-branch-setup",
            scenario: "migrate_tsv_fixture",
            reason: format!(
                "expected three initial branches/links, got initial_branch_count={initial_branch_count}, active_branch_links={active_branch_links}"
            ),
        }
        .fail();
    }

    if !source_retained {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-tsv-fixture-assert-source-retained",
            scenario: "migrate_tsv_fixture",
            reason: "legacy TSV source disappeared after import".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_migrate_idempotent(db_path: &str) -> RunnerResult<()> {
    reset_sqlite_files(db_path)?;
    let _fixture_guard = LegacyFixtureGuard::install(TASK6_VALID_TSV_FIXTURE)?;

    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-idempotent-open",
        })?;
    let first_import = storage
        .import_legacy_conversations_from_default_path()
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-idempotent-import-first",
        })?;
    let second_import = storage
        .import_legacy_conversations_from_default_path()
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-idempotent-import-second",
        })?;

    let sessions = storage
        .list_sessions(false)
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-idempotent-list",
        })?;
    let branch_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM branches WHERE deleted_at IS NULL")
            .fetch_one(storage.pool())
            .await
            .context(SqliteQuerySnafu {
                stage: "scenario-migrate-idempotent-count-branches",
            })?;

    let idempotent = first_import.imported_sessions == 3
        && second_import.imported_sessions == 0
        && second_import.already_migrated
        && sessions.len() == 3
        && branch_count == 3;

    println!("first_imported_sessions={}", first_import.imported_sessions);
    println!(
        "second_imported_sessions={}",
        second_import.imported_sessions
    );
    println!("already_migrated={}", second_import.already_migrated);
    println!("idempotent={idempotent}");

    if !idempotent {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-idempotent-assert",
            scenario: "migrate_idempotent",
            reason: format!(
                "expected idempotent import behavior but got first_imported_sessions={}, second_imported_sessions={}, already_migrated={}, session_count={}, branch_count={branch_count}",
                first_import.imported_sessions,
                second_import.imported_sessions,
                second_import.already_migrated,
                sessions.len(),
            ),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

async fn run_migrate_malformed_row(db_path: &str) -> RunnerResult<()> {
    reset_sqlite_files(db_path)?;
    let _fixture_guard = LegacyFixtureGuard::install(TASK6_MALFORMED_TSV_FIXTURE)?;

    let storage = SqliteStorage::open(db_path)
        .await
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-malformed-row-open",
        })?;
    let report = storage
        .import_legacy_conversations_from_default_path()
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-malformed-row-import",
        })?;

    let sessions = storage
        .list_sessions(false)
        .context(StorageValidationSnafu {
            stage: "scenario-migrate-malformed-row-list",
        })?;

    let malformed_row_signal = report.skipped_rows == 3 && !report.warnings.is_empty();
    let warning_lines = report
        .warnings
        .iter()
        .map(|warning| warning.line_number.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let default_title_applied = sessions
        .iter()
        .any(|session| session.title == DEFAULT_SESSION_TITLE);

    println!("imported_sessions={}", report.imported_sessions);
    println!("malformed_rows_skipped={}", report.skipped_rows);
    println!("malformed_row_signal={malformed_row_signal}");
    println!("warning_lines={warning_lines}");
    println!("default_title_applied={default_title_applied}");

    if report.imported_sessions != 2 {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-malformed-row-assert-imported-count",
            scenario: "migrate_malformed_row",
            reason: format!(
                "expected imported_sessions=2 after skipping malformed rows, got {}",
                report.imported_sessions
            ),
        }
        .fail();
    }

    if !malformed_row_signal {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-malformed-row-assert-signal",
            scenario: "migrate_malformed_row",
            reason: format!(
                "expected malformed row skip signal with skipped_rows=3, got skipped_rows={} warning_lines={warning_lines}",
                report.skipped_rows
            ),
        }
        .fail();
    }

    if !default_title_applied {
        return ScenarioFailedSnafu {
            stage: "scenario-migrate-malformed-row-assert-default-title",
            scenario: "migrate_malformed_row",
            reason: "expected empty legacy title to map to default session title".to_string(),
        }
        .fail();
    }

    println!("runner_ok=true");
    Ok(())
}

const TASK6_VALID_TSV_FIXTURE: &str =
    "10\t1700000100\tLegacy Old\n11\t1700000200\tLegacy Mid\n12\t1700000300\tLegacy New\n";

const TASK6_MALFORMED_TSV_FIXTURE: &str = "21\t1700001000\tValid One\nnot-a-number\t1700002000\tBroken Id\n22\tnot-a-timestamp\tBroken Timestamp\n23\t1700003000\n24\t1700004000\t   \n";

#[derive(Debug)]
struct LegacyFixtureGuard {
    path: PathBuf,
    previous_contents: Option<String>,
    existed_before: bool,
}

impl LegacyFixtureGuard {
    fn install(contents: &str) -> RunnerResult<Self> {
        let path = legacy_fixture_path();
        let previous_contents = match std::fs::read_to_string(&path) {
            Ok(existing) => Some(existing),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(RunnerError::FileIo {
                    stage: "scenario-task6-read-existing-fixture",
                    path: path.display().to_string(),
                    source,
                });
            }
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).context(FileIoSnafu {
                stage: "scenario-task6-create-fixture-directory",
                path: parent.display().to_string(),
            })?;
        }
        std::fs::write(&path, contents).context(FileIoSnafu {
            stage: "scenario-task6-write-fixture",
            path: path.display().to_string(),
        })?;

        let existed_before = previous_contents.is_some();
        Ok(Self {
            path,
            previous_contents,
            existed_before,
        })
    }
}

impl Drop for LegacyFixtureGuard {
    fn drop(&mut self) {
        // Restore the workspace fixture state so QA scenarios stay deterministic across reruns.
        if self.existed_before {
            if let Some(previous) = &self.previous_contents {
                let _ = std::fs::write(&self.path, previous);
            }
        } else {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn legacy_fixture_path() -> PathBuf {
    PathBuf::from(LEGACY_CONVERSATIONS_TSV_RELATIVE_PATH)
}

fn reset_sqlite_files(db_path: &str) -> RunnerResult<()> {
    remove_file_if_exists(Path::new(db_path), "scenario-reset-sqlite-db")?;
    let wal_path = format!("{db_path}-wal");
    remove_file_if_exists(Path::new(&wal_path), "scenario-reset-sqlite-wal")?;
    let shm_path = format!("{db_path}-shm");
    remove_file_if_exists(Path::new(&shm_path), "scenario-reset-sqlite-shm")?;
    Ok(())
}

fn remove_file_if_exists(path: &Path, stage: &'static str) -> RunnerResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RunnerError::FileIo {
            stage,
            path: path.display().to_string(),
            source,
        }),
    }
}

fn assert_id_roundtrip<T>(label: &'static str, id: T) -> RunnerResult<()>
where
    T: Copy + Eq + FromStr<Err = StorageError> + std::fmt::Display,
{
    let encoded = id.to_string();
    let decoded = encoded.parse::<T>().context(StorageValidationSnafu {
        stage: "scenario-id-roundtrip-parse",
    })?;

    if decoded != id {
        return ScenarioFailedSnafu {
            stage: "scenario-id-roundtrip-compare",
            scenario: "id_roundtrip",
            reason: format!("{label} parse/format roundtrip mismatch"),
        }
        .fail();
    }

    println!("{label}_roundtrip=true");
    Ok(())
}

fn is_session_list_ordered(sessions: &[zova_storage::SessionRecord]) -> bool {
    sessions.windows(2).all(|pair| {
        let left = &pair[0];
        let right = &pair[1];

        if left.updated_at_unix_seconds != right.updated_at_unix_seconds {
            return left.updated_at_unix_seconds > right.updated_at_unix_seconds;
        }

        // UUIDv7 IDs are textual in sqlite ordering; compare the serialized forms to mirror SQL.
        left.id.to_string().cmp(&right.id.to_string()) == Ordering::Greater
    })
}

fn invalid_input_is_rejected<T>(raw: &str) -> bool
where
    T: FromStr<Err = StorageError>,
{
    matches!(raw.parse::<T>(), Err(StorageError::InvalidId { .. }))
}

fn require_db_path<'a>(args: &'a RunnerArgs, scenario: &'static str) -> RunnerResult<&'a str> {
    args.db_path.as_deref().context(MissingDbPathSnafu {
        stage: "require-db-path",
        scenario,
    })
}

fn is_foreign_key_violation(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => {
            if let Some(code) = database_error.code()
                && code == "787"
            {
                return true;
            }

            database_error
                .message()
                .contains("FOREIGN KEY constraint failed")
        }
        _ => false,
    }
}
