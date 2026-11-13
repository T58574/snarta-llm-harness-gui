use crate::config::{ModelSettings, ServerConfig};
use crate::gguf::GgufMeta;
use std::path::Path;

pub fn build_server_args(
    model_path: &Path,
    server: &ServerConfig,
    settings: &ModelSettings,
    meta: Option<&GgufMeta>,
    ctx_override: Option<u32>,
    prompt_cache_path: Option<&Path>,
) -> Vec<String> {
    let ctx = ctx_override.unwrap_or(settings.ctx);
    let mut args = vec![
        "-m".to_string(),
        model_path.to_string_lossy().into_owned(),
        "--host".to_string(),
        server.host.clone(),
        "--port".to_string(),
        server.port.to_string(),
        "--jinja".to_string(),
        "-fit".to_string(),
        "off".to_string(),
        "--reasoning-format".to_string(),
        "auto".to_string(),
    ];

    if settings.reasoning_effort == "off" {
        args.push("--reasoning".to_string());
        args.push("off".to_string());
    } else {
        args.push("--reasoning".to_string());
        args.push("on".to_string());
        args.push("--reasoning-effort".to_string());
        args.push(settings.reasoning_effort.clone());
    }

    if settings.reasoning_preserve {
        args.push("--reasoning-preserve".to_string());
    }

    args.extend([
        "-c".to_string(),
        ctx.to_string(),
        "-ngl".to_string(),
        settings.ngl.to_string(),
        "-b".to_string(),
        settings.batch.to_string(),
        "-ub".to_string(),
        settings.ubatch.to_string(),
        "-fa".to_string(),
        if settings.flash_attn { "on" } else { "off" }.to_string(),
    ]);

    if settings.mlock {
        args.push("--load-mode".to_string());
        args.push("mlock".to_string());
    } else if !settings.mmap {
        args.push("--load-mode".to_string());
        args.push("none".to_string());
    }

    args.extend([
        "-np".to_string(),
        "1".to_string(),
        "-ctk".to_string(),
        settings.kv_type.clone(),
        "-ctv".to_string(),
        settings.kv_type.clone(),
    ]);

    if settings.prompt_cache
        && let Some(cache_file) = prompt_cache_path
    {
        args.push("--prompt-cache".to_string());
        args.push(cache_file.to_string_lossy().into_owned());
    }

    let mmproj_file = match settings.mmproj.as_deref().map(str::trim) {
        None | Some("") | Some("none") | Some("off") | Some("false") => None,
        Some("auto") => find_matching_mmproj(model_path),
        Some(mm) => {
            let p = Path::new(mm);
            if p.is_absolute() {
                Some(p.to_path_buf())
            } else if let Some(parent) = model_path.parent() {
                Some(parent.join(p))
            } else {
                Some(p.to_path_buf())
            }
        }
    };
    if let Some(mm_path) = mmproj_file {
        args.push("--mmproj".to_string());
        args.push(mm_path.to_string_lossy().into_owned());
    }

    // Auto speculative decoding for MTP models (companion mtp-*.gguf or nextn predict layers)
    let mtp_file = match settings.mtp.as_deref() {
        Some("none") | Some("off") | Some("false") => None,
        Some("auto") | None => find_matching_mtp_layer(model_path),
        Some(custom) if !custom.trim().is_empty() => {
            let p = Path::new(custom.trim());
            if p.is_absolute() {
                Some(p.to_path_buf())
            } else if let Some(parent) = model_path.parent() {
                Some(parent.join(p))
            } else {
                Some(p.to_path_buf())
            }
        }
        _ => None,
    };

    if let Some(mtp_path) = mtp_file {
        args.push("-md".to_string());
        args.push(mtp_path.to_string_lossy().into_owned());
        if !settings.extra_args.contains("--spec-type") {
            args.push("--spec-type".to_string());
            args.push("draft-mtp".to_string());
        }
    } else if meta.is_some_and(GgufMeta::has_mtp) && !settings.extra_args.contains("--spec-") {
        args.push("--spec-type".to_string());
        args.push("draft-mtp".to_string());
    }

    // Auto-detect optimal -ncmoe for MoE models (e.g. Gemma 4 A4B) if not overridden
    if !settings.extra_args.contains("-ncmoe") {
        let model_fname = model_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if let Some(nc) = detect_optimal_ncmoe_from_name(model_fname) {
            args.push("-ncmoe".to_string());
            args.push(nc.to_string());
        }
    }

    for extra in settings.extra_args.split_whitespace() {
        if extra == "--no-mmap" {
            if let Some(pos) = args.iter().position(|a| a == "--load-mode") {
                args.remove(pos);
                if pos < args.len() && !args[pos].starts_with('-') {
                    args.remove(pos);
                }
            }
            args.push("--load-mode".to_string());
            args.push("none".to_string());
            continue;
        }

        if extra.starts_with('-') {
            let flag_name = extra.split('=').next().unwrap_or(extra);
            if let Some(pos) = args.iter().position(|a| a == flag_name) {
                args.remove(pos);
                if pos < args.len() && !args[pos].starts_with('-') {
                    args.remove(pos);
                }
            }
        }
        args.push(extra.to_string());
    }

    args
}

