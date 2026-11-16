mod model_tab;
mod persistence;
mod tabs;
#[cfg(test)]
mod tests;

use super::{DoctorModal, RuntimeModal};
use crate::config::Config;
use crate::gui::icons::icons;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_ACCENT_ORANGE, COLOR_BG_CANVAS, COLOR_BORDER, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::InputState;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{Sizable, Size, h_flex, v_flex};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Models,
    Server,
    Runtime,
    Doctor,
    SearchAndTools,
    Vram,
}

pub struct SettingsState {
    pub current_tab: SettingsTab,
    pub default_persona: String,
    pub auto_scroll: bool,
    pub stream_reasoning: bool,
    pub save_notice: Option<String>,
    pub selected_model: Option<String>,
    pub kv_type: String,
    pub flash_attn: bool,
    pub mlock: bool,
    pub mmap: bool,
    pub prompt_cache: bool,
    pub reasoning_effort: String,
    pub reasoning_preserve: bool,
    pub search_provider: String,
}

#[derive(Clone)]
pub struct SettingsModal {
    pub shared: Rc<RefCell<SettingsState>>,

    // General / Limits
    pub memory_token_limit_input: Entity<InputState>,
    pub compact_threshold_pct_input: Entity<InputState>,
    pub max_tool_steps_input: Entity<InputState>,
    pub max_read_file_bytes_input: Entity<InputState>,

    // Model & Generation
    pub temperature_input: Entity<InputState>,
    pub top_k_input: Entity<InputState>,
    pub top_p_input: Entity<InputState>,
    pub min_p_input: Entity<InputState>,
    pub repeat_penalty_input: Entity<InputState>,
    pub max_tokens_input: Entity<InputState>,
    pub ctx_input: Entity<InputState>,
    pub ngl_input: Entity<InputState>,
    pub batch_input: Entity<InputState>,
    pub ubatch_input: Entity<InputState>,
    pub mmproj_input: Entity<InputState>,
    pub mtp_input: Entity<InputState>,
    pub extra_args_input: Entity<InputState>,

    // Server
    pub host_input: Entity<InputState>,
    pub port_input: Entity<InputState>,
    pub health_timeout_input: Entity<InputState>,

    // Search & Tools
    pub tavily_api_key_input: Entity<InputState>,
    pub parallel_api_key_input: Entity<InputState>,
    pub parallel_mode_input: Entity<InputState>,
    pub exa_api_key_input: Entity<InputState>,
    pub exa_mode_input: Entity<InputState>,
    pub firecrawl_api_key_input: Entity<InputState>,
    pub firecrawl_api_url_input: Entity<InputState>,
    pub firecrawl_mode_input: Entity<InputState>,
    pub firecrawl_credit_limit_input: Entity<InputState>,

    // VRAM
    pub max_ctx_ceiling_input: Entity<InputState>,
    pub reserve_mib_input: Entity<InputState>,
    pub shared_spill_threshold_mib_input: Entity<InputState>,
}

