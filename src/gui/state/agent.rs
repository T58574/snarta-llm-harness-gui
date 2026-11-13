use super::{AppState, ServerStatus};
use crate::chat::engine::AgentEvent;
use crate::chat::session::{ChatTurn, ToolTrace, TurnStep};
use crate::server;

impl AppState {
    pub fn cancel_generation(&mut self) {
        if let Some(ref stream) = self.active_stream {
            stream.cancel();
        }
        self.is_generating = false;

        // Persist any partial output — including tool traces and sequential steps — as an assistant
        // turn so it is not silently discarded.
        let mut steps = std::mem::take(&mut self.streaming_steps);
        steps.retain(|s| {
            s.reasoning.is_some() || !s.content.trim().is_empty() || !s.tool_calls.is_empty()
        });
        if !self.streaming_content.is_empty()
            || !self.streaming_reasoning.is_empty()
            || !self.streaming_tools.is_empty()
            || !steps.is_empty()
        {
            let turn = ChatTurn {
                role: "assistant".to_string(),
                content: std::mem::take(&mut self.streaming_content),
                reasoning: if self.streaming_reasoning.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut self.streaming_reasoning))
                },
                gen_tps: if self.vram.tps > 0.0 {
                    Some(self.vram.tps)
                } else {
                    None
                },
                duration_secs: None,
                tokens: None,
                image: None,
                images: Vec::new(),
                tool_traces: std::mem::take(&mut self.streaming_tools),
                steps,
            };
            let _ = self.workspace.session.append_turn_with_tools(turn);
        }
        self.active_stream = None;
    }

    /// Drains any pending events from the active agent stream, updating session
    /// turns, streaming buffers, and scroll flags. Returns true if state changed.
    pub fn poll_agent_events(&mut self, is_near_bottom: bool) -> bool {
        if !self.is_generating {
            return false;
        }

        // Collect available events from the channel first to release the immutable
        // borrow on `self.active_stream` before applying mutations to `self`.
        let events: Vec<AgentEvent> = match self.active_stream {
            Some(ref stream) => {
                let mut batch = Vec::new();
                while let Ok(event) = stream.rx.try_recv() {
                    batch.push(event);
                }
                batch
            }
            None => return false,
        };

        if events.is_empty() {
            return false;
        }

        let mut stream_had_delta = false;
        for event in events {
            match event {
                AgentEvent::StepStarted { step_idx } => {
                    while self.streaming_steps.len() <= step_idx {
                        self.streaming_steps.push(TurnStep::default());
                    }
                    stream_had_delta = true;
                }
                AgentEvent::ContentDelta { step_idx, delta } => {
                    while self.streaming_steps.len() <= step_idx {
                        self.streaming_steps.push(TurnStep::default());
                    }
                    self.streaming_steps[step_idx].content.push_str(&delta);
                    self.streaming_content.push_str(&delta);
                    stream_had_delta = true;
                }
                AgentEvent::ReasoningDelta { step_idx, delta } => {
                    while self.streaming_steps.len() <= step_idx {
                        self.streaming_steps.push(TurnStep::default());
                    }
                    let r = self.streaming_steps[step_idx]
                        .reasoning
                        .get_or_insert_with(String::new);
                    r.push_str(&delta);
                    self.streaming_reasoning.push_str(&delta);
                    stream_had_delta = true;
                }
                AgentEvent::ToolStarted {
                    step_idx,
                    call_idx,
                    name,
                    arguments,
                } => {
                    while self.streaming_steps.len() <= step_idx {
                        self.streaming_steps.push(TurnStep::default());
                    }
                    let step_calls = &mut self.streaming_steps[step_idx].tool_calls;
                    while step_calls.len() <= call_idx {
                        step_calls.push(ToolTrace {
                            name: String::new(),
                            arguments: String::new(),
                            output: "Running...".to_string(),
                        });
                    }
                    step_calls[call_idx].name = name;
                    step_calls[call_idx].arguments = arguments;
                    self.sync_flat_streaming_tools();
                    stream_had_delta = true;
                }
                AgentEvent::ToolFinished {
                    step_idx,
                    call_idx,
                    name,
                    arguments,
                    output,
                } => {
                    while self.streaming_steps.len() <= step_idx {
                        self.streaming_steps.push(TurnStep::default());
                    }
                    let step_calls = &mut self.streaming_steps[step_idx].tool_calls;
                    while step_calls.len() <= call_idx {
                        step_calls.push(ToolTrace {
                            name: String::new(),
                            arguments: String::new(),
                            output: String::new(),
                        });
                    }
                    step_calls[call_idx] = ToolTrace {
                        name,
                        arguments,
                        output,
                    };
                    self.sync_flat_streaming_tools();
                    stream_had_delta = true;
                }
                AgentEvent::MemoryNotice(fact) => {
                    let _ = self.workspace.session.append_mem_notice(&fact);
                }
                AgentEvent::CwdChanged(new_cwd) => {
                    self.paths.cwd = new_cwd;
                }
                AgentEvent::TurnComplete {
                    turn,
                    gen_tps,
                    prompt_tokens,
                    completion_tokens,
                } => {
                    self.is_generating = false;
                    self.vram.tps = gen_tps;
                    self.last_gen_tokens = completion_tokens;
                    if let Some(pt) = prompt_tokens {
                        self.workspace.used_ctx =
                            pt.saturating_add(completion_tokens.unwrap_or(0));
                    }
                    let _ = self.workspace.session.append_turn_with_tools(turn);
                    self.streaming_content.clear();
                    self.streaming_reasoning.clear();
                    self.streaming_tools.clear();
                    self.streaming_steps.clear();
                    self.active_stream = None;
                    if self.cfg.ui.auto_scroll && is_near_bottom {
                        self.needs_scroll_to_bottom = true;
                    }
                    break;
                }
                AgentEvent::Cancelled { partial_turn } => {
                    self.is_generating = false;
                    if let Some(turn) = partial_turn {
                        let _ = self.workspace.session.append_turn_with_tools(turn);
                    }
                    self.streaming_content.clear();
                    self.streaming_reasoning.clear();
                    self.streaming_tools.clear();
                    self.streaming_steps.clear();
                    self.active_stream = None;
                    break;
                }
                AgentEvent::Error(err) => {
                    self.is_generating = false;
                    let mut steps = std::mem::take(&mut self.streaming_steps);
                    steps.retain(|s| {
                        s.reasoning.is_some()
                            || !s.content.trim().is_empty()
                            || !s.tool_calls.is_empty()
                    });
                    steps.push(TurnStep {
                        reasoning: None,
                        content: format!("Ошибка генерации: {err}"),
                        tool_calls: Vec::new(),
                    });
                    let turn = ChatTurn {
                        role: "assistant".to_string(),
                        content: format!("Ошибка генерации: {err}"),
                        reasoning: None,
                        gen_tps: None,
                        duration_secs: None,
                        tokens: None,
                        image: None,
                        images: Vec::new(),
                        tool_traces: std::mem::take(&mut self.streaming_tools),
                        steps,
                    };
                    let _ = self.workspace.session.append_turn_with_tools(turn);
                    self.streaming_content.clear();
                    self.streaming_reasoning.clear();
                    self.streaming_tools.clear();
                    self.streaming_steps.clear();
                    self.active_stream = None;
                    if self.cfg.ui.auto_scroll && is_near_bottom {
                        self.needs_scroll_to_bottom = true;
                    }
                    break;
                }
            }
        }

        if stream_had_delta && self.cfg.ui.auto_scroll && is_near_bottom {
            self.needs_scroll_to_bottom = true;
        }

        true
    }

    fn sync_flat_streaming_tools(&mut self) {
        self.streaming_tools = self
            .streaming_steps
            .iter()
            .flat_map(|s| s.tool_calls.iter().cloned())
            .collect();
    }

    pub fn send_message(&mut self, text: String) {
        // Guard against re-entrant sends while a generation is in progress.
        if self.is_generating {
            self.status_notice =
                Some("Generation in progress; wait for completion.".to_string());
            return;
        }

        let trimmed = text.trim();
        if trimmed.is_empty() && self.attached_images.is_empty() {
            return;
        }

        self.reload_config_if_changed();
        self.refresh_server_status();

        // Keep the reported context size in sync with a running server.
        if let ServerStatus::Running { ctx, .. } = self.server_status {
            if ctx > 0 {
                self.vram.active_ctx = ctx;
            }
        }

        let is_running = matches!(self.server_status, ServerStatus::Running { .. })
            || server::LockState::read_active(&self.paths.lock_file()).is_some();

        if !is_running {
            if matches!(self.server_status, ServerStatus::Starting { .. }) {
                // Do not spam the session with identical notices on every
                // send attempt while the server is booting.
                let already_notified = self
                    .workspace
                    .session
                    .turns
                    .last()
                    .map(|t| {
                        t.role == "assistant"
                            && t.content.contains("Сервер llama.cpp сейчас запускается")
                    })
                    .unwrap_or(false);
                if already_notified {
                    return;
                }
                let turn = ChatTurn {
                    role: "assistant".to_string(),
                    content:
                        "Сервер llama.cpp сейчас запускается. Дождитесь готовности перед отправкой сообщения."
                            .to_string(),
                    reasoning: None,
                    gen_tps: None,
                    duration_secs: None,
                    tokens: None,
                    image: None,
                    images: Vec::new(),
                    tool_traces: Vec::new(),
                    steps: Vec::new(),
                };
                let _ = self.workspace.session.append_turn_with_tools(turn);
                return;
            }

            if let Some(ref meta) = self.active_model_meta {
                let model_name = meta.file_name.clone();
                let settings = self.settings_for(&model_name);
                self.start_server_for_model(&model_name, settings.ctx);
                let turn = ChatTurn {
                    role: "assistant".to_string(),
                    content: format!(
                        "Сервер llama.cpp не был запущен. Запускаю модель `{model_name}` (ctx: {})... Повторите отправку сообщения после запуска сервера.",
                        settings.ctx
                    ),
                    reasoning: None,
                    gen_tps: None,
                    duration_secs: None,
                    tokens: None,
                    image: None,
                    images: Vec::new(),
                    tool_traces: Vec::new(),
                    steps: Vec::new(),
                };
                let _ = self.workspace.session.append_turn_with_tools(turn);
                return;
            } else {
                let turn = ChatTurn {
                    role: "assistant".to_string(),
                    content:
                        "Сервер llama.cpp выключен, и нет выбранной модели для автозапуска. Выберите модель в меню моделей."
                            .to_string(),
                    reasoning: None,
                    gen_tps: None,
                    duration_secs: None,
                    tokens: None,
                    image: None,
                    images: Vec::new(),
                    tool_traces: Vec::new(),
                    steps: Vec::new(),
                };
                let _ = self.workspace.session.append_turn_with_tools(turn);
                return;
            }
        }

        let images: Vec<String> = self
            .attached_images
            .drain(..)
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let image_attachment = images.first().cloned();

        let user_turn = ChatTurn {
            role: "user".to_string(),
            content: trimmed.to_string(),
            reasoning: None,
            gen_tps: None,
            duration_secs: None,
            tokens: None,
            image: image_attachment,
            images,
            tool_traces: Vec::new(),
            steps: Vec::new(),
        };
        let _ = self.workspace.session.append_turn_with_tools(user_turn);

        let (messages, used_toks) =
            self.workspace
                .build_turn_messages(&self.paths, &self.cfg, self.vram.active_ctx);
        self.workspace.used_ctx = used_toks;

        let settings = self.settings_for_active();
        let (host, port) = self.current_endpoint();
        let agent_config = crate::chat::engine::AgentConfig {
            host,
            port,
            settings,
            cwd: self.paths.cwd.clone(),
            life_dir: self.paths.life_dir(),
            max_read_file_bytes: self.cfg.limits.max_read_file_bytes,
            max_tool_steps: self.cfg.limits.max_tool_steps,
        };
        let stream = crate::chat::engine::spawn_agent_turn(agent_config, messages);

        self.needs_scroll_to_bottom = true;
        self.active_stream = Some(stream);
        self.is_generating = true;
        self.streaming_content.clear();
        self.streaming_reasoning.clear();
        self.streaming_tools.clear();
        self.streaming_steps.clear();
    }
}
