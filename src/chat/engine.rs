use crate::chat::client::{StreamEvent, ToolCallItem, spawn_chat_stream};
use crate::chat::session::{ChatTurn, ToolTrace, TurnStep};
use crate::config::ModelSettings;
use crate::tools::{self, ToolLoopGuard};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Configuration for driving an autonomous agent turn.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub host: String,
    pub port: u16,
    pub settings: ModelSettings,
    pub cwd: PathBuf,
    pub life_dir: PathBuf,
    pub max_read_file_bytes: u64,
    pub max_tool_steps: u32,
}

/// Events emitted during autonomous multi-step agent reasoning and tool execution.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    StepStarted {
        step_idx: usize,
    },
    ContentDelta {
        step_idx: usize,
        delta: String,
    },
    ReasoningDelta {
        step_idx: usize,
        delta: String,
    },
    ToolStarted {
        step_idx: usize,
        call_idx: usize,
        name: String,
        arguments: String,
    },
    ToolFinished {
        step_idx: usize,
        call_idx: usize,
        name: String,
        arguments: String,
        output: String,
    },
    MemoryNotice(String),
    CwdChanged(PathBuf),
    TurnComplete {
        turn: ChatTurn,
        gen_tps: f64,
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
    },
    Cancelled {
        partial_turn: Option<ChatTurn>,
    },
    Error(String),
}

/// Handle to an in-flight autonomous agent execution loop.
pub struct ActiveAgentStream {
    pub rx: Receiver<AgentEvent>,
    pub cancel: Arc<AtomicBool>,
}

impl ActiveAgentStream {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// Spawns an autonomous agent execution loop in a background thread.
///
/// Encapsulates SSE stream reading, incremental token streaming, tool
/// execution, tool loop recursion protection, working directory tracking,
/// memory notification, and multi-turn prompt continuation behind a clean seam.
pub fn spawn_agent_turn(config: AgentConfig, initial_messages: Vec<Value>) -> ActiveAgentStream {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);

    thread::spawn(move || {
        run_agent_loop(config, initial_messages, tx, cancel_clone);
    });

    ActiveAgentStream { rx, cancel }
}

