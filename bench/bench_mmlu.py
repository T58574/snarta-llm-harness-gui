#!/usr/bin/env python3
"""
Snarta MMLU Benchmark Suite

Measures academic reasoning accuracy (Massive Multitask Language Understanding)
across quantized LLM models (Q4, Q5, Q6, Q8) to determine intelligence degradation,
category-specific strengths, and generation speed Pareto trade-offs.

Supports:
- Direct evaluation against an existing llama-server or automated multi-model sweeps
- Automatic optimal -ncmoe selection per quantization tier
- Curated offline dataset (bench/data/mmlu_sample.json) or official MMLU data.tar
- Multi-domain breakdown: STEM, Humanities, Social Sciences, Other
- Interactive standalone HTML/SVG dashboards and Markdown/JSON reports
- Zero mandatory external dependencies (pure Python standard library)
"""

import argparse
import glob
import json
import os
import platform
import re
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import asdict, dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

BENCH_DIR = Path(__file__).resolve().parent
if str(BENCH_DIR) not in sys.path:
    sys.path.insert(0, str(BENCH_DIR))

from mmlu_dataset import (
    MmluQuestion,
    SingleQuestionEval,
    download_official_mmlu,
    extract_predicted_choice,
    format_mmlu_prompt,
    is_cyrillic,
    load_dataset,
)

# Optimal -ncmoe concurrency empirically discovered in Snarta sweeps
NCMOE_OPTIMAL_MAP: Dict[str, int] = {
    "q3": 6,
    "q4": 6,
    "q5": 10,
    "q6": 14,
    "q8": 17,
}


@dataclass
class ModelMmluResult:
    model_name: str
    model_path: Optional[str]
    ncmoe: Optional[int]
    load_time_sec: float
    total_questions: int = 0
    correct_count: int = 0
    overall_accuracy: float = 0.0
    category_accuracy: Dict[str, float] = field(default_factory=dict)
    category_counts: Dict[str, Dict[str, int]] = field(default_factory=dict)
    subject_accuracy: Dict[str, float] = field(default_factory=dict)
    mean_latency_ms: float = 0.0
    mean_tps: float = 0.0
    total_eval_time_sec: float = 0.0
    evaluations: List[SingleQuestionEval] = field(default_factory=list)


def is_moe_model(model_str: str) -> bool:
    """Detects if model architecture is Mixture of Experts (MoE)."""
    lower = model_str.lower()
    moe_patterns = [
        r"\bmoe\b",
        r"[-_.]moe[-_.]",
        r"a\d+b",           # e.g. A4B (active 4 experts in Gemma), A2B, A16B
        r"\d+x\d+b",        # e.g. 8x7b, 8x22b, 4x7b (Mixtral style)
        r"mixtral",
        r"dbrx",
        r"deepseek.*v[23]",
        r"qwen.*moe",
        r"gemma[-_]?4",
    ]
    for pat in moe_patterns:
        if re.search(pat, lower):
            return True
    return False


def detect_optimal_ncmoe(model_str: str) -> Optional[int]:
    """Detects the optimal -ncmoe value from model name/path. Returns None for dense models."""
    if not is_moe_model(model_str):
        return None

    lower = model_str.lower()
    for q_tier, opt_val in NCMOE_OPTIMAL_MAP.items():
        if f"-{q_tier}" in lower or f"_{q_tier}" in lower or f".{q_tier}" in lower:
            return opt_val
    return None


