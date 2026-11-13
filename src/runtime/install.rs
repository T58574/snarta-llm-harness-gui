use crate::runtime::releases::{BackendKind, ReleaseCandidate, USER_AGENT};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const SERVER_EXE_NAME: &str = "llama-server.exe";
pub const HIP_BACKEND_DLL: &str = "ggml-hip.dll";
pub const ROCBLAS_DLL: &str = "rocblas.dll";
pub const VULKAN_BACKEND_DLL: &str = "ggml-vulkan.dll";
pub const DOWNLOAD_TIMEOUT_SECS: u64 = 600;
pub const STREAM_CHUNK_BYTES: usize = 64 * 1024;
pub const BYTES_PER_MIB: f64 = 1024.0 * 1024.0;

#[derive(Debug, Clone)]
pub struct InstalledBuild {
    pub build_id: String,
    pub dir: PathBuf,
    pub server_exe: PathBuf,
    pub backend: BackendKind,
    pub size_bytes: u64,
}

pub fn compute_dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if p.is_dir() {
                total += compute_dir_size(&p);
            }
        }
    }
    total
}

pub fn format_bytes(bytes: u64) -> String {
    let mib = bytes as f64 / BYTES_PER_MIB;
    if mib >= 1024.0 {
        format!("{:.2} GiB", mib / 1024.0)
    } else {
        format!("{:.1} MiB", mib)
    }
}

pub fn remove_old_builds(
    runtime_dir: &Path,
    active_build: Option<&str>,
) -> io::Result<(usize, u64)> {
    let builds = list_installed(runtime_dir)?;
    let mut count = 0;
    let mut freed = 0;
    for b in builds {
        if active_build != Some(&b.build_id) && fs::remove_dir_all(&b.dir).is_ok() {
            count += 1;
            freed += b.size_bytes;
        }
    }
    Ok((count, freed))
}

#[derive(Debug, Clone)]
pub struct VerificationReport {
    pub version_line: String,
    pub devices_output: String,
    pub has_gpu_device: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallProgress {
    Downloading {
        downloaded: u64,
        total: u64,
        pct: u8,
    },
    Extracting,
    Verifying,
    Done,
}

pub fn list_installed(runtime_dir: &Path) -> io::Result<Vec<InstalledBuild>> {
    let mut builds = Vec::new();
    let Ok(entries) = fs::read_dir(runtime_dir) else {
        return Ok(builds);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !path.is_dir() || name.starts_with('.') {
            continue;
        }
        if let Some(server_exe) = find_file_recursive(&path, SERVER_EXE_NAME) {
            let is_hip = find_file_recursive(&path, HIP_BACKEND_DLL).is_some()
                || find_file_recursive(&path, ROCBLAS_DLL).is_some()
                || name.to_ascii_lowercase().contains("rocm")
                || name.to_ascii_lowercase().contains("hip");
            let backend = if is_hip {
                BackendKind::RocmHip
            } else {
                BackendKind::Vulkan
            };
            let size_bytes = compute_dir_size(&path);
            builds.push(InstalledBuild {
                build_id: name.to_string(),
                dir: path,
                server_exe,
                backend,
                size_bytes,
            });
        }
    }

    // Check if runtime_dir itself contains a standalone llama-server.exe
    if builds.is_empty()
        && let Some(server_exe) = find_file_recursive(runtime_dir, SERVER_EXE_NAME)
    {
        let is_hip = find_file_recursive(runtime_dir, HIP_BACKEND_DLL).is_some()
            || find_file_recursive(runtime_dir, ROCBLAS_DLL).is_some();
        let backend = if is_hip {
            BackendKind::RocmHip
        } else {
            BackendKind::Vulkan
        };
        let size_bytes = compute_dir_size(runtime_dir);
        builds.push(InstalledBuild {
            build_id: "local".to_string(),
            dir: runtime_dir.to_path_buf(),
            server_exe,
            backend,
            size_bytes,
        });
    }

    builds.sort_by(|a, b| a.build_id.cmp(&b.build_id));
    Ok(builds)
}

pub fn download_and_install_with_reporter<F>(
    runtime_dir: &Path,
    candidate: &ReleaseCandidate,
    mut on_progress: F,
) -> io::Result<(InstalledBuild, VerificationReport)>
where
    F: FnMut(InstallProgress),
{
    fs::create_dir_all(runtime_dir)?;
    let target_dir = runtime_dir.join(&candidate.build_id);
    let tmp_zip = runtime_dir.join(format!(".{}.download.zip", candidate.build_id));
    let staging_dir = runtime_dir.join(format!(".{}.staging", candidate.build_id));

    if staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }

