use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const ENV_SNARTA_HOME: &str = "SNARTA_HOME";
pub const CONFIG_FILE_NAME: &str = "snarta.toml";
pub const LOCK_FILE_NAME: &str = "snarta.lock";
pub const DIR_PERSONAS: &str = "personas";
pub const DIR_LIFE: &str = "life";
pub const DIR_LIFE_NOTES: &str = "notes";
pub const DIR_LIFE_SESSIONS: &str = "sessions";
pub const FILE_MEMORY: &str = "memory.md";
pub const DIR_RUNTIME: &str = "runtime";
pub const DIR_MODELS: &str = "models";
pub const DIR_LOGS: &str = "logs";
pub const FILE_SNARTA_LOG: &str = "snarta.log";
pub const FILE_LLAMA_SERVER_LOG: &str = "llama-server.log";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootSource {
    EnvVar,
    ConfigAscent,
    ExeDir,
}

impl RootSource {
    pub fn label(&self) -> &'static str {
        match self {
            Self::EnvVar => "SNARTA_HOME",
            Self::ConfigAscent => "found snarta.toml upwards from exe",
            Self::ExeDir => "executable directory",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub source: RootSource,
    pub exe_dir: PathBuf,
    pub cwd: PathBuf,
}

impl AppPaths {
    pub fn discover() -> io::Result<Self> {
        let exe_path = env::current_exe()?;
        let exe_dir = exe_path.parent().map(Path::to_path_buf).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "executable has no parent dir")
        })?;
        let env_home = env::var_os(ENV_SNARTA_HOME).map(PathBuf::from);
        let mut p = Self::resolve_from(&exe_dir, env_home.as_deref());
        if let Ok(cwd) = env::current_dir() {
            p.cwd = cwd;
        }
        Ok(p)
    }

    pub fn resolve_from(exe_dir: &Path, env_home: Option<&Path>) -> Self {
        let cwd = env::current_dir().unwrap_or_else(|_| exe_dir.to_path_buf());
        if let Some(home) = env_home
            && !home.as_os_str().is_empty()
        {
            return Self {
                root: home.to_path_buf(),
                source: RootSource::EnvVar,
                exe_dir: exe_dir.to_path_buf(),
                cwd,
            };
        }

        if let Some(found) = find_root_with_config(exe_dir) {
            return Self {
                root: found,
                source: RootSource::ConfigAscent,
                exe_dir: exe_dir.to_path_buf(),
                cwd,
            };
        }

        Self {
            root: exe_dir.to_path_buf(),
            source: RootSource::ExeDir,
            exe_dir: exe_dir.to_path_buf(),
            cwd,
        }
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join(CONFIG_FILE_NAME)
    }

    pub fn lock_file(&self) -> PathBuf {
        self.root.join(LOCK_FILE_NAME)
    }

    pub fn personas_dir(&self) -> PathBuf {
        self.root.join(DIR_PERSONAS)
    }

    pub fn life_dir(&self) -> PathBuf {
        self.root.join(DIR_LIFE)
    }

    pub fn memory_file(&self) -> PathBuf {
        self.life_dir().join(FILE_MEMORY)
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.life_dir().join(DIR_LIFE_NOTES)
    }

    pub fn attachments_dir(&self) -> PathBuf {
        self.notes_dir().join("attachments")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.life_dir().join(DIR_LIFE_SESSIONS)
    }

    pub fn prompt_cache_dir(&self) -> PathBuf {
        self.life_dir().join("cache")
    }

    pub fn runtime_dir(&self) -> PathBuf {
        let p = self.root.join(DIR_RUNTIME);
        if !p.exists() && self.cwd.join(DIR_RUNTIME).is_dir() {
            return self.cwd.join(DIR_RUNTIME);
        }
        p
    }

    pub fn models_dir(&self) -> PathBuf {
        let p = self.root.join(DIR_MODELS);
        if !p.exists() && self.cwd.join(DIR_MODELS).is_dir() {
            return self.cwd.join(DIR_MODELS);
        }
        p
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join(DIR_LOGS)
    }

    pub fn snarta_log_file(&self) -> PathBuf {
        self.logs_dir().join(FILE_SNARTA_LOG)
    }

    pub fn llama_server_log_file(&self) -> PathBuf {
        self.logs_dir().join(FILE_LLAMA_SERVER_LOG)
    }

    pub fn ensure_dirs(&self) -> io::Result<()> {
        for dir in [
            self.personas_dir(),
            self.life_dir(),
            self.notes_dir(),
            self.sessions_dir(),
            self.runtime_dir(),
            self.models_dir(),
            self.logs_dir(),
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

fn find_root_with_config(start_dir: &Path) -> Option<PathBuf> {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        if dir.join(CONFIG_FILE_NAME).is_file() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("snarta_paths_test_{tag}_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn prefers_snarta_home_when_set() {
        let base = make_temp_dir("env");
        let exe_dir = base.join("bin");
        let custom_home = base.join("custom");
        fs::create_dir_all(&exe_dir).unwrap();
        fs::write(base.join(CONFIG_FILE_NAME), "").unwrap();

        let paths = AppPaths::resolve_from(&exe_dir, Some(&custom_home));
        assert_eq!(paths.root, custom_home);
        assert_eq!(paths.source, RootSource::EnvVar);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn ascends_to_parent_with_snarta_toml() {
        let base = make_temp_dir("ascend");
        let exe_dir = base.join("target").join("release");
        fs::create_dir_all(&exe_dir).unwrap();
        fs::write(base.join(CONFIG_FILE_NAME), "# config\n").unwrap();

        let paths = AppPaths::resolve_from(&exe_dir, None);
        assert_eq!(paths.root, base);
        assert_eq!(paths.source, RootSource::ConfigAscent);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn falls_back_to_exe_dir_when_no_config_found() {
        let base = make_temp_dir("fallback");
        let exe_dir = base.join("standalone");
        fs::create_dir_all(&exe_dir).unwrap();

        let paths = AppPaths::resolve_from(&exe_dir, None);
        assert_eq!(paths.root, exe_dir);
        assert_eq!(paths.source, RootSource::ExeDir);

        paths.ensure_dirs().unwrap();
        assert!(paths.personas_dir().is_dir());
        assert!(paths.notes_dir().is_dir());
        assert!(paths.sessions_dir().is_dir());
        assert!(paths.runtime_dir().is_dir());
        assert!(paths.models_dir().is_dir());
        assert!(paths.logs_dir().is_dir());

        let _ = fs::remove_dir_all(&base);
    }
}
