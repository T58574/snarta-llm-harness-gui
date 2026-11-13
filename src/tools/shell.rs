use crate::tools::security::is_dangerous_command;
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const EXEC_TIMEOUT_SECS: u64 = 30;
pub const MAX_HEAD_LINES: usize = 60;
pub const MAX_TAIL_LINES: usize = 60;

pub type JobKiller = Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>;

#[allow(dead_code)]
pub struct BackgroundJob {
    pub id: String,
    pub command: String,
    pub start_time: Instant,
    pub stdout_buf: Arc<Mutex<String>>,
    pub is_finished: Arc<AtomicBool>,
    pub exit_status: Arc<Mutex<Option<i32>>>,
    pub killer: JobKiller,
}

pub struct ShellJobManager {
    jobs: Mutex<HashMap<String, Arc<BackgroundJob>>>,
    next_id: Mutex<u64>,
}

impl Default for ShellJobManager {
    fn default() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
        }
    }
}

impl ShellJobManager {
    pub fn global() -> &'static ShellJobManager {
        static MANAGER: OnceLock<ShellJobManager> = OnceLock::new();
        MANAGER.get_or_init(ShellJobManager::default)
    }

    pub fn start_job(&self, cwd: &Path, command: &str) -> io::Result<String> {
        let sanitized = sanitize_shell_command(command);
        if let Some(pat) = is_dangerous_command(&sanitized) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("command blocked by safety filter (pattern: '{pat}')"),
            ));
        }

        let id = {
            let mut counter = self.next_id.lock().unwrap();
            let id = format!("job-{}", *counter);
            *counter += 1;
            id
        };

        let prog = if cfg!(windows) { "powershell" } else { "sh" };
        let win_args = ["-NoProfile", "-NonInteractive", "-Command", sanitized.as_str()];
        let unix_args = ["-c", sanitized.as_str()];
        let args: &[&str] = if cfg!(windows) {
            &win_args
        } else {
            &unix_args
        };

        let mut cmd = Command::new(prog);
        cmd.current_dir(cwd)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn()?;
        let child_pid = child.id();

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let is_finished = Arc::new(AtomicBool::new(false));
        let exit_status = Arc::new(Mutex::new(None));

        let out_handle = stdout.map(|mut so| {
            let buf = Arc::clone(&stdout_buf);
            thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                while let Ok(n) = so.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    let text = String::from_utf8_lossy(&chunk[..n]);
                    if let Ok(mut g) = buf.lock() {
                        g.push_str(&text);
                    }
                }
            })
        });

        let err_handle = stderr.map(|mut se| {
            let buf = Arc::clone(&stdout_buf);
            thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                let mut wrote_header = false;
                while let Ok(n) = se.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    let text = String::from_utf8_lossy(&chunk[..n]);
                    if let Ok(mut g) = buf.lock() {
                        if !wrote_header {
                            if !g.is_empty() {
                                g.push_str("\n[stderr]\n");
                            }
                            wrote_header = true;
                        }
                        g.push_str(&text);
                    }
                }
            })
        });

        let fin_clone = Arc::clone(&is_finished);
        let status_clone = Arc::clone(&exit_status);
        thread::spawn(move || {
            let status = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
            if let Some(h) = out_handle {
                let _ = h.join();
            }
            if let Some(h) = err_handle {
                let _ = h.join();
            }
            if let Ok(mut code_guard) = status_clone.lock() {
                *code_guard = Some(status);
            }
            fin_clone.store(true, Ordering::SeqCst);
        });

        let killer_fn: JobKiller = Arc::new(Mutex::new(Some(Box::new(move || {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                let _ = Command::new("taskkill")
                    .args(["/PID", &child_pid.to_string(), "/T", "/F"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            #[cfg(not(windows))]
            {
                let _ = Command::new("kill")
                    .args(["-9", &child_pid.to_string()])
                    .output();
            }
        }))));

        let job = Arc::new(BackgroundJob {
            id: id.clone(),
            command: command.to_string(),
            start_time: Instant::now(),
            stdout_buf,
            is_finished,
            exit_status,
            killer: killer_fn,
        });

        if let Ok(mut map) = self.jobs.lock() {
            map.insert(id.clone(), job);
        }

        Ok(id)
    }

    pub fn read_output(
        &self,
        job_id: &str,
        byte_cursor: usize,
    ) -> io::Result<(String, usize, bool)> {
        let job = {
            let map = self.jobs.lock().unwrap();
            map.get(job_id).cloned().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Задача '{job_id}' не найдена"),
                )
            })?
        };

        let is_done = job.is_finished.load(Ordering::SeqCst);
        let full = job.stdout_buf.lock().unwrap().clone();

        let available = full.len();
        let chunk = if byte_cursor >= available {
            String::new()
        } else {
            let mut start = byte_cursor;
            while !full.is_char_boundary(start) && start < available {
                start += 1;
            }
            full[start..].to_string()
        };

        Ok((chunk, available, is_done))
    }

    pub fn kill_job(&self, job_id: &str) -> io::Result<String> {
        let job = {
            let map = self.jobs.lock().unwrap();
            map.get(job_id).cloned().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Задача '{job_id}' не найдена"),
                )
            })?
        };

        if let Ok(mut k) = job.killer.lock()
            && let Some(kill) = k.take()
        {
            kill();
        }
        job.is_finished.store(true, Ordering::SeqCst);
        Ok(format!("Фоновая задача '{job_id}' остановлена."))
    }
}

