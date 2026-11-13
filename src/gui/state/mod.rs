mod agent;

pub fn measure_vram_snapshot(
    srv_pid: Option<u32>,
    spill_threshold_mib: u64,
    reserve_mib: u64,
) -> (u64, u64, u64) {
    let gpus = gpu::detect_gpus().unwrap_or_default();
    let total = gpus
        .iter()
        .find(|g| g.vendor_id == gpu::VENDOR_AMD)
        .or_else(|| gpus.first())
        .map(|g| g.dedicated_vram_mib())
        .or_else(|| gpu::cached_primary_gpu().map(|g| g.dedicated_vram_mib()))
        .unwrap_or(16_384);

    let free = server::measure_free_vram_mib();
    let used = total.saturating_sub(free);
    let spill = if let Some(pid) = srv_pid {
        crate::sysmon::procs::check_server_vram_spill(pid, spill_threshold_mib, free, reserve_mib)
            .shared_mib
    } else {
        0
    };
    (total, used, spill)
}

#[cfg(test)]
mod tests;

use crate::chat::engine::ActiveAgentStream;
use crate::chat::session::{ToolTrace, TurnStep};
use crate::chat::workspace::Workspace;
use crate::config::{Config, ModelSettings};
use crate::gpu;
use crate::log::Logger;
use crate::paths::AppPaths;
use crate::server;
use gpui::*;
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveModal {
    Prepare,
    Models,
    Personas,
}

pub use crate::server::{ServerController, ServerStatus};

#[derive(Debug, Clone)]
pub struct VramTelemetry {
    pub used_mib: u64,
    pub total_mib: u64,
    pub spill_mib: u64,
    pub tps: f64,
    pub active_ctx: u32,
    #[allow(dead_code)]
    pub fits_in_vram: bool,
}

pub struct AppState {
    pub paths: AppPaths,
    pub cfg: Config,
    #[allow(dead_code)]
    pub logger: Logger,
    pub workspace: Workspace,
    pub vram: VramTelemetry,
    pub active_stream: Option<ActiveAgentStream>,
    pub streaming_content: String,
    pub streaming_reasoning: String,
    pub streaming_tools: Vec<ToolTrace>,
    pub streaming_steps: Vec<TurnStep>,
    pub collapsed_reasoning: HashSet<String>,
    pub is_generating: bool,
    pub attached_images: Vec<PathBuf>,
    pub sidebar_open: bool,
    pub settings_open: bool,
    pub active_modal: Option<ActiveModal>,
    #[allow(dead_code)]
    pub fit_chosen_ctx: Option<u32>,
    pub prepare_candidates: Vec<(crate::sysmon::optimize::PrepareCandidate, bool)>,
    pub status_notice: Option<String>,
    pub server: ServerController,
    pub server_status: ServerStatus,
    pub needs_scroll_to_bottom: bool,
    pub draft_tokens: Option<usize>,
    pub last_gen_tokens: Option<u32>,
    pub last_config_mtime: Option<std::time::SystemTime>,
}

impl Deref for AppState {
    type Target = Workspace;

    fn deref(&self) -> &Self::Target {
        &self.workspace
    }
}

impl DerefMut for AppState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.workspace
    }
}

impl AppState {
    pub fn new(paths: AppPaths, mut cfg: Config, logger: Logger) -> Self {
        let workspace = Workspace::new(&paths, &mut cfg);
        let server = ServerController::new(paths.clone(), cfg.clone(), logger.clone());
        let server_status = server.status().clone();

        let initial_ctx = workspace
            .active_model_meta
            .as_ref()
            .map(|m| cfg.model_settings(&m.file_name).ctx)
            .unwrap_or(cfg.model_defaults.ctx);

        let last_config_mtime = std::fs::metadata(paths.config_file())
            .and_then(|m| m.modified())
            .ok();

        let mut state = Self {
            paths,
            cfg,
            logger,
            workspace,
            vram: VramTelemetry {
                used_mib: 0,
                total_mib: 0,
                spill_mib: 0,
                tps: 0.0,
                active_ctx: initial_ctx,
                fits_in_vram: true,
            },
            active_stream: None,
            streaming_content: String::new(),
            streaming_reasoning: String::new(),
            streaming_tools: Vec::new(),
            streaming_steps: Vec::new(),
            collapsed_reasoning: HashSet::new(),
            is_generating: false,
            attached_images: Vec::new(),
            sidebar_open: true,
            settings_open: false,
            active_modal: None,
            fit_chosen_ctx: None,
            prepare_candidates: Vec::new(),
            status_notice: None,
            server,
            server_status,
            needs_scroll_to_bottom: true,
            draft_tokens: Some(0),
            last_gen_tokens: None,
            last_config_mtime,
        };
        state.refresh_vram();
        state.recalculate_used_ctx();
        state
    }

