pub mod install;
pub mod releases;

use crate::config::Config;
use crate::gpu;
use crate::log::Logger;
use crate::paths::AppPaths;
use install::BYTES_PER_MIB;
use std::fs;
use std::io;

pub fn handle_cli(
    args: &[String],
    paths: &AppPaths,
    cfg: &mut Config,
    logger: &Logger,
) -> io::Result<()> {
    let sub = args.first().map(String::as_str).unwrap_or("list");
    match sub {
        "list" => cmd_list(paths, cfg),
        "check" => cmd_check(cfg),
        "install" => cmd_install(args.get(1).map(String::as_str), paths, cfg, logger),
        "use" => {
            let id = args.get(1).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: snarta runtime use <build-id>",
                )
            })?;
            cmd_use(id, paths, cfg, logger)
        }
        "remove" => {
            let id = args.get(1).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: snarta runtime remove <build-id>",
                )
            })?;
            cmd_remove(id, paths, cfg, logger)
        }
        "clean" => {
            let (count, freed) = install::remove_old_builds(
                &paths.runtime_dir(),
                cfg.runtime.active_build.as_deref(),
            )?;
            if cfg.runtime.last_good_build.as_deref() != cfg.runtime.active_build.as_deref() {
                cfg.runtime.last_good_build = cfg.runtime.active_build.clone();
                cfg.save(&paths.config_file())?;
            }
            let _ = logger.info(&format!("cleaned {count} old runtime builds via CLI"));
            println!(
                "Cleaned {count} old builds (freed {}).",
                install::format_bytes(freed)
            );
            Ok(())
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unknown runtime subcommand '{other}' (expected: list | check | install | use | remove | clean)"
            ),
        )),
    }
}

fn cmd_list(paths: &AppPaths, cfg: &Config) -> io::Result<()> {
    let builds = install::list_installed(&paths.runtime_dir())?;
    let active = cfg.runtime.active_build.as_deref();
    let last_good = cfg.runtime.last_good_build.as_deref();

    println!(
        "Installed llama.cpp builds in {}:",
        paths.runtime_dir().display()
    );
    if builds.is_empty() {
        println!("  (none installed - run `snarta runtime install`)");
        return Ok(());
    }

    for b in builds {
        let mut tags = Vec::new();
        if Some(b.build_id.as_str()) == active {
            tags.push("active");
        }
        if Some(b.build_id.as_str()) == last_good {
            tags.push("last-good");
        }
        let suffix = if tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", tags.join(", "))
        };
        println!(
            "  - {} ({}, {}) -> {}{}",
            b.build_id,
            b.backend.as_str(),
            install::format_bytes(b.size_bytes),
            b.server_exe.display(),
            suffix
        );
    }
    Ok(())
}

pub fn detect_preferred_gfx() -> Option<String> {
    gpu::detect_gpus()
        .ok()
        .and_then(|gpus| gpus.into_iter().find_map(|g| g.gfx_target))
}

fn cmd_check(cfg: &Config) -> io::Result<()> {
    let gfx = detect_preferred_gfx();
    println!(
        "Checking releases (ROCm: {}, Vulkan: {}, target: {})...",
        cfg.runtime.rocm_repo,
        cfg.runtime.vulkan_repo,
        gfx.as_deref().unwrap_or(releases::DEFAULT_GFX_TARGET)
    );

    let candidates = releases::fetch_candidates(
        &cfg.runtime.rocm_repo,
        &cfg.runtime.vulkan_repo,
        gfx.as_deref(),
    )?;

    let active = cfg.runtime.active_build.as_deref();
    for (idx, c) in candidates.iter().take(6).enumerate() {
        let marker = if Some(c.build_id.as_str()) == active {
            " [active]"
        } else if idx == 0 {
            " [recommended]"
        } else {
            ""
        };
        println!(
            "  [{}] {} | {} | {:.1} MiB | {}{}",
            idx + 1,
            c.build_id,
            c.backend.as_str(),
            c.size_bytes as f64 / BYTES_PER_MIB,
            c.asset_name,
            marker
        );
    }
    Ok(())
}