fn run_agent_loop(
    mut config: AgentConfig,
    mut messages: Vec<Value>,
    tx: Sender<AgentEvent>,
    cancel: Arc<AtomicBool>,
) {
    let mut tool_guard = ToolLoopGuard::default();
    let mut accumulated_content = String::new();
    let mut accumulated_reasoning = String::new();
    let mut tool_traces: Vec<ToolTrace> = Vec::new();
    let mut steps: Vec<TurnStep> = Vec::new();
    let mut last_gen_tps = 0.0;
    let mut last_prompt_tokens = None;
    let mut last_completion_tokens = None;

    loop {
        if cancel.load(Ordering::SeqCst) {
            let partial = build_partial_turn(
                &accumulated_content,
                &accumulated_reasoning,
                tool_traces,
                steps,
                last_gen_tps,
                last_completion_tokens,
            );
            let _ = tx.send(AgentEvent::Cancelled {
                partial_turn: partial,
            });
            return;
        }

        let step_idx = steps.len();
        steps.push(TurnStep::default());
        let _ = tx.send(AgentEvent::StepStarted { step_idx });

        let tools = if tool_guard.steps >= config.max_tool_steps {
            None
        } else {
            Some(tools::tool_definitions_json())
        };

        let stream = spawn_chat_stream(
            config.host.clone(),
            config.port,
            messages.clone(),
            config.settings.clone(),
            tools,
        );

        let mut step_content = String::new();
        let mut step_reasoning = String::new();
        let mut step_tool_calls: Vec<ToolCallItem> = Vec::new();
        let mut stream_cancelled = false;
        let mut stream_error = None;

        while let Ok(event) = stream.rx.recv() {
            if cancel.load(Ordering::SeqCst) {
                stream.cancel();
                stream_cancelled = true;
                break;
            }

            match event {
                StreamEvent::ContentDelta(delta) => {
                    step_content.push_str(&delta);
                    steps[step_idx].content.push_str(&delta);
                    let _ = tx.send(AgentEvent::ContentDelta { step_idx, delta });
                }
                StreamEvent::ReasoningDelta(delta) => {
                    step_reasoning.push_str(&delta);
                    let r = steps[step_idx].reasoning.get_or_insert_with(String::new);
                    r.push_str(&delta);
                    let _ = tx.send(AgentEvent::ReasoningDelta { step_idx, delta });
                }
                StreamEvent::ToolCallProgress(calls) => {
                    for (call_idx, call) in calls.into_iter().enumerate() {
                        while steps[step_idx].tool_calls.len() <= call_idx {
                            steps[step_idx].tool_calls.push(ToolTrace {
                                name: String::new(),
                                arguments: String::new(),
                                output: "Running...".to_string(),
                            });
                        }
                        steps[step_idx].tool_calls[call_idx].name = call.name.clone();
                        steps[step_idx].tool_calls[call_idx].arguments = call.arguments.clone();
                        let _ = tx.send(AgentEvent::ToolStarted {
                            step_idx,
                            call_idx,
                            name: call.name,
                            arguments: call.arguments,
                        });
                    }
                }
                StreamEvent::Done {
                    content,
                    reasoning,
                    tool_calls,
                    gen_tps,
                    prompt_tokens,
                    completion_tokens,
                } => {
                    last_gen_tps = gen_tps;
                    last_prompt_tokens = prompt_tokens;
                    last_completion_tokens = completion_tokens;

                    if !content.is_empty() {
                        step_content = content;
                    }
                    if let Some(r) = reasoning {
                        step_reasoning = r;
                    }
                    step_tool_calls = tool_calls;
                    break;
                }
                StreamEvent::Cancelled {
                    partial_content,
                    partial_reasoning,
                } => {
                    if !partial_content.is_empty() {
                        step_content = partial_content;
                    }
                    if let Some(r) = partial_reasoning {
                        step_reasoning = r;
                    }
                    stream_cancelled = true;
                    break;
                }
                StreamEvent::Error(err) => {
                    stream_error = Some(err);
                    break;
                }
            }
        }

        steps[step_idx].content = step_content.clone();
        steps[step_idx].reasoning = (!step_reasoning.trim().is_empty()).then(|| step_reasoning.clone());

        if stream_cancelled || cancel.load(Ordering::SeqCst) {
            if !step_content.trim().is_empty() {
                if !accumulated_content.is_empty() {
                    accumulated_content.push_str("\n\n");
                }
                accumulated_content.push_str(step_content.trim());
            }
            append_reasoning(&mut accumulated_reasoning, &step_reasoning);
            let partial = build_partial_turn(
                &accumulated_content,
                &accumulated_reasoning,
                tool_traces,
                steps,
                last_gen_tps,
                last_completion_tokens,
            );
            let _ = tx.send(AgentEvent::Cancelled {
                partial_turn: partial,
            });
            return;
        }

        if let Some(err) = stream_error {
            let _ = tx.send(AgentEvent::Error(err));
            return;
        }

        append_reasoning(&mut accumulated_reasoning, &step_reasoning);

        if step_tool_calls.is_empty() {
            // Model finished generation with a final response
            if !step_content.trim().is_empty() {
                if !accumulated_content.is_empty() {
                    accumulated_content.push_str("\n\n");
                }
                accumulated_content.push_str(step_content.trim());
            }
            // Prune empty steps
            steps.retain(|s| {
                s.reasoning.is_some() || !s.content.trim().is_empty() || !s.tool_calls.is_empty()
            });
            let turn = ChatTurn {
                role: "assistant".to_string(),
                content: accumulated_content,
                reasoning: if accumulated_reasoning.is_empty() {
                    None
                } else {
                    Some(accumulated_reasoning)
                },
                gen_tps: (last_gen_tps > 0.0).then_some(last_gen_tps),
                duration_secs: None,
                tokens: last_completion_tokens,
                image: None,
                images: Vec::new(),
                tool_traces,
                steps,
            };
            let _ = tx.send(AgentEvent::TurnComplete {
                turn,
                gen_tps: last_gen_tps,
                prompt_tokens: last_prompt_tokens,
                completion_tokens: last_completion_tokens,
            });
            return;
        }

        // Multi-turn tool execution step
        if !step_content.trim().is_empty() {
            if !accumulated_content.is_empty() {
                accumulated_content.push_str("\n\n");
            }
            accumulated_content.push_str(step_content.trim());
        }

        let mut tc_json = Vec::new();
        let mut results = Vec::new();
        let mut extra_user_imgs = Vec::new();

        for (call_idx, tc) in step_tool_calls.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                for rem_idx in call_idx..step_tool_calls.len() {
                    let rem_tc = &step_tool_calls[rem_idx];
                    if rem_idx < steps[step_idx].tool_calls.len() {
                        steps[step_idx].tool_calls[rem_idx].output = "Cancelled".to_string();
                    }
                    let _ = tx.send(AgentEvent::ToolFinished {
                        step_idx,
                        call_idx: rem_idx,
                        name: rem_tc.name.clone(),
                        arguments: rem_tc.arguments.clone(),
                        output: "Cancelled".to_string(),
                    });
                }
                let partial = build_partial_turn(
                    &accumulated_content,
                    &accumulated_reasoning,
                    tool_traces,
                    steps,
                    last_gen_tps,
                    last_completion_tokens,
                );
                let _ = tx.send(AgentEvent::Cancelled { partial_turn: partial });
                return;
            }

            let call_id = if tc.id.trim().is_empty() {
                format!("call_{step_idx}_{call_idx}_{}", tc.name)
            } else {
                tc.id.clone()
            };

            let safe_args_for_server = tools::sanitize_tool_arguments_for_server(&tc.arguments);
            let fn_call = serde_json::json!({
                "name": tc.name,
                "arguments": safe_args_for_server,
            });

            while steps[step_idx].tool_calls.len() <= call_idx {
                steps[step_idx].tool_calls.push(ToolTrace {
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    output: "Running...".to_string(),
                });
            }

            if let Err(r) = tool_guard.allow_call(&tc.name, &tc.arguments, config.max_tool_steps) {
                let err_out = format!("ERROR: {r}");
                steps[step_idx].tool_calls[call_idx].name = tc.name.clone();
                steps[step_idx].tool_calls[call_idx].arguments = tc.arguments.clone();
                steps[step_idx].tool_calls[call_idx].output = err_out.clone();
                tool_traces.push(steps[step_idx].tool_calls[call_idx].clone());

                tc_json.push(serde_json::json!({
                    "id": call_id,
                    "type": "function",
                    "function": fn_call,
                }));
                results.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "name": tc.name,
                    "content": err_out.clone(),
                }));
                let _ = tx.send(AgentEvent::ToolFinished {
                    step_idx,
                    call_idx,
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    output: err_out,
                });
                for rem_idx in (call_idx + 1)..step_tool_calls.len() {
                    let rem_tc = &step_tool_calls[rem_idx];
                    let skip_out = "Skipped (loop guard triggered)".to_string();
                    while steps[step_idx].tool_calls.len() <= rem_idx {
                        steps[step_idx].tool_calls.push(ToolTrace {
                            name: rem_tc.name.clone(),
                            arguments: rem_tc.arguments.clone(),
                            output: skip_out.clone(),
                        });
                    }
                    steps[step_idx].tool_calls[rem_idx].output = skip_out.clone();
                    let _ = tx.send(AgentEvent::ToolFinished {
                        step_idx,
                        call_idx: rem_idx,
                        name: rem_tc.name.clone(),
                        arguments: rem_tc.arguments.clone(),
                        output: skip_out,
                    });
                }
                break;
            }

            let out = tools::execute_tool_call(
                &config.cwd,
                &config.life_dir,
                tc,
                config.max_read_file_bytes,
            );

            if let Some(fact) = out.mem_added {
                let _ = tx.send(AgentEvent::MemoryNotice(fact));
            }

            if let Some(new_cwd) = out.new_cwd {
                config.cwd = new_cwd.clone();
                let _ = tx.send(AgentEvent::CwdChanged(new_cwd));
            }

            if let Some(img_path) = out.viewed_image {
                if let Ok(img) = crate::vision::load_image_file(&img_path) {
                    let img_val = serde_json::json!({ "url": img.data_url() });
                    extra_user_imgs.push(serde_json::json!({
                        "role": "user",
                        "content": [
                            { "type": "text", "text": format!("Image from tool ({}):", img.file_name) },
                            { "type": "image_url", "image_url": img_val }
                        ]
                    }));
                }
            }

            steps[step_idx].tool_calls[call_idx] = ToolTrace {
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
                output: out.output.clone(),
            };
            tool_traces.push(steps[step_idx].tool_calls[call_idx].clone());

            let _ = tx.send(AgentEvent::ToolFinished {
                step_idx,
                call_idx,
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
                output: out.output.clone(),
            });

            tc_json.push(serde_json::json!({
                "id": call_id,
                "type": "function",
                "function": fn_call,
            }));
            results.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": call_id,
                "name": tc.name,
                "content": out.output,
            }));
        }

        messages.push(serde_json::json!({
            "role": "assistant",
            "content": step_content,
            "tool_calls": tc_json,
        }));
        messages.extend(results);
        messages.extend(extra_user_imgs);
    }
}