pub fn quote_arg(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_string();
    }
    if s.contains(' ') || s.contains('\t') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

pub fn find_matching_mmproj(model_path: &Path) -> Option<std::path::PathBuf> {
    let parent = model_path.parent()?;
    let model_name = model_path
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    let is_gemma4 = model_name.contains("gemma-4") || model_name.contains("gemma4");

    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension()
                && ext.eq_ignore_ascii_case("gguf")
            {
                let fname = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                if fname.contains("mmproj") {
                    if is_gemma4 && (fname.contains("gemma-4") || fname.contains("gemma4")) {
                        return Some(path);
                    } else if !is_gemma4 && !fname.contains("gemma") {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

pub fn find_matching_mtp_layer(model_path: &Path) -> Option<std::path::PathBuf> {
    let parent = model_path.parent()?;
    let model_name = model_path
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    let is_gemma4 = model_name.contains("gemma-4") || model_name.contains("gemma4");

    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("gguf") {
                        let fname = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_ascii_lowercase();
                        if fname.contains("mtp") {
                            if is_gemma4
                                && (fname.contains("gemma-4")
                                    || fname.contains("gemma4")
                                    || fname.starts_with("mtp-"))
                            {
                                return Some(path);
                            } else if !is_gemma4 && !fname.contains("gemma") {
                                return Some(path);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn detect_optimal_ncmoe_from_name(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let is_moe = lower.contains("moe")
        || lower.contains("a4b")
        || lower.contains("gemma-4")
        || lower.contains("gemma4")
        || lower.contains("mixtral")
        || lower.contains("dbrx");
    if !is_moe {
        return None;
    }
    if lower.contains("-q8") || lower.contains("_q8") || lower.contains(".q8") {
        Some(17)
    } else if lower.contains("-q6") || lower.contains("_q6") || lower.contains(".q6") {
        Some(14)
    } else if lower.contains("-q5") || lower.contains("_q5") || lower.contains(".q5") {
        Some(10)
    } else if lower.contains("-q4") || lower.contains("_q4") || lower.contains(".q4") {
        Some(6)
    } else if lower.contains("-q3") || lower.contains("_q3") || lower.contains(".q3") {
        Some(6)
    } else {
        None
    }
}

pub fn format_cmd(exe: &Path, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(quote_arg(&exe.to_string_lossy()));
    for a in args {
        parts.push(quote_arg(a));
    }
    parts.join(" ")
}

pub fn format_launch_params(s: &ModelSettings, ctx: u32) -> String {
    let mut parts = vec![
        format!("ctx={ctx}"),
        format!("ngl={}", s.ngl),
        format!("batch={}", s.batch),
        format!("ubatch={}", s.ubatch),
        format!("kv_type={}", s.kv_type),
        format!("flash_attn={}", s.flash_attn),
        format!("mlock={}", s.mlock),
        format!("mmap={}", s.mmap),
        format!("prompt_cache={}", s.prompt_cache),
        format!("reasoning_effort={}", s.reasoning_effort),
        format!("reasoning_preserve={}", s.reasoning_preserve),
    ];
    if let Some(ref mm) = s.mmproj {
        parts.push(format!("mmproj=\"{mm}\""));
    }
    if let Some(ref mtp) = s.mtp {
        parts.push(format!("mtp=\"{mtp}\""));
    }
    if !s.extra_args.trim().is_empty() {
        parts.push(format!("extra_args=\"{}\"", s.extra_args.trim()));
    }
    parts.join(" ")
}

#[allow(dead_code)]
pub const PARAMS: &[(&str, &str)] = &[
    ("temperature", "запрос"),
    ("top_k", "запрос"),
    ("top_p", "запрос"),
    ("min_p", "запрос"),
    ("repeat_penalty", "запрос"),
    ("max_tokens", "запрос"),
    ("ctx", "запуск"),
    ("ngl", "запуск"),
    ("batch", "запуск"),
    ("ubatch", "запуск"),
    ("kv_type", "запуск: f16/q8_0/q4_0"),
    ("flash_attn", "запуск: bool"),
    ("mlock", "запуск: bool"),
    ("mmap", "запуск: bool"),
    ("prompt_cache", "запуск: bool"),
    ("reasoning_preserve", "запуск: bool"),
    ("mmproj", "запуск: проектор"),
    ("mtp", "запуск: mtp драфт"),
    ("extra_args", "запуск"),
];

pub fn launch_params_changed(a: &ModelSettings, b: &ModelSettings) -> bool {
    a.ctx != b.ctx
        || a.ngl != b.ngl
        || a.batch != b.batch
        || a.ubatch != b.ubatch
        || a.kv_type != b.kv_type
        || a.flash_attn != b.flash_attn
        || a.mlock != b.mlock
        || a.mmap != b.mmap
        || a.prompt_cache != b.prompt_cache
        || a.reasoning_effort != b.reasoning_effort
        || a.reasoning_preserve != b.reasoning_preserve
        || a.mmproj != b.mmproj
        || a.mtp != b.mtp
        || a.extra_args != b.extra_args
}

#[allow(dead_code)]
pub fn param_val_str(s: &ModelSettings, idx: usize) -> String {
    match idx {
        0 => s.temperature.to_string(),
        1 => s.top_k.to_string(),
        2 => s.top_p.to_string(),
        3 => s.min_p.to_string(),
        4 => s.repeat_penalty.to_string(),
        5 => s.max_tokens.to_string(),
        6 => s.ctx.to_string(),
        7 => s.ngl.to_string(),
        8 => s.batch.to_string(),
        9 => s.ubatch.to_string(),
        10 => s.kv_type.clone(),
        11 => s.flash_attn.to_string(),
        12 => s.mlock.to_string(),
        13 => s.mmap.to_string(),
        14 => s.prompt_cache.to_string(),
        15 => s.reasoning_preserve.to_string(),
        16 => s.mmproj.clone().unwrap_or_else(|| "none".to_string()),
        17 => s.mtp.clone().unwrap_or_else(|| "auto".to_string()),
        _ => s.extra_args.clone(),
    }
}

#[allow(dead_code)]
pub fn toggle_or_cycle_param(s: &mut ModelSettings, idx: usize) -> bool {
    match idx {
        6 => s.ctx = crate::server::fit::cycle_ctx_preset(s.ctx, true),
        10 => {
            s.kv_type = match s.kv_type.as_str() {
                "f16" => "q8_0",
                "q8_0" => "q4_0",
                _ => "f16",
            }
            .into()
        }
        11 => s.flash_attn = !s.flash_attn,
        12 => s.mlock = !s.mlock,
        13 => s.mmap = !s.mmap,
        14 => s.prompt_cache = !s.prompt_cache,
        15 => s.reasoning_preserve = !s.reasoning_preserve,
        _ => return false,
    }
    true
}

#[allow(dead_code)]
pub fn apply_param_str(s: &mut ModelSettings, idx: usize, v: &str) {
    match idx {
        0 => s.temperature = v.parse().unwrap_or(s.temperature),
        1 => s.top_k = v.parse().unwrap_or(s.top_k),
        2 => s.top_p = v.parse().unwrap_or(s.top_p),
        3 => s.min_p = v.parse().unwrap_or(s.min_p),
        4 => s.repeat_penalty = v.parse().unwrap_or(s.repeat_penalty),
        5 => s.max_tokens = v.parse().unwrap_or(s.max_tokens),
        6 => s.ctx = v.parse().unwrap_or(s.ctx),
        7 => s.ngl = v.parse().unwrap_or(s.ngl),
        8 => s.batch = v.parse().unwrap_or(s.batch),
        9 => s.ubatch = v.parse().unwrap_or(s.ubatch),
        16 => {
            s.mmproj = if v.is_empty() || v == "none" {
                None
            } else {
                Some(v.to_string())
            }
        }
        17 => {
            s.mtp = if v.is_empty() || v == "none" {
                None
            } else {
                Some(v.to_string())
            }
        }
        18 => s.extra_args = v.to_string(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn builds_known_good_command_line() {
        let srv = ServerConfig::default();
        let settings = ModelSettings {
            reasoning_preserve: true,
            ..Default::default()
        };
        let args = build_server_args(
            Path::new("models/Qwen3.8.gguf"),
            &srv,
            &settings,
            None,
            Some(32768),
            None,
        );
        assert!(args.contains(&"--jinja".to_string()));
        assert!(args.contains(&"--reasoning-preserve".to_string()));
        assert!(
            args.windows(2)
                .any(|w| w == ["--reasoning-effort", "medium"])
        );
        assert!(args.windows(2).any(|w| w == ["--load-mode", "mlock"]));
        assert!(args.windows(2).any(|w| w == ["-ctk", "q8_0"]));
        assert!(args.windows(2).any(|w| w == ["-c", "32768"]));
    }

    #[test]
    fn adds_spec_type_mtp_when_model_has_nextn_layers() {
        let srv = ServerConfig::default();
        let settings = ModelSettings::default();
        let meta = GgufMeta {
            file_name: "m.gguf".to_string(),
            path: PathBuf::from("m.gguf"),
            total_size_bytes: 1000,
            quant_label: "IQ3_M".to_string(),
            architecture: "qwen3next".to_string(),
            block_count: 64,
            kv_layer_count: 16,
            head_count_kv: 8,
            head_dim: 128,
            context_length: 32768,
            nextn_predict_layers: 1,
            supports_reasoning: true,
            reasoning_mode: crate::gguf::ReasoningMode::Toggleable,
        };
        let args = build_server_args(
            Path::new("m.gguf"),
            &srv,
            &settings,
            Some(&meta),
            None,
            None,
        );
        assert!(args.windows(2).any(|w| w == ["--spec-type", "draft-mtp"]));
    }

    #[test]
    fn formats_cmd_with_quotes() {
        let exe = Path::new("C:\\Program Files\\Snarta\\llama-server.exe");
        let args = vec![
            "-m".to_string(),
            "C:\\models\\test model.gguf".to_string(),
            "-c".to_string(),
            "32768".to_string(),
        ];
        let cmd = format_cmd(exe, &args);
        assert_eq!(
            cmd,
            "\"C:\\Program Files\\Snarta\\llama-server.exe\" -m \"C:\\models\\test model.gguf\" -c 32768"
        );
    }

    #[test]
    fn formats_launch_params_summary() {
        let s = ModelSettings::default();
        let summary = format_launch_params(&s, 65536);
        assert!(summary.contains("ctx=65536"));
        assert!(summary.contains("ngl=99"));
        assert!(summary.contains("batch=2048"));
        assert!(summary.contains("kv_type=q8_0"));
    }

    #[test]
    fn applies_custom_context_and_extra_args() {
        let srv = ServerConfig::default();
        let settings = ModelSettings {
            ctx: 65536,
            mmap: false,
            mlock: false,
            extra_args: "-ncmoe 35".to_string(),
            ..Default::default()
        };
        let args = build_server_args(
            Path::new("models/gemma.gguf"),
            &srv,
            &settings,
            None,
            None,
            None,
        );
        assert!(args.windows(2).any(|w| w == ["-c", "65536"]));
        assert!(args.windows(2).any(|w| w == ["-ncmoe", "35"]));
        assert!(args.windows(2).any(|w| w == ["--load-mode", "none"]));
    }

    #[test]
    fn extra_args_overrides_defaults() {
        let srv = ServerConfig::default();
        let settings = ModelSettings {
            extra_args: "--reasoning-format deepseek -c 16384".to_string(),
            ..Default::default()
        };
        let args = build_server_args(
            Path::new("models/gemma.gguf"),
            &srv,
            &settings,
            None,
            None,
            None,
        );
        assert!(args.windows(2).any(|w| w == ["--reasoning-format", "deepseek"]));
        assert!(!args.contains(&"auto".to_string()));
        assert!(args.windows(2).any(|w| w == ["-c", "16384"]));
    }

    #[test]
    fn handles_legacy_no_mmap_in_extra_args() {
        let srv = ServerConfig::default();
        let settings = ModelSettings {
            extra_args: "-ncmoe 16 --no-mmap".to_string(),
            ..Default::default()
        };
        let args = build_server_args(
            Path::new("models/gemma.gguf"),
            &srv,
            &settings,
            None,
            None,
            None,
        );
        assert!(args.windows(2).any(|w| w == ["-ncmoe", "16"]));
        assert!(args.windows(2).any(|w| w == ["--load-mode", "none"]));
        assert!(!args.contains(&"--no-mmap".to_string()));
    }

    #[test]
    fn auto_detects_ncmoe_for_gemma4_moe() {
        let srv = ServerConfig::default();
        let settings = ModelSettings::default();
        let args = build_server_args(
            Path::new("models/gemma-4-26B-A4B-it-ultra-uncensored-heretic-Q4_K_M.gguf"),
            &srv,
            &settings,
            None,
            None,
            None,
        );
        assert!(args.windows(2).any(|w| w == ["-ncmoe", "6"]));
    }

    #[test]
    fn custom_mtp_draft_model_appended() {
        let srv = ServerConfig::default();
        let settings = ModelSettings {
            mtp: Some("mtp-gemma-4-26B-A4B-it.gguf".to_string()),
            ..Default::default()
        };
        let args = build_server_args(
            Path::new("models/gemma-4-26B-A4B.gguf"),
            &srv,
            &settings,
            None,
            None,
            None,
        );
        assert!(args.contains(&"-md".to_string()));
        assert!(args.windows(2).any(|w| w == ["--spec-type", "draft-mtp"]));
    }
}
