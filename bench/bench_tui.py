#!/usr/bin/env python3
"""
Snarta Benchmark Terminal User Interface (TUI) Runner

Interactive terminal UI for executing MMLU intelligence and MoE concurrency sweeps.
Runs on Windows 11 (PowerShell/Windows Terminal) and Linux with zero external dependencies.
Strictly English code and comments. ZERO emojis across all strings and code.
"""

import argparse
import glob
import json
import os
import platform
import random
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import webbrowser
from dataclasses import asdict
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Tuple

# Ensure bench folder is importable
BENCH_DIR = Path(__file__).resolve().parent
REPO_ROOT = BENCH_DIR.parent
if str(BENCH_DIR) not in sys.path:
    sys.path.insert(0, str(BENCH_DIR))

from mock_runner import run_mock_benchmark
from mmlu_dataset import (
    DatasetInfo,
    scan_available_datasets,
)
from tui_ui import (
    Colors,
    CompletionState,
    HARDCORE_CATEGORIES,
    LiveState,
    MMLU_EN_CATEGORIES,
    MenuState,
    RUMMLU_CATEGORIES,
    TerminalSession,
    clear_screen,
    flush_input,
    read_key,
    render_completion_view,
    render_live_view,
    render_menu,
)


def scan_available_models(models_dir: Optional[str] = None) -> List[Path]:
    """Scans models in target/release/models and models excluding mmproj/clip."""
    candidates: List[Path] = []
    if models_dir and Path(models_dir).is_dir():
        candidates.extend(Path(models_dir).glob("**/*.gguf"))
    else:
        search_dirs = [
            REPO_ROOT / "target" / "release" / "models",
            REPO_ROOT / "models",
            Path.cwd() / "target" / "release" / "models",
            Path.cwd() / "models",
        ]
        for d in search_dirs:
            if d.is_dir():
                candidates.extend(d.glob("**/*.gguf"))

    seen = set()
    filtered: List[Path] = []
    for c in candidates:
        r = c.resolve()
        name_lower = r.name.lower()
        if any(x in name_lower for x in ("mmproj", "clip", "projector", "mtp")):
            continue
        if r not in seen and r.is_file():
            seen.add(r)
            filtered.append(r)
    return sorted(filtered, key=lambda x: x.name)


def locate_server_binary(server_arg: Optional[str] = None) -> Optional[Path]:
    """Locates llama-server executable."""
    if server_arg and Path(server_arg).exists():
        return Path(server_arg).resolve()

    patterns = [
        "target/release/runtime/**/llama-server.exe",
        "target/release/runtime/*/llama-server.exe",
        "runtime/**/llama-server.exe",
        "runtime/*/llama-server.exe",
        "runtime/llama-server.exe",
        "target/release/runtime/**/llama-server",
        "target/release/runtime/*/llama-server",
        "runtime/**/llama-server",
        "runtime/*/llama-server",
        "runtime/llama-server",
        "llama-server.exe",
        "llama-server",
    ]
    for pat in patterns:
        for root in [REPO_ROOT, Path.cwd()]:
            matches = glob.glob(str(root / pat), recursive=True)
            if matches and Path(matches[0]).exists():
                return Path(matches[0]).resolve()
    return None


def sync_menu_categories(state: MenuState) -> None:
    """Updates categories list when dataset changes."""
    if state.datasets and state.dataset_idx < len(state.datasets):
        ds = state.datasets[state.dataset_idx]
        state.all_categories = list(ds.categories)
    else:
        active_ds = state.dataset_options[state.dataset_idx].lower() if state.dataset_options else ""
        if "hardcore" in active_ds or "stress" in active_ds:
            state.all_categories = list(HARDCORE_CATEGORIES)
        elif "russian" in active_ds or "rummlu" in active_ds:
            state.all_categories = list(RUMMLU_CATEGORIES)
        else:
            state.all_categories = list(MMLU_EN_CATEGORIES)
    state.selected_categories = set(state.all_categories)

    # Ensure focused item is valid if it pointed to a removed category
    items = get_focusable_items(state)
    if state.focused_item not in items:
        state.focused_item = "dataset"


