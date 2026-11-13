#!/usr/bin/env python3
"""
Snarta Benchmark Terminal User Interface (TUI) - UI Rendering & Input

Pure Python standard library implementation supporting Windows 11 and Linux.
Provides ANSI styling, box drawing, keyboard handling, and screen renderers.
Zero external dependencies. ZERO emojis across all strings and code.
"""

import os
import platform
import re
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Tuple

ANSI_RE = re.compile(r"\x1b\[[0-9;]*[a-zA-Z]")


class Colors:
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    UNDERLINE = "\033[4m"
    REVERSE = "\033[7m"
    BLACK, RED, GREEN, YELLOW = "\033[30m", "\033[31m", "\033[32m", "\033[33m"
    BLUE, MAGENTA, CYAN, WHITE = "\033[34m", "\033[35m", "\033[36m", "\033[37m"
    GRAY, BRIGHT_RED, BRIGHT_GREEN = "\033[90m", "\033[91m", "\033[92m"
    BRIGHT_YELLOW, BRIGHT_BLUE = "\033[93m", "\033[94m"
    BRIGHT_MAGENTA, BRIGHT_CYAN, BRIGHT_WHITE = "\033[95m", "\033[96m", "\033[97m"
    BG_BLUE, BG_CYAN, BG_DARK = "\033[44m", "\033[46m", "\033[100m"


def strip_ansi(text: str) -> str:
    """Removes ANSI escape codes to determine true visual character width."""
    return ANSI_RE.sub("", text)


def truncate_visible(text: str, max_w: int) -> str:
    """Safely truncates string to max visual width without breaking ANSI sequences."""
    vis_len = len(strip_ansi(text))
    if vis_len <= max_w:
        return text
    target = max(1, max_w - 3)
    result = []
    curr_w = 0
    in_ansi = False
    for ch in text:
        if ch == "\033":
            in_ansi = True
            result.append(ch)
        elif in_ansi:
            result.append(ch)
            if ch.isalpha():
                in_ansi = False
        else:
            if curr_w < target:
                result.append(ch)
                curr_w += 1
            else:
                break
    result.append("...")
    result.append(Colors.RESET)
    return "".join(result)


def pad_line(text: str, width: int) -> str:
    """Pads or truncates text to ensure its visual length matches width exactly."""
    vis_len = len(strip_ansi(text))
    if vis_len > width:
        return truncate_visible(text, width)
    return text + (" " * (width - vis_len))


def clear_screen() -> None:
    """Clears terminal screen and returns cursor to origin."""
    sys.stdout.write("\033[2J\033[H")
    sys.stdout.flush()


def hide_cursor() -> None:
    """Hides the terminal cursor."""
    sys.stdout.write("\033[?25l")
    sys.stdout.flush()


def show_cursor() -> None:
    """Restores the terminal cursor."""
    sys.stdout.write("\033[?25h")
    sys.stdout.flush()


def enable_ansi_windows() -> None:
    """Enables virtual terminal processing on Windows consoles."""
    if sys.platform == "win32":
        os.system("")
        try:
            import ctypes
            kernel32 = ctypes.windll.kernel32
            h_out = kernel32.GetStdHandle(-11)
            mode = ctypes.c_ulong()
            if kernel32.GetConsoleMode(h_out, ctypes.byref(mode)):
                kernel32.SetConsoleMode(h_out, mode.value | 0x0004)
        except Exception:
            pass


class TerminalSession:
    """Context manager for raw terminal input and safe cleanup on exit."""

    def __init__(self):
        self.is_linux = sys.platform != "win32"
        self.old_term = None

    def __enter__(self):
        enable_ansi_windows()
        if self.is_linux and sys.stdin.isatty():
            try:
                import termios
                import tty
                self.old_term = termios.tcgetattr(sys.stdin)
                tty.setcbreak(sys.stdin.fileno())
            except Exception:
                pass
        hide_cursor()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        show_cursor()
        if self.is_linux and self.old_term is not None:
            try:
                import termios
                termios.tcsetattr(sys.stdin, termios.TCSADRAIN, self.old_term)
            except Exception:
                pass


def flush_input() -> None:
    """Flushes any pending characters from keyboard input buffer."""
    if sys.platform == "win32":
        try:
            import msvcrt
            while msvcrt.kbhit():
                msvcrt.getwch()
        except Exception:
            pass
    else:
        try:
            import termios
            termios.tcflush(sys.stdin, termios.TCIFLUSH)
        except Exception:
            pass


