# Issues

## 2026-02-13 - Task 4 issues

- Initial implementation attempted to run pooled sqlx operations on worker-thread runtimes and hit `pool timed out while waiting for an open connection` during `session-create`; resolved by switching store operations to dedicated per-call sqlite connections.

## 2026-02-13 - Task 6 issues

- No blocking runtime issue surfaced in importer logic; the main risk was cross-scenario fixture leakage from `.zova/conversations.tsv`, resolved by restoring prior fixture state after each scenario run.

## 2026-02-13 - Task 6 verification-only issues

- No new blocking issues surfaced during the required three-scenario rerun; current Task 6 importer behavior is already stable for fixture import, idempotency, and malformed-row handling.

## 2026-02-13 - Task 7 issues

- Environment `sg` binary resolves to a non-ast-grep command (`用法：sg 组 ...`), so AST verification used the built-in `ast_grep_search` tool plus targeted `ConversationStore` absence checks.
- `cargo clippy -D warnings` flagged `let ... else` Option unwraps in new sidebar Option-returning methods; resolved by switching to `?` to satisfy `clippy::question_mark`.

## 2026-02-13 - Task 7 verification continuation issues

- First rerun of `storage_qa_runner --scenario all` failed on `session_crud` because `.sisyphus/evidence/final-storage.db` already contained prior scenario data; resolved by deleting `final-storage.db`, `final-storage.db-wal`, and `final-storage.db-shm` before rerunning required checks.

## 2026-02-13 - Final DoD/checklist closure issues

- No new blockers surfaced during final closure; remaining work was evidence regeneration for missing mandatory scenario files (`task-1` to `task-6`), then checklist confirmation.
