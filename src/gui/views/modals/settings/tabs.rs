use super::SettingsModal;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_ELEVATED, COLOR_GRAPHITE_INPUT, COLOR_TEXT_MUTED,
    COLOR_TEXT_PRIMARY,
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputState};
use gpui_component::{Sizable, Size, h_flex, v_flex};

impl SettingsModal {
    pub(super) fn render_general_tab(
        &mut self,
        personas: &[crate::persona::Persona],
        current_persona: &str,
        auto_scroll: bool,
        stream_reasoning: bool,
        state_entity: &Entity<AppState>,
    ) -> AnyElement {
        v_flex()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(rems(0.85))
                            .font_weight(FontWeight::BOLD)
                            .text_color(COLOR_TEXT_PRIMARY)
                            .child("Персона по умолчанию:"),
                    )
                    .child(
                        div()
                            .text_size(rems(0.75))
                            .text_color(COLOR_TEXT_MUTED)
                            .child("Активная персона, загружаемая при старте новой сессии"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .children(personas.iter().map(|p| {
                                let is_sel = p.name == current_persona;
                                let pname = p.name.clone();
                                let shared = self.shared.clone();
                                let state_entity = state_entity.clone();
                                Button::new(format!("persona-opt-{}", p.name))
                                    .with_variant(if is_sel {
                                        gpui_component::button::ButtonVariant::Primary
                                    } else {
                                        gpui_component::button::ButtonVariant::Secondary
                                    })
                                    .with_size(Size::Small)
                                    .label(p.name.clone())
                                    .on_click(move |_, _, cx| {
                                        shared.borrow_mut().default_persona = pname.clone();
                                        state_entity.update(cx, |_, cx| cx.notify());
                                    })
                            })),
                    ),
            )
            .child(Self::render_bool_row(
                "toggle-autoscroll",
                "Автопрокрутка чата",
                "Автоматически следовать за новыми токенами стриминга",
                auto_scroll,
                {
                    let shared = self.shared.clone();
                    let state_entity = state_entity.clone();
                    move |cx| {
                        let mut s = shared.borrow_mut();
                        s.auto_scroll = !s.auto_scroll;
                        drop(s);
                        state_entity.update(cx, |_, cx| cx.notify());
                    }
                },
            ))
            .child(Self::render_bool_row(
                "toggle-reasoning",
                "Потоковый вывод рассуждений",
                "Отображать цепочку размышлений в реальном времени",
                stream_reasoning,
                {
                    let shared = self.shared.clone();
                    let state_entity = state_entity.clone();
                    move |cx| {
                        let mut s = shared.borrow_mut();
                        s.stream_reasoning = !s.stream_reasoning;
                        drop(s);
                        state_entity.update(cx, |_, cx| cx.notify());
                    }
                },
            ))
            .child(Self::render_field(
                "Потолок токенов долговременной памяти:",
                "Максимальное число токенов для файла life/memory.md в промпте",
                &self.memory_token_limit_input,
            ))
            .child(Self::render_field(
                "Порог сжатия контекста в процентах:",
                "Процент заполнения окна контекста для предупреждения или сжатия, от 1 до 100",
                &self.compact_threshold_pct_input,
            ))
            .child(Self::render_field(
                "Максимум шагов инструментов за ход:",
                "Ограничение числа последовательных вызовов инструментов агентом",
                &self.max_tool_steps_input,
            ))
            .child(Self::render_field(
                "Лимит чтения файла в байтах:",
                "Максимальный размер файла для инструмента чтения",
                &self.max_read_file_bytes_input,
            ))
            .into_any_element()
    }

    pub(super) fn render_server_tab(&mut self) -> AnyElement {
        v_flex()
            .gap_3()
            .child(Self::render_field(
                "Хост llama-server",
                "IP-адрес локального бэкенда",
                &self.host_input,
            ))
            .child(Self::render_field(
                "Порт",
                "TCP порт сервера 8089",
                &self.port_input,
            ))
            .child(Self::render_field(
                "Таймаут проверки здоровья",
                "Максимальное время ожидания ответа /health в секундах",
                &self.health_timeout_input,
            ))
            .into_any_element()
    }

    pub(super) fn render_search_tab(
        &mut self,
        search_provider: &str,
        state_entity: &Entity<AppState>,
    ) -> AnyElement {
        let providers = [
            ("cascade", "Каскад"),
            ("tavily", "Tavily"),
            ("parallel", "Parallel"),
            ("exa", "Exa"),
        ];

        v_flex()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(rems(0.85))
                            .font_weight(FontWeight::BOLD)
                            .text_color(COLOR_TEXT_PRIMARY)
                            .child("Провайдер веб-поиска"),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .flex_wrap()
                            .children(providers.iter().map(|(val, label)| {
                                let is_sel = search_provider == *val;
                                let val_str = (*val).to_string();
                                let shared = self.shared.clone();
                                let state_entity = state_entity.clone();
                                Button::new(format!("search-prov-{}", val))
                                    .with_variant(if is_sel {
                                        gpui_component::button::ButtonVariant::Primary
                                    } else {
                                        gpui_component::button::ButtonVariant::Secondary
                                    })
                                    .with_size(Size::Small)
                                    .label(*label)
                                    .on_click(move |_, _, cx| {
                                        shared.borrow_mut().search_provider = val_str.clone();
                                        state_entity.update(cx, |_, cx| cx.notify());
                                    })
                            })),
                    ),
            )
            .child(Self::render_field(
                "Ключ API Tavily",
                "Ключ API для поисковика Tavily",
                &self.tavily_api_key_input,
            ))
            .child(Self::render_field(
                "Ключ API Parallel",
                "Ключ API для поисковика Parallel",
                &self.parallel_api_key_input,
            ))
            .child(Self::render_field(
                "Режим Parallel",
                "Режим поиска Parallel, например fast",
                &self.parallel_mode_input,
            ))
            .child(Self::render_field(
                "Ключ API Exa",
                "Ключ API для поисковика Exa",
                &self.exa_api_key_input,
            ))
            .child(Self::render_field(
                "Режим Exa",
                "Режим поиска Exa, например auto",
                &self.exa_mode_input,
            ))
            .child(Self::render_field(
                "Ключ API Firecrawl",
                "Ключ API для скрапинга и базы знаний Alexandria",
                &self.firecrawl_api_key_input,
            ))
            .child(Self::render_field(
                "Адрес Firecrawl API",
                "Эндпоинт Firecrawl API",
                &self.firecrawl_api_url_input,
            ))
            .child(Self::render_field(
                "Режим Firecrawl",
                "Режим скрапинга Firecrawl, например auto",
                &self.firecrawl_mode_input,
            ))
            .child(Self::render_field(
                "Месячный лимит кредитов Firecrawl",
                "Максимальный расход кредитов скрапинга в месяц",
                &self.firecrawl_credit_limit_input,
            ))
            .into_any_element()
    }

    pub(super) fn render_vram_tab(&mut self) -> AnyElement {
        v_flex()
            .gap_3()
            .child(Self::render_field(
                "Потолок контекста VRAM",
                "Верхний предел токенов для расчета VRAM",
                &self.max_ctx_ceiling_input,
            ))
            .child(Self::render_field(
                "Резерв VRAM для ОС и DWM",
                "Гарантированный запас видеопамяти под интерфейс Windows и приложения",
                &self.reserve_mib_input,
            ))
            .child(Self::render_field(
                "Порог переполнения в системную память",
                "Порог предупреждения об утечке весов в медленный системный RAM",
                &self.shared_spill_threshold_mib_input,
            ))
            .into_any_element()
    }

    pub(super) fn render_bool_row(
        id: &'static str,
        title: &'static str,
        hint: &'static str,
        enabled: bool,
        on_toggle: impl Fn(&mut App) + 'static,
    ) -> AnyElement {
        h_flex()
            .justify_between()
            .items_center()
            .p_2()
            .rounded_lg()
            .bg(COLOR_GRAPHITE_ELEVATED)
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(rems(0.85))
                            .font_weight(FontWeight::BOLD)
                            .text_color(COLOR_TEXT_PRIMARY)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(rems(0.75))
                            .text_color(COLOR_TEXT_MUTED)
                            .child(hint),
                    ),
            )
            .child(
                Button::new(id)
                    .with_variant(if enabled {
                        gpui_component::button::ButtonVariant::Primary
                    } else {
                        gpui_component::button::ButtonVariant::Secondary
                    })
                    .with_size(Size::Small)
                    .label(if enabled { "Вкл" } else { "Выкл" })
                    .on_click(move |_, _, cx| {
                        on_toggle(cx);
                    }),
            )
            .into_any_element()
    }

    pub(super) fn render_field(
        title: &'static str,
        hint: &'static str,
        input: &Entity<InputState>,
    ) -> AnyElement {
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_size(rems(0.85))
                    .font_weight(FontWeight::BOLD)
                    .text_color(COLOR_TEXT_PRIMARY)
                    .child(title),
            )
            .child(
                div()
                    .text_size(rems(0.75))
                    .text_color(COLOR_TEXT_MUTED)
                    .child(hint),
            )
            .child(
                div()
                    .rounded_md()
                    .bg(COLOR_GRAPHITE_INPUT)
                    .border_1()
                    .border_color(COLOR_GRAPHITE_BORDER)
                    .child(Input::new(input)),
            )
            .into_any_element()
    }
}
