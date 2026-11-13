use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use sysinfo::{Pid, ProcessesToUpdate, System};

pub const BYTES_PER_MIB: u64 = 1024 * 1024;

static SHARED_SYS: OnceLock<Mutex<System>> = OnceLock::new();

fn with_shared_sys<F, R>(f: F) -> R
where
    F: FnOnce(&mut System) -> R,
{
    let mtx = SHARED_SYS.get_or_init(|| Mutex::new(System::new()));
    let mut guard = mtx.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GpuProcessMemory {
    pub pid: u32,
    pub dedicated_bytes: u64,
    pub shared_bytes: u64,
}

impl GpuProcessMemory {
    pub fn dedicated_mib(&self) -> u64 {
        self.dedicated_bytes / BYTES_PER_MIB
    }

    pub fn shared_mib(&self) -> u64 {
        self.shared_bytes / BYTES_PER_MIB
    }
}

#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub ram_mib: u64,
    pub vram_dedicated_mib: u64,
    pub vram_shared_mib: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillWatchdogStatus {
    pub pid: u32,
    pub dedicated_mib: u64,
    pub shared_mib: u64,
    pub threshold_mib: u64,
    pub spilled: bool,
}

pub fn check_server_vram_spill(
    pid: u32,
    threshold_mib: u64,
    free_vram_mib: u64,
    reserve_mib: u64,
) -> SpillWatchdogStatus {
    let m = query_gpu_process_memory().remove(&pid).unwrap_or_default();
    let (d, s) = (m.dedicated_mib(), m.shared_mib());
    SpillWatchdogStatus {
        pid,
        dedicated_mib: d,
        shared_mib: s,
        threshold_mib,
        spilled: free_vram_mib <= reserve_mib && s > threshold_mib,
    }
}

#[allow(dead_code)]
pub fn total_measured_vram_used_mib() -> u64 {
    total_measured_vram_used_mib_filtered(None)
}

pub fn total_measured_vram_used_mib_filtered(luid_filter: Option<&str>) -> u64 {
    if let Some(bytes) = query_gpu_adapter_dedicated_bytes(luid_filter) {
        return bytes / BYTES_PER_MIB;
    }
    let map = query_gpu_process_memory();
    let total_bytes: u64 = map.values().map(|m| m.dedicated_bytes).sum();
    total_bytes / BYTES_PER_MIB
}

#[allow(dead_code)]
pub fn system_cpu_and_ram() -> (f32, u64, u64) {
    with_shared_sys(|sys| {
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        (
            sys.global_cpu_usage(),
            sys.used_memory() / BYTES_PER_MIB,
            sys.total_memory() / BYTES_PER_MIB,
        )
    })
}

pub fn snapshot_top_processes(limit: usize) -> Vec<ProcessSnapshot> {
    let gpu_map = query_gpu_process_memory();

    let mut list: Vec<ProcessSnapshot> = with_shared_sys(|sys| {
        sys.refresh_processes(ProcessesToUpdate::All, true);
        sys.processes()
            .iter()
            .map(|(pid, proc_info)| {
                let p_u32 = pid.as_u32();
                let gpu = gpu_map.get(&p_u32);
                ProcessSnapshot {
                    pid: p_u32,
                    name: proc_info.name().to_string_lossy().into_owned(),
                    cpu_pct: proc_info.cpu_usage(),
                    ram_mib: proc_info.memory() / BYTES_PER_MIB,
                    vram_dedicated_mib: gpu.map_or(0, GpuProcessMemory::dedicated_mib),
                    vram_shared_mib: gpu.map_or(0, GpuProcessMemory::shared_mib),
                }
            })
            .collect()
    });

    list.sort_by_key(|p| std::cmp::Reverse((p.vram_dedicated_mib, p.ram_mib, p.cpu_pct as u64)));
    list.truncate(limit);
    list
}

pub fn is_pid_alive(pid: u32) -> bool {
    with_shared_sys(|sys| {
        let target = Pid::from_u32(pid);
        sys.refresh_processes(ProcessesToUpdate::Some(&[target]), true);
        sys.process(target).is_some()
    })
}

pub fn parse_pid_from_pdh_instance(instance_name: &str) -> Option<u32> {
    let rest = instance_name.strip_prefix("pid_")?;
    let end = rest.find('_').unwrap_or(rest.len());
    rest[..end].parse::<u32>().ok()
}

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
unsafe fn read_raw_counter_array(counter: *mut core::ffi::c_void) -> Vec<(String, u64)> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Performance::{
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_LARGE, PDH_MORE_DATA, PdhGetFormattedCounterArrayW,
    };

    unsafe {
        let mut buf_size: u32 = 0;
        let mut item_count: u32 = 0;
        let status = PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_LARGE,
            &mut buf_size,
            &mut item_count,
            null_mut(),
        );
        if (status != PDH_MORE_DATA && status != ERROR_SUCCESS) || buf_size == 0 || item_count == 0
        {
            return Vec::new();
        }

        // Allocate 8-byte aligned backing storage for PDH_FMT_COUNTERVALUE_ITEM_W
        let u64_len = ((buf_size as usize) + 64).div_ceil(8);
        let mut buffer: Vec<u64> = vec![0u64; u64_len];
        let items_ptr = buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
        if PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_LARGE,
            &mut buf_size,
            &mut item_count,
            items_ptr,
        ) != ERROR_SUCCESS
        {
            return Vec::new();
        }

        let items = std::slice::from_raw_parts(items_ptr, item_count as usize);
        let mut out = Vec::with_capacity(item_count as usize);
        for item in items {
            // PDH_CSTATUS_VALID_DATA (0) or PDH_CSTATUS_NEW_DATA (1)
            if item.FmtValue.CStatus > 1 || item.szName.is_null() {
                continue;
            }
            let mut len = 0usize;
            while *item.szName.add(len) != 0 && len < 256 {
                len += 1;
            }
            let name = String::from_utf16_lossy(std::slice::from_raw_parts(item.szName, len));
            let val = item.FmtValue.Anonymous.largeValue.max(0) as u64;
            out.push((name, val));
        }
        out
    }
}

