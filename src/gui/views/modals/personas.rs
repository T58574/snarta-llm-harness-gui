use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_ELEVATED, COLOR_PINK_MUTED_BG,
    COLOR_PINK_PRIMARY, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{IconName, Sizable, Size, h_flex, v_flex};

pub struct PersonasModal;

impl PersonasModal {
    pub fn render(
        state_entity: &Entity<AppState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let active_persona = state.persona.name.clone();
        let personas = state.available_personas.clone();
        let personas_dir = state.paths.personas_dir();

        v_flex()
            .w(rems(42.0))
            .p_4()
            .rounded_xl()
            .bg(COLOR_GRAPHITE_CANVAS)
            .border_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .shadow_lg()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(rems(1.1))
                            .font_weight(FontWeight::BOLD)
                            .child("Выбор персоны собеседника"),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("open-personas-folder")
                                    .secondary()
                                    .with_size(Size::Small)
                                    .icon(IconName::Folder)
                                    .label("Открыть папку")
                                    .on_click({
                                        let dir = personas_dir.clone();
                                        move |_, _, _cx| {
                                            crate::vision::open_in_explorer(&dir);
                                        }
                                    }),
                            )
                            .child(
                                Button::new("close-personas")
                                    .ghost()
                                    .with_size(Size::Small)
                                    .icon(IconName::Close)
                                    .tooltip("Закрыть")
                                    .on_click({
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |s, cx| {
                                                s.active_modal = None;
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .id("personas-scroll")
                    .max_h(rems(24.0))
                    .overflow_y_scroll()
                    .gap_2()
                    .children(personas.into_iter().map(|p| {
                        let is_active = p.name == active_persona;
                        let name = p.name.clone();
                        let desc = p.description.clone().unwrap_or_default();
                        let prompt_preview = {
                            let clean = p.system_prompt.trim();
                            let chars: Vec<char> = clean.chars().collect();
                            if chars.len() > 220 {
                                format!("{}...", chars[..220].iter().collect::<String>())
                            } else {
                                clean.to_string()
                            }
                        };
                        let p_path = p.path.clone();

                        v_flex()
                            .p_3()
                            .rounded_lg()
                            .bg(if is_active {
                                COLOR_PINK_MUTED_BG
                            } else {
                                COLOR_GRAPHITE_ELEVATED
                            })
                            .border_1()
                            .border_color(if is_active {
                                COLOR_PINK_PRIMARY.opacity(0.6)
                            } else {
                                COLOR_GRAPHITE_BORDER
                            })
                            .gap_1p5()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, {
                                                let state_entity = state_entity.clone();
                                                let pname = name.clone();
                                                move |_, _, cx| {
                                                    state_entity.update(cx, |s, cx| {
                                                        s.switch_persona(&pname);
                                                        s.active_modal = None;
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(if is_active {
                                                        COLOR_PINK_PRIMARY
                                                    } else {
                                                        COLOR_TEXT_PRIMARY
                                                    })
                                                    .child(name.clone()),
                                            )
                                            .children((!desc.is_empty()).then(|| {
                                                div()
                                                    .text_size(rems(0.75))
                                                    .text_color(COLOR_TEXT_MUTED)
                                                    .child(format!("· {desc}"))
                                            })),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                Button::new(format!("open-persona-file-{}", name))
                                                    .ghost()
                                                    .with_size(Size::XSmall)
                                                    .icon(IconName::FolderOpen)
                                                    .label("В проводнике")
                                                    .on_click({
                                                        let path = p_path.clone();
                                                        move |_, _, _cx| {
                                                            crate::vision::open_in_explorer(&path);
                                                        }
                                                    }),
                                            )
                                            .child(if is_active {
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(COLOR_PINK_PRIMARY.opacity(0.15))
                                                    .text_color(COLOR_PINK_PRIMARY)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(rems(0.75))
                                                    .child("Активна")
                                                    .into_any_element()
                                            } else {
                                                Button::new(format!("select-persona-{}", name))
                                                    .primary()
                                                    .with_size(Size::XSmall)
                                                    .label("Выбрать")
                                                    .on_click({
                                                        let state_entity = state_entity.clone();
                                                        let pname = name.clone();
                                                        move |_, _, cx| {
                                                            state_entity.update(cx, |s, cx| {
                                                                s.switch_persona(&pname);
                                                                s.active_modal = None;
                                                                cx.notify();
                                                            });
                                                        }
                                                    })
                                                    .into_any_element()
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .p_2()
                                    .rounded_md()
                                    .bg(COLOR_GRAPHITE_CANVAS)
                                    .text_size(rems(0.76))
                                    .text_color(COLOR_TEXT_MUTED)
                                    .child(prompt_preview),
                            )
                    })),
            )
    }
}
