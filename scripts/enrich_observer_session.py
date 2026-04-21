#!/usr/bin/env python3
"""Post-hoc enrichment: merge per-blind detail (name / target score / tag / boss effect)
from snapshot dumps into an observer session's events.jsonl.

The observer's event summaries don't always carry full blind metadata (older
versions only stored status flags). Snapshots are a full gamestate dump every
N ticks — this script pulls the missing fields from the *nearest* snapshot for
each event and writes `events.enriched.jsonl` alongside.

    python scripts/enrich_observer_session.py \
        --session results/real-client-trajectories/round2

Produces:
    results/real-client-trajectories/<session>/events.enriched.jsonl

Each line is the original event with one extra key:
    "blinds_detail": {
        "small": {status, name, score, effect, tag_name, tag_effect},
        "big":   {...},
        "boss":  {...}
    }
"""
from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime
from pathlib import Path
from typing import Any


def parse_ts(s: str) -> datetime | None:
    try:
        return datetime.fromisoformat(s)
    except Exception:  # noqa: BLE001
        return None


def load_snapshots(snap_dir: Path) -> list[tuple[int, Path, dict[str, Any]]]:
    """Return [(tick_number, path, parsed_json), ...] sorted by tick."""
    items: list[tuple[int, Path, dict[str, Any]]] = []
    if not snap_dir.exists():
        return items
    for p in sorted(snap_dir.glob("tick-*.json")):
        try:
            tick = int(p.stem.split("-")[1])
        except Exception:  # noqa: BLE001
            continue
        try:
            data = json.loads(p.read_text())
        except Exception:  # noqa: BLE001
            continue
        items.append((tick, p, data))
    return items


def blind_detail(raw_blinds: Any, key: str) -> dict[str, Any]:
    if not isinstance(raw_blinds, dict):
        return {}
    b = raw_blinds.get(key)
    if not isinstance(b, dict):
        return {}
    return {
        "status": b.get("status"),
        "name": b.get("name"),
        "score": b.get("score"),
        "effect": b.get("effect") or "",
        "tag_name": b.get("tag_name") or "",
        "tag_effect": b.get("tag_effect") or "",
    }


def snapshot_to_blinds_detail(snap: dict[str, Any]) -> dict[str, Any]:
    blinds = snap.get("blinds") or {}
    return {
        "small": blind_detail(blinds, "small"),
        "big": blind_detail(blinds, "big"),
        "boss": blind_detail(blinds, "boss"),
    }


def find_nearest_snapshot(
    event_ts: datetime | None,
    event_index: int,
    total_events: int,
    snapshots: list[tuple[int, Path, dict[str, Any]]],
    ts_map: dict[int, datetime | None],
) -> dict[str, Any] | None:
    """Pick the snapshot whose event_ts is closest to this event's ts.

    Fallback: if timestamps can't line up, pick by proportional index —
    the i-th of N events ≈ the round(i * len(snaps) / N)-th snapshot.
    """
    if not snapshots:
        return None

    # time-based nearest
    if event_ts is not None:
        best = None
        best_dt = None
        for tick, _, snap in snapshots:
            snap_ts = ts_map.get(tick)
            if snap_ts is None:
                continue
            dt = abs((snap_ts - event_ts).total_seconds())
            if best_dt is None or dt < best_dt:
                best = snap
                best_dt = dt
        if best is not None:
            return best

    # index-based fallback
    if total_events > 0:
        idx = min(
            len(snapshots) - 1,
            max(0, int(event_index * len(snapshots) / max(total_events, 1))),
        )
        return snapshots[idx][2]
    return snapshots[0][2]


def enrich(session_dir: Path) -> Path:
    events_path = session_dir / "events.jsonl"
    snap_dir = session_dir / "snapshots"
    out_path = session_dir / "events.enriched.jsonl"

    if not events_path.exists():
        print(f"no events.jsonl at {events_path}", file=sys.stderr)
        sys.exit(2)

    events = [json.loads(l) for l in events_path.read_text().splitlines() if l.strip()]
    snapshots = load_snapshots(snap_dir)

    # estimate per-snapshot timestamp from snapshot_every + interval
    # (observer writes mtime but we read JSON only; rely on file mtime)
    ts_map: dict[int, datetime | None] = {}
    for tick, path, _ in snapshots:
        try:
            ts_map[tick] = datetime.fromtimestamp(path.stat().st_mtime).astimezone()
        except Exception:  # noqa: BLE001
            ts_map[tick] = None

    with out_path.open("w") as out:
        enriched = 0
        for i, ev in enumerate(events):
            ets = parse_ts(ev.get("ts", ""))
            snap = find_nearest_snapshot(ets, i, len(events), snapshots, ts_map)
            if snap is not None:
                ev = dict(ev)
                ev["blinds_detail"] = snapshot_to_blinds_detail(snap)
                enriched += 1
            out.write(json.dumps(ev, ensure_ascii=False) + "\n")

    print(f"wrote {out_path}")
    print(f"  events total   : {len(events)}")
    print(f"  enriched       : {enriched}")
    print(f"  snapshots used : {len(snapshots)}")
    return out_path


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--session", type=Path, required=True)
    args = p.parse_args()
    enrich(args.session)
    return 0


if __name__ == "__main__":
    sys.exit(main())
