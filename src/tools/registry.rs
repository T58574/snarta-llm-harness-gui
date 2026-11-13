use crate::chat::client::ToolCallItem;
use crate::tools::fs::{
    ReadFileOptions, harness_delete_file, harness_edit_file, harness_read_file,
    harness_write_file, remember_fact, write_note_doc,
};
use crate::tools::plan::{
    TaskManager, TaskStatus, enter_plan_mode, exit_plan_mode, format_ask_user, is_plan_mode,
};
use crate::tools::search_local::{harness_glob, harness_grep};
use crate::tools::security::resolve_user_path;
use crate::tools::shell::{ShellJobManager, run_exec_cmd};
use crate::tools::web_scrape::{alexandria_discover, alexandria_scrape};
use crate::tools::web_search::{execute_search_cascade, format_search_results};
use crate::tools::{
    ToolOutcome, get_exa_api_key, get_firecrawl_api_key, get_parallel_api_key, get_tavily_api_key,
};
use serde_json::{Value, json};
use std::io;
use std::path::Path;

/// Authoritative JSON schema definitions for all built-in tools.
pub fn tool_definitions_json() -> Value {
    const JSON_STR: &str = r#"[
      {"type":"function","function":{"name":"remember","description":"Record an important fact about the user in long-term memory (life/memory.md).","parameters":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}},
      {"type":"function","function":{"name":"write_note","description":"Create or append to a note in life/notes/.","parameters":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"},"append":{"type":"boolean"}},"required":["path","content"]}}},
      {"type":"function","function":{"name":"write_file","description":"Create or overwrite a file on the user's local filesystem at the given path (relative to cwd or absolute path). You run natively on the host PC with full permissions.","parameters":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"},"append":{"type":"boolean"}},"required":["path","content"]}}},
      {"type":"function","function":{"name":"edit_file","description":"Surgically edit a file on the user's local filesystem by replacing unique old_text with new_text.","parameters":{"type":"object","properties":{"path":{"type":"string"},"old_text":{"type":"string"},"new_text":{"type":"string"}},"required":["path","old_text","new_text"]}}},
      {"type":"function","function":{"name":"read_file","description":"Read a text file from the user's local filesystem with pagination and ledger tracking.","parameters":{"type":"object","properties":{"path":{"type":"string"},"start_line":{"type":"integer"},"end_line":{"type":"integer"},"byte_offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}}},
      {"type":"function","function":{"name":"glob_files","description":"Search files by glob pattern returning paths, line counts, and file sizes. Use to locate files instead of shell.","parameters":{"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}}},
      {"type":"function","function":{"name":"grep_files","description":"Fast regex/substring search through file contents (ripgrep with fallback). Use to locate definitions, types, and code snippets instead of grep in exec.","parameters":{"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string"}},"required":["query"]}}},
      {"type":"function","function":{"name":"list_files","description":"List files in directory on the local filesystem (defaults to current working directory, or specified path).","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":[]}}},
      {"type":"function","function":{"name":"change_dir","description":"Change current working directory (cwd) on the local filesystem.","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}}},
      {"type":"function","function":{"name":"view_image","description":"Inspect an image file (PNG, JPG, WEBP, BMP, GIF) from the local filesystem.","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}}},
      {"type":"function","function":{"name":"delete_file","description":"Delete a file at the given path (relative to cwd, life/, or absolute path).","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}}},
      {"type":"function","function":{"name":"exec","description":"Execute a PowerShell command directly on the user's local Windows machine. You are NOT in a sandbox or container; you have full local user permissions. Do not wrap with powershell -Command.","parameters":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}}},
      {"type":"function","function":{"name":"shell_start","description":"Start a long-running command in the background and receive a job_id.","parameters":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}}},
      {"type":"function","function":{"name":"shell_output","description":"Read fresh output of a background job by job_id from cursor.","parameters":{"type":"object","properties":{"job_id":{"type":"string"},"cursor":{"type":"integer"}},"required":["job_id"]}}},
      {"type":"function","function":{"name":"shell_kill","description":"Stop a background job by job_id.","parameters":{"type":"object","properties":{"job_id":{"type":"string"}},"required":["job_id"]}}},
      {"type":"function","function":{"name":"task_create","description":"Create a new task in the agent tracker.","parameters":{"type":"object","properties":{"subject":{"type":"string"},"description":{"type":"string"}},"required":["subject"]}}},
      {"type":"function","function":{"name":"task_list","description":"View current task list and their statuses.","parameters":{"type":"object","properties":{},"required":[]}}},
      {"type":"function","function":{"name":"task_update","description":"Update task status (pending, in_progress, completed, blocked).","parameters":{"type":"object","properties":{"id":{"type":"integer"},"status":{"type":"string"}},"required":["id","status"]}}},
      {"type":"function","function":{"name":"ask_user","description":"Ask user a structured question when facing ambiguity or to select options.","parameters":{"type":"object","properties":{"question":{"type":"string"},"options":{"type":"array","items":{"type":"string"}},"allow_free_text":{"type":"boolean"}},"required":["question"]}}},
      {"type":"function","function":{"name":"plan_mode","description":"Enter or exit strict planning mode (read-only mode prior to user plan approval).","parameters":{"type":"object","properties":{"action":{"type":"string","enum":["enter","exit"]},"summary":{"type":"string"}},"required":["action"]}}},
      {"type":"function","function":{"name":"web_search","description":"Cascading web search (Tavily / Parallel.ai / Exa.ai) with built-in failover.","parameters":{"type":"object","properties":{"query":{"type":"string"},"provider":{"type":"string"}},"required":["query"]}}},
      {"type":"function","function":{"name":"alexandria_search","description":"Search providers in Firecrawl Alexandria knowledge base (free).","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}},
      {"type":"function","function":{"name":"alexandria_scrape","description":"Extract data via discovered Firecrawl Alexandria capability (uses credits).","parameters":{"type":"object","properties":{"provider":{"type":"string"},"capability":{"type":"string"},"options":{"type":"object"}},"required":["provider","capability"]}}}
    ]"#;
    serde_json::from_str(JSON_STR).unwrap_or(Value::Array(Vec::new()))
}

/// Format compact, human-readable label for a tool call execution chip.
pub fn format_tool_summary(name: &str, args_json: &str) -> String {
    let args: Value = parse_or_repair_tool_args(args_json).unwrap_or_else(|_| json!({}));
    let p = args["path"]
        .as_str()
        .or_else(|| args["subdir"].as_str())
        .unwrap_or("");
    match name {
        "exec" | "shell_start" => {
            let one = args["command"]
                .as_str()
                .unwrap_or(args_json.trim())
                .trim()
                .lines()
                .next()
                .unwrap_or("");
            format!("$ {}", truncate_str(one, 72))
        }
        "shell_output" => format!("shell output: {}", args["job_id"].as_str().unwrap_or("")),
        "shell_kill" => format!("stop task: {}", args["job_id"].as_str().unwrap_or("")),
        "read_file" => format!("read: {p}"),
        "write_file" => {
            let m = if args["append"].as_bool().unwrap_or(false) {
                "append"
            } else {
                "write"
            };
            format!("{m}: {p}")
        }
        "edit_file" => format!("edit: {p}"),
        "glob_files" => format!("glob: {}", args["pattern"].as_str().unwrap_or("")),
        "grep_files" => format!("grep: {}", args["query"].as_str().unwrap_or("")),
        "write_note" => format!("note: {p}"),
        "list_files" => format!("list: {}", if p.is_empty() { "." } else { p }),
        "change_dir" | "cd" => format!("cd: {p}"),
        "view_image" => format!("view image: {p}"),
        "delete_file" => format!("delete: {p}"),
        "task_create" => format!("task: {}", args["subject"].as_str().unwrap_or("")),
        "task_list" => "task list".to_string(),
        "task_update" => format!(
            "update task #{}: {}",
            args["id"].as_i64().unwrap_or(0),
            args["status"].as_str().unwrap_or("")
        ),
        "ask_user" => format!("ask: {}", args["question"].as_str().unwrap_or("")),
        "plan_mode" => format!("plan: {}", args["action"].as_str().unwrap_or("")),
        "web_search" => format!("web search: {}", args["query"].as_str().unwrap_or("")),
        "alexandria_search" => {
            format!("Alexandria search: {}", args["query"].as_str().unwrap_or(""))
        }
        "alexandria_scrape" => format!(
            "Alexandria scrape: {}",
            args["capability"].as_str().unwrap_or("")
        ),
        "remember" => format!(
            "memory: {}",
            truncate_str(args["text"].as_str().unwrap_or(""), 60)
        ),
        other if p.is_empty() => format!("{other}({})", truncate_str(args_json.trim(), 56)),
        other => format!("{other}: {p}"),
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// Executes a tool call with plan mode protection and outcome capture.
pub fn execute_tool_call(
    cwd: &Path,
    life_dir: &Path,
    call: &ToolCallItem,
    max_read_bytes: u64,
) -> ToolOutcome {
    let mut outcome = ToolOutcome {
        output: String::new(),
        mem_added: None,
        new_cwd: None,
        viewed_image: None,
    };
    outcome.output = match run_tool(cwd, life_dir, call, max_read_bytes, &mut outcome) {
        Ok(s) => s,
        Err(e) => format!("Error: {e}"),
    };
    outcome
}

/// Parses tool call JSON arguments or attempts lightweight repair if truncated or containing unescaped newlines.
pub fn parse_or_repair_tool_args(raw: &str) -> Result<Value, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return Ok(v);
    }

    // Attempt 1: Escape literal unescaped newlines/tabs inside JSON strings
    let escaped = escape_newlines_in_json_strings(trimmed);
    if let Ok(v) = serde_json::from_str::<Value>(&escaped) {
        return Ok(v);
    }

    // Attempt 2: Close truncated JSON string and object braces if cut off by max_tokens
    for suffix in ["\"}", "\"}", "}", "\"]}", "null}"] {
        let candidate = format!("{escaped}{suffix}");
        if let Ok(v) = serde_json::from_str::<Value>(&candidate) {
            return Err(format!(
                "Tool call JSON arguments were truncated mid-stream (likely hit max_tokens or had unescaped quotes). Parsed partial keys: {:?}. Please retry with smaller content chunks or valid JSON.",
                v.as_object().map(|o| o.keys().collect::<Vec<_>>()).unwrap_or_default()
            ));
        }
    }

    Err(format!(
        "Invalid or truncated JSON in tool call arguments (length {} chars). Ensure all quotes and backslashes are properly escaped and output is not cut off.",
        trimmed.len()
    ))
}

/// Ensures an arguments string is valid JSON before sending it back to llama-server's Jinja template engine.
pub fn sanitize_tool_arguments_for_server(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "{}".to_string();
    }
    if serde_json::from_str::<Value>(trimmed).is_ok() {
        return trimmed.to_string();
    }
    let escaped = escape_newlines_in_json_strings(trimmed);
    if serde_json::from_str::<Value>(&escaped).is_ok() {
        return escaped;
    }
    "{}".to_string()
}

fn escape_newlines_in_json_strings(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let mut in_str = false;
    let mut escaped = false;
    for ch in s.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && in_str {
            out.push(ch);
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_str = !in_str;
            out.push(ch);
            continue;
        }
        if in_str {
            match ch {
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn run_tool(
    cwd: &Path,
    life_dir: &Path,
    call: &ToolCallItem,
    max_read_bytes: u64,
    out_meta: &mut ToolOutcome,
) -> io::Result<String> {
    let args: Value = parse_or_repair_tool_args(&call.arguments)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    // Enforce Plan Mode guard: prevent mutations in read-only planning mode
    if is_plan_mode() {
        match call.name.as_str() {
            "write_file" | "edit_file" | "delete_file" | "exec" | "shell_start" => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "PLAN_MODE_ACTIVE: File modifications and command execution are blocked in plan mode. Use plan_mode(action: 'exit') after plan approval.",
                ));
            }
            _ => {}
        }
    }

    match call.name.as_str() {
        "remember" => {
            let raw = args["text"].as_str().unwrap_or("");
            let fact = remember_fact(life_dir, raw)?;
            out_meta.mem_added = Some(fact.clone());
            Ok(format!("Recorded in memory.md: {fact}"))
        }
        "write_note" => {
            let rel = args["path"].as_str().unwrap_or("");
            let content = args["content"].as_str().unwrap_or("");
            let append = args["append"].as_bool().unwrap_or(true);
            write_note_doc(life_dir, rel, content, append)
        }
        "write_file" => {
            let rel = args["path"].as_str().unwrap_or("");
            let content = args["content"].as_str().unwrap_or("");
            let append = args["append"].as_bool().unwrap_or(false);
            harness_write_file(cwd, life_dir, rel, content, append)
        }
        "edit_file" => {
            let rel = args["path"].as_str().unwrap_or("");
            let old_text = args["old_text"].as_str().unwrap_or("");
            let new_text = args["new_text"].as_str().unwrap_or("");
            harness_edit_file(cwd, life_dir, rel, old_text, new_text)
        }
        "read_file" => {
            let rel = args["path"].as_str().unwrap_or("").trim();
            let target = resolve_user_path(cwd, life_dir, rel)?;

            // Check if binary image (guard against reading multi-GB files into RAM)
            if let Ok(meta) = std::fs::metadata(&target)
                && meta.len() <= crate::vision::MAX_IMAGE_BYTES as u64
                && let Ok(bytes) = std::fs::read(&target)
                && crate::vision::detect_mime(&target, &bytes) != "application/octet-stream"
                && let Ok(img) = crate::vision::load_image_file(&target)
            {
                out_meta.viewed_image = Some(target.clone());
                return Ok(format!(
                    "Image: {} ({})",
                    target.display(),
                    img.summary_label()
                ));
            }

            let opts = ReadFileOptions {
                start_line: args["start_line"].as_u64().map(|n| n as usize),
                end_line: args["end_line"].as_u64().map(|n| n as usize),
                byte_offset: args["byte_offset"].as_u64().map(|n| n as usize),
                limit: args["limit"].as_u64().map(|n| n as usize),
            };

            harness_read_file(cwd, life_dir, rel, opts, max_read_bytes)
        }
        "glob_files" => {
            let pattern = args["pattern"].as_str().unwrap_or("*");
            let sub = args["path"].as_str();
            harness_glob(cwd, life_dir, pattern, sub, 100)
        }
        "grep_files" => {
            let query = args["query"].as_str().unwrap_or("");
            let sub = args["path"].as_str();
            harness_grep(cwd, life_dir, query, sub, 100)
        }
        "view_image" => {
            let rel = args["path"].as_str().unwrap_or("").trim();
            let target = resolve_user_path(cwd, life_dir, rel)?;
            let img = crate::vision::load_image_file(&target)?;
            out_meta.viewed_image = Some(target.clone());
            Ok(format!(
                "Image opened: {} ({})",
                target.display(),
                img.summary_label()
            ))
        }
        "change_dir" | "cd" => {
            let rel = args["path"].as_str().unwrap_or("").trim();
            let target = resolve_user_path(cwd, life_dir, rel)?;
            if !target.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("directory not found: {}", target.display()),
                ));
            }
            let canon = std::fs::canonicalize(&target).unwrap_or(target);
            out_meta.new_cwd = Some(canon.clone());
            Ok(format!("Current working directory: {}", canon.display()))
        }
        "list_files" => {
            let raw = args["path"]
                .as_str()
                .or_else(|| args["subdir"].as_str())
                .unwrap_or(".")
                .trim();
            let dir = resolve_user_path(cwd, life_dir, raw)?;
            let mut items = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    let name = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    items.push(if p.is_dir() { format!("{name}/") } else { name });
                }
            }
            items.sort();
            Ok(if items.is_empty() {
                "(empty)".into()
            } else {
                items.join("\n")
            })
        }
        "delete_file" => {
            let p = args["path"].as_str().unwrap_or("");
            harness_delete_file(cwd, life_dir, p)
        }
        "exec" => {
            let cmd_str = args["command"].as_str().unwrap_or("");
            run_exec_cmd(cwd, cmd_str, max_read_bytes)
        }
        "shell_start" => {
            let cmd_str = args["command"].as_str().unwrap_or("");
            let job_id = ShellJobManager::global().start_job(cwd, cmd_str)?;
            Ok(format!("Background job started: {job_id}"))
        }
        "shell_output" => {
            let job_id = args["job_id"].as_str().unwrap_or("");
            let cursor = args["cursor"].as_u64().unwrap_or(0) as usize;
            let (chunk, new_cursor, is_done) =
                ShellJobManager::global().read_output(job_id, cursor)?;
            let status = if is_done {
                "completed"
            } else {
                "running"
            };
            Ok(format!(
                "Status: {status}, new cursor: {new_cursor}\nOutput:\n{chunk}"
            ))
        }
        "shell_kill" => {
            let job_id = args["job_id"].as_str().unwrap_or("");
            ShellJobManager::global().kill_job(job_id)
        }
        "task_create" => {
            let sub = args["subject"].as_str().unwrap_or("");
            let desc = args["description"].as_str().unwrap_or("");
            let item = TaskManager::global().create_task(life_dir, sub, desc)?;
            Ok(format!("Task created #{}: {}", item.id, item.subject))
        }
        "task_list" => Ok(TaskManager::global().format_list()),
        "task_update" => {
            let id = args["id"].as_u64().unwrap_or(0) as usize;
            let st_str = args["status"].as_str().unwrap_or("");
            let st = TaskStatus::from_str(st_str).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "status must be pending, in_progress, completed, or blocked",
                )
            })?;
            let updated = TaskManager::global().update_task(life_dir, id, st)?;
            Ok(format!(
                "Task #{} updated: {}",
                updated.id,
                updated.status.as_str()
            ))
        }
        "ask_user" => {
            let q = args["question"].as_str().unwrap_or("");
            let options: Vec<String> = args["options"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let allow_free = args["allow_free_text"].as_bool().unwrap_or(true);
            Ok(format_ask_user(q, &options, allow_free))
        }
        "plan_mode" => {
            let action = args["action"].as_str().unwrap_or("");
            let summary = args["summary"].as_str().unwrap_or("");
            match action {
                "enter" => Ok(enter_plan_mode()),
                "exit" => Ok(exit_plan_mode(summary)),
                other => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "unknown plan_mode action '{other}' (expected 'enter' or 'exit')"
                    ),
                )),
            }
        }
        "web_search" => {
            let q = args["query"].as_str().unwrap_or("");
            let pref = args["provider"].as_str();
            let outcome = execute_search_cascade(
                q,
                5,
                pref,
                get_tavily_api_key().as_deref(),
                get_parallel_api_key().as_deref(),
                get_exa_api_key().as_deref(),
            )?;
            Ok(format_search_results(&outcome))
        }
        "alexandria_search" => {
            let q = args["query"].as_str().unwrap_or("");
            let key = get_firecrawl_api_key().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "FIRECRAWL_API_KEY is not set in [firecrawl] snarta.toml or environment",
                )
            })?;
            let caps = alexandria_discover(&key, q)?;
            if caps.is_empty() {
                return Ok("Alexandria returned no matching providers.".to_string());
            }
            let mut out = format!(
                "### Found {} Alexandria capabilities (free):\n\n",
                caps.len()
            );
            for (idx, c) in caps.iter().enumerate() {
                out.push_str(&format!(
                    "{}. **{}** (`{}`)\n   Price: {} credits | {}\n",
                    idx + 1,
                    c.provider,
                    c.address,
                    c.price_credits,
                    c.description
                ));
            }
            Ok(out)
        }
        "alexandria_scrape" => {
            let key = get_firecrawl_api_key().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "FIRECRAWL_API_KEY is not set in [firecrawl] snarta.toml or environment",
                )
            })?;
            let provider = args["provider"].as_str().unwrap_or("");
            let cap = args["capability"].as_str().unwrap_or("");
            let opts = args.get("options").cloned().unwrap_or(json!({}));
            let (content, cost) = alexandria_scrape(life_dir, &key, provider, cap, opts)?;
            Ok(format!("Credits charged: {cost}\nData:\n{content}"))
        }
        other => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("unknown tool '{other}'"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn tool_definitions_is_valid_json_array() {
        let defs = tool_definitions_json();
        assert!(defs.is_array());
        let arr = defs.as_array().unwrap();
        assert!(arr.len() >= 20);
        for item in arr {
            assert_eq!(item["type"].as_str(), Some("function"));
            assert!(item["function"]["name"].is_string());
            assert!(item["function"]["description"].is_string());
        }
    }

    #[core::prelude::v1::test]
    fn format_summary_for_common_tools() {
        assert_eq!(
            format_tool_summary("exec", r#"{"command":"cargo --version"}"#),
            "$ cargo --version"
        );
        assert_eq!(
            format_tool_summary("read_file", r#"{"path":"src/main.rs"}"#),
            "read: src/main.rs"
        );
        assert_eq!(
            format_tool_summary("write_file", r#"{"path":"test.txt","append":false}"#),
            "write: test.txt"
        );
        assert_eq!(
            format_tool_summary("write_file", r#"{"path":"test.txt","append":true}"#),
            "append: test.txt"
        );
    }
}
