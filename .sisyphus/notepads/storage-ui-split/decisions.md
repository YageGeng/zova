# Decisions

- 2026-02-13: Added a standalone workspace crate `crates/storage` (`package = "zova-storage"`) and moved all storage domain files there.
- 2026-02-13: Removed `chat-app` local `storage` module export and rewired UI imports to the external `zova_storage` crate.
- 2026-02-13: Replaced storage dependency on `crate::chat::Role` with storage-local `MessageRole`, with explicit conversion at the UI boundary (`sidebar.rs` and `view.rs`).
- 2026-02-13: Replaced storage dependency on `crate::database::DEFAULT_CONVERSATION_TITLE` with storage-local `DEFAULT_SESSION_TITLE` to keep legacy import behavior identical without UI coupling.
- 2026-02-13: Renamed UI crate directory to `crates/ui` and package name to `ui`, while keeping storage crate identity/path (`zova-storage`, `crates/storage`) unchanged.
- 2026-02-13: Preserved runtime data-location strings (for example `.zova`) during crate/package rename to avoid behavior changes beyond module naming.
