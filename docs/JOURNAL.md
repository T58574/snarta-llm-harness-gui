# Project Journal — snarta-llm-harness-gui

- `last_updated`: 2026-11-13T00:12:10Z
- `scope`: Snarta LLM Harness & GPUI desktop client (`snarta-llm-harness-gui`)
- `owner`: T58574

## Current State

- 2026-11-13: Repository initialized as a clean public baseline (`snarta-llm-harness-gui`) with GPUI desktop UI, `ServerController` (`src/server/controller.rs`), `AgentEngine` (`src/chat/engine.rs`), `Workspace` (`src/chat/workspace.rs`), and `tools::registry` (`src/tools/registry.rs`). (`verified-this-turn`, clean `git archive` export from prior baseline)

## Active Decisions

- **GUI subsystem & headless subprocesses on Windows**: Use `#![windows_subsystem = "windows"]` with conditional `AttachConsole(ATTACH_PARENT_PROCESS)` for CLI subcommands (`src/main.rs`), and `CREATE_NO_WINDOW` (`0x0800_0000`) when spawning `llama-server.exe` and shell subprocesses (`src/server/launch.rs`, `src/runtime/install.rs`, `src/tools/shell.rs`). (`verified-earlier`)
- **GPUI macro & test disambiguation**: Set `#![recursion_limit = "1024"]` in `src/main.rs` and annotate synchronous unit tests in GPUI-coupled modules with `#[core::prelude::v1::test]` so `gpui::test` does not shadow `#[test]`. (`verified-earlier`)
- **GPUI asset & modal lifecycle**: Initialize `gpui_kit::application().with_assets(gpui_kit::assets::AllAssets)` for Lucide SVG icons, store reactive modal state in `Rc<RefCell<State>>` with `state_entity.update(...)`, and initialize `mpsc::channel()` in `InputDock::new` rather than `render()`. (`verified-earlier`)
- **Context & settings resolution**: Resolve per-model settings through `Config::model_settings(&self, name)` (falling back to `[model_defaults]`), unify context config under `model_defaults.ctx` and `vram.max_ctx_ceiling`, and preserve requested `chosen_ctx` in `calculate_fit` while reporting `spill_mib` when overflowing into shared RAM. (`verified-earlier`)
- **Reasoning & tool-calling protocol**: Pass `--reasoning-format auto` and `-fit off` to `llama-server`; sanitize truncated `function.arguments` via `sanitize_tool_arguments_for_server` before echoing tool calls back; pass `tools: None` when `ToolLoopGuard` reaches its step limit. (`verified-earlier`)
- **Line endings**: Enforce `eol=lf` via `.gitattributes` so `edit_file` string matching is not broken by Windows CRLF conversions. (`verified-earlier`)

## Technical Lessons Learned (llama.cpp, Windows, GPUI, Rust)

### llama.cpp & GPU / VRAM
- **Shared RAM spill**: On Windows, exceeding physical VRAM silently spills into shared system memory over PCIe (e.g. 64k ctx on a 27B IQ3_M model on a 16 GB RX 7800 XT drops generation from ~28.6 t/s to ~15.9 t/s). Size context from measured free VRAM and monitor the `llama-server` Shared Usage PDH counter.
- **ROCm vs Vulkan on RDNA3 (gfx110X)**: Prefer HIP builds (`lemonade-sdk/llamacpp-rocm`) over Vulkan for 27B dense models; verify `ggml-hip.dll`, `rocblas.dll`, and `--list-devices` showing `ROCm0`. Match `-x64` explicitly when filtering release assets.
- **Release filtering**: Continuous release pipelines (e.g. `ggml-org/llama.cpp`) mark daily builds as `prerelease: true`; filter only on `rel.draft`, and scan both `runtime/` root and subdirectories in `list_installed`.
- **VRAM measurement**: WMI `Win32_VideoController.AdapterRAM` clamps at `uint32` (4095 MiB); query DXGI (`CreateDXGIFactory1` with minimal `#[repr(C)]` vtables in `src/gpu.rs`). For real physical VRAM usage matching Task Manager, query `\GPU Adapter Memory(*)\Dedicated Usage` matched by DXGI adapter LUID rather than summing `\GPU Process Memory(*)`.
- **Speculative decoding & MoE**: Keep `--repeat-penalty 1.0` when `draft-mtp` is active; pass `--spec-type draft-mtp` (not `mtp`). For large MoE models that spill across PCIe, use `-ncmoe` CPU expert offloading calibrated via `bench/bench_ncmoe.py`.
- **Log parsing**: `llama-server` emits 400+ lines on load; parse telemetry from the live stream instead of a fixed tail buffer.

