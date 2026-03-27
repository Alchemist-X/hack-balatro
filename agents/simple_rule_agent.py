from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import numpy as np

from env.action_space import action_name

from scripts.behavior_log import plan_simple_rule_action


def _default_ruleset_path() -> Path:
    return Path("fixtures/ruleset/balatro-1.0.1o-full.json")


class SimpleRuleAgent:
    policy_id = "simple_rule_v1"

    def __init__(self, ruleset_path: str | None = None) -> None:
        bundle_path = Path(ruleset_path) if ruleset_path else _default_ruleset_path()
        self.ruleset_path = str(bundle_path)
        self.bundle = json.loads(bundle_path.read_text())

    def legal_actions_from_mask(self, action_mask: np.ndarray | list[bool]) -> list[dict[str, Any]]:
        mask = np.asarray(action_mask, dtype=bool)
        return [
            {"index": int(index), "name": action_name(int(index)), "enabled": True}
            for index in np.flatnonzero(mask)
        ]

    def plan(self, snapshot: dict[str, Any], legal_actions: list[dict[str, Any]]) -> dict[str, Any]:
        return plan_simple_rule_action(snapshot, legal_actions, self.bundle)

    def choose_action(self, snapshot: dict[str, Any], legal_actions: list[dict[str, Any]]) -> tuple[int, dict[str, Any]]:
        plan = self.plan(snapshot, legal_actions)
        return int(plan["action_index"]), plan

    def act(
        self,
        obs: np.ndarray,
        info: dict[str, Any] | None = None,
        action_mask: np.ndarray | None = None,
    ) -> int:
        del obs
        if info is None or "state_snapshot" not in info:
            raise ValueError("SimpleRuleAgent requires info['state_snapshot']")
        if action_mask is None:
            raise ValueError("SimpleRuleAgent requires action_mask")
        legal_actions = self.legal_actions_from_mask(action_mask)
        action, _plan = self.choose_action(info["state_snapshot"], legal_actions)
        return action
