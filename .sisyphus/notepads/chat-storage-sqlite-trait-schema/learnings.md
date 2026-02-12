# Learnings

- 2026-02-12: Storage-side identifiers can be introduced as UUIDv7 wrappers (`SessionId`, `MessageId`, `BranchId`, `MediaRefId`, `AgentEventId`) without forcing immediate migration of existing UI/chat `u64` IDs.
- 2026-02-12: Using `Uuid::parse_str(...).context(InvalidIdSnafu { ... })` provides clean SNAFU-based invalid-ID mapping while preserving the concrete `uuid::Error` source for diagnostics.
- 2026-02-12: A strict key=value output contract in `storage_qa_runner` makes scenarios CI-friendly (`id_roundtrip`, `id_invalid`, `prep_noop`) and easy to assert with simple text matching.

## 2026-02-12 - Task 3 read-only seam inventory

### Scan coverage
- Plan reference reviewed: `.sisyphus/plans/chat-storage-sqlite-trait-schema.md:325`
- Files enumerated and mapped: `crates/chat-app/src/chat/sidebar.rs`, `crates/chat-app/src/chat/view.rs`, `crates/chat-app/src/database/mod.rs`, `crates/chat-app/src/database/conversation.rs`
- Supporting callsite validation: `crates/chat-app/src/app.rs:127` (top-level new-chat trigger)

### Tool validation snapshots
- AST-grep `'$X.create_conversation($$$)'` hits (5 total): `crates/chat-app/src/app.rs:127`, `crates/chat-app/src/chat/sidebar.rs:110`, `crates/chat-app/src/chat/sidebar.rs:233`, `crates/chat-app/src/chat/view.rs:81`, `crates/chat-app/src/chat/view.rs:199`
- LSP references confirm `ConversationStore` session methods are only consumed in sidebar seams:
  - `ConversationStore::create_conversation` definition `crates/chat-app/src/database/conversation.rs:50` -> use `crates/chat-app/src/chat/sidebar.rs:112`
  - `ConversationStore::list_conversations` definition `crates/chat-app/src/database/conversation.rs:78` -> uses `crates/chat-app/src/chat/sidebar.rs:63`, `crates/chat-app/src/chat/sidebar.rs:143`
  - `ConversationStore::load_conversation` definition `crates/chat-app/src/database/conversation.rs:95` -> use `crates/chat-app/src/chat/sidebar.rs:127`

### Seam checklist (Task 7 replacement map)

1) Sidebar session list seam
- Current boundary: `ChatSidebar::new` loads persisted sessions via `ConversationStore::list_conversations`.
- Function anchor: `crates/chat-app/src/chat/sidebar.rs:59`
- Direct persistence call: `crates/chat-app/src/chat/sidebar.rs:63`
- Replacement seam for Task 7: replace constructor-time session list fetch with storage facade session listing.

2) Sidebar session refresh seam
- Current boundary: `ChatSidebar::refresh_from_store` re-reads session list and reconciles selected id.
- Function anchor: `crates/chat-app/src/chat/sidebar.rs:142`
- Direct persistence call: `crates/chat-app/src/chat/sidebar.rs:143`
- Replacement seam for Task 7: replace with storage facade session listing (same ordering semantics).

3) Sidebar session create seam
- Current boundary: `ChatSidebar::create_conversation` persists new conversation and selects it.
- Function anchor: `crates/chat-app/src/chat/sidebar.rs:109`
- Direct persistence call: `crates/chat-app/src/chat/sidebar.rs:112`
- Upstream triggers validated by LSP/AST:
  - Sidebar toolbar button: `crates/chat-app/src/chat/sidebar.rs:233`
  - ChatView bootstrap path: `crates/chat-app/src/chat/view.rs:81`
  - ChatView delegated new-chat action: `crates/chat-app/src/chat/view.rs:199`
  - App shell new-chat action: `crates/chat-app/src/app.rs:127`
- Replacement seam for Task 7: swap create path to storage facade session creation without changing event flow (`select_conversation`).

4) Sidebar session metadata load seam
- Current boundary: `ChatSidebar::load_conversation` loads one record by id.
- Function anchor: `crates/chat-app/src/chat/sidebar.rs:126`
- Direct persistence call: `crates/chat-app/src/chat/sidebar.rs:127`
- Downstream consumers:
  - Initial fallback title during first conversation bootstrap: `crates/chat-app/src/chat/view.rs:83`
  - Activation-time lazy title fill in `ensure_conversation_exists`: `crates/chat-app/src/chat/view.rs:737`
