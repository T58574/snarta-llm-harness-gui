use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const BACKUP_SUFFIX: &str = ".1";

#[derive(Debug, Clone)]
pub struct Logger {
    path: PathBuf,
    max_bytes: u64,
    lock: Arc<Mutex<()>>,
}

impl Logger {
    pub fn new(path: PathBuf, max_bytes: u64) -> Self {
        Self {
            path,
            max_bytes,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn info(&self, msg: &str) -> io::Result<()> {
        self.write_line("INFO", msg)
    }

    pub fn error(&self, msg: &str) -> io::Result<()> {
        self.write_line("ERROR", msg)
    }

    fn write_line(&self, level: &str, msg: &str) -> io::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        rotate_if_needed(&self.path, self.max_bytes)?;

        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "[{secs}] [{level}] {msg}")
    }
}

pub fn rotate_if_needed(path: &Path, max_bytes: u64) -> io::Result<()> {
    if max_bytes == 0 {
        return Ok(());
    }
    let len = match fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if len < max_bytes {
        return Ok(());
    }

    let mut backup = path.as_os_str().to_os_string();
    backup.push(BACKUP_SUFFIX);
    let backup_path = PathBuf::from(backup);

    if backup_path.exists() {
        let _ = fs::remove_file(&backup_path);
    }
    fs::rename(path, backup_path)
}

pub fn date_stamp_utc() -> String {
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (s / 86400) as i64 + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let (mp, d) = (
        (5 * doy + 2) / 153,
        doy - (153 * ((5 * doy + 2) / 153) + 2) / 5 + 1,
    );
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = if m <= 2 { y + 1 } else { y };
    format!("{yr:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn rotates_when_exceeding_max_bytes() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("snarta_log_test_{nanos}"));
        let log_path = dir.join("snarta.log");

        let logger = Logger::new(log_path.clone(), 40);
        logger
            .info("first line that exceeds forty bytes easily")
            .unwrap();
        logger.info("second line after rotation").unwrap();

        let backup_path = dir.join("snarta.log.1");
        assert!(log_path.is_file());
        assert!(backup_path.is_file());

        let current = fs::read_to_string(&log_path).unwrap();
        let rotated = fs::read_to_string(&backup_path).unwrap();
        assert!(current.contains("second line"));
        assert!(rotated.contains("first line"));

        let _ = fs::remove_dir_all(&dir);
    }
}
