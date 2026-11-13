use crate::config::ModelSettings;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

pub const STREAM_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCallItem {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum StreamEvent {
    ReasoningDelta(String),
    ContentDelta(String),
    ToolCallProgress(Vec<ToolCallItem>),
    Done {
        content: String,
        reasoning: Option<String>,
        tool_calls: Vec<ToolCallItem>,
        gen_tps: f64,
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
    },
    Cancelled {
        partial_content: String,
        partial_reasoning: Option<String>,
    },
    Error(String),
}

pub struct ActiveStream {
    pub rx: Receiver<StreamEvent>,
    pub cancel: Arc<AtomicBool>,
}

impl ActiveStream {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

pub fn spawn_chat_stream(
    host: String,
    port: u16,
    messages: Vec<Value>,
    settings: ModelSettings,
    tools: Option<Value>,
) -> ActiveStream {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);

    thread::spawn(move || {
        run_sse_stream(&host, port, messages, &settings, tools, &tx, &cancel_clone);
    });

    ActiveStream { rx, cancel }
}

fn run_sse_stream(
    host: &str,
    port: u16,
    messages: Vec<Value>,
    settings: &ModelSettings,
    tools: Option<Value>,
    tx: &Sender<StreamEvent>,
    cancel: &Arc<AtomicBool>,
) {
    let url = format!("http://{host}:{port}/v1/chat/completions");
    let mut body = json!({
        "messages": messages,
        "stream": true,
        "timings_per_token": true,
        "stream_options": { "include_usage": true },
        "temperature": settings.temperature,
        "top_k": settings.top_k,
        "top_p": settings.top_p,
        "min_p": settings.min_p,
        "repeat_penalty": settings.repeat_penalty,
        "max_tokens": settings.max_tokens,
    });
    apply_reasoning_request_fields(&mut body, &settings.reasoning_effort);
    if let Some(t) = tools {
        body["tools"] = t;
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(STREAM_TIMEOUT_SECS)))
        .build()
        .into();

    let mut resp = match agent.post(&url).send_json(&body) {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(StreamEvent::Error(format!("llama.cpp error: {e}")));
            return;
        }
    };

    let reader = BufReader::new(resp.body_mut().as_reader());
    let mut full_content = String::new();
    let mut full_reasoning = String::new();
    let mut tool_calls: Vec<ToolCallItem> = Vec::new();
    let mut server_gen_tps: Option<f64> = None;
    let mut prompt_tokens: Option<u32> = None;
    let mut completion_tokens: Option<u32> = None;

    for line_res in reader.lines() {
        if cancel.load(Ordering::SeqCst) {
            let _ = tx.send(StreamEvent::Cancelled {
                partial_content: full_content,
                partial_reasoning: (!full_reasoning.is_empty()).then_some(full_reasoning),
            });
            return;
        }
        let line = match line_res {
            Ok(l) => l,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(format!("Refusal SSE: {e}")));
                return;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.starts_with("data:") {
            continue;
        }
        let payload = trimmed.trim_start_matches("data:").trim();
        if payload == "[DONE]" {
            break;
        }
        let Ok(chunk) = serde_json::from_str::<SseChunk>(payload) else {
            continue;
        };
        if let Some(t) = chunk.timings
            && t.predicted_per_second > 0.0
        {
            server_gen_tps = Some(t.predicted_per_second);
        }
        if let Some(u) = chunk.usage {
            if u.prompt_tokens > 0 {
                prompt_tokens = Some(u.prompt_tokens);
            }
            if u.completion_tokens > 0 {
                completion_tokens = Some(u.completion_tokens);
            }
        }
        for choice in chunk.choices {
            if let Some(r_delta) = choice
                .delta
                .reasoning_content
                .or(choice.delta.reasoning)
                .filter(|s| !s.is_empty())
            {
                let clean_r = strip_raw_thought_markers(&r_delta);
                if !clean_r.is_empty() {
                    full_reasoning.push_str(&clean_r);
                    let _ = tx.send(StreamEvent::ReasoningDelta(clean_r));
                }
            }
            if let Some(c_delta) = choice.delta.content.filter(|s| !s.is_empty()) {
                full_content.push_str(&c_delta);
                let _ = tx.send(StreamEvent::ContentDelta(c_delta));
            }
            if let Some(tcs) = choice.delta.tool_calls {
                for tc in tcs {
                    let idx = tc.index.unwrap_or(0);
                    while tool_calls.len() <= idx {
                        tool_calls.push(ToolCallItem::default());
                    }
                    if let Some(id) = tc.id {
                        tool_calls[idx].id = id;
                    }
                    if let Some(f) = tc.function {
                        if let Some(n) = f.name {
                            tool_calls[idx].name.push_str(&n);
                        }
                        if let Some(a) = f.arguments {
                            tool_calls[idx].arguments.push_str(&a);
                        }
                    }
                }
                let _ = tx.send(StreamEvent::ToolCallProgress(tool_calls.clone()));
            }
        }
    }

    let gen_tps = server_gen_tps
        .filter(|&v| v > 0.0)
        .unwrap_or_else(crate::server::launch::latest_server_gen_tps);

    if full_reasoning.is_empty() {
        extract_fallback_reasoning(&mut full_content, &mut full_reasoning);
    }
    full_content = strip_raw_thought_markers(&full_content);

    let _ = tx.send(StreamEvent::Done {
        content: full_content,
        reasoning: (!full_reasoning.is_empty()).then_some(full_reasoning),
        tool_calls,
        gen_tps,
        prompt_tokens,
        completion_tokens,
    });
}

