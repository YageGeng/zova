# ChatGPUI-Style Chat App (GPUI + gpui-component + Rig) - MVP Plan

## TL;DR

> **Quick Summary**: Build a desktop chat app that follows ChatGPUI’s core interaction architecture (sidebar + model selector + virtualized message area + streaming input flow), implemented with GPUI + gpui-component and a thin Rig-backed provider adapter.
>
> **Deliverables**:
> - New Rust crate in this workspace implementing the MVP desktop chat app.
> - Rig adapter layer for streaming chat responses and model listing.
> - Core UI modules: sidebar, message list, input bar, model selector, minimal settings.
> - Local persistence for conversations/messages and automated tests for core behaviors.
>
> **Estimated Effort**: Large
> **Parallel Execution**: YES - 4 waves
> **Critical Path**: Task 0 -> Task 1 -> Task 2 -> Task 7 -> Task 9

---

## Context

### Original Request
- "详细分析 ChatGPUI 的 UI 构成，并仿照这个项目写一个 chat 应用；用 gpui + gpui-component；模型 provider 用 rig；尽量使用 gpui-component 提供的功能。"

### Interview Summary
**Key Discussions**:
- User confirmed proceeding from analysis to concrete implementation blueprint.
- Scope is locked to **MVP parity first**, not full high-fidelity parity.
- Test strategy is locked to **set up test infrastructure first, then add tests after implementation**.

**Research Findings**:
- ChatGPUI root layout and event orchestration: `/Users/isbset/Documents/ChatGPUI/src/app.rs:46`.
- Chat orchestration, streaming debounce, and conversation switching: `/Users/isbset/Documents/ChatGPUI/src/chat/view.rs:60`.
- Message virtualization and dynamic measurement: `/Users/isbset/Documents/ChatGPUI/src/chat/message_list.rs:157`.
- Input interactions and toolbar behavior: `/Users/isbset/Documents/ChatGPUI/src/chat/message_input.rs:54`.
- Sidebar grouping and context menu patterns: `/Users/isbset/Documents/ChatGPUI/src/chat/sidebar.rs:70`.
- Model selector popover two-panel behavior: `/Users/isbset/Documents/ChatGPUI/src/model_selector.rs:31`.
- gpui-component initialization and Root requirements: `/Users/isbset/Documents/ChatGPUI/src/main.rs:102`.

### Metis Review
**Identified Gaps (addressed in this plan)**:
- Explicitly lock down MVP non-goals to prevent scope creep.
- Define stream/session isolation semantics for conversation switching.
- Add concrete acceptance checks for debounce, scroll-follow, and provider-error handling.
- Keep provider abstraction thin (Rig adapter only), avoid speculative plugin framework.

---

## Work Objectives

### Core Objective
Deliver a production-shaped MVP desktop chat app that reproduces ChatGPUI’s core UX architecture and streaming behavior while using gpui-component-first composition and Rig as the model-provider backend.

### Concrete Deliverables
- `crates/chat-app/Cargo.toml` with GPUI/gpui-component/Rig dependencies and test setup.
- `crates/chat-app/src/main.rs` bootstrapping `gpui_component::init`, `Root`, and theme loading.
- `crates/chat-app/src/app.rs` root split layout (sidebar + main + resize handle).
- `crates/chat-app/src/chat/{view.rs,sidebar.rs,message_list.rs,message_input.rs,scroll_manager.rs,message.rs}`.
- `crates/chat-app/src/llm/{mod.rs,provider.rs,rig_adapter.rs,model.rs}`.
- `crates/chat-app/src/settings/{mod.rs,state.rs,view.rs}` (minimal provider/model settings).
- `crates/chat-app/tests/*` covering core state and streaming behavior.

### Definition of Done
- [x] `cargo check --workspace` exits 0.
- [x] `cargo test --workspace` exits 0.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [x] Core MVP flows pass automated scenarios: create/switch conversation, stream response, stop response, select model/provider, persist/reload history.

### Must Have
- ChatGPUI-like layout architecture with independent sidebar and main chat region.
- Virtualized conversation list and virtualized message list.
- Streaming response flow with debounced UI updates and explicit stop behavior.
- Rig-backed provider adapter with model list retrieval and stream event mapping.
- gpui-component-first implementation (Input, VirtualList, ListItem, Popover, PopupMenu, Button, Select, Switch, Notification, TextView).

### Must NOT Have (Guardrails)
- No attachment upload in MVP.
- No message edit/regenerate in MVP.
- No import/export, search, favorites, telemetry, plugin framework in MVP.
- No cross-platform parity guarantee in this milestone (default target: macOS first).
- No custom low-level `Element` implementation unless gpui-component cannot satisfy requirement and rationale is documented.

---