def update_ncmoe_preview(state: MenuState) -> None:
    """Updates ncmoe preview text for the first selected model."""
    if not state.selected_model_indices or not state.models:
        state.detected_ncmoe_preview = None
        return
    try:
        from bench_mmlu import detect_optimal_ncmoe
    except ImportError:
        state.detected_ncmoe_preview = None
        return
    first_idx = sorted(state.selected_model_indices)[0]
    if first_idx < len(state.models):
        m_name = state.models[first_idx]
        detected = detect_optimal_ncmoe(m_name)
        if detected is not None:
            state.detected_ncmoe_preview = f"MoE: {detected}"
        else:
            state.detected_ncmoe_preview = "Dense"


def get_focusable_items(state: MenuState) -> List[str]:
    """Returns the ordered list of focusable UI elements in the menu."""
    items: List[str] = []
    for idx in range(len(state.models)):
        items.append(f"model:{idx}")
    items.append("dataset")
    items.append("mode")
    if state.mode_index != 1:  # Not MoE Sweep
        for cat in state.all_categories:
            items.append(f"cat:{cat}")
    items.extend(["setting:ncmoe", "setting:mtp", "setting:reasoning", "setting:max_tokens", "setting:ctx"])
    items.extend(["action:start", "action:exit"])
    return items


def toggle_category(cat: str, state: MenuState) -> None:
    """Toggles a category selection adhering to All tree semantics."""
    subs = [c for c in state.all_categories if c != "All"]
    if cat == "All":
        if "All" in state.selected_categories:
            state.selected_categories.clear()
        else:
            state.selected_categories.update(state.all_categories)
    else:
        if cat in state.selected_categories:
            state.selected_categories.discard(cat)
            state.selected_categories.discard("All")
        else:
            state.selected_categories.add(cat)
            if all(c in state.selected_categories for c in subs):
                state.selected_categories.add("All")


