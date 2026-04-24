#!/usr/bin/env python3
"""Migrate legacy llm_play_game.py trajectories to the canonical schema.

Legacy shape (top-level):
    {
      "seed": int,
      "agent": str,
      "won": bool,
      "final_ante": int,
      "steps": int,
      "trajectory": [
        {
          "step": int,
          "state_text": str,
          "reasoning": str,
          "action": str,
          "score_before": int,
          "score_after": int
        },
        ...
      ]
    }

Canonical output shape: see env/canonical_trajectory.py.

This is a one-shot migrator. It does NOT delete the legacy file; it writes
`<legacy>.canonical.json` alongside. The legacy file stays as an archive.

Usage
-----
    # single file
    python scripts/migrate_llm_trajectories_to_canonical.py \\
        --input  results/trajectories/llm_claude_code/game_0042.json \\
        --output results/trajectories/llm_claude_code/game_0042.canonical.json

    # every legacy shape file under results/trajectories/
    python scripts/migrate_llm_trajectories_to_canonical.py --all

Notes / known lossy conversions
-------------------------------
1. `state_text` is a rendered LLM prompt. We PARSE enough of it to populate
   a lightweight CanonicalState (stage, ante, round, hands_left, discards_left,
   money, round_chips, hand_cards). Anything we can't parse stays None and
   the raw state_text is preserved in info.state_text_legacy.
2. `action` is a raw action NAME (e.g. "play", "select_card_3"). We store
   it in `requested_action` and in `executed_action` as the integer index
   IF the legacy trace includes a LEGAL ACTIONS line where we can look up
   the index. If we can't map it, executed_action stays None.
3. `reasoning` goes into info.reasoning.
4. `score_before`/`score_after` go into info for auditability, and we
   compute `reward = score_after - score_before` as a float.
5. `legal_actions` is parsed from the state_text LEGAL ACTIONS line when
   present.
6. `terminal` is True only on the final step of the trajectory.
7. `fallback_used` is always {False, None} for legacy data — the original
   collector didn't record fallback info.

The conversion is lossy on hand_cards identity (no stable card ids in
legacy), so hand_ids stays empty.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from env.canonical_trajectory import (
    ACTION_TYPES,
    CanonicalAction,
    CanonicalMeta,
    CanonicalState,
    CanonicalStep,
    CanonicalTrajectory,
    FallbackInfo,
    now_iso,
)


# ---- state_text parsing -------------------------------------------------

STAGE_RE = re.compile(r"^\[STAGE\]\s+(\S+)(?:\s+\|\s+(.+))?$", re.M)
ANTE_RE = re.compile(r"^\[ANTE\]\s+(\d+)\s+\|\s+Round\s+(\d+)", re.M)
SCORE_RE = re.compile(r"^\[SCORE\]\s+(\d+)/(\d+)", re.M)
RES_RE = re.compile(r"^\[RESOURCES\]\s+Plays:\s+(\d+)\s+\|\s+Discards:\s+(\d+)\s+\|\s+Money:\s+\$(\d+)", re.M)
HAND_RE = re.compile(r"^\[HAND\]\s+(.+)$", re.M)
LEGAL_RE = re.compile(r"^\[LEGAL ACTIONS\]\s+(.+)$", re.M)

STAGE_STATE_MAP = {
    "PreBlind": "BLIND_SELECT",
    "Blind": "SELECTING_HAND",
    "PostBlind": "ROUND_EVAL",
    "Shop": "SHOP",
    "End": "GAME_OVER",
}


def parse_state_text(text: str) -> tuple[CanonicalState, list[str]]:
    """Best-effort parse; returns (state, legal_actions)."""
    stage: str | None = None
    ante = round_ = hands_left = discards_left = money = round_chips = None
    hand_cards: list[str] = []
    legal: list[str] = []

    m = STAGE_RE.search(text)
    if m:
        stage_label = m.group(1)
        stage = STAGE_STATE_MAP.get(stage_label, stage_label)
    m = ANTE_RE.search(text)
    if m:
        ante = int(m.group(1))
        round_ = int(m.group(2))
    m = SCORE_RE.search(text)
    if m:
        round_chips = int(m.group(1))
    m = RES_RE.search(text)
    if m:
        hands_left = int(m.group(1))
        discards_left = int(m.group(2))
        money = int(m.group(3))
    m = HAND_RE.search(text)
    if m:
        # strip selection markers like *5D* -> 5D
        raw = m.group(1).strip()
        hand_cards = [c.strip().strip("*") for c in raw.split("|") if c.strip()]
    m = LEGAL_RE.search(text)
    if m:
        legal = [a.strip() for a in m.group(1).split(",") if a.strip()]

    return (
        CanonicalState(
            state=stage,
            ante=ante,
            round=round_,
            hands_left=hands_left,
            discards_left=discards_left,
            round_chips=round_chips,
            money=money,
            hand_cards=hand_cards,
        ),
        legal,
    )


# ---- action mapping -----------------------------------------------------

# Map action names (like "buy_shop_item_0") to a canonical action type.
ACTION_NAME_PREFIX_MAP: list[tuple[str, str]] = [
    ("select_blind_", "select_blind"),
    ("skip_blind", "skip_blind"),
    ("select_card_", "select_card"),
    ("play", "play"),
    ("discard", "discard"),
    ("buy_shop_item_", "buy"),
    ("buy_consumable_", "buy"),
    ("buy_voucher_", "buy"),
    ("buy_pack_", "buy"),
    ("sell_joker_", "sell"),
    ("sell_consumable_", "sell"),
    ("use_consumable_", "use_consumable"),
    ("reroll", "reroll"),
    ("cashout", "cash_out"),
    ("cash_out", "cash_out"),
    ("next_round", "next_round"),
]


def canonicalize_action_name(name: str) -> tuple[str, dict[str, Any]]:
    """Return (canonical_type, params) for an action name."""
    for prefix, ct in ACTION_NAME_PREFIX_MAP:
        if name == prefix or name.startswith(prefix):
            params: dict[str, Any] = {}
            suffix = name[len(prefix):]
            if suffix.isdigit():
                params["slot"] = int(suffix)
            if ct == "buy":
                if "consumable" in name:
                    params["kind"] = "consumable"
                elif "voucher" in name:
                    params["kind"] = "voucher"
                elif "pack" in name:
                    params["kind"] = "pack"
                elif "shop_item" in name:
                    params["kind"] = "joker"
            if ct == "sell":
                params["kind"] = "joker" if "joker" in name else "consumable"
            return ct, params
    return "observe", {"reason": f"unmapped:{name}"}


# ---- migration ----------------------------------------------------------

def is_legacy_shape(doc: dict[str, Any]) -> bool:
    return (
        isinstance(doc.get("trajectory"), list)
        and bool(doc.get("trajectory"))
        and isinstance(doc["trajectory"][0], dict)
        and "state_text" in doc["trajectory"][0]
        and "action" in doc["trajectory"][0]
    )


def migrate(legacy_path: Path, out_path: Path) -> None:
    doc = json.loads(legacy_path.read_text())
    if not is_legacy_shape(doc):
        raise ValueError(f"{legacy_path}: not in legacy llm_play_game shape")

    steps_raw = doc["trajectory"]
    meta = CanonicalMeta(
        source="llm-claude-code",
        captured_at=now_iso(),
        seed=str(doc.get("seed")) if doc.get("seed") is not None else None,
        deck=None,
        stake=None,
        agent_id=doc.get("agent"),
        extra={
            "legacy_file": str(legacy_path),
            "legacy_steps": doc.get("steps"),
            "legacy_final_ante": doc.get("final_ante"),
            "legacy_won": doc.get("won"),
        },
    )

    steps: list[CanonicalStep] = []
    # seed the "prev state" with the first step's parsed state_before
    prev_state: CanonicalState | None = None

    for i, raw in enumerate(steps_raw):
        state_before, legal = parse_state_text(raw.get("state_text", ""))
        if prev_state is not None:
            # use prev step's parsed state as a slightly more accurate
            # state_before (legacy didn't emit state_after explicitly)
            state_before_use = prev_state
        else:
            state_before_use = state_before

        action_name = raw.get("action", "")
        ct, params = canonicalize_action_name(action_name)
        try:
            action_obj: CanonicalAction | None = CanonicalAction(type=ct, params=params)
        except ValueError:
            action_obj = CanonicalAction(type="observe", params={"reason": f"unknown:{action_name}"})

        parsed_idx: int | None = None
        if action_name in legal:
            parsed_idx = legal.index(action_name)

        score_before = raw.get("score_before")
        score_after = raw.get("score_after")
        reward: float | None = None
        if isinstance(score_before, (int, float)) and isinstance(score_after, (int, float)):
            reward = float(score_after) - float(score_before)

        # state_after is unknown in legacy; best approximation is the next
        # step's state_before, or fall back to current state_before.
        if i + 1 < len(steps_raw):
            next_state, _ = parse_state_text(steps_raw[i + 1].get("state_text", ""))
            state_after = next_state
        else:
            state_after = state_before

        terminal = (i == len(steps_raw) - 1)

        steps.append(CanonicalStep(
            step_idx=raw.get("step", i),
            ts="",  # legacy had no per-step timestamp
            state_before=state_before_use,
            legal_actions=legal or None,
            requested_action=action_name or None,
            parsed_action=parsed_idx,
            executed_action=parsed_idx,  # assume the named legal action did execute
            fallback_used=FallbackInfo(used=False, reason=None),
            action=action_obj,
            state_after=state_after,
            reward=reward,
            terminal=terminal,
            info={
                "reasoning": raw.get("reasoning"),
                "state_text_legacy": raw.get("state_text"),
                "score_before": score_before,
                "score_after": score_after,
                "reconstructed": True,
                "legacy_migration": True,
            },
        ))
        prev_state = state_after

    traj = CanonicalTrajectory(meta=meta, steps=steps)
    traj.to_json(out_path)
    # readback sanity
    roundtrip = CanonicalTrajectory.from_json(out_path)
    assert len(roundtrip.steps) == len(traj.steps)
    print(f"wrote {out_path}  ({len(traj.steps)} steps)")


def find_legacy_files(root: Path) -> list[Path]:
    out: list[Path] = []
    for p in root.rglob("*.json"):
        if ".canonical" in p.name:
            continue
        try:
            d = json.loads(p.read_text())
        except Exception:
            continue
        if is_legacy_shape(d):
            out.append(p)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", type=Path, help="single legacy trajectory JSON")
    ap.add_argument("--output", type=Path, help="output canonical JSON (default: alongside input)")
    ap.add_argument("--all", action="store_true", help="migrate every legacy-shape file under results/trajectories/")
    ap.add_argument("--root", type=Path, default=Path("results/trajectories"), help="root for --all")
    args = ap.parse_args()

    if args.all:
        files = find_legacy_files(args.root)
        if not files:
            print(f"no legacy-shape files found under {args.root}")
            return 0
        for p in files:
            out = p.with_name(p.stem + ".canonical.json")
            migrate(p, out)
        return 0

    if not args.input:
        ap.error("--input required when --all not given")

    out = args.output or args.input.with_name(args.input.stem + ".canonical.json")
    migrate(args.input, out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