def read_key(timeout_sec: Optional[float] = None) -> Optional[str]:
    """Reads a single keypress cross-platform (Windows & Linux)."""
    if sys.platform == "win32":
        import msvcrt
        if timeout_sec is not None:
            if timeout_sec <= 0:
                if not msvcrt.kbhit():
                    return None
            else:
                start_t = time.time()
                while not msvcrt.kbhit():
                    if time.time() - start_t >= timeout_sec:
                        return None
                    time.sleep(0.01)

        ch = msvcrt.getwch()
        if ch in ("\x00", "\xe0"):
            ch2 = msvcrt.getwch()
            key_map = {
                "H": "UP",
                "P": "DOWN",
                "K": "LEFT",
                "M": "RIGHT",
                "G": "HOME",
                "O": "END",
            }
            return key_map.get(ch2, "SPECIAL")
        if ch in ("\r", "\n"):
            return "ENTER"
        if ch == " ":
            return "SPACE"
        if ch == "\x1b":
            return "ESC"
        if ch == "\t":
            return "TAB"
        if ch == "\x03":
            raise KeyboardInterrupt
        return ch
    else:
        import select
        if timeout_sec is not None:
            r, _, _ = select.select([sys.stdin], [], [], timeout_sec)
            if not r:
                return None
        ch = sys.stdin.read(1)
        if ch == "\x1b":
            r, _, _ = select.select([sys.stdin], [], [], 0.05)
            if not r:
                return "ESC"
            ch2 = sys.stdin.read(1)
            if ch2 == "[":
                ch3 = sys.stdin.read(1)
                key_map = {
                    "A": "UP",
                    "B": "DOWN",
                    "C": "RIGHT",
                    "D": "LEFT",
                    "H": "HOME",
                    "F": "END",
                }
                return key_map.get(ch3, "SPECIAL")
            return "ESC"
        if ch in ("\r", "\n"):
            return "ENTER"
        if ch == " ":
            return "SPACE"
        if ch == "\t":
            return "TAB"
        if ch == "\x03":
            raise KeyboardInterrupt
        return ch


def make_box(title: str, lines: List[str], width: int = 78, border_color: str = Colors.CYAN) -> str:
    """Draws a clean box with rounded corners and aligned border columns."""
    inner_w = width - 2
    c_rst = Colors.RESET
    c_bdr = border_color
    top_title = f"─ {title} " if title else ""
    top_bar = "╭" + top_title + ("─" * max(0, inner_w - len(top_title))) + "╮"
    bot_bar = "╰" + ("─" * inner_w) + "╯"

    out = [f"{c_bdr}{top_bar}{c_rst}"]
    for line in lines:
        padded = pad_line(line, inner_w)
        out.append(f"{c_bdr}│{c_rst}{padded}{c_bdr}│{c_rst}")
    out.append(f"{c_bdr}{bot_bar}{c_rst}")
    return "\n".join(out)


def progress_bar(pct: float, width: int = 24) -> str:
    """Generates a text progress bar."""
    clamped = max(0.0, min(100.0, pct))
    filled_len = int((clamped / 100.0) * width)
    empty_len = width - filled_len
    fill_str = "=" * filled_len
    empty_str = "-" * empty_len
    return f"[{Colors.BRIGHT_GREEN}{fill_str}{Colors.GRAY}{empty_str}{Colors.RESET}] {clamped:4.1f}%"


HARDCORE_CATEGORIES: List[str] = [
    "All", "Math & Algo", "Cognitive Traps", "Low-Level Systems", "Formal Logic", "Adversarial Syntax"
]
RUMMLU_CATEGORIES: List[str] = [
    "All", "STEM", "Humanities", "Social Sciences", "Russian Culture", "IT Edge Cases"
]
MMLU_EN_CATEGORIES: List[str] = [
    "All", "STEM", "Humanities", "Social Sciences", "Other", "IT Edge Cases"
]