def handle_menu_input(key: str, state: MenuState) -> Optional[str]:
    """Handles keyboard interaction on the menu screen. Returns action if triggered."""
    items = get_focusable_items(state)
    if state.focused_item not in items:
        state.focused_item = items[0] if items else "action:start"
    curr_idx = items.index(state.focused_item)

    if key == "DOWN":
        next_idx = (curr_idx + 1) % len(items)
        state.focused_item = items[next_idx]
        if state.focused_item.startswith("model:"):
            m_idx = int(state.focused_item.split(":")[1])
            if m_idx >= state.models_scroll_offset + state.max_visible_models:
                state.models_scroll_offset = m_idx - state.max_visible_models + 1
    elif key == "UP":
        prev_idx = (curr_idx - 1) % len(items)
        state.focused_item = items[prev_idx]
        if state.focused_item.startswith("model:"):
            m_idx = int(state.focused_item.split(":")[1])
            if m_idx < state.models_scroll_offset:
                state.models_scroll_offset = m_idx

    elif key in ("LEFT", "RIGHT"):
        delta = 1 if key == "RIGHT" else -1
        if state.focused_item == "dataset":
            state.dataset_idx = (state.dataset_idx + delta) % len(state.dataset_options)
            sync_menu_categories(state)
        elif state.focused_item == "mode":
            state.mode_index = (state.mode_index + delta) % len(state.modes)
        elif state.focused_item == "setting:ncmoe":
            state.ncmoe_idx = (state.ncmoe_idx + delta) % len(state.ncmoe_options)
        elif state.focused_item == "setting:mtp":
            state.mtp_idx = (state.mtp_idx + delta) % len(state.mtp_options)
        elif state.focused_item == "setting:reasoning":
            state.reasoning_idx = (state.reasoning_idx + delta) % len(state.reasoning_options)
        elif state.focused_item == "setting:max_tokens":
            state.max_tokens_idx = (state.max_tokens_idx + delta) % len(state.max_tokens_options)
        elif state.focused_item == "setting:ctx":
            state.ctx_idx = (state.ctx_idx + delta) % len(state.ctx_options)
        elif state.focused_item in ("action:start", "action:exit"):
            state.focused_item = "action:exit" if state.focused_item == "action:start" else "action:start"

    elif key in ("SPACE", "ENTER"):
        if state.focused_item.startswith("model:"):
            m_idx = int(state.focused_item.split(":")[1])
            if m_idx in state.selected_model_indices:
                state.selected_model_indices.discard(m_idx)
            else:
                state.selected_model_indices.add(m_idx)
        elif state.focused_item == "dataset":
            state.dataset_idx = (state.dataset_idx + 1) % len(state.dataset_options)
            sync_menu_categories(state)
        elif state.focused_item == "mode":
            state.mode_index = (state.mode_index + 1) % len(state.modes)
        elif state.focused_item.startswith("cat:"):
            c_name = state.focused_item.split(":", 1)[1]
            toggle_category(c_name, state)
        elif state.focused_item == "setting:ncmoe":
            state.ncmoe_idx = (state.ncmoe_idx + 1) % len(state.ncmoe_options)
        elif state.focused_item == "setting:mtp":
            state.mtp_idx = (state.mtp_idx + 1) % len(state.mtp_options)
        elif state.focused_item == "setting:reasoning":
            state.reasoning_idx = (state.reasoning_idx + 1) % len(state.reasoning_options)
        elif state.focused_item == "setting:max_tokens":
            state.max_tokens_idx = (state.max_tokens_idx + 1) % len(state.max_tokens_options)
        elif state.focused_item == "setting:ctx":
            state.ctx_idx = (state.ctx_idx + 1) % len(state.ctx_options)
        elif state.focused_item == "action:start":
            return "START"
        elif state.focused_item == "action:exit":
            return "EXIT"

    elif key in ("a", "A"):
        if len(state.selected_model_indices) == len(state.models):
            state.selected_model_indices.clear()
        else:
            state.selected_model_indices = set(range(len(state.models)))

    elif key in ("q", "Q", "ESC"):
        return "EXIT"

    return None


