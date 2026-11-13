use crate::chat::session::ToolTrace;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_CANVAS, COLOR_PINK_MUTED_BG, COLOR_PINK_PRIMARY, COLOR_TEXT_MUTED,
    COLOR_TEXT_PRIMARY,
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Sizable, Size, h_flex, v_flex};
use serde_json::Value;

/// Renders a specialized graphical card for the `ask_user` tool call.
pub fn render_ask_user(
    trace: &ToolTrace,
    is_streaming: bool,
    state_entity: &Entity<AppState>,
    _window: &mut Window,
    _cx: &mut App,
) -> AnyElement {
    let args: Value = serde_json::from_str(&trace.arguments).unwrap_or(Value::Null);

    let question = args
        .get("question")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let options: Vec<String> = args
        .get("options")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let allow_free_text = args
        .get("allow_free_text")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let container = v_flex()
        .w_full()
        .p_3()
        .rounded_lg()
        .bg(COLOR_GRAPHITE_CANVAS)
        .border_1()
        .border_color(COLOR_PINK_PRIMARY)
        .gap_2();

    // 1. Header: question badge
    let header = h_flex()
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
                .child("Вопрос к пользователю"),
        )
        .child(if is_streaming {
            div()
                .text_size(rems(0.75))
                .text_color(COLOR_TEXT_MUTED)
                .child("Формирование вопроса...")
        } else {
            div()
        });

    let card = container.child(header);

    // If question has not streamed yet
    let Some(q_text) = question else {
        return card
            .child(
                div()
                    .text_size(rems(0.85))
                    .text_color(COLOR_TEXT_MUTED)
                    .child(if is_streaming {
                        "Ожидание текста вопроса от модели...".to_string()
                    } else if !trace.arguments.is_empty() && trace.arguments != "{}" {
                        trace.arguments.clone()
                    } else {
                        "Вопрос сформирован без текстового описания.".to_string()
                    }),
            )
            .into_any_element();
    };

    // 2. Question body
    let body = div()
        .text_size(rems(0.95))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(COLOR_TEXT_PRIMARY)
        .whitespace_normal()
        .child(q_text);

    let mut card = card.child(body);

    // 3. Options list
    if !options.is_empty() {
        let options_list = v_flex()
            .w_full()
            .gap_1p5()
            .pt_1()
            .children(options.into_iter().enumerate().map(|(idx, opt)| {
                let opt_for_click = opt.clone();
                let state_clone = state_entity.clone();

                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new(format!("ask-user-opt-{idx}-{}", opt.chars().take(8).collect::<String>()))
                            .secondary()
                            .with_size(Size::Small)
                            .label(format!("{}. {}", idx + 1, opt))
                            .on_click(move |_, _window, cx| {
                                let msg = opt_for_click.clone();
                                state_clone.update(cx, |state, _cx| {
                                    state.send_message(msg);
                                });
                            }),
                    )
            }));

        card = card.child(options_list);
    }

    // 4. Free text hint
    if allow_free_text {
        card = card.child(
            h_flex()
                .items_center()
                .gap_1()
                .pt_1()
                .text_size(rems(0.75))
                .text_color(COLOR_TEXT_MUTED)
                .child("Нажмите вариант выше или отправьте ответ в строке ввода снизу"),
        );
    }

    card.into_any_element()
}
