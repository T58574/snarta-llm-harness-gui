use crate::config::{Config, ConfigLoadState};
use crate::gpu;
use crate::paths::AppPaths;
use crate::sysmon::procs;
use crate::winpath;
use std::io;

pub fn run(paths: &AppPaths, cfg: &Config, cfg_state: ConfigLoadState) -> io::Result<()> {
    println!("Snarta doctor");
    println!("-------------");
    println!(
        "Root:         {} ({})",
        paths.root.display(),
        paths.source.label()
    );
    println!("Working dir:  {}", paths.cwd.display());
    println!("Executable:   {}", paths.exe_dir.display());
    println!(
        "Config:       {} ({})",
        paths.config_file().display(),
        match cfg_state {
            ConfigLoadState::LoadedExisting => "loaded",
            ConfigLoadState::CreatedDefault => "created default",
        }
    );
    println!("Personas dir: {}", paths.personas_dir().display());
    println!("Life dir:     {}", paths.life_dir().display());
    println!("Memory file:  {}", paths.memory_file().display());
    println!("Runtime dir:  {}", paths.runtime_dir().display());
    println!("Models dir:   {}", paths.models_dir().display());
    println!("Logs dir:     {}", paths.logs_dir().display());
    println!("Server log:   {}", paths.llama_server_log_file().display());
    println!("Lock file:    {}", paths.lock_file().display());
    println!("Server addr:  {}:{}", cfg.server.host, cfg.server.port);
    println!(
        "Active build: {} (last good: {})",
        cfg.runtime.active_build.as_deref().unwrap_or("none"),
        cfg.runtime.last_good_build.as_deref().unwrap_or("none")
    );

    match winpath::is_installed(&paths.exe_dir) {
        Ok(true) => println!("User PATH:    installed"),
        Ok(false) => println!("User PATH:    not installed (run `snarta path install`)"),
        Err(e) => println!("User PATH:    check error ({e})"),
    }

    println!();
    println!("GPU (DXGI):");
    match gpu::detect_gpus() {
        Ok(gpus) if gpus.is_empty() => {
            println!("  no hardware DXGI adapters detected");
        }
        Ok(gpus) => {
            for (idx, g) in gpus.iter().enumerate() {
                let target = g.gfx_target.as_deref().unwrap_or("none");
                let used = procs::total_measured_vram_used_mib_filtered(g.luid_str.as_deref());
                let free = g.dedicated_vram_mib().saturating_sub(used);
                println!(
                    "  [{}] {} | VRAM: {} MiB (free: {} MiB, used: {} MiB) | Shared: {} MiB | target: {}",
                    idx,
                    g.name,
                    g.dedicated_vram_mib(),
                    free,
                    used,
                    g.shared_system_mib(),
                    target
                );
            }
        }
        Err(e) => {
            println!("  DXGI query failed: {e}");
        }
    }

    println!();
    println!("Top processes (VRAM / RAM / CPU):");
    for p in procs::snapshot_top_processes(5) {
        println!(
            "  PID {:6} | {:24} | VRAM: {:5} MiB (shared {:4} MiB) | RAM: {:5} MiB | CPU: {:4.1}%",
            p.pid, p.name, p.vram_dedicated_mib, p.vram_shared_mib, p.ram_mib, p.cpu_pct
        );
    }

    Ok(())
}
