use crate::gui::state::{AppState, ServerStatus};
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_ELEVATED, COLOR_PINK_MUTED_BG,
    COLOR_PINK_PRIMARY, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{IconName, Sizable, Size, h_flex, v_flex};

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
                            .text_color(COLOR_TEXT_PRIMARY)
                            .child("Выбор и запуск моделей"),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("open-models-folder-btn")
                                    .secondary()
                                    .with_size(Size::Small)
                                    .icon(IconName::Folder)
                                    .label("Открыть папку")
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
                                    .with_size(Size::Small)
                                    .icon(IconName::RotateCw)
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
                    .id("models-scroll")
                    .max_h(rems(24.0))
                    .overflow_y_scroll()
                    .gap_1p5()
                    .children(if models.is_empty() {
                        vec![v_flex()
                            .p_6()
                            .items_center()
                            .justify_center()
                            .gap_3()
                            .bg(COLOR_GRAPHITE_ELEVATED)
                            .rounded_lg()
                            .border_1()
                            .border_color(COLOR_GRAPHITE_BORDER)
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(rems(0.95))
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child("Модели .gguf не найдены"),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.8))
                                    .text_color(COLOR_TEXT_MUTED)
                                    .child(format!(
                                        "Поместите файлы моделей формата .gguf в папку:\n{models_dir_str}"
                                    )),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("empty-open-folder-btn")
                                            .secondary()
                                            .with_size(Size::Small)
                                            .icon(IconName::Folder)
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
                                            .icon(IconName::RotateCw)
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
                            .into_any_element()]
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
                                    .hover(|s| s.bg(COLOR_GRAPHITE_BORDER))
                                    .child(
                                        v_flex()
                                            .flex_1()
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
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(if is_active {
                                                        COLOR_PINK_PRIMARY
                                                    } else {
                                                        COLOR_TEXT_PRIMARY
                                                    })
                                                    .child(file_name.clone()),
                                            )
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .text_size(rems(0.75))
                                                    .text_color(COLOR_TEXT_MUTED)
                                                    .child(
                                                        div()
                                                            .px_1p5()
                                                            .py_0p5()
                                                            .rounded_md()
                                                            .bg(COLOR_GRAPHITE_BORDER)
                                                            .text_color(COLOR_TEXT_PRIMARY)
                                                            .child(quant),
                                                    )
                                                    .child(div().child(format!("{arch} • {:.2} GB • {ctx_k}k ctx", size_gib)))
                                                    .children(if has_mtp {
                                                        Some(
                                                            div()
                                                                .px_1p5()
                                                                .py_0p5()
                                                                .rounded_md()
                                                                .bg(COLOR_PINK_PRIMARY.opacity(0.2))
                                                                .text_color(COLOR_PINK_PRIMARY)
                                                                .child("Черновик MTP"),
                                                        )
                                                    } else {
                                                        None
                                                    })
                                                    .children(match reasoning_mode {
                                                        crate::gguf::ReasoningMode::Toggleable => Some(
                                                            div()
                                                                .px_1p5()
                                                                .py_0p5()
                                                                .rounded_md()
                                                                .bg(COLOR_GRAPHITE_BORDER)
                                                                .text_color(COLOR_TEXT_PRIMARY)
                                                                .child("Рассуждения"),
                                                        ),
                                                        crate::gguf::ReasoningMode::ForcedInTemplate => Some(
                                                            div()
                                                                .px_1p5()
                                                                .py_0p5()
                                                                .rounded_md()
                                                                .bg(COLOR_GRAPHITE_BORDER)
                                                                .text_color(COLOR_TEXT_PRIMARY)
                                                                .child("Рассуждения в шаблоне"),
                                                        ),
                                                        crate::gguf::ReasoningMode::None => None,
                                                    }),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .children(if is_active {
                                                Some(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_md()
                                                        .bg(COLOR_PINK_PRIMARY.opacity(0.15))
                                                        .text_color(COLOR_PINK_PRIMARY)
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_size(rems(0.75))
                                                        .child("Выбрана"),
                                                )
                                            } else {
                                                None
                                            })
                                            .child(if is_running {
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .px_2()
                                                            .py_1()
                                                            .rounded_md()
                                                            .bg(rgb(0x22c55e).opacity(0.15))
                                                            .text_color(rgb(0x22c55e))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_size(rems(0.75))
                                                            .child("В сервере"),
                                                    )
                                                    .child(
                                                        Button::new(format!("stop-srv-{}", file_name))
                                                            .danger()
                                                            .with_size(Size::Small)
                                                            .label("Остановить")
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
                                                div()
                                                    .text_color(COLOR_PINK_PRIMARY)
                                                    .text_size(rems(0.8))
                                                    .child("Запуск...")
                                                    .into_any_element()
                                            } else {
                                                Button::new(format!("start-srv-{}", file_name))
                                                    .primary()
                                                    .with_size(Size::Small)
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