    download_with_progress(
        &candidate.download_url,
        &tmp_zip,
        candidate.size_bytes,
        |downloaded, total, pct| {
            on_progress(InstallProgress::Downloading {
                downloaded,
                total,
                pct,
            });
        },
    )?;

    on_progress(InstallProgress::Extracting);
    let extract_res = extract_zip_safe(&tmp_zip, &staging_dir);
    let _ = fs::remove_file(&tmp_zip);
    extract_res?;

    on_progress(InstallProgress::Verifying);
    let verify_res = verify_staged(&staging_dir, candidate.backend);
    let report = match verify_res {
        Ok(r) => r,
        Err(e) => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(e);
        }
    };

    if target_dir.exists() {
        fs::remove_dir_all(&target_dir)?;
    }
    fs::rename(&staging_dir, &target_dir)?;

    let server_exe = find_file_recursive(&target_dir, SERVER_EXE_NAME)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "llama-server.exe missing"))?;

    on_progress(InstallProgress::Done);
    let size_bytes = compute_dir_size(&target_dir);
    Ok((
        InstalledBuild {
            build_id: candidate.build_id.clone(),
            dir: target_dir,
            server_exe,
            backend: candidate.backend,
            size_bytes,
        },
        report,
    ))
}

pub fn download_and_install(
    runtime_dir: &Path,
    candidate: &ReleaseCandidate,
) -> io::Result<(InstalledBuild, VerificationReport)> {
    let mut last_pct = 101;
    download_and_install_with_reporter(runtime_dir, candidate, |prog| match prog {
        InstallProgress::Downloading {
            downloaded,
            total,
            pct,
        } => {
            if pct != last_pct && (pct.is_multiple_of(5) || downloaded == total) {
                last_pct = pct;
                let dl_mib = downloaded as f64 / BYTES_PER_MIB;
                let tot_mib = total as f64 / BYTES_PER_MIB;
                print!("\r  Downloading: {dl_mib:5.1} / {tot_mib:5.1} MiB ({pct}%)");
                let _ = io::stdout().flush();
            }
        }
        InstallProgress::Extracting => {
            println!("\r  Extracting archive...                              ");
        }
        InstallProgress::Verifying => {
            println!("  Verifying binary and GPU support...");
        }
        InstallProgress::Done => {}
    })
}

fn verify_staged(staging_dir: &Path, backend: BackendKind) -> io::Result<VerificationReport> {
    check_required_files(staging_dir, backend)?;
    let server_exe = find_file_recursive(staging_dir, SERVER_EXE_NAME)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "llama-server.exe missing"))?;
    verify_server_binary(&server_exe, backend)
}

fn download_with_progress<F>(
    url: &str,
    dest: &Path,
    expected_bytes: u64,
    mut on_progress: F,
) -> io::Result<()>
where
    F: FnMut(u64, u64, u8),
{
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS)))
        .build();
    let agent: ureq::Agent = config.into();
    let mut resp = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| io::Error::other(format!("download failed: {e}")))?;

    let mut reader = resp.body_mut().as_reader();
    let mut file = File::create(dest)?;
    let mut buf = vec![0u8; STREAM_CHUNK_BYTES];
    let mut downloaded: u64 = 0;
    let mut last_pct: u64 = 101;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;

        if let Some(pct) = (downloaded * 100).checked_div(expected_bytes)
            && pct != last_pct
        {
            last_pct = pct;
            on_progress(downloaded, expected_bytes, pct as u8);
        }
    }
    file.flush()
}