#[cfg(windows)]
pub fn query_gpu_adapter_dedicated_bytes(luid_filter: Option<&str>) -> Option<u64> {
    use core::ffi::c_void;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhOpenQueryW,
    };

    const PATH_ADAPTER: &str = "\\GPU Adapter Memory(*)\\Dedicated Usage";

    unsafe {
        let mut query: *mut c_void = null_mut();
        if PdhOpenQueryW(null(), 0, &mut query) != ERROR_SUCCESS {
            return None;
        }
        let adp_w = to_wide(PATH_ADAPTER);
        let mut counter: *mut c_void = null_mut();
        let ok = PdhAddEnglishCounterW(query, adp_w.as_ptr(), 0, &mut counter) == ERROR_SUCCESS;
        let mut result = None;
        if ok && PdhCollectQueryData(query) == ERROR_SUCCESS {
            let items = read_raw_counter_array(counter);
            if let Some(target) = luid_filter {
                let low = target.to_lowercase();
                for (name, val) in &items {
                    if name.to_lowercase().contains(&low) {
                        result = Some(*val);
                        break;
                    }
                }
            }
            if result.is_none() && !items.is_empty() {
                result = items.into_iter().map(|(_, val)| val).max();
            }
        }
        let _ = PdhCloseQuery(query);
        result
    }
}

#[cfg(windows)]
pub fn query_gpu_process_memory() -> HashMap<u32, GpuProcessMemory> {
    use core::ffi::c_void;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhOpenQueryW,
    };

    const PATH_DEDICATED: &str = "\\GPU Process Memory(*)\\Dedicated Usage";
    const PATH_SHARED: &str = "\\GPU Process Memory(*)\\Shared Usage";

    unsafe {
        let mut query: *mut c_void = null_mut();
        if PdhOpenQueryW(null(), 0, &mut query) != ERROR_SUCCESS {
            return HashMap::new();
        }

        let ded_w = to_wide(PATH_DEDICATED);
        let shr_w = to_wide(PATH_SHARED);
        let mut ded_counter: *mut c_void = null_mut();
        let mut shr_counter: *mut c_void = null_mut();

        let ok_ded =
            PdhAddEnglishCounterW(query, ded_w.as_ptr(), 0, &mut ded_counter) == ERROR_SUCCESS;
        let ok_shr =
            PdhAddEnglishCounterW(query, shr_w.as_ptr(), 0, &mut shr_counter) == ERROR_SUCCESS;

        let mut map: HashMap<u32, GpuProcessMemory> = HashMap::new();
        if (ok_ded || ok_shr) && PdhCollectQueryData(query) == ERROR_SUCCESS {
            let mut record = |counter, is_shared| {
                for (name, bytes) in read_raw_counter_array(counter) {
                    if let Some(pid) = parse_pid_from_pdh_instance(&name) {
                        let e = map.entry(pid).or_insert(GpuProcessMemory {
                            pid,
                            dedicated_bytes: 0,
                            shared_bytes: 0,
                        });
                        if is_shared {
                            e.shared_bytes += bytes;
                        } else {
                            e.dedicated_bytes += bytes;
                        }
                    }
                }
            };
            if ok_ded {
                record(ded_counter, false);
            }
            if ok_shr {
                record(shr_counter, true);
            }
        }

        let _ = PdhCloseQuery(query);
        map
    }
}

#[cfg(not(windows))]
pub fn query_gpu_adapter_dedicated_bytes(_luid_filter: Option<&str>) -> Option<u64> {
    None
}

#[cfg(not(windows))]
pub fn query_gpu_process_memory() -> HashMap<u32, GpuProcessMemory> {
    let _ = parse_pid_from_pdh_instance("pid_1_luid_0");
    HashMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pdh_gpu_process_instance_name() {
        let inst = "pid_14280_luid_0x00000000_0x0001245A_phys_0";
        assert_eq!(parse_pid_from_pdh_instance(inst), Some(14280));
        assert_eq!(parse_pid_from_pdh_instance("invalid"), None);
    }

    #[test]
    fn detects_current_process_alive() {
        let my_pid = std::process::id();
        assert!(is_pid_alive(my_pid));
    }
}
