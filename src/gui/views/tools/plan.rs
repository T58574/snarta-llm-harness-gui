use crate::chat::session::ToolTrace;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_ELEVATED,
    COLOR_PINK_MUTED_BG, COLOR_PINK_PRIMARY, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use gpui::*;
use gpui_component::{h_flex, v_flex};
use serde_json::Value;

/// Renders a graphical card for `plan_mode` tool calls.
pub fn render_plan_mode(trace: &ToolTrace, _is_streaming: bool) -> AnyElement {
    let args: Value = serde_json::from_str(&trace.arguments).unwrap_or(Value::Null);
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let summary = args
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let is_enter = action == "enter";

    v_flex()
        .w_full()
        .p_3()
        .rounded_lg()
        .bg(COLOR_GRAPHITE_CANVAS)
        .border_1()
        .border_color(if is_enter { COLOR_PINK_PRIMARY } else { COLOR_GRAPHITE_BORDER })
        .gap_2()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(COLOR_PINK_MUTED_BG)
                        .text_size(rems(0.75))
                        .font_weight(FontWeight::BOLD)
                        .text_color(COLOR_PINK_PRIMARY)
                        .child(if is_enter {
                            "Режим планирования"
                        } else {
                            "План утверждён"
                        }),
                ),
        )
        .child(
            div()
                .text_size(rems(0.85))
                .text_color(COLOR_TEXT_PRIMARY)
                .whitespace_normal()
                .child(if is_enter {
                    "Агент активировал режим строгого планирования. Файловые модификации и запуск терминальных команд заблокированы до утверждения плана.".to_string()
                } else if !summary.is_empty() {
                    summary
                } else {
                    "Режим планирования успешно завершён. Переход к выполнению.".to_string()
                }),
        )
        .into_any_element()
}

/// Renders a graphical card for `task_create`, `task_update`, or `task_list`.
pub fn render_task_card(trace: &ToolTrace, _is_streaming: bool) -> AnyElement {
    let args: Value = serde_json::from_str(&trace.arguments).unwrap_or(Value::Null);

    match trace.name.as_str() {
        "task_create" => {
            let subject = args
                .get("subject")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let desc = args
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

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
                                        .child("+ Новая задача"),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(rems(0.85))
                                        .text_color(COLOR_TEXT_PRIMARY)
                                        .child(subject),
                                ),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(COLOR_GRAPHITE_ELEVATED)
                                .text_size(rems(0.7))
                                .text_color(COLOR_TEXT_MUTED)
                                .child("ожидает"),
                        ),
                )
                .children((!desc.is_empty()).then(|| {
                    div()
                        .text_size(rems(0.78))
                        .text_color(COLOR_TEXT_MUTED)
                        .whitespace_normal()
                        .child(desc)
                }))
                .into_any_element()
        }
        "task_update" => {
            let id = args.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let status = args
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let status_label = match status.as_str() {
                "completed" | "done" => "выполнена",
                "in_progress" | "progress" => "в работе",
                "blocked" => "заблокирована",
                "pending" => "ожидает",
                other => other,
            };

            let (badge_bg, badge_fg) = match status.as_str() {
                "completed" | "done" => (hsla(140.0 / 360.0, 0.40, 0.15, 1.0), hsla(140.0 / 360.0, 0.85, 0.65, 1.0)),
                "in_progress" | "progress" => (COLOR_PINK_MUTED_BG, COLOR_PINK_PRIMARY),
                "blocked" => (hsla(0.0, 0.40, 0.15, 1.0), hsla(0.0, 0.85, 0.65, 1.0)),
                _ => (COLOR_GRAPHITE_ELEVATED, COLOR_TEXT_MUTED),
            };

            h_flex()
                .w_full()
                .p_2()
                .rounded_md()
                .bg(COLOR_GRAPHITE_CANVAS)
                .border_1()
                .border_color(COLOR_GRAPHITE_BORDER)
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_size(rems(0.8))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(format!("Задача #{id}")),
                        )
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .text_color(COLOR_TEXT_MUTED)
                                .child("статус изменен"),
                        ),
                )
                .child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(badge_bg)
                        .text_size(rems(0.72))
                        .font_weight(FontWeight::BOLD)
                        .text_color(badge_fg)
                        .child(status_label.to_string()),
                )
                .into_any_element()
        }
        "task_list" => {
            let lines: Vec<String> = trace.output.lines().map(ToString::to_string).collect();

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
                                .child("Список задач агента"),
                        ),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .children(lines.into_iter().filter(|l| !l.starts_with("###")).map(|line_str| {
                            let (badge_char, color) = if line_str.starts_with("[x]") {
                                ("[✓]", hsla(140.0 / 360.0, 0.85, 0.65, 1.0))
                            } else if line_str.starts_with("[>]") {
                                ("[▶]", COLOR_PINK_PRIMARY)
                            } else if line_str.starts_with("[!]") {
                                ("[✕]", hsla(0.0, 0.85, 0.65, 1.0))
                            } else {
                                ("[ ]", COLOR_TEXT_MUTED)
                            };

                            let clean_label = line_str
                                .trim_start_matches(&['[', ']', 'x', '>', '!', ' '])
                                .trim()
                                .to_string();

                            h_flex()
                                .items_center()
                                .gap_2()
                                .text_size(rems(0.78))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(color)
                                        .child(badge_char),
                                )
                                .child(
                                    div()
                                        .text_color(COLOR_TEXT_PRIMARY)
                                        .child(clean_label),
                                )
                        })),
                )
                .into_any_element()
        }
        _ => div().into_any_element(),
    }
}
