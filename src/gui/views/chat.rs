use crate::chat::session::{ChatTurn, TurnStep};
use crate::gui::anim::avatar::{render_avatar_circle, render_hero_avatar};
use crate::gui::anim::presence::{
    apply_streaming_glow, render_breathing_cursor, render_thinking_dots,
};
use crate::gui::icons::icons;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_SELECTION, COLOR_BG_CANVAS, COLOR_BG_ELEVATED,
    COLOR_BORDER, COLOR_BORDER_GLASS, COLOR_GLASS_SURFACE, COLOR_GLASS_SURFACE_USER,
    COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY, COLOR_TEXT_SUBTLE,
};
use crate::gui::views::tools::render_tool_traces;
use crate::gui::views::topbar::format_compact_ctx;
use crate::persona::Persona;
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Sizable, Size, h_flex, v_flex};
use gpui_kit::base::text::{TextView, TextViewMotion, TextViewStyle};
use std::path::PathBuf;
use std::time::Duration;

pub struct ChatView;

impl ChatView {
    pub fn render(
        state_entity: &Entity<AppState>,
        scroll_handle: &ScrollHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let turns = state.session.turns.clone();
        let turns_empty = turns.is_empty();
        let turns_len = turns.len();
        let is_generating = state.is_generating;
        let streaming_steps = state.streaming_steps.clone();
        let used_ctx = state.used_ctx;
        let active_ctx = state.vram.active_ctx;
        let persona = state.persona.clone();
        let expanded_menus = state.expanded_turn_menus.clone();

        v_flex()
            .id("chat-scroll")
            .track_scroll(scroll_handle)
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .px_6()
            .py_5()
            .gap_4()
            .children(turns.into_iter().enumerate().map(|(idx, turn)| {
                let is_last = idx + 1 == turns_len && !is_generating;
                let menu_open = expanded_menus.contains(&idx);
                Self::render_turn(
                    idx,
                    &turn,
                    is_last,
                    menu_open,
                    used_ctx,
                    active_ctx,
                    &persona,
                    state_entity,
                    window,
                    cx,
                )
            }))
            .child(if is_generating {
                div()
                    .child(Self::render_streaming(
                        &streaming_steps,
                        &persona,
                        state_entity,
                        window,
                        cx,
                    ))
                    .into_any_element()
            } else if turns_empty {
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .py_12()
                    .child(render_hero_avatar(&persona, state_entity))
                    .into_any_element()
            } else {
                div().into_any_element()
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_turn(
        idx: usize,
        turn: &ChatTurn,
        is_last: bool,
        menu_open: bool,
        used_ctx: u32,
        active_ctx: u32,
        persona: &Persona,
        state_entity: &Entity<AppState>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let is_user = turn.role == "user";
        let content = turn.content.clone();
        let reasoning = turn.reasoning.clone();
        let tools = turn.tool_traces.clone();
        let steps = turn.effective_steps();
        let images = turn.effective_images();
        let tps = turn.gen_tps;
        let tokens = turn.tokens;

        if is_user {
            h_flex()
                .w_full()
                .justify_end()
                .child(
                    v_flex()
                        .max_w(rems(42.0))
                        .px_4()
                        .py_3()
                        .rounded_2xl()
                        .bg(COLOR_GLASS_SURFACE_USER)
                        .border_1()
                        .border_color(COLOR_BORDER_GLASS)
                        .text_color(COLOR_TEXT_PRIMARY)
                        .gap_2()
                        .children(images.into_iter().map(|img_path| {
                            let path_buf = PathBuf::from(&img_path);
                            let fname = path_buf
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(&img_path)
                                .to_string();
                            v_flex()
                                .p_1p5()
                                .rounded_lg()
                                .bg(COLOR_BG_CANVAS.opacity(0.35))
                                .gap_1()
                                .child(
                                    img(path_buf)
                                        .max_w(px(280.0))
                                        .max_h(px(220.0))
                                        .rounded_md()
                                        .object_fit(ObjectFit::Contain),
                                )
                                .child(
                                    div()
                                        .text_size(rems(0.72))
                                        .text_color(COLOR_TEXT_MUTED)
                                        .child(fname),
                                )
                        }))
                        .children((!content.trim().is_empty()).then(|| {
                            TextView::markdown(format!("user-turn-{}", idx), content.clone())
                                .selectable(true)
                                .style(user_markdown_style())
                                .w_full()
                        }))
                        .child(
                            h_flex()
                                .justify_end()
                                .items_center()
                                .gap_2()
                                .children(menu_open.then(|| {
                                    Button::new(format!("copy-user-{}", idx))
                                        .ghost()
                                        .with_size(Size::XSmall)
                                        .icon(icons::copy())
                                        .label("Копировать")
                                        .on_click({
                                            let copy_text = content.clone();
                                            move |_, _window, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    copy_text.clone(),
                                                ));
                                            }
                                        })
                                }))
                                .child(
                                    Button::new(format!("menu-user-{}", idx))
                                        .ghost()
                                        .with_size(Size::XSmall)
                                        .label("•••")
                                        .tooltip("Действия")
                                        .on_click({
                                            let state_entity = state_entity.clone();
                                            move |_, _, cx| {
                                                state_entity.update(cx, |s, cx| {
                                                    s.toggle_turn_menu(idx);
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                ),
                        ),
                )
                .into_any_element()
        } else {
            let full_copy_text = {
                let mut text = String::new();
                if !content.trim().is_empty() {
                    text.push_str(&content);
                }
                if text.is_empty() {
                    if let Some(ref r) = reasoning {
                        if !r.trim().is_empty() {
                            text.push_str("Рассуждения:\n");
                            text.push_str(r);
                            text.push_str("\n\n");
                        }
                    }
                    for t in &tools {
                        if !t.output.is_empty() {
                            text.push_str(&format!(
                                "{}({}):\n{}\n\n",
                                t.name, t.arguments, t.output
                            ));
                        } else {
                            text.push_str(&format!("{}({})\n\n", t.name, t.arguments));
                        }
                    }
                    text = text.trim_end().to_string();
                }
                text
            };

            let footer_stats = {
                let mut parts = Vec::new();
                if let Some(tok) = tokens {
                    parts.push(format!("{tok} ток"));
                }
                if let Some(speed) = tps.filter(|&s| s > 0.0) {
                    parts.push(format!("{speed:.1} t/s"));
                }
                if is_last && active_ctx > 0 {
                    let pct = (used_ctx as f32 / active_ctx as f32) * 100.0;
                    parts.push(format!(
                        "Контекст: {}/{} ({:.0}%)",
                        format_compact_ctx(used_ctx),
                        format_compact_ctx(active_ctx),
                        pct
                    ));
                }
                parts.join(" · ")
            };

            v_flex()
                .w_full()
                .items_start()
                .gap_1p5()
                .child(
                    h_flex()
                        .w_full()
                        .max_w(rems(52.0))
                        .items_center()
                        .justify_between()
                        .px_1()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(render_avatar_circle(
                                    &persona.display_name,
                                    persona.avatar_path.as_deref(),
                                    26.0,
                                ))
                                .child(
                                    div()
                                        .text_size(rems(0.82))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(COLOR_TEXT_SUBTLE)
                                        .child(persona.display_name.clone()),
                                ),
                        )
                        .child(
                            Button::new(format!("menu-turn-{}", idx))
                                .ghost()
                                .with_size(Size::XSmall)
                                .label("•••")
                                .tooltip("Технические данные и копирование")
                                .on_click({
                                    let state_entity = state_entity.clone();
                                    move |_, _, cx| {
                                        state_entity.update(cx, |s, cx| {
                                            s.toggle_turn_menu(idx);
                                            cx.notify();
                                        });
                                    }
                                }),
                        ),
                )
                .child(
                    v_flex()
                        .max_w(rems(52.0))
                        .w_full()
                        .p_5()
                        .rounded_2xl()
                        .bg(COLOR_GLASS_SURFACE)
                        .border_1()
                        .border_color(COLOR_BORDER_GLASS)
                        .gap_3()
                        .children(steps.into_iter().enumerate().map(|(step_idx, step)| {
                            Self::render_step(
                                &format!("turn-{idx}-step-{step_idx}"),
                                &step,
                                false,
                                state_entity,
                                window,
                                cx,
                            )
                        }))
                        .children(menu_open.then(|| {
                            h_flex()
                                .items_center()
                                .justify_between()
                                .pt_2p5()
                                .border_t_1()
                                .border_color(COLOR_BORDER_GLASS)
                                .text_size(rems(0.74))
                                .text_color(COLOR_TEXT_MUTED)
                                .child(if footer_stats.is_empty() {
                                    "Нет метрик".to_string()
                                } else {
                                    footer_stats
                                })
                                .child(
                                    Button::new(format!("copy-turn-{}", idx))
                                        .ghost()
                                        .with_size(Size::XSmall)
                                        .icon(icons::copy())
                                        .label("Копировать")
                                        .on_click({
                                            let copy_text = full_copy_text;
                                            move |_, _window, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    copy_text.clone(),
                                                ));
                                            }
                                        }),
                                )
                        })),
                )
                .into_any_element()
        }
    }

    fn render_step(
        step_key: &str,
        step: &TurnStep,
        is_streaming: bool,
        state_entity: &Entity<AppState>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let reasoning_key = format!("r-{step_key}");
        let is_collapsed = state_entity.read(cx).is_reasoning_collapsed(&reasoning_key);
        let stream_reasoning_enabled = state_entity.read(cx).cfg.ui.stream_reasoning;

        v_flex()
            .w_full()
            .gap_2p5()
            .children(
                step.reasoning
                    .as_ref()
                    .filter(|r| !r.trim().is_empty() && (!is_streaming || stream_reasoning_enabled))
                    .map(|r| {
                        let normalized_r = normalize_reasoning_markdown(r);
                        let toggle_key = reasoning_key.clone();
                        let state_for_toggle = state_entity.clone();
                        v_flex()
                            .p_2p5()
                            .rounded_md()
                            .bg(COLOR_BG_CANVAS)
                            .border_1()
                            .border_color(COLOR_BORDER)
                            .text_size(rems(0.85))
                            .text_color(COLOR_TEXT_MUTED)
                            .gap_1p5()
                            .child(
                                h_flex()
                                    .id(SharedString::from(format!("hdr-{reasoning_key}")))
                                    .items_center()
                                    .justify_between()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        state_for_toggle.update(cx, |s, cx| {
                                            s.toggle_reasoning_collapsed(&toggle_key);
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                if is_collapsed {
                                                    icons::chevron_right()
                                                } else {
                                                    icons::chevron_down()
                                                }
                                                .with_size(Size::XSmall)
                                                .text_color(COLOR_ACCENT_ORANGE),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(COLOR_ACCENT_ORANGE)
                                                    .child("Рассуждения"),
                                            )
                                            .children(is_streaming.then(|| {
                                                render_thinking_dots("reasoning-dots")
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.72))
                                            .text_color(COLOR_TEXT_MUTED)
                                            .child(if is_collapsed {
                                                "Развернуть"
                                            } else {
                                                "Свернуть"
                                            }),
                                    ),
                            )
                            .children((!is_collapsed).then(|| {
                                TextView::markdown(
                                    SharedString::from(format!("md-{reasoning_key}")),
                                    normalized_r,
                                )
                                .selectable(true)
                                .style(reasoning_markdown_style())
                                .w_full()
                            }))
                    }),
            )
            .children((!step.content.trim().is_empty()).then(|| {
                let tv = TextView::markdown(
                    SharedString::from(format!("content-{step_key}")),
                    step.content.clone(),
                )
                .selectable(true)
                .style(assistant_markdown_style())
                .w_full();
                if is_streaming {
                    tv.motion(
                        TextViewMotion::default()
                            .with_stream_fade(Duration::from_millis(160))
                            .with_stream_fade_stagger(Duration::from_millis(8)),
                    )
                } else {
                    tv
                }
            }))
            .children((!step.tool_calls.is_empty()).then(|| {
                render_tool_traces(&step.tool_calls, is_streaming, state_entity, window, cx)
            }))
            .into_any_element()
    }

    fn render_streaming(
        steps: &[TurnStep],
        persona: &Persona,
        state_entity: &Entity<AppState>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let non_empty_steps: Vec<(usize, &TurnStep)> = steps
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.reasoning.as_ref().is_some_and(|r| !r.trim().is_empty())
                    || !s.content.trim().is_empty()
                    || !s.tool_calls.is_empty()
            })
            .collect();

        let card = v_flex()
            .max_w(rems(52.0))
            .w_full()
            .p_5()
            .rounded_2xl()
            .bg(COLOR_GLASS_SURFACE)
            .border_1()
            .border_color(COLOR_ACCENT_ORANGE.opacity(0.65))
            .gap_3()
            .children(if non_empty_steps.is_empty() {
                vec![
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .text_size(rems(0.85))
                                .text_color(COLOR_TEXT_MUTED)
                                .child("Печатает..."),
                        )
                        .child(render_breathing_cursor("streaming-cursor-pre"))
                        .into_any_element(),
                ]
            } else {
                let last_idx = non_empty_steps.len().saturating_sub(1);
                non_empty_steps
                    .into_iter()
                    .enumerate()
                    .map(|(vis_idx, (step_idx, step))| {
                        let is_active_step = vis_idx == last_idx;
                        Self::render_step(
                            &format!("stream-step-{step_idx}"),
                            step,
                            is_active_step,
                            state_entity,
                            window,
                            cx,
                        )
                    })
                    .collect()
            });

