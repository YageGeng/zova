#![deny(unsafe_code)]

/// Chat application shell and components.
///
/// This crate provides a desktop chat application built with GPUI and gpui-component.
/// The current implementation provides the app shell with theme pipeline and layout scaffold.
pub mod app;
/// Chat domain contracts shared across UI modules.
pub mod chat;
pub mod database;
pub mod llm;
/// Model selector component for changing LLM models.
pub mod model_selector;
/// Settings persistence and UI.
pub mod settings;
/// Returns a stable marker used by integration smoke tests.
pub fn smoke_marker() -> &'static str {
    "zova"
}
