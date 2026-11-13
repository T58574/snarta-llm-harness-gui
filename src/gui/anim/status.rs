use super::{DUR_BREATHING, DUR_PULSE};
use gpui::*;
use gpui_component::h_flex;

/// Renders a server online indicator with a gentle synchronized breathing green glow.
pub fn render_server_online_dot(id: &'static str) -> impl IntoElement {
    let green = hsla(142.0 / 360.0, 0.72, 0.45, 1.0);
    h_flex()
        .items_center()
        .justify_center()
        .w(px(14.0))
        .h(px(14.0))
        .child(
            div()
                .w(px(7.0))
                .h(px(7.0))
                .rounded_full()
                .bg(green)
                .with_animation(
                    id,
                    Animation::new(DUR_BREATHING)
                        .repeat()
                        .with_easing(bounce(ease_in_out)),
                    move |dot, delta| {
                        let alpha = 0.35 + delta * 0.65;
                        dot.opacity(alpha)
                    },
                ),
        )
}

/// Applies an alert breathing pulse to the VRAM badge when VRAM is near capacity or spilling into RAM.
pub fn apply_vram_pulse<E: Styled + IntoElement + 'static>(
    element: E,
    is_danger: bool,
    danger_color: Hsla,
) -> AnyElement {
    if !is_danger {
        return element.into_any_element();
    }

    element
        .with_animation(
            "vram-danger-pulse",
            Animation::new(DUR_PULSE)
                .repeat()
                .with_easing(bounce(ease_in_out)),
            move |el, delta| {
                let alpha = 0.35 + delta * 0.65;
                el.border_color(danger_color.opacity(alpha))
            },
        )
        .into_any_element()
}
