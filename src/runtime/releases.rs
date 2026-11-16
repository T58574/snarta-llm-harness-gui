use serde::Deserialize;
use std::io;
use std::time::Duration;

pub const GITHUB_API_BASE: &str = "https://api.github.com/repos";
pub const GITHUB_WEB_BASE: &str = "https://github.com";
pub const HTTP_TIMEOUT_SECS: u64 = 20;
pub const USER_AGENT: &str = "snarta-llm-harness-gui/0.1";
pub const DEFAULT_GFX_TARGET: &str = "gfx110X";
const MAX_WEB_FALLBACK_TAGS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    RocmHip,
    Vulkan,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RocmHip => "rocm-hip",
            Self::Vulkan => "vulkan",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseCandidate {
    pub build_id: String,
    pub repo: String,
    pub tag: String,
    pub asset_name: String,
    pub download_url: String,
    pub size_bytes: u64,
    pub backend: BackendKind,
    pub score: u32,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[allow(dead_code)]
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub fn fetch_candidates(
    rocm_repo: &str,
    vulkan_repo: &str,
    gfx_target: Option<&str>,
) -> io::Result<Vec<ReleaseCandidate>> {
    let target = gfx_target.unwrap_or(DEFAULT_GFX_TARGET);
    let mut out = Vec::new();
    let mut errors = Vec::new();

    match fetch_repo_releases(rocm_repo) {
        Ok(rocm_releases) => {
            out.extend(filter_releases(rocm_repo, &rocm_releases, target));
        }
        Err(e) => {
            errors.push(format!("{rocm_repo}: {e}"));
        }
    }

    match fetch_repo_releases(vulkan_repo) {
        Ok(vk_releases) => {
            out.extend(filter_releases(vulkan_repo, &vk_releases, target));
        }
        Err(e) => {
            errors.push(format!("{vulkan_repo}: {e}"));
        }
    }

    if out.is_empty() {
        let msg = if errors.is_empty() {
            format!("no matching Windows x64 llama.cpp release assets found (target: {target})")
        } else {
            format!("no releases found ({})", errors.join(", "))
        };
        return Err(io::Error::new(io::ErrorKind::NotFound, msg));
    }

    out.sort_by_key(|a| std::cmp::Reverse(a.score));
    Ok(out)
}

fn build_http_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(HTTP_TIMEOUT_SECS)))
        .build();
    config.into()
}

fn github_auth_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GH_TOKEN").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn fetch_repo_releases(repo: &str) -> io::Result<Vec<GhRelease>> {
    match fetch_repo_releases_api(repo) {
        Ok(releases) => Ok(releases),
        Err(api_err) => match fetch_repo_releases_web(repo) {
            Ok(releases) if !releases.is_empty() => Ok(releases),
            Ok(_) => Err(api_err),
            Err(web_err) => Err(io::Error::other(format!("{api_err}; web fallback: {web_err}"))),
        },
    }
}

fn fetch_repo_releases_api(repo: &str) -> io::Result<Vec<GhRelease>> {
    let url = format!("{GITHUB_API_BASE}/{repo}/releases?per_page=5");
    let agent = build_http_agent();

    let mut req = agent
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");

    if let Some(token) = github_auth_token() {
        req = req.header("Authorization", &format!("Bearer {token}"));
    }

    let mut resp = req
        .call()
        .map_err(|e| io::Error::other(format!("GitHub API request failed ({repo}): {e}")))?;

    resp.body_mut().read_json::<Vec<GhRelease>>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad JSON ({repo}): {e}"),
        )
    })
}

fn fetch_repo_releases_web(repo: &str) -> io::Result<Vec<GhRelease>> {
    let agent = build_http_agent();
    let atom_url = format!("{GITHUB_WEB_BASE}/{repo}/releases.atom");
    let mut atom_resp = agent
        .get(&atom_url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/atom+xml,application/xml,text/xml")
        .call()
        .map_err(|e| io::Error::other(format!("GitHub atom request failed ({repo}): {e}")))?;
    let atom_body = atom_resp
        .body_mut()
        .read_to_string()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("atom read error: {e}")))?;

    let tags = parse_atom_release_tags(&atom_body, MAX_WEB_FALLBACK_TAGS);
    let mut releases = Vec::new();
    for tag in tags {
        let assets_url = format!("{GITHUB_WEB_BASE}/{repo}/releases/expanded_assets/{tag}");
        let Ok(mut assets_resp) = agent
            .get(&assets_url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "text/html")
            .call()
        else {
            continue;
        };
        let Ok(html) = assets_resp.body_mut().read_to_string() else {
            continue;
        };
        let assets = parse_expanded_assets_html(repo, &tag, &html);
        if !assets.is_empty() {
            releases.push(GhRelease {
                tag_name: tag,
                draft: false,
                prerelease: false,
                assets,
            });
        }
    }
    Ok(releases)
}

