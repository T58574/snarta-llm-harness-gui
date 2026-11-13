use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

pub const GGUF_MAGIC: &[u8; 4] = b"GGUF";
pub const MAX_STRING_LEN: u64 = 1024 * 1024;
pub const MAX_ARRAY_SKIP_LEN: u64 = 2_000_000;
pub const DEFAULT_HEAD_DIM: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReasoningMode {
    #[default]
    None,
    Toggleable,
    ForcedInTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GgufMeta {
    pub file_name: String,
    pub path: PathBuf,
    pub total_size_bytes: u64,
    pub quant_label: String,
    pub architecture: String,
    pub block_count: u32,
    pub kv_layer_count: u32,
    pub head_count_kv: u32,
    pub head_dim: u32,
    pub context_length: u32,
    pub nextn_predict_layers: u32,
    pub supports_reasoning: bool,
    pub reasoning_mode: ReasoningMode,
}

impl GgufMeta {
    pub fn has_mtp(&self) -> bool {
        self.nextn_predict_layers > 0
    }
}

#[derive(Debug, Clone)]
enum MetaVal {
    U32(u32),
    Str(String),
    ArrayNonZeroCount { total: u32, non_zero: u32 },
}

fn collect_gguf_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 2 {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file()
                && p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("gguf"))
            {
                out.push(p);
            } else if p.is_dir()
                && let Some(name) = p.file_name().and_then(|s| s.to_str())
                && !name.starts_with('.')
            {
                collect_gguf_files(&p, depth + 1, out);
            }
        }
    }
}

pub fn scan_models_dir(models_dir: &Path) -> io::Result<Vec<GgufMeta>> {
    let mut files = Vec::new();
    collect_gguf_files(models_dir, 0, &mut files);
    files.sort();

    let mut models = Vec::new();
    for path in &files {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let lower = name.to_ascii_lowercase();
        let is_auxiliary = lower.contains("mmproj")
            || lower.contains("mtp")
            || lower.starts_with("mtp-")
            || is_non_first_shard(&lower);
        if !is_auxiliary {
            let mut meta = match parse_gguf_file(path) {
                Ok(m) => m,
                Err(_) => {
                    let total_size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                    let quant = infer_quant_from_filename(name);
                    let arch = infer_arch_from_filename(name);
                    let reasoning_mode = detect_reasoning_mode(None, &arch, name);
                    GgufMeta {
                        file_name: name.to_string(),
                        path: path.clone(),
                        total_size_bytes,
                        quant_label: quant,
                        architecture: arch,
                        block_count: 32,
                        kv_layer_count: 32,
                        head_count_kv: 8,
                        head_dim: DEFAULT_HEAD_DIM,
                        context_length: 32768,
                        nextn_predict_layers: 0,
                        supports_reasoning: reasoning_mode != ReasoningMode::None,
                        reasoning_mode,
                    }
                }
            };
            meta.total_size_bytes = sum_shard_sizes(path, &files);
            models.push(meta);
        }
    }
    Ok(models)
}

fn infer_arch_from_filename(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.contains("qwen") {
        "qwen2".to_string()
    } else if lower.contains("deepseek") {
        "deepseek2".to_string()
    } else if lower.contains("gemma") {
        "gemma2".to_string()
    } else if lower.contains("phi") {
        "phi3".to_string()
    } else if lower.contains("mistral") {
        "mistral".to_string()
    } else {
        "llama".to_string()
    }
}

pub fn is_non_first_shard(name_lower: &str) -> bool {
    name_lower.find("-of-").is_some_and(|p| {
        p >= 5
            && name_lower.is_char_boundary(p - 5)
            && name_lower[p - 5..p].chars().all(|c| c.is_ascii_digit())
            && &name_lower[p - 5..p] != "00001"
    })
}

fn sum_shard_sizes(first_path: &Path, all_files: &[PathBuf]) -> u64 {
    let default_len = fs::metadata(first_path).map(|m| m.len()).unwrap_or(0);
    let lower = first_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let Some((prefix, _)) = lower.split_once("-00001-of-") else {
        return default_len;
    };
    all_files
        .iter()
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.to_ascii_lowercase().starts_with(prefix))
        })
        .filter_map(|p| fs::metadata(p).ok().map(|m| m.len()))
        .sum()
}

pub fn parse_gguf_file(path: &Path) -> io::Result<GgufMeta> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("model.gguf");
    parse_gguf_reader(&mut BufReader::new(file), name, path.to_path_buf(), size)
}

