#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from agents.simple_rule_agent import SimpleRuleAgent

SMALL_BIG = {"Small Blind", "Big Blind"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run strict boss/shop/consumable fidelity coverage scenarios")
    parser.add_argument("--output", type=Path, default=Path("results/fidelity-coverage.json"))
    parser.add_argument("--artifacts-dir", type=Path, default=Path("results/fidelity-coverage"))
    parser.add_argument("--shop-search-seeds", type=int, default=16)
    parser.add_argument("--boss-search-seeds", type=int, default=64)
    parser.add_argument("--max-steps", type=int, default=192)
    parser.add_argument("--ruleset-path", type=str, default=None)
    return parser.parse_args()


def base_replay(engine_name: str, seed: int, policy: str, ruleset_path: str) -> dict[str, Any]:
    return {
        "engine": engine_name,
        "seed": seed,
        "policy": policy,
        "ruleset_path": ruleset_path,
        "transitions": [],
        "final_snapshot": {},
    }


def snapshot_dict(engine: Any) -> dict[str, Any]:
    return json.loads(engine.snapshot().to_json())


def legal_actions(engine: Any) -> list[dict[str, Any]]:
    return [
        {"index": action.index, "name": action.name, "enabled": action.enabled}
        for action in engine.legal_actions()
        if action.enabled
    ]


def action_lookup(actions: list[dict[str, Any]]) -> dict[str, int]:
    return {action["name"]: int(action["index"]) for action in actions}


def sorted_buy_actions(snapshot: dict[str, Any], actions: list[dict[str, Any]]) -> list[tuple[int, str, int]]:
    names = action_lookup(actions)
    candidates: list[tuple[int, str, int]] = []
    for slot, joker in enumerate(snapshot.get("shop_jokers", [])):
        action_name = f"buy_shop_item_{slot}"
        if action_name in names:
            candidates.append((int(joker.get("cost", 999)), action_name, names[action_name]))
    return sorted(candidates)


def update_observed_targets(
    report: dict[str, Any],
    snapshot: dict[str, Any],
    actions: list[dict[str, Any]],
    transition: dict[str, Any] | None,
    seed: int,
) -> None:
    consumables = list(snapshot.get("consumables", []) or [])
    shop_consumables = list(snapshot.get("shop_consumables", []) or [])
    if consumables or shop_consumables:
        report["targets"]["consumable_visible"]["covered"] = True
        report["targets"]["consumable_visible"]["seed"] = seed
    if any(action["name"].startswith("use_consumable_") for action in actions):
        report["targets"]["consumable_slot_enabled"]["covered"] = True
        report["targets"]["consumable_slot_enabled"]["seed"] = seed
    if transition is None:
        return
    before = transition["snapshot_before"]
    after = transition["snapshot_after"]
    action_name = transition["action"]["name"]
    events = transition.get("events", [])

    if before.get("stage") == "Stage_PostBlind" and action_name == "cashout" and after.get("stage") == "Stage_Shop":
        report["targets"]["shop_cashout"]["covered"] = True
        report["targets"]["shop_cashout"]["seed"] = seed
    if action_name.startswith("buy_shop_item_"):
        report["targets"]["shop_buy"]["covered"] = True
        report["targets"]["shop_buy"]["seed"] = seed
    if action_name == "reroll_shop":
        report["targets"]["shop_reroll"]["covered"] = True
        report["targets"]["shop_reroll"]["seed"] = seed
    if action_name.startswith("sell_joker_"):
        report["targets"]["shop_sell"]["covered"] = True
        report["targets"]["shop_sell"]["seed"] = seed
    if before.get("stage") == "Stage_Shop" and action_name == "next_round":
        report["targets"]["shop_next_round"]["covered"] = True
        report["targets"]["shop_next_round"]["seed"] = seed
    if action_name == "select_blind_2":
        report["targets"]["boss_select"]["covered"] = True
        report["targets"]["boss_select"]["seed"] = seed
    if after.get("stage") == "Stage_Blind" and after.get("blind_name") not in SMALL_BIG:
        report["targets"]["boss_enter"]["covered"] = True
        report["targets"]["boss_enter"]["seed"] = seed
    if any(event.get("kind") == "blind_cleared" for event in events) and after.get("blind_states", {}).get("Boss") == "Defeated":
        report["targets"]["boss_defeat"]["covered"] = True
        report["targets"]["boss_defeat"]["seed"] = seed
    if action_name.startswith("use_consumable_"):
        report["targets"]["consumable_use"]["covered"] = True
        report["targets"]["consumable_use"]["seed"] = seed


def choose_shop_harvest_action(
    snapshot: dict[str, Any],
    actions: list[dict[str, Any]],
    agent: SimpleRuleAgent,
    target_report: dict[str, Any],
) -> int:
    names = action_lookup(actions)
    stage = snapshot.get("stage")
    if stage == "Stage_PreBlind":
        for candidate in ("select_blind_0", "select_blind_1", "select_blind_2"):
            if candidate in names:
                return names[candidate]
    if stage == "Stage_Blind":
        return agent.choose_action(snapshot, actions)[0]
    if stage == "Stage_PostBlind":
        return names["cashout"]
    if stage == "Stage_Shop":
        if not target_report["targets"]["shop_reroll"]["covered"] and "reroll_shop" in names:
            return names["reroll_shop"]
        if not target_report["targets"]["shop_buy"]["covered"]:
            buys = sorted_buy_actions(snapshot, actions)
            if buys:
                return buys[0][2]
        if not target_report["targets"]["shop_sell"]["covered"]:
            for slot in range(5):
                action_name = f"sell_joker_{slot}"
                if action_name in names:
                    return names[action_name]
        if "next_round" in names:
            return names["next_round"]
    return actions[0]["index"]


def choose_boss_rush_action(snapshot: dict[str, Any], actions: list[dict[str, Any]], agent: SimpleRuleAgent) -> int:
    names = action_lookup(actions)
    stage = snapshot.get("stage")
    if stage == "Stage_PreBlind":
        if "select_blind_2" in names:
            return names["select_blind_2"]
        if "skip_blind" in names:
            return names["skip_blind"]
        for candidate in ("select_blind_0", "select_blind_1"):
            if candidate in names:
                return names[candidate]
    if stage == "Stage_Blind":
        return agent.choose_action(snapshot, actions)[0]
    if stage == "Stage_PostBlind":
        return names["cashout"]
    if stage == "Stage_Shop" and "next_round" in names:
        return names["next_round"]
    return actions[0]["index"]


def choose_boss_clear_action(snapshot: dict[str, Any], actions: list[dict[str, Any]], agent: SimpleRuleAgent) -> int:
    names = action_lookup(actions)
    stage = snapshot.get("stage")
    if stage == "Stage_PreBlind":
        for candidate in ("select_blind_0", "select_blind_1", "select_blind_2"):
            if candidate in names:
                return names[candidate]
    if stage == "Stage_Blind":
        return agent.choose_action(snapshot, actions)[0]
    if stage == "Stage_PostBlind":
        return names["cashout"]
    if stage == "Stage_Shop" and "next_round" in names:
        return names["next_round"]
    return actions[0]["index"]


def run_episode(
    *,
    seed: int,
    mode: str,
    max_steps: int,
    ruleset_path: str | None,
    agent: SimpleRuleAgent,
    target_report: dict[str, Any],
) -> dict[str, Any]:
    import balatro_native

    engine = balatro_native.Engine(seed=seed, ruleset_path=ruleset_path, stake=1)
    replay = base_replay("balatro_native", seed, mode, ruleset_path or balatro_native.default_ruleset_path())
    for step_index in range(max_steps):
        snapshot = snapshot_dict(engine)
        actions = legal_actions(engine)
        update_observed_targets(target_report, snapshot, actions, None, seed)
        if not actions:
            break
        if mode == "shop_harvest":
            action = choose_shop_harvest_action(snapshot, actions, agent, target_report)
        elif mode == "boss_rush":
            action = choose_boss_rush_action(snapshot, actions, agent)
        elif mode == "boss_clear":
            action = choose_boss_clear_action(snapshot, actions, agent)
        else:
            raise ValueError(f"unknown mode {mode}")
        transition = json.loads(engine.step(int(action)).to_json())
        transition["step_index"] = step_index
        replay["transitions"].append(transition)
        update_observed_targets(target_report, snapshot, actions, transition, seed)
        if transition["snapshot_after"].get("over"):
            break
    replay["final_snapshot"] = snapshot_dict(engine)
    return replay


def save_replay(path: Path, replay: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(replay, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def coverage_report_template() -> dict[str, Any]:
    return {
        "ok": False,
        "targets": {
            "shop_cashout": {"covered": False, "seed": None},
            "shop_buy": {"covered": False, "seed": None},
            "shop_reroll": {"covered": False, "seed": None},
            "shop_sell": {"covered": False, "seed": None},
            "shop_next_round": {"covered": False, "seed": None},
            "boss_select": {"covered": False, "seed": None},
            "boss_enter": {"covered": False, "seed": None},
            "boss_defeat": {"covered": False, "seed": None},
            "consumable_visible": {"covered": False, "seed": None},
            "consumable_slot_enabled": {"covered": False, "seed": None},
            "consumable_use": {"covered": False, "seed": None},
        },
        "artifacts": {},
        "notes": [],
    }


def fully_covered(targets: dict[str, Any], names: list[str]) -> bool:
    return all(targets[name]["covered"] for name in names)


def main() -> int:
    args = parse_args()
    import balatro_native

    ruleset_path = args.ruleset_path or balatro_native.default_ruleset_path()
    agent = SimpleRuleAgent(ruleset_path)
    report = coverage_report_template()

    shop_targets = ["shop_cashout", "shop_buy", "shop_reroll", "shop_sell", "shop_next_round"]
    shop_replay = None
    for seed in range(1, args.shop_search_seeds + 1):
        replay = run_episode(
            seed=seed,
            mode="shop_harvest",
            max_steps=args.max_steps,
            ruleset_path=ruleset_path,
            agent=agent,
            target_report=report,
        )
        if shop_replay is None:
            shop_replay = replay
        if fully_covered(report["targets"], shop_targets):
            shop_replay = replay
            break
    if shop_replay is not None:
        shop_path = args.artifacts_dir / "shop-harvest.replay.json"
        save_replay(shop_path, shop_replay)
        report["artifacts"]["shop_harvest_replay"] = str(shop_path)

    boss_rush_replay = None
    for seed in range(1, args.boss_search_seeds + 1):
        replay = run_episode(
            seed=seed,
            mode="boss_rush",
            max_steps=args.max_steps,
            ruleset_path=ruleset_path,
            agent=agent,
            target_report=report,
        )
        if boss_rush_replay is None:
            boss_rush_replay = replay
        if fully_covered(report["targets"], ["boss_select", "boss_enter"]):
            boss_rush_replay = replay
            break
    if boss_rush_replay is not None:
        boss_rush_path = args.artifacts_dir / "boss-rush.replay.json"
        save_replay(boss_rush_path, boss_rush_replay)
        report["artifacts"]["boss_rush_replay"] = str(boss_rush_path)

    boss_clear_replay = None
    for seed in range(1, args.boss_search_seeds + 1):
        replay = run_episode(
            seed=seed,
            mode="boss_clear",
            max_steps=args.max_steps,
            ruleset_path=ruleset_path,
            agent=agent,
            target_report=report,
        )
        boss_clear_replay = replay
        if report["targets"]["boss_defeat"]["covered"]:
            break
    if boss_clear_replay is not None:
        boss_clear_path = args.artifacts_dir / "boss-clear.replay.json"
        save_replay(boss_clear_path, boss_clear_replay)
        report["artifacts"]["boss_clear_replay"] = str(boss_clear_path)

    if not report["targets"]["consumable_visible"]["covered"]:
        report["notes"].append("engine never surfaced shop_consumables/consumables in snapshots")
    if not report["targets"]["consumable_slot_enabled"]["covered"]:
        report["notes"].append("engine never enabled use_consumable_* actions")
    if not report["targets"]["consumable_use"]["covered"]:
        report["notes"].append("engine never executed a consumable use path")

    required = [
        "shop_cashout",
        "shop_buy",
        "shop_reroll",
        "shop_sell",
        "shop_next_round",
        "boss_select",
        "boss_enter",
        "boss_defeat",
        "consumable_visible",
        "consumable_slot_enabled",
        "consumable_use",
    ]
    report["ok"] = fully_covered(report["targets"], required)

    rendered = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")
    print(f"wrote {args.output}")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
