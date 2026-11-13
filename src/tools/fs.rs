use crate::tools::security::{is_protected_path, resolve_inside_life, resolve_user_path};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Ledger for tracking files read during the current agent session.
/// Enforces the read-before-edit pattern to prevent blind hallucinated edits.
pub struct ReadLedger {
    read_paths: Mutex<HashSet<PathBuf>>,
}

impl Default for ReadLedger {
    fn default() -> Self {
        Self {
            read_paths: Mutex::new(HashSet::new()),
        }
    }
}

impl ReadLedger {
    pub fn global() -> &'static ReadLedger {
        static LEDGER: OnceLock<ReadLedger> = OnceLock::new();
        LEDGER.get_or_init(ReadLedger::default)
    }

    pub fn record_read(&self, path: &Path) {
        let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Ok(mut set) = self.read_paths.lock() {
            set.insert(canon);
        }
    }

    pub fn has_read(&self, path: &Path) -> bool {
        let cand = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Ok(set) = self.read_paths.lock() {
            set.contains(&cand)
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        if let Ok(mut set) = self.read_paths.lock() {
            set.clear();
        }
    }
}

/// Options for reading files with pagination and offset support.
#[derive(Debug, Clone, Default)]
pub struct ReadFileOptions {
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub byte_offset: Option<usize>,
    pub limit: Option<usize>,
}

pub fn harness_read_file(
    cwd: &Path,
    life_dir: &Path,
    rel_path: &str,
    opts: ReadFileOptions,
    max_bytes: u64,
) -> io::Result<String> {
    let target = resolve_user_path(cwd, life_dir, rel_path)?;
    if !target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("FILE_NOT_FOUND: file '{}' does not exist", target.display()),
        ));
    }
    if target.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IS_DIRECTORY: '{}' is a directory, not a file",
                target.display()
            ),
        ));
    }

    let meta = fs::metadata(&target)?;
    let total_len = meta.len();
    if total_len > max_bytes && opts.start_line.is_none() && opts.byte_offset.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "FILE_TOO_LARGE: file is {total_len} bytes (limit: {max_bytes} bytes). Use start_line/end_line or byte_offset pagination."
            ),
        ));
    }

    let raw = fs::read_to_string(&target)?;
    ReadLedger::global().record_read(&target);

    // Apply byte_offset if provided
    let slice = if let Some(offset) = opts.byte_offset {
        if offset >= raw.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "OUT_OF_BOUNDS: byte_offset {offset} exceeds file size {}",
                    raw.len()
                ),
            ));
        }
        let mut valid_offset = offset;
        while !raw.is_char_boundary(valid_offset) && valid_offset < raw.len() {
            valid_offset += 1;
        }
        &raw[valid_offset..]
    } else {
        &raw[..]
    };

    let all_lines: Vec<&str> = slice.lines().collect();
    let total_lines = all_lines.len();

    let start = opts.start_line.unwrap_or(1).max(1);
    let end = opts.end_line.unwrap_or(total_lines).min(total_lines);

    if start > total_lines && total_lines > 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("OUT_OF_BOUNDS: start_line {start} exceeds total lines {total_lines}"),
        ));
    }

    let limit = opts.limit.unwrap_or(usize::MAX);
    let mut out = String::new();
    let mut count = 0;

    let line_offset = if opts.byte_offset.is_some() {
        0
    } else {
        start.saturating_sub(1)
    };
    let slice_lines = if total_lines == 0 {
        &[]
    } else {
        let actual_end = if let Some(lim) = opts.limit {
            end.min(start.saturating_add(lim.saturating_sub(1)))
        } else {
            end
        };
        let s_idx = start.saturating_sub(1);
        let e_idx = actual_end.min(total_lines);
        if s_idx < e_idx {
            &all_lines[s_idx..e_idx]
        } else {
            &[]
        }
    };

    let width = format!("{}", line_offset + slice_lines.len()).len().max(3);

    for (idx, line) in slice_lines.iter().enumerate() {
        let line_num = line_offset + idx + 1;
        out.push_str(&format!("{line_num:>width$} | {line}\n"));
        count += 1;
        if count >= limit {
            break;
        }
    }

    if out.is_empty() && total_lines == 0 {
        return Ok("(file is empty)".to_string());
    }

    Ok(out.trim_end().to_string())
}

