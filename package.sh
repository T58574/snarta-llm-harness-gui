#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

./build.sh

STAGE_ROOT="target/release-pkg"
PKG_DIR="$STAGE_ROOT/Snarta"
ZIP_OUT="target/snarta-windows-x64.zip"

rm -rf "$STAGE_ROOT" "$ZIP_OUT"
mkdir -p \
  "$PKG_DIR/personas" \
  "$PKG_DIR/life/notes" \
  "$PKG_DIR/life/sessions" \
  "$PKG_DIR/models" \
  "$PKG_DIR/runtime" \
  "$PKG_DIR/logs"

EXE_PATH="target/x86_64-pc-windows-gnu/release/snarta.exe"
if [ ! -f "$EXE_PATH" ]; then
  EXE_PATH="target/release/snarta.exe"
fi

cp "$EXE_PATH" "$PKG_DIR/snarta.exe"
cp README.md "$PKG_DIR/README.md"

cat > "$PKG_DIR/personas/default.md" <<'EOF'
+++
description = "Спокойный, точный собеседник без лишней воды"
+++
Ты — личный локальный ИИ-собеседник, работающий на машине пользователя через Snarta.
Отвечай по делу, без воды, канцелярита и извинений.
Когда узнаёшь важный долгосрочный факт о собеседнике, его планах, привычках или предпочтениях — запиши одну короткую строку через инструмент remember.
Если нужно сохранить развёрнутую заметку или черновик — используй write_note внутри папки notes/.
EOF

cat > "$PKG_DIR/snarta.toml" <<'EOF'
default_persona = "default"

[server]
host = "127.0.0.1"
port = 8089
health_timeout_secs = 120

[vram]
reserve_mib = 512
shared_spill_threshold_mib = 128
max_ctx_ceiling = 32768
start_ctx = 8192

[runtime]
max_speed_drop_pct = 10
rocm_repo = "lemonade-sdk/llamacpp-rocm"
vulkan_repo = "ggml-org/llama.cpp"

[limits]
compact_threshold_pct = 80
memory_token_limit = 2048
max_tool_steps = 8
max_read_file_bytes = 262144
log_max_bytes = 5242880

[prepare]
kill_list = [
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
]
never_list = []
dry_run = false

[model_defaults]
temperature = 1.0
top_k = 20
top_p = 0.95
min_p = 0.0
repeat_penalty = 1.0
max_tokens = 4096
ctx = 32768
ngl = 99
batch = 2048
ubatch = 512
kv_type = "q8_0"
flash_attn = true
mlock = true
mmap = true
prompt_cache = false
reasoning_effort = "medium"
reasoning_preserve = false
extra_args = ""

[models]
EOF

python3 - "$STAGE_ROOT" "$ZIP_OUT" <<'PY'
import os, sys, zipfile

stage_root, zip_out = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(zip_out, "w", zipfile.ZIP_DEFLATED) as zf:
    for root, dirs, files in os.walk(stage_root):
        rel_root = os.path.relpath(root, stage_root).replace("\\", "/")
        if not files and not dirs and rel_root != ".":
            zf.writestr(rel_root + "/", "")
        for f in files:
            full = os.path.join(root, f)
            arc = os.path.relpath(full, stage_root).replace("\\", "/")
            zf.write(full, arc)
PY

ls -lh "$ZIP_OUT"
