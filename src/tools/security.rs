use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const DANGEROUS_CMD_PATTERNS: &[&str] = &[
    "format ",
    "diskpart",
    "cipher /w",
    "del /s",
    "del /f /s",
    "rd /s",
    "rmdir /s",
    "remove-item -recurse",
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "rm -rf c:",
    "bcdedit",
    "vssadmin delete",
    "wbadmin delete",
    "reg delete hklm",
    "reg delete hkcu\\software\\microsoft\\windows",
    "shutdown",
    "restart-computer",
    "stop-computer",
    "mkfs",
    "dd if=",
    ":(){ :|:& };:",
];

pub const PROTECTED_DELETE_DIRS: &[&str] = &[
    "c:/windows",
    "c:/program files",
    "c:/program files (x86)",
    "c:/programdata",
    "/bin",
    "/sbin",
    "/usr",
    "/etc",
    "/boot",
    "/lib",
    "/proc",
    "/sys",
    "/dev",
];

pub fn is_dangerous_command(cmd: &str) -> Option<&'static str> {
    let norm: String = cmd
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    DANGEROUS_CMD_PATTERNS
        .iter()
        .find(|&&pat| norm.contains(pat))
        .copied()
}

fn normalize_lexical_for_check(raw: &str) -> String {
    let mut s = raw.replace('\\', "/").to_ascii_lowercase();
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in s.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.len() > 1 || (parts.len() == 1 && !parts[0].ends_with(':')) {
                    parts.pop();
                }
            }
            other => parts.push(other),
        }
    }
    if s.starts_with('/') {
        format!("/{}", parts.join("/"))
    } else {
        parts.join("/")
    }
}

fn matches_protected_dir(norm: &str) -> bool {
    let trimmed = norm.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" || trimmed == "c:" || trimmed == "d:" {
        return true;
    }
    PROTECTED_DELETE_DIRS
        .iter()
        .any(|&d| trimmed == d || trimmed.starts_with(&format!("{d}/")))
}

pub fn is_protected_path(target: &Path) -> bool {
    let lexical = normalize_lexical_for_check(&target.to_string_lossy());
    if matches_protected_dir(&lexical) {
        return true;
    }
    if let Ok(canon) = fs::canonicalize(target) {
        let canon_norm = normalize_lexical_for_check(&canon.to_string_lossy());
        if matches_protected_dir(&canon_norm) {
            return true;
        }
    }
    false
}

pub fn resolve_user_path(cwd: &Path, life_dir: &Path, raw: &str) -> io::Result<PathBuf> {
    let t = raw.trim().replace('\\', "/");
    if t.is_empty() || t == "." {
        return Ok(cwd.to_path_buf());
    }
    if t == "~" || t.starts_with("~/") {
        let h = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| cwd.to_path_buf());
        return Ok(if t == "~" { h } else { h.join(&t[2..]) });
    }
    if let Some(sub) = t.strip_prefix("life/") {
        return Ok(life_dir.join(sub));
    }
    let p = PathBuf::from(&t);
    if p.is_absolute() || t.contains(':') {
        return Ok(p);
    }
    let in_cwd = cwd.join(&t);
    if in_cwd.exists() {
        return Ok(in_cwd);
    }
    let in_life = life_dir.join(&t);
    if in_life.exists() {
        return Ok(in_life);
    }
    Ok(in_cwd)
}

pub fn resolve_inside_life(life_dir: &Path, user_rel: &str) -> io::Result<PathBuf> {
    let trimmed = user_rel.trim().replace('\\', "/");
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.contains(':') {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "invalid path",
        ));
    }
    let rel = Path::new(&trimmed);
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "path escapes life/",
        ));
    }
    fs::create_dir_all(life_dir)?;
    let cl = fs::canonicalize(life_dir)?;
    let cand = cl.join(rel);

    // Reject any symlink along the relative path inside life/ (including dangling symlinks)
    let mut cur = cl.clone();
    for comp in rel.components() {
        cur.push(comp);
        if let Ok(meta) = fs::symlink_metadata(&cur)
            && meta.file_type().is_symlink()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "symlinks are not allowed inside life/",
            ));
        }
    }

    let mut check = cand.as_path();
    while !check.exists() {
        check = check
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "invalid path"))?;
    }
    if !fs::canonicalize(check)?.starts_with(&cl) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "path escapes life/",
        ));
    }
    Ok(cand)
}
