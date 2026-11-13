use super::DUR_AMBIENT;
use crate::gui::theme::{COLOR_PINK_MUTED_BG, COLOR_PINK_PRIMARY};
use gpui::*;
use gpui_component::v_flex;

/// Renders a stylish breathing ambient emblem for the empty chat state.
pub fn render_ambient_emblem(id: &'static str) -> impl IntoElement {
    v_flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(64.0))
                .h(px(64.0))
                .rounded_xl()
                .border_1()
                .border_color(COLOR_PINK_PRIMARY)
                .bg(COLOR_PINK_MUTED_BG)
                .items_center()
                .justify_center()
                .text_size(rems(1.8))
                .font_weight(FontWeight::BOLD)
                .text_color(COLOR_PINK_PRIMARY)
                .child("S")
                .with_animation(
                    id,
                    Animation::new(DUR_AMBIENT)
                        .repeat()
                        .with_easing(bounce(ease_in_out)),
                    |emblem, delta| {
                        let border_alpha = 0.35 + delta * 0.65;
                        let bg_alpha = 0.08 + delta * 0.22;
                        let opacity = 0.70 + delta * 0.30;
                        emblem
                            .border_color(COLOR_PINK_PRIMARY.opacity(border_alpha))
                            .bg(COLOR_PINK_PRIMARY.opacity(bg_alpha))
                            .opacity(opacity)
                    },
                ),
        )
}
