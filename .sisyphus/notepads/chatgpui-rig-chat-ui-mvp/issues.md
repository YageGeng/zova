# Issues

- 2026-02-12: `cargo check --workspace` initially failed in `zed-font-kit` due to mixed `core_graphics` versions (`0.24` vs `0.25`) from transitive dependencies.
- 2026-02-12: Resolved by pinning `core-text` to `=21.0.0` at workspace level and including it in `chat-app` dependencies to force a compatible resolution.
- 2026-02-12: Local `lsp_diagnostics` does not support Rust in this environment; verification must rely on cargo commands until Rust LSP is configured.
- 2026-02-12: Local shell does not provide `sg` CLI; use built-in `ast_grep_search` tool instead of shell `sg` command.
- 2026-02-12: `NotificationList` import path is in `gpui_component::notification` submodule, not root. Fixed in both app.rs and main.rs.
- 2026-02-12: Button `.small()` method requires `Sizable` trait in scope. Added `use gpui_component::Sizable` to imports.
- 2026-02-12: `tracing` and `tracing-subscriber` needed to be added to workspace dependencies for logging in main.rs and app.rs.
- 2026-02-12: `actions!` macro doesn't accept doc comments on individual action variants - causes "unused doc comment" warnings.
- 2026-02-12: `clippy -D warnings` flagged manual `Default` impl for `StreamState` (`derivable_impls`); fixed by deriving `Default` and marking `Idle` as `#[default]`.
- 2026-02-12: Task 5 message input module initially had unused imports (`FluentBuilder`, `Disableable`) and would fail strict linting; resolved by removing unused imports before final `clippy -D warnings` run.
- 2026-02-12: Repository had no local `v_virtual_list` usage to copy from; Task 3 implementation relied directly on ChatGPUI sidebar reference patterns.
- 2026-02-12: Using a relative persistence path (`.chat-app/conversations.tsv`) means restart data is tied to process working directory; Task 7+ should keep launch cwd stable or introduce an explicit app data root.
- 2026-02-12: `lsp_diagnostics` briefly reported stale Rust errors while `message_list.rs` edits were in-flight; re-running diagnostics after the full patch returned clean results.
- 2026-02-12: Task 7 coordinator streaming depends on runtime provider env config (`OPENAI_API_KEY`/optional `OPENAI_BASE_URL` and `OPENAI_MODEL`); without key the submit path intentionally emits a provider-not-configured assistant error.
- 2026-02-12: `app.rs` is shared by lib/bin targets; referencing `crate::chat` from the shell required declaring `mod chat`, `mod database`, and `mod llm` in `main.rs` for the binary target to compile cleanly.
- 2026-02-12: Task 8 compile errors: gpui_component imports must use module paths (`button::Button`), not direct imports. `FluentBuilder` trait required for `.when()` methods. Theme error color is `theme.danger`. Settings persistence refactored to use simple key=value format instead of JSON to avoid adding serde/serde_json/thiserror/dirs dependencies. SettingsView simplified to read-only display to avoid InputState Window lifecycle complexity in subscription callbacks.

- 2026-02-12: Settings view initially implemented as read-only text display. Refactored to use InputState with Input components for editable fields. Key challenge: InputState requires Window reference which is only available in ChatView::new() or via cx.update_window(). Solved by creating SettingsView once in ChatView::new() and using boolean flag for visibility toggle.

- 2026-02-12: `#[gpui::test]` + `TestAppContext` was unavailable in this environment for integration tests; attempting a GPUI test fixture failed compile. Resolved by using a deterministic pure unit test fixture in `message_list.rs` for the 2k-history virtualization stability check.
- 2026-02-12: `cargo fmt --all -- --check` initially failed due formatting drift across existing chat-app files; had to run `cargo fmt --all` before rerunning the full gate.
- 2026-02-12: Final `clippy -D warnings` gate flagged `let_unit_value` in `chat/view.rs` and `redundant_iter_cloned` in `model_selector.rs`; resolved via direct cleanup (removed unit binding, iterated by reference).

- 2026-02-12: New Task 9 requirement expected an integration test target named `perf_message_virtualization`; existing coverage was unit-test only, so `cargo test -p chat-app --test perf_message_virtualization ...` initially had no target until adding `tests/perf_message_virtualization.rs`.
- 2026-02-12: `cargo fmt --all -- --check` failed immediately after new tests due formatting drift; required `cargo fmt --all` before final full-gate rerun.
- 2026-02-12: Provider error consolidation requires updating both SNAFU selector imports and direct `ProviderError` constructors in stream workers; missing either side causes compile errors.
