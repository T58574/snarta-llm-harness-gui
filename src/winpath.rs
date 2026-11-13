use std::io;
use std::path::Path;

// The helpers below are string manipulations on semicolon-separated PATH
// values, as used by the Windows registry. They are cross-platform pure
// functions (which is why they are tested on every platform), but they are
// only invoked from the Windows code paths in production.
#[allow(dead_code)]
pub fn add_entry_to_path_str(current: &str, dir: &str) -> (String, bool) {
    let dir_trimmed = dir.trim();
    if dir_trimmed.is_empty() {
        return (current.to_string(), false);
    }
    let normalized_target = normalize_win_path(dir_trimmed);

    let mut parts: Vec<&str> = current
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if parts
        .iter()
        .any(|entry| normalize_win_path(entry) == normalized_target)
    {
        return (current.to_string(), false);
    }

    parts.push(dir_trimmed);
    (parts.join(";"), true)
}

#[allow(dead_code)]
pub fn remove_entry_from_path_str(current: &str, dir: &str) -> (String, bool) {
    let normalized_target = normalize_win_path(dir);
    let mut removed = false;
    let kept: Vec<&str> = current
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|entry| {
            if normalize_win_path(entry) == normalized_target {
                removed = true;
                false
            } else {
                true
            }
        })
        .collect();

    if !removed {
        // Return the original string verbatim when nothing was removed, so
        // callers can trust the (value, false) == "no change" contract.
        return (current.to_string(), false);
    }
    (kept.join(";"), true)
}

#[allow(dead_code)]
pub fn contains_entry_in_path_str(current: &str, dir: &str) -> bool {
    let normalized_target = normalize_win_path(dir);
    current
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .any(|entry| normalize_win_path(entry) == normalized_target)
}

fn normalize_win_path(p: &str) -> String {
    let trimmed = p.trim().trim_end_matches(['\\', '/']);
    // Preserve drive roots: "C:" means "current directory on C:", which is
    // not the same as "C:\". Keep the trailing separator for that case.
    let rooted: String = if trimmed.len() == 2 && trimmed.ends_with(':') {
        format!("{trimmed}\\")
    } else {
        trimmed.to_string()
    };
    rooted.replace('/', "\\").to_ascii_lowercase()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS, HWND,
    };
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_SZ, RegCloseKey,
        RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };

    const ENV_SUBKEY: &str = "Environment";
    const PATH_VALUE_NAME: &str = "Path";
    const BROADCAST_TIMEOUT_MS: u32 = 2000;
    const READ_RETRIES: u32 = 3;

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Normalizes `reg_type` to one of `REG_SZ` / `REG_EXPAND_SZ` so that
    /// unsupported types can never be written back as the PATH value.
    fn sanitize_reg_type(reg_type: u32) -> u32 {
        if reg_type == REG_SZ || reg_type == REG_EXPAND_SZ {
            reg_type
        } else {
            REG_EXPAND_SZ
        }
    }

    pub fn read_user_path() -> io::Result<(String, u32)> {
        unsafe {
            let subkey = to_wide(ENV_SUBKEY);
            let mut hkey: HKEY = null_mut();
            let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey);
            if status != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(status as i32));
            }

            let val_name = to_wide(PATH_VALUE_NAME);
            let mut val_type: u32 = REG_EXPAND_SZ;
            let mut byte_len: u32 = 0;

            // First call: query the type and the required size.
            let q1 = RegQueryValueExW(
                hkey,
                val_name.as_ptr(),
                null(),
                &mut val_type,
                null_mut(),
                &mut byte_len,
            );
            if q1 == ERROR_FILE_NOT_FOUND {
                // No Path value yet: that is a valid empty state.
                let _ = RegCloseKey(hkey);
                return Ok((String::new(), REG_EXPAND_SZ));
            }
            if q1 != ERROR_SUCCESS || byte_len == 0 {
                let _ = RegCloseKey(hkey);
                return Err(io::Error::from_raw_os_error(q1 as i32));
            }

            // Second call: read the actual value. The size query and the
            // value read are two separate registry calls, so another process
            // (or another snarta instance) may have grown the value in
            // between. Retry with the new size on ERROR_MORE_DATA.
            let mut buf: Vec<u16> = Vec::new();
            let mut last_status: u32 = ERROR_SUCCESS;
            for attempt in 0..READ_RETRIES {
                let u16_len = (byte_len as usize).div_ceil(2);
                buf = vec![0u16; u16_len];

                let mut read_len = byte_len;
                let q2 = RegQueryValueExW(
                    hkey,
                    val_name.as_ptr(),
                    null(),
                    &mut val_type,
                    buf.as_mut_ptr() as *mut u8,
                    &mut read_len,
                );

                match q2 {
                    ERROR_SUCCESS => {
                        last_status = ERROR_SUCCESS;
                        break;
                    }
                    ERROR_MORE_DATA if attempt + 1 < READ_RETRIES => {
                        // Value grew: `read_len` now holds the new required
                        // size. Retry with a buffer of that size.
                        byte_len = read_len;
                        last_status = ERROR_MORE_DATA;
                        continue;
                    }
                    other => {
                        let _ = RegCloseKey(hkey);
                        return Err(io::Error::from_raw_os_error(other as i32));
                    }
                }
            }

            let _ = RegCloseKey(hkey);
            if last_status != ERROR_SUCCESS {
                // We exhausted the retries without a stable value.
                return Err(io::Error::from_raw_os_error(last_status as i32));
            }

            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let value = String::from_utf16_lossy(&buf[..end]);
            Ok((value, sanitize_reg_type(val_type)))
        }
    }

    pub fn write_user_path(new_path: &str, reg_type: u32) -> io::Result<()> {
        unsafe {
            let subkey = to_wide(ENV_SUBKEY);
            let mut hkey: HKEY = null_mut();
            let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_WRITE, &mut hkey);
            if status != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(status as i32));
            }

            let val_name = to_wide(PATH_VALUE_NAME);
            let wide_val = to_wide(new_path);
            let byte_len = (wide_val.len() * 2) as u32;

            let set_status = RegSetValueExW(
                hkey,
                val_name.as_ptr(),
                0,
                sanitize_reg_type(reg_type),
                wide_val.as_ptr() as *const u8,
                byte_len,
            );
            let _ = RegCloseKey(hkey);
            if set_status != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(set_status as i32));
            }

            broadcast_env_change();
            Ok(())
        }
    }

    fn broadcast_env_change() {
        unsafe {
            let env_wide = to_wide(ENV_SUBKEY);
            let mut result: usize = 0;
            let ok = SendMessageTimeoutW(
                HWND_BROADCAST as HWND,
                WM_SETTINGCHANGE,
                0,
                env_wide.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                BROADCAST_TIMEOUT_MS,
                &mut result,
            );
            if ok == 0 {
                // Best-effort: some windows may not be listening, and a
                // broadcast failure must not prevent the PATH write from
                // being committed.
                eprintln!("snarta: WM_SETTINGCHANGE broadcast timed out or was rejected");
            }
        }
    }
}