#[allow(dead_code)]
pub fn summarize_turns_sync(host: &str, port: u16, transcript: &str) -> io::Result<String> {
    let url = format!("http://{host}:{port}/v1/chat/completions");
    let body = json!({
        "messages": [
            {
                "role": "system",
                "content": "You are a prompt engineer; your task is to condense the dialogue context so that the next model is informed about what has transpired. Create a concise, precise summary containing key facts, decisions, context, and open questions. Do not include introductory phrases."
            },
            {
                "role": "user",
                "content": transcript
            }
        ],
        "stream": false,
        "temperature": 0.3,
        "max_tokens": 2048,
        "reasoning_effort": "low"
    });
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(180)))
        .build()
        .into();
    let mut resp = agent
        .post(&url)
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("compact request failed: {e}")))?;
    let val: Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::other(e.to_string()))?;
    let text = val["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(io::Error::other("empty response during context compression"));
    }
    Ok(text)
}

/// Exactly counts tokens for `content` using llama-server's `/tokenize` endpoint.
/// Returns the exact count calculated by the loaded model's compiled tokenizer.
pub fn count_tokens_exact(host: &str, port: u16, content: &str) -> io::Result<usize> {
    if content.trim().is_empty() {
        return Ok(0);
    }
    let url = format!("http://{host}:{port}/tokenize");
    let body = json!({
        "content": content,
        "add_special": false,
    });
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(600)))
        .build()
        .into();
    let mut resp = agent
        .post(&url)
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("tokenize request failed: {e}")))?;
    let val: Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::other(e.to_string()))?;
    if let Some(tokens) = val.get("tokens").and_then(|t| t.as_array()) {
        Ok(tokens.len())
    } else {
        Err(io::Error::other("invalid tokenize response: missing tokens array"))
    }
}

#[derive(Deserialize)]
struct SseChunk {
    #[serde(default)]
    choices: Vec<SseChoice>,
    timings: Option<SseTimings>,
    usage: Option<SseUsage>,
}

#[derive(Deserialize)]
struct SseChoice {
    delta: SseDelta,
}

#[derive(Deserialize)]
struct SseDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
    reasoning: Option<String>,
    tool_calls: Option<Vec<SseToolCallDelta>>,
}

#[derive(Deserialize)]
struct SseToolCallDelta {
    index: Option<usize>,
    id: Option<String>,
    function: Option<SseFunctionDelta>,
}

