use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

pub const DEFAULT_SERVER_HOST: &str = "127.0.0.1";
pub const DEFAULT_SERVER_PORT: u16 = 8089;
pub const DEFAULT_MAX_CTX_CEILING: u32 = 262144;
pub const DEFAULT_START_CTX: u32 = 32768;
pub const DEFAULT_LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
pub const DEFAULT_SERVER_NGL: u32 = 99;
pub const DEFAULT_FIRECRAWL_API_URL: &str = "https://api.firecrawl.dev";

pub const DEFAULT_KILL_LIST: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "Discord.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "wallpaper32.exe",
    "wallpaper64.exe",
    "obs64.exe",
    "RadeonSoftware.exe",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub default_persona: String,
    pub active_model: Option<String>,
    pub server: ServerConfig,
    pub vram: VramConfig,
    pub runtime: RuntimeConfig,
    pub limits: LimitsConfig,
    pub prepare: PrepareConfig,
    pub ui: UiConfig,
    pub search: SearchConfig,
    pub firecrawl: FirecrawlConfig,
    pub model_defaults: ModelSettings,
    pub models: BTreeMap<String, ModelSettings>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_persona: "default".to_string(),
            active_model: None,
            server: ServerConfig::default(),
            vram: VramConfig::default(),
            runtime: RuntimeConfig::default(),
            limits: LimitsConfig::default(),
            prepare: PrepareConfig::default(),
            ui: UiConfig::default(),
            search: SearchConfig::default(),
            firecrawl: FirecrawlConfig::default(),
            model_defaults: ModelSettings::default(),
            models: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub health_timeout_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_max_ctx_ceiling: Option<u32>,
    pub default_ngl: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_SERVER_HOST.to_string(),
            port: DEFAULT_SERVER_PORT,
            health_timeout_secs: 120,
            default_max_ctx_ceiling: None,
            default_ngl: DEFAULT_SERVER_NGL,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VramConfig {
    pub reserve_mib: u64,
    pub shared_spill_threshold_mib: u64,
    pub max_ctx_ceiling: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_ctx: Option<u32>,
}