## Verification Strategy (MANDATORY)

> **UNIVERSAL RULE: ZERO HUMAN INTERVENTION**
>
> Every acceptance criterion in this plan must be verifiable through commands or automated tests. No manual clicking, visual confirmation, or user-driven checks are allowed.

### Test Decision
- **Infrastructure exists**: NO (current workspace has no crate members yet).
- **Automated tests**: Tests-after (setup first, then add tests per module).
- **Framework**: Rust built-in `cargo test` + integration tests under `crates/chat-app/tests`.

### Test Setup Task (included in Task 0)
- Create test layout (`tests/`) and smoke tests for app boot/state transitions.
- Define deterministic stream simulation tests for provider adapter.
- Ensure CI-grade commands are available: `check`, `test`, `clippy`, `fmt --check`.

### Agent-Executed QA Scenarios (Applies to all tasks)
- All tasks include at least one happy-path and one failure-path scenario.
- For desktop GPUI behaviors, verification uses deterministic integration tests and command assertions.
- Evidence is captured under `.sisyphus/evidence/` (logs, snapshots, or command outputs).

---

## Execution Strategy

### Parallel Execution Waves

```text
Wave 1 (Start Immediately):
├── Task 0: Workspace + test infrastructure bootstrap
└── Task 1: App shell + Root/theme/window initialization

Wave 2 (After Wave 1):
├── Task 2: Domain models + event contracts + state boundaries
├── Task 3: Sidebar + persistence repository
├── Task 4: MessageList virtualization + scroll manager
├── Task 5: MessageInput interactions (send/stop/shortcuts)
└── Task 6: Rig adapter + provider/model boundary

Wave 3 (After Wave 2):
├── Task 7: Chat orchestration integration (streaming/debounce/switch)
└── Task 8: Model selector + minimal settings integration

Wave 4 (After Wave 3):
└── Task 9: End-to-end QA + hardening + performance checks

Critical Path: 0 -> 1 -> 2 -> 7 -> 9
Parallel Speedup: ~35-45% vs fully sequential
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|----------------------|
| 0 | None | 1,2,3,4,5,6 | 1 |
| 1 | 0 | 3,4,5,8 | 2,6 |
| 2 | 0 | 7 | 3,4,5,6 |
| 3 | 0,1 | 7 | 4,5,6 |
| 4 | 0,1 | 7 | 3,5,6 |
| 5 | 0,1 | 7 | 3,4,6 |
| 6 | 0,2 | 7,8 | 3,4,5 |
| 7 | 2,3,4,5,6 | 8,9 | None |
| 8 | 1,6,7 | 9 | None |
| 9 | 7,8 | None | None |

### Agent Dispatch Summary

| Wave | Tasks | Recommended Agents |
|------|-------|--------------------|
| 1 | 0,1 | `task(category="quick"/"unspecified-low", load_skills=["gpui-context","gpui-style-guide"], run_in_background=false)` |
| 2 | 2,3,4,5,6 | Domain split across `gpui-entity`, `gpui-async`, `gpui-layout-and-style` |
| 3 | 7,8 | `unspecified-high` with `gpui-entity`, `gpui-context`, `gpui-async` |
| 4 | 9 | `unspecified-high` with `gpui-test`, plus QA-oriented verification agent |

---

## TODOs

- [x] 0. Bootstrap workspace crate and test infrastructure

  **What to do**:
  - Create `crates/chat-app` package and register workspace member.
  - Add dependencies: `gpui`, `gpui-component`, `gpui-component-assets` (optional), `rig-core`, async/runtime dependencies.
  - Add baseline test scaffold under `crates/chat-app/tests` and smoke test command wiring.

  **Must NOT do**:
  - Do not implement feature modules yet.
  - Do not add non-essential dependencies (telemetry/plugin systems).

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: deterministic bootstrap and scaffolding task.
  - **Skills**: `gpui-context`, `gpui-test`
    - `gpui-context`: ensures correct app/window bootstrap patterns.
    - `gpui-test`: ensures test scaffolding is valid and runnable early.
  - **Skills Evaluated but Omitted**:
    - `frontend-ui-ux`: visual design work is not needed in this scaffold task.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Task 1)
  - **Blocks**: 1,2,3,4,5,6
  - **Blocked By**: None

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/main.rs:96` - Demonstrates GPUI application bootstrap and assets wiring.
  - `https://longbridge.github.io/gpui-component/docs/getting-started` - Official dependency and init sequence for gpui-component.
  - `/Users/isbset/Documents/zova/Cargo.toml:1` - Current workspace baseline; member registration target.
  - `https://docs.rig.rs/` - Rig crate usage and capability surface.

  **Acceptance Criteria**:
  - [ ] `crates/chat-app/Cargo.toml` exists and is included by workspace.
  - [ ] `cargo check --workspace` -> exit code 0.
  - [ ] `cargo test --workspace` -> exit code 0 with at least one smoke test discovered.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Workspace bootstrap succeeds
    Tool: Bash
    Preconditions: Rust toolchain installed
    Steps:
      1. Run: cargo metadata --format-version 1
      2. Assert: output includes package name "chat-app"
      3. Run: cargo check --workspace
      4. Assert: exit code is 0
    Expected Result: New crate is recognized and compiles
    Failure Indicators: package missing from metadata, non-zero compile exit
    Evidence: .sisyphus/evidence/task-0-bootstrap-check.log

  Scenario: Test scaffold executes
    Tool: Bash
    Preconditions: smoke test file exists under crates/chat-app/tests
    Steps:
      1. Run: cargo test --workspace -- --nocapture
      2. Assert: output contains "test result: ok"
      3. Assert: output contains "chat_app_smoke" (or configured smoke test name)
    Expected Result: Baseline tests run successfully
    Failure Indicators: no tests discovered or non-zero exit
    Evidence: .sisyphus/evidence/task-0-tests.log
  ```

  **Commit**: YES
  - Message: `chore(workspace): bootstrap chat app crate and test scaffold`
  - Files: `Cargo.toml`, `crates/chat-app/**`
  - Pre-commit: `cargo check --workspace && cargo test --workspace`

- [x] 1. Build app shell, Root layer, theme pipeline, and split layout skeleton

  **What to do**:
  - Implement app startup with `gpui_component::init`, `Root::new`, and theme loading via `ThemeRegistry`.
  - Create root layout with sidebar area, resize handle area, and main content area placeholders.
  - Add shell-level actions (`new chat`, `toggle sidebar`) and notification layer placeholder.

  **Must NOT do**:
  - Do not implement actual message/conversation logic in this task.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
    - Reason: foundational UI shell with framework-specific patterns.
  - **Skills**: `gpui-context`, `gpui-layout-and-style`
    - `gpui-context`: app/window/root lifecycle correctness.
    - `gpui-layout-and-style`: split layout and sizing behavior.
  - **Skills Evaluated but Omitted**:
    - `gpui-async`: no heavy async orchestration needed yet.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Task 0)
  - **Blocks**: 3,4,5,8
  - **Blocked By**: 0

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/main.rs:102` - Required `gpui_component::init(cx)` ordering.
  - `/Users/isbset/Documents/ChatGPUI/src/main.rs:197` - `Root::new` at first window level.
  - `/Users/isbset/Documents/ChatGPUI/src/main.rs:108` - Theme registry watcher usage.
  - `/Users/isbset/Documents/ChatGPUI/src/app.rs:206` - Root horizontal split pattern.
  - `https://longbridge.github.io/gpui-component/docs/theme` - Theme loading and application model.

  **Acceptance Criteria**:
  - [ ] App boots to a visible shell window with sidebar/main split.
  - [ ] Theme loading does not crash when `./themes` is missing or empty.
  - [ ] `cargo test -p chat-app --test app_shell_boot` passes.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: App shell boot test passes
    Tool: Bash
    Preconditions: app_shell_boot integration test exists
    Steps:
      1. Run: cargo test -p chat-app --test app_shell_boot -- --exact
      2. Assert: output contains "app_shell_boot ... ok"
    Expected Result: root layout and initialization path are valid
    Failure Indicators: panic in init/root/theme path
    Evidence: .sisyphus/evidence/task-1-shell-boot.log

  Scenario: Missing theme directory handled gracefully
    Tool: Bash
    Preconditions: test simulates missing themes directory
    Steps:
      1. Run: cargo test -p chat-app --test app_shell_boot missing_theme_dir_is_non_fatal -- --exact
      2. Assert: output contains "... ok"
    Expected Result: app still boots with default theme
    Failure Indicators: panic or process abort on missing directory
    Evidence: .sisyphus/evidence/task-1-theme-fallback.log
  ```

  **Commit**: YES
  - Message: `feat(shell): initialize root window and split layout scaffold`
  - Files: `crates/chat-app/src/main.rs`, `crates/chat-app/src/app.rs`
  - Pre-commit: `cargo test -p chat-app --test app_shell_boot`

- [x] 2. Define domain model, event contracts, and state boundaries

  **What to do**:
  - Create core domain structs/enums: `Role`, `Message`, `MessageStatus`, `Conversation`, stream session identity.
  - Define typed events for UI wiring (`ConversationSelected`, `Submit`, `Stop`, `ModelChanged`, `StreamEventMapped`).
  - Define deterministic state transition rules (idle -> streaming -> done/error/cancelled).

  **Must NOT do**:
  - Do not hardcode provider-specific behavior into UI state types.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: load-bearing contracts that affect all subsequent modules.
  - **Skills**: `gpui-entity`, `gpui-event`
    - `gpui-entity`: state encapsulation and update safety.
    - `gpui-event`: event emission/subscription boundary design.
  - **Skills Evaluated but Omitted**:
    - `gpui-layout-and-style`: layout is not the focus of this task.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: 7
  - **Blocked By**: 0

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/chat/message.rs:10` - Message and role baseline.
  - `/Users/isbset/Documents/ChatGPUI/src/llm/provider.rs:14` - Stream event contract baseline.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/view.rs:30` - Conversation/UI event emissions.
  - `https://docs.rig.rs/docs/concepts/agent` - Rig chat flow semantics.

  **Acceptance Criteria**:
  - [ ] Contract tests validate legal/illegal state transitions.
  - [ ] Stream session identity prevents cross-conversation content leakage by design.
  - [ ] `cargo test -p chat-app --test domain_state_transitions` passes.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Valid stream state transitions
    Tool: Bash
    Preconditions: domain transition tests exist
    Steps:
      1. Run: cargo test -p chat-app --test domain_state_transitions stream_happy_path -- --exact
      2. Assert: output contains "stream_happy_path ... ok"
    Expected Result: idle -> streaming -> done path is deterministic
    Failure Indicators: illegal transition panic or assertion failure
    Evidence: .sisyphus/evidence/task-2-state-happy.log

  Scenario: Illegal cross-session update rejected
    Tool: Bash
    Preconditions: negative transition test exists
    Steps:
      1. Run: cargo test -p chat-app --test domain_state_transitions rejects_cross_session_chunk -- --exact
      2. Assert: output contains "... ok"
    Expected Result: stale/foreign session chunks are ignored
    Failure Indicators: wrong session mutates current conversation
    Evidence: .sisyphus/evidence/task-2-state-negative.log
  ```

  **Commit**: YES
  - Message: `feat(core): define chat domain and event contracts`
  - Files: `crates/chat-app/src/chat/message.rs`, `crates/chat-app/src/chat/events.rs`
  - Pre-commit: `cargo test -p chat-app --test domain_state_transitions`

- [x] 3. Implement sidebar conversation module with grouped virtual list and persistence

  **What to do**:
  - Implement sidebar search input + grouped virtual list (Today/Yesterday/Older).
  - Add repository/service for conversation CRUD and listing by updated time.
  - Wire conversation selection event to chat coordinator.

  **Must NOT do**:
  - Do not add context-menu power features beyond MVP (favorite/export/clone).

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: combines UI virtualization and persistence semantics.
  - **Skills**: `gpui-layout-and-style`, `gpui-entity`
    - `gpui-layout-and-style`: list/group rendering and virtual sizing.
    - `gpui-entity`: local state + repository synchronization.
  - **Skills Evaluated but Omitted**:
    - `gpui-action`: no advanced global shortcut logic required.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with 2/4/5/6)
  - **Blocks**: 7
  - **Blocked By**: 0,1

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/chat/sidebar.rs:70` - Sidebar state structure.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/sidebar.rs:133` - Date-group rebuild logic.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/sidebar.rs:565` - `v_virtual_list` usage for conversations.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/sidebar.rs:233` - Selection event emission pattern.

  **Acceptance Criteria**:
  - [ ] Conversation list renders grouped headers and selectable rows.
  - [ ] New conversation persists and appears after app restart.
  - [ ] `cargo test -p chat-app --test sidebar_conversations` passes.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Conversation create/select/persist works
    Tool: Bash
    Preconditions: sqlite-backed repository tests exist
    Steps:
      1. Run: cargo test -p chat-app --test sidebar_conversations create_select_persist -- --exact
      2. Assert: output contains "create_select_persist ... ok"
    Expected Result: create + select + reload all succeed
    Failure Indicators: conversation missing after reload
    Evidence: .sisyphus/evidence/task-3-sidebar-persist.log

  Scenario: Empty state is stable
    Tool: Bash
    Preconditions: empty database fixture exists
    Steps:
      1. Run: cargo test -p chat-app --test sidebar_conversations empty_db_shows_empty_state -- --exact
      2. Assert: output contains "... ok"
    Expected Result: no panic and deterministic empty-state rendering
    Failure Indicators: render panic or stale selection state
    Evidence: .sisyphus/evidence/task-3-sidebar-empty.log
  ```

  **Commit**: YES
  - Message: `feat(sidebar): add virtualized conversation list with persistence`
  - Files: `crates/chat-app/src/chat/sidebar.rs`, `crates/chat-app/src/database/**`
  - Pre-commit: `cargo test -p chat-app --test sidebar_conversations`

- [x] 4. Implement message list virtualization, markdown rendering, and scroll-follow manager

  **What to do**:
  - Build `MessageList` with `v_virtual_list`, dynamic size cache, and per-item rendering.
  - Implement `ScrollManager` behavior: follow bottom, stop following on manual scroll, resume near bottom.
  - Render assistant markdown with code-copy action and streaming/error indicators.

  **Must NOT do**:
  - Do not implement advanced markdown plugins (math/mermaid/custom blocks).

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: highest UI complexity and performance sensitivity in MVP.
  - **Skills**: `gpui-layout-and-style`, `gpui-test`
    - `gpui-layout-and-style`: virtual list + item composition.
    - `gpui-test`: deterministic checks for sizing/scroll logic.
  - **Skills Evaluated but Omitted**:
    - `frontend-ui-ux`: visual exploration is secondary to behavior correctness.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: 7
  - **Blocked By**: 0,1

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/chat/message_list.rs:157` - MessageList data model and caches.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/message_list.rs:634` - Virtual list render pipeline.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/message_list.rs:1247` - Markdown rendering with code actions.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/scroll_manager.rs:26` - Scroll-follow state machine.
  - `https://longbridge.github.io/gpui-component/docs/components/virtual-list` - Official virtual list patterns.

  **Acceptance Criteria**:
  - [ ] Message list supports >1000 messages without full-list rerender behavior.
  - [ ] Scroll-follow logic passes behavior tests (pause/resume rules).
  - [ ] `cargo test -p chat-app --test message_list_behavior` passes.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Long history virtualized rendering remains stable
    Tool: Bash
    Preconditions: message_list_behavior tests include long-history fixture
    Steps:
      1. Run: cargo test -p chat-app --test message_list_behavior long_history_virtualization -- --exact
      2. Assert: output contains "... ok"
    Expected Result: long histories render without panic/timeouts
    Failure Indicators: OOM/panic or failing assertion on visible range updates
    Evidence: .sisyphus/evidence/task-4-virtualization.log

  Scenario: Scroll-follow pauses when user scrolls up
    Tool: Bash
    Preconditions: scroll-follow unit/integration tests exist
    Steps:
      1. Run: cargo test -p chat-app --test message_list_behavior scroll_follow_pause_resume -- --exact
      2. Assert: output contains "... ok"
    Expected Result: auto-follow disables on manual up-scroll and resumes near bottom
    Failure Indicators: forced jump-to-bottom while user is reading history
    Evidence: .sisyphus/evidence/task-4-scroll-follow.log
  ```

  **Commit**: YES
  - Message: `feat(chat): add virtualized message list and scroll-follow manager`
  - Files: `crates/chat-app/src/chat/message_list.rs`, `crates/chat-app/src/chat/scroll_manager.rs`
  - Pre-commit: `cargo test -p chat-app --test message_list_behavior`

- [x] 5. Implement message input interactions (multiline, send/stop, shortcuts)

  **What to do**:
  - Build input with `InputState` + `auto_grow(1,10)`.
  - Implement Enter send, Shift+Enter newline, disabled send while streaming, explicit stop event.
  - Include minimal MVP toolbar state (non-functional toggles can be hidden for MVP clarity).

  **Must NOT do**:
  - Do not implement attachments in MVP.
  - Do not implement reasoning-level split button in MVP first pass.

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: focused interaction logic in single module.
  - **Skills**: `gpui-focus-handle`, `gpui-event`
    - `gpui-focus-handle`: keyboard/input focus behavior.
    - `gpui-event`: submit/stop event wiring.
  - **Skills Evaluated but Omitted**:
    - `gpui-global`: no global state required for this module.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: 7
  - **Blocked By**: 0,1

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/chat/message_input.rs:85` - InputState configuration pattern.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/message_input.rs:96` - Enter behavior interception.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/message_input.rs:358` - Send/stop button toggle pattern.
  - `https://longbridge.github.io/gpui-component/docs/components/input` - Input component behavior surface.

  **Acceptance Criteria**:
  - [ ] Enter submits non-empty input.
  - [ ] Shift+Enter inserts newline without submit.
  - [ ] Stop button emits stop event and returns input to editable state.
  - [ ] `cargo test -p chat-app --test message_input_behavior` passes.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Keyboard submit/newline behavior is deterministic
    Tool: Bash
    Preconditions: message_input_behavior tests include key-event simulation
    Steps:
      1. Run: cargo test -p chat-app --test message_input_behavior enter_vs_shift_enter -- --exact
      2. Assert: output contains "... ok"
    Expected Result: Enter submits, Shift+Enter creates newline
    Failure Indicators: accidental submit on Shift+Enter
    Evidence: .sisyphus/evidence/task-5-input-keys.log

  Scenario: Stop action exits loading state
    Tool: Bash
    Preconditions: stop-flow test exists
    Steps:
      1. Run: cargo test -p chat-app --test message_input_behavior stop_resets_loading_state -- --exact
      2. Assert: output contains "... ok"
    Expected Result: loading flag resets and send path is re-enabled
    Failure Indicators: stuck loading state after stop
    Evidence: .sisyphus/evidence/task-5-input-stop.log
  ```

  **Commit**: YES
  - Message: `feat(input): add multiline chat input with send and stop flows`
  - Files: `crates/chat-app/src/chat/message_input.rs`
  - Pre-commit: `cargo test -p chat-app --test message_input_behavior`

- [x] 6. Build Rig provider adapter layer and model catalog boundary

  **What to do**:
  - Implement provider trait abstraction for the app with a Rig-backed adapter.
  - Map Rig stream outputs to app stream events (`Delta`, `Done`, `Error`).
  - Implement model list fetch + cache strategy for configured provider.
  - Day-1 scope: one Rig provider route (default), architecture open for additional providers later.

  **Must NOT do**:
  - Do not create plugin architecture or multi-backend orchestration framework in MVP.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: provider boundary is critical to app correctness.
  - **Skills**: `gpui-async`, `gpui-entity`
    - `gpui-async`: stream task management and cancellation semantics.
    - `gpui-entity`: safe boundary integration with UI state.
  - **Skills Evaluated but Omitted**:
    - `rust-utoipa-axum-api`: no web API endpoints are being built.

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: 7,8
  - **Blocked By**: 0,2

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/llm/provider.rs:44` - Provider trait shape and stream contract baseline.
  - `/Users/isbset/Documents/ChatGPUI/src/llm/mod.rs:25` - Factory wiring from settings.
  - `/Users/isbset/Documents/ChatGPUI/src/llm/cache.rs:21` - Model cache TTL pattern.
  - `https://docs.rig.rs/docs/concepts/provider_clients` - Rig provider client abstraction.
  - `https://docs.rig.rs/docs/concepts/agent` - Rig prompt/stream concepts.

  **Acceptance Criteria**:
  - [ ] Rig adapter streams delta events and terminal events reliably.
  - [ ] Invalid API key path surfaces structured error event.
  - [ ] Model list fetch has cache/fallback behavior.
  - [ ] `cargo test -p chat-app --test rig_adapter_behavior` passes.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Adapter emits stream delta/done in order
    Tool: Bash
    Preconditions: rig adapter tests use deterministic mock stream
    Steps:
      1. Run: cargo test -p chat-app --test rig_adapter_behavior emits_ordered_stream_events -- --exact
      2. Assert: output contains "... ok"
    Expected Result: Delta* -> Done order is preserved
    Failure Indicators: Done emitted before Delta flush or missing terminal event
    Evidence: .sisyphus/evidence/task-6-rig-stream.log

  Scenario: Invalid credentials produce recoverable error event
    Tool: Bash
    Preconditions: invalid-credential fixture exists
    Steps:
      1. Run: cargo test -p chat-app --test rig_adapter_behavior invalid_key_surfaces_error -- --exact
      2. Assert: output contains "... ok"
    Expected Result: app receives Error event with non-empty message
    Failure Indicators: silent failure or panic
    Evidence: .sisyphus/evidence/task-6-rig-error.log
  ```

  **Commit**: YES
  - Message: `feat(llm): add Rig-backed provider adapter and model cache`
  - Files: `crates/chat-app/src/llm/**`
  - Pre-commit: `cargo test -p chat-app --test rig_adapter_behavior`

- [x] 7. Integrate chat orchestration (send/stream/debounce/stop/switch isolation)

  **What to do**:
  - Implement ChatView coordinator that links sidebar, message list, input, and provider adapter.
  - Add stream debounce window (target ~50ms) for rendering updates.
  - Enforce conversation/session isolation when switching active conversation mid-stream.
  - Define MVP default: switching conversation auto-cancels active stream for simplicity and correctness.

  **Must NOT do**:
  - Do not run concurrent active streams across multiple conversations in MVP.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: highest integration and race-condition risk.
  - **Skills**: `gpui-async`, `gpui-entity`
    - `gpui-async`: stream lifecycle and cancellation.
    - `gpui-entity`: safe updates across multiple entities.
  - **Skills Evaluated but Omitted**:
    - `gpui-layout-and-style`: integration logic, not visual layout, is primary.

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3 (sequential entry)
  - **Blocks**: 8,9
  - **Blocked By**: 2,3,4,5,6

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/chat/view.rs:790` - Response-generation flow anchor.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/view.rs:1240` - Debounce update scheduler pattern.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/view.rs:997` - Current-stream validity checks.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/view.rs:1097` - Stream finalization/error handling.

  **Acceptance Criteria**:
  - [ ] Send -> streaming -> done updates state and UI consistently.
  - [ ] Stop cancels active stream and leaves app in ready state.
  - [ ] Conversation switch during stream does not leak chunks into the new conversation.
  - [ ] `cargo test -p chat-app --test chat_orchestration` passes.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Happy-path streaming with debounce
    Tool: Bash
    Preconditions: orchestration tests include deterministic stream fixture
    Steps:
      1. Run: cargo test -p chat-app --test chat_orchestration streams_with_debounce -- --exact
      2. Assert: output contains "... ok"
    Expected Result: multiple deltas are batched and final content is complete
    Failure Indicators: excessive state churn or missing content tail
    Evidence: .sisyphus/evidence/task-7-stream-debounce.log

  Scenario: Switching conversation cancels current stream
    Tool: Bash
    Preconditions: cancellation-on-switch test exists
    Steps:
      1. Run: cargo test -p chat-app --test chat_orchestration switch_cancels_active_stream -- --exact
      2. Assert: output contains "... ok"
    Expected Result: old stream stops, new conversation remains clean
    Failure Indicators: chunk leakage to wrong conversation
    Evidence: .sisyphus/evidence/task-7-switch-isolation.log
  ```

  **Commit**: YES
  - Message: `feat(chat): integrate stream orchestration with debounce and isolation`
  - Files: `crates/chat-app/src/chat/view.rs`, related wiring files
  - Pre-commit: `cargo test -p chat-app --test chat_orchestration`

- [x] 8. Implement model selector and minimal settings integration

  **What to do**:
  - Build header model selector with popover list of provider/model options.
  - Add minimal settings page for provider credentials, base URL, and default model.
  - Persist settings and reload provider adapter on model/provider change.

  **Must NOT do**:
  - Do not implement advanced settings categories beyond MVP provider essentials.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
    - Reason: bounded UI composition with straightforward state persistence.
  - **Skills**: `gpui-layout-and-style`, `gpui-context`
    - `gpui-layout-and-style`: popover/list/form composition.
    - `gpui-context`: action dispatch and window integration.
  - **Skills Evaluated but Omitted**:
    - `gpui-action`: shortcut heavy work is not required here.

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3 (after Task 7 starts stabilizing)
  - **Blocks**: 9
  - **Blocked By**: 1,6,7

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/model_selector.rs:403` - Popover trigger/content pattern.
  - `/Users/isbset/Documents/ChatGPUI/src/model_selector.rs:425` - Provider/model selection update flow.
  - `/Users/isbset/Documents/ChatGPUI/src/settings/view.rs:1093` - Provider list + detail panel split.
  - `/Users/isbset/Documents/ChatGPUI/src/settings/state.rs:133` - Persisted settings structure.
  - `https://longbridge.github.io/gpui-component/docs/components/popover` - Popover composition guidance.
  - `https://longbridge.github.io/gpui-component/docs/components/select` - Select state/event model.

  **Acceptance Criteria**:
  - [ ] Changing provider/model updates active adapter without app restart.
  - [ ] Invalid config is surfaced as non-crashing error state.
  - [ ] `cargo test -p chat-app --test model_settings_flow` passes.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Model switch applies immediately
    Tool: Bash
    Preconditions: model_settings_flow tests include valid provider fixture
    Steps:
      1. Run: cargo test -p chat-app --test model_settings_flow switch_model_reloads_provider -- --exact
      2. Assert: output contains "... ok"
    Expected Result: active model/provider change propagates to send pipeline
    Failure Indicators: stale model used after switch
    Evidence: .sisyphus/evidence/task-8-model-switch.log

  Scenario: Model fetch failure falls back safely
    Tool: Bash
    Preconditions: fetch-failure fixture exists
    Steps:
      1. Run: cargo test -p chat-app --test model_settings_flow fetch_failure_uses_cached_or_static -- --exact
      2. Assert: output contains "... ok"
    Expected Result: selector remains usable with fallback data
    Failure Indicators: empty selector and blocked send flow
    Evidence: .sisyphus/evidence/task-8-model-fallback.log
  ```

  **Commit**: YES
  - Message: `feat(settings): add model selector and provider settings MVP`
  - Files: `crates/chat-app/src/model_selector.rs`, `crates/chat-app/src/settings/**`
  - Pre-commit: `cargo test -p chat-app --test model_settings_flow`

- [x] 9. End-to-end hardening, automated QA completion, and performance gate

  **What to do**:
  - Add integration tests for end-to-end MVP behavior matrix.
  - Add performance-focused test fixture for large message history and repeated streaming.
  - Run formatting, lint, and full test suite as release gate.

  **Must NOT do**:
  - Do not add new features in this task; only stabilization and verification.

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: final confidence gate across all modules.
  - **Skills**: `gpui-test`, `gpui-async`
    - `gpui-test`: robust coverage at integration level.
    - `gpui-async`: race/retry/cancellation edge-case validation.
  - **Skills Evaluated but Omitted**:
    - `new-component`: no new UI components should be introduced here.

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 4 (final)
  - **Blocks**: None
  - **Blocked By**: 7,8

  **References**:
  - `/Users/isbset/Documents/ChatGPUI/src/chat/view.rs:857` - Stream event processing lifecycle.
  - `/Users/isbset/Documents/ChatGPUI/src/chat/message_list.rs:525` - Visible-range measurement path.
  - `https://longbridge.github.io/gpui-component/docs/components/virtual-list` - Performance best practices.

  **Acceptance Criteria**:
  - [ ] `cargo fmt --all -- --check` -> exit code 0.
  - [ ] `cargo clippy --workspace --all-targets -- -D warnings` -> exit code 0.
  - [ ] `cargo test --workspace` -> exit code 0.
  - [ ] End-to-end behavior tests all pass for MVP matrix.

  **Agent-Executed QA Scenarios**:

  ```text
  Scenario: Full verification gate passes
    Tool: Bash
    Preconditions: all previous tasks merged
    Steps:
      1. Run: cargo fmt --all -- --check
      2. Assert: exit code is 0
      3. Run: cargo clippy --workspace --all-targets -- -D warnings
      4. Assert: exit code is 0
      5. Run: cargo test --workspace
      6. Assert: output contains "test result: ok"
    Expected Result: codebase is lint-clean and test-clean
    Failure Indicators: warnings, failed tests, format drift
    Evidence: .sisyphus/evidence/task-9-full-gate.log

  Scenario: Performance fixture remains within threshold
    Tool: Bash
    Preconditions: perf-oriented integration test exists
    Steps:
      1. Run: cargo test -p chat-app --test perf_message_virtualization handles_2k_messages -- --exact
      2. Assert: output contains "... ok"
    Expected Result: virtualization path remains stable under large history
    Failure Indicators: timeout/panic/excessive memory-related failures
    Evidence: .sisyphus/evidence/task-9-perf.log
  ```

  **Commit**: YES
  - Message: `test(chat): finalize MVP verification matrix and quality gates`
  - Files: `crates/chat-app/tests/**`, any minor stabilization edits
  - Pre-commit: full gate commands above

---

## Commit Strategy

| After Task | Message | Files | Verification |
|------------|---------|-------|--------------|
| 0 | `chore(workspace): bootstrap chat app crate and test scaffold` | workspace + crate scaffold | `cargo check --workspace && cargo test --workspace` |
| 1 | `feat(shell): initialize root window and split layout scaffold` | `src/main.rs`, `src/app.rs` | shell boot test |
| 2 | `feat(core): define chat domain and event contracts` | `src/chat/message.rs`, `src/chat/events.rs` | domain transition tests |
| 3 | `feat(sidebar): add virtualized conversation list with persistence` | `src/chat/sidebar.rs`, `src/database/**` | sidebar tests |
| 4 | `feat(chat): add virtualized message list and scroll-follow manager` | `src/chat/message_list.rs`, `src/chat/scroll_manager.rs` | message list tests |
| 5 | `feat(input): add multiline chat input with send and stop flows` | `src/chat/message_input.rs` | input behavior tests |
| 6 | `feat(llm): add Rig-backed provider adapter and model cache` | `src/llm/**` | rig adapter tests |
| 7 | `feat(chat): integrate stream orchestration with debounce and isolation` | `src/chat/view.rs` + wiring | orchestration tests |
| 8 | `feat(settings): add model selector and provider settings MVP` | `src/model_selector.rs`, `src/settings/**` | settings flow tests |
| 9 | `test(chat): finalize MVP verification matrix and quality gates` | `tests/**` + stabilization | fmt/clippy/test full gate |

---

## Success Criteria

### Verification Commands

```bash
cargo fmt --all -- --check
# Expected: exit code 0

cargo check --workspace
# Expected: exit code 0

cargo clippy --workspace --all-targets -- -D warnings
# Expected: exit code 0

cargo test --workspace
# Expected: all tests pass, output contains "test result: ok"
```

### Final Checklist
- [x] All "Must Have" capabilities are implemented.
- [x] All "Must NOT Have" features remain absent from MVP.
- [x] Provider integration is Rig-backed through a thin adapter boundary.
- [x] No manual verification is required for acceptance.
- [x] Streaming isolation, debounce, and stop behavior are all test-covered.