- Replacement seam for Task 7: replace with storage facade session lookup.

5) Chat activation history-load seam (currently metadata-only)
- Current boundary: `ChatView::activate_conversation` delegates to `ensure_conversation_exists`, then renders in-memory conversation messages.
- Function anchors: `crates/chat-app/src/chat/view.rs:356`, `crates/chat-app/src/chat/view.rs:725`
- Current behavior gap:
  - `ensure_conversation_exists` only reads title from sidebar store (`crates/chat-app/src/chat/view.rs:737`)
  - It creates `Conversation::new` with empty `messages` (`crates/chat-app/src/chat/view.rs:742`)
- Replacement seam for Task 7: hydrate full message history on activation through storage facade before `sync_active_conversation_messages` (`crates/chat-app/src/chat/view.rs:365`).

6) Submit-time persistence insertion seams
- Current in-memory-only write points inside `ChatView::handle_submit` (`crates/chat-app/src/chat/view.rs:369`):
  - user message append: `crates/chat-app/src/chat/view.rs:405`
  - assistant streaming placeholder append: `crates/chat-app/src/chat/view.rs:412`
- Replacement seam for Task 7: add storage inserts at both points (or one transactional helper wrapping both) while preserving stream transition ordering.

7) Stream finalize/update persistence seams
- Current in-memory-only mutation points:
  - assistant content chunk append in `flush_pending_stream_chunk`: `crates/chat-app/src/chat/view.rs:557`, mutation at `crates/chat-app/src/chat/view.rs:583`
  - assistant final status update in `finalize_stream`: `crates/chat-app/src/chat/view.rs:650`, mutation at `crates/chat-app/src/chat/view.rs:675`
- Terminal callers for `finalize_stream` (all must inherit persistence semantics):
  - done path: `crates/chat-app/src/chat/view.rs:602`
  - error path: `crates/chat-app/src/chat/view.rs:624`
  - cancel path: `crates/chat-app/src/chat/view.rs:642`
- Replacement seam for Task 7: route status/content persistence via storage facade with strict scoped predicates.

8) Submit error-side insertion seam
- Current in-memory-only assistant error insertion in `push_provider_not_configured_error`: `crates/chat-app/src/chat/view.rs:755`, append at `crates/chat-app/src/chat/view.rs:773`
- Entry trigger from submit path: `crates/chat-app/src/chat/view.rs:384`
- Replacement seam for Task 7: persist assistant error message using the same message-insert API as normal submit flow.

9) Module export seam for old persistence boundary
- Current export boundary re-exports `ConversationStore`: `crates/chat-app/src/database/mod.rs:3`
- Existing implementation lives in `crates/chat-app/src/database/conversation.rs:41`
- Replacement seam for Task 7: consume storage facade as primary boundary in chat flow, while legacy module can remain only for migration/import compatibility.

### Required `session_id + msg_id` scoping enforcement points for Task 7
- Scope all message updates by `(session_id, msg_id)` at:
  - streaming chunk updates (`flush_pending_stream_chunk`): `crates/chat-app/src/chat/view.rs:583`
  - finalize status updates (`finalize_stream`): `crates/chat-app/src/chat/view.rs:675`
- Scope all message inserts by explicit session id at:
  - submit user insert (`crates/chat-app/src/chat/view.rs:405`)
  - submit assistant placeholder insert (`crates/chat-app/src/chat/view.rs:412`)
  - provider-not-configured assistant error insert (`crates/chat-app/src/chat/view.rs:773`)
- Guardrail note: `StreamTarget.session_id` (`crates/chat-app/src/chat/message.rs:40`) is stream-run identity, not persistent storage session identity. Task 7 must map storage scope to conversation/session id (`StreamTarget.conversation_id`, `crates/chat-app/src/chat/message.rs:39`) plus persistent `msg_id`.

## 2026-02-12 - Task 2 sqlite schema/bootstrap

