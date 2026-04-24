#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import argparse
import json
from pathlib import Path

import numpy as np

from env.legacy.action_space import ACTION_DIM
from env.legacy.balatro_gym_wrapper import BalatroEnv
from env.legacy.state_encoder import OBS_DIM
from utils.config import load_yaml, with_strategy
from utils.reporting import write_json


def run_doctor(
    config_path: str = "configs/legacy/repro.yaml",
    output_path: str | None = None,
    config: dict | None = None,
) -> dict:
    if config is None:
        config = with_strategy(load_yaml(config_path))

    checks: dict[str, dict] = {}
    env = BalatroEnv(config)

    obs, info = env.reset(seed=int(config.get("env", {}).get("seed", 42)))
    checks["engine_backend"] = {
        "ok": True,
        "backend": info.get("engine_backend", "unknown"),
    }

    mask = env.get_action_mask()
    checks["obs_action_dims"] = {
        "ok": bool(obs.shape == (OBS_DIM,) and mask.shape == (ACTION_DIM,)),
        "obs_dim": int(obs.shape[0]),
        "action_dim": int(mask.shape[0]),
        "expected_obs_dim": OBS_DIM,
        "expected_action_dim": ACTION_DIM,
    }

    stages = {str(info.get("stage", "unknown"))}
    terminated = False
    truncated = False
    steps = 0
    while not (terminated or truncated) and steps < 200:
        mask = env.get_action_mask()
        legal = np.where(mask)[0]
        stage_name = str(info.get("stage", ""))
        action = int(legal[0]) if legal.size else 0

        if "PreBlind" in stage_name:
            for candidate in (10, 11, 12, 85):
                if candidate < mask.size and mask[candidate]:
                    action = candidate
                    break
        elif "Blind" in stage_name:
            for candidate in (8, 9):
                if candidate < mask.size and mask[candidate]:
                    action = candidate
                    break
        elif "PostBlind" in stage_name:
            if 13 < mask.size and mask[13]:
                action = 13
        elif "Shop" in stage_name:
            for candidate in (70, 79):
                if candidate < mask.size and mask[candidate]:
                    action = candidate
                    break

        obs, reward, terminated, truncated, info = env.step(action)
        del reward, obs
        stages.add(str(info.get("stage", "unknown")))
        steps += 1

    checks["stage_flow"] = {
        "ok": len(stages) >= 3,
        "observed_stages": sorted(stages),
        "num_observed": len(stages),
    }

    seeds_file = Path(config.get("eval", {}).get("seeds_file", "eval/seeds.json"))
    try:
        seeds = json.loads(seeds_file.read_text(encoding="utf-8"))
    except Exception as exc:
        checks["seeds_file"] = {
            "ok": False,
            "path": str(seeds_file),
            "error": str(exc),
        }
    else:
        checks["seeds_file"] = {
            "ok": isinstance(seeds, list) and len(seeds) >= 100,
            "path": str(seeds_file),
            "count": len(seeds) if isinstance(seeds, list) else 0,
        }

    all_ok = all(entry.get("ok", False) for entry in checks.values())
    result = {
        "ok": all_ok,
        "checks": checks,
    }

    if output_path:
        write_json(output_path, result)

    return result


def main() -> None:
    parser = argparse.ArgumentParser(description="Run environment doctor checks")
    parser.add_argument("--config", default="configs/legacy/repro.yaml")
    parser.add_argument("--output", default=None)
    args = parser.parse_args()

    result = run_doctor(args.config, args.output)
    print(json.dumps(result, indent=2, ensure_ascii=True))

    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
