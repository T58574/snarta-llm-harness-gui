use super::{DUR_BREATHING, DUR_PULSE};
use crate::gui::theme::COLOR_PINK_PRIMARY;
use gpui::*;
use gpui_component::h_flex;

/// Renders a neon pink terminal cursor with synchronized breathing opacity for streaming states.
pub fn render_breathing_cursor(id: &'static str) -> impl IntoElement {
    div()
        .w(px(7.0))
        .h(px(16.0))
        .rounded_sm()
        .bg(COLOR_PINK_PRIMARY)
        .with_animation(
            id,
            Animation::new(DUR_PULSE)
                .repeat()
                .with_easing(bounce(ease_in_out)),
            |cursor, delta| {
                // Smooth breathing opacity between 15% and 100%
                let alpha = 0.15 + delta * 0.85;
                cursor.opacity(alpha)
            },
        )
}

/// Applies a subtle pulsating accent glow to a container element during generation.
pub fn apply_streaming_glow<E: Styled + IntoElement + 'static>(
    element: E,
    id: &'static str,
    is_active: bool,
) -> AnyElement {
    if !is_active {
        return element.into_any_element();
    }

    element
        .with_animation(
            id,
            Animation::new(DUR_BREATHING)
                .repeat()
                .with_easing(bounce(ease_in_out)),
            |el, delta| {
                let border_alpha = 0.4 + delta * 0.6;
                el.border_color(COLOR_PINK_PRIMARY.opacity(border_alpha))
            },
        )
        .into_any_element()
}

/// Renders a 3-dot thinking indicator with animated wave pulsation for reasoning phases.
pub fn render_thinking_dots(_id: &'static str) -> impl IntoElement {
    h_flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded_full()
                .bg(COLOR_PINK_PRIMARY)
                .with_animation(
                    "think-dot-1",
                    Animation::new(DUR_PULSE)
                        .repeat()
                        .with_easing(bounce(ease_in_out)),
                    |dot, delta| dot.opacity(0.2 + delta * 0.8),
                ),
        )
        .child(
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded_full()
                .bg(COLOR_PINK_PRIMARY)
                .with_animation(
                    "think-dot-2",
                    Animation::new(DUR_PULSE)
                        .repeat()
                        .with_easing(bounce(ease_in_out)),
                    |dot, delta| {
                        // Offset phase by 33%
                        let phase = (delta + 0.33) % 1.0;
                        dot.opacity(0.2 + phase * 0.8)
                    },
                ),
        )
        .child(
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded_full()
                .bg(COLOR_PINK_PRIMARY)
                .with_animation(
                    "think-dot-3",
                    Animation::new(DUR_PULSE)
                        .repeat()
                        .with_easing(bounce(ease_in_out)),
                    |dot, delta| {
                        // Offset phase by 66%
                        let phase = (delta + 0.66) % 1.0;
                        dot.opacity(0.2 + phase * 0.8)
                    },
                ),
        )
}
