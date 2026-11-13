use super::SettingsModal;
use crate::config::{Config, ModelSettings};
use crate::gguf::GgufMeta;
use crate::gui::state::{AppState, ServerStatus};
use gpui::*;
use gpui_component::input::InputState;

impl SettingsModal {
    pub fn sync_from_state(
        &self,
        cfg: &Config,
        available_models: &[GgufMeta],
        window: &mut Window,
        cx: &mut App,
    ) {
        let selected_model = {
            let mut s = self.shared.borrow_mut();
            s.default_persona = cfg.default_persona.clone();
            s.auto_scroll = cfg.ui.auto_scroll;
            s.stream_reasoning = cfg.ui.stream_reasoning;
            s.search_provider = cfg.search.provider.clone();
            s.save_notice = None;
            if let Some(ref sel) = s.selected_model {
                if !available_models.iter().any(|m| &m.file_name == sel) {
                    s.selected_model = cfg.active_model.clone();
                }
            } else if cfg.active_model.is_some() {
                s.selected_model = cfg.active_model.clone();
            }
            s.selected_model.clone()
        };

        let ms = selected_model
            .as_deref()
            .map(|m| cfg.model_settings(m))
            .unwrap_or_else(|| cfg.model_defaults.clone());
        self.load_model_settings_into_inputs(&ms, window, cx);

        let set_inp = |inp: &Entity<InputState>, val: String, window: &mut Window, cx: &mut App| {
            inp.update(cx, |s, cx| s.set_value(val, window, cx));
        };

        set_inp(
            &self.memory_token_limit_input,
            cfg.limits.memory_token_limit.to_string(),
            window,
            cx,
        );
        set_inp(
            &self.compact_threshold_pct_input,
            cfg.limits.compact_threshold_pct.to_string(),
            window,
            cx,
        );
        set_inp(
            &self.max_tool_steps_input,
            cfg.limits.max_tool_steps.to_string(),
            window,
            cx,
        );
        set_inp(
            &self.max_read_file_bytes_input,
            cfg.limits.max_read_file_bytes.to_string(),
            window,
            cx,
        );
        set_inp(&self.host_input, cfg.server.host.clone(), window, cx);
        set_inp(&self.port_input, cfg.server.port.to_string(), window, cx);
        set_inp(
            &self.health_timeout_input,
            cfg.server.health_timeout_secs.to_string(),
            window,
            cx,
        );
        set_inp(
            &self.tavily_api_key_input,
            cfg.search.tavily_api_key.clone(),
            window,
            cx,
        );
        set_inp(
            &self.parallel_api_key_input,
            cfg.search.parallel_api_key.clone(),
            window,
            cx,
        );
        set_inp(
            &self.parallel_mode_input,
            cfg.search.parallel_mode.clone(),
            window,
            cx,
        );
        set_inp(
            &self.exa_api_key_input,
            cfg.search.exa_api_key.clone(),
            window,
            cx,
        );
        set_inp(&self.exa_mode_input, cfg.search.exa_mode.clone(), window, cx);
        set_inp(
            &self.firecrawl_api_key_input,
            cfg.firecrawl.api_key.clone(),
            window,
            cx,
        );
        set_inp(
            &self.firecrawl_api_url_input,
            cfg.firecrawl.api_url.clone(),
            window,
            cx,
        );
        set_inp(
            &self.firecrawl_mode_input,
            cfg.firecrawl.mode.clone(),
            window,
            cx,
        );
        set_inp(
            &self.firecrawl_credit_limit_input,
            cfg.firecrawl.monthly_credit_limit.to_string(),
            window,
            cx,
        );
        set_inp(
            &self.max_ctx_ceiling_input,
            cfg.vram.max_ctx_ceiling.to_string(),
            window,
            cx,
        );
        set_inp(
            &self.reserve_mib_input,
            cfg.vram.reserve_mib.to_string(),
            window,
            cx,
        );
        set_inp(
            &self.shared_spill_threshold_mib_input,
            cfg.vram.shared_spill_threshold_mib.to_string(),
            window,
            cx,
        );
    }

