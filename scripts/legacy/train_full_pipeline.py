#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from utils.config import load_yaml, with_strategy
from legacy.training.pipeline import run_phase1, run_phase2


def main() -> None:
    config = with_strategy(load_yaml("configs/legacy/repro.yaml"))
    phase1 = run_phase1(config)
    print("phase1", phase1)
    if not phase1.ok:
        raise SystemExit(2)

    strategy = config.get("selected_strategy", config.get("strategy", {}).get("default", "strategy_stable"))
    phase2 = run_phase2(config, strategy_name=str(strategy))
    print("phase2", phase2)
    raise SystemExit(0 if phase2.ok else 3)


if __name__ == "__main__":
    main()
