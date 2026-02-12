use gpui::{Bounds, Pixels, Point, Size, point, px};
use gpui_component::VirtualListScrollHandle;

/// Near-bottom distance used to resume follow mode deterministically.
const AUTO_FOLLOW_RESUME_THRESHOLD: Pixels = px(24.);
/// Small delta used to ignore floating-point scroll jitter.
const SCROLL_DELTA_EPSILON: f32 = 1.0;

/// Manages virtual-list follow behavior independent from message content.
pub struct ScrollManager {
    scroll_handle: VirtualListScrollHandle,
    pending_scroll_to_bottom: bool,
    follow_bottom: bool,
    last_scroll_offset: Pixels,
    last_max_offset: Pixels,
}

impl ScrollManager {
    pub fn new() -> Self {
        Self {
            scroll_handle: VirtualListScrollHandle::new(),
            pending_scroll_to_bottom: false,
            follow_bottom: true,
            last_scroll_offset: Pixels::ZERO,
            last_max_offset: Pixels::ZERO,
        }
    }

    pub fn handle(&self) -> &VirtualListScrollHandle {
        &self.scroll_handle
    }

    pub fn is_following_bottom(&self) -> bool {
        self.follow_bottom
    }

    pub fn request_scroll_to_bottom(&mut self) {
        self.pending_scroll_to_bottom = true;
        self.follow_bottom = true;
    }

    pub fn request_scroll_to_bottom_if_following(&mut self) {
        if self.follow_bottom || self.was_near_bottom() {
            self.pending_scroll_to_bottom = true;
        }
    }

    pub fn reset(&mut self) {
        self.last_scroll_offset = Pixels::ZERO;
        self.last_max_offset = Pixels::ZERO;
        self.follow_bottom = true;
        self.pending_scroll_to_bottom = true;
    }

    pub fn update_follow_state(&mut self) {
        let offset = self.scroll_handle.offset().y;
        let max_offset = self.scroll_handle.max_offset().height;
        let offset_delta = f32::from(offset) - f32::from(self.last_scroll_offset);
        let max_delta = (f32::from(max_offset) - f32::from(self.last_max_offset)).abs();
        let content_size_changed = max_delta > SCROLL_DELTA_EPSILON;
        let user_scrolled_up = offset_delta > SCROLL_DELTA_EPSILON && !content_size_changed;
        let user_scrolled_down = offset_delta < -SCROLL_DELTA_EPSILON && !content_size_changed;

        // Keep follow mode enabled while we are fulfilling an explicit follow request.
        if self.pending_scroll_to_bottom || (content_size_changed && self.was_near_bottom()) {
            self.follow_bottom = true;
        } else if self.follow_bottom {
            // Pause follow mode only when the user manually scrolls away from the tail.
            if user_scrolled_up {
                self.follow_bottom = false;
            }
        } else if user_scrolled_down && self.is_near_bottom() {
            // Resume follow mode once user intentionally returns near the bottom boundary.
            self.follow_bottom = true;
        }

        self.last_scroll_offset = offset;
        self.last_max_offset = max_offset;
    }

    pub fn apply_pending_scroll(&mut self) -> bool {
        let should_scroll = self.follow_bottom || self.pending_scroll_to_bottom;

        if should_scroll {
            let max_offset = self.scroll_handle.max_offset().height;
            let current_x = self.scroll_handle.offset().x;
            let target_y = if max_offset > Pixels::ZERO {
                -max_offset
            } else {
                Pixels::ZERO
            };
            self.scroll_handle.set_offset(point(current_x, target_y));
        }

        self.pending_scroll_to_bottom = false;
        should_scroll
    }

    pub fn bounds(&self) -> Bounds<Pixels> {
        self.scroll_handle.bounds()
    }

    pub fn offset(&self) -> Point<Pixels> {
        self.scroll_handle.offset()
    }

    pub fn max_offset(&self) -> Size<Pixels> {
        self.scroll_handle.max_offset()
    }

    fn is_near_bottom(&self) -> bool {
        let max_offset = self.scroll_handle.max_offset().height;
        if max_offset <= Pixels::ZERO {
            return true;
        }

        // GPUI uses negative Y offsets for scrolling down, so `offset + max` approaches 0 at tail.
        let offset = self.scroll_handle.offset().y;
        (offset + max_offset).abs() <= AUTO_FOLLOW_RESUME_THRESHOLD
    }

    fn was_near_bottom(&self) -> bool {
        let max_offset = self.last_max_offset;
        if max_offset <= Pixels::ZERO {
            return true;
        }

        let offset = self.last_scroll_offset;
        (offset + max_offset).abs() <= AUTO_FOLLOW_RESUME_THRESHOLD
    }
}

impl Default for ScrollManager {
    fn default() -> Self {
        Self::new()
    }
}