pub fn harness_write_file(
    cwd: &Path,
    life_dir: &Path,
    rel_path: &str,
    content: &str,
    append: bool,
) -> io::Result<String> {
    let clean = rel_path.trim();
    if clean.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is required",
        ));
    }
    let target = resolve_user_path(cwd, life_dir, clean)?;
    if is_protected_path(&target) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "PERMISSION_DENIED: writing to protected path '{}' is forbidden",
                target.display()
            ),
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    if append {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)?
            .write_all(content.as_bytes())?;
    } else {
        fs::write(&target, content)?;
    }

    ReadLedger::global().record_read(&target);
    Ok(format!(
        "File saved: {} ({} bytes)",
        target.display(),
        content.len()
    ))
}

pub fn harness_edit_file(
    cwd: &Path,
    life_dir: &Path,
    rel_path: &str,
    old_text: &str,
    new_text: &str,
) -> io::Result<String> {
    let clean = rel_path.trim();
    if clean.is_empty() || old_text.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path and old_text parameters are required",
        ));
    }
    let target = resolve_user_path(cwd, life_dir, clean)?;
    if is_protected_path(&target) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "PERMISSION_DENIED: editing protected path '{}' is forbidden",
                target.display()
            ),
        ));
    }
    if !target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("FILE_NOT_FOUND: file '{}' does not exist", target.display()),
        ));
    }

    // Ledger check
    let has_read = ReadLedger::global().has_read(&target);
    let file_content = fs::read_to_string(&target)?;

    // 1. Check exact occurrences
    let occurrences = file_content.match_indices(old_text).count();
    if occurrences == 0 {
        // Tolerant match 1: line endings mismatch (CRLF vs LF)
        let norm_crlf_file = file_content.replace("\r\n", "\n");
        let norm_crlf_old = old_text.replace("\r\n", "\n");
        let crlf_matches: Vec<_> = norm_crlf_file.match_indices(&norm_crlf_old).collect();
        if crlf_matches.len() == 1 {
            let norm_crlf_new = new_text.replace("\r\n", "\n");
            let mut updated = norm_crlf_file.replacen(&norm_crlf_old, &norm_crlf_new, 1);
            if file_content.contains("\r\n") {
                updated = updated.replace('\n', "\r\n");
            }
            fs::write(&target, updated)?;
            ReadLedger::global().record_read(&target);
            let ledger_notice = if !has_read {
                " [Warning: file was edited without prior read in this session]"
            } else {
                ""
            };
            return Ok(format!(
                "File edited successfully (normalized line endings): {}{ledger_notice}",
                target.display()
            ));
        }

        // Tolerant match 2: whitespace/indentation tolerant line sequence match
        let file_lines: Vec<&str> = file_content.lines().collect();
        let old_lines: Vec<&str> = old_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();

        if !old_lines.is_empty() {
            let mut match_starts = Vec::new();
            for i in 0..=file_lines.len().saturating_sub(old_lines.len()) {
                let mut matched = true;
                for (j, old_l) in old_lines.iter().enumerate() {
                    if file_lines[i + j].trim() != *old_l {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    match_starts.push(i);
                }
            }

            if match_starts.len() == 1 {
                let start_line = match_starts[0];
                let end_line = start_line + old_lines.len();
                let le = if file_content.contains("\r\n") {
                    "\r\n"
                } else {
                    "\n"
                };
                let mut new_lines = Vec::new();
                new_lines.extend_from_slice(&file_lines[..start_line]);
                new_lines.push(new_text.trim_end_matches(['\r', '\n']));
                new_lines.extend_from_slice(&file_lines[end_line..]);
                let mut updated = new_lines.join(le);
                if file_content.ends_with(le) {
                    updated.push_str(le);
                }
                fs::write(&target, updated)?;
                ReadLedger::global().record_read(&target);
                let ledger_notice = if !has_read {
                    " [Warning: file was edited without prior read in this session]"
                } else {
                    ""
                };
                return Ok(format!(
                    "File edited successfully (normalized indentation lines {}-{}): {}{ledger_notice}",
                    start_line + 1,
                    end_line,
                    target.display()
                ));
            } else if match_starts.len() > 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "OLD_STRING_NOT_UNIQUE: found {} line-trimmed occurrences in {}. Please provide more surrounding lines.",
                        match_starts.len(),
                        target.display()
                    ),
                ));
            }
        }

        // Attempt fuzzy diagnosis (normalized whitespace and line endings)
        let norm_file = normalize_whitespace(&file_content);
        let norm_old = normalize_whitespace(old_text);
        let fuzzy_hint = if norm_file.contains(&norm_old) {
            "\nHint: match found with whitespace/newline normalization. Check exact indentation and line endings."
        } else if let Some(similar_line) = find_best_line_match(&file_content, old_text) {
            &format!("\nHint: similar line in file:\n  \"{similar_line}\"")
        } else {
            ""
        };

        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "NOT_FOUND: old_text not found in {}.{}",
                target.display(),
                fuzzy_hint
            ),
        ));
    }

    if occurrences > 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "OLD_STRING_NOT_UNIQUE: found {occurrences} occurrences of old_text in {}. Please provide more surrounding lines for a unique match.",
                target.display()
            ),
        ));
    }

    // Single exact match -> replace
    let updated = file_content.replacen(old_text, new_text, 1);
    fs::write(&target, updated)?;
    ReadLedger::global().record_read(&target);

    let ledger_notice = if !has_read {
        " [Warning: file was edited without prior read in this session]"
    } else {
        ""
    };

    Ok(format!(
        "File edited successfully: {}{ledger_notice}",
        target.display()
    ))
}

