use std::path::PathBuf;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::notification::NotificationList;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::chat::{ChatSidebar, ChatView};

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
const WINDOW_TOOLBAR_HEIGHT: f32 = 48.0;
#[cfg(target_os = "macos")]
const WINDOW_TOOLBAR_LEFT_SAFE_PADDING: f32 = 78.0;
#[cfg(not(target_os = "macos"))]
const WINDOW_TOOLBAR_LEFT_SAFE_PADDING: f32 = 16.0;
#[cfg(target_os = "windows")]
const WINDOW_TOOLBAR_RIGHT_SAFE_PADDING: f32 = 120.0;
#[cfg(not(target_os = "windows"))]
const WINDOW_TOOLBAR_RIGHT_SAFE_PADDING: f32 = 16.0;
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
            .child(
                v_flex()
                    .size_full()
                    .child(
                        h_flex()
                            .id("app-shell-body")
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .pt(px(WINDOW_TOOLBAR_HEIGHT))
                            .overflow_hidden()
                            .child(self.render_sidebar(sidebar, cx))
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
                    .child(self.render_bottom_bar(cx)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(self.render_top_bar(cx)),
            )
            .child(self.notification_list.clone())
    }
}

impl ChatAppShell {
    fn render_collapsed_sidebar(&self, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .id("collapsed-sidebar")
            .size_full()
            .items_center()
            .justify_start()
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
            .into_any_element()
    }

    fn render_top_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (provider_id, model_selector) = {
            let chat_view = self.chat_view.read(cx);
            (
                chat_view.resolved_provider_id(cx),
                chat_view.model_selector().clone(),
            )
        };

        h_flex()
            .id("app-top-bar")
            .w_full()
            .h(px(WINDOW_TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .pl(px(WINDOW_TOOLBAR_LEFT_SAFE_PADDING))
            .pr(px(WINDOW_TOOLBAR_RIGHT_SAFE_PADDING))
            .items_center()
            .justify_end()
            .bg(theme.background)
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .id("chat-view-provider-id")
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .bg(theme.muted)
                            .border_1()
                            .border_color(theme.border)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(provider_id),
                    )
                    .child(model_selector),
            )
    }

    fn render_bottom_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .id("app-bottom-bar")
            .w_full()
            .flex_shrink_0()
            .items_center()
            .border_t_1()
            .border_color(theme.border)
            .child(self.render_bottom_sidebar_controls(cx))
            .child(div().id("app-bottom-main-spacer").flex_1().min_w_0())
    }

    fn render_bottom_sidebar_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .id("app-bottom-sidebar-controls")
            .w(px(SIDEBAR_DEFAULT_WIDTH))
            .h_full()
            .flex_shrink_0()
            .items_center()
            .justify_start()
            .gap_1()
            .px_3()
            .py_1()
            .child(
                Button::new("sidebar-toggle")
                    .ghost()
                    .small()
                    .icon(IconName::PanelLeftClose)
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.toggle_sidebar(cx);
                    })),
            )
            .child(
                Button::new("sidebar-settings")
                    .ghost()
                    .small()
                    .icon(IconName::Settings)
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.open_settings(cx);
                    })),
            )
            .child(
                div()
                    .id("sidebar-user-center")
                    .size(px(28.))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::CircleUser)
                            .size(px(16.))
                            .text_color(theme.foreground),
                    ),
            )
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