        v_flex()
            .w_full()
            .items_start()
            .gap_1p5()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .px_1()
                    .child(render_avatar_circle(
                        &persona.display_name,
                        persona.avatar_path.as_deref(),
                        26.0,
                    ))
                    .child(
                        div()
                            .text_size(rems(0.82))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(COLOR_TEXT_SUBTLE)
                            .child(persona.display_name.clone()),
                    )
                    .child(render_breathing_cursor("streaming-header-cursor")),
            )
            .child(apply_streaming_glow(card, "streaming-card-glow", true))
            .into_any_element()
    }
}

/// Normalizes reasoning markdown to prevent accidental indented code blocks or unclosed code fences
/// from splitting the reasoning view into nested bordered boxes (seen in Gemma 4 and Qwen 3.8).
pub fn normalize_reasoning_markdown(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 16);
    let mut in_fence = false;

    for line in raw.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("```") {
            in_fence = !in_fence;
            out.push_str(trimmed_start);
            out.push('\n');
            continue;
        }

        if !in_fence && (line.starts_with("    ") || line.starts_with('\t')) {
            // Strip up to 4 leading spaces or 1 tab outside fenced blocks so CommonMark
            // does not treat thought paragraphs as 4-space indented code blocks.
            let stripped = line
                .strip_prefix("    ")
                .or_else(|| line.strip_prefix('\t'))
                .unwrap_or(trimmed_start);
            out.push_str(stripped);
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    if in_fence {
        out.push_str("```\n");
    }

    out.trim_end().to_string()
}

