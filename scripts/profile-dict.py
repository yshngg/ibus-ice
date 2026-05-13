#!/usr/bin/env python3
"""Visualize dict-compiler profiling output.

Usage:
    DICT_PROFILE=1 dict-compiler ... 2>/tmp/perf.jsonl 1>&2
    python scripts/profile-dict.py /tmp/perf.jsonl [output_prefix]

Generates:
    <prefix>_timeline.png    — phase timeline + trie growth over time
    <prefix>_find_base.png   — cursor position and attempt histogram
    <prefix>_memory.png      — RSS memory over time
"""

import json
import sys
from pathlib import Path


def load_data(path):
    events = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    events.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    return events


def analyze(events):
    phases = {}
    progress = []
    find_bases = []
    memory = []
    final = None

    prev_cat = None
    for e in events:
        cat = e["cat"]
        if cat == "phase":
            phases[e["name"]] = e["ts"]
        elif cat == "progress":
            progress.append((e["ts"], e["inserted"], e["total"], e["base_len"]))
        elif cat == "find_base":
            find_bases.append((e["ts"], e["cursor"], e["base_len"], e["attempts"]))
        elif cat == "memory":
            memory.append((e["ts"], e["rss_mb"]))
        elif cat == "final":
            final = e
    return phases, progress, find_bases, memory, final


def plot_timeline(phases, progress, memory, output_path):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("matplotlib not installed. Install with: pip install matplotlib")
        return

    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(12, 8), sharex=True)

    # Phase timeline
    colors = {"parse_start": "#4caf50", "build_start": "#2196f3",
              "serialize_start": "#ff9800"}
    labels = {"parse_start": "Parse", "build_start": "Build",
              "serialize_start": "Serialize"}
    y_pos = 0
    for name, ts in sorted(phases.items(), key=lambda x: x[1]):
        if name in colors:
            ax1.barh(y_pos, 0.5, left=ts / 1000, height=0.6,
                     color=colors[name], label=labels.get(name, name))
    ax1.set_yticks([])
    ax1.set_xlabel("Time (seconds)")
    ax1.set_title("Phase Timeline")
    ax1.legend(loc="upper right")

    # Trie growth (base_len)
    if progress:
        ts_vals = [p[0] / 1000 for p in progress]
        pct_vals = [p[1] / p[2] * 100 for p in progress]
        base_vals = [p[3] for p in progress]
        ax2.plot(ts_vals, base_vals, "b-", alpha=0.7, label="Trie nodes")
        ax2.set_ylabel("base.len()")
        ax2.yaxis.set_major_formatter(
            plt.FuncFormatter(lambda x, _: f"{x/1e6:.1f}M" if x > 1e6 else f"{x/1e3:.0f}K"))
        ax2.set_title("Trie Size Growth")
        ax2.legend(loc="upper left")
        ax2.set_xlabel("Time (seconds)")

    plt.tight_layout()
    plt.savefig(output_path, dpi=100)
    plt.close()
    print(f"  Saved {output_path}")


def plot_find_base(find_bases, output_path):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        return

    if not find_bases:
        return

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(12, 4))

    ts_vals = [f[0] / 1000 for f in find_bases]
    cursor_vals = [f[1] for f in find_bases]
    attempts = [f[3] for f in find_bases]

    # Cursor position over time
    ax1.scatter(ts_vals, cursor_vals, s=2, alpha=0.5, c=attempts, cmap="YlOrRd")
    ax1.set_xlabel("Time (seconds)")
    ax1.set_ylabel("Cursor position")
    ax1.set_title("find_base: Cursor Position (color=attempts)")

    # Attempt histogram
    ax2.hist(attempts, bins=50, color="#2196f3", alpha=0.8)
    ax2.set_xlabel("Attempts to find free slot")
    ax2.set_ylabel("Count")
    ax2.set_title(f"find_base: Attempt Distribution (n={len(find_bases)})")

    # Stats
    avg = sum(attempts) / len(attempts)
    p99 = sorted(attempts)[int(len(attempts) * 0.99)]
    ax2.axvline(avg, color="red", linestyle="--", label=f"avg={avg:.1f}")
    ax2.axvline(p99, color="orange", linestyle=":", label=f"p99={p99}")
    ax2.legend()

    plt.tight_layout()
    plt.savefig(output_path, dpi=100)
    plt.close()
    print(f"  Saved {output_path}")


def plot_memory(memory, output_path):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        return

    if not memory:
        return

    fig, ax = plt.subplots(figsize=(12, 4))
    ts_vals = [m[0] / 1000 for m in memory]
    rss_vals = [m[1] for m in memory]
    ax.fill_between(ts_vals, rss_vals, alpha=0.3, color="#e91e63")
    ax.plot(ts_vals, rss_vals, color="#e91e63", linewidth=1)
    ax.set_xlabel("Time (seconds)")
    ax.set_ylabel("RSS (MB)")
    ax.set_title("Memory Usage Over Time")
    ax.yaxis.set_major_formatter(
        plt.FuncFormatter(lambda x, _: f"{x/1024:.1f}GB" if x > 1024 else f"{x:.0f}MB"))

    plt.tight_layout()
    plt.savefig(output_path, dpi=100)
    plt.close()
    print(f"  Saved {output_path}")


def print_summary(phases, find_bases, memory, final):
    print()
    print("=== Performance Summary ===")
    if final:
        print(f"  Total time:     {final['elapsed_ms']/1000:.1f}s")
        print(f"  Trie nodes:     {final['trie_nodes']:,}")
        print(f"  Entries:        {final['entries']:,}")
        print(f"  Peak RSS:       {final['rss_mb']} MB")

    # Phase durations
    phases_sorted = sorted(phases.items(), key=lambda x: x[1])
    for i in range(0, len(phases_sorted) - 1, 2):
        name1, ts1 = phases_sorted[i]
        name2, ts2 = phases_sorted[i + 1] if i + 1 < len(phases_sorted) else (None, None)
        if name2 and name1.replace("_start", "") == name2.replace("_end", ""):
            duration = ts2 - ts1
            print(f"  {name1.replace('_start', ''):12s}: {duration/1000:6.1f}s")

    if find_bases:
        attempts = [f[3] for f in find_bases]
        avg = sum(attempts) / len(attempts)
        print(f"  find_base calls:{len(find_bases):6d}")
        print(f"  avg attempts:   {avg:6.1f}")
        print(f"  max attempts:   {max(attempts):6d}")


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    jsonl_path = sys.argv[1]
    prefix = sys.argv[2] if len(sys.argv) > 2 else "profile"

    events = load_data(jsonl_path)
    if not events:
        print(f"No events found in {jsonl_path}")
        sys.exit(1)

    phases, progress, find_bases, memory, final = analyze(events)

    plot_timeline(phases, progress, memory, f"{prefix}_timeline.png")
    plot_find_base(find_bases, f"{prefix}_find_base.png")
    plot_memory(memory, f"{prefix}_memory.png")
    print_summary(phases, find_bases, memory, final)


if __name__ == "__main__":
    main()
