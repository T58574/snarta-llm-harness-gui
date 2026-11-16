use crate::gui::theme::{COLOR_ACCENT_ORANGE, COLOR_TEXT_MUTED};
use gpui::*;
use gpui_component::progress::Progress;
use gpui_component::spinner::Spinner;
use gpui_component::{Sizable, Size, h_flex};

/// Renders a status badge with an active spinning loader for tools during `is_streaming: true`.
pub fn render_tool_status_indicator(is_streaming: bool) -> impl IntoElement {
    h_flex()
        .items_center()
        .gap_1p5()
        .child(if is_streaming {
            Spinner::new()
                .with_size(Size::XSmall)
                .color(COLOR_ACCENT_ORANGE)
                .into_any_element()
        } else {
            div().into_any_element()
        })
        .child(
            div()
                .text_size(rems(0.72))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_streaming {
                    COLOR_ACCENT_ORANGE
                } else {
                    COLOR_TEXT_MUTED
                })
                .child(if is_streaming {
                    "Выполняется..."
                } else {
                    "Готово"
                }),
        )
}

/// Renders a horizontal indeterminate progress bar for active tool execution.
pub fn render_tool_progress_bar(id: impl Into<ElementId>, is_streaming: bool) -> AnyElement {
    if !is_streaming {
        return div().into_any_element();
    }

    Progress::new(id)
        .loading(true)
        .with_size(Size::XSmall)
        .color(COLOR_ACCENT_ORANGE)
        .w_full()
        .into_any_element()
}
