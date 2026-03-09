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
from typing import Any

from agents.greedy_agent import GreedyAgent
from agents.random_agent import RandomAgent
from env.balatro_gym_wrapper import BalatroEnv
from eval.compare_baselines import compare_agents
from training.pipeline import run_phase1, run_phase2
from utils.config import load_yaml, with_strategy
from utils.reporting import write_csv, write_json


def _load_config(path: str, strategy: str | None) -> dict[str, Any]:
    base = load_yaml(path)
    return with_strategy(base, strategy_name=strategy)


def _env_factory(config: dict[str, Any]):
    def _factory(seed: int):
        env = BalatroEnv(config)
        env.reset(seed=seed)
        return env

    return _factory


def _load_seeds(config: dict[str, Any]) -> list[int]:
    path = Path(config.get("eval", {}).get("seeds_file", "eval/seeds.json"))
    with path.open("r", encoding="utf-8") as f:
        return [int(x) for x in json.load(f)]


def cmd_phase1(args: argparse.Namespace) -> int:
    config = _load_config(args.config, args.strategy)
    result = run_phase1(config=config, run_id=args.run_id)
    print(json.dumps(result.__dict__, indent=2, ensure_ascii=True))
    return 0 if result.ok else 2


def cmd_phase2(args: argparse.Namespace) -> int:
    config = _load_config(args.config, args.strategy)
    strategy = config.get("selected_strategy", args.strategy or config.get("strategy", {}).get("default", "strategy_stable"))
    result = run_phase2(
        config=config,
        strategy_name=str(strategy),
        run_id=args.run_id,
        resume_path=args.resume,
    )
    print(json.dumps(result.__dict__, indent=2, ensure_ascii=True))
    return 0 if result.ok else 3


def cmd_eval(args: argparse.Namespace) -> int:
    config = _load_config(args.config, args.strategy)
    seeds = _load_seeds(config)[: args.episodes]
    env_factory = _env_factory(config)

    agents = {
        "Random": RandomAgent(seed=42),
        "Greedy": GreedyAgent(),
    }

    if args.ppo_checkpoint:
        from agents.ppo_agent import PPOAgent

        model_type = config.get("model", {}).get("type", "mlp")
        ppo = PPOAgent(model_type=model_type)
        ppo.load(args.ppo_checkpoint, strict=False)
        agents["PPO"] = ppo

    results = compare_agents(agents=agents, env_factory=env_factory, seeds=seeds, episodes_per_seed=1)

    out_dir = Path(args.output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    write_json(out_dir / "metrics.json", results)
    write_csv(
        out_dir / "comparison.csv",
        [
            {
                "agent": name,
                "win_rate": metrics["win_rate"],
                "avg_blinds_passed": metrics["avg_blinds_passed"],
                "avg_episode_reward": metrics["avg_episode_reward"],
                "max_win_streak": metrics["max_win_streak"],
            }
            for name, metrics in results.items()
        ],
        ["agent", "win_rate", "avg_blinds_passed", "avg_episode_reward", "max_win_streak"],
    )

    print(json.dumps(results, indent=2, ensure_ascii=True))
    return 0


def cmd_report(args: argparse.Namespace) -> int:
    metrics_path = Path(args.metrics)
    if not metrics_path.exists():
        print(f"Missing metrics file: {metrics_path}")
        return 1

    data = json.loads(metrics_path.read_text(encoding="utf-8"))
    rows = []
    for name, metrics in data.items():
        rows.append(
            {
                "agent": name,
                "win_rate": metrics.get("win_rate", 0.0),
                "avg_blinds_passed": metrics.get("avg_blinds_passed", 0.0),
                "avg_episode_reward": metrics.get("avg_episode_reward", 0.0),
                "max_win_streak": metrics.get("max_win_streak", 0),
            }
        )

    out = Path(args.output)
    write_csv(
        out,
        rows,
        ["agent", "win_rate", "avg_blinds_passed", "avg_episode_reward", "max_win_streak"],
    )
    print(f"wrote {out}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Balatro rebuild reproduction CLI")

    sub = parser.add_subparsers(dest="command", required=True)

    p1 = sub.add_parser("phase1", help="Run phase1 doctor + baseline evaluation")
    p1.add_argument("--config", default="configs/repro.yaml")
    p1.add_argument("--strategy", default=None)
    p1.add_argument("--run-id", default=None)
    p1.set_defaults(func=cmd_phase1)

    p2 = sub.add_parser("phase2", help="Run BC + PPO pipeline")
    p2.add_argument("--config", default="configs/repro.yaml")
    p2.add_argument("--strategy", default=None)
    p2.add_argument("--run-id", default=None)
    p2.add_argument("--resume", default=None)
    p2.set_defaults(func=cmd_phase2)

    resume = sub.add_parser("resume", help="Alias of phase2 with resume checkpoint")
    resume.add_argument("--config", default="configs/repro.yaml")
    resume.add_argument("--strategy", default=None)
    resume.add_argument("--run-id", default=None)
    resume.add_argument("--resume", required=True)
    resume.set_defaults(func=cmd_phase2)

    pe = sub.add_parser("eval", help="Evaluate agents and generate report files")
    pe.add_argument("--config", default="configs/repro.yaml")
    pe.add_argument("--strategy", default=None)
    pe.add_argument("--episodes", type=int, default=100)
    pe.add_argument("--ppo-checkpoint", default=None)
    pe.add_argument("--output-dir", default="results/eval")
    pe.set_defaults(func=cmd_eval)

    pr = sub.add_parser("report", help="Render CSV report from metrics JSON")
    pr.add_argument("--config", default="configs/repro.yaml")
    pr.add_argument("--metrics", required=True)
    pr.add_argument("--output", default="results/report.csv")
    pr.set_defaults(func=cmd_report)

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    code = args.func(args)
    raise SystemExit(code)


if __name__ == "__main__":
    main()