@dataclass
class MenuState:
    models: List[str] = field(default_factory=list)
    selected_model_indices: Set[int] = field(default_factory=set)
    models_scroll_offset: int = 0
    max_visible_models: int = 4

    dataset_options: List[str] = field(default_factory=list)
    datasets: List[Any] = field(default_factory=list)
    dataset_idx: int = 0

    modes: List[str] = field(default_factory=lambda: [
        "MMLU Intelligence",
        "MoE Concurrency Sweep",
        "Full Pareto Suite",
    ])
    mode_index: int = 0

    all_categories: List[str] = field(default_factory=lambda: list(HARDCORE_CATEGORIES))
    selected_categories: Set[str] = field(default_factory=lambda: set(HARDCORE_CATEGORIES))

    ncmoe_options: List[str] = field(default_factory=lambda: [
        "auto", "none", "2", "4", "6", "8", "10", "12", "14", "16", "18", "20", "24", "26"
    ])
    ncmoe_idx: int = 0

    reasoning_options: List[str] = field(default_factory=lambda: ["off", "on"])
    reasoning_idx: int = 0

    mtp_options: List[str] = field(default_factory=lambda: ["off", "on"])
    mtp_idx: int = 0

    max_tokens_options: List[int] = field(default_factory=lambda: [128, 256, 512, 1024, 2048])
    max_tokens_idx: int = 1

    ctx_options: List[int] = field(default_factory=lambda: [4096, 8192, 16384, 32000])
    ctx_idx: int = 3

    focused_item: str = "model:0"
    status_message: Optional[str] = None
    detected_ncmoe_preview: Optional[str] = None

    def __post_init__(self) -> None:
        if self.datasets and not self.dataset_options:
            self.dataset_options = [d.name for d in self.datasets]
        elif not self.dataset_options and not self.datasets:
            self.dataset_options = [
                "Hardcore Stress (Traps & Logic, 100q)",
                "RuMMLU / MERA (Russian, 120q)",
                "English MMLU (120q)",
            ]


@dataclass
class LiveState:
    model_name: str = ""
    mode_name: str = ""
    ncmoe_label: str = ""
    active_flags: str = ""
    current_step: int = 0
    total_steps: int = 0
    progress_pct: float = 0.0
    accuracy: float = 0.0
    correct_count: int = 0
    total_evaluated: int = 0
    current_tps: float = 0.0
    mean_latency_ms: float = 0.0
    elapsed_sec: float = 0.0
    eta_sec: Optional[float] = None
    active_subject: str = ""
    active_category: str = ""
    active_question: str = ""
    last_status: str = ""
    recent_logs: List[str] = field(default_factory=list)


@dataclass
class CompletionState:
    mode_name: str = ""
    table_headers: List[str] = field(default_factory=list)
    table_rows: List[List[str]] = field(default_factory=list)
    category_breakdown: Dict[str, float] = field(default_factory=dict)
    md_path: Optional[Path] = None
    json_path: Optional[Path] = None
    dashboard_path: Optional[Path] = None
    notice_message: Optional[str] = None


