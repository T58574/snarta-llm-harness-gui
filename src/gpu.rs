use std::io;
use std::sync::OnceLock;

pub const BYTES_PER_MIB: u64 = 1024 * 1024;
pub const VENDOR_AMD: u32 = 0x1002;
pub const VENDOR_MICROSOFT: u32 = 0x1414;

static CACHED_GPUS: OnceLock<Vec<GpuInfo>> = OnceLock::new();

pub fn cached_primary_gpu() -> Option<GpuInfo> {
    CACHED_GPUS
        .get_or_init(|| detect_gpus().unwrap_or_default())
        .first()
        .cloned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuInfo {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_vram_bytes: u64,
    pub shared_system_bytes: u64,
    pub gfx_target: Option<String>,
    pub luid_str: Option<String>,
}

impl GpuInfo {
    pub fn dedicated_vram_mib(&self) -> u64 {
        self.dedicated_vram_bytes / BYTES_PER_MIB
    }

    pub fn shared_system_mib(&self) -> u64 {
        self.shared_system_bytes / BYTES_PER_MIB
    }
}

pub fn infer_gfx_target(name: &str, vendor_id: u32) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if vendor_id != VENDOR_AMD && !lower.contains("radeon") && !lower.contains("amd") {
        return None;
    }

    // RDNA3: RX 7600 / 7700 / 7800 / 7900 series -> gfx110X
    for token in [
        "7900", "7800", "7700", "7600", "7650", "w7900", "w7800", "w7700", "gfx110",
    ] {
        if lower.contains(token) {
            return Some("gfx110X".to_string());
        }
    }

    // RDNA3.5 / Strix Halo / Strix Point
    if lower.contains("8060s") || lower.contains("8050s") || lower.contains("gfx1151") {
        return Some("gfx1151".to_string());
    }

    // RDNA4: RX 9070 / 9060 series -> gfx120X
    for token in ["9070", "9060", "gfx120"] {
        if lower.contains(token) {
            return Some("gfx120X".to_string());
        }
    }

    // RDNA2: RX 6800 / 6900 / 6700 -> gfx103X
    for token in [
        "6950", "6900", "6800", "6750", "6700", "6650", "6600", "gfx103",
    ] {
        if lower.contains(token) {
            return Some("gfx103X".to_string());
        }
    }

    None
}