fn reasoning_markdown_style() -> TextViewStyle {
    let mut inline_code_style = StyleRefinement::default();
    let inline_radius = px(4.0);
    inline_code_style.corner_radii.top_left = Some(inline_radius.into());
    inline_code_style.corner_radii.top_right = Some(inline_radius.into());
    inline_code_style.corner_radii.bottom_left = Some(inline_radius.into());
    inline_code_style.corner_radii.bottom_right = Some(inline_radius.into());

    TextViewStyle::default()
        .with_foreground(COLOR_TEXT_MUTED)
        .with_muted_foreground(COLOR_TEXT_MUTED)
        .with_link(COLOR_ACCENT_ORANGE)
        .with_selection(COLOR_ACCENT_ORANGE_SELECTION)
        .with_code_background(COLOR_BG_ELEVATED)
        .with_border(hsla(0.0, 0.0, 0.0, 0.0))
        .with_inline_code(gpui::HighlightStyle {
            background_color: Some(COLOR_BG_ELEVATED),
            color: Some(COLOR_TEXT_SUBTLE),
            ..Default::default()
        })
        .with_inline_code_style(inline_code_style)
        .with_dark(true)
}

fn assistant_markdown_style() -> TextViewStyle {
    let mut code_block = StyleRefinement::default();
    let radius = px(6.0);
    code_block.corner_radii.top_left = Some(radius.into());
    code_block.corner_radii.top_right = Some(radius.into());
    code_block.corner_radii.bottom_left = Some(radius.into());
    code_block.corner_radii.bottom_right = Some(radius.into());

    let mut inline_code_style = StyleRefinement::default();
    let inline_radius = px(4.0);
    inline_code_style.corner_radii.top_left = Some(inline_radius.into());
    inline_code_style.corner_radii.top_right = Some(inline_radius.into());
    inline_code_style.corner_radii.bottom_left = Some(inline_radius.into());
    inline_code_style.corner_radii.bottom_right = Some(inline_radius.into());

    TextViewStyle::default()
        .with_foreground(COLOR_TEXT_PRIMARY)
        .with_muted_foreground(COLOR_TEXT_MUTED)
        .with_link(COLOR_ACCENT_ORANGE)
        .with_selection(COLOR_ACCENT_ORANGE_SELECTION)
        .with_code_background(COLOR_BG_CANVAS)
        .with_border(COLOR_BORDER)
        .with_code_block(code_block)
        .with_inline_code(gpui::HighlightStyle {
            background_color: Some(COLOR_BG_CANVAS),
            color: Some(COLOR_TEXT_SUBTLE),
            ..Default::default()
        })
        .with_inline_code_style(inline_code_style)
        .with_dark(true)
}

fn user_markdown_style() -> TextViewStyle {
    TextViewStyle::default()
        .with_foreground(COLOR_TEXT_PRIMARY)
        .with_muted_foreground(COLOR_TEXT_MUTED)
        .with_link(COLOR_ACCENT_ORANGE)
        .with_selection(COLOR_ACCENT_ORANGE_SELECTION)
        .with_code_background(COLOR_BG_CANVAS.opacity(0.45))
        .with_border(COLOR_BORDER_GLASS)
        .with_dark(true)
}