fn parse_atom_release_tags(atom: &str, max_tags: usize) -> Vec<String> {
    let mut tags = Vec::new();
    let needle = "/releases/tag/";
    let mut rest = atom;
    while let Some(pos) = rest.find(needle) {
        let after = &rest[pos + needle.len()..];
        let end = after
            .find(['"', '\'', '<', '&', '?', '#', ' ', '\n', '\r'])
            .unwrap_or(after.len());
        let tag = after[..end].trim();
        if !tag.is_empty() && !tags.iter().any(|t| t == tag) {
            tags.push(tag.to_string());
            if tags.len() >= max_tags {
                break;
            }
        }
        rest = &after[end..];
    }
    tags
}

fn parse_expanded_assets_html(repo: &str, tag: &str, html: &str) -> Vec<GhAsset> {
    let mut assets = Vec::new();
    let dl_prefix = format!("/{repo}/releases/download/{tag}/");
    for chunk in html.split("<li").skip(1) {
        let Some(href_pos) = chunk.find(&dl_prefix) else {
            continue;
        };
        let href_start = &chunk[href_pos..];
        let Some(href_end) = href_start.find('"') else {
            continue;
        };
        let href = &href_start[..href_end];
        let Some(name) = href.rsplit('/').next().filter(|n| !n.is_empty()) else {
            continue;
        };
        let browser_download_url = format!("{GITHUB_WEB_BASE}{href}");
        let after_link = &href_start[href_end..];
        let size = extract_size_from_li(after_link).unwrap_or(0);
        if !assets.iter().any(|a: &GhAsset| a.name == name) {
            assets.push(GhAsset {
                name: name.to_string(),
                browser_download_url,
                size,
            });
        }
    }
    assets
}

fn extract_size_from_li(li_tail: &str) -> Option<u64> {
    let mut rest = li_tail;
    while let Some(gt) = rest.find('>') {
        let after_gt = &rest[gt + 1..];
        let Some(lt) = after_gt.find('<') else {
            break;
        };
        let text = after_gt[..lt].trim();
        if let Some(bytes) = parse_human_size(text) {
            return Some(bytes);
        }
        rest = &after_gt[lt..];
    }
    None
}

