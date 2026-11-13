use serde::Deserialize;
use std::io;
use std::time::Duration;

pub const GITHUB_API_BASE: &str = "https://api.github.com/repos";
pub const HTTP_TIMEOUT_SECS: u64 = 20;
pub const USER_AGENT: &str = "snarta/0.1";
pub const DEFAULT_GFX_TARGET: &str = "gfx110X";

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

fn fetch_repo_releases(repo: &str) -> io::Result<Vec<GhRelease>> {
    let url = format!("{GITHUB_API_BASE}/{repo}/releases?per_page=5");
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(HTTP_TIMEOUT_SECS)))
        .build();
    let agent: ureq::Agent = config.into();

    let mut resp = agent
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| io::Error::other(format!("GitHub API request failed ({repo}): {e}")))?;

    resp.body_mut().read_json::<Vec<GhRelease>>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad JSON ({repo}): {e}"),
        )
    })
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
}
