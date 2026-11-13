#!/usr/bin/env python3
"""
Snarta MoE Benchmark Tool

Automated benchmark for finding the optimal -ncmoe parameter and launch
characteristics for llama-server with MoE models (such as Gemma 4 26B-A4B).

Measures:
- Generation speed (eval tokens/sec)
- Prompt evaluation speed (prompt eval tokens/sec)
- Total latency and time-to-first-token
- Model loading duration
- Statistical stability across multiple runs (mean, min, max, stdev)

Exports results to terminal table, Markdown report, and JSON dataset.
Zero external mandatory dependencies (pure Python standard library).
"""

import argparse
import glob
import json
import os
import platform
import re
import socket
import statistics
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

from mmlu_dataset import find_matching_mtp_layer

# Regex patterns for parsing llama-server stderr/stdout log lines
PROMPT_EVAL_RE = re.compile(
    r"prompt eval time\s*=\s*([\d\.]+)\s*ms\s*/\s*(\d+)\s*tokens\s*\(\s*[\d\.]+\s*ms per token,\s*([\d\.]+)\s*tokens per second\)"
)
EVAL_TIME_RE = re.compile(
    r"(?<!prompt )eval time\s*=\s*([\d\.]+)\s*ms\s*/\s*(\d+)\s*tokens\s*\(\s*[\d\.]+\s*ms per token,\s*([\d\.]+)\s*tokens per second\)"
)
TOTAL_TIME_RE = re.compile(
    r"total time\s*=\s*([\d\.]+)\s*ms\s*/\s*(\d+)\s*tokens"
)
TG_LOG_RE = re.compile(
    r"tg\s*=\s*([\d\.]+)\s*t/s"
)
SERVER_LISTENING_RE = re.compile(
    r"listening on\s+(http://[^\s]+)"
)

DEFAULT_PROMPT = (
    "Solve the following reasoning problem step by step:\n"
    "A train leaves Station A at 08:00 traveling at 60 km/h toward Station B. "
    "At 09:30, an express train leaves Station B traveling at 100 km/h toward Station A. "
    "The total track distance between A and B is 450 km. "
    "At what time will the two trains cross each other, and how far from Station A will the meeting point be? "
    "Show full mathematical calculations and verify your answer."
)


@dataclass
class SingleRunResult:
    run_idx: int
    eval_tps: float
    prompt_eval_tps: float
    eval_ms: float
    prompt_ms: float
    total_ms: float
    prompt_tokens: int
    completion_tokens: int
    error: Optional[str] = None


@dataclass
class NcmoeBenchmarkResult:
    ncmoe_label: str
    ncmoe_val: Optional[int]
    load_time_sec: float
    runs: List[SingleRunResult] = field(default_factory=list)
    mean_eval_tps: float = 0.0
    min_eval_tps: float = 0.0
    max_eval_tps: float = 0.0
    stdev_eval_tps: float = 0.0
    mean_prompt_eval_tps: float = 0.0
    mean_total_ms: float = 0.0
    success: bool = True
    error_message: Optional[str] = None

    def compute_summary(self) -> None:
        valid_runs = [r for r in self.runs if r.error is None and r.eval_tps > 0.0]
        if not valid_runs:
            self.success = False
            if not self.error_message:
                self.error_message = "All test runs failed or returned zero tokens."
            return

        tps_list = [r.eval_tps for r in valid_runs]
        p_tps_list = [r.prompt_eval_tps for r in valid_runs]
        tot_list = [r.total_ms for r in valid_runs]

        self.mean_eval_tps = statistics.mean(tps_list)
        self.min_eval_tps = min(tps_list)
        self.max_eval_tps = max(tps_list)
        self.stdev_eval_tps = statistics.stdev(tps_list) if len(tps_list) > 1 else 0.0
        self.mean_prompt_eval_tps = statistics.mean(p_tps_list)
        self.mean_total_ms = statistics.mean(tot_list)
        self.success = True


