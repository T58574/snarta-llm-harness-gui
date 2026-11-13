#!/usr/bin/env python3
"""
Snarta MMLU Plot & Dashboard Generator

Generates interactive standalone HTML/SVG dashboards and optional high-resolution
PNG charts from MMLU benchmark results.

Zero external dependencies required for HTML/SVG visualization (pure Python).
Optionally utilizes matplotlib if installed for PNG exports.
"""

import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional


def render_svg_bar_chart(
    labels: List[str],
    values: List[float],
    title: str,
    y_unit: str = "%",
    width: int = 680,
    height: int = 320,
    color: str = "#6366f1",
) -> str:
    """Renders a responsive standalone SVG bar chart."""
    pad_left = 60
    pad_right = 30
    pad_top = 50
    pad_bottom = 60
    chart_w = width - pad_left - pad_right
    chart_h = height - pad_top - pad_bottom

    max_val = max(values) if values else 100.0
    ceil_val = min(100.0, ((max_val // 10) + 2) * 10) if y_unit == "%" else max_val * 1.2
    if ceil_val <= 0:
        ceil_val = 100.0

    n_bars = max(1, len(labels))
    slot_w = chart_w / n_bars
    bar_w = min(60.0, slot_w * 0.55)

    svg: List[str] = [
        f'<svg viewBox="0 0 {width} {height}" class="w-full h-auto" xmlns="http://www.w3.org/2000/svg">',
        f'<text x="{width / 2:.1f}" y="28" text-anchor="middle" fill="#f8fafc" font-size="16" font-weight="600">{title}</text>',
    ]

    # Gridlines and Y-axis marks
    y_steps = 5
    for i in range(y_steps + 1):
        step_val = (ceil_val / y_steps) * i
        y_pos = pad_top + chart_h - (step_val / ceil_val) * chart_h
        svg.append(
            f'<line x1="{pad_left}" y1="{y_pos:.1f}" x2="{width - pad_right}" y2="{y_pos:.1f}" stroke="#334155" stroke-dasharray="3 3" stroke-width="1" />'
        )
        svg.append(
            f'<text x="{pad_left - 10}" y="{y_pos + 4:.1f}" text-anchor="end" fill="#94a3b8" font-size="11">{step_val:.0f}{y_unit}</text>'
        )

    # Bars and data labels
    for idx, (lbl, val) in enumerate(zip(labels, values)):
        center_x = pad_left + idx * slot_w + (slot_w / 2.0)
        bar_x = center_x - (bar_w / 2.0)
        bar_h = (val / ceil_val) * chart_h
        bar_y = pad_top + chart_h - bar_h

        # Bar rect
        svg.append(
            f'<rect x="{bar_x:.1f}" y="{bar_y:.1f}" width="{bar_w:.1f}" height="{bar_h:.1f}" rx="6" fill="{color}" opacity="0.9">'
            f'<title>{lbl}: {val:.2f}{y_unit}</title>'
            f'</rect>'
        )
        # Value text above bar
        svg.append(
            f'<text x="{center_x:.1f}" y="{bar_y - 8:.1f}" text-anchor="middle" fill="#f8fafc" font-size="12" font-weight="bold">{val:.1f}{y_unit}</text>'
        )
        # X label below
        short_lbl = lbl if len(lbl) <= 16 else lbl[:14] + ".."
        svg.append(
            f'<text x="{center_x:.1f}" y="{pad_top + chart_h + 24:.1f}" text-anchor="middle" fill="#cbd5e1" font-size="12">{short_lbl}</text>'
        )

    svg.append('</svg>')
    return "\n".join(svg)


def render_svg_grouped_bar_chart(
    models: List[str],
    categories: List[str],
    cat_data: Dict[str, List[float]],
    width: int = 760,
    height: int = 360,
) -> str:
    """Renders a grouped SVG bar chart comparing categories across multiple models."""
    colors = ["#38bdf8", "#818cf8", "#34d399", "#fbbf24", "#f472b6"]
    pad_left = 60
    pad_right = 30
    pad_top = 60
    pad_bottom = 70
    chart_w = width - pad_left - pad_right
    chart_h = height - pad_top - pad_bottom

    n_groups = max(1, len(categories))
    group_w = chart_w / n_groups
    n_models = max(1, len(models))
    bar_w = min(22.0, (group_w * 0.75) / n_models)

    svg: List[str] = [
        f'<svg viewBox="0 0 {width} {height}" class="w-full h-auto" xmlns="http://www.w3.org/2000/svg">',
        f'<text x="{width / 2:.1f}" y="28" text-anchor="middle" fill="#f8fafc" font-size="16" font-weight="600">Accuracy by Domain Category</text>',
    ]

    # Legend at top right
    leg_x = pad_left
    leg_y = 44
    for idx, m_lbl in enumerate(models):
        c = colors[idx % len(colors)]
        short_m = m_lbl.split("-")[-1] if "-" in m_lbl else m_lbl
        svg.append(f'<rect x="{leg_x}" y="{leg_y - 9}" width="12" height="12" rx="2" fill="{c}" />')
        svg.append(f'<text x="{leg_x + 16}" y="{leg_y + 1}" fill="#cbd5e1" font-size="11">{short_m}</text>')
        leg_x += len(short_m) * 8 + 40

    # Gridlines
    for step in range(0, 101, 25):
        y_pos = pad_top + chart_h - (step / 100.0) * chart_h
        svg.append(
            f'<line x1="{pad_left}" y1="{y_pos:.1f}" x2="{width - pad_right}" y2="{y_pos:.1f}" stroke="#334155" stroke-dasharray="3 3" stroke-width="1" />'
        )
        svg.append(
            f'<text x="{pad_left - 10}" y="{y_pos + 4:.1f}" text-anchor="end" fill="#94a3b8" font-size="11">{step}%</text>'
        )

    # Groups & Bars
    for g_idx, cat in enumerate(categories):
        group_center = pad_left + g_idx * group_w + (group_w / 2.0)
        group_start_x = group_center - ((n_models * bar_w) / 2.0)
        vals = cat_data.get(cat, [0.0] * n_models)

        for m_idx in range(n_models):
            val = vals[m_idx] if m_idx < len(vals) else 0.0
            bx = group_start_x + m_idx * bar_w
            bh = (val / 100.0) * chart_h
            by = pad_top + chart_h - bh
            c = colors[m_idx % len(colors)]

            svg.append(
                f'<rect x="{bx:.1f}" y="{by:.1f}" width="{bar_w - 2:.1f}" height="{bh:.1f}" rx="3" fill="{c}" opacity="0.9">'
                f'<title>{models[m_idx]} - {cat}: {val:.1f}%</title>'
                f'</rect>'
            )

        # Category label below
        svg.append(
            f'<text x="{group_center:.1f}" y="{pad_top + chart_h + 25:.1f}" text-anchor="middle" fill="#f8fafc" font-size="12" font-weight="500">{cat}</text>'
        )

    svg.append('</svg>')
    return "\n".join(svg)


def render_svg_scatter_pareto(
    models: List[str],
    tps_list: List[float],
    acc_list: List[float],
    width: int = 680,
    height: int = 340,
) -> str:
    """Renders a scatter plot showing Accuracy vs Generation Speed (TPS) Pareto frontier."""
    pad_left = 70
    pad_right = 40
    pad_top = 50
    pad_bottom = 60
    chart_w = width - pad_left - pad_right
    chart_h = height - pad_top - pad_bottom

    min_tps = max(0.0, min(tps_list) * 0.8) if tps_list else 0.0
    max_tps = max(tps_list) * 1.15 if tps_list else 60.0
    min_acc = max(0.0, min(acc_list) - 10.0) if acc_list else 0.0
    max_acc = min(100.0, max(acc_list) + 5.0) if acc_list else 100.0

    if max_tps <= min_tps:
        max_tps = min_tps + 10.0
    if max_acc <= min_acc:
        max_acc = min_acc + 10.0

    svg: List[str] = [
        f'<svg viewBox="0 0 {width} {height}" class="w-full h-auto" xmlns="http://www.w3.org/2000/svg">',
        f'<text x="{width / 2:.1f}" y="28" text-anchor="middle" fill="#f8fafc" font-size="16" font-weight="600">Pareto Frontier: Accuracy vs Generation Speed</text>',
        f'<text x="{width / 2:.1f}" y="{height - 12}" text-anchor="middle" fill="#94a3b8" font-size="12">Generation Speed (eval tokens/sec) →</text>',
        f'<text x="20" y="{height / 2:.1f}" text-anchor="middle" transform="rotate(-90 20 {height/2:.1f})" fill="#94a3b8" font-size="12">MMLU Accuracy (%) →</text>',
    ]

    # Gridlines
    for i in range(5):
        # Y lines
        y_val = min_acc + (max_acc - min_acc) * (i / 4.0)
        y_pos = pad_top + chart_h - (i / 4.0) * chart_h
        svg.append(f'<line x1="{pad_left}" y1="{y_pos:.1f}" x2="{width - pad_right}" y2="{y_pos:.1f}" stroke="#334155" stroke-dasharray="2 2" stroke-width="1" />')
        svg.append(f'<text x="{pad_left - 10}" y="{y_pos + 4:.1f}" text-anchor="end" fill="#94a3b8" font-size="10">{y_val:.1f}%</text>')

        # X lines
        x_val = min_tps + (max_tps - min_tps) * (i / 4.0)
        x_pos = pad_left + (i / 4.0) * chart_w
        svg.append(f'<line x1="{x_pos:.1f}" y1="{pad_top}" x2="{x_pos:.1f}" y2="{pad_top + chart_h}" stroke="#334155" stroke-dasharray="2 2" stroke-width="1" />')
        svg.append(f'<text x="{x_pos:.1f}" y="{pad_top + chart_h + 18:.1f}" text-anchor="middle" fill="#94a3b8" font-size="10">{x_val:.1f}</text>')

    # Points & Labels
    colors = ["#38bdf8", "#818cf8", "#34d399", "#fbbf24", "#f472b6"]
    for idx, (m, tps, acc) in enumerate(zip(models, tps_list, acc_list)):
        px = pad_left + ((tps - min_tps) / (max_tps - min_tps)) * chart_w
        py = pad_top + chart_h - ((acc - min_acc) / (max_acc - min_acc)) * chart_h
        c = colors[idx % len(colors)]

        short_m = m.split("-")[-1] if "-" in m else m
        svg.append(
            f'<circle cx="{px:.1f}" cy="{py:.1f}" r="7" fill="{c}" stroke="#ffffff" stroke-width="2">'
            f'<title>{m}: {acc:.1f}% @ {tps:.1f} t/s</title>'
            f'</circle>'
        )
        svg.append(
            f'<text x="{px + 10:.1f}" y="{py - 8:.1f}" fill="{c}" font-size="12" font-weight="bold">{short_m} ({acc:.1f}%)</text>'
        )

    svg.append('</svg>')
    return "\n".join(svg)


def generate_html_dashboard(
    report_data: Dict[str, Any],
    output_html_path: Path,
) -> None:
    """Generates a standalone, dark-themed HTML/SVG dashboard from MMLU benchmark results."""
    models_data = report_data.get("models", [])
    if not models_data:
        return

    model_names: List[str] = [m.get("model_name", "Unknown") for m in models_data]
    accuracies: List[float] = [float(m.get("overall_accuracy", 0.0)) for m in models_data]
    tps_values: List[float] = [float(m.get("mean_tps", 0.0)) for m in models_data]
    load_times: List[float] = [float(m.get("load_time_sec", 0.0)) for m in models_data]

    # Find winners
    highest_acc_idx = max(range(len(accuracies)), key=lambda i: accuracies[i]) if accuracies else 0
    fastest_tps_idx = max(range(len(tps_values)), key=lambda i: tps_values[i]) if tps_values else 0

    # Collect category metrics dynamically
    base_order = ["STEM", "Humanities", "Social Sciences", "Other", "IT Edge Cases"]
    discovered: List[str] = []
    for m in models_data:
        for c in m.get("category_accuracy", {}).keys():
            if c not in discovered:
                discovered.append(c)
    all_cats = [c for c in base_order if c in discovered] + [c for c in discovered if c not in base_order]
    if not all_cats:
        all_cats = ["STEM", "Humanities", "Social Sciences", "Other"]

    cat_data: Dict[str, List[float]] = {c: [] for c in all_cats}
    for m in models_data:
        m_cats = m.get("category_accuracy", {})
        for c in all_cats:
            cat_data[c].append(float(m_cats.get(c, 0.0)))

    # Render SVGs
    svg_acc_bar = render_svg_bar_chart(model_names, accuracies, "Overall MMLU Accuracy by Model", y_unit="%")
    svg_cat_grouped = render_svg_grouped_bar_chart(model_names, all_cats, cat_data)
    svg_pareto = render_svg_scatter_pareto(model_names, tps_values, accuracies)

    # Build Table Rows
    table_rows: List[str] = []
    base_acc = accuracies[0] if accuracies else 0.0
    for idx, m in enumerate(models_data):
        name = m.get("model_name", "")
        acc = float(m.get("overall_accuracy", 0.0))
        tps = float(m.get("mean_tps", 0.0))
        load_s = float(m.get("load_time_sec", 0.0))
        ncmoe_val = m.get("ncmoe", "default")
        delta = acc - base_acc
        delta_str = f"+{delta:.2f}%" if delta > 0 else f"{delta:.2f}%" if delta < 0 else "baseline"

        badge = ""
        if idx == highest_acc_idx:
            badge = '<span class="px-2 py-1 text-xs font-semibold rounded bg-indigo-500/20 text-indigo-400 border border-indigo-500/30">Intelligence Leader</span>'
        elif idx == fastest_tps_idx:
            badge = '<span class="px-2 py-1 text-xs font-semibold rounded bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">Speed Leader</span>'

        row_html = f"""
        <tr class="border-b border-slate-800 hover:bg-slate-800/50 transition">
            <td class="py-3 px-4 font-medium text-slate-200">{name}</td>
            <td class="py-3 px-4 text-slate-300 font-mono">-ncmoe {ncmoe_val}</td>
            <td class="py-3 px-4 font-bold text-indigo-400 font-mono text-base">{acc:.2f}%</td>
            <td class="py-3 px-4 font-mono text-emerald-400">{tps:.1f} t/s</td>
            <td class="py-3 px-4 text-slate-400 font-mono">{load_s:.1f}s</td>
            <td class="py-3 px-4 font-mono text-slate-300">{delta_str}</td>
            <td class="py-3 px-4">{badge}</td>
        </tr>
        """
        table_rows.append(row_html)

    table_body = "\n".join(table_rows)

    html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Snarta MMLU Benchmark Dashboard</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <style>
        body {{ background-color: #0b0f19; font-family: ui-sans-serif, system-ui, -apple-system, sans-serif; }}
        .card {{ background-color: #131c2e; border: 1px solid #1e293b; border-radius: 0.75rem; }}
    </style>
</head>
<body class="text-slate-100 min-h-screen p-6 md:p-10">
    <div class="max-w-7xl mx-auto space-y-8">
        
        <!-- Header -->
        <div class="flex flex-col md:flex-row md:items-center justify-between border-b border-slate-800 pb-6 gap-4">
            <div>
                <h1 class="text-2xl md:text-3xl font-bold tracking-tight text-white flex items-center gap-3">
                    Snarta MMLU Intelligence Benchmark
                </h1>
                <p class="text-slate-400 text-sm mt-1">
                    Multi-domain academic evaluation & quant efficiency Pareto analysis
                </p>
            </div>
            <div class="text-xs text-slate-500 font-mono bg-slate-900 px-3 py-2 rounded-lg border border-slate-800">
                Timestamp: {report_data.get("timestamp", "N/A")} | Evaluated Questions: {report_data.get("questions_evaluated", "N/A")}
            </div>
        </div>

        <!-- Metrics Overview Grid -->
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-5">
            <div class="card p-5">
                <div class="text-xs font-medium text-slate-400 uppercase tracking-wider">Top Intelligence</div>
                <div class="text-2xl font-bold text-indigo-400 mt-2">{model_names[highest_acc_idx]}</div>
                <div class="text-sm text-slate-400 mt-1 font-mono">{accuracies[highest_acc_idx]:.2f}% overall accuracy</div>
            </div>
            <div class="card p-5">
                <div class="text-xs font-medium text-slate-400 uppercase tracking-wider">Top Generation Speed</div>
                <div class="text-2xl font-bold text-emerald-400 mt-2">{model_names[fastest_tps_idx]}</div>
                <div class="text-sm text-slate-400 mt-1 font-mono">{tps_values[fastest_tps_idx]:.1f} eval tokens/sec</div>
            </div>
            <div class="card p-5">
                <div class="text-xs font-medium text-slate-400 uppercase tracking-wider">Evaluated Models</div>
                <div class="text-2xl font-bold text-white mt-2">{len(model_names)}</div>
                <div class="text-sm text-slate-400 mt-1">Quantizations compared</div>
            </div>
            <div class="card p-5">
                <div class="text-xs font-medium text-slate-400 uppercase tracking-wider">Context & Framework</div>
                <div class="text-2xl font-bold text-sky-400 mt-2">Zero-Shot MMLU</div>
                <div class="text-sm text-slate-400 mt-1">4 Academic Disciplines</div>
            </div>
        </div>

        <!-- Charts Grid -->
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
            <!-- Accuracy Bar Chart -->
            <div class="card p-6">
                {svg_acc_bar}
            </div>

            <!-- Pareto Scatter Chart -->
            <div class="card p-6">
                {svg_pareto}
            </div>
        </div>

        <!-- Domain Categories Grouped Chart -->
        <div class="card p-6">
            {svg_cat_grouped}
        </div>

        <!-- Summary Table -->
        <div class="card p-6 overflow-x-auto">
            <h3 class="text-lg font-semibold text-white mb-4">Benchmark Comparison Table</h3>
            <table class="w-full text-left text-sm border-collapse">
                <thead>
                    <tr class="border-b border-slate-800 text-slate-400 uppercase text-xs">
                        <th class="py-3 px-4">Model</th>
                        <th class="py-3 px-4">Concurrency</th>
                        <th class="py-3 px-4">MMLU Accuracy</th>
                        <th class="py-3 px-4">Eval Speed</th>
                        <th class="py-3 px-4">Cold Load</th>
                        <th class="py-3 px-4">Accuracy Δ</th>
                        <th class="py-3 px-4">Status</th>
                    </tr>
                </thead>
                <tbody>
                    {table_body}
                </tbody>
            </table>
        </div>

        <div class="text-center text-xs text-slate-600 pt-4">
            Generated automatically by Snarta MoE Benchmark Suite. Zero local server dependencies.
        </div>

    </div>
</body>
</html>
"""
    output_html_path.parent.mkdir(parents=True, exist_ok=True)
    output_html_path.write_text(html_content, encoding="utf-8")


def try_generate_matplotlib_charts(
    report_data: Dict[str, Any],
    output_png_path: Path,
) -> bool:
    """Optionally exports high-resolution PNG charts if matplotlib is installed."""
    try:
        import matplotlib.pyplot as plt
    except ImportError:
        return False

    models_data = report_data.get("models", [])
    if not models_data:
        return False

    names = [m.get("model_name", "") for m in models_data]
    accs = [float(m.get("overall_accuracy", 0.0)) for m in models_data]
    tps = [float(m.get("mean_tps", 0.0)) for m in models_data]

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(14, 5), dpi=200)

    # Bar chart
    bars = ax1.bar(names, accs, color="#6366f1", width=0.55)
    ax1.set_title("MMLU Accuracy (%)", fontsize=13, fontweight="bold")
    ax1.set_ylim(0, 100)
    ax1.set_ylabel("Accuracy %")
    for bar in bars:
        h = bar.get_height()
        ax1.annotate(f"{h:.1f}%", (bar.get_x() + bar.get_width() / 2, h + 1), ha="center", fontsize=10)

    # Pareto Scatter
    ax2.scatter(tps, accs, s=120, c="#34d399", edgecolors="#0f172a", zorder=3)
    ax2.set_title("Pareto: Accuracy vs Generation Speed", fontsize=13, fontweight="bold")
    ax2.set_xlabel("Eval TPS (tokens/sec)")
    ax2.set_ylabel("Accuracy %")
    ax2.grid(True, linestyle="--", alpha=0.4)
    for name, x, y in zip(names, tps, accs):
        ax2.annotate(name, (x + 0.5, y + 0.3), fontsize=10)

    plt.tight_layout()
    output_png_path.parent.mkdir(parents=True, exist_ok=True)
    plt.savefig(str(output_png_path))
    plt.close()
    return True
