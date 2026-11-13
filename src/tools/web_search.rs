use serde_json::{Value, json};
use std::io;
use std::time::Duration;

pub const DEFAULT_SEARCH_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone)]
pub struct SearchResultItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct SearchOutcome {
    pub provider: String,
    pub query: String,
    pub results: Vec<SearchResultItem>,
    pub estimated_cost: String,
}

pub fn execute_search_cascade(
    query: &str,
    max_results: usize,
    preferred_provider: Option<&str>,
    tavily_key: Option<&str>,
    parallel_key: Option<&str>,
    exa_key: Option<&str>,
) -> io::Result<SearchOutcome> {
    let clean_query = query.trim();
    if clean_query.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "search query is empty",
        ));
    }

    let default_order = ["tavily", "parallel", "exa"];
    let providers_to_try: Vec<&str> =
        if let Some(pref) = preferred_provider.filter(|p| !p.is_empty()) {
            let mut order = vec![pref];
            for p in default_order {
                if p != pref {
                    order.push(p);
                }
            }
            order
        } else {
            default_order.to_vec()
        };

    let mut last_err = String::from("no API keys configured for web search");

    for p in providers_to_try {
        match p {
            "tavily" => {
                if let Some(key) = tavily_key.filter(|k| !k.is_empty()) {
                    match search_tavily(clean_query, key, max_results) {
                        Ok(res) => return Ok(res),
                        Err(e) => last_err = format!("Tavily: {e}"),
                    }
                }
            }
            "parallel" => {
                if let Some(key) = parallel_key.filter(|k| !k.is_empty()) {
                    match search_parallel(clean_query, key, max_results) {
                        Ok(res) => return Ok(res),
                        Err(e) => last_err = format!("Parallel.ai: {e}"),
                    }
                }
            }
            "exa" => {
                if let Some(key) = exa_key.filter(|k| !k.is_empty()) {
                    match search_exa(clean_query, key, max_results) {
                        Ok(res) => return Ok(res),
                        Err(e) => last_err = format!("Exa.ai: {e}"),
                    }
                }
            }
            _ => {}
        }
    }

    Err(io::Error::other(format!(
        "Web search failed (cascade exhausted): {last_err}. Configure tavily_api_key, parallel_api_key, or exa_api_key in snarta.toml."
    )))
}

fn search_tavily(query: &str, api_key: &str, max_results: usize) -> io::Result<SearchOutcome> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(DEFAULT_SEARCH_TIMEOUT_SECS)))
        .build()
        .into();

    let body = json!({
        "api_key": api_key,
        "query": query,
        "max_results": max_results.min(10),
        "search_depth": "basic",
        "include_answer": true
    });

    let mut resp = agent
        .post("https://api.tavily.com/search")
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("HTTP error: {e}")))?;

    let val: Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {e}")))?;

    let mut items = Vec::new();
    if let Some(results) = val["results"].as_array() {
        for r in results {
            items.push(SearchResultItem {
                title: r["title"].as_str().unwrap_or("").to_string(),
                url: r["url"].as_str().unwrap_or("").to_string(),
                snippet: r["content"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(SearchOutcome {
        provider: "Tavily".to_string(),
        query: query.to_string(),
        results: items,
        estimated_cost: "1 credit (of 1,000/mo)".to_string(),
    })
}

fn search_parallel(query: &str, api_key: &str, max_results: usize) -> io::Result<SearchOutcome> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(DEFAULT_SEARCH_TIMEOUT_SECS)))
        .build()
        .into();

    let body = json!({
        "query": query,
        "mode": "fast",
        "max_results": max_results.min(10)
    });

    let mut resp = agent
        .post("https://api.parallel.ai/v1/search")
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("HTTP error: {e}")))?;

    let val: Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {e}")))?;

    let mut items = Vec::new();
    if let Some(results) = val["results"].as_array() {
        for r in results {
            items.push(SearchResultItem {
                title: r["title"].as_str().unwrap_or("").to_string(),
                url: r["url"].as_str().unwrap_or("").to_string(),
                snippet: r["snippet"]
                    .as_str()
                    .or_else(|| r["text"].as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }

    Ok(SearchOutcome {
        provider: "Parallel.ai".to_string(),
        query: query.to_string(),
        results: items,
        estimated_cost: "$0.001 (Fast mode, ~$1/1k reqs)".to_string(),
    })
}

fn search_exa(query: &str, api_key: &str, max_results: usize) -> io::Result<SearchOutcome> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(DEFAULT_SEARCH_TIMEOUT_SECS)))
        .build()
        .into();

    let body = json!({
        "query": query,
        "type": "auto",
        "numResults": max_results.min(10),
        "contents": { "text": true }
    });

    let mut resp = agent
        .post("https://api.exa.ai/search")
        .header("x-api-key", api_key)
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| io::Error::other(format!("HTTP error: {e}")))?;

    let val: Value = resp
        .body_mut()
        .read_json()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {e}")))?;

    let mut items = Vec::new();
    if let Some(results) = val["results"].as_array() {
        for r in results {
            items.push(SearchResultItem {
                title: r["title"].as_str().unwrap_or("").to_string(),
                url: r["url"].as_str().unwrap_or("").to_string(),
                snippet: r["text"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(SearchOutcome {
        provider: "Exa.ai".to_string(),
        query: query.to_string(),
        results: items,
        estimated_cost: "$0.007 (Auto mode, ~$7/1k reqs)".to_string(),
    })
}

pub fn format_search_results(outcome: &SearchOutcome) -> String {
    if outcome.results.is_empty() {
        return format!(
            "Provider {} returned no results for query \"{}\".",
            outcome.provider, outcome.query
        );
    }

    let mut out = format!(
        "### Search results [{}] (cost: {}):\n\n",
        outcome.provider, outcome.estimated_cost
    );

    for (idx, r) in outcome.results.iter().enumerate() {
        let snippet_clean = r.snippet.split_whitespace().collect::<Vec<_>>().join(" ");
        let snippet_short = if snippet_clean.chars().count() > 280 {
            let s: String = snippet_clean.chars().take(280).collect();
            format!("{s}…")
        } else {
            snippet_clean
        };
        out.push_str(&format!(
            "{}. [{}]({})\n   {}\n\n",
            idx + 1,
            r.title,
            r.url,
            snippet_short
        ));
    }

    out
}
