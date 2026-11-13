use crate::chat::client;
use crate::chat::session::Session;
use crate::persona::Persona;
use serde_json::{Value, json};
use std::{fs, io, path::Path};

#[allow(dead_code)]
pub const KEEP_RECENT_TURNS_ON_COMPACT: usize = 2;

#[derive(Debug, Clone)]
pub struct BuiltSystemPrompt {
    pub full_text: String,
    #[allow(dead_code)]
    pub memory_tokens: u32,
    #[allow(dead_code)]
    pub memory_over_limit: bool,
}

pub fn estimate_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let a = text.chars().filter(char::is_ascii).count();
    let tok = (a as f64 / 3.5) + (text.chars().count().saturating_sub(a) as f64 / 1.8);
    (tok.ceil() as u32).max(1)
}

pub fn format_calendar_datetime(
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    weekday_idx: u32,
) -> String {
    let weekday = match weekday_idx {
        0 => "Sunday",
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
        _ => "None",
    };
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}, {weekday}")
}

#[allow(dead_code)]
pub fn civil_datetime_from_epoch(s: i64) -> String {
    let days = (s / 86400) + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let (mp, d) = (
        (5 * doy + 2) / 153,
        doy - (153 * ((5 * doy + 2) / 153) + 2) / 5 + 1,
    );
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = if m <= 2 { y + 1 } else { y };
    let secs_of_day = s.rem_euclid(86400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let wday = (s / 86400 + 4).rem_euclid(7);
    format_calendar_datetime(yr, m, d, hour as u32, minute as u32, wday as u32)
}

#[cfg(windows)]
pub fn current_local_datetime() -> String {
    use std::mem::MaybeUninit;
    use windows_sys::Win32::Foundation::SYSTEMTIME;

    unsafe extern "system" {
        fn GetLocalTime(lpSystemTime: *mut SYSTEMTIME);
    }

    unsafe {
        let mut st = MaybeUninit::<SYSTEMTIME>::uninit();
        GetLocalTime(st.as_mut_ptr());
        let st = st.assume_init();
        format_calendar_datetime(
            st.wYear as i64,
            st.wMonth as u32,
            st.wDay as u32,
            st.wHour as u32,
            st.wMinute as u32,
            st.wDayOfWeek as u32,
        )
    }
}

#[cfg(not(windows))]
pub fn current_local_datetime() -> String {
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const std::ffi::c_char,
    }

    unsafe extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }

    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);

    unsafe {
        let mut tm = std::mem::MaybeUninit::<Tm>::uninit();
        if !localtime_r(&s, tm.as_mut_ptr()).is_null() {
            let tm = tm.assume_init();
            format_calendar_datetime(
                (tm.tm_year + 1900) as i64,
                (tm.tm_mon + 1) as u32,
                tm.tm_mday as u32,
                tm.tm_hour as u32,
                tm.tm_min as u32,
                tm.tm_wday as u32,
            )
        } else {
            civil_datetime_from_epoch(s)
        }
    }
}

pub fn build_system_prompt(
    persona: &Persona,
    memory_file: &Path,
    cwd: &Path,
    limit: u32,
) -> BuiltSystemPrompt {
    let raw = fs::read_to_string(memory_file).unwrap_or_default();
    let trimmed = raw.trim();
    let now_str = current_local_datetime();
    let cwd_line = format!(
        "\n\n# Environment & tools\n\
        - Date/time: {}\n\
        - Host environment: Native Windows 11 desktop machine (local execution). You are NOT in a sandbox, container, or isolated VM. You have full native access to the user's local filesystem, local network, and PowerShell via your tools.\n\
        - Never claim that you are running in a sandbox or restricted cloud environment.\n\
        - Current working directory (cwd): {}\n\
        - All file and shell tools (`read_file`, `write_file`, `edit_file`, `glob_files`, `grep_files`, `list_files`, `exec`) operate directly on the local machine from `cwd` (or accept any full Windows path like `C:\\...`). Use `change_dir` to switch `cwd` if needed.\n\
        - Inspect files with dedicated tools:\n\
          * `glob_files` — glob file search (e.g. `**/*.rs`), returns paths, line counts, sizes.\n\
          * `grep_files` — fast text search via ripgrep.\n\
          * `read_file` — read specific lines.\n\
        - On Windows `exec` runs directly inside native PowerShell (`powershell.exe`). Do not wrap commands with `powershell -Command ...`; use `;` instead of `&&`.",
        now_str,
        cwd.display()
    );
    let (full_text, memory_tokens) = if trimmed.is_empty() {
        (format!("{}{cwd_line}", persona.system_prompt), 0)
    } else {
        (
            format!(
                "{}{cwd_line}\n\n# Long term memory (life/memory.md)\n{trimmed}",
                persona.system_prompt
            ),
            estimate_tokens(trimmed),
        )
    };
    BuiltSystemPrompt {
        full_text,
        memory_tokens,
        memory_over_limit: memory_tokens > limit,
    }
}