def render_menu(state: MenuState, width: int = 78) -> str:
    """Renders the interactive configuration menu."""
    lines: List[str] = []
    c_bld = Colors.BOLD
    c_rst = Colors.RESET
    c_cyn = Colors.CYAN
    c_grn = Colors.GREEN
    c_gry = Colors.GRAY
    c_ylw = Colors.BRIGHT_YELLOW

    # Title header
    header_lines = [
        f" {c_bld}{Colors.BRIGHT_WHITE}SNARTA INTERACTIVE BENCHMARK RUNNER{c_rst}",
        f" {c_gry}Local LLM & MoE Concurrency Performance Suite{c_rst}",
    ]
    lines.append(make_box("", header_lines, width=width, border_color=Colors.CYAN))
    lines.append("")

    # Models section
    lines.append(f" {c_bld}[ MODELS TO BENCHMARK ]{c_rst} {c_gry}(Space to toggle, 'a' for all){c_rst}")
    if not state.models:
        lines.append(f"   {c_gry}(No .gguf models found in models/ or target/release/models/){c_rst}")
    else:
        vis_count = min(state.max_visible_models, len(state.models))
        start_idx = state.models_scroll_offset
        end_idx = min(len(state.models), start_idx + vis_count)

        if start_idx > 0:
            lines.append(f"   {c_gry}^ ({start_idx} more models above){c_rst}")

        for idx in range(start_idx, end_idx):
            m_name = state.models[idx]
            is_focused = (state.focused_item == f"model:{idx}")
            is_selected = (idx in state.selected_model_indices)
            box_sym = f"{c_grn}[x]{c_rst}" if is_selected else f"{c_gry}[ ]{c_rst}"

            if is_focused:
                pointer = f"{c_ylw}>{c_rst} "
                disp_name = f"{c_bld}{Colors.BRIGHT_WHITE}{m_name}{c_rst}"
                lines.append(f" {pointer}{box_sym} {disp_name}")
            else:
                lines.append(f"   {box_sym} {m_name}")

        if end_idx < len(state.models):
            rem = len(state.models) - end_idx
            lines.append(f"   {c_gry}v ({rem} more models below){c_rst}")

    lines.append("")

    # Dataset section
    lines.append(f" {c_bld}[ BENCHMARK DATASET ]{c_rst} {c_gry}(Left/Right or Space to change){c_rst}")
    ds_focused = (state.focused_item == "dataset")
    active_ds = state.dataset_options[state.dataset_idx]
    ds_ptr = f"{c_ylw}>{c_rst} " if ds_focused else "  "
    ds_disp = f"{c_bld}{Colors.BRIGHT_CYAN}< {active_ds} >{c_rst}" if ds_focused else f"< {active_ds} >"
    lines.append(f" {ds_ptr}Dataset: {ds_disp}")
    lines.append("")

    # Mode section
    lines.append(f" {c_bld}[ BENCHMARK MODE ]{c_rst} {c_gry}(Space or Left/Right to change){c_rst}")
    mode_focused = (state.focused_item == "mode")
    active_mode = state.modes[state.mode_index]
    mode_ptr = f"{c_ylw}>{c_rst} " if mode_focused else "  "
    mode_disp = f"{c_bld}{Colors.BRIGHT_CYAN}< {active_mode} >{c_rst}" if mode_focused else f"< {active_mode} >"
    lines.append(f" {mode_ptr}Mode: {mode_disp}")

    # Mode description
    mode_descs = {
        "MMLU Intelligence": "Measures academic reasoning accuracy across selected categories",
        "MoE Concurrency Sweep": "Tests -ncmoe concurrency levels (2..26) to find peak throughput",
        "Full Pareto Suite": "Plots multi-model accuracy vs generation speed Pareto frontier",
    }
    lines.append(f"    {c_gry}{mode_descs.get(active_mode, '')}{c_rst}")
    lines.append("")

    # Category section (only shown if mode is MMLU or Pareto)
    if state.mode_index != 1 and state.all_categories:
        lines.append(f" {c_bld}[ CATEGORIES ]{c_rst} {c_gry}(Space to toggle){c_rst}")

        def fmt_cat(c_name: str) -> str:
            is_foc = (state.focused_item == f"cat:{c_name}")
            is_sel = (c_name in state.selected_categories)
            bx = f"{c_grn}[x]{c_rst}" if is_sel else f"{c_gry}[ ]{c_rst}"
            if is_foc:
                return f"{c_ylw}>{bx} {c_bld}{Colors.BRIGHT_WHITE}{c_name}{c_rst}"
            return f" {bx} {c_name}"

        chunk_size = 3 if len(state.all_categories) > 4 else 2
        for i in range(0, len(state.all_categories), chunk_size):
            chunk = state.all_categories[i:i + chunk_size]
            lines.append("    " + "    ".join([fmt_cat(c) for c in chunk]))
        lines.append("")

    # Settings section
    lines.append(f" {c_bld}[ RUNTIME SETTINGS ]{c_rst} {c_gry}(Left/Right to adjust){c_rst}")

    def fmt_setting(name: str, val: str, item_id: str) -> str:
        is_foc = (state.focused_item == item_id)
        ptr = f"{c_ylw}>{c_rst} " if is_foc else "  "
        val_str = f"{c_bld}{Colors.BRIGHT_CYAN}< {val} >{c_rst}" if is_foc else f"< {val} >"
        return f" {ptr}{name:<22}: {val_str}"

    ncmoe_val = state.ncmoe_options[state.ncmoe_idx]
    if ncmoe_val == "auto" and state.detected_ncmoe_preview:
        ncmoe_disp = f"auto ({state.detected_ncmoe_preview})"
    else:
        ncmoe_disp = ncmoe_val
    reason_val = state.reasoning_options[state.reasoning_idx]
    tok_val = str(state.max_tokens_options[state.max_tokens_idx])
    ctx_val = str(state.ctx_options[state.ctx_idx])

    lines.append(fmt_setting("Concurrency (-ncmoe)", ncmoe_disp, "setting:ncmoe"))
    lines.append(fmt_setting("MTP Draft Layer", state.mtp_options[state.mtp_idx], "setting:mtp"))
    lines.append(fmt_setting("Reasoning Mode", reason_val, "setting:reasoning"))
    lines.append(fmt_setting("Max Output Tokens", tok_val, "setting:max_tokens"))
    lines.append(fmt_setting("Context Window Size", ctx_val, "setting:ctx"))
    lines.append("")

    # Action buttons
    start_focused = (state.focused_item == "action:start")
    exit_focused = (state.focused_item == "action:exit")

    if start_focused:
        btn_start = f"{Colors.BG_BLUE}{c_bld} [ START BENCHMARK ] {c_rst}"
    else:
        btn_start = f"{c_grn}[ START BENCHMARK ]{c_rst}"

    if exit_focused:
        btn_exit = f"{Colors.BG_DARK}{c_bld} [ EXIT ] {c_rst}"
    else:
        btn_exit = f"{c_gry}[ EXIT ]{c_rst}"

    lines.append(f"   {btn_start}          {btn_exit}")

    if state.status_message:
        lines.append("")
        lines.append(f" {c_bld}{c_ylw}[!] {state.status_message}{c_rst}")

    lines.append("")
    lines.append(f" {c_gry}[Up/Down] Navigate  [Left/Right] Adjust  [Space] Toggle  [Enter] Confirm  [q] Quit{c_rst}")
    return "\n".join(lines)


