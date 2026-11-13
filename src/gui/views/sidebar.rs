use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_ELEVATED, COLOR_GRAPHITE_SIDEBAR, COLOR_PINK_MUTED_BG,
    COLOR_PINK_PRIMARY, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY, SIDEBAR_WIDTH,
};
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme, IconName, Sizable, Size, h_flex, v_flex};

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

        v_flex()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .border_r_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .bg(COLOR_GRAPHITE_SIDEBAR)
            .p_2()
            .gap_2()
            .child(
                Button::new("new-session-btn")
                    .primary()
                    .w_full()
                    .label("+ Новая сессия")
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
                    .text_size(rems(0.75))
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().muted_foreground)
                    .px_1()
                    .child("ИСТОРИЯ СЕССИЙ"),
            )
            .child(
                v_flex()
                    .id("sessions-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(sessions.into_iter().map(|item| {
                        let is_active = item.path == active_path && !settings_open;
                        let item_path = item.path.clone();
                        let item_title = item.title.clone();
                        let count = item.message_count;

                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .p_2()
                            .gap_1p5()
                            .rounded_md()
                            .bg(if is_active {
                                COLOR_PINK_MUTED_BG
                            } else {
                                COLOR_GRAPHITE_SIDEBAR
                            })
                            .hover(|s| s.bg(COLOR_GRAPHITE_ELEVATED))
                            .children(is_active.then(|| {
                                div()
                                    .w(px(3.0))
                                    .h(px(16.0))
                                    .rounded_full()
                                    .bg(COLOR_PINK_PRIMARY)
                            }))
                            .child(
                                div()
                                    .flex_1()
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
                                                    .text_size(rems(0.85))
                                                    .font_weight(if is_active {
                                                        FontWeight::SEMIBOLD
                                                    } else {
                                                        FontWeight::NORMAL
                                                    })
                                                    .text_color(if is_active {
                                                        COLOR_PINK_PRIMARY
                                                    } else {
                                                        COLOR_TEXT_PRIMARY
                                                    })
                                                    .child(item_title),
                                            )
                                            .child(
                                                div()
                                                    .text_size(rems(0.7))
                                                    .text_color(COLOR_TEXT_MUTED)
                                                    .child(format!("{count} сообщений")),
                                            ),
                                    ),
                            )
                            .child(
                                Button::new(format!("del-{}", item.id))
                                    .ghost()
                                    .danger()
                                    .with_size(Size::XSmall)
                                    .icon(IconName::Close)
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
                    })),
            )
            .child(
                h_flex()
                    .w_full()
                    .pt_2()
                    .gap_1()
                    .border_t_1()
                    .border_color(COLOR_GRAPHITE_BORDER)
                    .child(
                        Button::new("sidebar-settings-btn")
                            .with_variant(if settings_open {
                                gpui_component::button::ButtonVariant::Primary
                            } else {
                                gpui_component::button::ButtonVariant::Secondary
                            })
                            .w_full()
                            .with_size(Size::Small)
                            .icon(IconName::Settings)
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