#[cfg(windows)]
pub fn detect_gpus() -> io::Result<Vec<GpuInfo>> {
    use core::ffi::c_void;
    use std::mem::zeroed;
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::S_OK;
    use windows_sys::core::GUID;

    const DXGI_ERROR_NOT_FOUND: i32 = 0x887A_0002u32 as i32;
    const DXGI_ADAPTER_FLAG_SOFTWARE: u32 = 2;

    // IID_IDXGIFactory1: 770aae78-f26f-4dba-a829-253c83d1b387
    const IID_IDXGI_FACTORY1: GUID = GUID {
        data1: 0x770a_ae78,
        data2: 0xf26f,
        data3: 0x4dba,
        data4: [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87],
    };

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct Luid {
        low_part: u32,
        high_part: i32,
    }

    #[repr(C)]
    struct DxgiAdapterDesc1 {
        description: [u16; 128],
        vendor_id: u32,
        device_id: u32,
        sub_sys_id: u32,
        revision: u32,
        dedicated_video_memory: usize,
        dedicated_system_memory: usize,
        shared_system_memory: usize,
        adapter_luid: Luid,
        flags: u32,
    }

    #[repr(C)]
    struct IdxgiAdapter1Vtbl {
        query_interface: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut IdxgiAdapter1) -> u32,
        set_private_data: usize,
        set_private_data_interface: usize,
        get_private_data: usize,
        get_parent: usize,
        enum_outputs: usize,
        get_desc: usize,
        check_interface_support: usize,
        get_desc1: unsafe extern "system" fn(*mut IdxgiAdapter1, *mut DxgiAdapterDesc1) -> i32,
    }

    #[repr(C)]
    struct IdxgiAdapter1 {
        vtbl: *const IdxgiAdapter1Vtbl,
    }

    #[repr(C)]
    struct IdxgiFactory1Vtbl {
        query_interface: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut IdxgiFactory1) -> u32,
        set_private_data: usize,
        set_private_data_interface: usize,
        get_private_data: usize,
        get_parent: usize,
        enum_adapters: usize,
        make_window_association: usize,
        get_window_association: usize,
        create_swap_chain: usize,
        create_software_adapter: usize,
        enum_adapters1:
            unsafe extern "system" fn(*mut IdxgiFactory1, u32, *mut *mut IdxgiAdapter1) -> i32,
        is_current: usize,
    }

    #[repr(C)]
    struct IdxgiFactory1 {
        vtbl: *const IdxgiFactory1Vtbl,
    }

    #[link(name = "dxgi")]
    unsafe extern "system" {
        fn CreateDXGIFactory1(riid: *const GUID, pp_factory: *mut *mut c_void) -> i32;
    }

    unsafe {
        let mut factory_ptr: *mut c_void = null_mut();
        let hr = CreateDXGIFactory1(&IID_IDXGI_FACTORY1, &mut factory_ptr);
        if hr != S_OK || factory_ptr.is_null() {
            return Err(io::Error::other(format!(
                "CreateDXGIFactory1 failed with HRESULT 0x{hr:08X}"
            )));
        }
        let factory = factory_ptr as *mut IdxgiFactory1;
        let factory_vtbl = &*(*factory).vtbl;

        let mut gpus = Vec::new();
        let mut index: u32 = 0;

        loop {
            let mut adapter: *mut IdxgiAdapter1 = null_mut();
            let hr_enum = (factory_vtbl.enum_adapters1)(factory, index, &mut adapter);
            if hr_enum == DXGI_ERROR_NOT_FOUND {
                break;
            }
            if hr_enum != S_OK || adapter.is_null() {
                break;
            }
            index += 1;

            let adapter_vtbl = &*(*adapter).vtbl;
            let mut desc: DxgiAdapterDesc1 = zeroed();
            let hr_desc = (adapter_vtbl.get_desc1)(adapter, &mut desc);
            let _ = (adapter_vtbl.release)(adapter);

            if hr_desc != S_OK {
                continue;
            }

            let is_software = (desc.flags & DXGI_ADAPTER_FLAG_SOFTWARE) != 0;
            if is_software || desc.vendor_id == VENDOR_MICROSOFT {
                continue;
            }

            let name_len = desc
                .description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(desc.description.len());
            let name = String::from_utf16_lossy(&desc.description[..name_len])
                .trim()
                .to_string();

            let dedicated_vram_bytes = desc.dedicated_video_memory as u64;
            let shared_system_bytes = desc.shared_system_memory as u64;
            let gfx_target = infer_gfx_target(&name, desc.vendor_id);
            let luid_str = Some(
                format!(
                    "0x{:08x}_0x{:08x}",
                    desc.adapter_luid.high_part as u32, desc.adapter_luid.low_part
                )
                .to_lowercase(),
            );

            gpus.push(GpuInfo {
                name,
                vendor_id: desc.vendor_id,
                device_id: desc.device_id,
                dedicated_vram_bytes,
                shared_system_bytes,
                gfx_target,
                luid_str,
            });
        }

        let _ = (factory_vtbl.release)(factory);
        gpus.sort_by_key(|a| std::cmp::Reverse(a.dedicated_vram_bytes));
        Ok(gpus)
    }
}

#[cfg(not(windows))]
pub fn detect_gpus() -> io::Result<Vec<GpuInfo>> {
    let _ = (
        VENDOR_AMD,
        VENDOR_MICROSOFT,
        infer_gfx_target as fn(&str, u32) -> Option<String>,
    );
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_rx_7800_xt_to_gfx110x() {
        let target = infer_gfx_target("AMD Radeon RX 7800 XT", VENDOR_AMD);
        assert_eq!(target.as_deref(), Some("gfx110X"));
    }

    #[test]
    fn ignores_non_amd_gpu() {
        let target = infer_gfx_target("NVIDIA GeForce RTX 4080", 0x10DE);
        assert_eq!(target, None);
    }

    #[test]
    fn computes_mib_correctly() {
        let gpu = GpuInfo {
            name: "AMD Radeon RX 7800 XT".to_string(),
            vendor_id: VENDOR_AMD,
            device_id: 0x747E,
            dedicated_vram_bytes: 16_368 * BYTES_PER_MIB,
            shared_system_bytes: 16_384 * BYTES_PER_MIB,
            gfx_target: Some("gfx110X".to_string()),
            luid_str: Some("0x00000000_0x0001245a".to_string()),
        };
        assert_eq!(gpu.dedicated_vram_mib(), 16_368);
        assert_eq!(gpu.shared_system_mib(), 16_384);
    }
}