pub fn sanitize_shell_command(command: &str) -> String {
    let mut cmd = command.trim();

    // 1. Unwrap redundant nested powershell invocation, e.g.
    // powershell -NoProfile -Command "..." or powershell "..."
    if let Some(rest) = cmd
        .strip_prefix("powershell.exe")
        .or_else(|| cmd.strip_prefix("powershell"))
    {
        let mut tokens = rest.trim();
        while tokens.starts_with('-') {
            if let Some(idx) = tokens.find(' ') {
                let flag = &tokens[..idx];
                tokens = tokens[idx..].trim_start();
                if flag.eq_ignore_ascii_case("-command") || flag.eq_ignore_ascii_case("-c") {
                    break;
                }
            } else {
                break;
            }
        }
        let inner = tokens.trim();
        if (inner.starts_with('"') && inner.ends_with('"'))
            || (inner.starts_with('\'') && inner.ends_with('\''))
        {
            if inner.len() >= 2 {
                cmd = &inner[1..inner.len() - 1];
            }
        } else if !inner.is_empty() {
            cmd = inner;
        }
    }

    // 2. Windows PowerShell 5.1 rejects '&&' (ParserError: The token '&&' is not a valid statement separator).
    // Normalize ' && ' to '; ' so chaining commands succeeds cleanly.
    if cfg!(windows) {
        cmd.replace(" && ", "; ")
    } else {
        cmd.to_string()
    }
}

pub fn run_exec_cmd(cwd: &Path, command: &str, max_bytes: u64) -> io::Result<String> {
    let sanitized = sanitize_shell_command(command);
    if let Some(pat) = is_dangerous_command(&sanitized) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("команда заблокирована фильтром безопасности (паттерн: '{pat}')"),
        ));
    }

    let prog = if cfg!(windows) { "powershell" } else { "sh" };
    let win_args = ["-NoProfile", "-NonInteractive", "-Command", sanitized.as_str()];
    let unix_args = ["-c", sanitized.as_str()];
    let args: &[&str] = if cfg!(windows) {
        &win_args
    } else {
        &unix_args
    };

    let mut cmd = Command::new(prog);
    cmd.current_dir(cwd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn()?;

    let out_handle = child.stdout.take().map(|mut s| {
        thread::spawn(move || {
            let mut buf = String::new();
            let _ = s.read_to_string(&mut buf);
            buf
        })
    });
    let err_handle = child.stderr.take().map(|mut s| {
        thread::spawn(move || {
            let mut buf = String::new();
            let _ = s.read_to_string(&mut buf);
            buf
        })
    });

    let deadline = Instant::now() + Duration::from_secs(EXEC_TIMEOUT_SECS);
    while child.try_wait()?.is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(format!("Error: command timed out ({EXEC_TIMEOUT_SECS}s)"));
        }
        thread::sleep(Duration::from_millis(50));
    }

    let out = out_handle.and_then(|h| h.join().ok()).unwrap_or_default();
    let err = err_handle.and_then(|h| h.join().ok()).unwrap_or_default();

    let res = match (out.is_empty(), err.is_empty()) {
        (true, true) => return Ok("(completed with no output)".into()),
        (false, true) => out,
        (true, false) => err,
        (false, false) => format!("{out}\n[stderr]\n{err}"),
    };

    Ok(truncate_head_tail(&res, max_bytes as usize))
}

pub fn truncate_head_tail(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }

    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= (MAX_HEAD_LINES + MAX_TAIL_LINES) {
        let mut cut = max_bytes;
        while !content.is_char_boundary(cut) && cut > 0 {
            cut -= 1;
        }
        return format!("{}\n... [обрезано]", &content[..cut]);
    }

    let head = &lines[..MAX_HEAD_LINES];
    let tail = &lines[lines.len() - MAX_TAIL_LINES..];
    let skipped = lines.len() - (MAX_HEAD_LINES + MAX_TAIL_LINES);

    format!(
        "{}\n\n... [пропущено {skipped} строк вывода] ...\n\n{}",
        head.join("\n"),
        tail.join("\n")
    )
}