fn cmd_install(
    requested: Option<&str>,
    paths: &AppPaths,
    cfg: &mut Config,
    logger: &Logger,
) -> io::Result<()> {
    let gfx = detect_preferred_gfx();
    let candidates = releases::fetch_candidates(
        &cfg.runtime.rocm_repo,
        &cfg.runtime.vulkan_repo,
        gfx.as_deref(),
    )?;

    let chosen = match requested {
        None => candidates.first().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no release candidate available")
        })?,
        Some("vulkan") => candidates
            .iter()
            .find(|c| c.backend == releases::BackendKind::Vulkan)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no Vulkan candidate found"))?,
        Some("rocm" | "hip") => candidates
            .iter()
            .find(|c| c.backend == releases::BackendKind::RocmHip)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no ROCm candidate found"))?,
        Some(query) => candidates
            .iter()
            .find(|c| c.build_id == query || c.tag == query || c.asset_name.contains(query))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no release matching '{query}'"),
                )
            })?,
    };

    println!(
        "Installing {} ({}, {:.1} MiB) from {}...",
        chosen.build_id,
        chosen.backend.as_str(),
        chosen.size_bytes as f64 / BYTES_PER_MIB,
        chosen.repo
    );
    let _ = logger.info(&format!(
        "installing runtime build {} from {}",
        chosen.build_id, chosen.download_url
    ));

    let (installed, report) = install::download_and_install(&paths.runtime_dir(), chosen)?;
    println!("Verified: {}", report.version_line);
    if !report.devices_output.is_empty() {
        println!("Devices:\n{}", report.devices_output);
    }

    if cfg.runtime.active_build.is_some() && cfg.runtime.last_good_build.is_none() {
        cfg.runtime.last_good_build = cfg.runtime.active_build.clone();
    }
    cfg.runtime.active_build = Some(installed.build_id.clone());
    if report.has_gpu_device {
        cfg.runtime.last_good_build = Some(installed.build_id.clone());
    }
    cfg.save(&paths.config_file())?;

    let _ = logger.info(&format!("activated runtime build {}", installed.build_id));
    println!("Active build set to: {}", installed.build_id);
    Ok(())
}

fn cmd_use(build_id: &str, paths: &AppPaths, cfg: &mut Config, logger: &Logger) -> io::Result<()> {
    let builds = install::list_installed(&paths.runtime_dir())?;
    let Some(found) = builds.into_iter().find(|b| b.build_id == build_id) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("build '{build_id}' is not installed"),
        ));
    };

    install::check_required_files(&found.dir, found.backend)?;
    let report = install::verify_server_binary(&found.server_exe, found.backend)?;
    println!("Verified {}: {}", found.build_id, report.version_line);

    cfg.runtime.active_build = Some(found.build_id.clone());
    if report.has_gpu_device {
        cfg.runtime.last_good_build = Some(found.build_id.clone());
    }
    cfg.save(&paths.config_file())?;
    let _ = logger.info(&format!(
        "switched active runtime build to {}",
        found.build_id
    ));
    println!("Switched active build to: {}", found.build_id);
    Ok(())
}

fn cmd_remove(
    build_id: &str,
    paths: &AppPaths,
    cfg: &mut Config,
    logger: &Logger,
) -> io::Result<()> {
    if cfg.runtime.active_build.as_deref() == Some(build_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("cannot remove active build '{build_id}' (switch to another build first)"),
        ));
    }

    if build_id.is_empty()
        || build_id.contains('/')
        || build_id.contains('\\')
        || build_id.contains(':')
        || build_id.contains("..")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid build id '{build_id}'"),
        ));
    }

    let runtime_root = fs::canonicalize(paths.runtime_dir())?;
    let dir = paths.runtime_dir().join(build_id);
    if !dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "build '{build_id}' not found in {}",
                paths.runtime_dir().display()
            ),
        ));
    }
    let canon_dir = fs::canonicalize(&dir)?;
    if !canon_dir.starts_with(&runtime_root) || canon_dir == runtime_root {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("refusing to remove directory outside runtime dir: {build_id}"),
        ));
    }

    fs::remove_dir_all(&canon_dir)?;
    if cfg.runtime.last_good_build.as_deref() == Some(build_id) {
        cfg.runtime.last_good_build = cfg.runtime.active_build.clone();
        cfg.save(&paths.config_file())?;
    }

    let _ = logger.info(&format!("removed runtime build {build_id}"));
    println!("Removed build: {build_id}");
    Ok(())
}
