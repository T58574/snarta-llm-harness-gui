use crate::config::PrepareConfig;
use crate::server;
use crate::sysmon::procs::{BYTES_PER_MIB, GpuProcessMemory, query_gpu_process_memory};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
#[cfg(windows)]
use std::time::Instant;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[allow(dead_code)]
pub const SOFT_CLOSE_TIMEOUT_MS: u64 = 350;
pub use crate::sysmon::{run_cli_prepare, set_process_high_priority};

pub const PROTECTED_SYSTEM_PROCS: &[&str] = &[
    "csrss.exe",
    "wininit.exe",
    "services.exe",
    "lsass.exe",
    "smss.exe",
    "winlogon.exe",
    "dwm.exe",
    "explorer.exe",
    "fontdrvhost.exe",
    "svchost.exe",
    "system",
    "registry",
    "amdfendrsr.exe",
    "atieclxx.exe",
    "atiesrxx.exe",
    "conhost.exe",
    "windowsterminal.exe",
    "openconsole.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "snarta.exe",
    "textinputhost.exe",
    "ctfmon.exe",
    "sihost.exe",
    "taskhostw.exe",
];

#[derive(Debug, Clone)]
pub struct PrepareCandidate {
    pub pid: u32,
    pub pids: Vec<u32>,
    pub name: String,
    pub reason: &'static str,
    pub ram_mib: u64,
    pub vram_mib: u64,
}

#[derive(Debug, Clone)]
pub struct PreparePlan {
    pub candidates: Vec<PrepareCandidate>,
    pub est_ram_mib: u64,
    pub est_vram_mib: u64,
    #[allow(dead_code)]
    pub port_busy_by_other: bool,
}

#[derive(Debug, Clone)]
pub struct PrepareReport {
    pub closed_count: usize,
    pub dry_run: bool,
    pub free_vram_before_mib: u64,
    pub free_vram_after_mib: u64,
    pub used_ram_before_mib: u64,
    pub used_ram_after_mib: u64,
}

pub fn is_protected_process(
    name: &str,
    pid: u32,
    protected_pids: &[u32],
    never_list: &[String],
) -> bool {
    if pid <= 4 || protected_pids.contains(&pid) {
        return true;
    }
    let low = name.trim().to_ascii_lowercase();
    if PROTECTED_SYSTEM_PROCS.iter().any(|&p| p == low) {
        return true;
    }
    never_list
        .iter()
        .any(|n| n.trim().eq_ignore_ascii_case(&low))
}

fn ancestor_pids(sys: &System, mut cur: u32) -> Vec<u32> {
    let mut out = vec![cur];
    for _ in 0..16 {
        let Some(p) = sys.process(Pid::from_u32(cur)) else {
            break;
        };
        let Some(parent) = p.parent() else {
            break;
        };
        let p_u32 = parent.as_u32();
        if p_u32 == 0 || out.contains(&p_u32) {
            break;
        }
        out.push(p_u32);
        cur = p_u32;
    }
    out
}

#[derive(Default)]
struct ProcessGroup {
    name: String,
    pids: Vec<u32>,
    vram_mib: u64,
    ram_mib: u64,
    orphan: Option<&'static str>,
}

pub fn scan_prepare_candidates(
    cfg: &PrepareConfig,
    active_srv_pid: Option<u32>,
    host: &str,
    port: u16,
) -> PreparePlan {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let gpu_map = query_gpu_process_memory();
    let mut protected_pids = ancestor_pids(&sys, std::process::id());
    if let Some(srv_pid) = active_srv_pid {
        protected_pids.push(srv_pid);
    }

    let mut groups: std::collections::BTreeMap<String, ProcessGroup> =
        std::collections::BTreeMap::new();

    for (pid, proc_info) in sys.processes() {
        let p_u32 = pid.as_u32();
        let name = proc_info.name().to_string_lossy().into_owned();
        if is_protected_process(&name, p_u32, &protected_pids, &cfg.never_list) {
            continue;
        }
        let low = name.to_ascii_lowercase();
        let (gpu, r) = (gpu_map.get(&p_u32), proc_info.memory() / BYTES_PER_MIB);
        let v = gpu.map_or(0, GpuProcessMemory::dedicated_mib);
        let entry = groups.entry(low.clone()).or_insert_with(|| ProcessGroup {
            name,
            ..Default::default()
        });
        entry.pids.push(p_u32);
        (entry.vram_mib, entry.ram_mib) = (entry.vram_mib + v, entry.ram_mib + r);
        if low == "llama-server.exe" && active_srv_pid != Some(p_u32) {
            entry.orphan = Some("хвост llama-server");
        } else if low == "node.exe"
            && proc_info
                .cmd()
                .iter()
                .any(|a| a.to_string_lossy().contains("0xAgent"))
        {
            entry.orphan = Some("хвост 0xAgent (node.exe)");
        }
    }

    let mut candidates = Vec::new();
    for (low, grp) in groups {
        let reason = if let Some(r) = grp.orphan {
            Some(r)
        } else if cfg
            .kill_list
            .iter()
            .any(|k| k.trim().eq_ignore_ascii_case(&low))
        {
            Some("kill_list")
        } else if grp.vram_mib >= 20 {
            Some("держит VRAM")
        } else if grp.ram_mib >= 400 {
            Some("высокий RAM")
        } else {
            None
        };

        if let Some(reason) = reason
            && !grp.pids.is_empty()
        {
            let name = if grp.pids.len() > 1 {
                format!("{} ({} шт.)", grp.name, grp.pids.len())
            } else {
                grp.name
            };
            candidates.push(PrepareCandidate {
                pid: grp.pids[0],
                pids: grp.pids,
                name,
                reason,
                ram_mib: grp.ram_mib,
                vram_mib: grp.vram_mib,
            });
        }
    }

    candidates.sort_by_key(|c| std::cmp::Reverse((c.vram_mib, c.ram_mib)));
    let est_ram_mib = candidates.iter().map(|c| c.ram_mib).sum();
    let est_vram_mib = candidates.iter().map(|c| c.vram_mib).sum();
    let port_busy_by_other = active_srv_pid.is_none()
        && TcpStream::connect_timeout(
            &format!("{host}:{port}")
                .parse()
                .unwrap_or_else(|_| ([127, 0, 0, 1], port).into()),
            Duration::from_millis(150),
        )
        .is_ok();

    PreparePlan {
        candidates,
        est_ram_mib,
        est_vram_mib,
        port_busy_by_other,
    }
}