impl SettingsModal {
    pub fn new(cfg: &Config, window: &mut Window, cx: &mut App) -> Self {
        let initial_model = cfg.active_model.clone();
        let ms = initial_model
            .as_deref()
            .map(|m| cfg.model_settings(m))
            .unwrap_or_else(|| cfg.model_defaults.clone());

        let shared = Rc::new(RefCell::new(SettingsState {
            current_tab: SettingsTab::General,
            default_persona: cfg.default_persona.clone(),
            auto_scroll: cfg.ui.auto_scroll,
            stream_reasoning: cfg.ui.stream_reasoning,
            save_notice: None,
            selected_model: initial_model,
            kv_type: ms.kv_type.clone(),
            flash_attn: ms.flash_attn,
            mlock: ms.mlock,
            mmap: ms.mmap,
            prompt_cache: ms.prompt_cache,
            reasoning_effort: ms.reasoning_effort.clone(),
            reasoning_preserve: ms.reasoning_preserve,
            search_provider: cfg.search.provider.clone(),
        }));

        let new_inp = |val: String, window: &mut Window, cx: &mut App| {
            cx.new(|cx| InputState::new(window, cx).default_value(val))
        };

        Self {
            shared,
            memory_token_limit_input: new_inp(
                cfg.limits.memory_token_limit.to_string(),
                window,
                cx,
            ),
            compact_threshold_pct_input: new_inp(
                cfg.limits.compact_threshold_pct.to_string(),
                window,
                cx,
            ),
            max_tool_steps_input: new_inp(cfg.limits.max_tool_steps.to_string(), window, cx),
            max_read_file_bytes_input: new_inp(
                cfg.limits.max_read_file_bytes.to_string(),
                window,
                cx,
            ),
            temperature_input: new_inp(ms.temperature.to_string(), window, cx),
            top_k_input: new_inp(ms.top_k.to_string(), window, cx),
            top_p_input: new_inp(ms.top_p.to_string(), window, cx),
            min_p_input: new_inp(ms.min_p.to_string(), window, cx),
            repeat_penalty_input: new_inp(ms.repeat_penalty.to_string(), window, cx),
            max_tokens_input: new_inp(ms.max_tokens.to_string(), window, cx),
            ctx_input: new_inp(ms.ctx.to_string(), window, cx),
            ngl_input: new_inp(ms.ngl.to_string(), window, cx),
            batch_input: new_inp(ms.batch.to_string(), window, cx),
            ubatch_input: new_inp(ms.ubatch.to_string(), window, cx),
            mmproj_input: new_inp(ms.mmproj.unwrap_or_default(), window, cx),
            mtp_input: new_inp(ms.mtp.unwrap_or_else(|| "auto".to_string()), window, cx),
            extra_args_input: new_inp(ms.extra_args, window, cx),
            host_input: new_inp(cfg.server.host.clone(), window, cx),
            port_input: new_inp(cfg.server.port.to_string(), window, cx),
            health_timeout_input: new_inp(cfg.server.health_timeout_secs.to_string(), window, cx),
            tavily_api_key_input: new_inp(cfg.search.tavily_api_key.clone(), window, cx),
            parallel_api_key_input: new_inp(cfg.search.parallel_api_key.clone(), window, cx),
            parallel_mode_input: new_inp(cfg.search.parallel_mode.clone(), window, cx),
            exa_api_key_input: new_inp(cfg.search.exa_api_key.clone(), window, cx),
            exa_mode_input: new_inp(cfg.search.exa_mode.clone(), window, cx),
            firecrawl_api_key_input: new_inp(cfg.firecrawl.api_key.clone(), window, cx),
            firecrawl_api_url_input: new_inp(cfg.firecrawl.api_url.clone(), window, cx),
            firecrawl_mode_input: new_inp(cfg.firecrawl.mode.clone(), window, cx),
            firecrawl_credit_limit_input: new_inp(
                cfg.firecrawl.monthly_credit_limit.to_string(),
                window,
                cx,
            ),
            max_ctx_ceiling_input: new_inp(cfg.vram.max_ctx_ceiling.to_string(), window, cx),
            reserve_mib_input: new_inp(cfg.vram.reserve_mib.to_string(), window, cx),
            shared_spill_threshold_mib_input: new_inp(
                cfg.vram.shared_spill_threshold_mib.to_string(),
                window,
                cx,
            ),
        }
    }

