from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Iterable

import numpy as np


@dataclass
class EpisodeResult:
    seed: int
    game_won: bool
    blinds_passed: int
    episode_reward: float
    episode_length: int


def _extract_action(output: Any) -> int:
    if isinstance(output, tuple):
        return int(output[0])
    return int(output)


def _streak_stats(wins: list[bool]) -> tuple[int, float, dict[str, int]]:
    streaks = []
    cur = 0
    for w in wins:
        if w:
            cur += 1
        elif cur > 0:
            streaks.append(cur)
            cur = 0
    if cur > 0:
        streaks.append(cur)

    max_streak = max(streaks) if streaks else 0
    avg_streak = float(np.mean(streaks)) if streaks else 0.0

    dist = {"1-2": 0, "3-4": 0, "5-9": 0, "10+": 0}
    for s in streaks:
        if s <= 2:
            dist["1-2"] += 1
        elif s <= 4:
            dist["3-4"] += 1
        elif s <= 9:
            dist["5-9"] += 1
        else:
            dist["10+"] += 1

    return max_streak, avg_streak, dist


def run_episode(agent: Any, env: Any, max_steps: int = 2000) -> EpisodeResult:
    obs, info = env.reset()
    terminated = False
    truncated = False
    episode_reward = 0.0
    steps = 0

    while not (terminated or truncated) and steps < max_steps:
        action_mask = env.get_action_mask()
        action = _extract_action(agent.act(obs, info, action_mask))
        obs, reward, terminated, truncated, info = env.step(action)
        episode_reward += float(reward)
        steps += 1

    return EpisodeResult(
        seed=int(info.get("seed", -1)),
        game_won=bool(info.get("game_won", False)),
        blinds_passed=int(info.get("blinds_passed", 0)),
        episode_reward=float(episode_reward),
        episode_length=int(steps),
    )


def evaluate_agent(
    agent: Any,
    env_factory: Callable[[int], Any],
    seeds: Iterable[int],
    episodes_per_seed: int = 1,
    max_steps: int = 2000,
) -> dict[str, Any]:
    results: list[EpisodeResult] = []

    for seed in seeds:
        for _ in range(episodes_per_seed):
            env = env_factory(int(seed))
            obs, info = env.reset(seed=int(seed))
            del obs, info
            episode = run_episode(agent, env, max_steps=max_steps)
            episode.seed = int(seed)
            results.append(episode)

    wins = [r.game_won for r in results]
    blinds = np.asarray([r.blinds_passed for r in results], dtype=np.float32)
    rewards = np.asarray([r.episode_reward for r in results], dtype=np.float32)
    lengths = np.asarray([r.episode_length for r in results], dtype=np.float32)

    max_streak, avg_streak, streak_distribution = _streak_stats(wins)

    return {
        "episodes": len(results),
        "wins": int(sum(wins)),
        "win_rate": float(np.mean(wins) if wins else 0.0),
        "avg_blinds_passed": float(blinds.mean() if blinds.size else 0.0),
        "std_blinds_passed": float(blinds.std() if blinds.size else 0.0),
        "avg_episode_length": float(lengths.mean() if lengths.size else 0.0),
        "avg_episode_reward": float(rewards.mean() if rewards.size else 0.0),
        "max_win_streak": int(max_streak),
        "avg_win_streak": float(avg_streak),
        "streak_distribution": streak_distribution,
        "raw": [r.__dict__ for r in results],
    }
