# Zova Chat SQLite Storage Trait + Schema Plan

## TL;DR

> **Quick Summary**: Introduce a SQLite + `sqlx` storage layer behind a unified storage trait facade, supporting session CRUD, history load/edit via `session_id + msg_id`, latest-branch-only semantics, and future agent extensibility.
>
> **Deliverables**:
> - `storage` trait contracts + UUIDv7 ID model
> - SQLite schema + migrations (`sessions`, `branches`, `messages`, `media_refs`, `agent_events`)
> - `SqliteStorage` implementation with soft-delete, fork-from-history, and migration from legacy TSV session metadata
> - Agent-executable QA runner and evidence outputs
>
> **Estimated Effort**: Medium
> **Parallel Execution**: YES - 4 waves
> **Critical Path**: Task 1 -> Task 2 -> Task 4 -> Task 6 -> Task 7

---

## Context

### Original Request
Design a SQLite-based storage solution for the chat app with a storage trait abstraction. Support history loading and history modification using `session_id + msg_id`, full session CRUD, future agent-mode extensibility, `sqlx` integration, and media path storage (not DB blobs).

### Interview Summary
**Key Discussions**:
- User confirmed global ID direction and selected `UUIDv7`.
- "Modify history" means fork from a historical point, then continue on the forked path.
- User wants only the latest branch visible, and old branches logically deleted.
- Session/message deletion should be soft-delete + optional garbage collection.
- Media inputs (image/audio) should be copied into app-managed storage; DB stores URI/path + metadata.
- v1 does not require mandatory encryption-at-rest, but design should keep extension points.
- User selected no automated unit/integration test suite for this work; verification will rely on agent-executed QA scenarios.

**Research Findings**:
- Current persistence is split: session metadata is TSV-backed (`crates/chat-app/src/database/conversation.rs`), while messages are in-memory (`crates/chat-app/src/chat/view.rs`).
- Existing IDs are `u64` newtypes in `crates/chat-app/src/chat/message.rs`; migration is required for UUIDv7.
- `sqlx` is not yet present in `crates/chat-app/Cargo.toml`.
- Existing session ordering behavior is explicit (`updated_at DESC`, then id tie-break) and should be preserved unless intentionally changed.

### Metis Review
**Identified Gaps (addressed in this plan)**:
- Gap: Branch-model ambiguity (revision-chain vs branch rows).
  - Resolution: Oracle-guided decision to use copy-on-write branch rows + `active_branch_id` pointer for low read complexity.
- Gap: Missing explicit invariants.
  - Resolution: Added branch/session/message invariants and transaction rules under guardrails and task acceptance criteria.
- Gap: Missing deterministic QA approach without manual verification.
  - Resolution: Added `storage_qa_runner` scenario-based verification with concrete commands and evidence paths.

---

## Work Objectives

### Core Objective
Replace ad-hoc TSV/in-memory persistence with a trait-driven SQLite storage architecture that is safe for branch-edit semantics, supports session/message operations by `session_id + msg_id`, and remains extensible for future agent workflows.

### Concrete Deliverables
- New storage module and trait contracts under `crates/chat-app/src/storage/`.
- SQL migrations under `crates/chat-app/migrations/`.
- SQLite implementation (`SqliteStorage`) using `sqlx` and SNAFU error handling.
- Legacy session metadata importer from `.zova/conversations.tsv`.
- Integration in chat flow (`ChatSidebar` + `ChatView`) to read/write through storage facade.
- QA runner binary and evidence artifacts in `.sisyphus/evidence/`.

### Definition of Done
- [x] `cargo check -p zova` passes.
- [x] `cargo clippy -p zova --all-targets -- -D warnings` passes.
- [x] `cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/final-storage.db" --scenario all` exits 0.
- [x] Evidence files generated for every mandatory scenario.

