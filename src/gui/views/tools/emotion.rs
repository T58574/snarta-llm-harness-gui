use crate::chat::session::ToolTrace;
use crate::gui::anim::avatar::{face_art, AvatarEmotion};
use crate::gui::icons::icons;
use crate::gui::state::AppState;
use crate::gui::theme::{COLOR_BG_CANVAS, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY};
use gpui::*;
use gpui_component::{ActiveTheme, h_flex, v_flex};
use serde_json::Value;

/// Renders a compact inline card when the model updates its facial expression via `set_emotion`.
pub fn render_emotion_card(
    trace: &ToolTrace,
    state_entity: &Entity<AppState>,
    cx: &App,
) -> AnyElement {
    let args: Value = serde_json::from_str(&trace.arguments).unwrap_or(Value::Null);
    let raw_emotion = args
        .get("emotion")
        .and_then(|v| v.as_str())
        .unwrap_or("neutral");
    let thought = args
        .get("thought")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let emotion = AvatarEmotion::from_str(raw_emotion);
    let (accent, accent_bg) = emotion.colors();
    let gender = state_entity.read(cx).workspace.persona.gender;
    let art = face_art(gender, emotion);
    let mono = cx.theme().mono_font_family.clone();

    h_flex()
        .w_full()
        .px_2p5()
        .py_1p5()
        .rounded_lg()
        .bg(COLOR_BG_CANVAS)
        .border_1()
        .border_color(accent.opacity(0.45))
        .items_center()
        .gap_2p5()
        .child(
            v_flex()
                .items_start()
                .justify_center()
                .flex_shrink_0()
                .px_1p5()
                .py_1()
                .rounded_md()
                .bg(accent_bg)
                .font_family(mono)
                .text_size(px(9.0))
                .line_height(px(10.0))
                .text_color(accent)
                .child(div().whitespace_nowrap().child(art.brows))
                .child(div().whitespace_nowrap().child(art.eyes))
                .child(div().whitespace_nowrap().child(art.mouth)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .child(icons::sparkles().size(px(12.0)).text_color(accent))
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .font_weight(FontWeight::BOLD)
                                .text_color(accent)
                                .child(format!("Эмоция: {}", emotion.label_ru())),
                        ),
                )
                .children(thought.map(|t| {
                    div()
                        .text_size(rems(0.78))
                        .text_color(COLOR_TEXT_PRIMARY)
                        .whitespace_normal()
                        .child(t.to_string())
                }))
                .children((thought.is_none() && !trace.output.is_empty()).then(|| {
                    div()
                        .text_size(rems(0.72))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(trace.output.clone())
                })),
        )
        .into_any_element()
}
