use super::{SettingsModal, SettingsState};
use crate::gguf::ReasoningMode;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_ELEVATED, COLOR_PINK_MUTED_BG, COLOR_PINK_PRIMARY,
    COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{IconName, Sizable, Size, h_flex, v_flex};

impl SettingsModal {
    pub(super) fn render_models_tab(
        &mut self,
        state_entity: &Entity<AppState>,
        cx: &mut App,
    ) -> AnyElement {
        let (available_models, has_override, default_settings) = {
            let st = state_entity.read(cx);
            let sel = self.shared.borrow().selected_model.clone();
            let has_ov = sel
                .as_deref()
                .is_some_and(|name| st.cfg.models.contains_key(name));
            (
                st.available_models.clone(),
                has_ov,
                st.cfg.model_defaults.clone(),
            )
        };

        let (
            selected_model,
            kv_type,
            flash_attn,
            mlock,
            mmap,
            prompt_cache,
            reasoning_effort,
            reasoning_preserve,
        ) = {
            let s = self.shared.borrow();
            (
                s.selected_model.clone(),
                s.kv_type.clone(),
                s.flash_attn,
                s.mlock,
                s.mmap,
                s.prompt_cache,
                s.reasoning_effort.clone(),
                s.reasoning_preserve,
            )
        };

        let selected_meta = selected_model
            .as_deref()
            .and_then(|name| available_models.iter().find(|m| m.file_name == name))
            .cloned();

        v_flex()
            .gap_4()
            .child(self.render_model_target_selector(
                &available_models,
                selected_model.as_deref(),
                has_override,
                &default_settings,
                state_entity,
            ))
            .children(selected_meta.as_ref().map(|meta| Self::render_detected_info(meta)))
            .child(self.render_reasoning_section(
                &reasoning_effort,
                reasoning_preserve,
                selected_meta.as_ref().map(|m| m.reasoning_mode),
                state_entity,
            ))
            .child(self.render_sampling_section())
            .child(self.render_launch_section(
                &kv_type,
                flash_attn,
                mlock,
                mmap,
                prompt_cache,
                state_entity,
            ))
            .into_any_element()
    }