pub fn extract_zip_safe(zip_path: &Path, dest_dir: &Path) -> io::Result<()> {
    const MAX_ZIP_ENTRIES: usize = 4096;
    const MAX_ENTRY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    const MAX_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;

    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("invalid zip: {e}")))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zip archive has too many entries",
        ));
    }
    fs::create_dir_all(dest_dir)?;

    let mut total_written: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let Some(rel_path) = entry.enclosed_name() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "zip entry contains unsafe path traversal",
            ));
        };

        let out_path = dest_dir.join(rel_path);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = File::create(&out_path)?;
            let mut limited = io::Read::take(&mut entry, MAX_ENTRY_BYTES + 1);
            let written = io::copy(&mut limited, &mut out_file)?;
            if written > MAX_ENTRY_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "zip entry exceeds maximum allowed size",
                ));
            }
            total_written = total_written.saturating_add(written);
            if total_written > MAX_TOTAL_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "zip archive exceeds total extracted size limit",
                ));
            }
        }
    }
    Ok(())
}

pub fn check_required_files(dir: &Path, backend: BackendKind) -> io::Result<()> {
    if find_file_recursive(dir, SERVER_EXE_NAME).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {SERVER_EXE_NAME} in build"),
        ));
    }
    let required: &[&str] = match backend {
        BackendKind::RocmHip => &[HIP_BACKEND_DLL, ROCBLAS_DLL],
        BackendKind::Vulkan => &[VULKAN_BACKEND_DLL],
    };
    for dll in required {
        if find_file_recursive(dir, dll).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} build is incomplete: missing {dll}", backend.as_str()),
            ));
        }
    }
    Ok(())
}

pub fn verify_server_binary(
    server_exe: &Path,
    backend: BackendKind,
) -> io::Result<VerificationReport> {
    if !cfg!(windows) {
        return Ok(VerificationReport {
            version_line: "skipped on non-Windows host".to_string(),
            devices_output: String::new(),
            has_gpu_device: true,
        });
    }

    let work_dir = server_exe.parent().unwrap_or_else(|| Path::new("."));
    let ver_combined = run_cmd_combined(server_exe, work_dir, "--version", true)?;
    let version_line = ver_combined
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("unknown")
        .trim()
        .to_string();

    let devices_output = run_cmd_combined(server_exe, work_dir, "--list-devices", false)?;
    let has_gpu_device = match backend {
        BackendKind::RocmHip => devices_output.contains("ROCm0") || devices_output.contains("HIP"),
        BackendKind::Vulkan => devices_output.contains("Vulkan0"),
    };

    if !has_gpu_device {
        let msg = format!(
            "llama-server --list-devices missing GPU for {}:\n{}",
            backend.as_str(),
            devices_output.trim()
        );
        return Err(io::Error::other(msg));
    }
    Ok(VerificationReport {
        version_line,
        devices_output: devices_output.trim().to_string(),
        has_gpu_device,
    })
}

fn run_cmd_combined(exe: &Path, dir: &Path, arg: &str, require_ok: bool) -> io::Result<String> {
    let mut cmd = Command::new(exe);
    cmd.current_dir(dir).arg(arg);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output()?;
    if require_ok && !out.status.success() {
        return Err(io::Error::other(format!("llama-server {arg} failed")));
    }
    let s1 = String::from_utf8_lossy(&out.stdout);
    let s2 = String::from_utf8_lossy(&out.stderr);
    Ok(format!("{s1}\n{s2}"))
}

pub fn find_file_recursive(dir: &Path, file_name_lower: &str) -> Option<PathBuf> {
    let target = file_name_lower.to_ascii_lowercase();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.is_file()
            && p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(&target))
        {
            return Some(p);
        }
        if p.is_dir()
            && let Some(found) = find_file_recursive(&p, &target)
        {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn validates_hip_required_dlls() {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let dir = std::env::temp_dir().join(format!("snarta_rt_{}", ts.as_secs()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SERVER_EXE_NAME), b"exe").unwrap();
        assert!(check_required_files(&dir, BackendKind::RocmHip).is_err());
        fs::write(dir.join(HIP_BACKEND_DLL), b"dll").unwrap();
        fs::write(dir.join(ROCBLAS_DLL), b"dll").unwrap();
        assert!(check_required_files(&dir, BackendKind::RocmHip).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }
}
