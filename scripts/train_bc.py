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

from agents.ppo_agent import PPOAgent
from training.behavior_clone import BehaviorCloner
from utils.config import load_yaml, with_strategy


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="configs/repro.yaml")
    parser.add_argument("--strategy", default=None)
    parser.add_argument("--trajectories", required=True)
    parser.add_argument("--epochs", type=int, default=50)
    parser.add_argument("--output", default="checkpoints/bc_pretrained.pt")
    args = parser.parse_args()

    config = with_strategy(load_yaml(args.config), args.strategy)
    model_type = config.get("model", {}).get("type", "mlp")
    agent = PPOAgent(model_type=model_type)

    bc_cfg = config.get("training", {}).get("behavior_clone", {})
    cloner = BehaviorCloner(
        agent,
        lr=float(bc_cfg.get("lr", 1e-3)),
        batch_size=int(bc_cfg.get("batch_size", 256)),
    )
    result = cloner.train(args.trajectories, num_epochs=args.epochs)

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    torch.save({"model_state": agent.model.state_dict(), "model_type": model_type}, output)

    print(
        {
            "output": str(output),
            "final_loss": result.final_loss,
            "samples": result.samples,
            "epochs": result.epochs,
        }
    )


if __name__ == "__main__":
    main()
