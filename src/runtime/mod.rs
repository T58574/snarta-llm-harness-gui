pub mod install;
pub mod releases;

use crate::gpu;

pub fn detect_preferred_gfx() -> Option<String> {
    gpu::detect_gpus()
        .ok()
        .and_then(|gpus| gpus.into_iter().find_map(|g| g.gfx_target))
}