pub fn harness_delete_file(cwd: &Path, life_dir: &Path, rel_path: &str) -> io::Result<String> {
    let clean = rel_path.trim();
    if clean.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is required",
        ));
    }
    let target = resolve_user_path(cwd, life_dir, clean)?;
    if is_protected_path(&target) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "PERMISSION_DENIED: deletion in protected system directory is forbidden",
        ));
    }
    if !target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("FILE_NOT_FOUND: file '{}' not found", clean),
        ));
    }
    if target.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "delete_file only removes files, not directories",
        ));
    }
    fs::remove_file(&target)?;
    Ok(format!("File deleted: {}", target.display()))
}

pub fn remember_fact(life_dir: &Path, text: &str) -> io::Result<String> {
    let raw = text.trim();
    if raw.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "text is required",
        ));
    }
    let one_line = raw.lines().map(str::trim).collect::<Vec<_>>().join(" ");
    let entry = format!("- [{}] {one_line}", crate::log::date_stamp_utc());
    let mem_path = resolve_inside_life(life_dir, "memory.md")?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&mem_path)?
        .write_all(format!("{entry}\n").as_bytes())?;
    Ok(entry)
}

pub fn write_note_doc(
    life_dir: &Path,
    rel_path: &str,
    content: &str,
    append: bool,
) -> io::Result<String> {
    let rel = rel_path.trim();
    let sub = rel.strip_prefix("notes/").unwrap_or(rel);
    let target = resolve_inside_life(life_dir, &format!("notes/{sub}"))?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let (cp, cl) = (
        fs::canonicalize(target.parent().unwrap())?,
        fs::canonicalize(life_dir)?,
    );
    if !cp.starts_with(&cl) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "symlink escape from life/",
        ));
    }
    if append {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)?
            .write_all(format!("{content}\n").as_bytes())?;
    } else {
        fs::write(&target, content)?;
    }
    Ok(format!("Note saved: notes/{sub}"))
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_best_line_match<'a>(content: &'a str, target: &str) -> Option<&'a str> {
    let target_clean = target.trim();
    let target_first = target_clean.lines().next()?.trim();
    if target_first.is_empty() {
        return None;
    }
    content.lines().map(str::trim).find(|line| {
        !line.is_empty() && (line.contains(target_first) || target_first.contains(line))
    })
}
