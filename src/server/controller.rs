use crate::config::{Config, ModelSettings};
use crate::gguf::GgufMeta;
use crate::log::Logger;
use crate::paths::AppPaths;
use crate::server::{self, LockState};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use std::{fs, thread};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    Stopped,
    Starting {
        model: String,
        ctx: u32,
    },
    Running {
        pid: u32,
        host: String,
        port: u16,
        model: String,
        ctx: u32,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ServerProcessEvent {
    Started {
        pid: u32,
        host: String,
        port: u16,
        model: String,
        ctx: u32,
    },
    Failed(String),
    Exited,
}

/// Deep controller module for llama-server lifecycle management.
///
/// Encapsulates server process spawning, background heartbeat polling,
/// graceful stop signalling, auto-rollback integration, process priority
/// management, and lock-file tracking behind a clean, cohesive seam.
pub struct ServerController {
    paths: AppPaths,
    cfg: Config,
    logger: Logger,
    status: ServerStatus,
    event_rx: Option<Receiver<ServerProcessEvent>>,
    stop_tx: Option<Sender<()>>,
    active_gen: Arc<AtomicU64>,
}

impl ServerController {
    pub fn new(paths: AppPaths, cfg: Config, logger: Logger) -> Self {
        let status = if let Some(lock) = LockState::read_active(&paths.lock_file()) {
            ServerStatus::Running {
                pid: lock.server_pid,
                host: lock.host,
                port: lock.port,
                model: lock.model,
                ctx: lock.ctx,
            }
        } else {
            ServerStatus::Stopped
        };

        Self {
            paths,
            cfg,
            logger,
            status,
            event_rx: None,
            stop_tx: None,
            active_gen: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn status(&self) -> &ServerStatus {
        &self.status
    }

    pub fn set_status(&mut self, status: ServerStatus) {
        self.status = status;
    }

    pub fn set_error(&mut self, err: String) {
        self.status = ServerStatus::Error(err);
    }

    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        matches!(self.status, ServerStatus::Running { .. })
    }

    #[allow(dead_code)]
    pub fn is_starting(&self) -> bool {
        matches!(self.status, ServerStatus::Starting { .. })
    }

    #[allow(dead_code)]
    pub fn running_ctx(&self) -> Option<u32> {
        match self.status {
            ServerStatus::Running { ctx, .. } => Some(ctx),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn running_pid(&self) -> Option<u32> {
        match self.status {
            ServerStatus::Running { pid, .. } => Some(pid),
            _ => None,
        }
    }

    pub fn endpoint(&self) -> (String, u16) {
        match &self.status {
            ServerStatus::Running { host, port, .. } => (host.clone(), *port),
            _ => (self.cfg.server.host.clone(), self.cfg.server.port),
        }
    }

    pub fn update_config(&mut self, cfg: Config) {
        self.cfg = cfg;
    }

    pub fn start(&mut self, meta: GgufMeta, settings: ModelSettings, ctx: u32) {
        self.stop();
        let my_gen = self.active_gen.fetch_add(1, Ordering::SeqCst) + 1;

        self.status = ServerStatus::Starting {
            model: meta.file_name.clone(),
            ctx,
        };

        let paths = self.paths.clone();
        let mut cfg = self.cfg.clone();
        let logger = self.logger.clone();
        let model_file = meta.file_name.clone();
        let active_gen = Arc::clone(&self.active_gen);

        let (event_tx, event_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        self.event_rx = Some(event_rx);
        self.stop_tx = Some(stop_tx);

        thread::spawn(move || {
            let res = server::start_with_auto_rollback(
                &paths, &mut cfg, &logger, &meta, &settings, ctx,
            );
            match res {
                Ok(mut srv) => {
                    // Stop or supersede request may have arrived while booting
                    if active_gen.load(Ordering::SeqCst) != my_gen || stop_rx.try_recv().is_ok() {
                        srv.stop();
                        let _ = event_tx.send(ServerProcessEvent::Exited);
                        return;
                    }

                    let pid = srv.pid;
                    let host = cfg.server.host.clone();
                    let port = cfg.server.port;
                    let _ = crate::sysmon::optimize::set_process_high_priority(pid, true);

                    let (build_id, _) = server::resolve_active_server_exe(&paths, &cfg)
                        .unwrap_or_else(|_| ("unknown".to_string(), PathBuf::new()));

                    let lock = LockState {
                        snarta_pid: std::process::id(),
                        server_pid: pid,
                        host: host.clone(),
                        port,
                        model: model_file.clone(),
                        build_id,
                        ctx,
                    };
                    if let Err(e) = lock.write(&paths.lock_file()) {
                        let _ = logger.error(&format!("failed to write lock file: {e}"));
                    }

                    let _ = event_tx.send(ServerProcessEvent::Started {
                        pid,
                        host,
                        port,
                        model: model_file,
                        ctx,
                    });

                    loop {
                        if active_gen.load(Ordering::SeqCst) != my_gen
                            || stop_rx.try_recv().is_ok()
                        {
                            break;
                        }
                        if let Ok(Some(_)) = srv.try_wait() {
                            break;
                        }
                        thread::sleep(Duration::from_millis(200));
                    }
                    srv.stop();

                    // Clean up lock file only if this generation is still active and owns it
                    if active_gen.load(Ordering::SeqCst) == my_gen {
                        let lock_path = paths.lock_file();
                        if let Some(current) = LockState::read_active(&lock_path) {
                            if current.server_pid == pid {
                                let _ = fs::remove_file(&lock_path);
                            }
                        } else {
                            let _ = fs::remove_file(&lock_path);
                        }
                    }

                    let _ = event_tx.send(ServerProcessEvent::Exited);
                }
                Err(e) => {
                    if active_gen.load(Ordering::SeqCst) == my_gen {
                        let _ = logger.error(&format!("server launch failed: {e}"));
                        let _ = event_tx.send(ServerProcessEvent::Failed(e.to_string()));
                    }
                }
            }
        });
    }

    pub fn stop(&mut self) {
        self.active_gen.fetch_add(1, Ordering::SeqCst);
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.event_rx = None;

        let fallback_pid = match self.status {
            ServerStatus::Running { pid, .. } => Some(pid),
            _ => None,
        };
        stop_server_process(&self.paths, fallback_pid);
        self.status = ServerStatus::Stopped;
    }

    pub fn update_status(&mut self) -> bool {
        let prev_status = self.status.clone();

        if let Some(ref rx) = self.event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ServerProcessEvent::Started {
                        pid,
                        host,
                        port,
                        model,
                        ctx,
                    } => {
                        self.status = ServerStatus::Running {
                            pid,
                            host,
                            port,
                            model,
                            ctx,
                        };
                    }
                    ServerProcessEvent::Failed(err) => {
                        self.status = ServerStatus::Error(err);
                    }
                    ServerProcessEvent::Exited => {
                        if matches!(
                            self.status,
                            ServerStatus::Running { .. } | ServerStatus::Starting { .. }
                        ) {
                            self.status = ServerStatus::Stopped;
                        }
                    }
                }
            }
        }

        match &self.status {
            ServerStatus::Running { pid, .. } => {
                let pid = *pid;
                if !crate::sysmon::procs::is_pid_alive(pid) {
                    let _ = fs::remove_file(self.paths.lock_file());
                    self.status = ServerStatus::Stopped;
                }
            }
            ServerStatus::Stopped | ServerStatus::Error(_) => {
                if let Some(lock) = LockState::read_active(&self.paths.lock_file()) {
                    self.status = ServerStatus::Running {
                        pid: lock.server_pid,
                        host: lock.host,
                        port: lock.port,
                        model: lock.model,
                        ctx: lock.ctx,
                    };
                }
            }
            ServerStatus::Starting { .. } => {}
        }

        self.status != prev_status
    }
}

/// Terminate any active llama-server process and remove its lock file.
pub fn stop_server_process(paths: &AppPaths, fallback_pid: Option<u32>) {
    let lock_path = paths.lock_file();
    let lock_pid = LockState::read_active(&lock_path).map(|l| l.server_pid);
    let target_pid = lock_pid.or(fallback_pid);

    if let Some(pid) = target_pid {
        #[cfg(windows)]
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
        #[cfg(not(windows))]
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
    let _ = fs::remove_file(&lock_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_temp_dir(tag: &str) -> PathBuf {
        let unique = format!(
            "snarta_test_ctrl_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        let _ = fs::create_dir_all(&path);
        path
    }

    #[core::prelude::v1::test]
    fn controller_initializes_stopped_and_transitions_error() {
        let dir = make_test_temp_dir("init");
        let paths = AppPaths::resolve_from(&dir, None);
        paths.ensure_dirs().unwrap();

        let cfg = Config::default();
        let logger = Logger::new(paths.snarta_log_file(), 1024 * 1024);

        let mut ctrl = ServerController::new(paths.clone(), cfg, logger);
        assert_eq!(ctrl.status(), &ServerStatus::Stopped);
        assert!(!ctrl.is_running());
        assert!(!ctrl.is_starting());

        ctrl.set_error("failed to boot".to_string());
        assert_eq!(
            ctrl.status(),
            &ServerStatus::Error("failed to boot".to_string())
        );

        ctrl.stop();
        assert_eq!(ctrl.status(), &ServerStatus::Stopped);

        let _ = fs::remove_dir_all(&dir);
    }

    #[core::prelude::v1::test]
    fn controller_endpoint_fallback() {
        let dir = make_test_temp_dir("endpoint");
        let paths = AppPaths::resolve_from(&dir, None);
        paths.ensure_dirs().unwrap();

        let mut cfg = Config::default();
        cfg.server.host = "127.0.0.1".to_string();
        cfg.server.port = 8080;
        let logger = Logger::new(paths.snarta_log_file(), 1024 * 1024);

        let mut ctrl = ServerController::new(paths.clone(), cfg, logger);
        assert_eq!(ctrl.endpoint(), ("127.0.0.1".to_string(), 8080));

        ctrl.set_status(ServerStatus::Running {
            pid: 1234,
            host: "0.0.0.0".to_string(),
            port: 9999,
            model: "test.gguf".to_string(),
            ctx: 4096,
        });
        assert_eq!(ctrl.endpoint(), ("0.0.0.0".to_string(), 9999));
        assert_eq!(ctrl.running_pid(), Some(1234));
        assert_eq!(ctrl.running_ctx(), Some(4096));

        let _ = fs::remove_dir_all(&dir);
    }
}
