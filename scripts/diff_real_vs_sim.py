#!/usr/bin/env python3
"""Diff a real-client snapshot against a freshly-seeded simulator snapshot.

Initial-state schema alignment check (Phase P2, level 1).

  python scripts/diff_real_vs_sim.py \
      --real results/real-client-trajectories/observer-20260420T223706/snapshots/tick-000010.json \
      --sim-seed 42 --sim-stake 1 \
      --report results/sim-vs-real-gap-report.md

The real BalatroBot `gamestate` shape is treated as the canonical target.
This script normalizes the sim's snapshot into the same shape via
`env.state_mapping.to_real_shape` and emits a categorized markdown report.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import balatro_native  # type: ignore

from env.state_mapping import (
    ALIGNED,
    MISSING_IN_REAL,
    MISSING_IN_SIM,
    SHAPE_MISMATCH,
    VALUE_MISMATCH,
    diff_shapes,
    to_real_shape,
)


def load_real(path: Path) -> dict:
    return json.loads(path.read_text())


def build_sim_snapshot(
    seed: int,
    stake: int,
    advance_to_blind: bool = False,
) -> dict:
    eng = balatro_native.Engine(seed=seed, stake=stake)
    snap = eng.snapshot()
    d = json.loads(snap.to_json())
    return d


def classify(rows: list[dict]) -> Counter:
    return Counter(r["status"] for r in rows)


def render_report(
    rows: list[dict],
    real_path: Path,
    sim_info: dict,
) -> str:
    stats = classify(rows)
    total = len(rows)
    lines: list[str] = []
    lines.append("# Simulator vs Real-Client — Schema Alignment Report")
    lines.append("")
    lines.append(f"- **real source**: `{real_path}`")
    lines.append(f"- **sim build**: seed={sim_info['seed']} stake={sim_info['stake']}")
    lines.append(f"- **total fields compared**: {total}")
    lines.append("")
    lines.append("## Summary")
    lines.append("")
    lines.append("| Status | Count | % |")
    lines.append("|---|---:|---:|")
    for k in (ALIGNED, VALUE_MISMATCH, MISSING_IN_SIM, MISSING_IN_REAL, SHAPE_MISMATCH):
        n = stats.get(k, 0)
        pct = 100.0 * n / total if total else 0.0
        lines.append(f"| `{k}` | {n} | {pct:.1f}% |")
    lines.append("")
    # mismatch details
    for label, key in (
        ("Value mismatches (same field, different value)", VALUE_MISMATCH),
        ("Missing in simulator (real has it, sim doesn't)", MISSING_IN_SIM),
        ("Missing in real (sim-only extension)", MISSING_IN_REAL),
        ("Shape mismatches (type differs)", SHAPE_MISMATCH),
    ):
        bucket = [r for r in rows if r["status"] == key]
        lines.append(f"## {label} — {len(bucket)}")
        lines.append("")
        if not bucket:
            lines.append("_none_")
            lines.append("")
            continue
        lines.append("| Path | Real | Sim |")
        lines.append("|---|---|---|")
        for r in bucket:
            path = r["path"] or "(root)"
            lines.append(f"| `{path}` | `{r['real']}` | `{r['sim']}` |")
        lines.append("")
    lines.append("## Aligned (for reference)")
    lines.append("")
    aligned = [r for r in rows if r["status"] == ALIGNED]
    for r in aligned[:40]:
        path = r["path"] or "(root)"
        lines.append(f"- `{path}`")
    if len(aligned) > 40:
        lines.append(f"- … and {len(aligned) - 40} more")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--real", type=Path, required=True, help="real-client snapshot JSON")
    p.add_argument("--sim-seed", type=int, default=42)
    p.add_argument("--sim-stake", type=int, default=1)
    p.add_argument("--report", type=Path, default=Path("results/sim-vs-real-gap-report.md"))
    args = p.parse_args()

    real = load_real(args.real)
    sim_raw = build_sim_snapshot(args.sim_seed, args.sim_stake)
    sim_norm = to_real_shape(
        sim_raw,
        seed=real.get("seed"),     # pass real's seed so that field compares equal by intent
        deck_name=real.get("deck"),  # same
    )

    rows = diff_shapes(real, sim_norm)

    report = render_report(
        rows,
        real_path=args.real,
        sim_info={"seed": args.sim_seed, "stake": args.sim_stake},
    )
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(report)

    stats = classify(rows)
    total = len(rows)
    print(f"wrote {args.report}")
    print(f"  total fields: {total}")
    for k in (ALIGNED, VALUE_MISMATCH, MISSING_IN_SIM, MISSING_IN_REAL, SHAPE_MISMATCH):
        n = stats.get(k, 0)
        print(f"    {k:20s} {n:4d}  {100.0*n/total if total else 0:.1f}%")
    return 0


if __name__ == "__main__":
    sys.exit(main())
