#!/usr/bin/env python3
"""LLM Agent Evaluation Harness.

Evaluates any LLM-based Balatro agent across N games and produces a
structured evaluation report. Supports multiple agent backends.

Architecture:
    ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
    │   Engine     │────>│  Serializer  │────>│  LLM Agent  │
    │ (balatro_    │     │  (state →    │     │  (reason +  │
    │  native)     │<────│   text)      │<────│   action)   │
    └─────────────┘     └──────────────┘     └─────────────┘
                                                    │
                              ┌──────────────────────┘
                              │
                    ┌─────────▼─────────┐
                    │  Agent Backends    │
                    ├───────────────────┤
                    │ HeuristicAgent    │  Built-in, fast, no API
                    │ ClaudeAgent       │  Claude API + CoT
                    │ LocalLMAgent      │  Local model (vLLM/HF)
                    │ CustomAgent       │  User-defined
                    └───────────────────┘

Usage:
    # Evaluate built-in heuristic:
    python scripts/eval_llm_agent.py --agent heuristic --games 100

    # Evaluate Claude API:
    python scripts/eval_llm_agent.py --agent claude --games 20

    # Evaluate local model via OpenAI-compatible API:
    python scripts/eval_llm_agent.py --agent local --api-base http://localhost:8000/v1 --games 50

    # Compare two agents:
    python scripts/eval_llm_agent.py --agent heuristic --games 100 --tag baseline
    python scripts/eval_llm_agent.py --agent claude --games 20 --tag claude_v1
    python scripts/eval_llm_agent.py --compare results/eval/baseline results/eval/claude_v1
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Protocol

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import balatro_native
from env.state_serializer import serialize_state, serialize_for_llm_prompt


# ---------------------------------------------------------------------------
# Agent Protocol
# ---------------------------------------------------------------------------

class BalatroAgent(Protocol):
    """Protocol for any LLM-based Balatro agent."""

    def decide(self, snapshot_dict: dict, legal_actions: list[str]) -> tuple[str, str]:
        """Given game state and legal actions, return (action_name, reasoning)."""
        ...

    @property
    def name(self) -> str: ...

    @property
    def stats(self) -> dict[str, Any]: ...


# ---------------------------------------------------------------------------
# Evaluation Runner
# ---------------------------------------------------------------------------

def evaluate_game(agent: Any, seed: int, max_steps: int = 1000) -> dict:
    """Run a single evaluation game. Returns metrics dict."""
    eng = balatro_native.Engine(seed=seed, stake=1)
    step = 0
    selected_set: set[int] = set()
    blinds_cleared = 0
    actions_taken: dict[str, int] = {}
    play_scores: list[int] = []

    while not eng.is_over and step < max_steps:
        snap_dict = json.loads(eng.snapshot().to_json())
        snap_dict["selected_slots"] = sorted(selected_set)

        acts = [a for a in eng.legal_actions() if a.enabled]
        legal = [a.name for a in acts]
        if not legal:
            break

        action_name, _ = agent.decide(snap_dict, legal)

        # Track toggles
        if action_name.startswith("select_card_"):
            idx = int(action_name.split("_")[-1])
            selected_set.symmetric_difference_update({idx})
        elif action_name in ("play", "discard", "cashout", "next_round") or action_name.startswith("select_blind"):
            if action_name == "play":
                old_score = snap_dict.get("score", 0)
            selected_set.clear()

        # Execute
        for a in acts:
            if a.name == action_name:
                eng.step(a.index)
                break
        else:
            eng.step(acts[0].index)
            action_name = acts[0].name

        # Track metrics
        actions_taken[action_name] = actions_taken.get(action_name, 0) + 1

        new_dict = json.loads(eng.snapshot().to_json())
        if action_name == "play":
            score_delta = new_dict.get("score", 0) - old_score
            play_scores.append(score_delta)
        if snap_dict.get("stage") == "Stage_Blind" and new_dict.get("stage") == "Stage_PostBlind":
            blinds_cleared += 1

        step += 1

    final = json.loads(eng.snapshot().to_json())
    return {
        "seed": seed,
        "won": bool(eng.is_win),
        "final_ante": final.get("ante", 1),
        "final_money": final.get("money", 0),
        "steps": step,
        "blinds_cleared": blinds_cleared,
        "play_count": actions_taken.get("play", 0),
        "discard_count": actions_taken.get("discard", 0),
        "mean_play_score": sum(play_scores) / len(play_scores) if play_scores else 0,
        "max_play_score": max(play_scores) if play_scores else 0,
        "action_distribution": actions_taken,
    }


def run_evaluation(agent: Any, seeds: list[int], max_steps: int = 1000,
                   verbose: bool = False) -> dict:
    """Run full evaluation across multiple seeds."""
    results = []
    t_start = time.monotonic()

    for i, seed in enumerate(seeds):
        result = evaluate_game(agent, seed, max_steps)
        results.append(result)

        if verbose:
            status = "WIN" if result["won"] else f"Ante {result['final_ante']}"
            print(f"  [{i+1}/{len(seeds)}] seed={seed} → {status} "
                  f"({result['steps']}st, {result['blinds_cleared']}bl, "
                  f"{result['play_count']}pl)")

    elapsed = time.monotonic() - t_start

    # Aggregate metrics
    antes = [r["final_ante"] for r in results]
    wins = sum(1 for r in results if r["won"])
    blinds = [r["blinds_cleared"] for r in results]
    play_scores = [r["mean_play_score"] for r in results if r["mean_play_score"] > 0]

    report = {
        "agent_name": getattr(agent, "name", "unknown"),
        "agent_stats": getattr(agent, "stats", {}),
        "num_games": len(seeds),
        "seeds": seeds,
        "elapsed_s": round(elapsed, 1),
        "games_per_sec": round(len(seeds) / elapsed, 2) if elapsed > 0 else 0,
        "metrics": {
            "win_rate": round(wins / len(seeds), 4) if seeds else 0,
            "wins": wins,
            "mean_ante": round(sum(antes) / len(antes), 2) if antes else 0,
            "max_ante": max(antes) if antes else 0,
            "median_ante": sorted(antes)[len(antes)//2] if antes else 0,
            "ante_distribution": {str(a): antes.count(a) for a in sorted(set(antes))},
            "mean_blinds_cleared": round(sum(blinds) / len(blinds), 2) if blinds else 0,
            "mean_play_score": round(sum(play_scores) / len(play_scores), 1) if play_scores else 0,
            "mean_steps": round(sum(r["steps"] for r in results) / len(results), 0) if results else 0,
        },
        "per_game": results,
    }
    return report


# ---------------------------------------------------------------------------
# Comparison
# ---------------------------------------------------------------------------

def compare_reports(paths: list[Path]) -> None:
    """Compare multiple evaluation reports side by side."""
    reports = []
    for p in paths:
        rpath = p / "eval_report.json" if p.is_dir() else p
        reports.append(json.loads(rpath.read_text()))

    print(f"\n{'Metric':<25}", end="")
    for r in reports:
        print(f"  {r['agent_name']:<15}", end="")
    print()
    print("-" * (25 + 17 * len(reports)))

    metrics_to_show = ["win_rate", "mean_ante", "max_ante", "mean_blinds_cleared",
                       "mean_play_score", "mean_steps"]
    for m in metrics_to_show:
        print(f"{m:<25}", end="")
        for r in reports:
            val = r["metrics"].get(m, "?")
            if isinstance(val, float):
                print(f"  {val:<15.3f}", end="")
            else:
                print(f"  {str(val):<15}", end="")
        print()


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Evaluate LLM Balatro agent")
    parser.add_argument("--agent", choices=["heuristic", "smart", "claude", "local"], default="smart")
    parser.add_argument("--model", type=str, default="claude-sonnet-4-20250514")
    parser.add_argument("--api-base", type=str, default=None,
                        help="OpenAI-compatible API base URL for local models")
    parser.add_argument("--games", type=int, default=50)
    parser.add_argument("--start-seed", type=int, default=0)
    parser.add_argument("--max-steps", type=int, default=1000)
    parser.add_argument("--tag", type=str, default=None)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "results" / "eval")
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument("--compare", nargs="+", type=Path, default=None,
                        help="Compare existing eval reports")
    args = parser.parse_args()

    if args.compare:
        compare_reports(args.compare)
        return

    # Create agent
    if args.agent == "claude":
        from scripts.collect_llm_trajectories import ClaudeAgent
        agent = ClaudeAgent(model=args.model)
        agent.name = f"claude_{args.model.split('-')[1]}"
        agent.stats = {"model": args.model}
    elif args.agent == "smart":
        from agents.smart_agent import SmartAgent
        agent = SmartAgent()
    elif args.agent == "local":
        print("Local model agent not yet implemented. Use --agent smart or claude.")
        sys.exit(1)
    else:
        from scripts.collect_llm_trajectories import HeuristicAgent
        agent = HeuristicAgent()
        agent.name = "heuristic"
        agent.stats = {}

    seeds = list(range(args.start_seed, args.start_seed + args.games))
    tag = args.tag or agent.name
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    eval_dir = args.output_dir / f"{tag}_{ts}"
    eval_dir.mkdir(parents=True, exist_ok=True)

    print(f"Evaluating: {agent.name}")
    print(f"Games: {args.games} (seeds {args.start_seed}..{args.start_seed + args.games - 1})")
    print(f"Output: {eval_dir}")
    print()

    report = run_evaluation(agent, seeds, args.max_steps, args.verbose)

    # Save
    (eval_dir / "eval_report.json").write_text(json.dumps(report, indent=2))

    m = report["metrics"]
    print()
    print("=" * 50)
    print(f"EVALUATION: {agent.name}")
    print("=" * 50)
    print(f"  Win rate:       {m['win_rate']:.1%} ({m['wins']}/{report['num_games']})")
    print(f"  Mean ante:      {m['mean_ante']:.2f}")
    print(f"  Max ante:       {m['max_ante']}")
    print(f"  Ante dist:      {m['ante_distribution']}")
    print(f"  Blinds cleared: {m['mean_blinds_cleared']:.2f}/game")
    print(f"  Mean play score:{m['mean_play_score']:.1f}")
    print(f"  Time:           {report['elapsed_s']:.1f}s ({report['games_per_sec']:.1f} g/s)")
    print(f"  Report:         {eval_dir / 'eval_report.json'}")


if __name__ == "__main__":
    main()