def is_port_open(host: str, port: int) -> bool:
    """Checks whether the given TCP port is currently open and accepting connections."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(0.5)
        return s.connect_ex((host, port)) == 0


def wait_for_port_release(host: str, port: int, timeout_sec: float = 10.0) -> bool:
    """Blocks until the specified TCP port is released by the operating system."""
    start = time.time()
    while time.time() - start < timeout_sec:
        if not is_port_open(host, port):
            return True
        time.sleep(0.3)
    return False


def kill_process_tree(proc: subprocess.Popen, timeout_sec: float = 5.0) -> None:
    """Terminates a process and its child processes cleanly across Windows and Linux."""
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
    """Auto-detects the llama-server executable in the repo or environment."""
    patterns = [
        "target/release/runtime/*/llama-server.exe",
        "runtime/*/llama-server.exe",
        "runtime/llama-server.exe",
        "target/release/runtime/*/llama-server",
        "runtime/*/llama-server",
        "runtime/llama-server",
        "llama-server.exe",
        "llama-server",
    ]
    for pattern in patterns:
        matches = glob.glob(pattern, recursive=True)
        if matches:
            return Path(matches[0]).resolve()
    return None


def find_default_model() -> Optional[Path]:
    """Auto-detects the first GGUF model found in standard folders."""
    patterns = [
        "target/release/models/*.gguf",
        "models/*.gguf",
        "*.gguf",
    ]
    for pattern in patterns:
        matches = glob.glob(pattern, recursive=True)
        if matches:
            return Path(matches[0]).resolve()
    return None


class ServerSession:
    """Manages the background execution, log streaming, and teardown of llama-server."""

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
        """Starts the server process and waits for the /health endpoint to report ready."""
        t_start = time.time()

        creationflags = 0
        if platform.system() == "Windows":
            # 0x08000000 = CREATE_NO_WINDOW
            creationflags = 0x08000000

        self.proc = subprocess.Popen(
            self.cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            encoding="utf-8",
            errors="replace",
            creationflags=creationflags,
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

        # Wait for HTTP health check
        health_url = f"http://{self.host}:{self.port}/health"
        server_ready = False
        while time.time() - t_start < startup_timeout:
            if self.proc.poll() is not None:
                err_excerpt = "\n".join(self.logs[-15:])
                raise RuntimeError(
                    f"llama-server exited prematurely with code {self.proc.returncode}:\n{err_excerpt}"
                )

            try:
                req = urllib.request.Request(
                    health_url,
                    headers={"User-Agent": "Snarta-Bench/1.0"},
                )
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
            raise TimeoutError(
                f"llama-server did not become ready within {startup_timeout}s."
            )

        load_time = time.time() - t_start
        return load_time

    def stop(self) -> None:
        """Stops the server process and waits for socket release."""
        self._stop_event.set()
        if self.proc:
            kill_process_tree(self.proc)
            self.proc = None
        wait_for_port_release(self.host, self.port, timeout_sec=8.0)


def send_completion(
    host: str,
    port: int,
    prompt: str,
    max_tokens: int,
    temperature: float = 0.7,
    timeout: float = 300.0,
) -> Tuple[Dict[str, Any], float]:
    """Sends a chat completion request to llama-server and returns response JSON and wall-clock time."""
    url = f"http://{host}:{port}/v1/chat/completions"
    payload = {
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": False,
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json", "User-Agent": "Snarta-Bench/1.0"},
        method="POST",
    )

    t0 = time.time()
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        elapsed = time.time() - t0
        body = resp.read().decode("utf-8", errors="replace")
        return json.loads(body), elapsed


def parse_timings_from_logs(logs: List[str]) -> Tuple[Optional[float], Optional[float], Optional[float], Optional[float]]:
    """Fallback extractor: parses prompt eval and generation timings from recent server logs."""
    p_tps: Optional[float] = None
    g_tps: Optional[float] = None
    p_ms: Optional[float] = None
    g_ms: Optional[float] = None

    for line in reversed(logs):
        if p_tps is None:
            m_prompt = PROMPT_EVAL_RE.search(line)
            if m_prompt:
                p_ms = float(m_prompt.group(1))
                p_tps = float(m_prompt.group(3))
        if g_tps is None:
            m_eval = EVAL_TIME_RE.search(line)
            if m_eval:
                g_ms = float(m_eval.group(1))
                g_tps = float(m_eval.group(3))

        if p_tps is not None and g_tps is not None:
            break

    return p_tps, g_tps, p_ms, g_ms


def format_table(headers: List[str], rows: List[List[str]]) -> str:
    """Renders a clean ASCII table with aligned columns."""
    col_widths = [len(h) for h in headers]
    for row in rows:
        for i, val in enumerate(row):
            if i < len(col_widths):
                col_widths[i] = max(col_widths[i], len(str(val)))

    sep = "+-" + "-+-".join("-" * w for w in col_widths) + "-+"
    hdr = "| " + " | ".join(h.ljust(w) for h, w in zip(headers, col_widths)) + " |"

    lines = [sep, hdr, sep]
    for row in rows:
        r_str = "| " + " | ".join(str(val).ljust(w) for val, w in zip(row, col_widths)) + " |"
        lines.append(r_str)
    lines.append(sep)
    return "\n".join(lines)


def run_benchmark(args: argparse.Namespace) -> None:
    server_path = Path(args.server).resolve() if args.server else find_llama_server()
    if not server_path or not server_path.exists():
        print("Error: Could not locate llama-server executable.", file=sys.stderr)
        print("Please provide the path using --server <path/to/llama-server.exe>", file=sys.stderr)
        sys.exit(1)

    model_path = Path(args.model).resolve() if args.model else find_default_model()
    if not model_path or not model_path.exists():
        print("Error: Could not locate GGUF model file.", file=sys.stderr)
        print("Please provide the model path using --model <path/to/model.gguf>", file=sys.stderr)
        sys.exit(1)

    # Parse -ncmoe candidates (via --range or --ncmoe list)
    ncmoe_candidates: List[Optional[int]] = []
    if args.range:
        if len(args.range) == 1:
            start, stop, step = 2, args.range[0], 2
        elif len(args.range) == 2:
            start, stop = args.range[0], args.range[1]
            step = 1 if abs(stop - start) <= 4 else 2
        elif len(args.range) >= 3:
            start, stop, step = args.range[0], args.range[1], args.range[2]
        else:
            start, stop, step = 2, 28, 2

        if step <= 0:
            step = 1

        if not args.no_baseline:
            ncmoe_candidates.append(None)

        curr = start
        while curr <= stop:
            ncmoe_candidates.append(curr)
            curr += step
    else:
        raw_ncmoe = args.ncmoe.replace(" ", ",").split(",")
        for item in raw_ncmoe:
            clean = item.strip().lower()
            if not clean:
                continue
            if clean in ("none", "-1", "baseline", "off"):
                if not args.no_baseline:
                    ncmoe_candidates.append(None)
            else:
                try:
                    ncmoe_candidates.append(int(clean))
                except ValueError:
                    print(f"Warning: Invalid ncmoe value '{clean}', skipping.", file=sys.stderr)

    if not ncmoe_candidates:
        if not args.no_baseline:
            ncmoe_candidates.append(None)
        ncmoe_candidates.extend([2, 4, 6, 8, 12, 18, 24])

    print("=" * 80)
    print("Snarta MoE Sweep Benchmark")
    print("=" * 80)
    print(f"Executable : {server_path}")
    print(f"Model      : {model_path.name}")
    print(f"Host:Port  : {args.host}:{args.port}")
    print(f"Context    : {args.ctx} tokens")
    print(f"Candidates : {[('baseline' if v is None else f'-ncmoe {v}') for v in ncmoe_candidates]}")
    print(f"Runs/Config: {args.runs} measurement(s) (+ {args.warmup} warmup)")
    print(f"Max Tokens : {args.max_tokens}")
    print(f"Cooldown   : {args.cooldown:.1f}s (pre-unload delay: {args.pre_unload_delay:.1f}s)")

    # MTP Draft Layer
    mtp_path = None
    if args.mtp and args.mtp.lower() not in ("none", "off", "false", "no"):
        if args.mtp.lower() in ("auto", "on", "yes", "true"):
            mtp_path = find_matching_mtp_layer(model_path)
            if not mtp_path:
                print(f"Notice: --mtp={args.mtp} requested, but no matching mtp-*.gguf found near {model_path.name}.")
        else:
            custom_p = Path(args.mtp)
            if custom_p.exists():
                mtp_path = custom_p.resolve()
            elif (model_path.parent / custom_p).exists():
                mtp_path = (model_path.parent / custom_p).resolve()
            else:
                print(f"Warning: Specified MTP file not found: {args.mtp}")

    if mtp_path:
        print(f"MTP Drafter: {mtp_path.name} (speculative decoding active)")
    else:
        print("MTP Drafter: None (standard autoregressive generation)")
    print("=" * 80, flush=True)

    # Base server arguments
    base_args = [
        str(server_path),
        "-m", str(model_path),
        "--host", args.host,
        "--port", str(args.port),
        "--jinja",
        "--reasoning-format", "auto",
        "--reasoning", args.reasoning,
        "--reasoning-effort", args.reasoning_effort,
        "-c", str(args.ctx),
        "-ngl", str(args.ngl),
        "-b", str(args.batch),
        "-ub", str(args.ubatch),
        "-fa", args.fa,
        "-np", "1",
        "-ctk", args.kv_type,
        "-ctv", args.kv_type,
    ]

    if mtp_path:
        base_args.extend(["-md", str(mtp_path), "--spec-type", "draft-mtp"])

    if args.load_mode and args.load_mode != "none":
        base_args.extend(["--load-mode", args.load_mode])

    if args.extra_args:
        base_args.extend(args.extra_args.split())

    results: List[NcmoeBenchmarkResult] = []

    for idx, ncmoe in enumerate(ncmoe_candidates):
        label = "baseline (no -ncmoe)" if ncmoe is None else f"-ncmoe {ncmoe}"
        print(f"\n[{idx + 1}/{len(ncmoe_candidates)}] Benchmarking configuration: {label}")

        cmd = list(base_args)
        if ncmoe is not None:
            cmd.extend(["-ncmoe", str(ncmoe)])

        # Ensure port is clean before launch
        if is_port_open(args.host, args.port):
            print(f"  Notice: Port {args.port} is busy, waiting for release...")
            if not wait_for_port_release(args.host, args.port, timeout_sec=5.0):
                print(f"  Warning: Port {args.port} remained busy, will attempt to proceed.")

        bench_entry = NcmoeBenchmarkResult(
            ncmoe_label=label,
            ncmoe_val=ncmoe,
            load_time_sec=0.0,
        )

        session = ServerSession(cmd, args.host, args.port, verbose=args.verbose)
        try:
            print("  Starting llama-server and loading weights...", end="", flush=True)
            load_time = session.start(startup_timeout=args.startup_timeout)
            bench_entry.load_time_sec = load_time
            print(f" ready ({load_time:.2f}s)")

            # Warmup runs
            for w in range(args.warmup):
                print(f"  Warmup [{w + 1}/{args.warmup}]...", end="", flush=True)
                _, w_time = send_completion(
                    args.host,
                    args.port,
                    prompt="Hello! Explain briefly in 20 words what an expert layer is.",
                    max_tokens=32,
                    timeout=args.request_timeout,
                )
                print(f" done ({w_time:.2f}s)")

            # Measurement runs
            for r in range(args.runs):
                print(f"  Measurement run [{r + 1}/{args.runs}]...", end="", flush=True)
                try:
                    resp, wall_time = send_completion(
                        args.host,
                        args.port,
                        prompt=args.prompt,
                        max_tokens=args.max_tokens,
                        timeout=args.request_timeout,
                    )

                    # Extract timings from response JSON
                    timings = resp.get("timings", {})
                    usage = resp.get("usage", {})

                    g_tps = float(timings.get("predicted_per_second", 0.0))
                    p_tps = float(timings.get("prompt_per_second", 0.0))
                    g_ms = float(timings.get("predicted_ms", 0.0))
                    p_ms = float(timings.get("prompt_ms", 0.0))

                    # If API didn't report timings, parse from captured server logs
                    if g_tps <= 0.0:
                        lp_tps, lg_tps, lp_ms, lg_ms = parse_timings_from_logs(session.logs)
                        if lg_tps is not None:
                            g_tps = lg_tps
                        if lp_tps is not None:
                            p_tps = lp_tps
                        if lg_ms is not None:
                            g_ms = lg_ms
                        if lp_ms is not None:
                            p_ms = lp_ms

                    p_tokens = int(usage.get("prompt_tokens", timings.get("prompt_n", 0)))
                    c_tokens = int(usage.get("completion_tokens", timings.get("predicted_n", 0)))
                    tot_ms = wall_time * 1000.0

                    single_run = SingleRunResult(
                        run_idx=r + 1,
                        eval_tps=g_tps,
                        prompt_eval_tps=p_tps,
                        eval_ms=g_ms,
                        prompt_ms=p_ms,
                        total_ms=tot_ms,
                        prompt_tokens=p_tokens,
                        completion_tokens=c_tokens,
                    )
                    bench_entry.runs.append(single_run)
                    print(f" {g_tps:.2f} t/s (eval), {p_tps:.1f} t/s (prefill)")

                except Exception as ex:
                    print(f" failed: {ex}")
                    bench_entry.runs.append(
                        SingleRunResult(
                            run_idx=r + 1,
                            eval_tps=0.0,
                            prompt_eval_tps=0.0,
                            eval_ms=0.0,
                            prompt_ms=0.0,
                            total_ms=0.0,
                            prompt_tokens=0,
                            completion_tokens=0,
                            error=str(ex),
                        )
                    )

            bench_entry.compute_summary()

        except Exception as ex:
            bench_entry.success = False
            bench_entry.error_message = str(ex)
            print(f"\n  Run failed: {ex}")
        finally:
            if args.pre_unload_delay > 0:
                time.sleep(args.pre_unload_delay)
            print("  Stopping llama-server...", end="", flush=True)
            session.stop()
            print(" stopped.")

            # Cooldown delay before next model load to allow ROCm driver/VRAM to flush
            if idx + 1 < len(ncmoe_candidates) and args.cooldown > 0:
                print(f"  Cooldown ({args.cooldown:.1f}s) for VRAM and driver memory reclamation...", end="", flush=True)
                time.sleep(args.cooldown)
                print(" ready.")

        results.append(bench_entry)

    # Print Summary Table
    print("\n" + "=" * 90)
    print(f"BENCHMARK RESULTS: {model_path.name}")
    print("=" * 90)

    # Find baseline mean tps for speedup calculations
    baseline_entry = next((r for r in results if r.ncmoe_val is None and r.success), None)
    baseline_tps = baseline_entry.mean_eval_tps if baseline_entry else None

    # Find fastest configuration
    successful_runs = [r for r in results if r.success and r.mean_eval_tps > 0.0]
    best_entry = max(successful_runs, key=lambda x: x.mean_eval_tps) if successful_runs else None

    table_headers = [
        "Configuration",
        "Eval (t/s)",
        "Min..Max",
        "Prefill (t/s)",
        "Load (s)",
        "Speedup",
        "Status",
    ]
    table_rows = []

    for r in results:
        if not r.success:
            table_rows.append([
                r.ncmoe_label,
                "0.00",
                "-",
                "-",
                f"{r.load_time_sec:.1f}",
                "-",
                f"FAILED ({r.error_message[:20]}...)" if r.error_message else "FAILED",
            ])
            continue

        stdev_str = f"+/-{r.stdev_eval_tps:.2f}" if r.stdev_eval_tps > 0.0 else ""
        eval_str = f"{r.mean_eval_tps:.2f} {stdev_str}".strip()
        min_max_str = f"{r.min_eval_tps:.2f}..{r.max_eval_tps:.2f}"

        speedup_str = "1.00x (baseline)"
        if baseline_tps and baseline_tps > 0.0 and r.mean_eval_tps > 0.0:
            ratio = r.mean_eval_tps / baseline_tps
            pct = (ratio - 1.0) * 100.0
            speedup_str = f"{ratio:.2f}x ({pct:+.1f}%)"

        status_str = "OK"
        if best_entry and r == best_entry:
            status_str = "WINNER"

        table_rows.append([
            r.ncmoe_label,
            eval_str,
            min_max_str,
            f"{r.mean_prompt_eval_tps:.1f}",
            f"{r.load_time_sec:.1f}",
            speedup_str,
            status_str,
        ])

    print(format_table(table_headers, table_rows))

    if best_entry:
        opt_flag = "" if best_entry.ncmoe_val is None else f"-ncmoe {best_entry.ncmoe_val}"
        print("\n" + "=" * 90)
        print(f"Optimal Configuration: {best_entry.ncmoe_label}")
        print(f"Generation Speed     : {best_entry.mean_eval_tps:.2f} tokens/second")
        if baseline_tps and best_entry.mean_eval_tps > baseline_tps:
            pct_boost = ((best_entry.mean_eval_tps / baseline_tps) - 1.0) * 100.0
            print(f"Speedup vs Baseline  : +{pct_boost:.1f}%")
        print("\nRecommended setting for snarta.toml:")
        if opt_flag:
            print(f'  extra_args = "{opt_flag}"')
        else:
            print('  extra_args = ""  # Baseline (pure GPU offloading) yielded highest throughput')
        print("=" * 90)

    # Save Reports
    out_dir = Path(args.output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    clean_model_name = model_path.stem

    # 1. JSON Report
    json_path = out_dir / f"bench_{clean_model_name}_{timestamp}.json"
    report_data = {
        "timestamp": datetime.now().isoformat(),
        "model": str(model_path),
        "model_name": model_path.name,
        "server": str(server_path),
        "host": args.host,
        "port": args.port,
        "ctx": args.ctx,
        "runs": args.runs,
        "max_tokens": args.max_tokens,
        "mtp_layer": mtp_path.name if mtp_path else None,
        "best_ncmoe": best_entry.ncmoe_val if best_entry else None,
        "best_eval_tps": best_entry.mean_eval_tps if best_entry else 0.0,
        "results": [asdict(r) for r in results],
    }
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(report_data, f, indent=2)
    print(f"\nJSON results saved to    : {json_path}")

    # 2. Markdown Report
    md_path = out_dir / f"bench_{clean_model_name}_{timestamp}.md"
    md_lines = [
        f"# Snarta MoE Benchmark: {model_path.name}",
        "",
        f"- Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}",
        f"- Executable: `{server_path}`",
        f"- Model: `{model_path.name}`",
        f"- Context: `{args.ctx}` | Max Tokens: `{args.max_tokens}` | Runs per config: `{args.runs}`",
        f"- MTP Layer: `{mtp_path.name if mtp_path else 'None'}`",
        "",
        "## Summary Results",
        "",
        "| Configuration | Eval (t/s) | Min..Max | Prefill (t/s) | Load (s) | Speedup | Status |",
        "|---|---|---|---|---|---|---|",
    ]
    for row in table_rows:
        md_lines.append("| " + " | ".join(row) + " |")

    recommended_arg = (
        f"-ncmoe {best_entry.ncmoe_val}"
        if best_entry and best_entry.ncmoe_val is not None
        else ""
    )
    rec_toml_lines = [
        "",
        "## Recommended snarta.toml Configuration",
        "",
        "```toml",
        f"# Model: {model_path.name}",
    ]
    if mtp_path:
        rec_toml_lines.append(f'mtp = "{mtp_path.name}"')
    if recommended_arg:
        rec_toml_lines.append(f'extra_args = "{recommended_arg}"')
    rec_toml_lines.append("```")
    md_lines.extend(rec_toml_lines)
    with open(md_path, "w", encoding="utf-8") as f:
        f.write("\n".join(md_lines) + "\n")
    print(f"Markdown report saved to : {md_path}")


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Snarta MoE Benchmark Tool for llama-server",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "-s", "--server",
        type=str,
        default=None,
        help="Path to llama-server executable (auto-detected if omitted)",
    )
    parser.add_argument(
        "-m", "--model",
        type=str,
        default=None,
        help="Path to GGUF model file (auto-detected if omitted)",
    )
    parser.add_argument(
        "--ncmoe",
        type=str,
        default="none,2,4,6,8,12,18,24",
        help="Comma-separated list of -ncmoe values to sweep ('none' = without flag)",
    )
    parser.add_argument(
        "--mtp",
        type=str,
        default="none",
        nargs="?",
        const="auto",
        help="MTP draft layer file path, 'auto' or 'on' to auto-detect mtp-*.gguf near model, or 'none' to disable",
    )
    parser.add_argument(
        "--range",
        nargs="+",
        type=int,
        default=None,
        help="Generate ncmoe sweep range: START STOP [STEP]. Example: --range 2 28 2",
    )
    parser.add_argument(
        "--no-baseline",
        action="store_true",
        help="Do not include the baseline run (without -ncmoe) in the benchmark",
    )
    parser.add_argument(
        "--cooldown",
        type=float,
        default=4.0,
        help="Cooldown delay in seconds after unloading and before next load (VRAM cleanup)",
    )
    parser.add_argument(
        "--pre-unload-delay",
        type=float,
        default=1.0,
        help="Delay in seconds before stopping the server to allow socket/log flush",
    )
    parser.add_argument(
        "-r", "--runs",
        type=int,
        default=2,
        help="Number of measurement runs per configuration",
    )
    parser.add_argument(
        "--warmup",
        type=int,
        default=1,
        help="Number of initial warmup runs per configuration",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=256,
        help="Max tokens to generate per request",
    )
    parser.add_argument(
        "-c", "--ctx",
        type=int,
        default=32000,
        help="Context window size (-c)",
    )
    parser.add_argument(
        "-ngl",
        type=int,
        default=99,
        help="Number of GPU layers (-ngl)",
    )
    parser.add_argument(
        "-b", "--batch",
        type=int,
        default=2048,
        help="Batch size (-b)",
    )
    parser.add_argument(
        "-ub", "--ubatch",
        type=int,
        default=512,
        help="Physical micro-batch size (-ub)",
    )
    parser.add_argument(
        "--kv-type",
        type=str,
        default="q8_0",
        help="KV cache quant type (-ctk/-ctv)",
    )
    parser.add_argument(
        "--load-mode",
        type=str,
        default="mlock",
        help="Memory load mode (--load-mode: mlock, none, auto)",
    )
    parser.add_argument(
        "--fa",
        type=str,
        default="on",
        choices=["on", "off"],
        help="Flash attention (-fa)",
    )
    parser.add_argument(
        "--reasoning",
        type=str,
        default="on",
        choices=["on", "off"],
        help="Reasoning flag (--reasoning)",
    )
    parser.add_argument(
        "--reasoning-effort",
        type=str,
        default="xhigh",
        help="Reasoning effort level (--reasoning-effort)",
    )
    parser.add_argument(
        "--host",
        type=str,
        default="127.0.0.1",
        help="Server host address",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8089,
        help="Server port",
    )
    parser.add_argument(
        "--prompt",
        type=str,
        default=DEFAULT_PROMPT,
        help="Prompt text for benchmarking generation throughput",
    )
    parser.add_argument(
        "--extra-args",
        type=str,
        default="",
        help="Additional custom CLI arguments forwarded to llama-server",
    )
    parser.add_argument(
        "--startup-timeout",
        type=float,
        default=180.0,
        help="Max seconds to wait for server to load model and report healthy",
    )
    parser.add_argument(
        "--request-timeout",
        type=float,
        default=300.0,
        help="Max seconds to wait for completion response",
    )
    parser.add_argument(
        "-o", "--output-dir",
        type=str,
        default="bench/reports",
        help="Directory where JSON and Markdown benchmark reports are saved",
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Stream raw llama-server output to terminal during benchmark",
    )
    return parser.parse_args()


if __name__ == "__main__":
    run_benchmark(parse_arguments())