pub fn build_context_messages(
    session: &Session,
    system_prompt: &str,
    max_ctx_tokens: u32,
    max_reply_tokens: u32,
    preserve_reasoning: bool,
) -> (Vec<(String, String, Vec<String>)>, u32) {
    let sys_tok = estimate_tokens(system_prompt) + 16;
    let budget = max_ctx_tokens
        .saturating_sub(max_reply_tokens)
        .saturating_sub(sys_tok)
        .max(256);
    let (start_idx, cp) = match &session.last_compact {
        Some((cov, sum)) if *cov <= session.turns.len() => {
            (*cov, Some(format!("[Summary of the previous dialogue]\n{sum}")))
        }
        _ => (0, None),
    };
    let cp_tok = cp.as_deref().map_or(0, estimate_tokens);
    let avail = budget.saturating_sub(cp_tok);

    let mut rev = Vec::new();
    let mut used = 0u32;
    for turn in session.turns[start_idx..].iter().rev() {
        let mut text = turn.content.clone();
        if !turn.tool_traces.is_empty() {
            let mut tr_summary = String::from("Completed tool actions in this turn:\n");
            for tr in &turn.tool_traces {
                let s = crate::tools::format_tool_summary(&tr.name, &tr.arguments);
                let out_one = tr.output.lines().next().unwrap_or("done").trim();
                let out_short: String = out_one.chars().take(120).collect();
                tr_summary.push_str(&format!("- {s} => {out_short}\n"));
            }
            tr_summary.push('\n');
            text = format!("{tr_summary}{text}");
        }
        if let (Some(r), true) = (&turn.reasoning, preserve_reasoning) {
            let is_gemma = session
                .model
                .as_deref()
                .is_some_and(|m| m.to_lowercase().contains("gemma"));
            if is_gemma {
                text = format!("<|channel>thought\n{r}\n<channel|>\n{text}");
            } else {
                text = format!("<think>\n{r}\n</think>\n{text}");
            }
        }
        let imgs = turn.effective_images();
        let t_tok = estimate_tokens(&text) + 8 + (800 * imgs.len() as u32);
        if !rev.is_empty() && used + t_tok > avail {
            break;
        }
        used += t_tok;
        rev.push((turn.role.clone(), text, imgs));
    }
    rev.reverse();

    let sys_full = cp.map_or_else(
        || system_prompt.to_string(),
        |c| format!("{system_prompt}\n\n{c}"),
    );
    let mut msgs = vec![("system".to_string(), sys_full, Vec::new())];
    msgs.extend(rev);
    (msgs, sys_tok + cp_tok + used)
}

pub fn pairs_to_json_messages(pairs: &[(String, String, Vec<String>)]) -> Vec<Value> {
    pairs
        .iter()
        .map(|(r, c, imgs)| {
            let loaded: Vec<_> = imgs
                .iter()
                .filter_map(|p| crate::vision::load_image_file(Path::new(p)).ok())
                .collect();
            if !loaded.is_empty() {
                let prompt_text = if c.trim().is_empty() {
                    "Describe this image.".to_string()
                } else {
                    c.clone()
                };
                let mut content = vec![json!({ "type": "text", "text": prompt_text })];
                for i in loaded {
                    content.push(json!({ "type": "image_url", "image_url": { "url": i.data_url() } }));
                }
                json!({ "role": r, "content": content })
            } else {
                json!({ "role": r, "content": c })
            }
        })
        .collect()
}

#[allow(dead_code)]
pub fn should_auto_compact(used_tokens: u32, max_ctx: u32, threshold_pct: u8) -> bool {
    max_ctx > 0 && (used_tokens as u64 * 100) / (max_ctx as u64) >= u64::from(threshold_pct)
}

#[allow(dead_code)]
pub fn run_session_compact(session: &mut Session, host: &str, port: u16) -> io::Result<usize> {
    let (start_idx, total) = (
        session.last_compact.as_ref().map_or(0, |(c, _)| *c),
        session.turns.len(),
    );
    let end_idx = total.saturating_sub(KEEP_RECENT_TURNS_ON_COMPACT);
    if total <= start_idx + KEEP_RECENT_TURNS_ON_COMPACT {
        return Err(io::Error::other("Not enough lines for /compact"));
    }
    let mut tr = session
        .last_compact
        .as_ref()
        .map_or_else(String::new, |(_, p)| format!("Previous resume:\n{p}\n\n"));
    for t in &session.turns[start_idx..end_idx] {
        tr.push_str(&format!("[{}]: {}\n\n", t.role, t.content));
    }
    let summary = client::summarize_turns_sync(host, port, &tr)?;
    session.append_compact(end_idx, summary)?;
    Ok(end_idx)
}