### Must Have
- `storage` abstraction is the only persistence boundary used by chat/session flows.
- Message-level operations require `session_id + msg_id` at API level.
- Branch policy implemented as latest-branch-visible + old-branch-logically-deleted.
- Media stored outside DB; DB stores URI/path + metadata only.
- Rust storage errors follow SNAFU patterns.
- v1 garbage collection policy is manual-trigger only (no background auto-prune scheduler).
- v1 write concurrency assumption is single-process local app usage (with SQLite busy-timeout safeguards).

### Must NOT Have (Guardrails)
- No media BLOB columns in SQLite.
- No branch-switching UI in v1.
- No manual/human verification criteria.
- No encryption key-management rollout in v1.
- No unrelated stream-orchestration redesign in `crates/chat-app/src/chat/message.rs` state machine.

---

## Verification Strategy (MANDATORY)

> **UNIVERSAL RULE: ZERO HUMAN INTERVENTION**
>
> Every acceptance criterion must be agent-executable (command/tool driven). No manual clicking, visual-only checks, or user confirmation steps are allowed.

### Test Decision
- **Infrastructure exists**: PARTIAL (Rust `cargo test` baseline exists; sqlite/sqlx infra does not yet exist).
- **Automated tests**: None (user-selected).
- **Framework**: N/A for this scope (verification by scenario runner + compile/lint gates).

### Agent-Executed QA Scenarios (MANDATORY)

All tasks include executable scenarios via Bash commands and deterministic output assertions.

Scenario format used throughout this plan:

```text
Scenario: <name>
  Tool: Bash
  Preconditions: <state>
  Steps:
    1. <exact command>
    2. <exact assertion target in stdout/stderr/exit code>
  Expected Result: <concrete condition>
  Failure Indicators: <concrete failure signal>
  Evidence: <path>
```

Evidence folder convention:
- `.sisyphus/evidence/task-{N}-{scenario}.txt`
- `.sisyphus/evidence/task-{N}-{scenario}.json`

---

## Execution Strategy

### Parallel Execution Waves