fn parse_human_size(raw: &str) -> Option<u64> {
    let mut parts = raw.split_whitespace();
    let num_str = parts.next()?;
    let unit = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let val: f64 = num_str.parse().ok()?;
    if !val.is_finite() || val < 0.0 {
        return None;
    }
    let mult: f64 = match unit.to_ascii_lowercase().as_str() {
        "b" | "byte" | "bytes" => 1.0,
        "kb" | "kib" => 1024.0,
        "mb" | "mib" => 1024.0 * 1024.0,
        "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((val * mult).round() as u64)
}

fn filter_releases(repo: &str, releases: &[GhRelease], gfx_target: &str) -> Vec<ReleaseCandidate> {
    let mut candidates = Vec::new();
    for (rel_idx, rel) in releases.iter().enumerate() {
        if rel.draft {
            continue;
        }
        let recency_bonus = (10u32.saturating_sub(rel_idx as u32)) * 5;
        for asset in &rel.assets {
            if let Some((backend, base_score)) = score_asset_name(repo, &asset.name, gfx_target) {
                let build_id = make_build_id(&rel.tag_name, backend, gfx_target, &asset.name);
                candidates.push(ReleaseCandidate {
                    build_id,
                    repo: repo.to_string(),
                    tag: rel.tag_name.clone(),
                    asset_name: asset.name.clone(),
                    download_url: asset.browser_download_url.clone(),
                    size_bytes: asset.size,
                    backend,
                    score: base_score + recency_bonus,
                });
            }
        }
    }
    candidates
}

pub fn score_asset_name(
    repo: &str,
    asset_name: &str,
    gfx_target: &str,
) -> Option<(BackendKind, u32)> {
    let lower = asset_name.to_ascii_lowercase();
    if !lower.ends_with(".zip") {
        return None;
    }

    // Must be Windows x64, never arm64 / linux / macos / cudart / cuda / sycl / opencl
    let is_windows = lower.contains("win") || lower.contains("windows");
    let is_x64 = lower.contains("-x64") || lower.contains("_x64");
    if !is_windows || !is_x64 {
        return None;
    }

    for forbidden in [
        "arm64", "linux", "macos", "darwin", "ubuntu", "cuda", "cudart", "sycl", "opencl", "avx512",
    ] {
        if lower.contains(forbidden) {
            return None;
        }
    }

    // Reject incomplete ggml-org bin-win-rocm-* zip (lacks rocblas.dll / hipblaslt.dll)
    let repo_lower = repo.to_ascii_lowercase();
    if repo_lower.contains("ggml-org") && lower.contains("rocm") {
        return None;
    }
    if lower.contains("bin-win-rocm") && !lower.contains("gfx") {
        return None;
    }

    let target_lower = gfx_target.to_ascii_lowercase();
    let matches_rocm_target = lower.contains("rocm")
        && (lower.contains(&target_lower)
            || ((target_lower.starts_with("gfx110")
                || target_lower == "gfx1100"
                || target_lower == "gfx1101"
                || target_lower == "gfx1102")
                && lower.contains("gfx110x"))
            || ((target_lower.starts_with("gfx103")
                || target_lower == "gfx1030"
                || target_lower == "gfx1031"
                || target_lower == "gfx1032")
                && lower.contains("gfx103x"))
            || ((target_lower.starts_with("gfx120")
                || target_lower == "gfx1200"
                || target_lower == "gfx1201")
                && lower.contains("gfx120x")));

    if matches_rocm_target {
        return Some((BackendKind::RocmHip, 1000));
    }

    if lower.contains("vulkan") {
        return Some((BackendKind::Vulkan, 500));
    }

    None
}

pub fn make_build_id(
    tag: &str,
    backend: BackendKind,
    gfx_target: &str,
    asset_name: &str,
) -> String {
    let clean_tag: String = tag
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '.' || *c == '_')
        .collect();
    match backend {
        BackendKind::RocmHip => {
            let lower = asset_name.to_ascii_lowercase();
            let target_lower = gfx_target.to_ascii_lowercase();
            if lower.contains(&target_lower) {
                format!("{clean_tag}-rocm-{gfx_target}-x64")
            } else {
                format!("{clean_tag}-rocm-x64")
            }
        }
        BackendKind::Vulkan => format!("{clean_tag}-vulkan-x64"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lemonade_rocm_gfx110x_and_ranks_highest() {
        let res = score_asset_name(
            "lemonade-sdk/llamacpp-rocm",
            "llama-b1354-windows-rocm-gfx110X-x64.zip",
            "gfx110X",
        );
        assert_eq!(res, Some((BackendKind::RocmHip, 1000)));
    }

    #[test]
    fn rejects_incomplete_ggml_org_bin_win_rocm() {
        let res = score_asset_name(
            "ggml-org/llama.cpp",
            "llama-b5000-bin-win-rocm-x64.zip",
            "gfx110X",
        );
        assert_eq!(res, None);
    }

    #[test]
    fn rejects_vulkan_arm64_and_accepts_vulkan_x64() {
        let arm = score_asset_name(
            "ggml-org/llama.cpp",
            "llama-b5000-bin-win-vulkan-arm64.zip",
            "gfx110X",
        );
        assert_eq!(arm, None);

        let x64 = score_asset_name(
            "ggml-org/llama.cpp",
            "llama-b5000-bin-win-vulkan-x64.zip",
            "gfx110X",
        );
        assert_eq!(x64, Some((BackendKind::Vulkan, 500)));
    }

    #[test]
    fn rejects_wrong_gfx_target_and_cuda() {
        let gfx1151 = score_asset_name(
            "lemonade-sdk/llamacpp-rocm",
            "llama-b1354-windows-rocm-gfx1151-x64.zip",
            "gfx110X",
        );
        assert_eq!(gfx1151, None);

        let cuda = score_asset_name(
            "ggml-org/llama.cpp",
            "llama-b5000-bin-win-cuda-12.4-x64.zip",
            "gfx110X",
        );
        assert_eq!(cuda, None);
    }

    #[test]
    fn parses_atom_tags_and_expanded_assets_html() {
        let atom = r#"
        <feed>
          <entry><link href="https://github.com/lemonade-sdk/llamacpp-rocm/releases/tag/b1377"/></entry>
          <entry><link href="https://github.com/lemonade-sdk/llamacpp-rocm/releases/tag/b1377"/></entry>
          <entry><link href="https://github.com/lemonade-sdk/llamacpp-rocm/releases/tag/b1376"/></entry>
        </feed>
        "#;
        assert_eq!(parse_atom_release_tags(atom, 5), vec!["b1377", "b1376"]);

        let html = r#"
        <ul>
          <li class="Box-row">
            <a href="/lemonade-sdk/llamacpp-rocm/releases/download/b1377/llama-b1377-windows-rocm-gfx110X-x64.zip" rel="nofollow">
              <span>llama-b1377-windows-rocm-gfx110X-x64.zip</span>
            </a>
            <span>sha256:abc</span>
            <span>512 MB</span>
          </li>
          <li class="Box-row">
            <a href="/lemonade-sdk/llamacpp-rocm/releases/download/b1377/llama-b1377-ubuntu-rocm-gfx110X-x64.zip">
              <span>llama-b1377-ubuntu-rocm-gfx110X-x64.zip</span>
            </a>
            <span>1.06 GB</span>
          </li>
        </ul>
        "#;
        let assets = parse_expanded_assets_html("lemonade-sdk/llamacpp-rocm", "b1377", html);
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].name, "llama-b1377-windows-rocm-gfx110X-x64.zip");
        assert_eq!(
            assets[0].browser_download_url,
            "https://github.com/lemonade-sdk/llamacpp-rocm/releases/download/b1377/llama-b1377-windows-rocm-gfx110X-x64.zip"
        );
        assert_eq!(assets[0].size, 512 * 1024 * 1024);
        assert_eq!(assets[1].size, (1.06 * 1024.0 * 1024.0 * 1024.0_f64).round() as u64);
    }
}
