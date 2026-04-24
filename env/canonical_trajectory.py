"""Canonical trajectory schema shared by simulator, real-client, LLM agents.

One schema so that downstream consumers (training, eval, sim-vs-real diff)
don't need to know which producer they are reading.

Every collector (LLM, sim REPL, real-client observer, future online RL) MUST
emit this schema. There is exactly one canonical shape — see README /
docs/canonical_trajectory_schema.md for the authoritative spec.

Backward compatibility
----------------------
This module was extended on 2026-04-24 to add richer decision-provenance
fields (`legal_actions`, `requested_action`, `parsed_action`,
`executed_action`, `fallback_used`, `reward`, `terminal`). The legacy
`action: CanonicalAction` field is preserved so already-committed
`trajectory.canonical.json` files keep loading cleanly via
`CanonicalTrajectory.from_json(...)`. Missing new fields default to safe
values (empty list / None / False).

Usage
-----
    from env.canonical_trajectory import CanonicalStep, CanonicalTrajectory
    traj = CanonicalTrajectory(meta=..., steps=[...])
    traj.to_json(Path("out.json"))
    traj2 = CanonicalTrajectory.from_json(Path("out.json"))

Minimum example (one step):

    CanonicalStep(
        step_idx=0,
        ts="2026-04-24T09:00:00Z",
        state_before=CanonicalState(state="SHOP", money=4, ...),
        legal_actions=["buy_shop_item_0", "next_round"],
        requested_action="next_round",
        parsed_action=1,
        executed_action=1,
        fallback_used=FallbackInfo(used=False, reason=None),
        action=CanonicalAction(type="next_round"),
        state_after=CanonicalState(state="BLIND_SELECT", ...),
        reward=0.0,
        terminal=False,
        info={},
    )
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
    "select_card",      # toggle card selection (LLM / sim REPL granular control)
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
class FallbackInfo:
    """Record when a producer had to substitute an action."""
    used: bool = False
    reason: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict[str, Any] | None) -> "FallbackInfo":
        if not d:
            return cls()
        return cls(used=bool(d.get("used", False)), reason=d.get("reason"))


@dataclass
class CanonicalStep:
    """One trajectory step.

    Required fields:
        step_idx, ts, state_before, state_after

    Decision provenance (new 2026-04-24 — required for new producers,
    optional for backward-compat readback of older JSONs):
        legal_actions       list[str] | list[int] | None
        requested_action    str | None   — raw agent output (e.g. "play")
        parsed_action       int | None   — index after action parsing
        executed_action     int | None   — what actually ran (may differ from parsed on fallback)
        fallback_used       FallbackInfo
        reward              float | None
        terminal            bool

    Legacy field (kept for backward-compat; new producers may populate it
    as a structured summary of the executed action):
        action              CanonicalAction | None

    info dict is always available for free-form extension.
    """
    step_idx: int
    ts: str
    state_before: CanonicalState
    state_after: CanonicalState
    # new fields (defaults make old JSONs deserializable)
    legal_actions: list[Any] | None = None
    requested_action: str | None = None
    parsed_action: int | None = None
    executed_action: int | None = None
    fallback_used: FallbackInfo = field(default_factory=FallbackInfo)
    reward: float | None = None
    terminal: bool = False
    # legacy-compatible structured action summary (not mutually exclusive
    # with executed_action; still useful for human-readable consumption)
    action: CanonicalAction | None = None
    info: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "step_idx": self.step_idx,
            "ts": self.ts,
            "state_before": self.state_before.to_dict(),
            "legal_actions": self.legal_actions,
            "requested_action": self.requested_action,
            "parsed_action": self.parsed_action,
            "executed_action": self.executed_action,
            "fallback_used": self.fallback_used.to_dict(),
            "action": self.action.to_dict() if self.action is not None else None,
            "state_after": self.state_after.to_dict(),
            "reward": self.reward,
            "terminal": self.terminal,
            "info": self.info,
        }

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> "CanonicalStep":
        action_raw = d.get("action")
        action_obj: CanonicalAction | None
        if action_raw is None:
            action_obj = None
        elif isinstance(action_raw, dict):
            action_obj = CanonicalAction(**action_raw)
        else:
            action_obj = None
        return cls(
            step_idx=d["step_idx"],
            ts=d["ts"],
            state_before=CanonicalState(**d["state_before"]),
            state_after=CanonicalState(**d["state_after"]),
            legal_actions=d.get("legal_actions"),
            requested_action=d.get("requested_action"),
            parsed_action=d.get("parsed_action"),
            executed_action=d.get("executed_action"),
            fallback_used=FallbackInfo.from_dict(d.get("fallback_used")),
            reward=d.get("reward"),
            terminal=bool(d.get("terminal", False)),
            action=action_obj,
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
