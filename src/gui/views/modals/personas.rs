use crate::gui::anim::avatar::{AvatarEmotion, render_mini_avatar};
use crate::gui::icons::icons;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED, COLOR_BG_CANVAS, COLOR_BG_ELEVATED,
    COLOR_BORDER, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use crate::persona::PersonaGender;
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Sizable, Size, h_flex, v_flex};

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
            .w(rems(46.0))
            .p_5()
            .rounded_xl()
            .bg(COLOR_BG_CANVAS)
            .border_1()
            .border_color(COLOR_BORDER)
            .shadow_lg()
            .gap_3p5()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(icons::user().with_size(Size::Medium).text_color(COLOR_ACCENT_ORANGE))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_size(rems(1.1))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(COLOR_TEXT_PRIMARY)
                                            .child("Персоны собеседника"),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.75))
                                            .text_color(COLOR_TEXT_MUTED)
                                            .child("Выберите персону и пол её ASCII-арта (сохраняется в .md)"),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Button::new("open-personas-folder")
                                    .secondary()
                                    .outline()
                                    .with_size(Size::Small)
                                    .icon(icons::folder())
                                    .label("Папка персон")
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
                                    .icon(icons::close())
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
                    .max_h(rems(28.0))
                    .overflow_y_scroll()
                    .gap_2p5()
                    .children(personas.into_iter().map(|p| {
                        let is_active = p.name == active_persona;
                        let name = p.name.clone();
                        let gender = p.gender;
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

                        let gender_btn = |target: PersonaGender, btn_id: String| {
                            let is_sel = gender == target;
                            let state_entity = state_entity.clone();
                            let pname = name.clone();
                            let btn = Button::new(btn_id)
                                .with_size(Size::XSmall)
                                .label(target.label_ru())
                                .tooltip("Задать пол ASCII-арта для этой персоны")
                                .on_click(move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.set_persona_gender(&pname, target);
                                        cx.notify();
                                    });
                                });
                            if is_sel {
                                btn.primary()
                            } else {
                                btn.secondary().outline()
                            }
                        };

                        v_flex()
                            .p_3()
                            .rounded_lg()
                            .bg(if is_active {
                                COLOR_ACCENT_ORANGE_MUTED
                            } else {
                                COLOR_BG_ELEVATED
                            })
                            .border_1()
                            .border_color(if is_active {
                                COLOR_ACCENT_ORANGE.opacity(0.6)
                            } else {
                                COLOR_BORDER
                            })
                            .gap_2p5()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .items_center()
                                            .gap_2p5()
                                            .child(render_mini_avatar(gender, AvatarEmotion::Neutral, cx))
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap_1()
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
                                                                        COLOR_ACCENT_ORANGE
                                                                    } else {
                                                                        COLOR_TEXT_PRIMARY
                                                                    })
                                                                    .child(name.clone()),
                                                            )
                                                            .children((!desc.is_empty()).then(|| {
                                                                div()
                                                                    .min_w_0()
                                                                    .truncate()
                                                                    .text_size(rems(0.75))
                                                                    .text_color(COLOR_TEXT_MUTED)
                                                                    .child(desc)
                                                            })),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_1p5()
                                                            .child(
                                                                div()
                                                                    .text_size(rems(0.72))
                                                                    .text_color(COLOR_TEXT_MUTED)
                                                                    .child("Пол арта:"),
                                                            )
                                                            .child(gender_btn(
                                                                PersonaGender::Female,
                                                                format!("persona-gender-f-{}", name),
                                                            ))
                                                            .child(gender_btn(
                                                                PersonaGender::Male,
                                                                format!("persona-gender-m-{}", name),
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_shrink_0()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                Button::new(format!("open-persona-file-{}", name))
                                                    .ghost()
                                                    .with_size(Size::XSmall)
                                                    .icon(icons::folder_open())
                                                    .label("Файл")
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
                                                    .bg(COLOR_ACCENT_ORANGE.opacity(0.16))
                                                    .text_color(COLOR_ACCENT_ORANGE)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(rems(0.75))
                                                    .child("Активна")
                                                    .into_any_element()
                                            } else {
                                                Button::new(format!("select-persona-{}", name))
                                                    .primary()
                                                    .with_size(Size::XSmall)
                                                    .icon(icons::check())
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
                                    .bg(COLOR_BG_CANVAS)
                                    .text_size(rems(0.76))
                                    .text_color(COLOR_TEXT_MUTED)
                                    .child(prompt_preview),
                            )
                    })),
            )
    }
}
