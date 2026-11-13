#![recursion_limit = "1024"]
#![windows_subsystem = "windows"]

extern crate gpui_kit as gpui;

mod chat;
mod config;
mod doctor;
mod gguf;
mod gpu;
mod gui;
mod log;
mod paths;
mod persona;
mod runtime;
mod server;
mod sysmon;
mod tools;
mod vision;
mod winpath;

use config::Config;
use log::Logger;
use paths::AppPaths;
use std::env;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("gui");

    #[cfg(windows)]
    if cmd != "gui" {
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(
                windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }

    match run_cli(&args, cmd) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_cli(args: &[String], cmd: &str) -> io::Result<()> {
    let paths = AppPaths::discover()?;
    paths.ensure_dirs()?;

    let (mut cfg, cfg_state) = Config::load_or_create(&paths.config_file())?;
    let logger = Logger::new(paths.snarta_log_file(), cfg.limits.log_max_bytes);

    match cmd {
        "gui" => gui::run_gui(paths, cfg, logger),
        "doctor" => {
            let _ = logger.info("running snarta doctor");
            doctor::run(&paths, &cfg, cfg_state)
        }
        "prepare" => sysmon::optimize::run_cli_prepare(&paths, &cfg, &logger),
        "runtime" => runtime::handle_cli(&args[1..], &paths, &mut cfg, &logger),
        "fit" => server::cmd_fit(&paths, &cfg, args.get(1).map(String::as_str)),
        "serve" => server::cmd_serve(&paths, &mut cfg, &logger, args.get(1).map(String::as_str)),
        "stop" => server::cmd_stop(&paths),
        "bench" => server::cmd_bench(&paths, &mut cfg, &logger, args.get(1).map(String::as_str)),
        "path" => {
            let sub = args.get(1).map(String::as_str).unwrap_or("");
            match sub {
                "install" => {
                    let changed = winpath::install(&paths.exe_dir)?;
                    let _ = logger.info(&format!(
                        "path install: dir={}, changed={changed}",
                        paths.exe_dir.display()
                    ));
                    if changed {
                        println!(
                            "Added {} to user PATH (open a new terminal to use `snarta`).",
                            paths.exe_dir.display()
                        );
                    } else {
                        println!("{} is already in user PATH.", paths.exe_dir.display());
                    }
                    Ok(())
                }
                "uninstall" => {
                    let changed = winpath::uninstall(&paths.exe_dir)?;
                    let _ = logger.info(&format!(
                        "path uninstall: dir={}, changed={changed}",
                        paths.exe_dir.display()
                    ));
                    if changed {
                        println!("Removed {} from user PATH.", paths.exe_dir.display());
                    } else {
                        println!("{} was not found in user PATH.", paths.exe_dir.display());
                    }
                    Ok(())
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: snarta path <install|uninstall>",
                )),
            }
        }
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        "-V" | "--version" | "version" => {
            println!("snarta {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => {
            let _ = logger.error(&format!("unknown CLI command '{other}'"));
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown command '{other}' (run `snarta help`)"),
            ))
        }
    }
}

fn print_help() {
    println!("snarta {}", env!("CARGO_PKG_VERSION"));
    println!("Usage:");
    println!("  snarta [gui]                         Launch interactive desktop companion (GUI)");
    println!("  snarta doctor                        Show root, paths, config, and GPU info");
    println!("  snarta prepare                       Scan & close VRAM hogs with confirmation");
    println!("  snarta runtime list                  List installed llama.cpp builds");
    println!("  snarta runtime check                 Check latest GitHub releases (ROCm/Vulkan)");
    println!("  snarta runtime install [rocm|vulkan] Download, verify, and activate build");
    println!("  snarta runtime use <build-id>        Switch active llama.cpp build");
    println!("  snarta runtime remove <build-id>     Delete an installed build");
    println!(
        "  snarta runtime clean                 Remove old inactive builds to free disk space"
    );
    println!("  snarta fit [model]                   Calculate max context for free VRAM");
    println!("  snarta serve [model]                 Launch llama-server with VRAM watchdog");
    println!("  snarta stop                          Stop running llama-server");
    println!("  snarta bench [model]                 Benchmark 8k/16k/24k/32k speed & VRAM");
    println!("  snarta path install                  Add snarta directory to user PATH (HKCU)");
    println!("  snarta path uninstall                Remove snarta directory from user PATH");
    println!("  snarta help                          Show this help message");
}
