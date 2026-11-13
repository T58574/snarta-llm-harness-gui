use crate::chat::session::ToolTrace;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_INPUT,
    COLOR_PINK_MUTED_BG, COLOR_PINK_PRIMARY, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{IconName, Sizable, Size, h_flex, v_flex};
use serde_json::Value;

/// Renders a graphical console card for shell and command execution tools (`exec`, `shell_start`, `shell_output`, `shell_kill`).
pub fn render_shell_card(trace: &ToolTrace, _is_streaming: bool) -> AnyElement {
    let args: Value = serde_json::from_str(&trace.arguments).unwrap_or(Value::Null);

    let cmd = args
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let job_id = args
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let (badge_label, title_text) = match trace.name.as_str() {
        "exec" => ("$ ТЕРМИНАЛ", cmd.to_string()),
        "shell_start" => ("ФОНОВАЯ КОМАНДА", cmd.to_string()),
        "shell_output" => ("ВЫВОД ФОНА", format!("Задача: {job_id}")),
        "shell_kill" => ("ОСТАНОВКА", format!("Задача: {job_id}")),
        _ => ("ШЕЛЛ", cmd.to_string()),
    };

    let copy_cmd = cmd.to_string();

    v_flex()
        .w_full()
        .p_2p5()
        .rounded_lg()
        .bg(COLOR_GRAPHITE_CANVAS)
        .border_1()
        .border_color(COLOR_GRAPHITE_BORDER)
        .gap_1p5()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(COLOR_PINK_MUTED_BG)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_PINK_PRIMARY)
                                .child(badge_label),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(rems(0.8))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(title_text),
                        ),
                )
                .children((!copy_cmd.is_empty()).then(|| {
                    Button::new(format!("copy-cmd-{}", copy_cmd.chars().take(8).collect::<String>()))
                        .ghost()
                        .with_size(Size::XSmall)
                        .icon(IconName::Copy)
                        .on_click(move |_, _window, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_cmd.clone()));
                        })
                })),
        )
        .children((!trace.output.is_empty()).then(|| {
            div()
                .max_h(rems(12.0))
                .overflow_hidden()
                .p_2()
                .rounded_md()
                .bg(COLOR_GRAPHITE_INPUT)
                .border_1()
                .border_color(COLOR_GRAPHITE_BORDER)
                .text_size(rems(0.75))
                .text_color(COLOR_TEXT_MUTED)
                .child(trace.output.clone())
        }))
        .into_any_element()
}