fn append_reasoning(target: &mut String, delta: &str) {
    let trimmed = delta.trim();
    if trimmed.is_empty() {
        return;
    }
    if !target.contains(trimmed) {
        if !target.is_empty() {
            target.push_str("\n\n");
        }
        target.push_str(trimmed);
    }
}

fn build_partial_turn(
    content: &str,
    reasoning: &str,
    tool_traces: Vec<ToolTrace>,
    mut steps: Vec<TurnStep>,
    gen_tps: f64,
    tokens: Option<u32>,
) -> Option<ChatTurn> {
    steps.retain(|s| {
        s.reasoning.is_some() || !s.content.trim().is_empty() || !s.tool_calls.is_empty()
    });
    if content.is_empty() && reasoning.is_empty() && tool_traces.is_empty() && steps.is_empty() {
        None
    } else {
        Some(ChatTurn {
            role: "assistant".to_string(),
            content: content.to_string(),
            reasoning: if reasoning.is_empty() {
                None
            } else {
                Some(reasoning.to_string())
            },
            gen_tps: (gen_tps > 0.0).then_some(gen_tps),
            duration_secs: None,
            tokens,
            image: None,
            images: Vec::new(),
            tool_traces,
            steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn appends_reasoning_without_duplicates() {
        let mut r = String::new();
        append_reasoning(&mut r, "Thinking about architecture.");
        assert_eq!(r, "Thinking about architecture.");

        // Duplicate delta should not be appended again
        append_reasoning(&mut r, "Thinking about architecture.");
        assert_eq!(r, "Thinking about architecture.");

        // New distinct delta appends with double newline
        append_reasoning(&mut r, "Next consideration.");
        assert_eq!(
            r,
            "Thinking about architecture.\n\nNext consideration."
        );
    }

    #[core::prelude::v1::test]
    fn builds_partial_turn_correctly() {
        let empty = build_partial_turn("", "", Vec::new(), Vec::new(), 0.0, None);
        assert!(empty.is_none());

        let traces = vec![ToolTrace {
            name: "read_file".to_string(),
            arguments: "{}".to_string(),
            output: "test output".to_string(),
        }];
        let partial = build_partial_turn(
            "partial content",
            "reasoning step",
            traces,
            Vec::new(),
            24.5,
            Some(42),
        );
        assert!(partial.is_some());
        let turn = partial.unwrap();
        assert_eq!(turn.role, "assistant");
        assert_eq!(turn.content, "partial content");
        assert_eq!(turn.reasoning.as_deref(), Some("reasoning step"));
        assert_eq!(turn.gen_tps, Some(24.5));
        assert_eq!(turn.tokens, Some(42));
        assert_eq!(turn.tool_traces.len(), 1);
    }
}
