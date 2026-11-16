use crate::chat::session::ToolTrace;
use crate::gui::icons::icons;
use crate::gui::theme::{
    COLOR_ACCENT_BLUE, COLOR_ACCENT_BLUE_MUTED, COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED,
    COLOR_BG_CANVAS, COLOR_BG_ELEVATED, COLOR_BG_INPUT, COLOR_BORDER, COLOR_TEXT_MUTED,
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
                                        .bg(COLOR_ACCENT_BLUE_MUTED)
                                        .text_size(rems(0.7))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(COLOR_ACCENT_BLUE)
                                        .child(icons::globe().size(px(11.0)).text_color(COLOR_ACCENT_BLUE))
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
                                .bg(COLOR_BG_ELEVATED)
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
                        .bg(COLOR_BG_INPUT)
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
                                .bg(COLOR_ACCENT_BLUE_MUTED)
                                .text_size(rems(0.7))
                                .font_weight(FontWeight::BOLD)
                                .text_color(COLOR_ACCENT_BLUE)
                                .child(icons::globe().size(px(11.0)).text_color(COLOR_ACCENT_BLUE))
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
                        .bg(COLOR_BG_INPUT)
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
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(icons::search().size(px(11.0)).text_color(COLOR_ACCENT_ORANGE))
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
                        .bg(COLOR_BG_INPUT)
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
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(icons::search().size(px(11.0)).text_color(COLOR_ACCENT_ORANGE))
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
                        .bg(COLOR_BG_INPUT)
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
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(icons::folder().size(px(11.0)).text_color(COLOR_ACCENT_BLUE))
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
                        .bg(COLOR_BG_INPUT)
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
                .bg(COLOR_BG_CANVAS)
                .border_1()
                .border_color(COLOR_BORDER)
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
                        .child(icons::folder_open().size(px(11.0)).text_color(COLOR_ACCENT_ORANGE))
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
                .bg(COLOR_BG_CANVAS)
                .border_1()
                .border_color(COLOR_BORDER)
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
                        .child(icons::image().size(px(11.0)).text_color(COLOR_ACCENT_ORANGE))
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