def is_port_open(host: str, port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(0.5)
        return s.connect_ex((host, port)) == 0


def wait_for_port_release(host: str, port: int, timeout_sec: float = 8.0) -> bool:
    start = time.time()
    while time.time() - start < timeout_sec:
        if not is_port_open(host, port):
            return True
        time.sleep(0.3)
    return False


def kill_process_tree(proc: subprocess.Popen, timeout_sec: float = 5.0) -> None:
    if proc.poll() is not None:
        return
    try:
        proc.terminate()
        proc.wait(timeout=timeout_sec)
    except subprocess.TimeoutExpired:
        if platform.system() == "Windows":
            subprocess.run(
                ["taskkill", "/F", "/T", "/PID", str(proc.pid)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
        else:
            proc.kill()
        try:
            proc.wait(timeout=2.0)
        except Exception:
            pass


def find_llama_server() -> Optional[Path]:
    patterns = [
        "target/release/runtime/*/llama-server.exe", "runtime/*/llama-server.exe",
        "runtime/llama-server.exe", "target/release/runtime/*/llama-server",
        "runtime/*/llama-server", "runtime/llama-server", "llama-server.exe", "llama-server",
    ]
    for p in patterns:
        m = glob.glob(p, recursive=True)
        if m:
            return Path(m[0]).resolve()
    return None


def find_models(arg_models: Optional[str] = None, models_dir: Optional[str] = None) -> List[Path]:
    """Finds matching GGUF model files."""
    found: List[Path] = []
    if arg_models:
        for item in arg_models.split(","):
            clean = item.strip()
            if not clean:
                continue
            matched = glob.glob(clean, recursive=True)
            if matched:
                found.extend([Path(m).resolve() for m in matched])
            elif Path(clean).exists():
                found.append(Path(clean).resolve())
    elif models_dir:
        p = Path(models_dir)
        if p.exists() and p.is_dir():
            found.extend([f.resolve() for f in p.glob("**/*.gguf")])
    else:
        default_patterns = ["models/*.gguf", "target/release/models/*.gguf"]
        for pat in default_patterns:
            matches = glob.glob(pat, recursive=True)
            if matches:
                found.extend([Path(m).resolve() for m in matches])
                break

    seen = set()
    uniq: List[Path] = []
    for f in found:
        name_lower = f.name.lower()
        # Exclude multimodal projectors (CLIP/mmproj) as they cannot run as main LLM
        if "mmproj" in name_lower or "clip" in name_lower or "projector" in name_lower:
            continue
        if f not in seen:
            seen.add(f)
            uniq.append(f)
    return uniq


class ServerSession:
    def __init__(self, cmd: List[str], host: str, port: int, verbose: bool = False):
        self.cmd = cmd
        self.host = host
        self.port = port
        self.verbose = verbose
        self.proc: Optional[subprocess.Popen] = None
        self.logs: List[str] = []
        self._reader_thread: Optional[threading.Thread] = None
        self._stop_event = threading.Event()

    def start(self, startup_timeout: float = 180.0) -> float:
        t_start = time.time()
        cflags = 0x08000000 if platform.system() == "Windows" else 0
        self.proc = subprocess.Popen(
            self.cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, bufsize=1, encoding="utf-8", errors="replace", creationflags=cflags,
        )

        def _log_reader():
            assert self.proc and self.proc.stdout
            for line in iter(self.proc.stdout.readline, ""):
                if self._stop_event.is_set():
                    break
                stripped = line.rstrip()
                self.logs.append(stripped)
                if self.verbose:
                    print(f"  [llama-server] {stripped}", flush=True)

        self._reader_thread = threading.Thread(target=_log_reader, daemon=True)
        self._reader_thread.start()

        health_url = f"http://{self.host}:{self.port}/health"
        server_ready = False
        while time.time() - t_start < startup_timeout:
            if self.proc.poll() is not None:
                err_excerpt = "\n".join(self.logs[-15:])
                raise RuntimeError(f"llama-server exited with code {self.proc.returncode}:\n{err_excerpt}")
            try:
                req = urllib.request.Request(health_url, headers={"User-Agent": "Snarta-MMLU/1.0"})
                with urllib.request.urlopen(req, timeout=1.0) as resp:
                    if resp.status == 200:
                        data = json.loads(resp.read().decode("utf-8", errors="replace"))
                        if data.get("status") in ("ok", "ready", "loaded"):
                            server_ready = True
                            break
            except Exception:
                pass
            time.sleep(0.5)

        if not server_ready:
            self.stop()
            raise TimeoutError(f"llama-server did not become ready within {startup_timeout}s.")
        return time.time() - t_start

    def stop(self) -> None:
        self._stop_event.set()
        if self.proc:
            kill_process_tree(self.proc)
            self.proc = None
        wait_for_port_release(self.host, self.port, timeout_sec=8.0)


def send_chat_completion(
    base_url: str,
    messages: List[Dict[str, str]],
    max_tokens: int = 256,
    temperature: float = 0.0,
    timeout: float = 60.0,
) -> Tuple[str, float, int, int, Optional[str]]:
    """Sends a completion request and returns (response_text, latency_ms, prompt_tokens, completion_tokens, error_msg)."""
    url = f"{base_url.rstrip('/')}/v1/chat/completions"
    payload = {
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": False,
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        url, data=data,
        headers={"Content-Type": "application/json", "User-Agent": "Snarta-MMLU/1.0"},
        method="POST",
    )
    t0 = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            elapsed_ms = (time.time() - t0) * 1000.0
            body = json.loads(resp.read().decode("utf-8", errors="replace"))
    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8", errors="replace")
        # Attempt slot erase if context was exhausted
        try:
            erase_req = urllib.request.Request(f"{base_url.rstrip('/')}/slots/0?action=erase", method="POST")
            urllib.request.urlopen(erase_req, timeout=2.0)
        except Exception:
            pass
        # Retry once
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                elapsed_ms = (time.time() - t0) * 1000.0
                body = json.loads(resp.read().decode("utf-8", errors="replace"))
        except Exception:
            return "", 0.0, 0, 0, f"HTTP Error {e.code}: {err_body[:180]}"
    except Exception as e:
        return "", 0.0, 0, 0, str(e)

    msg = body.get("choices", [{}])[0].get("message", {})
    content = msg.get("content") or ""
    reasoning = msg.get("reasoning_content") or ""
    if content and reasoning:
        full_text = f"{reasoning}\n{content}"
    elif content:
        full_text = content
    else:
        full_text = reasoning

    usage = body.get("usage", {})
    prompt_tokens = usage.get("prompt_tokens", 0)
    completion_tokens = usage.get("completion_tokens", 0)
    return full_text, elapsed_ms, prompt_tokens, completion_tokens, None


def evaluate_model_on_questions(
    base_url: str,
    questions: List[MmluQuestion],
    model_name: str,
    max_tokens: int = 256,
    verbose: bool = False,
) -> ModelMmluResult:
    """Evaluates a model over a list of MMLU questions."""
    result = ModelMmluResult(
        model_name=model_name,
        model_path=None,
        ncmoe=None,
        load_time_sec=0.0,
        total_questions=len(questions),
    )

    t_eval_start = time.time()
    total_latency_ms = 0.0
    total_tokens = 0

    print(f"\nEvaluating '{model_name}' on {len(questions)} questions:")
    for idx, q in enumerate(questions):
        prompt = format_mmlu_prompt(q)

        resp_text, latency_ms, p_tok, c_tok, err = send_chat_completion(
            base_url=base_url,
            messages=[{"role": "user", "content": prompt}],
            max_tokens=max_tokens,
            temperature=0.0,
        )

        pred_choice = extract_predicted_choice(resp_text) if not err else None
        is_correct = (pred_choice == q.answer)
        if is_correct:
            result.correct_count += 1

        total_latency_ms += latency_ms
        total_tokens += c_tok

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
        result.evaluations.append(eval_entry)

        cat = q.category
        if cat not in result.category_counts:
            result.category_counts[cat] = {"correct": 0, "total": 0}
        result.category_counts[cat]["total"] += 1
        if is_correct:
            result.category_counts[cat]["correct"] += 1

        running_acc = (result.correct_count / (idx + 1)) * 100.0
        pct = int(((idx + 1) / len(questions)) * 20)
        bar = "=" * pct + "-" * (20 - pct)
        if err:
            status_sym = f"ERR ({err[:18]})"
        elif is_correct:
            status_sym = f"PASS ({pred_choice})"
        else:
            status_sym = f"FAIL (exp {q.answer}, got {pred_choice or '?'})"

        print(
            f"\r  [{bar}] {idx + 1}/{len(questions)} ({((idx + 1)/len(questions))*100:.0f}%) | "
            f"Acc: {running_acc:5.1f}% | {q.subject[:15]:15} | {status_sym:22}",
            end="", flush=True,
        )
        if verbose and (not is_correct or err):
            preview = resp_text.replace('\n', ' ')[:75]
            print(f"\n    [resp] {preview}")

    print()
    result.total_eval_time_sec = time.time() - t_eval_start
    result.overall_accuracy = (result.correct_count / result.total_questions) * 100.0 if result.total_questions > 0 else 0.0
    result.mean_latency_ms = total_latency_ms / max(1, len(questions))
    result.mean_tps = (total_tokens / result.total_eval_time_sec) if result.total_eval_time_sec > 0 else 0.0

    for cat, counts in result.category_counts.items():
        tot = counts["total"]
        cor = counts["correct"]
        result.category_accuracy[cat] = (cor / tot) * 100.0 if tot > 0 else 0.0

    return result


def format_mmlu_markdown(results: List[ModelMmluResult], questions_count: int) -> str:
    """Formats benchmark results as a clean Markdown report."""
    md = [
        "# Snarta MMLU Benchmark Report",
        "",
        f"- Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}",
        f"- Total Evaluated Questions: {questions_count}",
        "",
        "## Overall Results",
        "",
        "| Model | Concurrency | Accuracy (%) | Score | Eval TPS | Avg Latency | Cold Load | Status |",
        "|---|---|---|---|---|---|---|---|",
    ]

    best_idx = max(range(len(results)), key=lambda i: results[i].overall_accuracy) if results else 0
    for idx, r in enumerate(results):
        badge = "WINNER" if idx == best_idx else "OK"
        ncmoe_str = f"-ncmoe {r.ncmoe}" if r.ncmoe is not None else "default"
        md.append(
            f"| `{r.model_name}` | `{ncmoe_str}` | **{r.overall_accuracy:.2f}%** | "
            f"{r.correct_count}/{r.total_questions} | {r.mean_tps:.1f} t/s | {r.mean_latency_ms:.0f}ms | {r.load_time_sec:.1f}s | {badge} |"
        )

    # Category Breakdown
    cats_set: List[str] = []
    base_order = ["STEM", "Humanities", "Social Sciences", "Other", "IT Edge Cases"]
    for r in results:
        for c in r.category_accuracy.keys():
            if c not in cats_set:
                cats_set.append(c)
    ordered_cats = [c for c in base_order if c in cats_set] + [c for c in cats_set if c not in base_order]
    if not ordered_cats:
        ordered_cats = ["STEM", "Humanities", "Social Sciences", "Other"]

    md.extend([
        "",
        "## Category Breakdown",
        "",
        "| Model | " + " | ".join(ordered_cats) + " |",
        "|---|---" + "|---" * len(ordered_cats) + "|",
    ])
    for r in results:
        vals = [f"{r.category_accuracy.get(c, 0.0):.1f}%" for c in ordered_cats]
        md.append(f"| `{r.model_name}` | " + " | ".join(vals) + " |")

    return "\n".join(md)


def save_benchmark_artifacts(
    results: List[ModelMmluResult],
    questions_count: int,
    output_dir: Path,
    no_plots: bool = False,
    run_timestamp: Optional[str] = None,
) -> Tuple[Path, Path, Optional[Path]]:
    """Saves Markdown, JSON and HTML/SVG reports."""
    if not results:
        return Path(), Path(), None

    output_dir.mkdir(parents=True, exist_ok=True)
    ts = run_timestamp or datetime.now().strftime("%Y%m%d_%H%M%S")
    report_tag = results[0].model_name if len(results) == 1 else f"comparison_{len(results)}models"
    base_filename = f"mmlu_{report_tag}_{ts}"

    md_content = format_mmlu_markdown(results, questions_count)
    md_file = output_dir / f"{base_filename}.md"
    md_file.write_text(md_content, encoding="utf-8")

    json_data = {
        "timestamp": datetime.now().isoformat(),
        "questions_evaluated": questions_count,
        "models": [asdict(r) for r in results],
    }
    json_file = output_dir / f"{base_filename}.json"
    json_file.write_text(json.dumps(json_data, indent=2), encoding="utf-8")

    dash_file = None
    if not no_plots:
        try:
            from mmlu_plot import generate_html_dashboard, try_generate_matplotlib_charts
            dash_file = output_dir / f"{base_filename}_dashboard.html"
            generate_html_dashboard(json_data, dash_file)
            png_file = output_dir / f"{base_filename}_chart.png"
            try_generate_matplotlib_charts(json_data, png_file)
        except Exception as e:
            print(f"Notice: Plot generation skipped ({e})")

    return md_file, json_file, dash_file


def run_benchmark(args: argparse.Namespace) -> None:
    output_dir = Path(args.output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    if args.download_official:
        args.dataset = str(download_official_mmlu(Path("bench/data/official_mmlu")))

    cats_filter = args.categories.split(",") if args.categories else None
    subs_filter = args.subjects.split(",") if args.subjects else None
    questions = load_dataset(
        dataset_path=args.dataset,
        limit=args.limit,
        limit_per_subject=args.limit_per_subject,
        categories_filter=cats_filter,
        subjects_filter=subs_filter,
        seed=args.seed,
    )

    if not questions:
        print("Error: No questions matched the specified criteria.", file=sys.stderr)
        sys.exit(1)

    is_hardcore = any("Math & Algo" in q.category or "Cognitive Traps" in q.category or "Multi-Step" in q.category for q in questions)
    is_ru = any(is_cyrillic(q.question) or is_cyrillic(q.category) for q in questions)
    if is_hardcore:
        bench_title = "Snarta Hardcore Adversarial & Reasoning Stress Benchmark"
    elif is_ru:
        bench_title = "Snarta RuMMLU / MERA Intelligence Benchmark"
    elif args.dataset and not any(k in str(args.dataset).lower() for k in ("mmlu_sample", "official")):
        clean_ds_name = Path(args.dataset).stem.replace("-", " ").replace("_", " ").title()
        bench_title = f"Snarta {clean_ds_name} Benchmark"
    else:
        bench_title = "Snarta MMLU Intelligence Benchmark"

    print("=" * 80)
    print(bench_title)
    print("=" * 80)
    print(f"Total Questions : {len(questions)}")
    cats_dist = {}
    for q in questions:
        cats_dist[q.category] = cats_dist.get(q.category, 0) + 1
    print(f"Category Dist   : {cats_dist}")
    print("=" * 80)

    results: List[ModelMmluResult] = []

    if args.server_url:
        print(f"Connecting to running server: {args.server_url}")
        res = evaluate_model_on_questions(
            args.server_url,
            questions,
            model_name=args.model_name or "external-server",
            max_tokens=args.max_tokens,
            verbose=args.verbose,
        )
        results.append(res)
    else:
        server_path = Path(args.server).resolve() if args.server else find_llama_server()
        if not server_path or not server_path.exists():
            print("Error: Could not locate llama-server executable. Use --server <path>", file=sys.stderr)
            sys.exit(1)

        model_files = find_models(arg_models=args.models, models_dir=args.models_dir)
        if args.model:
            model_files = [Path(args.model).resolve()]

        if not model_files:
            print("Error: No GGUF models found. Specify with -m <model.gguf> or --models <glob>", file=sys.stderr)
            sys.exit(1)

        print(f"Models to evaluate ({len(model_files)}): {[m.name for m in model_files]}")
        run_timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

        for m_idx, m_path in enumerate(model_files):
            m_name = m_path.name
            print(f"\n[{m_idx + 1}/{len(model_files)}] Benchmarking model: {m_name}")

            if args.ncmoe == "auto":
                opt_ncmoe = detect_optimal_ncmoe(m_name)
            elif args.ncmoe.lower() in ("none", "off", "0"):
                opt_ncmoe = None
            else:
                opt_ncmoe = int(args.ncmoe)

            cmd = [
                str(server_path),
                "-m", str(m_path),
                "--host", args.host,
                "--port", str(args.port),
                "--jinja",
                "--reasoning-format", args.reasoning_format,
                "--reasoning", args.reasoning,
                "--reasoning-effort", args.reasoning_effort,
                "-c", str(args.ctx),
                "-ngl", str(args.ngl),
                "-b", str(args.batch),
                "-ub", str(args.ubatch),
                "-fa", args.fa,
                "-np", "1",
                "--slots",
                "-ctk", args.kv_type,
                "-ctv", args.kv_type,
            ]
            if args.load_mode and args.load_mode != "none":
                cmd.extend(["--load-mode", args.load_mode])
            if opt_ncmoe is not None:
                cmd.extend(["-ncmoe", str(opt_ncmoe)])
            if args.extra_args:
                cmd.extend(args.extra_args.split())

            session = ServerSession(cmd, args.host, args.port, verbose=args.verbose)
            try:
                print(f"  Starting server with -ncmoe {opt_ncmoe}...", end="", flush=True)
                load_time = session.start(startup_timeout=args.startup_timeout)
                print(f" ready ({load_time:.2f}s)")

                base_url = f"http://{args.host}:{args.port}"
                model_res = evaluate_model_on_questions(
                    base_url,
                    questions,
                    model_name=m_name,
                    max_tokens=args.max_tokens,
                    verbose=args.verbose,
                )
                model_res.model_path = str(m_path)
                model_res.ncmoe = opt_ncmoe
                model_res.load_time_sec = load_time
                results.append(model_res)

                # Progressive save after each successful model
                save_benchmark_artifacts(
                    results, len(questions), output_dir, args.no_plots, run_timestamp=run_timestamp
                )

            except Exception as e:
                print(f"\n  [ERROR] Model '{m_name}' encountered an error: {e}")
                print("  Skipping to next model...")
            finally:
                print("  Stopping server...", end="", flush=True)
                session.stop()
                print(" stopped.")
                if m_idx + 1 < len(model_files) and args.cooldown > 0:
                    print(f"  Cooldown ({args.cooldown:.1f}s) for VRAM release...", end="", flush=True)
                    time.sleep(args.cooldown)
                    print(" done.")

    if results:
        md_file, json_file, dash_file = save_benchmark_artifacts(
            results, len(questions), output_dir, args.no_plots
        )
        print(f"\nFinal Markdown report saved: {md_file}")
        print(f"Final JSON report saved:     {json_file}")
        if dash_file:
            print(f"Final HTML Dashboard saved:  {dash_file}")

    print("\n" + "=" * 80)
    print("MMLU BENCHMARK SUMMARY")
    print("=" * 80)
    for r in results:
        print(f"Model: {r.model_name:<36} | Accuracy: {r.overall_accuracy:5.2f}% ({r.correct_count}/{r.total_questions}) | Speed: {r.mean_tps:5.1f} t/s")
        parts = [f"{c}: {r.category_accuracy.get(c, 0.0):.1f}%" for c in r.category_accuracy.keys()]
        print(f"  Breakdown: {' | '.join(parts)}")
    print("=" * 80)


def build_arg_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Snarta MMLU Intelligence Benchmark Suite")
    p.add_argument("-s", "--server", help="Path to llama-server executable")
    p.add_argument("-m", "--model", help="Path to a single GGUF model")
    p.add_argument("--models", help="Comma-separated paths or glob patterns of models")
    p.add_argument("--models-dir", help="Directory containing GGUF models to test")
    p.add_argument("--server-url", help="URL of existing running llama-server (e.g. http://127.0.0.1:8089)")
    p.add_argument("--model-name", help="Display name when using --server-url")
    p.add_argument("--host", default="127.0.0.1", help="llama-server host (default: 127.0.0.1)")
    p.add_argument("--port", type=int, default=8089, help="llama-server port (default: 8089)")
    p.add_argument("--dataset", help="Path to JSON or CSV dataset (default: bench/data/mmlu_sample.json)")
    p.add_argument("--download-official", action="store_true", help="Download official 57-subject Hendrycks MMLU dataset")
    p.add_argument("--categories", help="Comma-separated categories to filter (e.g. STEM,Humanities)")
    p.add_argument("--subjects", help="Comma-separated subjects to filter")
    p.add_argument("--limit", type=int, help="Limit total questions evaluated")
    p.add_argument("--limit-per-subject", type=int, help="Limit questions per subject")
    p.add_argument("--seed", type=int, default=42, help="Random seed for sampling (default: 42)")
    p.add_argument("--ncmoe", default="auto", help="Concurrency -ncmoe value or 'auto' (default: auto)")
    p.add_argument("--cooldown", type=float, default=5.0, help="VRAM cooldown seconds between models (default: 5.0)")
    p.add_argument("--startup-timeout", type=float, default=180.0, help="Server startup timeout (default: 180s)")
    p.add_argument("-c", "--ctx", type=int, default=32000, help="Context size (default: 32000)")
    p.add_argument("-ngl", type=int, default=99, help="GPU offload layers (default: 99)")
    p.add_argument("-b", "--batch", type=int, default=2048, help="Batch size (default: 2048)")
    p.add_argument("-ub", "--ubatch", type=int, default=512, help="Microbatch size (default: 512)")
    p.add_argument("--max-tokens", type=int, default=256, help="Max output tokens per question (default: 256)")
    p.add_argument("--reasoning", default="off", help="Reasoning mode: off or on (default: off)")
    p.add_argument("--reasoning-format", default="auto", help="Reasoning format (default: auto)")
    p.add_argument("--reasoning-effort", default="none", help="Reasoning effort: none, low, medium, high (default: none)")
    p.add_argument("--kv-type", default="q8_0", help="KV cache quantization (default: q8_0)")
    p.add_argument("--load-mode", default="mlock", help="Model load mode (default: mlock)")
    p.add_argument("--fa", default="on", help="Flash attention (default: on)")
    p.add_argument("--extra-args", help="Extra arguments to pass to llama-server")
    p.add_argument("-o", "--output-dir", default="bench/reports", help="Output directory (default: bench/reports)")
    p.add_argument("-v", "--verbose", action="store_true", help="Verbose logging")
    p.add_argument("--no-plots", action="store_true", help="Disable HTML/SVG dashboard generation")
    return p


if __name__ == "__main__":
    parser = build_arg_parser()
    args = parser.parse_args()
    run_benchmark(args)