pub fn parse_gguf_reader<R: Read>(
    r: &mut R,
    file_name: &str,
    path: PathBuf,
    total_size_bytes: u64,
) -> io::Result<GgufMeta> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != GGUF_MAGIC || !(2..=3).contains(&read_u32(r)?) {
        return Err(bad_gguf("invalid or unsupported GGUF header"));
    }
    let (_, kv_count) = (read_u64(r)?, read_u64(r)?);

    let mut map: HashMap<String, MetaVal> = HashMap::new();
    for _ in 0..kv_count.min(4096) {
        let (Ok(key), Ok(vtype)) = (read_string(r), read_u32(r)) else {
            break;
        };
        let Ok(val) = read_meta_value(r, vtype, &key, 0) else {
            break;
        };
        if let Some(v) = val {
            map.insert(key, v);
        }
    }

    let arch = match map.get("general.architecture") {
        Some(MetaVal::Str(s)) => s.clone(),
        _ => "llama".to_string(),
    };
    let block_count = get_u32(&map, &format!("{arch}.block_count")).unwrap_or(32);
    let head_dim = get_u32(&map, &format!("{arch}.attention.key_length"))
        .or_else(|| {
            let embd = get_u32(&map, &format!("{arch}.embedding_length"))?;
            embd.checked_div(get_u32(&map, &format!("{arch}.attention.head_count"))?)
        })
        .unwrap_or(DEFAULT_HEAD_DIM);

    let (head_count_kv, kv_layer_count) = extract_kv_heads_and_layers(&map, &arch, block_count);
    let context_length = get_u32(&map, &format!("{arch}.context_length")).unwrap_or(32768);
    let nextn_predict_layers = get_u32(&map, &format!("{arch}.nextn_predict_layers")).unwrap_or(0);
    let chat_template = match map.get("tokenizer.chat_template") {
        Some(MetaVal::Str(tpl)) => Some(tpl.as_str()),
        _ => None,
    };
    let reasoning_mode = detect_reasoning_mode(chat_template, &arch, file_name);
    let supports_reasoning = reasoning_mode != ReasoningMode::None;
    Ok(GgufMeta {
        file_name: file_name.to_string(),
        path,
        total_size_bytes,
        quant_label: infer_quant_from_filename(file_name),
        architecture: arch,
        block_count,
        kv_layer_count,
        head_count_kv,
        head_dim,
        context_length,
        nextn_predict_layers,
        supports_reasoning,
        reasoning_mode,
    })
}

pub fn detect_reasoning_mode(
    chat_template: Option<&str>,
    arch: &str,
    file_name: &str,
) -> ReasoningMode {
    if let Some(tpl) = chat_template {
        if tpl.contains("enable_thinking") || tpl.contains("reasoning_effort") {
            return ReasoningMode::Toggleable;
        }
        if tpl.contains("<think")
            || tpl.contains("<|channel>thought")
            || tpl.contains("<|thought")
            || tpl.contains("reasoning")
        {
            return ReasoningMode::ForcedInTemplate;
        }
        return ReasoningMode::None;
    }

    let arch_lower = arch.to_ascii_lowercase();
    let name_lower = file_name.to_ascii_lowercase();
    if arch_lower.contains("qwen3")
        || arch_lower.contains("deepseek")
        || arch_lower.contains("gemma4")
        || name_lower.contains("think")
        || name_lower.contains("-r1")
        || name_lower.contains("qwen3")
        || name_lower.contains("gemma-4")
        || name_lower.contains("gemma4")
    {
        ReasoningMode::Toggleable
    } else {
        ReasoningMode::None
    }
}

