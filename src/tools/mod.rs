pub mod fs;
pub mod plan;
pub mod registry;
pub mod search_local;
pub mod security;
pub mod shell;
pub mod web_scrape;
pub mod web_search;

#[allow(unused_imports)]
pub use registry::{
    execute_tool_call, format_tool_summary, sanitize_tool_arguments_for_server,
    tool_definitions_json,
};

#[allow(unused_imports)]
pub use fs::{
    ReadFileOptions, ReadLedger, harness_delete_file, harness_edit_file, harness_read_file,
    harness_write_file, remember_fact, write_note_doc,
};
#[allow(unused_imports)]
pub use plan::{
    PLAN_MODE_ACTIVE, TaskItem, TaskManager, TaskStatus, enter_plan_mode, exit_plan_mode,
    format_ask_user, is_plan_mode, set_plan_mode,
};
#[allow(unused_imports)]
pub use search_local::{harness_glob, harness_grep};
#[allow(unused_imports)]
pub use security::{
    DANGEROUS_CMD_PATTERNS, PROTECTED_DELETE_DIRS, is_dangerous_command, is_protected_path,
    resolve_inside_life, resolve_user_path,
};
#[allow(unused_imports)]
pub use shell::{
    EXEC_TIMEOUT_SECS, MAX_HEAD_LINES, MAX_TAIL_LINES, ShellJobManager, run_exec_cmd,
    sanitize_shell_command, truncate_head_tail,
};
#[allow(unused_imports)]
pub use web_scrape::{
    AlexandriaCapability, AlexandriaTracker, alexandria_discover, alexandria_scrape,
    scrape_url_markdown,
};
#[allow(unused_imports)]
pub use web_search::{
    SearchOutcome, SearchResultItem, execute_search_cascade, format_search_results,
};

use crate::config::{FirecrawlConfig, SearchConfig};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Default, Clone)]
pub struct ExternalServiceKeys {
    pub tavily: Option<String>,
    pub parallel: Option<String>,
    pub exa: Option<String>,
    pub firecrawl: Option<String>,
}

fn external_service_keys() -> &'static Mutex<ExternalServiceKeys> {
    static KEYS: OnceLock<Mutex<ExternalServiceKeys>> = OnceLock::new();
    KEYS.get_or_init(|| Mutex::new(ExternalServiceKeys::default()))
}

pub fn sync_config_keys(search: &SearchConfig, firecrawl: &FirecrawlConfig) {
    if let Ok(mut g) = external_service_keys().lock() {
        if !search.tavily_api_key.trim().is_empty() {
            g.tavily = Some(search.tavily_api_key.trim().to_string());
        }
        if !search.parallel_api_key.trim().is_empty() {
            g.parallel = Some(search.parallel_api_key.trim().to_string());
        }
        if !search.exa_api_key.trim().is_empty() {
            g.exa = Some(search.exa_api_key.trim().to_string());
        }
        if !firecrawl.api_key.trim().is_empty() {
            g.firecrawl = Some(firecrawl.api_key.trim().to_string());
        }
    }
}

pub fn get_tavily_api_key() -> Option<String> {
    external_service_keys()
        .lock()
        .ok()
        .and_then(|k| k.tavily.clone())
        .or_else(|| std::env::var("TAVILY_API_KEY").ok())
}

pub fn get_parallel_api_key() -> Option<String> {
    external_service_keys()
        .lock()
        .ok()
        .and_then(|k| k.parallel.clone())
        .or_else(|| std::env::var("PARALLEL_API_KEY").ok())
}

pub fn get_exa_api_key() -> Option<String> {
    external_service_keys()
        .lock()
        .ok()
        .and_then(|k| k.exa.clone())
        .or_else(|| std::env::var("EXA_API_KEY").ok())
}

pub fn get_firecrawl_api_key() -> Option<String> {
    external_service_keys()
        .lock()
        .ok()
        .and_then(|k| k.firecrawl.clone())
        .or_else(|| std::env::var("FIRECRAWL_API_KEY").ok())
}

pub const MAX_IDENTICAL_TOOL_CALLS: usize = 3;

#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub output: String,
    pub mem_added: Option<String>,
    pub new_cwd: Option<PathBuf>,
    pub viewed_image: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct ToolLoopGuard {
    pub steps: u32,
    pub last_sig: Option<(String, String)>,
    pub repeat_count: usize,
}

impl ToolLoopGuard {
    pub fn allow_call(&mut self, name: &str, args: &str, max_steps: u32) -> Result<(), String> {
        if self.steps >= max_steps {
            return Err(format!("Tool step limit ({max_steps}) reached."));
        }
        self.steps += 1;
        let sig = (name.to_string(), args.trim().to_string());
        if self.last_sig.as_ref() == Some(&sig) {
            self.repeat_count += 1;
            if self.repeat_count >= MAX_IDENTICAL_TOOL_CALLS {
                return Err(format!(
                    "Repeated tool '{name}' {MAX_IDENTICAL_TOOL_CALLS} times."
                ));
            }
        } else {
            self.last_sig = Some(sig);
            self.repeat_count = 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
