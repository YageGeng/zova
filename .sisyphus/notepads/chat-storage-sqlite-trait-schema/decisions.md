# Decisions

- 2026-02-12: Defined storage contracts as trait boundaries (`SessionStore`, `MessageStore`, `MediaStore`, `AgentEventStore`) plus a blanket `Storage` facade trait so downstream implementations can compose all capabilities behind one type.
- 2026-02-12: Modeled core storage operation payloads/records in `storage/types.rs` (including `HistoryForkOutcome.message_id_remaps`) to lock API shape before SQLite implementation tasks.
- 2026-02-12: Added `uuid` as a workspace dependency with `v7` feature and wired it into `zova` crate to keep dependency declaration consistent with current workspace style.
- 2026-02-12: Implemented `storage_qa_runner` argument handling with optional `--db` passthrough (currently no-op) so future schema scenarios can accept DB path without breaking Task 1 scaffold compatibility.

## 2026-02-12 - Task 7 handoff migration map (from Task 3 seam prep)

### Decision 1: Keep ownership split by UI role, swap only persistence boundary
- `ChatSidebar` remains owner of session list/create/select UI flow (`crates/chat-app/src/chat/sidebar.rs:58`).
- `ChatView` remains owner of message lifecycle/stream orchestration (`crates/chat-app/src/chat/view.rs:60`).
- Only persistence seam changes in Task 7: replace `ConversationStore` usage with storage facade calls.

### Decision 2: Execute migration in deterministic order to avoid partial hydration bugs
1. Replace sidebar session list/load/create seams first (`crates/chat-app/src/chat/sidebar.rs:63`, `crates/chat-app/src/chat/sidebar.rs:112`, `crates/chat-app/src/chat/sidebar.rs:127`, `crates/chat-app/src/chat/sidebar.rs:143`).
2. Replace activation hydration seam second so `activate_conversation` loads message history, not title-only placeholder (`crates/chat-app/src/chat/view.rs:356`, `crates/chat-app/src/chat/view.rs:725`, `crates/chat-app/src/chat/view.rs:737`).
3. Replace submit insert seams third (`crates/chat-app/src/chat/view.rs:405`, `crates/chat-app/src/chat/view.rs:412`, plus error-side insert `crates/chat-app/src/chat/view.rs:773`).
4. Replace stream mutation/finalize seams last (`crates/chat-app/src/chat/view.rs:583`, `crates/chat-app/src/chat/view.rs:675`) and ensure terminal call paths still converge through `finalize_stream` (`crates/chat-app/src/chat/view.rs:602`, `crates/chat-app/src/chat/view.rs:624`, `crates/chat-app/src/chat/view.rs:642`).

### Decision 3: Enforce scoped storage contract at every message mutation seam
- Required contract shape for Task 7 integrations:
  - insert calls must include `session_id` and generated `msg_id`
  - update/load/delete calls must require both `session_id + msg_id`
- Mandatory scoped replacement points:
  - chunk updates: `crates/chat-app/src/chat/view.rs:583`
  - finalize status updates: `crates/chat-app/src/chat/view.rs:675`
  - submit inserts: `crates/chat-app/src/chat/view.rs:405`, `crates/chat-app/src/chat/view.rs:412`
  - error-side assistant insert: `crates/chat-app/src/chat/view.rs:773`

### Decision 4: Keep stream-state semantics unchanged while integrating storage
- Stream transition guardrails in `ChatView` remain source of truth (`crates/chat-app/src/chat/view.rs:398`, `crates/chat-app/src/chat/view.rs:650`).
- Storage writes in Task 7 must be inserted around existing transitions, not reorder transition checks.
- `StreamTarget.session_id` (`crates/chat-app/src/chat/message.rs:40`) is not a storage session key; storage scoping remains conversation/session id plus message id.

### Decision 5: Legacy module boundary remains for compatibility until full cutover
- Existing export boundary: `crates/chat-app/src/database/mod.rs:3`.
- Existing legacy implementation: `crates/chat-app/src/database/conversation.rs:41`.
- Task 7 should stop direct chat-flow dependence on this boundary; keep it available only for migration/bootstrap paths until removal is explicitly planned.

## 2026-02-12 - Task 2 sqlite schema/bootstrap decisions

- Added workspace-scoped `sqlx` dependency (`runtime-tokio-rustls`, `sqlite`, `migrate`, `macros`) and consumed it through `zova` crate to keep dependency wiring centralized.
- Chose a single initial migration (`crates/chat-app/migrations/202602120001_init_chat_storage.sql`) that creates `sessions`, `branches`, `messages`, `media_refs`, and `agent_events` with UUID-text keys and soft-delete columns on mutable domain tables.
- Kept `sessions.active_branch_id` as an indexed pointer column without FK to avoid circular-FK bootstrap complexity in v1; enforced stricter branch/session consistency through composite FK on `messages`.
- Implemented `SqliteStorage::open` as async bootstrap that creates parent directories, applies PRAGMAs (`WAL`, `foreign_keys=ON`, `busy_timeout=5000`), and runs embedded migrations via `sqlx::migrate!("./migrations")`.

## 2026-02-13 - Task 4 branch operation decisions

- Implemented SessionStore + MessageStore directly in `storage/sqlite/mod.rs` using typed row mappers and shared conversion helpers (`role`/timestamp/id parsing) to keep sqlite boundary explicit and SNAFU-mapped.
- Chose dedicated per-call sqlite connections for store methods (while retaining pooled connection for bootstrap/runner checks) to preserve existing sync trait contracts without introducing async trait changes.
- Implemented `fork_from_history` as one transaction that creates the new branch, copies prefix rows with source replacement, swaps `sessions.active_branch_id`, and soft-deletes the previous branch before commit.

## 2026-02-13 - Task 5 implementation decisions

- Implemented `MediaStore` and `AgentEventStore` in `storage/sqlite/mod.rs` using the same dedicated per-call sqlite connection pattern as Task 4 to avoid sync/async runtime contention regressions.
- Kept `agent_events.payload_json` as validated TEXT JSON via `SELECT json_valid(?)` and mapped invalid payloads to typed `StorageError::Conflict` with stage markers for consistent SNAFU error surfacing.
- Added QA scenarios `media_ref_roundtrip`, `media_blob_guard`, and `agent_event_roundtrip` to `storage_qa_runner` with deterministic key=value outputs to preserve CI-friendly assertions.

## 2026-02-13 - Task 6 migration decisions

- Added explicit importer APIs on `SqliteStorage` (`import_legacy_conversations_from_default_path` / `import_legacy_conversations_from_path`) instead of auto-running during `open`, so existing non-migration scenarios remain isolated.
- Kept malformed legacy rows as non-fatal warnings (`line_number` + reason code) and continued importing valid rows in the same run.
- QA scenarios now stage `.zova/conversations.tsv` fixtures through a restore guard so task checks validate real default-path migration behavior without leaving workspace residue.
