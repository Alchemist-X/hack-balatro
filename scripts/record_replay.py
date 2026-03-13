#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import random
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Record a structured replay from balatro_native")
    parser.add_argument("--output", type=Path, default=Path("results/replay.json"))
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--max-steps", type=int, default=128)
    parser.add_argument("--policy", choices=["first", "random"], default="first")
    parser.add_argument("--ruleset-path", type=str, default=None)
    parser.add_argument("--stake", type=int, default=1)
    return parser.parse_args()


def choose_action(engine, policy: str, rng: random.Random) -> int:
    legal = [action for action in engine.legal_actions() if action.enabled]
    if not legal:
        return 0
    if policy == "random":
        return rng.choice(legal).index
    return legal[0].index


def main() -> int:
    args = parse_args()
    import balatro_native

    engine = balatro_native.Engine(seed=args.seed, ruleset_path=args.ruleset_path, stake=args.stake)
    bundle_path = args.ruleset_path or balatro_native.default_ruleset_path()
    bundle = json.loads(Path(bundle_path).read_text())

    transitions = []
    rng = random.Random(args.seed)
    step = 0
    while not engine.is_over and step < args.max_steps:
        action = choose_action(engine, args.policy, rng)
        transition = json.loads(engine.step(action).to_json())
        after = transition["snapshot_after"]
        transition["ui_asset_refs"] = {
            "blind_name": after["blind_name"],
            "joker_ids": [joker["joker_id"] for joker in after["jokers"]],
            "shop_joker_ids": [joker["joker_id"] for joker in after["shop_jokers"]],
        }
        transitions.append(transition)
        step += 1

    replay = {
        "version": bundle["metadata"]["version"],
        "engine": "balatro_native",
        "seed": args.seed,
        "policy": args.policy,
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
    print(f"wrote {args.output}")
    print(f"  transitions: {len(transitions)}")
    print(f"  final stage: {replay['final_snapshot']['stage']}")
    print(f"  won: {replay['final_snapshot']['won']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
