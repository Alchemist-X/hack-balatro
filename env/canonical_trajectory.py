"""Canonical trajectory schema shared by simulator, real-client, LLM agents.

One schema so that downstream consumers (training, eval, sim-vs-real diff)
don't need to know which producer they are reading.

Usage:
    from env.canonical_trajectory import CanonicalStep, CanonicalTrajectory
    traj = CanonicalTrajectory(meta=..., steps=[...])
    traj.to_json(Path("out.json"))
    traj2 = CanonicalTrajectory.from_json(Path("out.json"))
"""
from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


# ---- action vocabulary --------------------------------------------------
ACTION_TYPES = frozenset({
    "play",             # play a poker hand; params: {"cards": [id, ...]}
    "discard",          # discard cards; params: {"cards": [id, ...]}
    "buy",              # buy item in shop; params: {"slot": int, "kind": "joker"|"consumable"|"voucher"|"pack"|"card"}
    "sell",             # sell from inventory; params: {"kind": "joker"|"consumable", "slot": int}
    "use_consumable",   # use a tarot/planet/spectral; params: {"slot": int, "target_cards": [id, ...]|None}
    "reroll",           # reroll shop
    "skip_blind",       # skip current blind
    "select_blind",     # accept current blind; params: {"which": "small"|"big"|"boss"}
    "cash_out",         # collect end-of-round reward
    "next_round",       # leave shop
    "pack_choice",      # pick an item from an opened booster pack; params: {"slot": int, "skip": bool}
    "rearrange",        # reorder hand/jokers; params: {"from": int, "to": int}
    "observe",          # placeholder for "looked but did nothing"
})


@dataclass
class CanonicalState:
    """Compact state representation. Anything bigger goes into info.raw_snapshot."""
    state: str | None = None
    ante: int | None = None
    round: int | None = None
    hands_left: int | None = None
    discards_left: int | None = None
    round_chips: int | float | None = None
    money: int | float | None = None
    hand_cards: list[str] = field(default_factory=list)
    hand_ids: list[int] = field(default_factory=list)
    jokers_count: int | None = None
    consumables_count: int | None = None
    blind_small: str | None = None
    blind_big: str | None = None
    blind_boss: str | None = None
    won: bool | None = None

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass
class CanonicalAction:
    type: str
    params: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.type not in ACTION_TYPES:
            raise ValueError(
                f"unknown action type: {self.type!r} "
                f"(allowed: {sorted(ACTION_TYPES)})"
            )

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass
class CanonicalStep:
    step_idx: int
    ts: str
    state_before: CanonicalState
    action: CanonicalAction
    state_after: CanonicalState
    info: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "step_idx": self.step_idx,
            "ts": self.ts,
            "state_before": self.state_before.to_dict(),
            "action": self.action.to_dict(),
            "state_after": self.state_after.to_dict(),
            "info": self.info,
        }

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> "CanonicalStep":
        return cls(
            step_idx=d["step_idx"],
            ts=d["ts"],
            state_before=CanonicalState(**d["state_before"]),
            action=CanonicalAction(**d["action"]),
            state_after=CanonicalState(**d["state_after"]),
            info=d.get("info", {}),
        )


@dataclass
class CanonicalMeta:
    source: str            # "real-client-observer" | "balatro-native-sim" | "llm-claude-code" | ...
    captured_at: str
    seed: str | None = None
    deck: str | None = None
    stake: str | None = None
    agent_id: str | None = None
    extra: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass
class CanonicalTrajectory:
    meta: CanonicalMeta
    steps: list[CanonicalStep] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return {
            "meta": self.meta.to_dict(),
            "steps": [s.to_dict() for s in self.steps],
        }

    def to_json(self, path: Path, indent: int | None = 2) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.to_dict(), ensure_ascii=False, indent=indent))

    @classmethod
    def from_json(cls, path: Path) -> "CanonicalTrajectory":
        d = json.loads(path.read_text())
        return cls(
            meta=CanonicalMeta(**d["meta"]),
            steps=[CanonicalStep.from_dict(s) for s in d["steps"]],
        )


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()
