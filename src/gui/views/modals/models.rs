use crate::gui::icons::icons;
use crate::gui::state::{AppState, ServerStatus};
use crate::gui::theme::{
    COLOR_ACCENT_BLUE, COLOR_ACCENT_BLUE_MUTED, COLOR_ACCENT_GREEN, COLOR_ACCENT_GREEN_MUTED,
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED, COLOR_BG_CANVAS, COLOR_BG_ELEVATED,
    COLOR_BG_ELEVATED_HOVER, COLOR_BORDER, COLOR_BORDER_GLASS_STRONG, COLOR_GLASS_MODAL,
    COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Sizable, Size, h_flex, v_flex};

pub struct ModelsModal;

impl ModelsModal {
    pub fn render(
        state_entity: &Entity<AppState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let active_model = state.cfg.active_model.clone();
        let models = state.available_models.clone();
        let models_dir_path = state.paths.models_dir();
        let models_dir_str = models_dir_path.display().to_string();
        let running_model = match &state.server_status {
            ServerStatus::Running { model, .. } => Some(model.clone()),
            _ => None,
        };
        let starting_model = match &state.server_status {
            ServerStatus::Starting { model, .. } => Some(model.clone()),
            _ => None,
        };

        v_flex()
            .w(rems(46.0))
            .p_5()
            .rounded_2xl()
            .bg(COLOR_GLASS_MODAL)
            .border_1()
            .border_color(COLOR_BORDER_GLASS_STRONG)
            .shadow_2xl()
            .gap_3p5()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(icons::cpu().with_size(Size::Medium).text_color(COLOR_ACCENT_ORANGE))
                            .child(
                                div()
                                    .text_size(rems(1.1))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child("Модели GGUF"),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Button::new("open-models-folder-btn")
                                    .secondary()
                                    .outline()
                                    .with_size(Size::Small)
                                    .icon(icons::folder())
                                    .label("Папка моделей")
                                    .on_click({
                                        let dir = models_dir_path.clone();
                                        move |_, _, _cx| {
                                            crate::vision::open_in_explorer(&dir);
                                        }
                                    }),
                            )
                            .child(
                                Button::new("refresh-models-btn")
                                    .secondary()
                                    .outline()
                                    .with_size(Size::Small)
                                    .icon(icons::refresh())
                                    .label("Обновить")
                                    .on_click({
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |s, cx| {
                                                s.rescan_models();
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("close-models")
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
                    .id("models-scroll")
                    .max_h(rems(25.0))
                    .overflow_y_scroll()
                    .gap_2()
                    .children(if models.is_empty() {
                        vec![
                            v_flex()
                                .p_6()
                                .items_center()
                                .justify_center()
                                .gap_3()
                                .bg(COLOR_BG_ELEVATED)
                                .rounded_lg()
                                .border_1()
                                .border_color(COLOR_BORDER)
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_size(rems(0.95))
                                        .text_color(COLOR_TEXT_PRIMARY)
                                        .child("Файлы .gguf не найдены"),
                                )
                                .child(
                                    div()
                                        .text_size(rems(0.8))
                                        .text_color(COLOR_TEXT_MUTED)
                                        .child(format!("Поместите модели .gguf в папку {models_dir_str}")),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new("empty-open-folder-btn")
                                                .secondary()
                                                .outline()
                                                .with_size(Size::Small)
                                                .icon(icons::folder())
                                                .label("Открыть папку")
                                                .on_click({
                                                    let dir = models_dir_path.clone();
                                                    move |_, _, _cx| {
                                                        crate::vision::open_in_explorer(&dir);
                                                    }
                                                }),
                                        )
                                        .child(
                                            Button::new("empty-refresh-btn")
                                                .primary()
                                                .with_size(Size::Small)
                                                .icon(icons::refresh())
                                                .label("Обновить список")
                                                .on_click({
                                                    let state_entity = state_entity.clone();
                                                    move |_, _, cx| {
                                                        state_entity.update(cx, |s, cx| {
                                                            s.rescan_models();
                                                            cx.notify();
                                                        });
                                                    }
                                                }),
                                        ),
                                )
                                .into_any_element(),
                        ]
                    } else {
                        models
                            .into_iter()
                            .map(|m| {
                                let is_active = Some(&m.file_name) == active_model.as_ref();
                                let is_running = running_model.as_deref() == Some(&m.file_name);
                                let is_starting = starting_model.as_deref() == Some(&m.file_name);
                                let file_name = m.file_name.clone();
                                let size_gib = m.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                                let arch = m.architecture.clone();
                                let quant = m.quant_label.clone();
                                let ctx_k = m.context_length / 1024;
                                let has_mtp = m.has_mtp()
                                    || crate::server::args::find_matching_mtp_layer(&m.path).is_some();
                                let reasoning_mode = m.reasoning_mode;

                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .p_3()
                                    .gap_3()
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
                                    .hover(|s| s.bg(COLOR_BG_ELEVATED_HOVER))
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_1()
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, {
                                                let state_entity = state_entity.clone();
                                                let fname = file_name.clone();
                                                move |_, _, cx| {
                                                    state_entity.update(cx, |s, cx| {
                                                        s.switch_model(&fname);
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                            .child(
                                                div()
                                                    .w_full()
                                                    .truncate()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(if is_active {
                                                        COLOR_ACCENT_ORANGE
                                                    } else {
                                                        COLOR_TEXT_PRIMARY
                                                    })
                                                    .child(file_name.clone()),
                                            )
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .flex_wrap()
                                                    .gap_1p5()
                                                    .text_size(rems(0.74))
                                                    .text_color(COLOR_TEXT_MUTED)
                                                    .child(
                                                        div()
                                                            .px_1p5()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(COLOR_BG_CANVAS)
                                                            .border_1()
                                                            .border_color(COLOR_BORDER)
                                                            .text_color(COLOR_TEXT_PRIMARY)
                                                            .child(quant),
                                                    )
                                                    .child(div().child(format!(
                                                        "{arch} · {size_gib:.2} GB · {ctx_k}k ctx"
                                                    )))
                                                    .children(has_mtp.then(|| {
                                                        div()
                                                            .px_1p5()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(COLOR_ACCENT_BLUE_MUTED)
                                                            .text_color(COLOR_ACCENT_BLUE)
                                                            .child("MTP")
                                                    }))
                                                    .children(match reasoning_mode {
                                                        crate::gguf::ReasoningMode::Toggleable => Some(
                                                            div()
                                                                .px_1p5()
                                                                .py_0p5()
                                                                .rounded_sm()
                                                                .bg(COLOR_BG_CANVAS)
                                                                .text_color(COLOR_TEXT_MUTED)
                                                                .child("Рассуждения"),
                                                        ),
                                                        crate::gguf::ReasoningMode::ForcedInTemplate => Some(
                                                            div()
                                                                .px_1p5()
                                                                .py_0p5()
                                                                .rounded_sm()
                                                                .bg(COLOR_BG_CANVAS)
                                                                .text_color(COLOR_TEXT_MUTED)
                                                                .child("Шаблон рассуждений"),
                                                        ),
                                                        crate::gguf::ReasoningMode::None => None,
                                                    }),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_shrink_0()
                                            .gap_2()
                                            .items_center()
                                            .children(is_active.then(|| {
                                                div()
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(COLOR_ACCENT_ORANGE.opacity(0.16))
                                                    .text_color(COLOR_ACCENT_ORANGE)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(rems(0.74))
                                                    .child("Выбрана")
                                            }))
                                            .child(if is_running {
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .px_2()
                                                            .py_1()
                                                            .rounded_md()
                                                            .bg(COLOR_ACCENT_GREEN_MUTED)
                                                            .text_color(COLOR_ACCENT_GREEN)
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_size(rems(0.74))
                                                            .child("Работает"),
                                                    )
                                                    .child(
                                                        Button::new(format!("stop-srv-{}", file_name))
                                                            .danger()
                                                            .with_size(Size::Small)
                                                            .icon(icons::square_stop())
                                                            .label("Стоп")
                                                            .on_click({
                                                                let state_entity = state_entity.clone();
                                                                move |_, _, cx| {
                                                                    state_entity.update(cx, |s, cx| {
                                                                        s.stop_server();
                                                                        cx.notify();
                                                                    });
                                                                }
                                                            }),
                                                    )
                                                    .into_any_element()
                                            } else if is_starting {
                                                Button::new(format!("starting-srv-{}", file_name))
                                                    .secondary()
                                                    .outline()
                                                    .with_size(Size::Small)
                                                    .loading(true)
                                                    .label("Запуск...")
                                                    .into_any_element()
                                            } else {
                                                Button::new(format!("start-srv-{}", file_name))
                                                    .primary()
                                                    .with_size(Size::Small)
                                                    .icon(icons::play())
                                                    .label("Запустить")
                                                    .on_click({
                                                        let state_entity = state_entity.clone();
                                                        let fname = file_name.clone();
                                                        move |_, _, cx| {
                                                            state_entity.update(cx, |s, cx| {
                                                                let settings = s.settings_for(&fname);
                                                                s.switch_model(&fname);
                                                                s.start_server_for_model(
                                                                    &fname,
                                                                    settings.ctx,
                                                                );
                                                                cx.notify();
                                                            });
                                                        }
                                                    })
                                                    .into_any_element()
                                            }),
                                    )
                                    .into_any_element()
                            })
                            .collect()
                    }),
            )
    }
}
