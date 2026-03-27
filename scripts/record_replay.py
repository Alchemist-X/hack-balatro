#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import random
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from agents.simple_rule_agent import SimpleRuleAgent

try:
    from scripts.behavior_log import (
        LOG_METADATA,
        TEST_FOCUS,
        build_behavior_log_record,
        build_decision_log,
        choose_policy_action,
    )
except ModuleNotFoundError:
    from behavior_log import (  # type: ignore
        LOG_METADATA,
        TEST_FOCUS,
        build_behavior_log_record,
        build_decision_log,
        choose_policy_action,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Record a structured replay from balatro_native")
    parser.add_argument("--output", type=Path, default=Path("results/replay.json"))
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--max-steps", type=int, default=128)
    parser.add_argument("--policy", choices=["first", "random", "simple_rule_v1"], default="first")
    parser.add_argument("--ruleset-path", type=str, default=None)
    parser.add_argument("--stake", type=int, default=1)
    parser.add_argument("--behavior-log-output", type=Path, default=None)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    import balatro_native

    engine = balatro_native.Engine(seed=args.seed, ruleset_path=args.ruleset_path, stake=args.stake)
    bundle_path = args.ruleset_path or balatro_native.default_ruleset_path()
    bundle = json.loads(Path(bundle_path).read_text())
    agent = SimpleRuleAgent(bundle_path) if args.policy == "simple_rule_v1" else None
    behavior_log_output = args.behavior_log_output
    if behavior_log_output is None:
        behavior_log_output = args.output.with_suffix(".behavior_log.jsonl")

    transitions = []
    behavior_records = []
    rng = random.Random(args.seed)
    started_at = datetime.now(timezone.utc).isoformat()
    session_id = f"replay-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}-{args.seed}-{uuid.uuid4().hex[:8]}"
    clock_start = time.perf_counter()
    step = 0
    while not engine.is_over and step < args.max_steps:
        snapshot_before = json.loads(engine.snapshot().to_json())
        legal_actions = [
            {"index": action.index, "name": action.name, "enabled": action.enabled}
            for action in engine.legal_actions()
        ]
        if agent is not None:
            action, plan = agent.choose_action(snapshot_before, legal_actions)
        else:
            action, plan = choose_policy_action(
                snapshot_before,
                legal_actions,
                bundle,
                args.policy,
                rng,
            )
        transition = json.loads(engine.step(action).to_json())
        after = transition["snapshot_after"]
        elapsed_ms = int((time.perf_counter() - clock_start) * 1000)
        transition["step_index"] = step
        transition["elapsed_ms"] = elapsed_ms
        if plan is not None:
            transition["decision_log"] = build_decision_log(
                transition["snapshot_before"],
                after,
                transition.get("events", []),
                plan,
            )
        transition["ui_asset_refs"] = {
            "blind_name": after["blind_name"],
            "joker_ids": [joker["joker_id"] for joker in after["jokers"]],
            "shop_joker_ids": [joker["joker_id"] for joker in after["shop_jokers"]],
        }
        behavior_records.append(
            build_behavior_log_record(
                seed=args.seed,
                step_index=step,
                elapsed_ms=elapsed_ms,
                transition=transition,
                decision_log=transition.get("decision_log"),
                policy_id=args.policy,
                started_at=started_at,
                finished_at=None,
                test_focus=TEST_FOCUS,
            )
        )
        transitions.append(transition)
        step += 1
    finished_at = datetime.now(timezone.utc).isoformat()
    for record in behavior_records:
        record["finished_at"] = finished_at

    replay = {
        "version": bundle["metadata"]["version"],
        "engine": "balatro_native",
        "seed": args.seed,
        "policy": args.policy,
        "test_metadata": {
            "session_id": session_id,
            "test_focus": TEST_FOCUS,
            "started_at": started_at,
            "finished_at": finished_at,
        },
        "log_metadata": {
            "policy_id": args.policy,
            **LOG_METADATA,
        },
        "ruleset_path": bundle_path,
        "asset_root": "../results/assets-preview/",
        "sprite_manifest": bundle.get("sprite_manifest", {}),
        "sprite_index": {
            "jokers": {
                joker["id"]: joker.get("sprite")
                for joker in bundle.get("jokers", [])
                if joker.get("sprite")
            },
            "blinds_by_name": {
                blind["name"]: blind.get("sprite")
                for blind in bundle.get("blinds", [])
                if blind.get("sprite")
            },
        },
        "transitions": transitions,
        "final_snapshot": json.loads(engine.snapshot().to_json()),
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(replay, ensure_ascii=True, indent=2) + "\n")
    behavior_log_output.parent.mkdir(parents=True, exist_ok=True)
    behavior_log_output.write_text(
        "".join(json.dumps(record, ensure_ascii=True) + "\n" for record in behavior_records),
        encoding="utf-8",
    )
    print(f"wrote {args.output}")
    print(f"wrote {behavior_log_output}")
    print(f"  transitions: {len(transitions)}")
    print(f"  final stage: {replay['final_snapshot']['stage']}")
    print(f"  won: {replay['final_snapshot']['won']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
