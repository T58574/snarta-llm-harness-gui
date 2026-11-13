"""
Mock Benchmark Simulator for Snarta Benchmark TUI.

Provides simulated multi-model sweeps and live view animations for UI testing
without requiring a live llama-server or GPU environment.
Strictly zero emojis in codebase.
"""

import json
import random
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

BENCH_DIR = Path(__file__).resolve().parent
if str(BENCH_DIR) not in sys.path:
    sys.path.insert(0, str(BENCH_DIR))

from tui_ui import (
    Colors,
    CompletionState,
    LiveState,
    clear_screen,
    read_key,
    render_live_view,
)


def run_mock_benchmark(
    selected_models: List[str],
    mode: str,
    ncmoe_val: str,
    output_dir: Path,
    is_russian: bool = False,
    mtp_mode: str = "off",
    dataset_path: Optional[str] = None,
) -> CompletionState:
    """Executes a simulated benchmark with live view animation and real report generation."""
    output_dir.mkdir(parents=True, exist_ok=True)
    ts = datetime.now().strftime("%Y%m%d_%H%M%S")
    live = LiveState(mode_name=mode, ncmoe_label=ncmoe_val)

    mock_questions: List[tuple] = []
    if dataset_path:
        try:
            from mmlu_dataset import load_dataset
            loaded = load_dataset(dataset_path=dataset_path)
            if loaded:
                sample_qs = loaded[:8] if len(loaded) > 8 else loaded
                mock_questions = [(q.subject, q.category, q.question, q.answer) for q in sample_qs]
        except Exception:
            mock_questions = []

    if not mock_questions:
        # Mock questions (localized fallback)
        if is_russian:
            mock_questions = [
                ("higher_mathematics", "STEM", "Чему равен предел lim (x->0) [sin(3x)/(e^(2x)-1)]?", "A"),
                ("russian_history", "Humanities", "В каком году князь Владимир Святославич крестил Русь?", "B"),
                ("microeconomics", "Social Sciences", "Какое условие максимизации прибыли фирмы (MR=MC)?", "A"),
                ("russian_orthography", "Russian Culture", "В каком ряду во всех словах пишется удвоенная НН?", "A"),
                ("linux_kernel", "IT Edge Cases", "Что происходит при записи в страницу памяти Copy-on-Write?", "A"),
                ("differential_equations", "STEM", "Каков общий вид решения уравнения y'' + 4y = 0?", "B"),
                ("formal_logic", "Humanities", "Каким термином является понятие 'человек' в силлогизме?", "C"),
                ("networking", "IT Edge Cases", "Какую проблему HTTP/2 решает переход на QUIC в HTTP/3?", "A"),
            ]
        else:
            mock_questions = [
                ("college_computer_science", "STEM", "What is the time complexity of lookup in a balanced red-black tree?", "B"),
                ("formal_logic", "Humanities", "If premise (P -> Q) and ~Q hold, what follows via Modus Tollens?", "A"),
                ("econometrics", "Social Sciences", "Which condition violates Gauss-Markov assumptions for OLS efficiency?", "C"),
                ("it_edge_cases", "IT Edge Cases", "Under ROCm HIP, how is concurrent kernel dispatch scheduled without graph?", "D"),
                ("high_school_physics", "STEM", "What is the work done on an object moving along a closed conservative path?", "A"),
                ("philosophy", "Humanities", "Which philosopher authored Tractatus Logico-Philosophicus?", "B"),
                ("security_studies", "Social Sciences", "What is the primary distinction between asymmetric and symmetric ciphers?", "B"),
                ("college_biology", "STEM", "Which organelle is primarily responsible for ATP synthesis via phosphorylation?", "C"),
            ]

    total_steps = len(selected_models) * len(mock_questions)
    curr_step = 0
    model_summaries: List[Dict[str, Any]] = []

    start_time = time.time()
    for m_name in selected_models:
        opt_nc = 6 if ncmoe_val == "auto" else (int(ncmoe_val) if ncmoe_val.isdigit() else None)
        live.model_name = m_name
        live.ncmoe_label = f"{opt_nc} (auto)" if (ncmoe_val == "auto" and opt_nc is not None) else (str(opt_nc) if opt_nc is not None else "None (Dense)")
        live.active_flags = f"-ncmoe {opt_nc}" if opt_nc is not None else "Dense"
        if mtp_mode != "off":
            live.active_flags += " -md (mtp-draft)"
            live.recent_logs.append(f"[{datetime.now().strftime('%H:%M:%S')}] MTP draft attached (speculative mode)")
        live.recent_logs.append(f"[{datetime.now().strftime('%H:%M:%S')}] Launching {m_name} (flags: {live.active_flags})")
        model_correct = 0
        cat_counts: Dict[str, Dict[str, int]] = {}

        for sub, cat, q_text, expected in mock_questions:
            curr_step += 1
            live.current_step = curr_step
            live.total_steps = total_steps
            live.progress_pct = (curr_step / total_steps) * 100.0
            live.active_subject = sub
            live.active_category = cat
            live.active_question = q_text

            elapsed = max(0.1, time.time() - start_time)
            live.elapsed_sec = elapsed
            rate = curr_step / elapsed
            live.eta_sec = (total_steps - curr_step) / rate if rate > 0 else 0.0

            # Simulate realistic response
            is_correct = random.random() < 0.82
            if is_correct:
                model_correct += 1
                pred = expected
                live.last_status = f"PASS (Predicted: {pred} | Expected: {expected})"
            else:
                pred = random.choice([c for c in ["A", "B", "C", "D"] if c != expected])
                live.last_status = f"FAIL (Predicted: {pred} | Expected: {expected})"

            cat_counts.setdefault(cat, {"correct": 0, "total": 0})
            cat_counts[cat]["total"] += 1
            if is_correct:
                cat_counts[cat]["correct"] += 1

            live.correct_count = model_correct
            live.total_evaluated = cat_counts[cat]["total"] if len(cat_counts) == 1 else curr_step
            live.accuracy = (model_correct / max(1, curr_step % (len(mock_questions) + 1))) * 100.0
            live.current_tps = random.uniform(26.0, 38.5)
            live.mean_latency_ms = random.uniform(340.0, 480.0)

            live.recent_logs.append(
                f"[{datetime.now().strftime('%H:%M:%S')}] Step {curr_step}: {sub} -> {pred} ({live.current_tps:.1f} t/s)"
            )

            clear_screen()
            sys.stdout.write(render_live_view(live) + "\n")
            sys.stdout.flush()

            # Check for cancellation
            k = read_key(timeout_sec=0.1)
            if k in ("q", "Q", "ESC"):
                live.recent_logs.append("Execution aborted by user.")
                break

        cat_acc: Dict[str, float] = {}
        for c, counts in cat_counts.items():
            cat_acc[c] = (counts["correct"] / counts["total"]) * 100.0 if counts["total"] > 0 else 0.0

        model_summaries.append({
            "model_name": m_name,
            "ncmoe": 6 if ncmoe_val == "auto" else (int(ncmoe_val) if ncmoe_val.isdigit() else None),
            "load_time_sec": 5.4,
            "total_questions": len(mock_questions),
            "correct_count": model_correct,
            "overall_accuracy": (model_correct / len(mock_questions)) * 100.0,
            "category_accuracy": cat_acc,
            "mean_tps": random.uniform(38.0, 48.0) if mtp_mode != "off" else random.uniform(28.0, 35.0),
            "mean_latency_ms": 280.0 if mtp_mode != "off" else 395.0,
        })

    # Save mock artifacts
    report_tag = "mock_suite"
    base_fn = f"mmlu_{report_tag}_{ts}"
    json_path = output_dir / f"{base_fn}.json"
    md_path = output_dir / f"{base_fn}.md"
    dash_path = output_dir / f"{base_fn}_dashboard.html"

    report_payload = {
        "timestamp": datetime.now().isoformat(),
        "questions_evaluated": total_steps,
        "models": model_summaries,
    }
    json_path.write_text(json.dumps(report_payload, indent=2), encoding="utf-8")

    # Generate Markdown
    md_lines = [
        "# Snarta Benchmark Report",
        "",
        f"- Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}",
        f"- Mode: {mode}",
        "",
        "| Model | ncmoe | Accuracy | Score | Speed | Status |",
        "|---|---|---|---|---|---|",
    ]
    best_acc = max((m["overall_accuracy"] for m in model_summaries), default=0.0)
    for m in model_summaries:
        status = "WINNER" if m["overall_accuracy"] == best_acc else "OK"
        md_lines.append(
            f"| `{m['model_name']}` | `{m['ncmoe']}` | **{m['overall_accuracy']:.1f}%** | "
            f"{m['correct_count']}/{m['total_questions']} | {m['mean_tps']:.1f} t/s | {status} |"
        )
    md_path.write_text("\n".join(md_lines), encoding="utf-8")

    # Generate HTML Dashboard
    try:
        from mmlu_plot import generate_html_dashboard
        generate_html_dashboard(report_payload, dash_path)
    except Exception:
        dash_path = None

    # Completion state
    headers = ["Model", "ncmoe", "Accuracy", "Score", "Speed", "Status"]
    rows = []
    for m in model_summaries:
        is_win = (m["overall_accuracy"] == best_acc)
        badge = f"{Colors.BRIGHT_GREEN}WINNER{Colors.RESET}" if is_win else "OK"
        rows.append([
            m["model_name"][:28],
            str(m["ncmoe"]),
            f"{m['overall_accuracy']:.1f}%",
            f"{m['correct_count']}/{m['total_questions']}",
            f"{m['mean_tps']:.1f} t/s",
            badge,
        ])

    return CompletionState(
        mode_name=mode,
        table_headers=headers,
        table_rows=rows,
        category_breakdown=model_summaries[0]["category_accuracy"] if model_summaries else {},
        md_path=md_path,
        json_path=json_path,
        dashboard_path=dash_path,
    )