#[cfg(windows)]
pub fn is_installed(exe_dir: &Path) -> io::Result<bool> {
    let dir_str = exe_dir.to_string_lossy();
    if dir_str.trim().is_empty() {
        return Ok(false);
    }
    let (current, _) = imp::read_user_path()?;
    Ok(contains_entry_in_path_str(&current, &dir_str))
}

#[cfg(not(windows))]
pub fn is_installed(exe_dir: &Path) -> io::Result<bool> {
    let dir_str = exe_dir.to_string_lossy();
    let target = dir_str.trim_end_matches('/');
    if target.is_empty() {
        return Ok(false);
    }
    let current = std::env::var("PATH").unwrap_or_default();
    Ok(current
        .split(':')
        .map(|s| s.trim().trim_end_matches('/'))
        .any(|s| s == target))
}

#[cfg(windows)]
pub fn install(exe_dir: &Path) -> io::Result<bool> {
    let dir_str = exe_dir.to_string_lossy();
    if dir_str.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "executable directory is empty",
        ));
    }
    let (current, reg_type) = imp::read_user_path()?;
    let (updated, changed) = add_entry_to_path_str(&current, &dir_str);
    if changed {
        imp::write_user_path(&updated, reg_type)?;
    }
    Ok(changed)
}

#[cfg(not(windows))]
pub fn install(_exe_dir: &Path) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "snarta path install is only supported on Windows",
    ))
}

#[cfg(windows)]
pub fn uninstall(exe_dir: &Path) -> io::Result<bool> {
    let dir_str = exe_dir.to_string_lossy();
    if dir_str.trim().is_empty() {
        return Ok(false);
    }
    let (current, reg_type) = imp::read_user_path()?;
    let (updated, changed) = remove_entry_from_path_str(&current, &dir_str);
    if changed {
        imp::write_user_path(&updated, reg_type)?;
    }
    Ok(changed)
}

#[cfg(not(windows))]
pub fn uninstall(_exe_dir: &Path) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "snarta path uninstall is only supported on Windows",
    ))
}