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
from agents.ppo_agent import PPOAgent
from agents.random_agent import RandomAgent
from env.legacy.balatro_gym_wrapper import BalatroEnv
from eval.eval_policy import evaluate_agent
from utils.config import load_yaml, with_strategy


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="configs/legacy/repro.yaml")
    parser.add_argument("--strategy", default=None)
    parser.add_argument("--agent", choices=["random", "greedy", "ppo"], required=True)
    parser.add_argument("--checkpoint", default=None)
    parser.add_argument("--episodes", type=int, default=100)
    args = parser.parse_args()

    config = with_strategy(load_yaml(args.config), args.strategy)
    with open(config.get("eval", {}).get("seeds_file", "eval/seeds.json"), "r", encoding="utf-8") as f:
        seeds = json.load(f)

    def env_factory(seed: int):
        env = BalatroEnv(config)
        env.reset(seed=seed)
        return env

    if args.agent == "random":
        agent = RandomAgent(seed=42)
    elif args.agent == "greedy":
        agent = GreedyAgent()
    else:
        model_type = config.get("model", {}).get("type", "mlp")
        agent = PPOAgent(model_type=model_type)
        if args.checkpoint:
            agent.load(args.checkpoint, strict=False)

    metrics = evaluate_agent(agent, env_factory, seeds[: args.episodes], episodes_per_seed=1)
    print(json.dumps(metrics, indent=2, ensure_ascii=True))


if __name__ == "__main__":
    main()
