pub mod optimize;
pub mod procs;

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

#[cfg(test)]
mod tests {
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
}
