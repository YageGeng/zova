use std::path::PathBuf;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::notification::NotificationList;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::chat::{ChatSidebar, ChatView, SidebarToggleClicked};

/// Returns the default themes directory path.
/// This is a pure function to allow deterministic testing of path resolution.
pub fn default_themes_path() -> PathBuf {
    PathBuf::from("./themes")
}

/// Default sidebar width when expanded.
pub const SIDEBAR_DEFAULT_WIDTH: f32 = 260.0;
/// Minimum allowed sidebar width.
pub const SIDEBAR_MIN_WIDTH: f32 = 200.0;
/// Maximum allowed sidebar width.
pub const SIDEBAR_MAX_WIDTH: f32 = 400.0;
pub const SIDEBAR_COLLAPSED_WIDTH: f32 = 64.0;
/// Compile-time validation of sidebar layout constraints.
/// These assertions ensure the constants maintain valid relationships.
const _: () = {
    assert!(SIDEBAR_COLLAPSED_WIDTH > 0.0);
    assert!(SIDEBAR_MIN_WIDTH < SIDEBAR_DEFAULT_WIDTH);
    assert!(SIDEBAR_DEFAULT_WIDTH < SIDEBAR_MAX_WIDTH);
    assert!(SIDEBAR_MIN_WIDTH > 0.0);
};

/// Computes the effective sidebar width given a drag position.
/// The result is clamped to [SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH].
///
/// # Arguments
/// * `drag_x` - The x-coordinate from a drag event (pixels from window left)
///
/// # Returns
/// The clamped width value suitable for sidebar sizing.
pub fn compute_sidebar_width(drag_x: f32) -> f32 {
    drag_x.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}

gpui::actions!(shell, [NewChat, ToggleSidebar, Quit,]);

/// Marker type for sidebar resize drag operations.
/// Used to identify drag events specific to the resize handle.
#[derive(Clone)]
struct SidebarResizeDrag;

/// Empty drag visual used during sidebar resize.
/// The drag preview itself is invisible; only the cursor changes.
struct EmptyDragView;

impl Render for EmptyDragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Main application shell that manages the root layout.
///
/// The shell provides:
/// - A collapsible sidebar (placeholder for conversation list)
/// - A resize handle for adjusting sidebar width
/// - A main content area (placeholder for chat view)
/// - Toolbar buttons for quick actions
/// - Notification layer for toasts
pub struct ChatAppShell {
    /// Notification list entity for displaying toasts.
    notification_list: Entity<NotificationList>,
    chat_view: Entity<ChatView>,
    /// Whether the sidebar is currently collapsed.
    sidebar_collapsed: bool,
    /// Current width of the sidebar when expanded.
    sidebar_width: f32,
}

impl ChatAppShell {
    /// Creates a new shell with the given notification list.
    ///
    /// # Arguments
    /// * `notification_list` - The entity that manages notification toasts
    /// * `_window` - Window context (unused but reserved for future use)
    /// * `_cx` - GPUI context (unused but reserved for future use)
    pub fn new(
        notification_list: Entity<NotificationList>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let chat_view = cx.new(|cx| ChatView::new(window, cx));

        cx.subscribe(&chat_view, |this, _, _event: &SidebarToggleClicked, cx| {
            this.toggle_sidebar(cx);
        })
        .detach();

        Self {
            notification_list,
            chat_view,
            sidebar_collapsed: false,
            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
        }
    }

    /// Toggles the sidebar between collapsed and expanded states.
    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    /// Resizes the sidebar to the specified width, clamped to min/max bounds.
    ///
    /// # Arguments
    /// * `new_width` - The desired width in pixels
    fn resize_sidebar(&mut self, new_width: f32, cx: &mut Context<Self>) {
        self.sidebar_width = compute_sidebar_width(new_width);
        cx.notify();
    }

    /// Handles the new chat action.
    fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.chat_view
            .update(cx, |chat_view, cx| chat_view.create_conversation(cx));
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.chat_view
            .update(cx, |chat_view, cx| chat_view.open_settings_panel(cx));
    }
}

impl Render for ChatAppShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let collapsed = self.sidebar_collapsed;
        let sidebar = self.chat_view.read(cx).sidebar().clone();

        div()
            .size_full()
            .relative()
            .bg(theme.background)
            // Main horizontal layout: sidebar + resize handle + main content
            .child(
                h_flex()
                    .size_full()
                    .child(self.render_sidebar(sidebar, cx))
                    // Resize handle only visible when sidebar is expanded
                    .when(!collapsed, |el| el.child(self.render_resize_handle(cx)))
                    .child(
                        v_flex()
                            .id("main-content")
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .child(self.chat_view.clone()),
                    ),
            )
            // Notification layer for toast messages
            .child(self.notification_list.clone())
    }
}

impl ChatAppShell {
    fn render_collapsed_sidebar(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();

        v_flex()
            .id("collapsed-sidebar")
            .size_full()
            .items_center()
            .justify_between()
            .py_3()
            .px_2()
            .child(
                v_flex().items_center().gap_2().child(
                    Button::new("new-chat-collapsed")
                        .ghost()
                        .small()
                        .icon(IconName::Plus)
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.new_chat(cx);
                        })),
                ),
            )
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("collapsed-user-center")
                            .size(px(32.))
                            .rounded_full()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.muted)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::CircleUser)
                                    .size(px(18.))
                                    .text_color(theme.foreground),
                            ),
                    )
                    .child(
                        v_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("sidebar-settings-collapsed")
                                    .ghost()
                                    .small()
                                    .icon(IconName::Settings)
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.open_settings(cx);
                                    })),
                            )
                            .child(
                                Button::new("toggle-sidebar-collapsed")
                                    .ghost()
                                    .small()
                                    .icon(IconName::PanelLeftOpen)
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_sidebar(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_sidebar(&self, sidebar: Entity<ChatSidebar>, cx: &Context<Self>) -> impl IntoElement {
        let collapsed = self.sidebar_collapsed;
        let expanded_width = self.sidebar_width;
        let sidebar_width = if collapsed {
            SIDEBAR_COLLAPSED_WIDTH
        } else {
            expanded_width
        };
        let sidebar_content = if collapsed {
            self.render_collapsed_sidebar(cx)
        } else {
            sidebar.into_any_element()
        };
        let theme = cx.theme();

        div()
            .id("sidebar-container")
            .h_full()
            .min_w_0()
            .flex_shrink_0()
            .w(px(sidebar_width))
            .overflow_hidden()
            .bg(theme.background)
            .border_r_1()
            .border_color(theme.border)
            .child(sidebar_content)
    }

    /// Renders the resize handle for adjusting sidebar width.
    ///
    /// The handle is a thin vertical line that shows a resize cursor on hover
    /// and allows dragging to adjust the sidebar width.
    fn render_resize_handle(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("sidebar-resize-handle")
            .w(px(1.0))
            .h_full()
            .flex_shrink_0()
            .cursor(CursorStyle::ResizeLeftRight)
            .bg(theme.border)
            .hover(|el| el.bg(theme.primary))
            // Start drag operation with an invisible drag view
            .on_drag(SidebarResizeDrag, |_, _, _, cx| cx.new(|_| EmptyDragView))
            // Handle drag movement to resize the sidebar
            .on_drag_move::<SidebarResizeDrag>(cx.listener(
                |this, event: &DragMoveEvent<SidebarResizeDrag>, _window, cx| {
                    // Use the x position of the drag event as the new sidebar width
                    let new_width: f32 = event.event.position.x.into();
                    this.resize_sidebar(new_width, cx);
                },
            ))
    }
}
