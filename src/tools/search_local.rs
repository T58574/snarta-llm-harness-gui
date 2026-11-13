use crate::tools::security::{is_protected_path, resolve_user_path};
use globset::{GlobBuilder, GlobMatcher};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

#[allow(dead_code)]
pub const DEFAULT_GLOB_LIMIT: usize = 100;
#[allow(dead_code)]
pub const DEFAULT_GREP_LIMIT: usize = 100;
pub const MAX_FILE_SIZE_FOR_GREP: u64 = 2 * 1024 * 1024; // 2 MB
const MAX_FILE_SIZE_FOR_LINE_COUNT: u64 = 5 * 1024 * 1024; // 5 MB

pub fn harness_glob(
    cwd: &Path,
    life_dir: &Path,
    pattern: &str,
    sub_path: Option<&str>,
    limit: usize,
) -> io::Result<String> {
    let base_dir = resolve_user_path(cwd, life_dir, sub_path.unwrap_or("."))?;
    if !base_dir.exists() || !base_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "DIRECTORY_NOT_FOUND: '{}' is not a valid directory",
                base_dir.display()
            ),
        ));
    }

    // Normalize separators: Windows agents might pass `src\*.rs`.
    let pat = {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            "*".to_string()
        } else {
            trimmed.replace('\\', "/")
        }
    };

    let matcher = PatternMatcher::new(&pat)?;
    let mut results = Vec::new();

    collect_glob_entries(&base_dir, &base_dir, &matcher, 0, limit, &mut results)?;

    if results.is_empty() {
        return Ok(format!("No matches found for pattern '{pat}'."));
    }

    results.sort();
    let total = results.len();
    let suffix = if total >= limit {
        format!("\n... (showing first {limit} matches)")
    } else {
        String::new()
    };

    Ok(format!(
        "{}\nTotal matches: {total}{suffix}",
        results.join("\n")
    ))
}

/// Compiled glob matcher and tree traversal rules.
///
/// Semantics:
/// * pattern without `/` (`*.rs`, `main.rs`, `*`) matches filename at any depth (gitignore-style);
/// * pattern with `/` (`src/*.rs`, `**/*.rs`, `a/b/c.rs`) matches relative path from `base_dir`.
struct PatternMatcher {
    glob: GlobMatcher,
    match_basename: bool,
    /// Maximum traversal depth if pattern does not contain `**`.
    /// `None` indicates unbounded traversal (basename patterns and `**`).
    max_depth: Option<usize>,
    skip_hidden: bool,
    skip_build_dirs: bool,
}

impl PatternMatcher {
    fn new(pattern: &str) -> io::Result<Self> {
        let glob = GlobBuilder::new(pattern)
            // `*` and `?` must not cross `/`, otherwise `src/*.rs` would match `src/foo/bar.rs`.
            .literal_separator(true)
            // Separators already normalized `\` -> `/`; backslash escape not needed on Windows.
            .backslash_escape(false)
            .build()
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("INVALID_GLOB_PATTERN: {e}"),
                )
            })?
            .compile_matcher();

        let match_basename = !pattern.contains('/');
        let has_recursive = pattern.contains("**");
        let max_depth = if match_basename || has_recursive {
            None
        } else {
            Some(pattern.split('/').filter(|s| !s.is_empty()).count())
        };
        let skip_hidden = !pattern.split('/').any(|s| s.starts_with('.'));
        let skip_build_dirs = !pattern.contains("target") && !pattern.contains("node_modules");

        Ok(Self {
            glob,
            match_basename,
            max_depth,
            skip_hidden,
            skip_build_dirs,
        })
    }

    fn matches(&self, rel_path: &str, file_name: &str) -> bool {
        if self.match_basename {
            self.glob.is_match(file_name)
        } else {
            self.glob.is_match(rel_path)
        }
    }
}