pub fn execute_prepare_plan(targets: &[PrepareCandidate], dry_run: bool) -> PrepareReport {
    let free_vram_before_mib = server::measure_free_vram_mib();
    let used_ram_before_mib = system_used_ram_mib();
    let mut closed_count = 0;

    if dry_run {
        closed_count = targets.len();
    } else {
        let all_pids: Vec<u32> = targets
            .iter()
            .flat_map(|t| t.pids.iter().copied())
            .collect();
        close_processes_fast(&all_pids);
        for t in targets {
            let any_alive = t
                .pids
                .iter()
                .any(|&pid| crate::sysmon::procs::is_pid_alive(pid));
            if !any_alive {
                closed_count += 1;
            }
        }
        if closed_count > 0 {
            thread::sleep(Duration::from_millis(50));
        }
    }

    PrepareReport {
        closed_count,
        dry_run,
        free_vram_before_mib,
        free_vram_after_mib: server::measure_free_vram_mib(),
        used_ram_before_mib,
        used_ram_after_mib: system_used_ram_mib(),
    }
}

pub fn system_used_ram_mib() -> u64 {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.used_memory() / BYTES_PER_MIB
}

#[cfg(windows)]
pub fn close_processes_fast(pids: &[u32]) -> usize {
    use std::collections::HashSet;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
    };

    if pids.is_empty() {
        return 0;
    }

    let my_pid = std::process::id();
    let target_set: HashSet<u32> = pids
        .iter()
        .copied()
        .filter(|&pid| pid > 4 && pid != my_pid)
        .collect();

    if target_set.is_empty() {
        return 0;
    }

    // 1. Single pass: post WM_CLOSE to all windows belonging to target PIDs simultaneously
    unsafe extern "system" fn enum_all_cb(hwnd: HWND, lparam: LPARAM) -> i32 {
        unsafe {
            let mut win_pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut win_pid);
            let set = &*(lparam as *const HashSet<u32>);
            if set.contains(&win_pid) {
                let _ = PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            1
        }
    }

    unsafe {
        let set_ptr = &target_set as *const HashSet<u32> as isize;
        let _ = EnumWindows(Some(enum_all_cb), set_ptr);
    }

    // 2. Wait up to 350ms in parallel for graceful exit
    let deadline = Instant::now() + Duration::from_millis(SOFT_CLOSE_TIMEOUT_MS);
    let mut remaining: Vec<u32> = target_set.iter().copied().collect();

    while Instant::now() < deadline && !remaining.is_empty() {
        remaining.retain(|&pid| crate::sysmon::procs::is_pid_alive(pid));
        if remaining.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(40));
    }

    // 3. Force terminate any remaining stubborn or windowless processes in parallel
    let mut closed_count = 0;
    for &pid in &target_set {
        if !crate::sysmon::procs::is_pid_alive(pid) {
            closed_count += 1;
            continue;
        }
        unsafe {
            let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if !h.is_null() {
                if TerminateProcess(h, 1) != 0 {
                    closed_count += 1;
                }
                let _ = CloseHandle(h);
            }
        }
    }

    closed_count
}

#[cfg(not(windows))]
pub fn close_processes_fast(pids: &[u32]) -> usize {
    pids.len()
}
