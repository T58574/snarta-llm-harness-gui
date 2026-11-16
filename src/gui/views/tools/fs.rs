use crate::chat::session::ToolTrace;
use crate::gui::icons::icons;
use crate::gui::theme::{
    COLOR_ACCENT_BLUE, COLOR_ACCENT_BLUE_MUTED, COLOR_ACCENT_GREEN, COLOR_ACCENT_GREEN_MUTED,
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED, COLOR_BG_CANVAS, COLOR_BG_ELEVATED,
    COLOR_BG_INPUT, COLOR_BORDER, COLOR_DANGER, COLOR_DANGER_MUTED, COLOR_TEXT_MUTED,
    COLOR_TEXT_PRIMARY,
};
use gpui::*;
use gpui_component::{h_flex, v_flex};
use serde_json::Value;

/// Renders a graphical card for filesystem tool calls (`read_file`, `write_file`, `edit_file`, `delete_file`, `remember`, `write_note`).
pub fn render_fs_card(trace: &ToolTrace, _is_streaming: bool) -> AnyElement {
    let args: Value = serde_json::from_str(&trace.arguments).unwrap_or(Value::Null);
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    match trace.name.as_str() {
        "read_file" => {
            let start = args.get("start_line").and_then(|v| v.as_u64());
            let end = args.get("end_line").and_then(|v| v.as_u64());

            let range_label = match (start, end) {
                (Some(s), Some(e)) => format!("строки {s}..{e}"),
                (Some(s), None) => format!("со строки {s}"),
                _ => String::new(),
            };

            v_flex()
                .w_full()
                .p_2p5()
                .rounded_lg()
                .bg(COLOR_BG_CANVAS)
                .border_1()
                .border_color(COLOR_BORDER)
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
                                    h_flex()
                                        .items_center()
                                        .gap_1()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(COLOR_BG_ELEVATED)
                                        .text_size(rems(0.7))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(COLOR_TEXT_PRIMARY)
                                        .child(icons::file_text().size(px(11.0)).text_color(COLOR_ACCENT_BLUE))
                                        .child("ЧТЕНИЕ"),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_size(rems(0.82))
                                        .text_color(COLOR_TEXT_PRIMARY)
                                        .child(path),
                                ),
                        )
                        .children((!range_label.is_empty()).then(|| {
                            div()
                                .text_size(rems(0.72))
                                .text_color(COLOR_TEXT_MUTED)
                                .child(range_label)
                        })),
                )
                .child(
                    div()
                        .max_h(rems(10.0))
                        .overflow_hidden()
                        .p_2()
                        .rounded_md()
                        .bg(COLOR_BG_INPUT)
                        .border_1()
                        .border_color(COLOR_BORDER)
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(if trace.output.is_empty() {
                            "Выполняется чтение...".to_string()
                        } else {
                            trace.output.clone()
                        }),
                )
                .into_any_element()
        }
        "write_file" => {
            let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);
            let content_len = args.get("content").and_then(|v| v.as_str()).map_or(0, |s| s.len());

            v_flex()
                .w_full()
                .p_2p5()
                .rounded_lg()
                .bg(COLOR_BG_CANVAS)
                .border_1()
                .border_color(COLOR_BORDER)
                .gap_1()
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_1()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(COLOR_ACCENT_GREEN_MUTED)
                                        .text_size(rems(0.7))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(COLOR_ACCENT_GREEN)
                                        .child(icons::file_pen().size(px(11.0)).text_color(COLOR_ACCENT_GREEN))
                                        .child(if append { "ДОЗАПИСЬ" } else { "СОХРАНЕНИЕ" }),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_size(rems(0.82))
                                        .text_color(COLOR_TEXT_PRIMARY)
                                        .child(path),
                                ),
                        )
                        .child(
                            div()
                                .text_size(rems(0.72))
                                .text_color(COLOR_TEXT_MUTED)
                                .child(format!("{content_len} байт")),
                        ),
                )
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(trace.output.clone()),
                )
                .into_any_element()
        }
        "edit_file" => {
            let old_text = args
                .get("old_text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let new_text = args
                .get("new_text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            v_flex()
                .w_full()
                .p_2p5()
                .rounded_lg()
                .bg(COLOR_BG_CANVAS)
                .border_1()
                .border_color(COLOR_BORDER)
                .gap_2()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(COLOR_ACCENT_ORANGE_MUTED)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_ACCENT_ORANGE)
                                .child(icons::file_pen().size(px(11.0)).text_color(COLOR_ACCENT_ORANGE))
                                .child("ПРАВКА"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(rems(0.82))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(path),
                        ),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .p_1p5()
                                .rounded_sm()
                                .bg(COLOR_DANGER_MUTED)
                                .text_size(rems(0.72))
                                .text_color(COLOR_DANGER)
                                .child(format!("- {old_text}")),
                        )
                        .child(
                            div()
                                .p_1p5()
                                .rounded_sm()
                                .bg(COLOR_ACCENT_GREEN_MUTED)
                                .text_size(rems(0.72))
                                .text_color(COLOR_ACCENT_GREEN)
                                .child(format!("+ {new_text}")),
                        ),
                )
                .into_any_element()
        }
        "delete_file" => {
            h_flex()
                .w_full()
                .p_2()
                .rounded_md()
                .bg(COLOR_BG_CANVAS)
                .border_1()
                .border_color(COLOR_BORDER)
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(COLOR_DANGER_MUTED)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_DANGER)
                                .child(icons::trash().size(px(11.0)).text_color(COLOR_DANGER))
                                .child("УДАЛЕНИЕ"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(rems(0.82))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(path),
                        ),
                )
                .child(
                    div()
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(trace.output.clone()),
                )
                .into_any_element()
        }
        "remember" => {
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            v_flex()
                .w_full()
                .p_2p5()
                .rounded_lg()
                .bg(COLOR_BG_CANVAS)
                .border_1()
                .border_color(COLOR_ACCENT_BLUE.opacity(0.6))
                .gap_1p5()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(COLOR_ACCENT_BLUE_MUTED)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_ACCENT_BLUE)
                                .child(icons::brain().size(px(11.0)).text_color(COLOR_ACCENT_BLUE))
                                .child("ПАМЯТЬ"),
                        )
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .text_color(COLOR_TEXT_MUTED)
                                .child("life/memory.md"),
                        ),
                )
                .child(
                    div()
                        .text_size(rems(0.82))
                        .text_color(COLOR_TEXT_PRIMARY)
                        .whitespace_normal()
                        .child(text),
                )
                .into_any_element()
        }
        "write_note" => {
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            v_flex()
                .w_full()
                .p_2p5()
                .rounded_lg()
                .bg(COLOR_BG_CANVAS)
                .border_1()
                .border_color(COLOR_BORDER)
                .gap_1p5()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(COLOR_BG_ELEVATED)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_ACCENT_ORANGE)
                                .child(icons::file_text().size(px(11.0)).text_color(COLOR_ACCENT_ORANGE))
                                .child("ЗАМЕТКА"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(rems(0.82))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(path),
                        ),
                )
                .child(
                    div()
                        .max_h(rems(6.0))
                        .overflow_hidden()
                        .p_1p5()
                        .rounded_sm()
                        .bg(COLOR_BG_INPUT)
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(content),
                )
                .into_any_element()
        }
        _ => div().into_any_element(),
    }
}
