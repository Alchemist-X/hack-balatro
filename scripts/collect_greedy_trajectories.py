#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import argparse
from pathlib import Path

import torch

from agents.greedy_agent import GreedyAgent
from env.balatro_gym_wrapper import BalatroEnv
from training.behavior_clone import collect_greedy_trajectories
from utils.config import load_yaml, with_strategy


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="configs/repro.yaml")
    parser.add_argument("--strategy", default=None)
    parser.add_argument("--num-games", type=int, default=5000)
    parser.add_argument("--output", default="trajectories/greedy.pt")
    args = parser.parse_args()

    config = with_strategy(load_yaml(args.config), args.strategy)

    def env_factory(seed: int):
        env = BalatroEnv(config)
        env.reset(seed=seed)
        return env

    data = collect_greedy_trajectories(
        env_factory=env_factory,
        expert_agent=GreedyAgent(),
        num_games=args.num_games,
    )
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    torch.save(data, output)
    print(f"saved trajectories: {output} ({data['observations'].shape[0]} samples)")


if __name__ == "__main__":
    main()
