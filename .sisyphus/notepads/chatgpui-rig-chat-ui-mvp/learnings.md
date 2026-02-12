# Learnings

- 2026-02-12: The minimal GPUI bootstrap that matches ChatGPUI conventions is `Application::new().with_assets(...)`, then `gpui_component::init(cx)`, then opening a window wrapped by `Root::new(...)`.
- 2026-02-12: On macOS with the current zed/gpui stack, pinning `core-text` to `=21.0.0` avoids `core_graphics` type mismatches during `cargo check`.
- 2026-02-12: A simple integration test under `crates/chat-app/tests` that calls a public library function is sufficient to verify workspace test discovery early.
- 2026-02-12: ChatGPUI state wiring pattern is parent-orchestrated: child entities emit typed events (`EventEmitter`), parent subscribes via `subscribe`/`subscribe_in`, then calls child update methods.
- 2026-02-12: ChatGPUI streaming uses a two-step bridge: `Tokio::spawn` for provider IO + channel back to `cx.spawn` for UI-safe updates; this avoids blocking GPUI context.
- 2026-02-12: gpui-component requires `gpui_component::init(cx)` before component usage and `Root::new(...)` as first window layer for notifications/dialogs/sheets to work.
- 2026-02-12: `v_virtual_list` in ChatGPUI depends on explicit `item_sizes` and content-width-aware measurement/caching; inaccurate size bookkeeping causes jumpy scrolling.
- 2026-02-12: Recommended Rig integration boundary is a thin adapter that maps Rig stream chunks to app-level `StreamEvent` enums, keeping UI provider-agnostic.
- 2026-02-12: `NotificationList` is located at `gpui_component::notification::NotificationList`, not in the root of `gpui_component`.
- 2026-02-12: The `Sizable` trait must be in scope to use `.small()` method on Button components.
- 2026-02-12: Theme loading with `ThemeRegistry::watch_dir()` is non-fatal when the themes directory is missing - the app falls back to built-in defaults.
- 2026-02-12: Sidebar animation uses `animation_trigger` counter pattern: increment on each toggle to reset animation state and ensure smooth collapse/expand transitions.
- 2026-02-12: The `actions!` macro from gpui does not support doc comments on individual actions - they should be documented separately.
- 2026-02-12: A provider-agnostic stream contract works best when each mapped stream event carries a `StreamTarget` (`ConversationId` + `StreamSessionId`), which makes stale-chunk rejection a pure equality check.
- 2026-02-12: Encoding stream lifecycle transitions (`Start/Complete/Fail/Cancel/ResetToIdle`) as a dedicated boundary type keeps orchestration deterministic before UI entities are wired.
- 2026-02-12: Chat-style Enter handling can stay deterministic by combining `on_key_down` Shift+Enter newline insertion with `InputEvent::PressEnter` and a `pending_newline` guard to suppress accidental submit.
- 2026-02-12: Emitting typed `Submit::new(StreamTarget, content)` and `Stop { target }` directly from `MessageInput` keeps the input module provider-agnostic while preserving Task 7 orchestration hooks.
- 2026-02-12: Clearing local streaming state immediately after stop emission re-enables input editing without waiting for outer orchestration callbacks.
- 2026-02-12: Sidebar grouping stays deterministic when the virtual list is built from a flattened enum (`GroupHeader` + `Conversation`) and an `item_sizes` vector with matching length/order.
- 2026-02-12: A lightweight TSV conversation store can provide restart-safe MVP persistence if titles are escaped before write and records are sorted by `updated_at_unix_seconds` descending on load.
- 2026-02-12: Message virtualization remains stable when `item_sizes` is rebuilt in message order and a per-`MessageId` layout hash invalidates only entries whose content/status actually changed.
- 2026-02-12: Measuring only the currently visible virtual-list range keeps sizing work bounded while preserving accurate row heights for markdown-heavy assistant messages.
- 2026-02-12: A practical Task 7 wiring is to keep `ChatView` as orchestration parent for sidebar/input events while rendering the sidebar entity from `ChatAppShell`; this preserves parent-owned stream/session state and existing shell resize behavior.
- 2026-02-12: In `Context<T>`, `cx.spawn` closures for entity tasks take `(this, cx)` and can call `this.update(...)`; this is the stable way to consume provider streams and debounce timers while mutating UI state.
- 2026-02-12: Debounced streaming updates stay deterministic by buffering deltas in coordinator state, allowing only one debounce task (~50ms), and flushing only when `StreamTarget` still matches the active conversation/session.
- 2026-02-12: Auto-cancel on conversation switch can be implemented by dropping the active GPUI task handle; because `ProviderEventStream` cancels on drop, this cleanly propagates cancellation to provider IO without concurrent stream overlap.
- 2026-02-12: Task 8 implementation: Model selector and settings integration. Created `model_selector.rs`, `settings/state.rs`, `settings/view.rs`. Settings persist to `.chat-app/settings.conf` using simple key=value format to avoid serde/thiserror dependencies. Provider adapter reloads on settings change via SettingsChanged event. Model selector button shows dropdown with available models and settings button. Settings panel displays as overlay in main window.
- 2026-02-12: GPUI component imports must use full module paths: `gpui_component::button::{Button, ButtonVariants}` not `gpui_component::{Button, ButtonVariants}`.
- 2026-02-12: `FluentBuilder` trait must be in scope to use `.when()` and `.when_some()` methods on elements.
- 2026-02-12: Theme error color is `theme.danger`, not `theme.error` or `theme.destructive`.
- 2026-02-12: `InputState` requires Window reference to create; for MVP settings view used static text display instead of interactive inputs to simplify lifecycle management.
- 2026-02-12: Settings persistence uses simple line-based format (key=value) to avoid adding serde, serde_json, thiserror, dirs dependencies. File stored at `.chat-app/settings.conf` relative to working directory.
- 2026-02-12: Subscription callbacks receive different signatures depending on context - some receive window, some don't. Use `|this, _, event, cx|` pattern consistently.

