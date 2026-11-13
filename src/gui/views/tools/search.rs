use crate::chat::session::ToolTrace;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_ELEVATED,
    COLOR_GRAPHITE_INPUT, COLOR_PINK_MUTED_BG, COLOR_PINK_PRIMARY, COLOR_TEXT_MUTED,
    COLOR_TEXT_PRIMARY,
};
use gpui::*;
use gpui_component::{h_flex, v_flex};
use serde_json::Value;

/// Renders a graphical card for search and discovery tools (`web_search`, `alexandria_*`, `glob_files`, `grep_files`, `list_files`, `change_dir`, `view_image`).
pub fn render_search_card(trace: &ToolTrace, _is_streaming: bool) -> AnyElement {
    let args: Value = serde_json::from_str(&trace.arguments).unwrap_or(Value::Null);

    match trace.name.as_str() {
        "web_search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let provider = args.get("provider").and_then(|v| v.as_str()).unwrap_or("cascade").to_string();

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
                                        .child("ВЕБ-ПОИСК"),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_size(rems(0.82))
                                        .text_color(COLOR_TEXT_PRIMARY)
                                        .child(format!("«{query}»")),
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
                                .child(provider),
                        ),
                )
                .children((!trace.output.is_empty()).then(|| {
                    div()
                        .max_h(rems(10.0))
                        .overflow_hidden()
                        .p_2()
                        .rounded_md()
                        .bg(COLOR_GRAPHITE_INPUT)
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(trace.output.clone())
                }))
                .into_any_element()
        }
        "alexandria_search" | "alexandria_scrape" => {
            let is_scrape = trace.name == "alexandria_scrape";
            let label = if is_scrape { "ALEXANDRIA СБОР" } else { "ALEXANDRIA КАТАЛОГ" };

            let title = if is_scrape {
                let p = args.get("provider").and_then(|v| v.as_str()).unwrap_or("");
                let c = args.get("capability").and_then(|v| v.as_str()).unwrap_or("");
                format!("{p} / {c}")
            } else {
                args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string()
            };

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
                                .child(label),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(rems(0.82))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(title),
                        ),
                )
                .children((!trace.output.is_empty()).then(|| {
                    div()
                        .max_h(rems(10.0))
                        .overflow_hidden()
                        .p_2()
                        .rounded_md()
                        .bg(COLOR_GRAPHITE_INPUT)
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(trace.output.clone())
                }))
                .into_any_element()
        }
        "glob_files" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("*").to_string();
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".").to_string();

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
                                .bg(COLOR_GRAPHITE_ELEVATED)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child("МАСКА ФАЙЛОВ"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(rems(0.82))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(format!("{pattern} ({path})")),
                        ),
                )
                .children((!trace.output.is_empty()).then(|| {
                    div()
                        .max_h(rems(8.0))
                        .overflow_hidden()
                        .p_2()
                        .rounded_md()
                        .bg(COLOR_GRAPHITE_INPUT)
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(trace.output.clone())
                }))
                .into_any_element()
        }
        "grep_files" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".").to_string();

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
                                .bg(COLOR_GRAPHITE_ELEVATED)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child("ПОИСК СТРОК"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(rems(0.82))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(format!("«{query}» ({path})")),
                        ),
                )
                .children((!trace.output.is_empty()).then(|| {
                    div()
                        .max_h(rems(8.0))
                        .overflow_hidden()
                        .p_2()
                        .rounded_md()
                        .bg(COLOR_GRAPHITE_INPUT)
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(trace.output.clone())
                }))
                .into_any_element()
        }
        "list_files" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".").to_string();

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
                                .bg(COLOR_GRAPHITE_ELEVATED)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child("СПИСОК ФАЙЛОВ"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(rems(0.82))
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(path),
                        ),
                )
                .children((!trace.output.is_empty()).then(|| {
                    div()
                        .max_h(rems(8.0))
                        .overflow_hidden()
                        .p_2()
                        .rounded_md()
                        .bg(COLOR_GRAPHITE_INPUT)
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(trace.output.clone())
                }))
                .into_any_element()
        }
        "change_dir" | "cd" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();

            h_flex()
                .w_full()
                .p_2()
                .rounded_md()
                .bg(COLOR_GRAPHITE_CANVAS)
                .border_1()
                .border_color(COLOR_GRAPHITE_BORDER)
                .items_center()
                .gap_2()
                .child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(COLOR_GRAPHITE_ELEVATED)
                        .text_size(rems(0.7))
                        .font_weight(FontWeight::BOLD)
                        .text_color(COLOR_PINK_PRIMARY)
                        .child("CWD"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(rems(0.8))
                        .text_color(COLOR_TEXT_PRIMARY)
                        .child(format!("Переход в папку: {path}")),
                )
                .into_any_element()
        }
        "view_image" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();

            h_flex()
                .w_full()
                .p_2()
                .rounded_md()
                .bg(COLOR_GRAPHITE_CANVAS)
                .border_1()
                .border_color(COLOR_GRAPHITE_BORDER)
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
                        .child("ИЗОБРАЖЕНИЕ"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(rems(0.8))
                        .text_color(COLOR_TEXT_PRIMARY)
                        .child(path),
                )
                .into_any_element()
        }
        _ => div().into_any_element(),
    }
}
