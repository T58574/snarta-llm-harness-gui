use crate::config::{ModelSettings, VramConfig};
use crate::gguf::GgufMeta;

pub const BYTES_PER_MIB: u64 = 1024 * 1024;
pub const MIN_CTX_TOKENS: u32 = 2048;
pub const CTX_STEP_TOKENS: u32 = 1024;
pub const BASE_COMPUTE_MIB: u64 = 220;
pub const RECURRENT_STATE_MIB_HYBRID: u64 = 128;

#[allow(dead_code)]
pub const CTX_PRESETS: &[u32] = &[8192, 16384, 32768, 65536, 131072, 262144];

#[allow(dead_code)]
pub fn format_ctx_k(ctx: u32) -> String {
    if ctx.is_multiple_of(1024) {
        format!("{}k", ctx / 1024)
    } else {
        format!("{ctx}")
    }
}

#[allow(dead_code)]
pub fn cycle_ctx_preset(cur: u32, forward: bool) -> u32 {
    let presets = CTX_PRESETS;
    if forward {
        for &p in presets {
            if p > cur {
                return p;
            }
        }
        presets[presets.len() - 1]
    } else {
        for &p in presets.iter().rev() {
            if p < cur {
                return p;
            }
        }
        presets[0]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FitEstimate {
    pub free_vram_mib: u64,
    pub reserve_mib: u64,
    pub weights_mib: u64,
    pub compute_mib: u64,
    pub kv_mib_at_chosen_ctx: u64,
    pub total_mib_at_chosen_ctx: u64,
    pub max_fitting_ctx: u32,
    pub chosen_ctx: u32,
    pub fits_in_vram: bool,
    pub spill_mib: u64,
}

impl FitEstimate {
    pub fn summary_line(&self, meta: &GgufMeta) -> String {
        format!(
            "- {} ({}, {}, {}/{}, MTP:{}) -> w={}M comp={}M KV@{}={}M tot={}M max={} fits={} spill={}M",
            meta.file_name,
            meta.quant_label,
            meta.architecture,
            meta.kv_layer_count,
            meta.block_count,
            meta.has_mtp(),
            self.weights_mib,
            self.compute_mib,
            self.chosen_ctx,
            self.kv_mib_at_chosen_ctx,
            self.total_mib_at_chosen_ctx,
            self.max_fitting_ctx,
            self.fits_in_vram,
            self.spill_mib,
        )
    }
}

pub fn calculate_fit(
    meta: &GgufMeta,
    settings: &ModelSettings,
    vram_cfg: &VramConfig,
    free_vram_mib: u64,
) -> FitEstimate {
    let mut weights_bytes = meta.total_size_bytes;
    if let Some(ref mm) = settings.mmproj
        && !mm.is_empty()
        && mm != "none"
        && mm != "auto"
    {
        let mm_path = if std::path::Path::new(mm).is_absolute() {
            std::path::PathBuf::from(mm)
        } else if let Some(parent) = meta.path.parent() {
            parent.join(mm)
        } else {
            std::path::PathBuf::from(mm)
        };
        if let Ok(m) = std::fs::metadata(&mm_path) {
            weights_bytes += m.len();
        } else {
            weights_bytes += 650 * BYTES_PER_MIB;
        }
    }
    let weights_mib = weights_bytes.div_ceil(BYTES_PER_MIB);
    let ubatch_scale = (settings.ubatch.max(128) as u64).div_ceil(512);
    let hybrid_extra = if meta.kv_layer_count < meta.block_count {
        RECURRENT_STATE_MIB_HYBRID
    } else {
        0
    };
    let compute_mib = BASE_COMPUTE_MIB * ubatch_scale + hybrid_extra;

    let budget_mib = free_vram_mib.saturating_sub(vram_cfg.reserve_mib);
    let fixed_mib = weights_mib + compute_mib;

    let cfg_ceiling = if vram_cfg.max_ctx_ceiling > 0 {
        vram_cfg.max_ctx_ceiling.max(MIN_CTX_TOKENS)
    } else {
        262_144
    };
    let ceiling = if meta.context_length > 0 {
        cfg_ceiling.min(meta.context_length.max(MIN_CTX_TOKENS))
    } else {
        cfg_ceiling
    };

    let mut max_fitting_ctx = 0u32;
    if fixed_mib < budget_mib {
        let avail_for_kv_mib = budget_mib - fixed_mib;
        let mut ctx = MIN_CTX_TOKENS;
        while ctx <= ceiling {
            let kv = estimate_kv_mib(meta, &settings.kv_type, ctx);
            if kv <= avail_for_kv_mib {
                max_fitting_ctx = ctx;
            } else {
                break;
            }
            ctx += CTX_STEP_TOKENS;
        }
    }

    let chosen_ctx = if settings.ctx > 0 {
        settings.ctx.max(MIN_CTX_TOKENS)
    } else if max_fitting_ctx > 0 {
        max_fitting_ctx
    } else {
        MIN_CTX_TOKENS
    };

    let kv_mib = estimate_kv_mib(meta, &settings.kv_type, chosen_ctx);
    let total_mib = fixed_mib + kv_mib;
    let fits_in_vram = max_fitting_ctx > 0 && chosen_ctx <= max_fitting_ctx;
    let spill_mib = total_mib.saturating_sub(free_vram_mib);

    FitEstimate {
        free_vram_mib,
        reserve_mib: vram_cfg.reserve_mib,
        weights_mib,
        compute_mib,
        kv_mib_at_chosen_ctx: kv_mib,
        total_mib_at_chosen_ctx: total_mib,
        max_fitting_ctx,
        chosen_ctx,
        fits_in_vram,
        spill_mib,
    }
}

pub fn estimate_kv_mib(meta: &GgufMeta, kv_type: &str, ctx: u32) -> u64 {
    let bytes_per_elem_x2: u64 = match kv_type {
        "f16" => 4,  // 2.0 bytes * 2 (K and V)
        "q8_0" => 2, // ~1.06 bytes * 2 -> ~2.125 bytes
        "q4_0" => 1, // ~0.56 bytes * 2 -> ~1.125 bytes
        _ => 2,
    };

    // Total elements per token across KV layers: kv_layer_count * head_count_kv * head_dim
    let elems_per_token = (meta.kv_layer_count as u64)
        .saturating_mul(meta.head_count_kv as u64)
        .saturating_mul(meta.head_dim as u64);
    // Add 10% overhead for quantization block scales
    let raw_bytes = (ctx as u64)
        .saturating_mul(elems_per_token)
        .saturating_mul(bytes_per_elem_x2)
        .saturating_mul(11)
        / 10;
    raw_bytes.div_ceil(BYTES_PER_MIB).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_qwen38_meta() -> GgufMeta {
        GgufMeta {
            file_name: "Qwen3.8-27B-IQ3_M.gguf".to_string(),
            path: PathBuf::from("Qwen3.8-27B-IQ3_M.gguf"),
            total_size_bytes: 14_100 * BYTES_PER_MIB,
            quant_label: "IQ3_M".to_string(),
            architecture: "qwen3next".to_string(),
            block_count: 64,
            kv_layer_count: 16,
            head_count_kv: 4,
            head_dim: 128,
            context_length: 65536,
            nextn_predict_layers: 1,
            supports_reasoning: true,
            reasoning_mode: crate::gguf::ReasoningMode::Toggleable,
        }
    }

    #[test]
    fn fits_qwen38_in_16gb_vram_and_caps_ctx() {
        let meta = sample_qwen38_meta();
        let settings = ModelSettings::default();
        let vram_cfg = VramConfig::default();

        // Suppose 15_600 MiB is free on RX 7800 XT after DWM
        let est = calculate_fit(&meta, &settings, &vram_cfg, 15_600);
        assert!(est.fits_in_vram);
        assert!(est.chosen_ctx >= 8192);
        assert!(est.total_mib_at_chosen_ctx <= 15_600 - vram_cfg.reserve_mib);
    }

    #[test]
    fn detects_when_free_vram_is_too_low() {
        let meta = sample_qwen38_meta();
        let settings = ModelSettings::default();
        let vram_cfg = VramConfig::default();

        let est = calculate_fit(&meta, &settings, &vram_cfg, 12_000);
        assert!(!est.fits_in_vram);
        assert_eq!(est.max_fitting_ctx, 0);
    }

    #[test]
    fn calculates_spill_at_large_context() {
        let meta = sample_qwen38_meta();
        let settings = ModelSettings {
            ctx: 131072,
            ..Default::default()
        };
        let vram_cfg = VramConfig::default();

        let est = calculate_fit(&meta, &settings, &vram_cfg, 15_600);
        assert_eq!(est.chosen_ctx, 131072);
        assert!(!est.fits_in_vram);
        assert!(est.max_fitting_ctx >= 16384);
        assert!(est.spill_mib > 0);
    }

    #[test]
    fn cycles_ctx_presets() {
        assert_eq!(cycle_ctx_preset(8192, true), 16384);
        assert_eq!(cycle_ctx_preset(16384, true), 32768);
        assert_eq!(cycle_ctx_preset(32768, true), 65536);
        assert_eq!(cycle_ctx_preset(65536, true), 131072);
        assert_eq!(cycle_ctx_preset(131072, true), 262144);
        assert_eq!(cycle_ctx_preset(262144, true), 262144);

        assert_eq!(cycle_ctx_preset(262144, false), 131072);
        assert_eq!(cycle_ctx_preset(131072, false), 65536);
        assert_eq!(cycle_ctx_preset(65536, false), 32768);
        assert_eq!(cycle_ctx_preset(32768, false), 16384);
        assert_eq!(cycle_ctx_preset(16384, false), 8192);
        assert_eq!(cycle_ctx_preset(8192, false), 8192);

        assert_eq!(format_ctx_k(32768), "32k");
        assert_eq!(format_ctx_k(65536), "64k");
        assert_eq!(format_ctx_k(131072), "128k");
        assert_eq!(format_ctx_k(262144), "256k");
    }
}
