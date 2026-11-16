# Snarta (`snarta-llm-harness-gui`)

**English** | [Русский](README.ru.md)

Snarta is a native Rust desktop GUI client and local agent harness for `llama.cpp` (`llama-server`). It keeps memory overhead low so weights and KV cache fit into physical VRAM.

## Why use Snarta

Browser-based wrappers such as Electron or WebView reserve hundreds of megabytes of RAM and VRAM before loading a model. On tight GPUs, that extra footprint pushes model layers or KV cache into shared system RAM over PCIe, which cuts token speed.

Snarta avoids browser runtimes and Python servers in its main binary:

- **Native GPUI interface**: Compiled Rust GUI rendered on the GPU with `gpui` and `gpui-component`.
- **Direct VRAM telemetry on Windows**: Reads physical dedicated VRAM through DXGI and PDH counters (`\GPU Adapter Memory(*)\Dedicated Usage`) instead of the capped WMI value. Calculates context fit with explicit shared-memory spill estimates and defaults KV cache quantization to `q8_0`.
- **Pre-launch cleanup (`Prepare`)**: Lists background apps that hold RAM and VRAM (browsers, overlays, chat clients) and closes selected processes from the GUI.
- **MoE and long-context controls**: Supports `-ncmoe` expert offloading and per-model launch settings in `snarta.toml`.

## Features

- **Streaming chat and reasoning**: Shows live token streaming, collapsible thinking blocks, image attachments from clipboard or file picker, session history, and persona switching (`personas/*.md`).
- **Live ASCII persona portrait**: Displays a gender-aware ASCII face (`female` or `male` via persona frontmatter `gender`) that reacts to tool runs, thinking states, and direct model `set_emotion` calls.
- **Built-in agent tools**: Renders structured cards for file reads/edits/writes, shell execution, local search, web search and scraping, task plans, user questions (`ask_user`), and emotion changes.
- **Runtime manager**: Downloads and switches `llama.cpp` builds (ROCm for AMD gfx110X and Vulkan) inside the settings window.
- **Python benchmark suite**: Measures token speed, runs `-ncmoe` sweeps, and evaluates MMLU, RuMMLU, and stress tasks under `bench/`. See [bench/README.md](bench/README.md).

## Requirements

- Target OS: Windows x64 (for DXGI/PDH VRAM telemetry, process cleanup, and `llama-server.exe`).
- Toolchain: Rust (`edition = "2024"`).

## Build and verify

### Windows (MSVC)

```powershell
cargo build --release
```

The executable is written to `target\release\snarta.exe`.

### Linux cross-check (`mingw-w64`)

`.cargo/config.toml` configures `x86_64-w64-mingw32-gcc` for `x86_64-pc-windows-gnu`:

```bash
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

### Tests

```bash
cargo test
```

## Running

Launch the desktop application directly:

```powershell
snarta.exe
```

All configuration, diagnostics, runtime installation, and memory cleanup tools live in the GUI windows (`Settings`, `Models`, `Personas`, `Prepare`). Runtime folders (`models/`, `runtime/`, `logs/`, `personas/`, `life/`) and `snarta.toml` are created next to the binary or in the active workspace.

## Repository layout

- `src/main.rs`: Desktop application entry point.
- `src/gui/`: GPUI windows, sidebar, chat view, Lucide icon wrappers, ASCII avatar, modals, and tool cards.
- `src/chat/`: `llama-server` HTTP client, prompt builder, `AgentEngine`, session storage, and workspace loader.
- `src/server/`: `ServerController`, launch flag builder, process lifecycle, and context fit math.
- `src/tools/`: Agent tool registry and implementations.
- `src/gpu.rs`, `src/sysmon/`: DXGI/PDH GPU monitoring and process cleanup helpers.
- `src/runtime/`: `llama.cpp` release discovery and installer.
- `bench/`: Python scripts for `-ncmoe` speed sweeps and MMLU evaluations.
- `docs/JOURNAL.md`: Technical notes and architecture decisions.

## License

MIT. See [Cargo.toml](Cargo.toml).
