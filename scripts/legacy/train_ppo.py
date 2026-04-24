#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import argparse

from legacy.training.pipeline import run_phase2
from utils.config import load_yaml, with_strategy


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="configs/legacy/repro.yaml")
    parser.add_argument("--strategy", default=None)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--resume", default=None)
    args = parser.parse_args()

    config = with_strategy(load_yaml(args.config), args.strategy)
    strategy = config.get("selected_strategy", args.strategy or "strategy_stable")

    result = run_phase2(
        config=config,
        strategy_name=str(strategy),
        run_id=args.run_id,
        resume_path=args.resume,
    )
    print(result)


if __name__ == "__main__":
    main()