- 2026-02-12: Task 8 settings view refactored to use editable InputState fields with Input components. SettingsView created once in ChatView::new() where Window is available. Boolean flag settings_open controls overlay visibility. Save reads from InputState values and writes to settings store, triggering SettingsChanged event. Provider reloads immediately without restart.
- 2026-02-12: InputState requires Window reference for creation and for set_value operations. Pattern: create InputState in constructor, update values via update() calls with window reference.
- 2026-02-12: GPUI subscription to custom events (like SettingsClose) works via cx.subscribe() on the entity emitting the event. EventEmitter trait implementation required.
- 2026-02-12: Using .when(boolean_flag, ...) instead of .when_some(Option, ...) for conditional rendering based on boolean state.

- 2026-02-12: Task 9 gate hardening: importing `chat_app::app` from the binary target (instead of redeclaring `mod chat/mod llm/...` in `main.rs`) prevents duplicate module compilation and eliminates a large class of dead-code/unused warnings under `clippy -D warnings`.
- 2026-02-12: A deterministic large-history fixture for message virtualization can stay timing-free by validating `estimate_message_height` and `layout_hash` over 2,000 messages, then asserting only the mutated tail message changes hash.
- 2026-02-12: In modules that glob-import `gpui::*`, unit tests may resolve `#[test]` to GPUI's macro; using `#[::core::prelude::v1::test]` keeps standard Rust unit tests deterministic without GPUI test runtime hooks.

- 2026-02-12: Task 9 follow-up perf target can stay deterministic as an integration test by exposing a pure `virtualization_metrics` helper in `message_list.rs` and asserting 2k-row metrics/hash stability without any GPUI runtime or timing checks.
- 2026-02-12: Explicit debounce coverage can be modeled as contract tests: assert `STREAM_DEBOUNCE_MS` configuration and verify delta/reasoning stream payloads remain transition-free (`into_transition() == None`) until terminal events.
- 2026-02-12: Explicit stream isolation coverage should assert both acceptance (`accepts_stream_event` for active target) and rejection (`SessionMismatch` on foreign terminal target) to mirror coordinator stale-event guards.
- 2026-02-12: For Rig completion failures, using one SNAFU variant with `stage` + `CompletionError` keeps `ProviderError` simpler while preserving open/chunk context via distinct stage values.
