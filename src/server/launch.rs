use crate::log::rotate_if_needed;
use serde::Deserialize;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{fs::OpenOptions, path::Path, thread};

pub const HEALTH_POLL_INTERVAL_MS: u64 = 350;
pub const BENCH_TIMEOUT_SECS: u64 = 120;

static LAST_SERVER_GEN_TPS_BITS: AtomicU64 = AtomicU64::new(0);

pub fn latest_server_gen_tps() -> f64 {
    f64::from_bits(LAST_SERVER_GEN_TPS_BITS.load(Ordering::Relaxed))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerTelemetry {
    pub offloaded_layers: Option<(u32, u32)>,
    pub model_buffer_mib: Option<f64>,
    pub kv_buffer_mib: Option<f64>,
    pub compute_buffer_mib: Option<f64>,
    pub n_ctx_slot: Option<u32>,
    pub server_listening: bool,
    pub last_gen_tps: Option<f64>,
}

pub struct ManagedServer {
    child: Child,
    pub pid: u32,
    pub telemetry: Arc<Mutex<ServerTelemetry>>,
    #[cfg(windows)]
    job_handle: isize,
}

impl ManagedServer {
    pub fn spawn(
        server_exe: &Path,
        args: &[String],
        log_path: &Path,
        log_max_bytes: u64,
    ) -> io::Result<Self> {
        let cmd_str = crate::server::args::format_cmd(server_exe, args);
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = rotate_if_needed(log_path, log_max_bytes);
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = writeln!(f, "[snarta] starting llama-server: {cmd_str}");
        }

        let work_dir = server_exe.parent().unwrap_or_else(|| Path::new("."));
        let mut cmd = Command::new(server_exe);
        cmd.current_dir(work_dir)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path) {
                    let _ = writeln!(f, "[snarta] failed to spawn llama-server: {e}");
                }
                return Err(e);
            }
        };

        let pid = child.id();
        #[cfg(windows)]
        let job_handle = attach_kill_on_close_job(&child)?;

        let telemetry = Arc::new(Mutex::new(ServerTelemetry::default()));
        for s in [
            child.stdout.take().map(|s| Box::new(s) as _),
            child.stderr.take().map(|s| Box::new(s) as _),
        ]
        .into_iter()
        .flatten()
        {
            spawn_log_reader(
                s,
                log_path.to_path_buf(),
                log_max_bytes,
                Arc::clone(&telemetry),
            );
        }
        Ok(Self {
            child,
            pid,
            telemetry,
            #[cfg(windows)]
            job_handle,
        })
    }

    pub fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        #[cfg(windows)]
        if self.job_handle != 0 {
            unsafe {
                let _ = windows_sys::Win32::Foundation::CloseHandle(self.job_handle as *mut _);
            }
            self.job_handle = 0;
        }
    }
}

impl Drop for ManagedServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spawn_log_reader(
    s: Box<dyn io::Read + Send>,
    log_path: std::path::PathBuf,
    max: u64,
    t: Arc<Mutex<ServerTelemetry>>,
) {
    thread::spawn(move || {
        for line in BufReader::new(s).lines().map_while(Result::ok) {
            if is_suppressed_server_warning(&line) {
                continue;
            }
            if let Ok(mut guard) = t.lock() {
                parse_telemetry_line(&line, &mut guard);
            }
            let _ = rotate_if_needed(&log_path, max);
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
                let _ = writeln!(f, "{line}");
            }
        }
    });
}

pub fn is_suppressed_server_warning(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("security: no api key is set and cors allows all origins")
        || lower.contains("gemma4assistant requires ctx_other to be set")
        || lower.contains("failed to measure the memory of the extra model")
        || lower.contains("override 'tokenizer.ggml.add_bos_token'")
        || lower.contains("control-looking token:")
        || lower.contains("is a normal token here, removing it from eog list")
}

pub fn parse_telemetry_line(line: &str, t: &mut ServerTelemetry) {
    let lower = line.to_ascii_lowercase();
    if let Some(pos) = lower.find("offloaded ")
        && let Some((a_str, after)) = lower[pos + 10..].split_once('/')
    {
        let end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        if let (Ok(a), Ok(b)) = (a_str.parse::<u32>(), after[..end].parse::<u32>()) {
            t.offloaded_layers = Some((a, b));
        }
    }

    if (lower.contains("rocm0") || lower.contains("vulkan0"))
        && lower.contains("buffer size")
        && let Some(mib) = extract_mib_after_equals(line)
    {
        if lower.contains("model buffer") {
            t.model_buffer_mib = Some(mib);
        } else if lower.contains("kv buffer") {
            t.kv_buffer_mib = Some(mib);
        } else if lower.contains("compute buffer") {
            t.compute_buffer_mib = Some(mib);
        }
    }

    if let Some(pos) = lower.find("n_ctx_slot")
        && let Some((_, tail)) = lower[pos..].split_once('=')
    {
        let s = tail.trim();
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        if let Ok(val) = s[..end].parse::<u32>() {
            t.n_ctx_slot = Some(val);
        }
    }

    if lower.contains("eval time =")
        && !lower.contains("prompt eval time")
        && let Some(tps) = extract_tokens_per_second(&lower)
        && tps > 0.0
    {
        t.last_gen_tps = Some(tps);
        LAST_SERVER_GEN_TPS_BITS.store(tps.to_bits(), Ordering::Relaxed);
    }

    if lower.contains("server is listening on") || lower.contains("starting the main loop") {
        t.server_listening = true;
    }
}