fn collect_glob_entries(
    root: &Path,
    dir: &Path,
    matcher: &PatternMatcher,
    depth: usize,
    limit: usize,
    results: &mut Vec<String>,
) -> io::Result<()> {
    if results.len() >= limit {
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        if results.len() >= limit {
            break;
        }

        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        if matcher.skip_hidden && name.starts_with('.') {
            continue;
        }
        if matcher.skip_build_dirs && (name == "target" || name == "node_modules") {
            continue;
        }

        // `file_type()` from readdir is cheaper than `path.is_dir()` + `metadata()`,
        // and avoids traversing symlinks (prevents cycles).
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        let is_match = matcher.matches(&rel, &name);

        if file_type.is_dir() {
            if is_match {
                results.push(format!("{rel}/"));
            }
            let may_descend = match matcher.max_depth {
                None => true,
                Some(max) => depth + 1 < max,
            };
            if may_descend {
                let _ = collect_glob_entries(root, &path, matcher, depth + 1, limit, results);
            }
        } else if file_type.is_file() && is_match {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if size < MAX_FILE_SIZE_FOR_LINE_COUNT {
                match fs::read_to_string(&path) {
                    Ok(c) => {
                        let lines = c.lines().count();
                        results.push(format!("{rel} ({lines} lines, {size} bytes)"));
                    }
                    Err(_) => results.push(format!("{rel} ({size} bytes)")),
                }
            } else {
                results.push(format!("{rel} ({size} bytes)"));
            }
        }
    }

    Ok(())
}

pub fn harness_grep(
    cwd: &Path,
    life_dir: &Path,
    query: &str,
    sub_path: Option<&str>,
    limit: usize,
) -> io::Result<String> {
    let clean_query = query.trim();
    if clean_query.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "query is required",
        ));
    }

    let base_dir = resolve_user_path(cwd, life_dir, sub_path.unwrap_or("."))?;
    if !base_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("PATH_NOT_FOUND: '{}' does not exist", base_dir.display()),
        ));
    }

    if let Ok(rg_output) = run_ripgrep(&base_dir, clean_query, limit)
        && !rg_output.trim().is_empty()
    {
        return Ok(rg_output);
    }

    let mut matches = Vec::new();
    walk_and_grep(&base_dir, &base_dir, clean_query, limit, &mut matches)?;

    if matches.is_empty() {
        return Ok(format!(
            "No matches found for query \"{clean_query}\"."
        ));
    }

    let count = matches.len();
    let note = if count >= limit {
        format!("\n... (showing maximum {limit} matches)")
    } else {
        String::new()
    };

    Ok(format!(
        "{}\nTotal matches: {count}{note}",
        matches.join("\n")
    ))
}

fn run_ripgrep(base_dir: &Path, query: &str, limit: usize) -> io::Result<String> {
    let mut cmd = Command::new("rg");
    cmd.current_dir(base_dir)
        .arg("--line-number")
        .arg("--no-heading")
        .arg("--color=never")
        .arg("--max-count")
        .arg(format!("{limit}"))
        .arg("-e")
        .arg(query)
        .arg(".");

    let output = cmd.output()?;
    if !output.status.success() && output.stdout.is_empty() {
        return Err(io::Error::other("rg returned no output"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn walk_and_grep(
    root: &Path,
    dir: &Path,
    query: &str,
    limit: usize,
    results: &mut Vec<String>,
) -> io::Result<()> {
    if results.len() >= limit {
        return Ok(());
    }

    if is_protected_path(dir) {
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        if results.len() >= limit {
            break;
        }

        let p = entry.path();
        let name = p.file_name().unwrap_or_default().to_string_lossy();

        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            let _ = walk_and_grep(root, &p, query, limit, results);
        } else if file_type.is_file() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.len() > MAX_FILE_SIZE_FOR_GREP {
                continue;
            }
            let content = match fs::read_to_string(&p) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");

            for (idx, line) in content.lines().enumerate() {
                if line.contains(query) {
                    let line_no = idx + 1;
                    let trimmed = line.trim();
                    results.push(format!("{rel}:{line_no}: {trimmed}"));
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}