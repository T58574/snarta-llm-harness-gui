pub mod args;
pub mod controller;
pub mod fit;
pub mod launch;

pub use controller::{ServerController, ServerStatus};

use crate::config::{Config, ModelSettings};
use crate::gguf::GgufMeta;
use crate::gpu;
use crate::log::Logger;
use crate::paths::AppPaths;
use crate::runtime::install;
use crate::sysmon::procs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockState {
    pub snarta_pid: u32,
    pub server_pid: u32,
    pub host: String,
    pub port: u16,
    pub model: String,
    pub build_id: String,
    #[serde(default)]
    pub ctx: u32,
}

impl LockState {
    pub fn read_active(lock_path: &Path) -> Option<Self> {
        let s: Self = toml::from_str(&fs::read_to_string(lock_path).ok()?).ok()?;
        if procs::is_pid_alive(s.server_pid) && launch::is_server_healthy(&s.host, s.port) {
            return Some(s);
        }
        let _ = fs::remove_file(lock_path);
        None
    }

    pub fn write(&self, lock_path: &Path) -> io::Result<()> {
        let body = toml::to_string_pretty(self).map_err(|e| io::Error::other(e.to_string()))?;
        fs::write(lock_path, body)
    }
}

pub fn measure_free_vram_mib() -> u64 {
    let g = gpu::cached_primary_gpu();
    let total = g.as_ref().map_or(16_368, |x| x.dedicated_vram_mib());
    let used = procs::total_measured_vram_used_mib_filtered(
        g.as_ref().and_then(|x| x.luid_str.as_deref()),
    );
    total.saturating_sub(used)
}

pub fn resolve_active_server_exe(paths: &AppPaths, cfg: &Config) -> io::Result<(String, PathBuf)> {
    let builds = install::list_installed(&paths.runtime_dir())?;
    if builds.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no llama.cpp runtime installed",
        ));
    }
    if let Some(active) = cfg.runtime.active_build.as_deref()
        && let Some(b) = builds.iter().find(|b| b.build_id == active)
    {
        return Ok((b.build_id.clone(), b.server_exe.clone()));
    }
    Ok((builds[0].build_id.clone(), builds[0].server_exe.clone()))
}

fn spawn_and_wait(
    exe: &Path,
    cmd_args: &[String],
    paths: &AppPaths,
    cfg: &Config,
) -> io::Result<launch::ManagedServer> {
    let log = paths.llama_server_log_file();
    let mut srv = launch::ManagedServer::spawn(exe, cmd_args, &log, cfg.limits.log_max_bytes)?;
    let s = &cfg.server;
    if let Err(e) = launch::wait_until_healthy(&mut srv, &s.host, s.port, s.health_timeout_secs) {
        srv.stop();
        return Err(e);
    }
    Ok(srv)
}

pub fn prompt_cache_file(paths: &AppPaths, model_name: &str) -> PathBuf {
    let dir = paths.prompt_cache_dir();
    let _ = fs::create_dir_all(&dir);
    let clean: String = model_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join(format!("{clean}.prompt.cache"))
}

pub fn start_with_auto_rollback(
    paths: &AppPaths,
    cfg: &mut Config,
    logger: &Logger,
    meta: &GgufMeta,
    settings: &ModelSettings,
    ctx: u32,
) -> io::Result<launch::ManagedServer> {
    let (build_id, exe) = resolve_active_server_exe(paths, cfg)?;
    let pcache = settings
        .prompt_cache
        .then(|| prompt_cache_file(paths, &meta.file_name));
    let cmd_args = args::build_server_args(
        &meta.path,
        &cfg.server,
        settings,
        Some(meta),
        Some(ctx),
        pcache.as_deref(),
    );
    let cmd_str = args::format_cmd(&exe, &cmd_args);
    let launch_params = args::format_launch_params(settings, ctx);
    let _ = logger.info(&format!("starting llama-server ({build_id}): {cmd_str}"));
    let _ = logger.info(&format!("llama-server launch params: {launch_params}"));

    match spawn_and_wait(&exe, &cmd_args, paths, cfg) {
        Ok(srv) => Ok(srv),
        Err(e) => {
            if let Some(last) = cfg.runtime.last_good_build.clone()
                && last != build_id
            {
                let _ = logger.error(&format!(
                    "build {build_id} failed ({e}); rollback to {last}"
                ));
                cfg.runtime.active_build = Some(last.clone());
                cfg.save(&paths.config_file())?;
                let (fb_build, fb_exe) = resolve_active_server_exe(paths, cfg)?;
                let fb_cmd = args::format_cmd(&fb_exe, &cmd_args);
                let _ = logger.info(&format!(
                    "starting fallback llama-server ({fb_build}): {fb_cmd}"
                ));
                return spawn_and_wait(&fb_exe, &cmd_args, paths, cfg);
            }
            Err(e)
        }
    }
}
