pub mod optimize;
pub mod procs;

use crate::config::Config;
use crate::log::Logger;
use crate::paths::AppPaths;
use crate::server::LockState;
use crate::sysmon::optimize::{PrepareCandidate, execute_prepare_plan, scan_prepare_candidates};
use std::io::{self, BufRead, Write};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[allow(dead_code)]
pub const HIGH_PERF_GUID: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

#[allow(dead_code)]
static SAVED_POWER_GUID: OnceLock<Mutex<Option<String>>> = OnceLock::new();
#[allow(dead_code)]
static HOOKS_INSTALLED: OnceLock<()> = OnceLock::new();

#[allow(dead_code)]
pub fn parse_powercfg_guid(output: &str) -> Option<String> {
    for token in output.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_ascii_hexdigit() && c != '-');
        if clean.len() == 36 && clean.chars().filter(|&c| c == '-').count() == 4 {
            return Some(clean.to_ascii_lowercase());
        }
    }
    None
}

#[allow(dead_code)]
pub fn activate_high_performance_power_plan() -> Option<String> {
    install_exit_restore_hooks();
    let out = Command::new("powercfg")
        .arg("/getactivescheme")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let cur_guid = parse_powercfg_guid(&text)?;
    let mtx = SAVED_POWER_GUID.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = mtx.lock()
        && guard.is_none()
    {
        *guard = Some(cur_guid.clone());
    }
    if cur_guid != HIGH_PERF_GUID {
        let _ = Command::new("powercfg")
            .args(["/setactive", HIGH_PERF_GUID])
            .status();
    }
    Some(cur_guid)
}

#[allow(dead_code)]
pub fn restore_power_plan() {
    let Some(mtx) = SAVED_POWER_GUID.get() else {
        return;
    };
    let Ok(mut guard) = mtx.lock() else {
        return;
    };
    if let Some(orig) = guard.take()
        && orig != HIGH_PERF_GUID
    {
        let _ = Command::new("powercfg")
            .args(["/setactive", &orig])
            .status();
    }
}

#[allow(dead_code)]
pub fn install_exit_restore_hooks() {
    HOOKS_INSTALLED.get_or_init(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_power_plan();
            prev(info);
        }));
        #[cfg(windows)]
        unsafe {
            unsafe extern "system" fn ctrl_handler(_ctrl_type: u32) -> i32 {
                restore_power_plan();
                0
            }
            let _ =
                windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(ctrl_handler), 1);
        }
    });
}

#[cfg(windows)]
pub fn set_process_high_priority(pid: u32, high: bool) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_SET_INFORMATION,
        SetPriorityClass,
    };
    unsafe {
        let h = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        let cls = if high {
            HIGH_PRIORITY_CLASS
        } else {
            NORMAL_PRIORITY_CLASS
        };
        let ok = SetPriorityClass(h, cls) != 0;
        let _ = CloseHandle(h);
        ok
    }
}

#[cfg(not(windows))]
pub fn set_process_high_priority(_pid: u32, _high: bool) -> bool {
    false
}

pub fn run_cli_prepare(paths: &AppPaths, cfg: &Config, logger: &Logger) -> io::Result<()> {
    let active_srv = LockState::read_active(&paths.lock_file()).map(|l| l.server_pid);
    let plan = scan_prepare_candidates(&cfg.prepare, active_srv, &cfg.server.host, cfg.server.port);
    if plan.candidates.is_empty() {
        println!("Prepare: лишних процессов из kill_list или хвостов не найдено.");
        return Ok(());
    }
    println!(
        "Кандидаты на закрытие (оценка освобождения: ~{} MiB VRAM, ~{} MiB RAM):",
        plan.est_vram_mib, plan.est_ram_mib
    );
    for c in &plan.candidates {
        println!(
            "  PID {:6} | {:22} | VRAM {:5} MiB | RAM {:5} MiB | {}",
            c.pid, c.name, c.vram_mib, c.ram_mib, c.reason
        );
    }
    print!("Закрыть процессы? [a = все / y = по одному / n = отмена]: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let choice = line.trim().to_ascii_lowercase();
    let chosen: Vec<PrepareCandidate> = match choice.as_str() {
        "a" | "all" => plan.candidates,
        "y" | "yes" => {
            let mut sel = Vec::new();
            for c in plan.candidates {
                print!("  Закрыть {} (PID {})? [y/N]: ", c.name, c.pid);
                io::stdout().flush()?;
                line.clear();
                io::stdin().lock().read_line(&mut line)?;
                if matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                    sel.push(c);
                }
            }
            sel
        }
        _ => return Ok(()),
    };
    let rep = execute_prepare_plan(&chosen, cfg.prepare.dry_run);
    let _ = logger.info(&format!(
        "prepare closed={} dry_run={} vram_before={} vram_after={}",
        rep.closed_count, rep.dry_run, rep.free_vram_before_mib, rep.free_vram_after_mib
    ));
    println!(
        "Готово (закрыто: {}, dry_run: {}). Свободно VRAM: {} -> {} MiB | Занято RAM: {} -> {} MiB",
        rep.closed_count,
        rep.dry_run,
        rep.free_vram_before_mib,
        rep.free_vram_after_mib,
        rep.used_ram_before_mib,
        rep.used_ram_after_mib
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sysmon::optimize::is_protected_process;

    #[test]
    fn protects_system_and_never_list_processes() {
        let never = vec!["my_music_app.exe".to_string()];
        assert!(is_protected_process("dwm.exe", 900, &[100, 200], &never));
        assert!(is_protected_process("csrss.exe", 500, &[100], &never));
        assert!(is_protected_process("anything.exe", 100, &[100], &never));
        assert!(is_protected_process(
            "My_Music_App.exe",
            777,
            &[100],
            &never
        ));
        assert!(!is_protected_process("chrome.exe", 888, &[100], &never));
    }

    #[test]
    fn extracts_powercfg_guid_from_output() {
        let out = "Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced)";
        assert_eq!(
            parse_powercfg_guid(out).as_deref(),
            Some("381b4222-f694-41f0-9685-ff5bb260df2e")
        );
        assert_eq!(HIGH_PERF_GUID.len(), 36);
    }
}
