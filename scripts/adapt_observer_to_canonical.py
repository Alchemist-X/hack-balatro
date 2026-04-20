#!/usr/bin/env python3
"""Convert an observer session (events.jsonl + snapshots/) into a canonical trajectory.

    python scripts/adapt_observer_to_canonical.py \\
        --session results/real-client-trajectories/observer-20260420T223706 \\
        --output  results/real-client-trajectories/observer-20260420T223706/trajectory.canonical.json

Reconciliation rules (the observer's known losses):
  - `hand_played` with empty `cards_played` is fixed up by diffing `pre_play_hand`
    against the earliest post-play `SELECTING_HAND` / `ROUND_EVAL` tick.
  - `discard` events with `prev.state == BLIND_SELECT` are dropped (round-boundary reset,
    not a real action).
  - state-only transitions are kept as `observe` steps so timing is preserved.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from env.canonical_trajectory import (
    CanonicalAction,
    CanonicalMeta,
    CanonicalState,
    CanonicalStep,
    CanonicalTrajectory,
    now_iso,
)


def summary_to_state(summary: dict[str, Any]) -> CanonicalState:
    return CanonicalState(
        state=summary.get("state"),
        ante=summary.get("ante"),
        round=summary.get("round"),
        hands_left=summary.get("hands_left"),
        discards_left=summary.get("discards_left"),
        round_chips=summary.get("round_chips"),
        money=summary.get("money"),
        hand_cards=list(summary.get("hand_cards") or []),
        hand_ids=list(summary.get("hand_ids") or []),
        jokers_count=summary.get("jokers"),
        consumables_count=summary.get("consumables"),
        blind_small=summary.get("blind_small"),
        blind_big=summary.get("blind_big"),
        blind_boss=summary.get("blind_boss"),
        won=summary.get("won"),
    )


def find_next_summary(events: list[dict[str, Any]], start_idx: int, predicate) -> dict[str, Any] | None:
    for i in range(start_idx + 1, len(events)):
        sa = events[i].get("summary_after")
        if isinstance(sa, dict) and predicate(sa):
            return sa
    return None


def reconcile_hand_played(
    ev: dict[str, Any],
    ev_idx: int,
    events: list[dict[str, Any]],
    prev_summary: dict[str, Any] | None = None,
) -> tuple[list[str], str | None]:
    """Return (cards_played, reconciled_from_state) — recovers empty cards_played.

    Recovery priority:
      1. event's own cards_played (fast path, no reconciliation needed)
      2. event.pre_play_hand (added by newer observer)
      3. prev_summary.hand_cards (for legacy sessions)
      4. event.summary_after.hand_cards (last resort — usually the post-play hand)
    """
    cards_played = ev.get("cards_played") or []
    if cards_played:
        return list(cards_played), None
    pre_hand = (
        ev.get("pre_play_hand")
        or (prev_summary.get("hand_cards") if isinstance(prev_summary, dict) else None)
        or ev.get("summary_after", {}).get("hand_cards")
        or []
    )
    # find the next post-play frame where state is SELECTING_HAND (next round of drawing)
    # or ROUND_EVAL (round ended). hand then is disjoint from the played cards.
    post = find_next_summary(
        events,
        ev_idx,
        lambda s: s.get("state") in {"SELECTING_HAND", "ROUND_EVAL", "DRAW_TO_HAND", "GAME_OVER"}
        and s.get("hand_cards") is not None,
    )
    # fallback: adopt the event's own summary_after (works for GAME_OVER terminal plays
    # where the post-play hand is the same tick)
    if not post:
        post = ev.get("summary_after") or {}
    post_hand = list(post.get("hand_cards") or [])
    played = [c for c in pre_hand if c not in post_hand]
    return played, post.get("state")


def build_trajectory(session_dir: Path) -> CanonicalTrajectory:
    events_path = session_dir / "events.jsonl"
    snaps_dir = session_dir / "snapshots"
    events = [json.loads(l) for l in events_path.read_text().splitlines() if l.strip()]

    # pull metadata from first snapshot (authoritative gamestate)
    snap_files = sorted(snaps_dir.glob("tick-*.json")) if snaps_dir.exists() else []
    first_snap: dict[str, Any] = {}
    if snap_files:
        first_snap = json.loads(snap_files[0].read_text())
    meta = CanonicalMeta(
        source="real-client-observer",
        captured_at=now_iso(),
        seed=first_snap.get("seed"),
        deck=first_snap.get("deck"),
        stake=first_snap.get("stake"),
        agent_id="human",
        extra={
            "session_dir": str(session_dir),
            "source_events_count": len(events),
            "source_snapshots_count": len(snap_files),
        },
    )

    steps: list[CanonicalStep] = []
    step_idx = 0
    last_summary: dict[str, Any] | None = None

    # find initial summary
    for ev in events:
        if ev.get("kind") == "initial":
            last_summary = ev.get("summary", {})
            break

    for i, ev in enumerate(events):
        kind = ev.get("kind")
        sa = ev.get("summary_after") or ev.get("summary")
        if not isinstance(sa, dict):
            continue

        before = last_summary if last_summary is not None else sa
        action: CanonicalAction | None = None

        if kind == "hand_played":
            cards, _ = reconcile_hand_played(ev, i, events, last_summary)
            action = CanonicalAction(
                type="play",
                params={
                    "cards": cards,
                    "cards_drawn": ev.get("cards_drawn") or [],
                    "hands_left_after": ev.get("hands_left_after"),
                    "round_chips_after": ev.get("round_chips_after"),
                    "reconciled": bool(ev.get("needs_reconcile") and cards),
                },
            )
        elif kind == "discard":
            # drop any discard that was recorded while prev state was BLIND_SELECT
            # (these are round-boot counter resets, not real discards).
            if last_summary and last_summary.get("state") == "BLIND_SELECT":
                continue
            action = CanonicalAction(
                type="discard",
                params={
                    "cards": ev.get("cards_discarded") or [],
                    "cards_drawn": ev.get("cards_drawn") or [],
                    "discards_left_after": ev.get("discards_left_after"),
                },
            )
        elif kind == "money_change":
            delta = ev.get("delta", 0)
            if delta > 0 and sa.get("state") == "SHOP":
                action = CanonicalAction(type="cash_out", params={"delta": delta})
            elif delta < 0 and sa.get("state") == "SHOP":
                action = CanonicalAction(type="buy", params={"delta": delta})
            elif delta < 0 and sa.get("state") in {"SMODS_BOOSTER_OPENED", "BOOSTER_OPENED"}:
                action = CanonicalAction(type="buy", params={"kind": "pack", "delta": delta})
            else:
                action = CanonicalAction(type="observe", params={"reason": "money_change", "delta": delta})
        elif kind == "jokers_count_change":
            if (ev.get("to") or 0) > (ev.get("from") or 0):
                action = CanonicalAction(type="buy", params={"kind": "joker"})
            else:
                action = CanonicalAction(type="sell", params={"kind": "joker"})
        elif kind == "consumables_count_change":
            if (ev.get("to") or 0) > (ev.get("from") or 0):
                action = CanonicalAction(type="buy", params={"kind": "consumable"})
            else:
                action = CanonicalAction(type="use_consumable", params={})
        elif kind == "blind_status_change":
            blind = ev.get("blind")
            to = ev.get("to")
            if to == "SKIPPED":
                action = CanonicalAction(type="skip_blind", params={"which": blind})
            elif to == "CURRENT":
                action = CanonicalAction(type="select_blind", params={"which": blind})
            else:
                continue  # UPCOMING/SELECT/DEFEATED are consequences, not user actions
        elif kind == "state_change":
            frm, to = ev.get("from"), ev.get("to")
            if frm == "SHOP" and to == "BLIND_SELECT":
                action = CanonicalAction(type="next_round", params={})
            elif to == "GAME_OVER":
                action = CanonicalAction(type="observe", params={"reason": "game_over"})
            else:
                last_summary = sa
                continue
        else:
            last_summary = sa
            continue

        if action is None:
            last_summary = sa
            continue

        steps.append(CanonicalStep(
            step_idx=step_idx,
            ts=ev.get("ts", ""),
            state_before=summary_to_state(before),
            action=action,
            state_after=summary_to_state(sa),
            info={"source_event_kind": kind},
        ))
        step_idx += 1
        last_summary = sa

    return CanonicalTrajectory(meta=meta, steps=steps)


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--session", type=Path, required=True, help="observer session directory")
    p.add_argument("--output", type=Path, default=None, help="output canonical JSON path")
    args = p.parse_args()

    traj = build_trajectory(args.session)
    out = args.output or (args.session / "trajectory.canonical.json")
    traj.to_json(out)

    # quick summary
    print(f"wrote {out}")
    print(f"  meta.seed={traj.meta.seed} deck={traj.meta.deck} stake={traj.meta.stake}")
    print(f"  steps: {len(traj.steps)}")
    from collections import Counter
    types = Counter(s.action.type for s in traj.steps)
    for t, n in types.most_common():
        print(f"    {n:3d}  {t}")

    # readback sanity
    roundtrip = CanonicalTrajectory.from_json(out)
    assert len(roundtrip.steps) == len(traj.steps), "round-trip step count mismatch"
    print("  readback: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