    pub(crate) fn active_server_ctx(&self) -> Option<u32> {
        match self.server_status {
            ServerStatus::Running { ctx, .. } if ctx > 0 => Some(ctx),
            _ => None,
        }
    }

    /// Returns the effective `ModelSettings` for a given model file, honoring
    /// user-configured defaults when no per-model override exists.
    pub fn settings_for(&self, model_file: &str) -> ModelSettings {
        self.workspace.settings_for(&self.cfg, model_file)
    }

    /// Returns the effective `ModelSettings` for the active model (or defaults
    /// if no model is selected).
    pub fn settings_for_active(&self) -> ModelSettings {
        self.workspace.settings_for_active(&self.cfg)
    }

    /// Returns the endpoint the chat client should talk to. When a server is
    /// running (ours or adopted via lock file), use its reported host/port;
    /// otherwise fall back to the configured values.
    pub(crate) fn current_endpoint(&self) -> (String, u16) {
        self.server.endpoint()
    }

    pub fn refresh_vram(&mut self) {
        let srv_pid = match self.server_status {
            ServerStatus::Running { pid, .. } => Some(pid),
            _ => None,
        };
        let (total, used, spill) = measure_vram_snapshot(
            srv_pid,
            self.cfg.vram.shared_spill_threshold_mib,
            self.cfg.vram.reserve_mib,
        );
        self.vram.total_mib = total;
        self.vram.used_mib = used;
        self.vram.spill_mib = spill;
    }

    pub fn recalculate_used_ctx(&mut self) {
        let active_ctx = self.active_server_ctx();
        self.workspace
            .recalculate_used_ctx(&self.paths, &self.cfg, active_ctx);
        if let Some(ctx) = active_ctx {
            self.vram.active_ctx = ctx;
        } else {
            let settings = self.settings_for_active();
            if settings.ctx > 0 {
                self.vram.active_ctx = settings.ctx;
            }
        }
    }

    #[allow(dead_code)]
    pub fn reload_sessions(&mut self) {
        self.workspace.reload_sessions(&self.paths);
    }

    pub fn create_new_session(&mut self) {
        // Cancel any in-flight generation before switching sessions so that
        // streaming state does not leak into the new session.
        if self.is_generating {
            self.cancel_generation();
        }
        let active_ctx = self.active_server_ctx();
        if self
            .workspace
            .create_new_session(&self.paths, &self.cfg, active_ctx)
            .is_ok()
        {
            self.collapsed_reasoning.clear();
            self.needs_scroll_to_bottom = true;
        }
    }

    pub fn switch_session(&mut self, path: &Path) {
        if self.is_generating {
            self.cancel_generation();
        }
        let active_ctx = self.active_server_ctx();
        if self
            .workspace
            .switch_session(&self.paths, &self.cfg, path, active_ctx)
            .is_ok()
        {
            self.collapsed_reasoning.clear();
            self.needs_scroll_to_bottom = true;
        }
    }

    pub fn delete_session(&mut self, path: &Path) {
        if self.workspace.session.path == path && self.is_generating {
            self.cancel_generation();
        }
        let active_ctx = self.active_server_ctx();
        match self
            .workspace
            .delete_session(&self.paths, &self.cfg, path, active_ctx)
        {
            Ok(was_active) => {
                if was_active {
                    self.needs_scroll_to_bottom = true;
                }
            }
            Err(e) => {
                self.status_notice = Some(format!("Failed to delete session: {e}"));
            }
        }
    }

    pub fn switch_persona(&mut self, persona_name: &str) {
        let active_ctx = self.active_server_ctx();
        self.workspace
            .switch_persona(&self.paths, &mut self.cfg, persona_name, active_ctx);
    }

    pub fn switch_model(&mut self, model_file: &str) {
        let active_ctx = self.active_server_ctx();
        self.workspace
            .switch_model(&self.paths, &mut self.cfg, model_file, active_ctx);
    }

    pub fn rescan_models(&mut self) {
        let active_ctx = self.active_server_ctx();
        self.workspace
            .rescan_models(&self.paths, &mut self.cfg, active_ctx);
    }