fn bad_gguf(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn extract_kv_heads_and_layers(
    map: &HashMap<String, MetaVal>,
    arch: &str,
    block_count: u32,
) -> (u32, u32) {
    match map.get(&format!("{arch}.attention.head_count_kv")) {
        Some(MetaVal::U32(h)) => {
            let interval = get_u32(map, &format!("{arch}.full_attention_interval")).unwrap_or(1);
            let kv_layers = if interval > 1 {
                block_count.div_ceil(interval)
            } else {
                block_count
            };
            (*h, kv_layers)
        }
        Some(MetaVal::ArrayNonZeroCount { total, non_zero }) => (
            total.checked_div(*non_zero).unwrap_or(8).max(1),
            (*non_zero).max(1),
        ),
        _ => (8, block_count),
    }
}

fn get_u32(map: &HashMap<String, MetaVal>, key: &str) -> Option<u32> {
    match map.get(key) {
        Some(MetaVal::U32(v)) => Some(*v),
        _ => None,
    }
}

const MAX_GGUF_ARRAY_DEPTH: u8 = 8;

fn read_meta_value<R: Read>(
    r: &mut R,
    vtype: u32,
    key: &str,
    depth: u8,
) -> io::Result<Option<MetaVal>> {
    match vtype {
        0 | 1 | 7 => {
            let mut b = [0u8; 1];
            r.read_exact(&mut b)
                .map(|()| Some(MetaVal::U32(b[0] as u32)))
        }
        2 | 3 => {
            let mut b = [0u8; 2];
            r.read_exact(&mut b)
                .map(|()| Some(MetaVal::U32(u16::from_le_bytes(b) as u32)))
        }
        4 | 5 => Ok(Some(MetaVal::U32(read_u32(r)?))),
        6 => read_u32(r).map(|_| None),
        8 => {
            let s = read_string(r)?;
            let keep = key == "general.architecture" || key == "tokenizer.chat_template";
            Ok(keep.then_some(MetaVal::Str(s)))
        }
        9 => read_array_value(r, key, depth),
        10..=12 => Ok(Some(MetaVal::U32(read_u64(r)? as u32))),
        _ => Err(bad_gguf("unknown GGUF type")),
    }
}

fn read_array_value<R: Read>(
    r: &mut R,
    key: &str,
    depth: u8,
) -> io::Result<Option<MetaVal>> {
    if depth >= MAX_GGUF_ARRAY_DEPTH {
        return Err(bad_gguf("array nesting too deep"));
    }
    let (elem_type, len) = (read_u32(r)?, read_u64(r)?);
    if len > MAX_ARRAY_SKIP_LEN {
        return Err(bad_gguf("array too large"));
    }
    if key.ends_with(".attention.head_count_kv") && (elem_type == 4 || elem_type == 5) {
        let (mut total, mut non_zero) = (0u32, 0u32);
        for _ in 0..len {
            let v = read_u32(r)?;
            if v > 0 {
                total = total.saturating_add(v);
                non_zero = non_zero.saturating_add(1);
            }
        }
        return Ok(Some(MetaVal::ArrayNonZeroCount { total, non_zero }));
    }
    for _ in 0..len {
        let _ = read_meta_value(r, elem_type, "", depth + 1)?;
    }
    Ok(None)
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).map(|()| u32::from_le_bytes(b))
}

fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).map(|()| u64::from_le_bytes(b))
}

fn read_string<R: Read>(r: &mut R) -> io::Result<String> {
    let len = read_u64(r)?;
    if len > MAX_STRING_LEN {
        io::copy(&mut r.take(len), &mut io::sink())?;
        return Ok(String::new());
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

pub fn infer_quant_from_filename(name: &str) -> String {
    let stem = name.strip_suffix(".gguf").unwrap_or(name);
    if let Some(idx) = stem.to_ascii_uppercase().rfind("-IQ") {
        return stem[idx + 1..].to_ascii_uppercase();
    }
    for part in stem.rsplit(['-', '_', '.']) {
        let up = part.to_ascii_uppercase();
        if up.starts_with("IQ") || up.starts_with('Q') || up == "F16" || up == "BF16" {
            return up;
        }
    }
    "GGUF".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_synthetic_hybrid_qwen3_gguf() {
        let mut b = Vec::from(&GGUF_MAGIC[..]);
        b.extend([3, 0, 0, 0]);
        b.extend([0; 8]);
        b.extend([0; 8]);
        let m =
            parse_gguf_reader(&mut io::Cursor::new(b), "M-IQ3_M.gguf", PathBuf::new(), 10).unwrap();
        assert_eq!(m.quant_label, "IQ3_M");
        assert_eq!(m.reasoning_mode, ReasoningMode::None);
    }

    #[test]
    fn detects_reasoning_modes_from_template_and_arch() {
        assert_eq!(
            detect_reasoning_mode(
                Some("{%- if enable_thinking -%}<|channel>thought\n{%- endif -%}"),
                "gemma4",
                "gemma-4-26B-A4B-it-Q8_0.gguf"
            ),
            ReasoningMode::Toggleable
        );
        assert_eq!(
            detect_reasoning_mode(
                Some("{{ bos_token }}<|turn>model\n<|channel>thought\n"),
                "gemma4",
                "gemma-4-26B-A4B-it-Q8_0.gguf"
            ),
            ReasoningMode::ForcedInTemplate
        );
        assert_eq!(
            detect_reasoning_mode(
                Some("{{ bos_token }}<|turn>model\n"),
                "gemma4",
                "gemma-4-26B-A4B-it-Q8_0.gguf"
            ),
            ReasoningMode::None
        );
        assert_eq!(
            detect_reasoning_mode(None, "gemma4", "gemma-4-26B-A4B-it-Q8_0.gguf"),
            ReasoningMode::Toggleable
        );
    }
}
