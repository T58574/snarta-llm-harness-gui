#![recursion_limit = "1024"]
#![windows_subsystem = "windows"]

extern crate gpui_kit as gpui;

mod chat;
mod config;
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

use config::Config;
use log::Logger;
use paths::AppPaths;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run_app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_app() -> io::Result<()> {
    let paths = AppPaths::discover()?;
    paths.ensure_dirs()?;

    let (cfg, _cfg_state) = Config::load_or_create(&paths.config_file())?;
    let logger = Logger::new(paths.snarta_log_file(), cfg.limits.log_max_bytes);

    gui::run_gui(paths, cfg, logger)
}