def run_benchmark_controller(
    state: MenuState,
    server_path: Optional[Path],
    output_dir: Path,
    mock_mode: bool = False,
    model_paths: Optional[Dict[str, Path]] = None,
) -> CompletionState:
    """Dispatches either real server execution or mock simulation."""
    selected_names = [state.models[i] for i in sorted(state.selected_model_indices)]
    if not selected_names and state.models:
        selected_names = [state.models[0]]

    ncmoe_val = state.ncmoe_options[state.ncmoe_idx]
    mtp_val = state.mtp_options[state.mtp_idx]
    active_mode = state.modes[state.mode_index]

    if state.datasets and state.dataset_idx < len(state.datasets):
        cur_ds = state.datasets[state.dataset_idx]
        dataset_path = str(cur_ds.path)
        is_hardcore = cur_ds.is_hardcore
        is_ru = cur_ds.is_russian
    else:
        active_ds = state.dataset_options[state.dataset_idx].lower() if state.dataset_options else ""
        is_hardcore = "hardcore" in active_ds or "stress" in active_ds
        is_ru = "russian" in active_ds or "rummlu" in active_ds
        if is_hardcore:
            dataset_path = str(BENCH_DIR / "data" / "hardcore_stress_sample.json")
        elif is_ru:
            dataset_path = str(BENCH_DIR / "data" / "rummlu_sample.json")
        else:
            dataset_path = str(BENCH_DIR / "data" / "mmlu_sample.json")

    if mock_mode or not server_path or any("mock" in m.lower() for m in selected_names):
        return run_mock_benchmark(
            selected_names,
            active_mode,
            ncmoe_val,
            output_dir,
            is_russian=(is_ru or is_hardcore),
            mtp_mode=mtp_val,
            dataset_path=dataset_path,
        )

    # Real evaluation
    try:
        from bench_mmlu import (
            ModelMmluResult,
            ServerSession,
            detect_optimal_ncmoe,
            load_dataset,
            save_benchmark_artifacts,
            send_chat_completion,
        )
        from mmlu_dataset import (
            SingleQuestionEval,
            extract_predicted_choice,
            find_matching_mtp_layer,
            format_mmlu_prompt,
        )
    except ImportError:
        return run_mock_benchmark(
            selected_names,
            active_mode,
            ncmoe_val,
            output_dir,
            is_russian=(is_ru or is_hardcore),
            mtp_mode=mtp_val,
            dataset_path=dataset_path,
        )

    output_dir.mkdir(parents=True, exist_ok=True)

    # Categories filter
    if "All" in state.selected_categories:
        cats_filter = None
    else:
        cats_filter = list(state.selected_categories)
        if "Russian Culture" in cats_filter:
            cats_filter.append("Russian Language & Culture")
        # Hardcore category mappings
        hard_map = {
            "Math & Algo": "Multi-Step Math & Algorithms",
            "Cognitive Traps": "Cognitive Traps & Paradoxes",
            "Low-Level Systems": "Low-Level Systems & Concurrency",
            "Formal Logic": "Formal Logic & Counterfactuals",
            "Adversarial Syntax": "Adversarial Linguistics & Russian Syntax",
        }
        for short_name, full_name in hard_map.items():
            if short_name in cats_filter and full_name not in cats_filter:
                cats_filter.append(full_name)

    questions = load_dataset(
        dataset_path=dataset_path,
        categories_filter=cats_filter,
    )
    if not questions:
        questions = load_dataset(dataset_path=dataset_path)

    flush_input()
    live = LiveState(mode_name=active_mode, ncmoe_label=ncmoe_val)
    results: List[ModelMmluResult] = []
    total_evals = len(selected_names) * len(questions)
    curr_eval = 0
    t0 = time.time()
    aborted = False

    max_tokens = state.max_tokens_options[state.max_tokens_idx]
    ctx_val = state.ctx_options[state.ctx_idx]
    reasoning_mode = state.reasoning_options[state.reasoning_idx]
    reasoning_effort = "none" if reasoning_mode == "off" else "medium"

    for m_idx, m_name in enumerate(selected_names):
        if aborted:
            break

        m_path = None
        if model_paths and m_name in model_paths:
            cand = model_paths[m_name]
            if cand.exists():
                m_path = cand

        if not m_path:
            for cand in [
                REPO_ROOT / "models" / m_name,
                REPO_ROOT / "target" / "release" / "models" / m_name,
                Path.cwd() / "models" / m_name,
                Path.cwd() / "target" / "release" / "models" / m_name,
            ]:
                if cand.exists():
                    m_path = cand
                    break

        if not m_path:
            for root in [REPO_ROOT / "models", Path.cwd() / "models"]:
                if root.is_dir():
                    matches = list(root.glob(f"**/{m_name}"))
                    if matches and matches[0].exists():
                        m_path = matches[0]
                        break

        if not m_path or not m_path.exists():
            live.recent_logs.append(f"Model file not found: {m_name}")
            continue

        if ncmoe_val == "auto":
            opt_ncmoe = detect_optimal_ncmoe(m_name)
        elif ncmoe_val.isdigit():
            opt_ncmoe = int(ncmoe_val)
        else:
            opt_ncmoe = None

        if opt_ncmoe is not None:
            ncmoe_label = f"{opt_ncmoe} (auto)" if ncmoe_val == "auto" else str(opt_ncmoe)
            live.active_flags = f"-ncmoe {opt_ncmoe}"
        else:
            ncmoe_label = "None (Dense)"
            live.active_flags = "Dense (no -ncmoe)"

        live.model_name = m_name
        live.ncmoe_label = ncmoe_label
        live.recent_logs.append(f"Starting server: {m_name}")
        live.recent_logs.append(f"Applied args: {live.active_flags}")
        clear_screen()
        sys.stdout.write(render_live_view(live) + "\n")
        sys.stdout.flush()

        cmd = [
            str(server_path),
            "-m", str(m_path),
            "--host", "127.0.0.1",
            "--port", "8089",
            "--jinja",
            "--reasoning-format", "auto",
            "--reasoning", reasoning_mode,
            "--reasoning-effort", reasoning_effort,
            "-c", str(ctx_val),
            "-ngl", "99",
            "-b", "2048",
            "-ub", "512",
            "-fa", "on",
            "-np", "1",
            "--slots",
            "-ctk", "q8_0",
            "-ctv", "q8_0",
        ]
        if opt_ncmoe is not None:
            cmd.extend(["-ncmoe", str(opt_ncmoe)])

        if mtp_val != "off":
            mtp_file = find_matching_mtp_layer(m_path)
            if mtp_file:
                cmd.extend(["-md", str(mtp_file), "--spec-type", "draft-mtp"])
                live.active_flags += f" -md {mtp_file.name}"
                live.recent_logs.append(f"MTP draft attached: {mtp_file.name}")
            else:
                live.recent_logs.append(f"Notice: MTP enabled but no mtp-*.gguf found near {m_name}")

        session = ServerSession(cmd, "127.0.0.1", 8089)
        try:
            load_time = session.start()
            live.recent_logs.append(f"Server ready in {load_time:.2f}s (active -ncmoe: {opt_ncmoe or 'none'})")
        except Exception as e:
            live.recent_logs.append(f"ERROR: {str(e)[:45]}")
            session.stop()
            time.sleep(2.0)
            continue

        flush_input()
        res = ModelMmluResult(
            model_name=m_name,
            model_path=str(m_path),
            ncmoe=opt_ncmoe,
            load_time_sec=load_time,
            total_questions=len(questions),
        )

        total_model_latency_ms = 0.0
        total_model_tokens = 0
        base_url = "http://127.0.0.1:8089"

        # Reset per-model live counters
        live.total_evaluated = 0
        live.correct_count = 0
        live.accuracy = 0.0
        live.current_tps = 0.0
        live.mean_latency_ms = 0.0

        try:
            for q in questions:
                curr_eval += 1
                live.current_step = curr_eval
                live.total_steps = total_evals
                live.progress_pct = (curr_eval / total_evals) * 100.0 if total_evals > 0 else 0.0
                live.active_subject = q.subject
                live.active_category = q.category
                live.active_question = q.question
                live.elapsed_sec = time.time() - t0

                # Calculate ETA
                if curr_eval > 1 and live.elapsed_sec > 0:
                    sec_per_item = live.elapsed_sec / curr_eval
                    rem_items = max(0, total_evals - curr_eval)
                    live.eta_sec = sec_per_item * rem_items

                clear_screen()
                sys.stdout.write(render_live_view(live) + "\n")
                sys.stdout.flush()

                prompt = format_mmlu_prompt(q)

                # Real chat completion HTTP evaluation
                resp_text, latency_ms, p_tok, c_tok, err = send_chat_completion(
                    base_url=base_url,
                    messages=[{"role": "user", "content": prompt}],
                    max_tokens=max_tokens,
                    temperature=0.0,
                )

                pred_choice = extract_predicted_choice(resp_text) if not err else None
                is_correct = (pred_choice == q.answer)
                if is_correct:
                    res.correct_count += 1

                total_model_latency_ms += latency_ms
                total_model_tokens += c_tok

                eval_entry = SingleQuestionEval(
                    question_id=q.id,
                    subject=q.subject,
                    category=q.category,
                    predicted=pred_choice,
                    expected=q.answer,
                    is_correct=is_correct,
                    latency_ms=latency_ms,
                    eval_tokens=c_tok,
                    prompt_tokens=p_tok,
                    response_text=resp_text.strip() if not err else f"ERROR: {err}",
                )
                res.evaluations.append(eval_entry)

                cat = q.category
                if cat not in res.category_counts:
                    res.category_counts[cat] = {"correct": 0, "total": 0}
                res.category_counts[cat]["total"] += 1
                if is_correct:
                    res.category_counts[cat]["correct"] += 1

                # Update live view stats
                eval_count = len(res.evaluations)
                live.total_evaluated = eval_count
                live.correct_count = res.correct_count
                live.accuracy = (res.correct_count / eval_count) * 100.0 if eval_count > 0 else 0.0
                live.mean_latency_ms = total_model_latency_ms / eval_count if eval_count > 0 else 0.0
                gen_sec = total_model_latency_ms / 1000.0
                live.current_tps = (total_model_tokens / gen_sec) if gen_sec > 0 else 0.0

                if err:
                    live.last_status = f"ERROR: {err[:32]}"
                elif is_correct:
                    live.last_status = f"PASS (exp {q.answer}, got {pred_choice})"
                else:
                    live.last_status = f"FAIL (exp {q.answer}, got {pred_choice or '?'})"

                log_entry = f"[{eval_count}/{len(questions)}] {q.subject[:14]}: {live.last_status} ({c_tok} tok, {latency_ms:.0f}ms)"
                live.recent_logs.append(log_entry)
                if len(live.recent_logs) > 6:
                    live.recent_logs = live.recent_logs[-6:]

                clear_screen()
                sys.stdout.write(render_live_view(live) + "\n")
                sys.stdout.flush()

                # Abort key check
                k = read_key(timeout_sec=0.0)
                if k in ("q", "Q", "ESC"):
                    live.recent_logs.append("User requested abort, stopping...")
                    aborted = True
                    break
        finally:
            session.stop()

        res.total_eval_time_sec = total_model_latency_ms / 1000.0
        res.overall_accuracy = (res.correct_count / res.total_questions) * 100.0 if res.total_questions > 0 else 0.0
        res.mean_latency_ms = (total_model_latency_ms / len(questions)) if questions else 0.0
        res.mean_tps = (total_model_tokens / res.total_eval_time_sec) if res.total_eval_time_sec > 0 else 0.0

        for cat, counts in res.category_counts.items():
            tot = counts["total"]
            cor = counts["correct"]
            res.category_accuracy[cat] = (cor / tot) * 100.0 if tot > 0 else 0.0

        results.append(res)
        save_benchmark_artifacts(results, len(questions), output_dir)

        if aborted:
            break

        # Cooldown between models
        if m_idx + 1 < len(selected_names):
            live.recent_logs.append("Cooling down VRAM (5.0s)...")
            clear_screen()
            sys.stdout.write(render_live_view(live) + "\n")
            sys.stdout.flush()
            time.sleep(5.0)

    md_f, json_f, dash_f = save_benchmark_artifacts(results, len(questions), output_dir)
    headers = ["Model", "ncmoe", "Accuracy", "Score", "Speed", "Status"]
    best_acc = max((r.overall_accuracy for r in results), default=0.0)
    rows = []
    for r in results:
        is_win = (r.overall_accuracy == best_acc and r.overall_accuracy > 0)
        badge = f"{Colors.BRIGHT_GREEN}WINNER{Colors.RESET}" if is_win else "OK"
        nc_str = str(r.ncmoe) if r.ncmoe is not None else "None"
        rows.append([
            r.model_name[:28],
            nc_str,
            f"{r.overall_accuracy:.1f}%",
            f"{r.correct_count}/{r.total_questions}",
            f"{r.mean_tps:.1f} t/s",
            badge,
        ])

    return CompletionState(
        mode_name=active_mode,
        table_headers=headers,
        table_rows=rows,
        category_breakdown=results[0].category_accuracy if results else {},
        md_path=md_f,
        json_path=json_f,
        dashboard_path=dash_f,
        notice_message="Benchmark completed successfully." if results else "No models were evaluated.",
    )


