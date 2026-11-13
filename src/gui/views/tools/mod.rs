pub mod ask_user;
pub mod fs;
pub mod plan;
pub mod search;
pub mod shell;


use crate::chat::session::ToolTrace;
use crate::gui::anim::tools::{render_tool_progress_bar, render_tool_status_indicator};
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_PINK_PRIMARY, COLOR_TEXT_MUTED,
};
use gpui::*;
use gpui_component::{h_flex, v_flex};

/// Main dispatcher that renders a specialized graphical representation for any agent harness tool.
pub fn render_tool_trace(
    trace: &ToolTrace,
    is_streaming: bool,
    state_entity: &Entity<AppState>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match trace.name.as_str() {
        "ask_user" => ask_user::render_ask_user(trace, is_streaming, state_entity, window, cx),
        "plan_mode" => plan::render_plan_mode(trace, is_streaming),
        "task_create" | "task_update" | "task_list" => plan::render_task_card(trace, is_streaming),
        "read_file" | "write_file" | "edit_file" | "delete_file" | "remember" | "write_note" => {
            fs::render_fs_card(trace, is_streaming)
        }
        "exec" | "shell_start" | "shell_output" | "shell_kill" => {
            shell::render_shell_card(trace, is_streaming)
        }
        "web_search"
        | "alexandria_search"
        | "alexandria_scrape"
        | "glob_files"
        | "grep_files"
        | "list_files"
        | "change_dir"
        | "cd"
        | "view_image" => search::render_search_card(trace, is_streaming),
        other => {
            // Generic fallback card for any other tool
            v_flex()
                .w_full()
                .p_2()
                .rounded_md()
                .bg(COLOR_GRAPHITE_CANVAS)
                .border_1()
                .border_color(COLOR_GRAPHITE_BORDER)
                .gap_1()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_size(rems(0.78))
                        .text_color(COLOR_PINK_PRIMARY)
                        .child(format!("{other}({})", trace.arguments)),
                )
                .children((!trace.output.is_empty()).then(|| {
                    div()
                        .text_size(rems(0.75))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(format!("Вывод: {}", trace.output))
                }))
                .into_any_element()
        }
    }
}

/// Renders a collection of tool traces for a turn or streaming message.
pub fn render_tool_traces(
    tools: &[ToolTrace],
    is_streaming: bool,
    state_entity: &Entity<AppState>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    if tools.is_empty() {
        return div().into_any_element();
    }

    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(rems(0.8))
                        .font_weight(FontWeight::BOLD)
                        .text_color(COLOR_PINK_PRIMARY)
                        .child(format!("Инструменты ({})", tools.len())),
                )
                .child(render_tool_status_indicator(is_streaming)),
        )
        .children(tools.iter().enumerate().map(|(idx, t)| {
            let is_running = is_streaming && (t.output == "Running..." || t.output == "Выполняется...");
            let card = render_tool_trace(t, is_running, state_entity, window, cx);
            if is_running {
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(card)
                    .child(render_tool_progress_bar(format!("tool-pb-{}", idx), true))
                    .into_any_element()
            } else {
                card
            }
        }))
        .into_any_element()
}