    pub fn load_model_settings_into_inputs(
        &self,
        ms: &ModelSettings,
        window: &mut Window,
        cx: &mut App,
    ) {
        {
            let mut s = self.shared.borrow_mut();
            s.kv_type = ms.kv_type.clone();
            s.flash_attn = ms.flash_attn;
            s.mlock = ms.mlock;
            s.mmap = ms.mmap;
            s.prompt_cache = ms.prompt_cache;
            s.reasoning_effort = ms.reasoning_effort.clone();
            s.reasoning_preserve = ms.reasoning_preserve;
        }

        let set_inp = |inp: &Entity<InputState>, val: String, window: &mut Window, cx: &mut App| {
            inp.update(cx, |s, cx| s.set_value(val, window, cx));
        };

        set_inp(&self.temperature_input, ms.temperature.to_string(), window, cx);
        set_inp(&self.top_k_input, ms.top_k.to_string(), window, cx);
        set_inp(&self.top_p_input, ms.top_p.to_string(), window, cx);
        set_inp(&self.min_p_input, ms.min_p.to_string(), window, cx);
        set_inp(
            &self.repeat_penalty_input,
            ms.repeat_penalty.to_string(),
            window,
            cx,
        );
        set_inp(&self.max_tokens_input, ms.max_tokens.to_string(), window, cx);
        set_inp(&self.ctx_input, ms.ctx.to_string(), window, cx);
        set_inp(&self.ngl_input, ms.ngl.to_string(), window, cx);
        set_inp(&self.batch_input, ms.batch.to_string(), window, cx);
        set_inp(&self.ubatch_input, ms.ubatch.to_string(), window, cx);
        set_inp(
            &self.mmproj_input,
            ms.mmproj.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_inp(
            &self.mtp_input,
            ms.mtp.clone().unwrap_or_else(|| "auto".to_string()),
            window,
            cx,
        );
        set_inp(&self.extra_args_input, ms.extra_args.clone(), window, cx);
    }

    pub fn save_to_state(&self, state: &mut AppState, cx: &App) {
        let (
            def_persona,
            auto_s,
            stream_r,
            selected_model,
            kv_type,
            flash_attn,
            mlock,
            mmap,
            prompt_cache,
            reasoning_effort,
            reasoning_preserve,
            search_provider,
        ) = {
            let s = self.shared.borrow();
            (
                s.default_persona.clone(),
                s.auto_scroll,
                s.stream_reasoning,
                s.selected_model.clone(),
                s.kv_type.clone(),
                s.flash_attn,
                s.mlock,
                s.mmap,
                s.prompt_cache,
                s.reasoning_effort.clone(),
                s.reasoning_preserve,
                s.search_provider.clone(),
            )
        };

        let mut next_cfg = state.cfg.clone();
        next_cfg.default_persona = def_persona;
        next_cfg.ui.auto_scroll = auto_s;
        next_cfg.ui.stream_reasoning = stream_r;

        let mut bad_fields: Vec<&'static str> = Vec::new();

        fn parse_int<T: std::str::FromStr>(
            inp: &Entity<InputState>,
            cx: &App,
            label: &'static str,
            dst: &mut T,
            bad: &mut Vec<&'static str>,
        ) {
            match inp.read(cx).value().trim().parse::<T>() {
                Ok(v) => *dst = v,
                Err(_) => bad.push(label),
            }
        }

        fn parse_f32(
            inp: &Entity<InputState>,
            cx: &App,
            label: &'static str,
            dst: &mut f32,
            bad: &mut Vec<&'static str>,
        ) {
            match inp
                .read(cx)
                .value()
                .trim()
                .replace(',', ".")
                .parse::<f32>()
            {
                Ok(v) if v.is_finite() => *dst = v,
                _ => bad.push(label),
            }
        }

        let read_str = |inp: &Entity<InputState>| inp.read(cx).value().trim().to_string();

        parse_int(
            &self.memory_token_limit_input,
            cx,
            "Потолок токенов памяти",
            &mut next_cfg.limits.memory_token_limit,
            &mut bad_fields,
        );
        parse_int(
            &self.compact_threshold_pct_input,
            cx,
            "Порог сжатия контекста",
            &mut next_cfg.limits.compact_threshold_pct,
            &mut bad_fields,
        );
        parse_int(
            &self.max_tool_steps_input,
            cx,
            "Максимум шагов инструментов",
            &mut next_cfg.limits.max_tool_steps,
            &mut bad_fields,
        );
        parse_int(
            &self.max_read_file_bytes_input,
            cx,
            "Лимит чтения файла",
            &mut next_cfg.limits.max_read_file_bytes,
            &mut bad_fields,
        );

        let mut ms = selected_model
            .as_deref()
            .map(|m| next_cfg.model_settings(m))
            .unwrap_or_else(|| next_cfg.model_defaults.clone());

        parse_f32(&self.temperature_input, cx, "Температура", &mut ms.temperature, &mut bad_fields);
        parse_int(&self.top_k_input, cx, "Top K", &mut ms.top_k, &mut bad_fields);
        parse_f32(&self.top_p_input, cx, "Top P", &mut ms.top_p, &mut bad_fields);
        parse_f32(&self.min_p_input, cx, "Min P", &mut ms.min_p, &mut bad_fields);
        parse_f32(
            &self.repeat_penalty_input,
            cx,
            "Штраф за повторы",
            &mut ms.repeat_penalty,
            &mut bad_fields,
        );
        parse_int(
            &self.max_tokens_input,
            cx,
            "Максимум токенов ответа",
            &mut ms.max_tokens,
            &mut bad_fields,
        );
        parse_int(&self.ctx_input, cx, "Размер контекста", &mut ms.ctx, &mut bad_fields);
        parse_int(&self.ngl_input, cx, "Слои на GPU", &mut ms.ngl, &mut bad_fields);
        parse_int(&self.batch_input, cx, "Размер пакета", &mut ms.batch, &mut bad_fields);
        parse_int(
            &self.ubatch_input,
            cx,
            "Физический микробатч",
            &mut ms.ubatch,
            &mut bad_fields,
        );
        ms.kv_type = kv_type;
        ms.flash_attn = flash_attn;
        ms.mlock = mlock;
        ms.mmap = mmap;
        ms.prompt_cache = prompt_cache;
        ms.reasoning_effort = reasoning_effort;
        ms.reasoning_preserve = reasoning_preserve;

        let mmproj_val = read_str(&self.mmproj_input);
        ms.mmproj = if mmproj_val.is_empty() || mmproj_val == "none" {
            None
        } else {
            Some(mmproj_val)
        };
        let mtp_val = read_str(&self.mtp_input);
        ms.mtp = if mtp_val.is_empty() {
            None
        } else {
            Some(mtp_val)
        };
        ms.extra_args = read_str(&self.extra_args_input);

        if let Some(model_name) = selected_model {
            next_cfg.models.insert(model_name, ms);
        } else {
            next_cfg.server.default_ngl = ms.ngl;
            next_cfg.model_defaults = ms;
        }

        let host = read_str(&self.host_input);
        if host.is_empty() {
            bad_fields.push("Хост llama-server");
        } else {
            next_cfg.server.host = host;
        }
        parse_int(&self.port_input, cx, "Порт", &mut next_cfg.server.port, &mut bad_fields);
        parse_int(
            &self.health_timeout_input,
            cx,
            "Таймаут проверки здоровья",
            &mut next_cfg.server.health_timeout_secs,
            &mut bad_fields,
        );

        next_cfg.search.provider = search_provider;
        next_cfg.search.tavily_api_key = read_str(&self.tavily_api_key_input);
        next_cfg.search.parallel_api_key = read_str(&self.parallel_api_key_input);
        let p_mode = read_str(&self.parallel_mode_input);
        if !p_mode.is_empty() {
            next_cfg.search.parallel_mode = p_mode;
        }
        next_cfg.search.exa_api_key = read_str(&self.exa_api_key_input);
        let e_mode = read_str(&self.exa_mode_input);
        if !e_mode.is_empty() {
            next_cfg.search.exa_mode = e_mode;
        }

        next_cfg.firecrawl.api_key = read_str(&self.firecrawl_api_key_input);
        let fc_url = read_str(&self.firecrawl_api_url_input);
        if !fc_url.is_empty() {
            next_cfg.firecrawl.api_url = fc_url;
        }
        let fc_mode = read_str(&self.firecrawl_mode_input);
        if !fc_mode.is_empty() {
            next_cfg.firecrawl.mode = fc_mode;
        }
        parse_int(
            &self.firecrawl_credit_limit_input,
            cx,
            "Лимит кредитов Firecrawl",
            &mut next_cfg.firecrawl.monthly_credit_limit,
            &mut bad_fields,
        );

        parse_int(
            &self.max_ctx_ceiling_input,
            cx,
            "Потолок контекста VRAM",
            &mut next_cfg.vram.max_ctx_ceiling,
            &mut bad_fields,
        );
        parse_int(
            &self.reserve_mib_input,
            cx,
            "Резерв VRAM",
            &mut next_cfg.vram.reserve_mib,
            &mut bad_fields,
        );
        parse_int(
            &self.shared_spill_threshold_mib_input,
            cx,
            "Порог переполнения в RAM",
            &mut next_cfg.vram.shared_spill_threshold_mib,
            &mut bad_fields,
        );

        if !bad_fields.is_empty() {
            self.shared.borrow_mut().save_notice = Some(format!(
                "Некорректные значения в полях: {}",
                bad_fields.join(", ")
            ));
            return;
        }

        if let Err(e) = next_cfg.validate() {
            self.shared.borrow_mut().save_notice = Some(format!("Ошибка проверки настроек: {e}"));
            return;
        }

        let restarted = state.apply_updated_config(next_cfg);
        if let Err(e) = state.save_config() {
            self.shared.borrow_mut().save_notice = Some(format!("Ошибка сохранения: {e}"));
            return;
        }

        let is_running = matches!(
            state.server_status,
            ServerStatus::Running { .. } | ServerStatus::Starting { .. }
        );
        let notice = if restarted {
            "Настройки сохранены, сервер перезапускается для применения параметров"
        } else if is_running {
            "Настройки сохранены и применены"
        } else {
            "Настройки сохранены в snarta.toml"
        };
        self.shared.borrow_mut().save_notice = Some(notice.to_string());
    }
}