#[derive(Deserialize)]
struct SseFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct SseTimings {
    #[serde(default)]
    predicted_per_second: f64,
}

#[derive(Deserialize)]
struct SseUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

fn strip_raw_thought_markers(s: &str) -> String {
    s.replace("<|channel>thought\n", "")
        .replace("<|channel>thought", "")
        .replace("<|channel|>thought\n", "")
        .replace("<|channel|>thought", "")
        .replace("<channel|>", "")
        .replace("<|channel|>", "")
}

fn apply_reasoning_request_fields(body: &mut Value, reasoning_effort: &str) {
    if reasoning_effort == "off" {
        body["chat_template_kwargs"] = json!({ "enable_thinking": false });
    } else {
        body["reasoning_effort"] = Value::String(reasoning_effort.to_string());
        body["chat_template_kwargs"] = json!({ "enable_thinking": true });
    }
}

fn extract_fallback_reasoning(content: &mut String, reasoning: &mut String) {
    if !reasoning.is_empty() {
        return;
    }
    let gemma_starts = ["<|channel>thought", "<|channel|>thought"];
    for start_pat in gemma_starts {
        if let Some(start_idx) = content.find(start_pat) {
            let after_start = &content[start_idx + start_pat.len()..];
            let gemma_ends = ["<channel|>", "<|channel|>"];
            for end_pat in gemma_ends {
                if let Some(end_rel) = after_start.find(end_pat) {
                    let thought_text = after_start[..end_rel].trim();
                    let rem = format!(
                        "{}{}",
                        content[..start_idx].trim_end(),
                        after_start[end_rel + end_pat.len()..].trim_start()
                    );
                    *reasoning = thought_text.to_string();
                    *content = rem.trim().to_string();
                    return;
                }
            }
        }
    }
    if let Some(start_idx) = content.find("<think>") {
        let after_start = &content[start_idx + "<think>".len()..];
        if let Some(end_rel) = after_start.find("</think>") {
            let thought_text = after_start[..end_rel].trim();
            let rem = format!(
                "{}{}",
                content[..start_idx].trim_end(),
                after_start[end_rel + "</think>".len()..].trim_start()
            );
            *reasoning = thought_text.to_string();
            *content = rem.trim().to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_gemma_channel_reasoning() {
        let mut content = "<|channel>thought\nLet me plan this step by step.\n<channel|>\nHere is the answer.".to_string();
        let mut reasoning = String::new();
        extract_fallback_reasoning(&mut content, &mut reasoning);
        assert_eq!(reasoning, "Let me plan this step by step.");
        assert_eq!(content, "Here is the answer.");
    }

    #[test]
    fn extracts_think_tag_reasoning() {
        let mut content = "<think>\nDeep thinking here.\n</think>\nFinal response.".to_string();
        let mut reasoning = String::new();
        extract_fallback_reasoning(&mut content, &mut reasoning);
        assert_eq!(reasoning, "Deep thinking here.");
        assert_eq!(content, "Final response.");
    }

    #[test]
    fn skips_extraction_when_reasoning_already_present() {
        let mut content = "<think>fake</think>real content".to_string();
        let mut reasoning = "pre-existing reasoning".to_string();
        extract_fallback_reasoning(&mut content, &mut reasoning);
        assert_eq!(reasoning, "pre-existing reasoning");
        assert_eq!(content, "<think>fake</think>real content");
    }

    #[test]
    fn sets_enable_thinking_kwargs_based_on_reasoning_effort() {
        let mut off_body = json!({});
        apply_reasoning_request_fields(&mut off_body, "off");
        assert!(off_body.get("reasoning_effort").is_none());
        assert_eq!(off_body["chat_template_kwargs"]["enable_thinking"], false);

        let mut high_body = json!({});
        apply_reasoning_request_fields(&mut high_body, "high");
        assert_eq!(high_body["reasoning_effort"], "high");
        assert_eq!(high_body["chat_template_kwargs"]["enable_thinking"], true);
    }
}
