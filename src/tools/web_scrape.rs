use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const FIRECRAWL_TIMEOUT_SECS: u64 = 30;
#[allow(dead_code)]
pub const DEFAULT_MONTHLY_CREDITS: u64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlexandriaCapability {
    pub provider: String,
    pub address: String,
    pub price_credits: u64,
    pub description: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FirecrawlCreditState {
    pub spent_credits: u64,
    pub last_updated_utc: String,
}

pub struct AlexandriaTracker {
    state: Mutex<FirecrawlCreditState>,
}

impl Default for AlexandriaTracker {
    fn default() -> Self {
        Self {
            state: Mutex::new(FirecrawlCreditState::default()),
        }
    }
}

impl AlexandriaTracker {
    pub fn global() -> &'static AlexandriaTracker {
        static TRACKER: OnceLock<AlexandriaTracker> = OnceLock::new();
        TRACKER.get_or_init(AlexandriaTracker::default)
    }

    pub fn ensure_loaded(&self, life_dir: &Path) {
        static LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOADED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            self.load_from_life(life_dir);
        }
    }

    pub fn load_from_life(&self, life_dir: &Path) {
        let p = life_dir.join("firecrawl_credits.json");
        if let Ok(data) = fs::read_to_string(&p)
            && let Ok(loaded) = serde_json::from_str::<FirecrawlCreditState>(&data)
            && let Ok(mut st) = self.state.lock()
        {
            *st = loaded;
        }
    }

    pub fn record_spent(&self, life_dir: &Path, credits: u64) {
        self.ensure_loaded(life_dir);
        if let Ok(mut st) = self.state.lock() {
            st.spent_credits += credits;
            st.last_updated_utc = crate::log::date_stamp_utc();
            let p = life_dir.join("firecrawl_credits.json");
            if let Ok(j) = serde_json::to_string_pretty(&*st) {
                let _ = fs::write(p, j);
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_spent(&self) -> u64 {
        self.state.lock().map(|s| s.spent_credits).unwrap_or(0)
    }
}

pub fn alexandria_discover(api_key: &str, query: &str) -> io::Result<Vec<AlexandriaCapability>> {
    let clean_query = query.trim();
    if clean_query.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "query is empty",
        ));
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(FIRECRAWL_TIMEOUT_SECS)))
        .build()
        .into();

    let body = json!({
        "query": clean_query,
        "sources": ["alexandria"]
    });

    let mut resp = agent
        .post("https://api.firecrawl.dev/v2/search")
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("Alexandria search HTTP error: {e}")))?;

    let val: Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {e}")))?;

    let mut capabilities = Vec::new();
    if let Some(list) = val["data"].as_array().or_else(|| val["results"].as_array()) {
        for item in list {
            capabilities.push(AlexandriaCapability {
                provider: item["provider"].as_str().unwrap_or("unknown").to_string(),
                address: item["address"].as_str().unwrap_or("").to_string(),
                price_credits: item["price"]
                    .as_u64()
                    .unwrap_or(item["credits"].as_u64().unwrap_or(1)),
                description: item["description"]
                    .as_str()
                    .or_else(|| item["summary"].as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }

    Ok(capabilities)
}

pub fn alexandria_scrape(
    life_dir: &Path,
    api_key: &str,
    provider: &str,
    capability: &str,
    options: Value,
) -> io::Result<(String, u64)> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(FIRECRAWL_TIMEOUT_SECS)))
        .build()
        .into();

    let body = json!({
        "alexandria": {
            "provider": provider,
            "capability": capability,
            "options": options
        }
    });

    let mut resp = agent
        .post("https://api.firecrawl.dev/v2/scrape")
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("Alexandria scrape HTTP error: {e}")))?;

    let val: Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {e}")))?;

    let cost = val["cost"]
        .as_u64()
        .or_else(|| val["credits_used"].as_u64())
        .unwrap_or(1);
    AlexandriaTracker::global().record_spent(life_dir, cost);

    let content = if let Some(markdown) = val["data"]["markdown"].as_str() {
        markdown.to_string()
    } else if let Some(text) = val["data"]["text"].as_str() {
        text.to_string()
    } else {
        serde_json::to_string_pretty(&val).unwrap_or_default()
    };

    Ok((content, cost))
}

#[allow(dead_code)]
pub fn scrape_url_markdown(
    life_dir: &Path,
    api_key: &str,
    target_url: &str,
) -> io::Result<(String, u64)> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(FIRECRAWL_TIMEOUT_SECS)))
        .build()
        .into();

    let body = json!({
        "url": target_url,
        "formats": ["markdown"]
    });

    let mut resp = agent
        .post("https://api.firecrawl.dev/v2/scrape")
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("Firecrawl scrape HTTP error: {e}")))?;

    let val: Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {e}")))?;

    let cost = val["cost"].as_u64().unwrap_or(1);
    AlexandriaTracker::global().record_spent(life_dir, cost);

    let md = val["data"]["markdown"]
        .as_str()
        .unwrap_or("(no content)")
        .to_string();

    Ok((md, cost))
}