fn extract_tokens_per_second(lower_line: &str) -> Option<f64> {
    let idx = lower_line.find("tokens per second")?;
    let before = lower_line[..idx].trim_end();
    let num_str = before.rsplit_once(|c: char| c.is_whitespace() || c == '(' || c == ',')
        .map(|(_, s)| s)
        .unwrap_or(before);
    num_str.parse::<f64>().ok()
}

fn extract_mib_after_equals(line: &str) -> Option<f64> {
    let (_, tail) = line.rsplit_once('=')?;
    tail.split_whitespace().next()?.parse::<f64>().ok()
}

fn http_agent(timeout_secs: u64) -> ureq::Agent {
    let b = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(timeout_secs)));
    b.build().into()
}

pub fn is_server_healthy(host: &str, port: u16) -> bool {
    let url = format!("http://{host}:{port}/health");
    http_agent(2)
        .get(&url)
        .call()
        .is_ok_and(|r| r.status().as_u16() == 200)
}

pub fn wait_until_healthy(
    srv: &mut ManagedServer,
    host: &str,
    port: u16,
    timeout_secs: u64,
) -> io::Result<()> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(timeout_secs) {
        if let Some(s) = srv.try_wait()? {
            return Err(io::Error::other(format!("llama-server exited with {s}")));
        }
        if is_server_healthy(host, port) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(HEALTH_POLL_INTERVAL_MS));
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("llama-server health timeout ({timeout_secs}s)"),
    ))
}

#[derive(Debug, Clone)]
pub struct BenchMetrics {
    pub prompt_tps: f64,
    pub gen_tps: f64,
}

#[derive(Deserialize)]
struct BenchResp {
    usage: Option<BenchUsage>,
    timings: Option<BenchTimings>,
}
#[derive(Deserialize)]
struct BenchUsage {
    #[serde(default)]
    completion_tokens: u32,
}
#[derive(Deserialize)]
struct BenchTimings {
    #[serde(default)]
    prompt_per_second: f64,
    #[serde(default)]
    predicted_per_second: f64,
}

pub fn measure_speed(
    host: &str,
    port: u16,
    max_tokens: u32,
    reasoning_effort: Option<&str>,
) -> io::Result<BenchMetrics> {
    let url = format!("http://{host}:{port}/v1/chat/completions");
    let mut body = serde_json::json!({
        "messages": [{"role": "user", "content": "Count from 1 to 60 separated by spaces."}],
        "max_tokens": max_tokens, "temperature": 0.0, "stream": false
    });
    if let Some(eff) = reasoning_effort {
        body["reasoning_effort"] = serde_json::Value::String(eff.to_string());
    }
    let t0 = Instant::now();
    let mut resp = http_agent(BENCH_TIMEOUT_SECS)
        .post(&url)
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("bench request failed: {e}")))?;
    let elapsed = t0.elapsed().as_secs_f64().max(0.001);
    let p: BenchResp = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::other(e.to_string()))?;
    let comp = p.usage.map_or(max_tokens, |u| u.completion_tokens);
    let (pt, gt) = match p.timings {
        Some(t) if t.predicted_per_second > 0.0 => (t.prompt_per_second, t.predicted_per_second),
        _ => (0.0, (comp as f64) / elapsed),
    };
    Ok(BenchMetrics {
        prompt_tps: pt,
        gen_tps: gt,
    })
}

#[cfg(windows)]
fn attach_kill_on_close_job(child: &Child) -> io::Result<isize> {
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::AsRawHandle;
    use std::ptr::null;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::*;

    unsafe {
        let job = CreateJobObjectW(null(), null());
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ptr = &info as *const _ as *const _;
        let sz = size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        if SetInformationJobObject(job, JobObjectExtendedLimitInformation, ptr, sz) == 0 {
            let err = io::Error::last_os_error();
            let _ = CloseHandle(job);
            return Err(err);
        }
        if AssignProcessToJobObject(job, child.as_raw_handle() as *mut _) == 0 {
            let _ = CloseHandle(job);
            return Ok(0);
        }
        Ok(job as isize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stream_telemetry_lines() {
        let mut t = ServerTelemetry::default();
        parse_telemetry_line("load_tensors: offloaded 65/65 layers to GPU", &mut t);
        parse_telemetry_line("llama_kv_cache: ROCm0 KV buffer size = 512.00 MiB", &mut t);
        parse_telemetry_line("slot 0 : n_ctx_slot = 32768", &mut t);
        assert_eq!(t.offloaded_layers, Some((65, 65)));
        assert_eq!(t.kv_buffer_mib, Some(512.0));
        assert_eq!(t.n_ctx_slot, Some(32768));
    }
}