def render_live_view(state: LiveState, width: int = 78) -> str:
    """Renders the real-time execution progress screen."""
    lines: List[str] = []
    c_bld = Colors.BOLD
    c_rst = Colors.RESET
    c_gry = Colors.GRAY
    c_cyn = Colors.BRIGHT_CYAN

    # Header Box
    nc_part = f"{Colors.BRIGHT_GREEN}{c_bld}{state.ncmoe_label}{c_rst}"
    flags_part = f" | {c_gry}Flags:{c_rst} {c_cyn}{state.active_flags}{c_rst}" if state.active_flags else ""
    h_lines = [
        f" {c_bld}{Colors.BRIGHT_WHITE}SNARTA LIVE BENCHMARK EXECUTION VIEW{c_rst}",
        f" {c_gry}Model:{c_rst} {c_bld}{truncate_visible(state.model_name, width - 12)}{c_rst}",
        f" {c_gry}Mode :{c_rst} {state.mode_name} | {c_gry}ncmoe:{c_rst} {nc_part}{flags_part}",
    ]
    lines.append(make_box("", h_lines, width=width, border_color=Colors.CYAN))
    lines.append("")

    # Progress bar line
    step_str = f"({state.current_step}/{state.total_steps})"
    lines.append(f" Progress: {progress_bar(state.progress_pct, width=28)}  {step_str}")
    lines.append("")

    # Metrics summary
    acc_str = f"{state.accuracy:5.1f}% ({state.correct_count}/{state.total_evaluated})"
    tps_str = f"{state.current_tps:5.1f} t/s"
    lat_str = f"{state.mean_latency_ms:4.0f} ms"
    elap_min = int(state.elapsed_sec // 60)
    elap_sec = int(state.elapsed_sec % 60)
    time_str = f"{elap_min:02d}:{elap_sec:02d}"

    if state.eta_sec is not None and state.eta_sec > 0:
        eta_min = int(state.eta_sec // 60)
        eta_sec = int(state.eta_sec % 60)
        time_str += f" (ETA {eta_min:02d}:{eta_sec:02d})"

    m_row1 = f" {c_gry}Accuracy    :{c_rst} {c_bld}{acc_str:<18}{c_rst}  {c_gry}Generation Speed:{c_rst} {c_cyn}{tps_str}{c_rst}"
    m_row2 = f" {c_gry}Mean Latency:{c_rst} {lat_str:<18}  {c_gry}Elapsed Time    :{c_rst} {time_str}"
    lines.append(m_row1)
    lines.append(m_row2)
    lines.append("")

    # Active Item Box
    nc_info = f" | {c_gry}ncmoe:{c_rst} {c_bld}{Colors.BRIGHT_GREEN}{state.ncmoe_label}{c_rst}"
    active_lines = [
        f" {c_gry}Subject:{c_rst} {state.active_subject} | {c_gry}Category:{c_rst} {state.active_category}{nc_info}",
        f" {c_bld}Q:{c_rst} {truncate_visible(state.active_question, width - 8)}",
        "",
        f" {c_gry}Last Status:{c_rst} {state.last_status}",
    ]
    lines.append(make_box("Active Evaluation", active_lines, width=width, border_color=Colors.YELLOW))
    lines.append("")

    # Recent Event Log
    lines.append(f" {c_bld}Recent Logs:{c_rst}")
    show_logs = state.recent_logs[-4:] if state.recent_logs else [" [idle] Waiting for events..."]
    for l in show_logs:
        clean_l = truncate_visible(l, width - 6)
        lines.append(f"  {c_gry}{clean_l}{c_rst}")

    lines.append("")
    lines.append(f" {c_gry}Press [Esc] or [q] to abort execution{c_rst}")
    return "\n".join(lines)


def render_completion_view(state: CompletionState, width: int = 78) -> str:
    """Renders the benchmark completion summary screen."""
    lines: List[str] = []
    c_bld = Colors.BOLD
    c_rst = Colors.RESET
    c_grn = Colors.BRIGHT_GREEN
    c_cyn = Colors.BRIGHT_CYAN
    c_gry = Colors.GRAY

    # Header Box
    h_lines = [
        f" {c_bld}{c_grn}BENCHMARK COMPLETE - SUMMARY REPORT{c_rst}",
        f" {c_gry}Mode:{c_rst} {state.mode_name}",
    ]
    lines.append(make_box("", h_lines, width=width, border_color=Colors.GREEN))
    lines.append("")

    # Summary table
    if state.table_headers and state.table_rows:
        lines.append(f" {c_bld}Results Summary Table:{c_rst}")
        col_widths = [len(h) for h in state.table_headers]
        for row in state.table_rows:
            for c_idx, cell in enumerate(row):
                if c_idx < len(col_widths):
                    col_widths[c_idx] = max(col_widths[c_idx], len(strip_ansi(cell)))

        # Constrain first col if too wide
        max_first = width - sum(col_widths[1:]) - (len(col_widths) * 3) - 2
        if col_widths[0] > max_first and max_first > 15:
            col_widths[0] = max_first

        hdr_cells = [pad_line(f"{c_bld}{h}{c_rst}", col_widths[i]) for i, h in enumerate(state.table_headers)]
        lines.append("  " + " | ".join(hdr_cells))
        lines.append("  " + "-+-".join(["-" * w for w in col_widths]))

        for row in state.table_rows:
            r_cells = []
            for i, cell in enumerate(row):
                w = col_widths[i] if i < len(col_widths) else 10
                r_cells.append(pad_line(cell, w))
            lines.append("  " + " | ".join(r_cells))
        lines.append("")

    # Category Breakdown
    if state.category_breakdown:
        lines.append(f" {c_bld}Category Accuracy Breakdown:{c_rst}")
        parts = [f"{k}: {v:.1f}%" for k, v in state.category_breakdown.items()]
        lines.append("  " + "  |  ".join(parts))
        lines.append("")

    # Artifacts Box
    art_lines = [
        f" Markdown Report : {state.md_path if state.md_path else 'None'}",
        f" JSON Dataset    : {state.json_path if state.json_path else 'None'}",
        f" HTML Dashboard  : {c_cyn}{state.dashboard_path if state.dashboard_path else 'None'}{c_rst}",
    ]
    lines.append(make_box("Generated Artifacts", art_lines, width=width, border_color=Colors.BLUE))

    if state.notice_message:
        lines.append("")
        lines.append(f" {c_bld}{c_grn}[*] {state.notice_message}{c_rst}")

    lines.append("")
    lines.append(f" {c_bld}Actions:{c_rst}")
    lines.append(f"   {c_cyn}[O]{c_rst} Open HTML Dashboard in Browser")
    lines.append(f"   {c_cyn}[R]{c_rst} Return to Main Menu")
    lines.append(f"   {c_cyn}[Q]{c_rst} Exit")
    return "\n".join(lines)
