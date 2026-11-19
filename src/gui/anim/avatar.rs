use crate::gui::state::{ActiveModal, AppState};
use crate::gui::theme::{
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED, COLOR_BORDER_GLASS, COLOR_GLASS_SURFACE,
    COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use crate::persona::Persona;
use gpui::*;
use gpui_component::{h_flex, v_flex};
use std::sync::Arc;

/// Renders a circular photo avatar of `size_px` from an in-memory content-hashed `Arc<gpui::Image>`
/// (or a clean initial badge if no photo is set).
pub fn render_avatar_circle(
    display_name: &str,
    avatar_image: Option<&Arc<gpui::Image>>,
    size_px: f32,
) -> AnyElement {
    let initial = display_name
        .chars()
        .next()
        .unwrap_or('S')
        .to_uppercase()
        .to_string();

    if let Some(image_arc) = avatar_image {
        div()
            .w(px(size_px))
            .h(px(size_px))
            .rounded_full()
            .overflow_hidden()
            .flex_shrink_0()
            .border_1()
            .border_color(COLOR_BORDER_GLASS)
            .bg(COLOR_GLASS_SURFACE)
            .child(
                img(image_arc.clone())
                    .w(px(size_px))
                    .h(px(size_px))
                    .object_fit(ObjectFit::Cover),
            )
            .into_any_element()
    } else {
        div()
            .w(px(size_px))
            .h(px(size_px))
            .rounded_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(COLOR_ACCENT_ORANGE_MUTED)
            .border_1()
            .border_color(COLOR_ACCENT_ORANGE.opacity(0.45))
            .text_size(px((size_px * 0.42).max(11.0)))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(COLOR_ACCENT_ORANGE)
            .child(initial)
            .into_any_element()
    }
}

/// Sidebar persona card with circular photo avatar, name, and age/character subtitle.
pub fn render_sidebar_avatar(
    persona: &Persona,
    is_generating: bool,
    state_entity: &Entity<AppState>,
) -> impl IntoElement {
    let subtitle = match (&persona.age, &persona.character) {
        (Some(age), Some(ch)) => format!("{age} · {ch}"),
        (Some(age), None) => format!("Возраст: {age}"),
        (None, Some(ch)) => ch.clone(),
        (None, None) => persona
            .description
            .clone()
            .unwrap_or_else(|| "Локальная персона".to_string()),
    };

    h_flex()
        .id("sidebar-persona-card")
        .w_full()
        .items_center()
        .gap_2p5()
        .p_2()
        .rounded_xl()
        .bg(COLOR_GLASS_SURFACE)
        .border_1()
        .border_color(if is_generating {
            COLOR_ACCENT_ORANGE.opacity(0.55)
        } else {
            COLOR_BORDER_GLASS
        })
        .cursor_pointer()
        .hover(|s| s.border_color(COLOR_ACCENT_ORANGE.opacity(0.6)))
        .on_click({
            let state_entity = state_entity.clone();
            move |_, _, cx| {
                state_entity.update(cx, |s, cx| {
                    s.open_modal(ActiveModal::Personas);
                    cx.notify();
                });
            }
        })
        .child(render_avatar_circle(
            &persona.display_name,
            persona.avatar_image.as_ref(),
            42.0,
        ))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(rems(0.90))
                        .text_color(COLOR_TEXT_PRIMARY)
                        .child(persona.display_name.clone()),
                )
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .text_size(rems(0.72))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(subtitle),
                ),
        )
}

/// Centered hero avatar shown when a chat session is empty.
pub fn render_hero_avatar(persona: &Persona, state_entity: &Entity<AppState>) -> impl IntoElement {
    let meta_line = match (&persona.age, &persona.character) {
        (Some(age), Some(ch)) => Some(format!("{age} · {ch}")),
        (Some(age), None) => Some(format!("Возраст: {age}")),
        (None, Some(ch)) => Some(ch.clone()),
        (None, None) => persona.description.clone(),
    };

    v_flex()
        .items_center()
        .justify_center()
        .gap_3()
        .p_6()
        .rounded_2xl()
        .bg(COLOR_GLASS_SURFACE)
        .border_1()
        .border_color(COLOR_BORDER_GLASS)
        .child(
            div()
                .id("hero-avatar-btn")
                .cursor_pointer()
                .on_click({
                    let state_entity = state_entity.clone();
                    move |_, _, cx| {
                        state_entity.update(cx, |s, cx| {
                            s.open_modal(ActiveModal::Personas);
                            cx.notify();
                        });
                    }
                })
                .child(render_avatar_circle(
                    &persona.display_name,
                    persona.avatar_image.as_ref(),
                    92.0,
                )),
        )
        .child(
            div()
                .text_size(rems(1.15))
                .font_weight(FontWeight::BOLD)
                .text_color(COLOR_TEXT_PRIMARY)
                .child(persona.display_name.clone()),
        )
        .children(meta_line.map(|line| {
            div()
                .max_w(rems(28.0))
                .text_center()
                .text_size(rems(0.84))
                .text_color(COLOR_TEXT_MUTED)
                .child(line)
        }))
        .child(
            div()
                .text_size(rems(0.80))
                .text_color(COLOR_TEXT_MUTED)
                .child("Напишите сообщение или перетащите файл в строку ввода."),
        )
}
