pub mod args;
pub mod controller;
pub mod fit;
pub mod launch;

pub use controller::{ServerController, ServerStatus, stop_server_process};

use crate::config::{Config, ModelSettings};
use crate::gguf::{self, GgufMeta};
use crate::gpu;
use crate::log::Logger;
use crate::paths::AppPaths;
use crate::runtime::install;
use crate::sysmon::procs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io, thread};

pub const BENCH_CONTEXTS: &[u32] = &[8192, 16384, 24576, 32768];
pub const BENCH_TOKENS: u32 = 64;

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

pub fn resolve_target_model(
    paths: &AppPaths,
    cfg: &Config,
    arg: Option<&str>,
) -> io::Result<GgufMeta> {
    let models = gguf::scan_models_dir(&paths.models_dir())?;
    if models.is_empty() {
        let msg = format!("no .gguf models found in {}", paths.models_dir().display());
        return Err(io::Error::new(io::ErrorKind::NotFound, msg));
    }
    let Some(q) = arg.or(cfg.active_model.as_deref()) else {
        return Ok(models.into_iter().next().unwrap());
    };
    models
        .into_iter()
        .find(|m| m.file_name == q || m.file_name.contains(q))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("model '{q}' not found")))
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

pub fn settings_for_model(cfg: &Config, model_name: &str) -> ModelSettings {
    cfg.model_settings(model_name)
}

pub fn cmd_fit(paths: &AppPaths, cfg: &Config, model_arg: Option<&str>) -> io::Result<()> {
    let free_vram = measure_free_vram_mib();
    let models = match model_arg {
        Some(q) => vec![resolve_target_model(paths, cfg, Some(q))?],
        None => gguf::scan_models_dir(&paths.models_dir())?,
    };
    if models.is_empty() {
        println!("No GGUF models in {}", paths.models_dir().display());
        return Ok(());
    }
    println!(
        "Free VRAM: {free_vram} MiB (reserve: {} MiB)",
        cfg.vram.reserve_mib
    );
    for m in models {
        let s = settings_for_model(cfg, &m.file_name);
        let e = fit::calculate_fit(&m, &s, &cfg.vram, free_vram);
        println!("{}", e.summary_line(&m));
    }
    Ok(())
}

pub fn cmd_serve(
    paths: &AppPaths,
    cfg: &mut Config,
    logger: &Logger,
    model_arg: Option<&str>,
) -> io::Result<()> {
    if let Some(ex) = LockState::read_active(&paths.lock_file()) {
        let (pid, h, p, m) = (ex.server_pid, &ex.host, ex.port, &ex.model);
        println!("Already running: PID {pid} on {h}:{p} ({m})");
        return Ok(());
    }
    let meta = resolve_target_model(paths, cfg, model_arg)?;
    let settings = settings_for_model(cfg, &meta.file_name);
    let e = fit::calculate_fit(&meta, &settings, &cfg.vram, measure_free_vram_mib());
    let (fv, wm, cc, km, cm, tm) = (
        e.free_vram_mib,
        e.weights_mib,
        e.chosen_ctx,
        e.kv_mib_at_chosen_ctx,
        e.compute_mib,
        e.total_mib_at_chosen_ctx,
    );
    println!("Fit: free={fv}M, weights={wm}M, KV@{cc}={km}M, compute={cm}M, total={tm}M");

    let mut srv = start_with_auto_rollback(paths, cfg, logger, &meta, &settings, e.chosen_ctx)?;
    let (build_id, _) = resolve_active_server_exe(paths, cfg)?;
    let (host, port) = (cfg.server.host.clone(), cfg.server.port);
    LockState {
        snarta_pid: std::process::id(),
        server_pid: srv.pid,
        host: host.clone(),
        port,
        model: meta.file_name.clone(),
        build_id,
        ctx: e.chosen_ctx,
    }
    .write(&paths.lock_file())?;

    println!(
        "llama-server ready on {host}:{port} (PID {}). Ctrl+C to stop.",
        srv.pid
    );
    while srv.try_wait()?.is_none() {
        let (thr, res) = (cfg.vram.shared_spill_threshold_mib, cfg.vram.reserve_mib);
        let sp = procs::check_server_vram_spill(srv.pid, thr, measure_free_vram_mib(), res);
        if sp.spilled {
            eprintln!(
                "[VRAM SPILL] shared={}M (> {}M). Speed drops ~2x!",
                sp.shared_mib, sp.threshold_mib
            );
        }
        thread::sleep(Duration::from_secs(2));
    }
    let _ = fs::remove_file(paths.lock_file());
    Ok(())
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
                println!("Build {build_id} failed ({e}), rolling back to {last}...");
                cfg.runtime.active_build = Some(last.clone());
                cfg.save(&paths.config_file())?;
                let (fb_build, fb_exe) = resolve_active_server_exe(paths, cfg)?;
                let fb_cmd = args::format_cmd(&fb_exe, &cmd_args);
                let _ = logger.info(&format!("starting fallback llama-server ({fb_build}): {fb_cmd}"));
                return spawn_and_wait(&fb_exe, &cmd_args, paths, cfg);
            }
            Err(e)
        }
    }
}

