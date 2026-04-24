#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import argparse
import json

from agents.greedy_agent import GreedyAgent
from env.legacy.balatro_gym_wrapper import BalatroEnv
from eval.eval_policy import evaluate_agent
from utils.config import load_yaml, with_strategy


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="configs/legacy/repro.yaml")
    parser.add_argument("--strategy", default=None)
    parser.add_argument("--num-games", type=int, default=200)
    parser.add_argument("--until-win", action="store_true")
    args = parser.parse_args()

    config = with_strategy(load_yaml(args.config), args.strategy)
    seeds = list(range(args.num_games if not args.until_win else 5000))

    def env_factory(seed: int):
        env = BalatroEnv(config)
        env.reset(seed=seed)
        return env

    metrics = evaluate_agent(GreedyAgent(), env_factory, seeds, episodes_per_seed=1)
    print(json.dumps(metrics, indent=2, ensure_ascii=True))

    if args.until_win and metrics["wins"] <= 0:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
