# Decisions

- 2026-02-12: Kept workspace membership pattern as `members = ["crates/*"]` and added shared `[workspace.dependencies]` for `gpui`, `gpui-component`, `gpui-component-assets`, `rig-core`, `tokio`, and `core-text`.
- 2026-02-12: Added `[patch.crates-io] gpui = { git = "https://github.com/zed-industries/zed" }` to keep GPUI sources aligned with gpui-component git dependencies.
- 2026-02-12: Chose a minimal bootstrap binary with `BootstrapView` plus a deterministic integration smoke test, deferring all UI shell behavior to Task 1.
- 2026-02-12: Keep provider layer as a thin Rig adapter plus app-level stream event mapping (not a plugin framework) for MVP scope control.
- 2026-02-12: Follow ChatGPUI conversation architecture: active `ChatView` orchestrates sidebar/input/message-list entities with typed event boundaries.
- 2026-02-12: Preserve gpui-component-first approach; custom low-level element work is deferred unless a concrete blocker appears.
- 2026-02-12: Use dependency-free typed ID newtypes (`ConversationId`, `MessageId`, `StreamSessionId`) for domain contracts in Task 2 to avoid adding crates while preserving compile-time boundary safety.
- 2026-02-12: Keep stream lifecycle as a domain state machine (`StreamState` + `StreamTransition`) and map only terminal mapped stream events into transitions; non-terminal deltas stay buffer-level concerns.
- 2026-02-12: Implement Task 3 persistence as a minimal file-backed `ConversationStore` (`.chat-app/conversations.tsv`) with SNAFU-typed errors, avoiding new dependencies and staying within MVP scope.
- 2026-02-12: Classify sidebar groups with elapsed-time buckets (`<24h` Today, `<48h` Yesterday, otherwise Older) to keep grouping deterministic without introducing timezone/date crates.
- 2026-02-12: Task 4 keeps a minimal `MessageList` API (`set_messages`, explicit scroll reset/request methods) and hides scroll-follow internals inside `ScrollManager` so Task 7 orchestration can drive state without coupling to virtualization details.
- 2026-02-12: Assistant markdown rendering uses `TextView::markdown` plus code-block copy actions, with a deterministic plain-text fallback for oversized payloads to keep MVP rendering safe.