impl Default for VramConfig {
    fn default() -> Self {
        Self {
            reserve_mib: 512,
            shared_spill_threshold_mib: 128,
            max_ctx_ceiling: DEFAULT_MAX_CTX_CEILING,
            start_ctx: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RuntimeConfig {
    pub active_build: Option<String>,
    pub last_good_build: Option<String>,
    pub max_speed_drop_pct: u8,
    pub rocm_repo: String,
    pub vulkan_repo: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            active_build: None,
            last_good_build: None,
            max_speed_drop_pct: 10,
            rocm_repo: "lemonade-sdk/llamacpp-rocm".to_string(),
            vulkan_repo: "ggml-org/llama.cpp".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LimitsConfig {
    pub compact_threshold_pct: u8,
    pub memory_token_limit: u32,
    pub max_tool_steps: u32,
    pub max_read_file_bytes: u64,
    pub log_max_bytes: u64,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            compact_threshold_pct: 80,
            memory_token_limit: 2048,
            max_tool_steps: 24,
            max_read_file_bytes: 262_144,
            log_max_bytes: DEFAULT_LOG_MAX_BYTES,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PrepareConfig {
    pub kill_list: Vec<String>,
    pub never_list: Vec<String>,
    pub dry_run: bool,
}

impl Default for PrepareConfig {
    fn default() -> Self {
        Self {
            kill_list: DEFAULT_KILL_LIST.iter().map(|s| (*s).to_string()).collect(),
            never_list: Vec::new(),
            dry_run: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UiConfig {
    pub auto_scroll: bool,
    pub stream_reasoning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_y: Option<f32>,
    #[serde(default)]
    pub window_maximized: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            auto_scroll: true,
            stream_reasoning: true,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            window_maximized: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SearchConfig {
    pub provider: String,
    pub cascade_order: Vec<String>,
    pub tavily_api_key: String,
    pub parallel_api_key: String,
    pub parallel_mode: String,
    pub exa_api_key: String,
    pub exa_mode: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            provider: "cascade".to_string(),
            cascade_order: vec!["tavily".into(), "parallel".into(), "exa".into()],
            tavily_api_key: String::new(),
            parallel_api_key: String::new(),
            parallel_mode: "fast".to_string(),
            exa_api_key: String::new(),
            exa_mode: "auto".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FirecrawlConfig {
    pub api_key: String,
    pub api_url: String,
    pub mode: String,
    pub monthly_credit_limit: u64,
}

impl Default for FirecrawlConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_url: DEFAULT_FIRECRAWL_API_URL.to_string(),
            mode: "auto".to_string(),
            monthly_credit_limit: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelSettings {
    pub temperature: f32,
    pub top_k: u32,
    pub top_p: f32,
    pub min_p: f32,
    pub repeat_penalty: f32,
    pub max_tokens: u32,
    pub ctx: u32,
    pub ngl: u32,
    pub batch: u32,
    pub ubatch: u32,
    pub kv_type: String,
    pub flash_attn: bool,
    pub mlock: bool,
    pub mmap: bool,
    pub prompt_cache: bool,
    pub reasoning_effort: String,
    pub reasoning_preserve: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mmproj: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtp: Option<String>,
    pub extra_args: String,
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            top_k: 20,
            top_p: 0.95,
            min_p: 0.0,
            repeat_penalty: 1.0,
            max_tokens: 16384,
            ctx: DEFAULT_START_CTX,
            ngl: 99,
            batch: 2048,
            ubatch: 512,
            kv_type: "q8_0".to_string(),
            flash_attn: true,
            mlock: true,
            mmap: true,
            prompt_cache: false,
            reasoning_effort: "medium".to_string(),
            reasoning_preserve: false,
            mmproj: None,
            mtp: None,
            extra_args: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigLoadState {
    LoadedExisting,
    CreatedDefault,
}

impl Config {
    pub fn load_or_create(path: &Path) -> io::Result<(Self, ConfigLoadState)> {
        if path.is_file() {
            let raw = fs::read_to_string(path)?;
            let mut cfg: Self = toml::from_str(&raw).map_err(|e| invalid_input(e.to_string()))?;
            if let Some(ceiling) = cfg.server.default_max_ctx_ceiling {
                if cfg.vram.max_ctx_ceiling == DEFAULT_MAX_CTX_CEILING {
                    cfg.vram.max_ctx_ceiling = ceiling;
                }
            }
            cfg.validate()?;
            crate::tools::sync_config_keys(&cfg.search, &cfg.firecrawl);
            Ok((cfg, ConfigLoadState::LoadedExisting))
        } else {
            let cfg = Self::default();
            cfg.validate()?;
            crate::tools::sync_config_keys(&cfg.search, &cfg.firecrawl);
            cfg.save(path)?;
            Ok((cfg, ConfigLoadState::CreatedDefault))
        }
    }

    pub fn model_settings(&self, model_name: &str) -> ModelSettings {
        self.models
            .get(model_name)
            .cloned()
            .unwrap_or_else(|| self.model_defaults.clone())
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = toml::to_string_pretty(self).map_err(|e| invalid_input(e.to_string()))?;
        fs::write(path, body)
    }

    pub fn validate(&self) -> io::Result<()> {
        if self.server.port == 0 {
            return Err(invalid_input("server.port cannot be 0"));
        }
        if self.limits.compact_threshold_pct == 0 || self.limits.compact_threshold_pct > 100 {
            return Err(invalid_input(
                "limits.compact_threshold_pct must be in 1..=100",
            ));
        }
        validate_model_settings(&self.model_defaults)?;
        for s in self.models.values() {
            validate_model_settings(s)?;
        }
        Ok(())
    }
}

fn invalid_input(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

fn validate_model_settings(s: &ModelSettings) -> io::Result<()> {
    if !matches!(s.kv_type.as_str(), "f16" | "q8_0" | "q4_0") {
        return Err(invalid_input(format!("bad kv_type '{}'", s.kv_type)));
    }
    if !matches!(
        s.reasoning_effort.as_str(),
        "off" | "low" | "medium" | "high" | "xhigh"
    ) {
        return Err(invalid_input(format!(
            "bad reasoning_effort '{}'",
            s.reasoning_effort
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reloads_default_config() {
        let path = std::env::temp_dir().join("snarta_cfg_roundtrip_test.toml");
        let (cfg1, s1) = Config::load_or_create(&path).unwrap();
        assert_eq!(s1, ConfigLoadState::CreatedDefault);
        let (cfg2, s2) = Config::load_or_create(&path).unwrap();
        assert_eq!(s2, ConfigLoadState::LoadedExisting);
        assert_eq!(cfg1, cfg2);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn validates_kv_and_effort() {
        let mut cfg = Config::default();
        cfg.model_defaults.kv_type = "invalid".into();
        assert!(cfg.validate().is_err());
        cfg.model_defaults.kv_type = "q8_0".into();
        cfg.model_defaults.reasoning_effort = "xhigh".into();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn settings_fields_serialize_and_deserialize_cleanly() {
        let mut cfg = Config::default();
        cfg.server.host = "0.0.0.0".to_string();
        cfg.server.port = 9090;
        cfg.server.health_timeout_secs = 45;
        cfg.vram.max_ctx_ceiling = 131072;
        cfg.server.default_ngl = 48;
        cfg.ui.auto_scroll = false;
        cfg.ui.stream_reasoning = false;
        cfg.search.tavily_api_key = "tvly-TEST_123".to_string();
        cfg.firecrawl.api_key = "fc-TEST-KEY".to_string();
        cfg.firecrawl.api_url = "https://custom.firecrawl.dev".to_string();
        cfg.firecrawl.mode = "scrape".to_string();
        cfg.vram.reserve_mib = 1024;
        cfg.vram.shared_spill_threshold_mib = 256;

        let path = std::env::temp_dir().join("snarta_cfg_settings_test.toml");
        cfg.save(&path).unwrap();

        let (loaded, _) = Config::load_or_create(&path).unwrap();
        assert_eq!(loaded.server.host, "0.0.0.0");
        assert_eq!(loaded.server.port, 9090);
        assert_eq!(loaded.server.health_timeout_secs, 45);
        assert_eq!(loaded.vram.max_ctx_ceiling, 131072);
        assert_eq!(loaded.server.default_ngl, 48);
        assert!(!loaded.ui.auto_scroll);
        assert!(!loaded.ui.stream_reasoning);
        assert_eq!(loaded.search.tavily_api_key, "tvly-TEST_123");
        assert_eq!(loaded.firecrawl.api_key, "fc-TEST-KEY");
        assert_eq!(loaded.firecrawl.api_url, "https://custom.firecrawl.dev");
        assert_eq!(loaded.firecrawl.mode, "scrape");
        assert_eq!(loaded.vram.reserve_mib, 1024);
        assert_eq!(loaded.vram.shared_spill_threshold_mib, 256);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn model_settings_inherits_defaults_when_model_not_specified() {
        let mut cfg = Config::default();
        cfg.model_defaults.ctx = 65536;
        cfg.model_defaults.extra_args = "-ncmoe 35 --no-mmap".to_string();

        let settings = cfg.model_settings("gemma-4.gguf");
        assert_eq!(settings.ctx, 65536);
        assert_eq!(settings.extra_args, "-ncmoe 35 --no-mmap");
    }

    #[test]
    fn parses_legacy_config_with_start_ctx_and_server_ceiling() {
        let raw = r#"
default_persona = "default"
[server]
host = "127.0.0.1"
port = 8089
default_max_ctx_ceiling = 131072
[vram]
start_ctx = 8192
max_ctx_ceiling = 262144
[model_defaults]
ctx = 65536
"#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.model_defaults.ctx, 65536);
        assert_eq!(cfg.server.default_max_ctx_ceiling, Some(131072));
        assert_eq!(cfg.vram.start_ctx, Some(8192));
    }
}