    fn render_model_target_selector(
        &self,
        available_models: &[crate::gguf::GgufMeta],
        selected_model: Option<&str>,
        has_override: bool,
        default_settings: &crate::config::ModelSettings,
        state_entity: &Entity<AppState>,
    ) -> AnyElement {
        v_flex()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(COLOR_GRAPHITE_ELEVATED)
            .border_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(rems(0.9))
                            .font_weight(FontWeight::BOLD)
                            .text_color(COLOR_TEXT_PRIMARY)
                            .child("Автообнаруженные локальные модели"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .children(if has_override {
                                let modal = self.clone();
                                let state_entity = state_entity.clone();
                                let defaults_copy = default_settings.clone();
                                Some(
                                    Button::new("reset-model-override-btn")
                                        .secondary()
                                        .with_size(Size::Small)
                                        .label("Сбросить к общим")
                                        .on_click(move |_, window, cx| {
                                            let sel = modal.shared.borrow().selected_model.clone();
                                            if let Some(name) = sel {
                                                state_entity.update(cx, |s, cx| {
                                                    let mut next = s.cfg.clone();
                                                    next.models.remove(&name);
                                                    let restarted = s.apply_updated_config(next);
                                                    let _ = s.save_config();
                                                    modal.shared.borrow_mut().save_notice = Some(
                                                        if restarted {
                                                            "Индивидуальные настройки сброшены, сервер перезапускается"
                                                        } else {
                                                            "Индивидуальные настройки сброшены к общим"
                                                        }
                                                        .to_string(),
                                                    );
                                                    cx.notify();
                                                });
                                                modal.load_model_settings_into_inputs(
                                                    &defaults_copy,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        }),
                                )
                            } else {
                                None
                            })
                            .child(
                                Button::new("rescan-models-settings-btn")
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
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(rems(0.75))
                    .text_color(COLOR_TEXT_MUTED)
                    .child("Выберите общие настройки по умолчанию или конкретную модель для индивидуальной настройки"),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .flex_wrap()
                    .child({
                        let is_def = selected_model.is_none();
                        let modal = self.clone();
                        let state_entity = state_entity.clone();
                        Button::new("target-model-defaults")
                            .with_variant(if is_def {
                                gpui_component::button::ButtonVariant::Primary
                            } else {
                                gpui_component::button::ButtonVariant::Secondary
                            })
                            .with_size(Size::Small)
                            .label("По умолчанию для всех")
                            .on_click(move |_, window, cx| {
                                modal.shared.borrow_mut().selected_model = None;
                                let ms = state_entity.read(cx).cfg.model_defaults.clone();
                                modal.load_model_settings_into_inputs(&ms, window, cx);
                                state_entity.update(cx, |_, cx| cx.notify());
                            })
                    })
                    .children(available_models.iter().map(|m| {
                        let fname = m.file_name.clone();
                        let is_sel = selected_model == Some(fname.as_str());
                        let label = fname.trim_end_matches(".gguf").to_string();
                        let modal = self.clone();
                        let state_entity = state_entity.clone();
                        Button::new(format!("target-model-{}", fname))
                            .with_variant(if is_sel {
                                gpui_component::button::ButtonVariant::Primary
                            } else {
                                gpui_component::button::ButtonVariant::Secondary
                            })
                            .with_size(Size::Small)
                            .label(label)
                            .on_click(move |_, window, cx| {
                                modal.shared.borrow_mut().selected_model = Some(fname.clone());
                                let ms = state_entity.read(cx).cfg.model_settings(&fname);
                                modal.load_model_settings_into_inputs(&ms, window, cx);
                                state_entity.update(cx, |_, cx| cx.notify());
                            })
                    })),
            )
            .into_any_element()
    }

    fn render_detected_info(meta: &crate::gguf::GgufMeta) -> AnyElement {
        let size_gib = meta.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let ctx_k = meta.context_length / 1024;
        let has_mtp =
            meta.has_mtp() || crate::server::args::find_matching_mtp_layer(&meta.path).is_some();
        let has_mmproj = crate::server::args::find_matching_mmproj(&meta.path).is_some();
        let auto_ncmoe = crate::server::args::detect_optimal_ncmoe_from_name(&meta.file_name);

        let (reasoning_text, reasoning_accent) = match meta.reasoning_mode {
            ReasoningMode::Toggleable => (
                "Рассуждения: поддерживается переключение и выбор уровня",
                true,
            ),
            ReasoningMode::ForcedInTemplate => (
                "Рассуждения: зашиты в шаблон GGUF без переключателя enable_thinking",
                true,
            ),
            ReasoningMode::None => ("Рассуждения: маркеры в шаблоне GGUF не обнаружены", false),
        };

        v_flex()
            .gap_1p5()
            .p_3()
            .rounded_lg()
            .bg(COLOR_PINK_MUTED_BG)
            .border_1()
            .border_color(COLOR_PINK_PRIMARY.opacity(0.4))
            .child(
                div()
                    .text_size(rems(0.85))
                    .font_weight(FontWeight::BOLD)
                    .text_color(COLOR_TEXT_PRIMARY)
                    .child(format!(
                        "Обнаружено в файле: {} · {} · {:.2} ГБ · контекст модели {}k",
                        meta.architecture, meta.quant_label, size_gib, ctx_k
                    )),
            )
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .text_size(rems(0.78))
                    .child(
                        div()
                            .text_color(if reasoning_accent {
                                COLOR_PINK_PRIMARY
                            } else {
                                COLOR_TEXT_MUTED
                            })
                            .child(reasoning_text),
                    )
                    .child(
                        div()
                            .text_color(if has_mtp {
                                COLOR_PINK_PRIMARY
                            } else {
                                COLOR_TEXT_MUTED
                            })
                            .child(if has_mtp {
                                "MTP: обнаружен"
                            } else {
                                "MTP: не обнаружен"
                            }),
                    )
                    .child(
                        div()
                            .text_color(if has_mmproj {
                                COLOR_PINK_PRIMARY
                            } else {
                                COLOR_TEXT_MUTED
                            })
                            .child(if has_mmproj {
                                "Зрение mmproj: обнаружен"
                            } else {
                                "Зрение mmproj: не обнаружен"
                            }),
                    )
                    .children(auto_ncmoe.map(|nc| {
                        div()
                            .text_color(COLOR_PINK_PRIMARY)
                            .child(format!("Авто MoE: -ncmoe {nc}"))
                    })),
            )
            .into_any_element()
    }

    fn render_reasoning_section(
        &self,
        reasoning_effort: &str,
        reasoning_preserve: bool,
        detected_mode: Option<ReasoningMode>,
        state_entity: &Entity<AppState>,
    ) -> AnyElement {
        let levels = [
            ("off", "Выкл"),
            ("low", "Низкий"),
            ("medium", "Средний"),
            ("high", "Высокий"),
            ("xhigh", "Максимум"),
        ];

        let hint = match detected_mode {
            Some(ReasoningMode::ForcedInTemplate) if reasoning_effort == "off" => {
                "Внимание: в GGUF шаблоне этой модели тег рассуждений прописан без условия enable_thinking. Если модель продолжит думать после отключения, потребуется вернуть стандартный шаблон GGUF"
            }
            Some(ReasoningMode::Toggleable) => {
                "Доступные режимы: отключение и выбор глубины рассуждений через флаги сервера и параметры шаблона"
            }
            _ => "Управление глубиной рассуждений для поддерживающих моделей",
        };

        v_flex()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(COLOR_GRAPHITE_ELEVATED)
            .border_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .child(
                div()
                    .text_size(rems(0.9))
                    .font_weight(FontWeight::BOLD)
                    .text_color(COLOR_TEXT_PRIMARY)
                    .child("Рассуждения"),
            )
            .child(
                div()
                    .text_size(rems(0.75))
                    .text_color(COLOR_TEXT_MUTED)
                    .child(hint),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .flex_wrap()
                    .children(levels.iter().map(|(val, label)| {
                        let is_sel = reasoning_effort == *val;
                        let val_str = (*val).to_string();
                        let shared = self.shared.clone();
                        let state_entity = state_entity.clone();
                        Button::new(format!("reasoning-opt-{}", val))
                            .with_variant(if is_sel {
                                gpui_component::button::ButtonVariant::Primary
                            } else {
                                gpui_component::button::ButtonVariant::Secondary
                            })
                            .with_size(Size::Small)
                            .label(*label)
                            .on_click(move |_, _, cx| {
                                shared.borrow_mut().reasoning_effort = val_str.clone();
                                state_entity.update(cx, |_, cx| cx.notify());
                            })
                    })),
            )
            .child(self.render_model_bool_toggle(
                "toggle-reasoning-preserve",
                "Сохранять рассуждения в контексте",
                "Передавать прошлые размышления модели обратно в историю сообщений",
                reasoning_preserve,
                state_entity,
                |s| &mut s.reasoning_preserve,
            ))
            .into_any_element()
    }

    fn render_sampling_section(&self) -> AnyElement {
        v_flex()
            .gap_2p5()
            .p_3()
            .rounded_lg()
            .bg(COLOR_GRAPHITE_ELEVATED)
            .border_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .child(
                div()
                    .text_size(rems(0.9))
                    .font_weight(FontWeight::BOLD)
                    .text_color(COLOR_TEXT_PRIMARY)
                    .child("Параметры генерации"),
            )
            .child(
                div()
                    .text_size(rems(0.75))
                    .text_color(COLOR_TEXT_MUTED)
                    .child("Применяются сразу со следующего сообщения без перезапуска сервера"),
            )
            .child(Self::render_field(
                "Температура",
                "Случайность генерации, например 1.0",
                &self.temperature_input,
            ))
            .child(Self::render_field(
                "Top K",
                "Ограничение выборки по K наиболее вероятным токенам",
                &self.top_k_input,
            ))
            .child(Self::render_field(
                "Top P",
                "Ядерная выборка по суммарной вероятности, например 0.95",
                &self.top_p_input,
            ))
            .child(Self::render_field(
                "Min P",
                "Нижний порог вероятности токена, например 0.0",
                &self.min_p_input,
            ))
            .child(Self::render_field(
                "Штраф за повторы",
                "Для моделей с MTP рекомендуется 1.0",
                &self.repeat_penalty_input,
            ))
            .child(Self::render_field(
                "Максимум токенов ответа",
                "Ограничение длины одного ответа модели",
                &self.max_tokens_input,
            ))
            .into_any_element()
    }

    fn render_launch_section(
        &self,
        kv_type: &str,
        flash_attn: bool,
        mlock: bool,
        mmap: bool,
        prompt_cache: bool,
        state_entity: &Entity<AppState>,
    ) -> AnyElement {
        let kv_types = ["f16", "q8_0", "q4_0"];

        v_flex()
            .gap_2p5()
            .p_3()
            .rounded_lg()
            .bg(COLOR_GRAPHITE_ELEVATED)
            .border_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .child(
                div()
                    .text_size(rems(0.9))
                    .font_weight(FontWeight::BOLD)
                    .text_color(COLOR_TEXT_PRIMARY)
                    .child("Параметры запуска движка"),
            )
            .child(
                div()
                    .text_size(rems(0.75))
                    .text_color(COLOR_TEXT_MUTED)
                    .child("При изменении этих параметров запущенный сервер перезапускается автоматически"),
            )
            .child(Self::render_field(
                "Размер контекста",
                "Окно контекста модели в токенах, например 32000 или 65536",
                &self.ctx_input,
            ))
            .child(Self::render_field(
                "Слои на GPU",
                "Количество слоев модели для выгрузки на видеокарту",
                &self.ngl_input,
            ))
            .child(Self::render_field(
                "Размер пакета",
                "Логический размер батча обработки промпта, например 2048",
                &self.batch_input,
            ))
            .child(Self::render_field(
                "Физический микробатч",
                "Размер микробатча вычислений на GPU, например 512",
                &self.ubatch_input,
            ))
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(rems(0.85))
                            .font_weight(FontWeight::BOLD)
                            .text_color(COLOR_TEXT_PRIMARY)
                            .child("Квантование KV-кэша"),
                    )
                    .child(
                        h_flex().gap_1p5().children(kv_types.iter().map(|kv| {
                            let is_sel = kv_type == *kv;
                            let kv_str = (*kv).to_string();
                            let shared = self.shared.clone();
                            let state_entity = state_entity.clone();
                            Button::new(format!("kv-type-{}", kv))
                                .with_variant(if is_sel {
                                    gpui_component::button::ButtonVariant::Primary
                                } else {
                                    gpui_component::button::ButtonVariant::Secondary
                                })
                                .with_size(Size::Small)
                                .label(*kv)
                                .on_click(move |_, _, cx| {
                                    shared.borrow_mut().kv_type = kv_str.clone();
                                    state_entity.update(cx, |_, cx| cx.notify());
                                })
                        })),
                    ),
            )
            .child(self.render_model_bool_toggle(
                "toggle-flash-attn",
                "Flash Attention",
                "Ускорение и экономия памяти внимания на поддерживаемых GPU",
                flash_attn,
                state_entity,
                |s| &mut s.flash_attn,
            ))
            .child(self.render_model_bool_toggle(
                "toggle-mlock",
                "Удержание в памяти mlock",
                "Блокировать веса модели в оперативной памяти без выгрузки в файл подкачки",
                mlock,
                state_entity,
                |s| &mut s.mlock,
            ))
            .child(self.render_model_bool_toggle(
                "toggle-mmap",
                "Отображение файла mmap",
                "Быстрая загрузка весов через отображение файла в память",
                mmap,
                state_entity,
                |s| &mut s.mmap,
            ))
            .child(self.render_model_bool_toggle(
                "toggle-prompt-cache",
                "Кэширование промпта на диск",
                "Сохранять состояние KV-кэша системного промпта между запусками",
                prompt_cache,
                state_entity,
                |s| &mut s.prompt_cache,
            ))
            .child(Self::render_field(
                "Проектор зрения mmproj",
                "Имя файла mmproj-*.gguf, auto для автопоиска или пусто для отключения",
                &self.mmproj_input,
            ))
            .child(Self::render_field(
                "Спекулятивный драфт MTP",
                "Файл mtp-*.gguf, auto для автопоиска или none для выключения",
                &self.mtp_input,
            ))
            .child(Self::render_field(
                "Дополнительные аргументы сервера",
                "Произвольные флаги запуска, например -ncmoe 7",
                &self.extra_args_input,
            ))
            .into_any_element()
    }

    fn render_model_bool_toggle(
        &self,
        id: &'static str,
        title: &'static str,
        hint: &'static str,
        enabled: bool,
        state_entity: &Entity<AppState>,
        field: fn(&mut SettingsState) -> &mut bool,
    ) -> AnyElement {
        let shared = self.shared.clone();
        let state_entity = state_entity.clone();
        Self::render_bool_row(id, title, hint, enabled, move |cx| {
            let mut s = shared.borrow_mut();
            let flag = field(&mut s);
            *flag = !*flag;
            drop(s);
            state_entity.update(cx, |_, cx| cx.notify());
        })
    }
}
