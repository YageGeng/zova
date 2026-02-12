# Learnings

- 2026-02-13: Moving `src/storage/*` into `crates/storage/src/*` with minimal edits preserved storage behavior while decoupling crate boundaries.
- 2026-02-13: Introducing storage-local `MessageRole` and explicit UI conversion functions keeps role coupling out of the storage crate.
- 2026-02-13: Keeping migration SQL unchanged and reusing `sqlx::migrate!("./migrations")` preserved schema/bootstrap semantics after extraction.
- 2026-02-13: Renaming UI crate from `zova` to `ui` only required package-name/import-path updates (`use ui::...`) because workspace membership is path-wildcarded (`crates/*`).