def interactive_main(
    mock_flag: bool = False,
    models_dir: Optional[str] = None,
    server_arg: Optional[str] = None,
    dataset_arg: Optional[str] = None,
) -> None:
    """Main application loop managing menu, execution, and completion screens."""
    available_models = scan_available_models(models_dir)
    model_names = [m.name for m in available_models]
    model_paths = {m.name: m for m in available_models}

    # Provide demo model if none found
    if not model_names:
        model_names = [
            "mock-gemma-4-26B-A4B-Q4_K_M.gguf",
            "mock-gemma-4-26B-A4B-Q8_0.gguf",
        ]

    available_datasets = scan_available_datasets()
    dataset_options = [d.name for d in available_datasets]
    initial_categories = list(available_datasets[0].categories) if available_datasets else list(HARDCORE_CATEGORIES)

    state = MenuState(
        models=model_names,
        selected_model_indices={0},
        focused_item="model:0",
        datasets=available_datasets,
        dataset_options=dataset_options,
        all_categories=initial_categories,
        selected_categories=set(initial_categories),
    )

    if dataset_arg and available_datasets:
        arg_low = dataset_arg.lower().strip()
        for idx, ds in enumerate(available_datasets):
            if arg_low in ds.id.lower() or arg_low in ds.name.lower() or arg_low in ds.path.name.lower():
                state.dataset_idx = idx
                sync_menu_categories(state)
                break
    server_path = locate_server_binary(server_arg)
    output_dir = REPO_ROOT / "bench" / "reports"
    output_dir.mkdir(parents=True, exist_ok=True)

    with TerminalSession():
        app_screen = "MENU"
        completion_data: Optional[CompletionState] = None

        while True:
            if app_screen == "MENU":
                update_ncmoe_preview(state)
                clear_screen()
                sys.stdout.write(render_menu(state) + "\n")
                sys.stdout.flush()

                k = read_key()
                if not k:
                    continue
                action = handle_menu_input(k, state)
                if action == "EXIT":
                    break
                elif action == "START":
                    if not state.selected_model_indices:
                        state.status_message = "Please select at least one model [x]"
                        continue
                    if state.mode_index != 1 and not state.selected_categories:
                        state.status_message = "Please select at least one category [x]"
                        continue
                    state.status_message = None
                    app_screen = "RUNNING"

            elif app_screen == "RUNNING":
                completion_data = run_benchmark_controller(
                    state,
                    server_path,
                    output_dir,
                    mock_mode=mock_flag,
                    model_paths=model_paths,
                )
                app_screen = "COMPLETED"

            elif app_screen == "COMPLETED":
                if not completion_data:
                    state.status_message = "Benchmark finished with no results."
                    app_screen = "MENU"
                    continue

                clear_screen()
                sys.stdout.write(render_completion_view(completion_data) + "\n")
                sys.stdout.flush()

                k = read_key()
                if k in ("o", "O"):
                    if completion_data.dashboard_path and completion_data.dashboard_path.exists():
                        webbrowser.open(completion_data.dashboard_path.resolve().as_uri())
                        completion_data.notice_message = f"Opened in browser: {completion_data.dashboard_path.name}"
                elif k in ("r", "R"):
                    app_screen = "MENU"
                elif k in ("q", "Q", "ESC"):
                    break

    clear_screen()
    print("Snarta Benchmark Runner closed.")


def build_arg_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Snarta Interactive Benchmark TUI Runner")
    p.add_argument("--mock", action="store_true", help="Run with simulated mock benchmarks")
    p.add_argument("--dry-run", action="store_true", help="Alias for --mock")
    p.add_argument("--models-dir", help="Custom directory containing GGUF models")
    p.add_argument("-s", "--server", help="Path to llama-server executable")
    p.add_argument("--dataset", help="Initial benchmark dataset name or path to select in TUI")
    return p


if __name__ == "__main__":
    parser = build_arg_parser()
    args = parser.parse_args()
    interactive_main(
        mock_flag=(args.mock or args.dry_run),
        models_dir=args.models_dir,
        server_arg=args.server,
        dataset_arg=args.dataset,
    )