```text
Wave 1 (Start Immediately)
├── Task 1: Storage trait contracts + UUIDv7 IDs + QA runner scaffold
└── Task 3: App callsite inventory + integration seam mapping (read-only prep)

Wave 2 (After Wave 1)
├── Task 2: SQLite schema + migrations + bootstrap pragmas
└── Task 5: Media/agent extension table and trait stubs (depends on Task 1 contracts)

Wave 3 (After Wave 2)
├── Task 4: SessionStore + MessageStore branch operations
└── Task 6: Legacy TSV import pipeline

Wave 4 (After Wave 3)
└── Task 7: ChatSidebar/ChatView integration + full QA sweep + hardening

Critical Path: 1 -> 2 -> 4 -> 6 -> 7
Parallel Speedup: ~30-35% vs strictly sequential
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|----------------------|
| 1 | None | 2, 4, 5 | 3 |
| 2 | 1 | 4, 6 | 5 |
| 3 | None | 7 | 1 |
| 4 | 1, 2 | 7 | 6 |
| 5 | 1, 2 | 7 | 2 |
| 6 | 2 | 7 | 4 |
| 7 | 3, 4, 5, 6 | None | None |

### Agent Dispatch Summary

| Wave | Tasks | Recommended Agents |
|------|-------|--------------------|
| 1 | 1, 3 | `task(category="unspecified-high", load_skills=["rust-error-snafu","coding-guidelines"], run_in_background=false)` |
| 2 | 2, 5 | `task(category="unspecified-high", load_skills=["rust-error-snafu","coding-guidelines"], run_in_background=false)` |
| 3 | 4, 6 | `task(category="deep", load_skills=["rust-error-snafu","coding-guidelines"], run_in_background=false)` |
| 4 | 7 | `task(category="unspecified-high", load_skills=["rust-error-snafu","coding-guidelines","git-master"], run_in_background=false)` |

---

## TODOs

- [x] 1. Define storage contracts, UUIDv7 IDs, and QA runner scaffold

  **What to do**:
  - Create `crates/chat-app/src/storage/mod.rs` and split contracts:
    - `SessionStore`
    - `MessageStore`
    - `MediaStore`
    - `AgentEventStore`
    - unified facade trait `Storage`
  - Introduce UUIDv7-backed ID wrappers (`SessionId`, `MessageId`, `BranchId`, `MediaRefId`, `AgentEventId`).
  - Add SNAFU-based storage error model (`StorageError`) with domain variants (not found, conflict, invalid id, invariant violation).
  - Create `crates/chat-app/src/bin/storage_qa_runner.rs` scaffold with scenario dispatch and machine-readable output.

  **Must NOT do**:
  - No SQL schema implementation in this task.
  - No UI behavior changes.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: foundational cross-module API contract work with compatibility impact.
  - **Skills**: `rust-error-snafu`, `coding-guidelines`
    - `rust-error-snafu`: required for consistent typed error scaffolding.
    - `coding-guidelines`: keeps naming/typing conventions aligned with existing Rust code.
  - **Skills Evaluated but Omitted**:
    - `playwright`: desktop GPUI storage work has no browser workflow.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Task 3)
  - **Blocks**: 2, 4, 5
  - **Blocked By**: None

  **References**:
  - `crates/chat-app/src/chat/message.rs` - existing domain ID newtype style to mirror and safely migrate.
  - `crates/chat-app/src/database/conversation.rs` - current persistence boundary and SNAFU error style baseline.
  - `crates/chat-app/src/settings/state.rs` - additional SNAFU + persistence pattern examples.
  - `crates/chat-app/src/lib.rs` - module export wiring pattern.
  - Official docs: `https://docs.rs/uuid/latest/uuid/` - UUIDv7 type handling and parsing behavior.

  **Acceptance Criteria**:
  - [x] `cargo check -p zova` passes after introducing new storage module contracts.
  - [x] `cargo run -p zova --bin storage_qa_runner -- --scenario id_roundtrip` prints `id_roundtrip=true`.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: UUIDv7 roundtrip works for all ID wrappers
    Tool: Bash
    Preconditions: Task 1 contract files compile
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --scenario id_roundtrip
      2. Assert: stdout contains "id_roundtrip=true"
      3. Save stdout to: .sisyphus/evidence/task-1-id-roundtrip.txt
    Expected Result: All ID wrapper parse/serialize checks pass
    Failure Indicators: non-zero exit code OR missing "id_roundtrip=true"
    Evidence: .sisyphus/evidence/task-1-id-roundtrip.txt

  Scenario: Invalid UUID input is rejected with domain error
    Tool: Bash
    Preconditions: Task 1 error model implemented
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --scenario id_invalid
      2. Assert: stdout contains "invalid_id_error=true"
      3. Save stdout to: .sisyphus/evidence/task-1-id-invalid.txt
    Expected Result: Invalid input is mapped to StorageError variant
    Failure Indicators: parse succeeds unexpectedly OR process exits 0 without expected flag
    Evidence: .sisyphus/evidence/task-1-id-invalid.txt
  ```

  **Commit**: YES
  - Message: `feat(storage): introduce storage contracts and UUIDv7 ids`
  - Files: `crates/chat-app/src/storage/*`, `crates/chat-app/src/bin/storage_qa_runner.rs`, `crates/chat-app/src/lib.rs`
  - Pre-commit: `cargo check -p zova`

- [x] 2. Implement SQLite schema, migrations, and storage bootstrap

  **What to do**:
  - Add `sqlx` + sqlite-related dependencies in workspace/package manifests.
  - Add migration files under `crates/chat-app/migrations/`:
    - `sessions` with `active_branch_id`, soft-delete, timestamps
    - `branches` with parent linkage and soft-delete
    - `messages` with `(session_id, branch_id, seq)` ordering and soft-delete
    - `media_refs` with URI/path metadata only
    - `agent_events` with typed event + JSON payload
  - Implement `SqliteStorage::open(...)` with:
    - `PRAGMA journal_mode=WAL`
    - `PRAGMA foreign_keys=ON`
    - `PRAGMA busy_timeout=5000`
    - migration execution (`sqlx::migrate!`)
  - Ensure bootstrap fails fast with typed SNAFU errors.

  **Must NOT do**:
  - No session/message business logic yet.
  - No branch-edit operation implementation yet.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: schema correctness and bootstrap reliability define all downstream behavior.
  - **Skills**: `rust-error-snafu`, `coding-guidelines`
    - `rust-error-snafu`: typed setup/bootstrap/migration failure mapping.
    - `coding-guidelines`: consistent module and API naming.
  - **Skills Evaluated but Omitted**:
    - `git-master`: commit discipline helpful but not core implementation logic.

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (critical infrastructure lane)
  - **Blocks**: 4, 6, 7
  - **Blocked By**: 1

  **References**:
  - `crates/chat-app/Cargo.toml` - dependency declaration style.
  - `Cargo.toml` - workspace dependency strategy.
  - `crates/chat-app/src/database/conversation.rs` - existing sort behavior to preserve semantically in SQL reads.
  - Official docs: `https://docs.rs/sqlx/latest/sqlx/` - pool, query macros, migration API.
  - SQLite docs: `https://www.sqlite.org/pragma.html` - PRAGMA semantics and defaults.

  **Acceptance Criteria**:
  - [x] `cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-2.db" --scenario schema_init` outputs `schema_ok=true` and pragma values.
  - [x] `cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-2.db" --scenario fk_violation` outputs `fk_violation_blocked=true`.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Bootstrap creates expected schema and pragmas
    Tool: Bash
    Preconditions: Migrations and open() bootstrap are implemented
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-2.db" --scenario schema_init
      2. Assert: stdout contains "schema_ok=true"
      3. Assert: stdout contains "journal_mode=wal" and "foreign_keys=1"
      4. Save stdout to: .sisyphus/evidence/task-2-schema-init.txt
    Expected Result: DB bootstraps with all required tables and pragmas
    Failure Indicators: missing table, wrong pragma, non-zero exit
    Evidence: .sisyphus/evidence/task-2-schema-init.txt

  Scenario: FK guard rejects orphan message insert
    Tool: Bash
    Preconditions: FK constraints enabled
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-2.db" --scenario fk_violation
      2. Assert: stdout contains "fk_violation_blocked=true"
      3. Save stdout to: .sisyphus/evidence/task-2-fk-violation.txt
    Expected Result: Invalid child row write is rejected
    Failure Indicators: orphan insert succeeds OR no constraint error signal
    Evidence: .sisyphus/evidence/task-2-fk-violation.txt
  ```

  **Commit**: YES
  - Message: `feat(storage): add sqlite schema and sqlx bootstrap`
  - Files: `crates/chat-app/migrations/*`, `crates/chat-app/src/storage/sqlite/*`, cargo manifests
  - Pre-commit: `cargo check -p zova`

- [x] 3. Inventory and adapt integration seams in chat flow (read-only prep task)

  **What to do**:
  - Map all callsites that currently depend on `ConversationStore` or in-memory message lifecycle.
  - Document exact adapter points for replacement:
    - sidebar session list/load/create
    - chat activation history load
    - submit path persistence points (user + assistant placeholder)
  - Create a migration map in plan notes or inline docs for Task 7 executor handoff.

  **Must NOT do**:
  - No functional behavior change yet.

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: read-only impact mapping with narrow deliverable.
  - **Skills**: `coding-guidelines`
    - `coding-guidelines`: keep naming and module boundaries consistent during seam docs.
  - **Skills Evaluated but Omitted**:
    - `rust-error-snafu`: no new runtime error surface introduced in this task.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Task 1)
  - **Blocks**: 7
  - **Blocked By**: None

  **References**:
  - `crates/chat-app/src/chat/sidebar.rs` - current session create/load/refresh flow.
  - `crates/chat-app/src/chat/view.rs` - message append, activation, and stream update flow.
  - `crates/chat-app/src/database/mod.rs` - current persistence export boundary.

  **Acceptance Criteria**:
  - [x] Integration seam checklist exists and references exact functions in `sidebar.rs` and `view.rs`.
  - [x] No behavior-changing diff outside storage planning artifacts.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Integration seam map is complete
    Tool: Bash
    Preconditions: Task 3 mapping artifact created
    Steps:
      1. Run: cargo check -p zova
      2. Assert: compile passes and no behavioral code changes introduced
      3. Save output to: .sisyphus/evidence/task-3-seam-map.txt
    Expected Result: Seam map ready without regressions
    Failure Indicators: compilation failure OR missing seam entries for sidebar/view
    Evidence: .sisyphus/evidence/task-3-seam-map.txt

  Scenario: Existing chat flow remains untouched in prep phase
    Tool: Bash
    Preconditions: Task 3 completed
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --scenario prep_noop
      2. Assert: stdout contains "prep_noop=true"
      3. Save output to: .sisyphus/evidence/task-3-prep-noop.txt
    Expected Result: Prep changes do not alter runtime behavior
    Failure Indicators: scenario reports side effects or incompatibility
    Evidence: .sisyphus/evidence/task-3-prep-noop.txt
  ```

  **Commit**: NO

- [x] 4. Implement SessionStore and MessageStore branch operations

  **What to do**:
  - Implement SessionStore:
    - create/list/get/update/delete(soft)/restore
    - preserve ordering semantics: `updated_at DESC`, tie-break deterministic
  - Implement MessageStore core:
    - append message to active branch
    - load history by `session_id` using active branch + `ORDER BY seq`
    - get/update by `session_id + msg_id`
    - edit-from-history transaction:
      1. open transaction (`BEGIN IMMEDIATE` equivalent via sqlx tx)
      2. create new branch
      3. copy prefix rows up to fork point
      4. apply edited message row
      5. set session active branch to new branch
      6. soft-delete previous branch
    - Default identity rule for v1: copied rows in new branch receive new `msg_id`; operation response returns an old->new id mapping so callers can refresh references safely.
  - Enforce branch invariants and cross-session isolation in queries.

  **Must NOT do**:
  - No branch-switch UI exposure.
  - No cross-session update path that omits `session_id` in predicates.

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: transactional branch semantics and invariants are correctness-critical.
  - **Skills**: `rust-error-snafu`, `coding-guidelines`
    - `rust-error-snafu`: domain-safe error propagation from SQL and invariant checks.
    - `coding-guidelines`: maintain clarity in complex transaction code.
  - **Skills Evaluated but Omitted**:
    - `playwright`: storage behavior verification is command/data oriented.

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3
  - **Blocks**: 7
  - **Blocked By**: 1, 2

  **References**:
  - `crates/chat-app/src/database/conversation.rs` - session ordering and default title behavior to preserve.
  - `crates/chat-app/src/chat/view.rs` - submit flow where persistence writes must align with message lifecycle.
  - `crates/chat-app/src/chat/message.rs` - message role/status semantics for storage serialization.
  - SQLite transaction docs: `https://www.sqlite.org/lang_transaction.html` - atomic branch swap behavior.

  **Acceptance Criteria**:
  - [x] `session_crud` scenario passes with soft-delete and restore behavior.
  - [x] `history_branch_fork` scenario passes and only latest branch is visible.
  - [x] `cross_session_guard` scenario proves `session_id + msg_id` mismatch is rejected.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Session CRUD with soft delete + restore
    Tool: Bash
    Preconditions: SessionStore methods implemented
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-4.db" --scenario session_crud
      2. Assert: stdout contains "created=2" and "soft_deleted=1" and "restored=1"
      3. Assert: stdout contains "list_order_ok=true"
      4. Save stdout to: .sisyphus/evidence/task-4-session-crud.txt
    Expected Result: CRUD + ordering + restore semantics are correct
    Failure Indicators: missing row, wrong order, restore failure
    Evidence: .sisyphus/evidence/task-4-session-crud.txt

  Scenario: Edit-from-history forks and only new branch is visible
    Tool: Bash
    Preconditions: MessageStore branch logic implemented
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-4.db" --scenario history_branch_fork
      2. Assert: stdout contains "fork_created=true"
      3. Assert: stdout contains "active_branch_visible_count=" with expected value
      4. Assert: stdout contains "old_branch_visible_count=0"
      5. Save stdout to: .sisyphus/evidence/task-4-history-branch.txt
    Expected Result: latest branch visibility rule enforced
    Failure Indicators: old branch rows still visible by default OR branch swap not atomic
    Evidence: .sisyphus/evidence/task-4-history-branch.txt

  Scenario: Cross-session message mutation is blocked
    Tool: Bash
    Preconditions: session-scoped predicates implemented
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-4.db" --scenario cross_session_guard
      2. Assert: stdout contains "cross_session_guard=true"
      3. Save stdout to: .sisyphus/evidence/task-4-cross-session.txt
    Expected Result: wrong session_id + msg_id pair cannot mutate data
    Failure Indicators: mutation succeeds across sessions
    Evidence: .sisyphus/evidence/task-4-cross-session.txt
  ```

  **Commit**: YES
  - Message: `feat(storage): implement session and message stores with branch forking`
  - Files: `crates/chat-app/src/storage/sqlite/*`, storage trait impl files
  - Pre-commit: `cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/precommit-task-4.db" --scenario session_crud`

- [x] 5. Implement MediaStore and AgentEventStore extension points

  **What to do**:
  - Implement media reference operations keyed by `session_id + msg_id`:
    - attach/list/soft-delete media refs
    - enforce path/URI storage only
    - store metadata: `mime_type`, `size_bytes`, optional duration/dimensions/hash
  - Implement agent event operations:
    - append/list events with `event_type + payload_json`
    - support optional `msg_id` for per-message or session-level events
  - Ensure both stores share transaction/error model with base storage.

  **Must NOT do**:
  - No binary media bytes persisted in SQLite.
  - No overfitted agent schema for every future event subtype.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: extensibility boundary design with compatibility implications.
  - **Skills**: `rust-error-snafu`, `coding-guidelines`
    - `rust-error-snafu`: map metadata validation and FK errors cleanly.
    - `coding-guidelines`: keep extensible APIs readable and stable.
  - **Skills Evaluated but Omitted**:
    - `xlsx`: unrelated to storage-layer Rust implementation.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2/3 boundary (can start after Task 2)
  - **Blocks**: 7
  - **Blocked By**: 1, 2

  **References**:
  - `crates/chat-app/src/chat/message.rs` - role/message associations for media and agent event linkage.
  - `crates/chat-app/src/lib.rs` - module export surface consistency.
  - `https://www.sqlite.org/json1.html` - JSON payload handling guidance for flexible event metadata.

  **Acceptance Criteria**:
  - [x] `media_ref_roundtrip` scenario passes with URI + metadata persisted.
  - [x] `media_blob_guard` scenario confirms blob payload writes are rejected by API/schema.
  - [x] `agent_event_roundtrip` scenario persists and reloads typed event payload.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Media reference path metadata roundtrip
    Tool: Bash
    Preconditions: MediaStore implemented
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-5.db" --scenario media_ref_roundtrip
      2. Assert: stdout contains "media_roundtrip=true"
      3. Assert: stdout contains "stored_uri=file://"
      4. Save stdout to: .sisyphus/evidence/task-5-media-roundtrip.txt
    Expected Result: URI/path + metadata can be written/read deterministically
    Failure Indicators: missing metadata fields OR URI normalization failure
    Evidence: .sisyphus/evidence/task-5-media-roundtrip.txt

  Scenario: Blob write attempt is rejected
    Tool: Bash
    Preconditions: MediaStore validation in place
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-5.db" --scenario media_blob_guard
      2. Assert: stdout contains "blob_guard=true"
      3. Save stdout to: .sisyphus/evidence/task-5-media-blob-guard.txt
    Expected Result: API refuses blob-like input payloads for media storage
    Failure Indicators: blob payload accepted or silently truncated
    Evidence: .sisyphus/evidence/task-5-media-blob-guard.txt

  Scenario: Agent event JSON payload persists correctly
    Tool: Bash
    Preconditions: AgentEventStore implemented
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-5.db" --scenario agent_event_roundtrip
      2. Assert: stdout contains "agent_event_roundtrip=true"
      3. Save stdout to: .sisyphus/evidence/task-5-agent-event.txt
    Expected Result: event_type + payload_json stored and retrieved accurately
    Failure Indicators: JSON payload mismatch or missing event ordering
    Evidence: .sisyphus/evidence/task-5-agent-event.txt
  ```

  **Commit**: YES
  - Message: `feat(storage): add media references and agent event stores`
  - Files: storage media/agent modules, migration updates if needed
  - Pre-commit: `cargo check -p zova`

- [x] 6. Implement TSV-to-SQLite migration pipeline

  **What to do**:
  - Build idempotent importer for legacy session metadata in `.zova/conversations.tsv`.
  - Migration flow:
    1. open SQLite
    2. if sessions table empty, import TSV sessions preserving title + updated timestamp semantics
    3. create one initial branch per imported session
    4. record migration marker/version
  - Add malformed-row handling policy (skip row + emit structured warning).

  **Must NOT do**:
  - No destructive removal of source TSV on first migration.
  - No forced message backfill from unavailable historical data.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: migration correctness and idempotency are data safety critical.
  - **Skills**: `rust-error-snafu`, `coding-guidelines`
    - `rust-error-snafu`: map parse/import failures with context.
    - `coding-guidelines`: maintain deterministic, maintainable importer flow.
  - **Skills Evaluated but Omitted**:
    - `frontend-ui-ux`: no UI rendering changes in migration code.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Task 4)
  - **Blocks**: 7
  - **Blocked By**: 2

  **References**:
  - `crates/chat-app/src/database/conversation.rs` - TSV parsing and ordering behavior to mirror.
  - `crates/chat-app/src/chat/sidebar.rs` - how imported sessions are expected to surface in list view ordering.

  **Acceptance Criteria**:
  - [x] `migrate_tsv_fixture` scenario imports fixture data with expected counts/order.
  - [x] `migrate_idempotent` scenario re-run produces no duplicates.
  - [x] `migrate_malformed_row` scenario reports row-level skip and keeps valid rows.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: TSV fixture imports into sqlite with preserved ordering
    Tool: Bash
    Preconditions: migration importer implemented
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-6.db" --scenario migrate_tsv_fixture
      2. Assert: stdout contains "imported_sessions=3"
      3. Assert: stdout contains "order_preserved=true"
      4. Save stdout to: .sisyphus/evidence/task-6-migrate-fixture.txt
    Expected Result: legacy sessions imported with deterministic sort behavior
    Failure Indicators: count mismatch OR order mismatch
    Evidence: .sisyphus/evidence/task-6-migrate-fixture.txt

  Scenario: Migration is idempotent
    Tool: Bash
    Preconditions: same DB used for second run
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/task-6.db" --scenario migrate_idempotent
      2. Assert: stdout contains "idempotent=true"
      3. Save stdout to: .sisyphus/evidence/task-6-idempotent.txt
    Expected Result: repeated migration creates no duplicate sessions/branches
    Failure Indicators: duplicate rows after second run
    Evidence: .sisyphus/evidence/task-6-idempotent.txt
  ```

  **Commit**: YES
  - Message: `feat(storage): add legacy tsv importer for session metadata`
  - Files: storage migration/import modules
  - Pre-commit: `cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/precommit-task-6.db" --scenario migrate_idempotent`

- [x] 7. Integrate storage facade into ChatSidebar/ChatView and run full QA sweep

  **What to do**:
  - Replace direct `ConversationStore` usage in `ChatSidebar` with storage facade calls.
  - Integrate history loading in `ChatView::activate_conversation` through storage abstraction.
  - Integrate message persistence points in submit/finalize paths without altering stream state machine semantics.
  - Ensure message operations call storage methods with explicit `session_id + msg_id`.
  - Execute full scenario suite and capture evidence.

  **Must NOT do**:
  - No unrelated UI redesign or stream protocol changes.
  - No fallback to direct file writes once storage facade is wired.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: cross-module integration and regression-sensitive orchestration.
  - **Skills**: `rust-error-snafu`, `coding-guidelines`, `git-master`
    - `rust-error-snafu`: integration boundary errors must stay typed and actionable.
    - `coding-guidelines`: preserve module cohesion and naming quality.
    - `git-master`: helps produce atomic, reviewable commit boundaries for large integration diff.
  - **Skills Evaluated but Omitted**:
    - `playwright`: desktop GPUI integration verified via deterministic scenario runner, not browser E2E.

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 4 (final integration)
  - **Blocks**: None
  - **Blocked By**: 3, 4, 5, 6

  **References**:
  - `crates/chat-app/src/chat/sidebar.rs` - replace session list persistence boundary.
  - `crates/chat-app/src/chat/view.rs` - activation/submit/finalize persistence insertion points.
  - `crates/chat-app/src/chat/events.rs` - event contracts that should remain stable.
  - `crates/chat-app/src/chat/message.rs` - stream state constraints that must not be altered.

  **Acceptance Criteria**:
  - [x] `cargo check -p zova` passes after integration.
  - [x] `cargo clippy -p zova --all-targets -- -D warnings` passes.
  - [x] Full scenario suite passes: `all=true` and exit code 0.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Full storage suite passes end-to-end
    Tool: Bash
    Preconditions: all previous tasks complete
    Steps:
      1. Run: cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/final-storage.db" --scenario all
      2. Assert: stdout contains "all_passed=true"
      3. Save stdout JSON to: .sisyphus/evidence/task-7-all.json
    Expected Result: all storage invariants and workflows pass deterministically
    Failure Indicators: any scenario false OR non-zero exit code
    Evidence: .sisyphus/evidence/task-7-all.json

  Scenario: Integration compile/lint gate
    Tool: Bash
    Preconditions: integration code merged
    Steps:
      1. Run: cargo check -p zova
      2. Run: cargo clippy -p zova --all-targets -- -D warnings
      3. Save outputs to: .sisyphus/evidence/task-7-check-clippy.txt
    Expected Result: code compiles cleanly and passes lints
    Failure Indicators: compile error, lint error, warnings-as-errors failure
    Evidence: .sisyphus/evidence/task-7-check-clippy.txt
  ```

  **Commit**: YES
  - Message: `feat(storage): wire sqlite storage into chat flows`
  - Files: sidebar/view integration points + storage modules + migration import glue
  - Pre-commit: `cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/precommit-final.db" --scenario all`

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 1 | `feat(storage): introduce storage contracts and UUIDv7 ids` | storage contracts + QA runner scaffold | `cargo check -p zova` |
| 2 | `feat(storage): add sqlite schema and sqlx bootstrap` | migrations + sqlite open/bootstrap | `schema_init` scenario |
| 4 | `feat(storage): implement session and message branch stores` | store implementations | `session_crud` + `history_branch_fork` |
| 6 | `feat(storage): add tsv metadata importer` | importer + migration marker logic | `migrate_idempotent` scenario |
| 7 | `feat(storage): integrate storage facade into chat app` | sidebar/view integration + final polish | `scenario all` + clippy gate |

---

## Success Criteria

### Verification Commands

```bash
cargo check -p zova
cargo clippy -p zova --all-targets -- -D warnings
cargo run -p zova --bin storage_qa_runner -- --db ".sisyphus/evidence/final-storage.db" --scenario all
```

### Final Checklist
- [x] All Must Have requirements implemented.
- [x] All Must NOT Have guardrails respected.
- [x] Branch semantics match user decision (latest branch visible, old branch logically deleted).
- [x] `session_id + msg_id` scoping enforced on message mutation/read APIs.
- [x] Media storage uses URI/path metadata only.
- [x] Legacy TSV sessions import safely and idempotently.
- [x] Full agent-executed QA evidence captured under `.sisyphus/evidence/`.