    pub fn render(
        &mut self,
        state_entity: &Entity<AppState>,
        runtime_modal: &mut RuntimeModal,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let personas = state.available_personas.clone();

        let (current_tab, default_persona, auto_scroll, stream_reasoning, save_notice, search_provider) = {
            let s = self.shared.borrow();
            (
                s.current_tab,
                s.default_persona.clone(),
                s.auto_scroll,
                s.stream_reasoning,
                s.save_notice.clone(),
                s.search_provider.clone(),
            )
        };

        let tab_index = match current_tab {
            SettingsTab::General => 0,
            SettingsTab::Models => 1,
            SettingsTab::Server => 2,
            SettingsTab::Runtime => 3,
            SettingsTab::Doctor => 4,
            SettingsTab::SearchAndTools => 5,
            SettingsTab::Vram => 6,
        };
        let tabs_shared = self.shared.clone();
        let tabs_state = state_entity.clone();

        v_flex()
            .flex_1()
            .w_full()
            .h_full()
            .p_6()
            .bg(COLOR_BG_CANVAS)
            .gap_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .pb_3()
                    .border_b_1()
                    .border_color(COLOR_BORDER)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2p5()
                            .child(icons::settings().size(px(18.0)).text_color(COLOR_ACCENT_ORANGE))
                            .child(
                                div()
                                    .text_size(rems(1.2))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child("Настройки и диагностика"),
                            ),
                    )
                    .child(
                        Button::new("close-settings")
                            .outline()
                            .with_size(Size::Small)
                            .icon(icons::close())
                            .label("Вернуться в чат")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.settings_open = false;
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
            .child(
                TabBar::new("settings-tabs")
                    .segmented()
                    .with_size(Size::Small)
                    .selected_index(tab_index)
                    .children([
                        Tab::new().label("Общие"),
                        Tab::new().label("Модель"),
                        Tab::new().label("Сервер"),
                        Tab::new().label("Движок"),
                        Tab::new().label("Диагностика"),
                        Tab::new().label("Инструменты"),
                        Tab::new().label("VRAM"),
                    ])
                    .on_click(move |ix, _, cx| {
                        let next_tab = match *ix {
                            0 => SettingsTab::General,
                            1 => SettingsTab::Models,
                            2 => SettingsTab::Server,
                            3 => SettingsTab::Runtime,
                            4 => SettingsTab::Doctor,
                            5 => SettingsTab::SearchAndTools,
                            6 => SettingsTab::Vram,
                            _ => SettingsTab::General,
                        };
                        tabs_shared.borrow_mut().current_tab = next_tab;
                        tabs_state.update(cx, |_, cx| {
                            cx.notify();
                        });
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .id("settings-tab-content")
                    .overflow_y_scroll()
                    .max_w(rems(60.0))
                    .p_1()
                    .gap_4()
                    .child(match current_tab {
                        SettingsTab::General => self.render_general_tab(
                            &personas,
                            &default_persona,
                            auto_scroll,
                            stream_reasoning,
                            state_entity,
                        ),
                        SettingsTab::Models => self.render_models_tab(state_entity, cx),
                        SettingsTab::Server => self.render_server_tab(),
                        SettingsTab::Runtime => runtime_modal
                            .render(state_entity, window, cx)
                            .into_any_element(),
                        SettingsTab::Doctor => {
                            DoctorModal::render(state_entity, window, cx).into_any_element()
                        }
                        SettingsTab::SearchAndTools => {
                            self.render_search_tab(&search_provider, state_entity)
                        }
                        SettingsTab::Vram => self.render_vram_tab(),
                    }),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .pt_3()
                    .border_t_1()
                    .border_color(COLOR_BORDER)
                    .child(
                        div()
                            .text_size(rems(0.85))
                            .text_color(if save_notice.is_some() {
                                COLOR_ACCENT_ORANGE
                            } else {
                                COLOR_TEXT_MUTED
                            })
                            .child(save_notice.unwrap_or_default()),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("cancel-settings-btn")
                                    .outline()
                                    .icon(icons::close())
                                    .label("Закрыть")
                                    .on_click({
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |s, cx| {
                                                s.settings_open = false;
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("save-settings-btn")
                                    .primary()
                                    .icon(icons::check())
                                    .label("Сохранить настройки")
                                    .on_click({
                                        let modal = self.clone();
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |state, cx| {
                                                modal.save_to_state(state, cx);
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
    }
}