### Windows & Rust FFI
- **`windows-sys` 0.61+ & `ureq` 3.5+**: `PDH_HQUERY` / `PDH_HCOUNTER` are `*mut c_void` (`null_mut()`), `BOOL` in callbacks is `windows_sys::core::BOOL` (`i32`), `GlobalFree` is in `windows_sys::Win32::Foundation::GlobalFree`. In `ureq` 3.x, use `Agent::config_builder().timeout_global(...)` and `resp.body_mut().read_json()`.
- **Aligned PDH & UTF-16 buffers**: Back `PdhGetFormattedCounterArrayW` buffers with `Vec<u64>` (8-byte aligned) and check `item.FmtValue.CStatus <= 1`; read `CF_HDROP` UTF-16 paths via `chunks_exact(2)` + `u16::from_le_bytes`.
- **Clipboard images**: Synthesize a 14-byte `BITMAPFILEHEADER` (`bfType = 0x4D42`) for `CF_DIB` screenshots; support `"PNG"` (`RegisterClipboardFormatW`), `CF_DIB`, and `CF_HDROP` via `.capture_action(|_: &Paste, ...| ...)` on `Textarea`.
- **Fast process shutdown**: In `optimize::close_processes_fast`, broadcast `WM_CLOSE` in a single `EnumWindows` pass, wait ~350ms in parallel, then terminate stubborn windowless child PIDs immediately instead of waiting sequentially per PID.
- **Cross-compile**: Verify on Linux with `x86_64-pc-windows-gnu` (`mingw-w64` windres); native Windows MSVC builds handle `embed-resource` natively.

### GPUI & Rust Patterns
- **Tuple arity**: Rust stdlib implements `PartialEq` only up to 12-element tuples; derive `PartialEq` on structs or compare fields individually.
- **DerefMut & borrow checker (E0502)**: Drain channel receivers into a local `Vec<Event>` before handling events, and reference nested fields explicitly (`self.workspace.session`) inside methods of the outer struct.
- **Auto-scroll & truncation**: Bind `track_scroll(&scroll_handle)` and gate `scroll_to_bottom()` on `(max_offset.y + offset.y) <= px(64.0)`. Apply `.overflow_hidden()` on `flex_1()` containers and `.w_full().truncate()` on text elements so long titles do not push action buttons off-screen.
- **Keyboard & animations**: Handle Cyrillic/layout variants (e.g. `k` / `л`) in `capture_key_down` for `Ctrl+K`. Use `.repeat().with_easing(...)` rather than `repeat_synced()` / `with_max_fps()` so animations tick via `request_animation_frame`.
- **Tool loop & streaming**: In `StreamEvent::Done`, use `tool_calls` from the stream event directly — never fall back to `state.streaming_tools` (which holds prior steps and causes infinite re-execution).

## Config and Environment

- Default target OS: Windows (`x86_64-pc-windows-msvc` release, `x86_64-pc-windows-gnu` cross-check on Linux).
- Runtime artifacts (`runtime/`, `models/`, `logs/`, `life/`, `personas/`, `snarta.toml`, `snarta.lock`, `target/`) are gitignored and local-only.

## Verification Log

- 2026-11-13: Prior baseline verified with `cargo test` (68/68) and `cargo check --target x86_64-pc-windows-gnu` (0 errors) before clean export (`verified-earlier`). Post-migration `git init` and `git push` to be verified this turn.
