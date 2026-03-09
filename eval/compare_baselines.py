from __future__ import annotations

from typing import Any, Callable

from eval.eval_policy import evaluate_agent


def compare_agents(
    agents: dict[str, Any],
    env_factory: Callable[[int], Any],
    seeds: list[int],
    episodes_per_seed: int = 1,
) -> dict[str, dict[str, Any]]:
    results: dict[str, dict[str, Any]] = {}
    for name, agent in agents.items():
        results[name] = evaluate_agent(
            agent=agent,
            env_factory=env_factory,
            seeds=seeds,
            episodes_per_seed=episodes_per_seed,
        )
    return results
