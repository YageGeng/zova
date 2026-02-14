use gpui::*;
use gpui_component::notification::NotificationList;
use gpui_component::{Root, ThemeRegistry};

use ui::app::{ChatAppShell, NewChat, Quit, ToggleSidebar, default_themes_path};
use ui::settings::state::SettingsStore;

/// Application entry point.
///
/// Bootstraps the GPUI application with:
/// 1. Asset loading via gpui-component-assets
/// 2. gpui-component initialization (required for Root, themes, notifications)
/// 3. Theme loading/watching from ./themes directory (non-fatal if missing)
/// 4. Global action handlers for shell-level commands
/// 5. Window creation with Root wrapper for gpui-component composition
fn main() {
    // Initialize tracing for development debugging
    tracing_subscriber::fmt::init();

    // Create application with bundled assets
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(|cx| {
        gpui_tokio_bridge::init(cx);

        // Initialize gpui-component - REQUIRED before any Root usage
        // This sets up the theme system, notification layer, and component registry
        gpui_component::init(cx);

        // Attempt to load and watch themes from ./themes directory
        // This is non-fatal: if the directory doesn't exist or is empty,
        // the app falls back to default built-in themes
        if let Err(err) = ThemeRegistry::watch_dir(default_themes_path(), cx, |_cx| {
            let settings_store = SettingsStore::load();
            settings_store.settings().apply_theme(None, _cx);
            tracing::info!("Theme directory watch initialized");
        }) {
            tracing::warn!(
                "Failed to watch themes directory: {}. Using default themes.",
                err
            );
            let settings_store = SettingsStore::load();
            settings_store.settings().apply_theme(None, cx);
        }

        // Register global action handlers
        // Quit action: cleanly shut down the application
        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });

        // Global keyboard shortcuts
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-n", NewChat, None),
            KeyBinding::new("cmd-b", ToggleSidebar, None),
        ]);

        // Spawn async window creation to ensure all initialization is complete
        cx.spawn(async move |cx| {
            cx.update(|cx| {
                // Window options with reasonable defaults for a chat app
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1200.), px(800.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        // Align traffic lights with Zed-style top titlebar inset.
                        traffic_light_position: Some(point(px(9.), px(9.))),
                        ..Default::default()
                    }),
                    // Match Zed-style client decorations on Linux/FreeBSD so the app draws
                    // its own title area instead of showing a system titlebar.
                    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                    window_decorations: Some(WindowDecorations::Client),
                    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
                    window_decorations: None,
                    ..Default::default()
                };

                // Open the main window with Root wrapper
                // Root is REQUIRED by gpui-component for notifications/dialogs/sheets
                cx.open_window(options, |window, cx| {
                    // Create notification list first (shared across shell)
                    let notification_list = cx.new(|cx| NotificationList::new(window, cx));

                    // Create the shell view
                    let shell = cx.new(|cx| ChatAppShell::new(notification_list, window, cx));

                    // Wrap in Root for gpui-component integration
                    cx.new(|cx| Root::new(shell, window, cx))
                })
                .expect("failed to open main window");

                // Activate the application
                cx.activate(true);
            })
        })
        .detach();
    });
}
