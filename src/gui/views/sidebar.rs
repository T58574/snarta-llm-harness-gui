use crate::gui::anim::avatar::render_sidebar_avatar;
use crate::gui::icons::icons;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED, COLOR_BG_ELEVATED, COLOR_BG_SIDEBAR,
    COLOR_BORDER, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY, SIDEBAR_WIDTH,
};
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme, Sizable, Size, h_flex, v_flex};

pub struct Sidebar;

impl Sidebar {
    pub fn render(
        state_entity: &Entity<AppState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let active_path = state.session.path.clone();
        let sessions = state.sessions_list.clone();
        let settings_open = state.settings_open;
        let persona_name = state.persona.name.clone();
        let persona_gender = state.persona.gender;
        let avatar_emotion = state.avatar_emotion;
        let avatar_thought = state.avatar_thought.clone();
        let is_generating = state.is_generating;

        v_flex()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .border_r_1()
            .border_color(COLOR_BORDER)
            .bg(COLOR_BG_SIDEBAR)
            .p_2p5()
            .gap_2p5()
            .child(render_sidebar_avatar(
                &persona_name,
                persona_gender,
                avatar_emotion,
                avatar_thought.as_deref(),
                is_generating,
                state_entity,
                cx,
            ))
            .child(
                Button::new("new-session-btn")
                    .primary()
                    .w_full()
                    .with_size(Size::Medium)
                    .icon(icons::plus())
                    .label("Новая сессия")
                    .on_click({
                        let state_entity = state_entity.clone();
                        move |_, _, cx| {
                            state_entity.update(cx, |s, cx| {
                                s.settings_open = false;
                                s.create_new_session();
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                div()
                    .text_size(rems(0.72))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
                    .px_1()
                    .pt_1()
                    .child("История сессий"),
            )
            .child(
                v_flex()
                    .id("sessions-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(if sessions.is_empty() {
                        vec![
                            div()
                                .px_2()
                                .py_3()
                                .text_size(rems(0.78))
                                .text_color(COLOR_TEXT_MUTED)
                                .child("Сохранённых диалогов пока нет.")
                                .into_any_element(),
                        ]
                    } else {
                        sessions
                            .into_iter()
                            .map(|item| {
                                let is_active = item.path == active_path && !settings_open;
                                let item_path = item.path.clone();
                                let item_title = item.title.clone();
                                let count = item.message_count;

                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .justify_between()
                                    .px_2()
                                    .py_1p5()
                                    .gap_1p5()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(if is_active {
                                        COLOR_ACCENT_ORANGE.opacity(0.4)
                                    } else {
                                        hsla(0.0, 0.0, 0.0, 0.0)
                                    })
                                    .bg(if is_active {
                                        COLOR_ACCENT_ORANGE_MUTED
                                    } else {
                                        COLOR_BG_SIDEBAR
                                    })
                                    .hover(|s| s.bg(COLOR_BG_ELEVATED))
                                    .children(is_active.then(|| {
                                        div()
                                            .w(px(3.0))
                                            .h(px(16.0))
                                            .rounded_full()
                                            .bg(COLOR_ACCENT_ORANGE)
                                    }))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, {
                                                let state_entity = state_entity.clone();
                                                let path = item_path.clone();
                                                move |_, _, cx| {
                                                    state_entity.update(cx, |s, cx| {
                                                        s.settings_open = false;
                                                        s.switch_session(&path);
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                            .child(
                                                v_flex()
                                                    .w_full()
                                                    .overflow_hidden()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .truncate()
                                                            .text_size(rems(0.84))
                                                            .font_weight(if is_active {
                                                                FontWeight::SEMIBOLD
                                                            } else {
                                                                FontWeight::NORMAL
                                                            })
                                                            .text_color(if is_active {
                                                                COLOR_ACCENT_ORANGE
                                                            } else {
                                                                COLOR_TEXT_PRIMARY
                                                            })
                                                            .child(item_title),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.7))
                                                            .text_color(COLOR_TEXT_MUTED)
                                                            .child(format_msg_count(count)),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        Button::new(format!("del-{}", item.id))
                                            .ghost()
                                            .with_size(Size::XSmall)
                                            .icon(icons::trash())
                                            .tooltip("Удалить сессию")
                                            .on_click({
                                                let state_entity = state_entity.clone();
                                                let path = item_path;
                                                move |_, _, cx| {
                                                    state_entity.update(cx, |s, cx| {
                                                        s.delete_session(&path);
                                                        cx.notify();
                                                    });
                                                }
                                            }),
                                    )
                                    .into_any_element()
                            })
                            .collect()
                    }),
            )
            .child(
                h_flex()
                    .w_full()
                    .pt_2()
                    .gap_1()
                    .border_t_1()
                    .border_color(COLOR_BORDER)
                    .child(
                        Button::new("sidebar-settings-btn")
                            .with_variant(if settings_open {
                                gpui_component::button::ButtonVariant::Primary
                            } else {
                                gpui_component::button::ButtonVariant::Secondary
                            })
                            .outline()
                            .w_full()
                            .with_size(Size::Small)
                            .icon(icons::settings())
                            .label("Настройки")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.settings_open = !s.settings_open;
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
    }
}

fn format_msg_count(n: usize) -> String {
    let rem100 = n % 100;
    let rem10 = n % 10;
    let word = if (11..=14).contains(&rem100) {
        "сообщений"
    } else if rem10 == 1 {
        "сообщение"
    } else if (2..=4).contains(&rem10) {
        "сообщения"
    } else {
        "сообщений"
    };
    format!("{n} {word}")
}
