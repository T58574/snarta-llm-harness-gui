# Snarta (`snarta-llm-harness-gui`)

**English** | [Русский](README.ru.md)

A native Rust LLM harness and desktop GUI client built around `llama.cpp` (`llama-server`), engineered from the ground up to **minimize VRAM and system RAM overhead**.

## Why Snarta

Most local LLM GUI launchers and agent harnesses are built on Electron, WebView, or heavy Python runtimes. They consume hundreds of megabytes to gigabytes of RAM and VRAM before a model even loads, and when you push context length higher, that extra footprint triggers silent spilling of weights and KV cache into shared system memory over PCIe, severely degrading generation speed.

**Snarta** strips away those layers so the maximum possible physical memory stays available for the model and its context:

* **Native stack with no browser engine.** A single compact Rust binary with GPU-accelerated UI rendering (`gpui` / `gpui-kit`) — no Electron, Node.js, or Python in the core runtime.
* **Accurate VRAM & KV-cache budgeting.** Reads real physical VRAM on Windows via DXGI and PDH (`\GPU Adapter Memory(*)\Dedicated Usage`) instead of capped WMI values, calculates context fit (`fit`) with explicit shared-RAM spill reporting (`spill_mib`), and defaults to quantized KV cache (`q8_0`).
* **Pre-launch memory reclamation (`prepare`).** Quickly closes background VRAM/RAM-heavy apps (browsers, overlays, chat clients) before loading large models.
* **MoE & large-context tuning.** Supports `-ncmoe` CPU expert offloading and includes a benchmark suite for tuning throughput.

## Features

* **Desktop GUI (`gpui`):** streaming output, collapsible reasoning blocks, clipboard and file-picker image attachments for vision models, persona management, and local session persistence.
* **Agentic loop (`AgentEngine`):** built-in tool calling with dedicated UI cards (filesystem, `edit_file`, shell execution, local search, web search/scrape, plan mode, interactive user prompts) and loop protection.
* **Runtime management:** downloads and switches `llama.cpp` builds (ROCm for AMD gfx110X and Vulkan), builds launch flags cleanly, and supports `extra_args` and `[model_defaults]` inheritance in `snarta.toml`.
* **Benchmarking (`bench/`):** TUI runner, MMLU/stress evaluations, and automated `-ncmoe` sweeps (see [bench/README.md](bench/README.md)).

## Requirements

* **Target OS:** Windows x64 (primary platform for GUI, DXGI/PDH telemetry, and `llama-server.exe`).
* **Toolchain:** Rust (`edition = "2024"`).

## Building

### On Windows (MSVC, recommended for release)

```powershell
cargo build --release
```

The compiled binary will be at `target\release\snarta.exe`.

### Cross-compiling / checking from Linux (`mingw-w64`)

`.cargo/config.toml` configures `x86_64-w64-mingw32-gcc` as the linker for `x86_64-pc-windows-gnu`:

```bash
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

### Tests

```bash
cargo test
```

## Usage

Running without arguments launches the desktop GUI:

```powershell
snarta
```

Available CLI subcommands:

* `snarta doctor` — inspect environment, config, and runtime health.
* `snarta prepare` — free RAM/VRAM according to `[prepare]` in `snarta.toml`.
* `snarta runtime ...` — manage installed `llama.cpp` builds.
* `snarta fit [model]` — calculate context fit against current free VRAM.
* `snarta serve [model]` / `snarta stop` — start and stop `llama-server`.
* `snarta bench [model]` — run the built-in benchmark.
* `snarta path install` / `snarta path uninstall` — add/remove the directory containing `snarta.exe` from user `PATH`.

## Repository Structure

* `src/main.rs` — entry point and CLI dispatch.
* `src/gui/` — `gpui` / `gpui-kit` UI, tool cards, modals, and state.
* `src/chat/` — `llama-server` client, prompt builder, `AgentEngine`, sessions, and `Workspace`.
* `src/server/` — `ServerController`, launch argument builder, and context `fit`.
* `src/tools/` — tool registry and implementations.
* `src/gpu.rs`, `src/sysmon/` — DXGI/PDH telemetry and process cleanup.
* `src/runtime/` — `llama.cpp` release installer and manager.
* `bench/` — Python benchmarking and evaluation scripts.
* `docs/JOURNAL.md` — architecture decisions and technical lessons learned.

## License

MIT (see [Cargo.toml](Cargo.toml)).