    pub fn rescan_prepare(&mut self) {
        let srv_pid = match self.server_status {
            ServerStatus::Running { pid, .. } => Some(pid),
            _ => None,
        };
        let plan = crate::sysmon::optimize::scan_prepare_candidates(
            &self.cfg.prepare,
            srv_pid,
            &self.cfg.server.host,
            self.cfg.server.port,
        );
        self.prepare_candidates = plan.candidates.into_iter().map(|h| (h, true)).collect();
    }

    pub fn open_modal(&mut self, modal: ActiveModal) {
        if modal == ActiveModal::Models {
            self.rescan_models();
        } else if modal == ActiveModal::Prepare {
            self.rescan_prepare();
        }
        self.active_modal = Some(modal);
    }

    pub fn toggle_reasoning_collapsed(&mut self, key: &str) {
        if !self.collapsed_reasoning.remove(key) {
            self.collapsed_reasoning.insert(key.to_string());
        }
    }

    pub fn is_reasoning_collapsed(&self, key: &str) -> bool {
        self.collapsed_reasoning.contains(key)
    }

    pub fn start_server_for_model(&mut self, model_name: &str, ctx: u32) {
        // Cancel any in-flight generation before the server goes away.
        self.cancel_generation();

        let meta = match self
            .available_models
            .iter()
            .find(|m| m.file_name == model_name)
            .cloned()
        {
            Some(m) => m,
            None => {
                self.server
                    .set_error(format!("Model '{model_name}' not found"));
                self.server_status = self.server.status().clone();
                return;
            }
        };

        let settings = self.settings_for(model_name);
        self.server.start(meta, settings, ctx);
        self.server_status = self.server.status().clone();
    }

    pub fn stop_server(&mut self) {
        if self.is_generating {
            self.cancel_generation();
        }
        self.server.stop();
        self.server_status = self.server.status().clone();
    }

    pub fn refresh_server_status(&mut self) {
        if self.server_status != *self.server.status() {
            self.server.set_status(self.server_status.clone());
        }
        self.server.update_status();
        self.server_status = self.server.status().clone();
    }

    pub fn save_config(&mut self) -> std::io::Result<()> {
        let cfg_path = self.paths.config_file();
        self.cfg.save(&cfg_path)?;
        self.last_config_mtime = std::fs::metadata(&cfg_path)
            .and_then(|m| m.modified())
            .ok();
        self.server.update_config(self.cfg.clone());
        crate::tools::sync_config_keys(&self.cfg.search, &self.cfg.firecrawl);
        Ok(())
    }

    /// Applies an updated config, syncs server/tool state, and restarts `llama-server`
    /// if it is running and launch parameters or server endpoint changed.
    /// Returns `true` if a server restart was triggered.
    pub fn apply_updated_config(&mut self, new_cfg: Config) -> bool {
        let old_cfg = std::mem::replace(&mut self.cfg, new_cfg);
        self.server.update_config(self.cfg.clone());
        crate::tools::sync_config_keys(&self.cfg.search, &self.cfg.firecrawl);
        self.recalculate_used_ctx();

        let running_model = match &self.server_status {
            ServerStatus::Running { model, .. } | ServerStatus::Starting { model, .. } => {
                Some(model.clone())
            }
            _ => None,
        };

        if let Some(model) = running_model {
            let old_settings = old_cfg.model_settings(&model);
            let new_settings = self.cfg.model_settings(&model);
            let endpoint_changed = old_cfg.server.host != self.cfg.server.host
                || old_cfg.server.port != self.cfg.server.port;
            if endpoint_changed || crate::server::args::launch_params_changed(&old_settings, &new_settings) {
                self.start_server_for_model(&model, new_settings.ctx);
                return true;
            }
        }
        false
    }

    /// Reloads `snarta.toml` from disk if its modification timestamp changed.
    /// Returns `true` if the configuration changed and was applied.
    pub fn reload_config_if_changed(&mut self) -> bool {
        let cfg_path = self.paths.config_file();
        let Ok(meta) = std::fs::metadata(&cfg_path) else {
            return false;
        };
        let Ok(mtime) = meta.modified() else {
            return false;
        };
        if self.last_config_mtime == Some(mtime) {
            return false;
        }
        self.last_config_mtime = Some(mtime);
        let Ok((new_cfg, _)) = Config::load_or_create(&cfg_path) else {
            return false;
        };
        if new_cfg == self.cfg {
            return false;
        }
        self.apply_updated_config(new_cfg);
        true
    }
}