- `sqlx` v0.8 works for compile-time embedded migrations when `migrate` + `macros` features are enabled and `sqlx::migrate!("./migrations")` is used from crate code.
- Applying PRAGMAs both in `SqliteConnectOptions` and explicit bootstrap queries keeps runtime behavior deterministic for QA assertions (`journal_mode`, `foreign_keys`, `busy_timeout`).
- Composite foreign keys (`messages(session_id, branch_id) -> branches(session_id, id)`) are a low-cost way to enforce cross-session isolation at schema level before store business logic exists.
- `storage_qa_runner` remains CI-friendly when every scenario emits machine-parsable key=value lines and fails fast on invariant mismatches.

## 2026-02-13 - Task 4 session/message branch operations

- Keeping `SessionStore`/`MessageStore` trait methods sync while using `sqlx` async queries is reliable when each operation opens a dedicated sqlite connection on a worker-thread runtime; sharing one `SqlitePool` across ad-hoc runtimes caused connection wait timeouts.
- Enforcing message updates with strict `WHERE session_id = ? AND id = ?` predicates naturally blocks cross-session mutation attempts and maps cleanly to `StorageError::NotFound` for guard scenarios.
- Branch fork remap determinism is straightforward when prefix copy uses `ORDER BY seq ASC, id ASC` and remaps are emitted in that exact insert order.

## 2026-02-13 - Task 5 media and agent event stores

- Implementing `MediaStore` with pre-check `messages(session_id,id)` scope guards plus ordered `ORDER BY created_at ASC, id ASC` reads keeps media operations deterministic and cross-session safe.
- Rejecting blob-like media references (`data:` / `;base64,`) at storage API boundary enforces URI/path-only persistence without adding blob columns or schema changes.
- Implementing `AgentEventStore` with optional `message_id` filtering and stable ordering (`created_at`, `id`) supports both session-level and message-level event timelines.

## 2026-02-13 - Task 6 TSV import pipeline

- Parsing legacy lines with `splitn(3, '\t')` plus legacy escape decoding keeps TSV compatibility while still allowing malformed-row skipping.
- Sorting parsed rows by `updated_at DESC, legacy_id DESC` before sqlite insert keeps imported session ordering compatible with the previous TSV list behavior.
- A pre-import `SELECT COUNT(*) FROM sessions` guard plus single-transaction inserts makes migration re-runs idempotent and branch/session invariants stable.

## 2026-02-13 - Task 6 verification-only rerun

- Re-ran required Task 6 scenarios against `.sisyphus/evidence/task-6.db`; `migrate_tsv_fixture`, `migrate_idempotent`, and `migrate_malformed_row` all passed without importer code changes.
- Verified migration behavior remains aligned with legacy ordering semantics and row policy: order preserved, one initial branch per imported session, and malformed rows skipped with structured line-number warnings.

## 2026-02-13 - Task 7 sidebar/view storage facade integration

- Wiring `ChatSidebar` to `SqliteStorage` with runtime bootstrap plus one-time legacy TSV import keeps legacy compatibility while replacing direct `ConversationStore` read/write callsites.
- Maintaining an in-process `ConversationId <-> SessionId` bridge in `ChatSidebar` allows chat flow/event contracts to stay on existing `u64` IDs while storage stays UUIDv7-native.
- Hydrating `ChatView` messages from `storage.list_messages(session_id)` on activation and tracking `MessageId(u64) <-> storage::MessageId` mappings enables stream chunk/finalize updates to call scoped `update_message(session_id, msg_id, ...)` without changing stream state-machine ordering.
- Persisting submit-time user + assistant placeholder inserts through sidebar storage methods immediately after transition acceptance keeps the previous stream lifecycle semantics intact while guaranteeing storage-side history writes.

## 2026-02-13 - Task 7 verification continuation

- Full `storage_qa_runner --scenario all` on a reused `.sisyphus/evidence/final-storage.db` can fail `session_crud` due to pre-existing rows; resetting the DB/WAL/SHM files before the final sweep restores deterministic scenario expectations.
- Post-integration seam checks confirm no `ConversationStore` references remain in `sidebar.rs`/`view.rs`, and message mutation persistence remains routed through scoped storage updates.

## 2026-02-13 - Final DoD and checklist closure

- Final verification rerun is stable when `final-storage.db` plus `-wal`/`-shm` are removed before `--scenario all`; regenerated evidence now covers every mandatory task evidence path in the plan.
- Using deterministic per-scenario evidence outputs (`task-1` through `task-7`) provides enough machine-verifiable support to close both DoD and final checklist items without source changes.