pub fn cmd_stop(paths: &AppPaths) -> io::Result<()> {
    let lock_path = paths.lock_file();
    let Some(state) = LockState::read_active(&lock_path) else {
        println!("No active llama-server found.");
        return Ok(());
    };
    stop_server_process(paths, Some(state.server_pid));
    println!("Stopped llama-server (PID {}).", state.server_pid);
    Ok(())
}

pub fn cmd_bench(
    paths: &AppPaths,
    cfg: &mut Config,
    logger: &Logger,
    model_arg: Option<&str>,
) -> io::Result<()> {
    let meta = resolve_target_model(paths, cfg, model_arg)?;
    let settings = settings_for_model(cfg, &meta.file_name);
    let (_, exe) = resolve_active_server_exe(paths, cfg)?;

    println!(
        "Benchmarking {} across {:?}...",
        meta.file_name, BENCH_CONTEXTS
    );
    for &ctx in BENCH_CONTEXTS {
        let est = fit::calculate_fit(&meta, &settings, &cfg.vram, measure_free_vram_mib());
        let s_cfg = &cfg.server;
        let cmd_args =
            args::build_server_args(&meta.path, s_cfg, &settings, Some(&meta), Some(ctx), None);
        let cmd_str = args::format_cmd(&exe, &cmd_args);
        let _ = logger.info(&format!("bench llama-server (ctx {ctx}): {cmd_str}"));
        let Ok(mut srv) = spawn_and_wait(&exe, &cmd_args, paths, cfg) else {
            println!("  ctx {ctx}: failed to start");
            continue;
        };
        let m = launch::measure_speed(&cfg.server.host, cfg.server.port, BENCH_TOKENS, None)?;
        let spill = procs::check_server_vram_spill(
            srv.pid,
            cfg.vram.shared_spill_threshold_mib,
            measure_free_vram_mib(),
            cfg.vram.reserve_mib,
        );
        let kv = srv
            .telemetry
            .lock()
            .ok()
            .and_then(|t| t.kv_buffer_mib)
            .unwrap_or(0.0);
        srv.stop();

        let (pt, gt, ded, shr) = (
            m.prompt_tps,
            m.gen_tps,
            spill.dedicated_mib,
            spill.shared_mib,
        );
        let fit_tot = est.total_mib_at_chosen_ctx;
        let msg = format!(
            "ctx {ctx:5} | prompt:{pt:6.1} t/s | gen:{gt:5.1} t/s | PDH ded:{ded:5}M shr:{shr:4}M | KV:{kv:5.1}M (fit:{fit_tot}M)"
        );
        println!("  {msg}");
        let _ = logger.info(&format!("bench: {msg}"));
    }
    Ok(())
}
