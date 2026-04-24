#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import random
import sys
import time
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from agents.simple_rule_agent import SimpleRuleAgent
from env.legacy.balatro_gym_wrapper import ParallelBalatroEnvs
from scripts.behavior_log import (
    LOG_METADATA,
    TEST_FOCUS,
    build_behavior_log_record,
    build_decision_log,
)


@dataclass
class EpisodeBuffer:
    seed: int
    started_at: str
    start_perf: float
    episode_reward: float = 0.0
    transitions: list[dict[str, Any]] = field(default_factory=list)
    behavior_records: list[dict[str, Any]] = field(default_factory=list)
    final_info: dict[str, Any] | None = None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run timestamped simple_rule_v1 coverage batches")
    parser.add_argument("--num-envs", type=int, choices=[5, 10], default=5)
    parser.add_argument("--episodes", type=int, default=5)
    parser.add_argument("--max-steps", type=int, default=512)
    parser.add_argument("--stake", type=int, default=1)
    parser.add_argument("--ruleset-path", type=str, default=None)
    parser.add_argument("--results-dir", type=Path, default=Path("results/coverage"))
    parser.add_argument("--render-html-count", type=int, default=1)
    parser.add_argument("--base-seed", type=int, default=None)
    parser.add_argument("--force-mock", action="store_true")
    return parser.parse_args()


def session_id(prefix: str) -> str:
    return f"{prefix}-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}-{uuid.uuid4().hex[:8]}"


def sprite_index(bundle: dict[str, Any]) -> dict[str, Any]:
    return {
        "jokers": {
            joker["id"]: joker.get("sprite")
            for joker in bundle.get("jokers", [])
            if joker.get("sprite")
        },
        "blinds_by_name": {
            blind["name"]: blind.get("sprite")
            for blind in bundle.get("blinds", [])
            if blind.get("sprite")
        },
    }


def render_replay_html(viewer_path: Path, replay: dict[str, Any], output_path: Path) -> None:
    html = viewer_path.read_text()
    injection = (
        "<script>\n"
        f"window.__REPLAY__ = {json.dumps(replay, ensure_ascii=False)};\n"
        "window.__AUTO_PLAY__ = true;\n"
        "</script>\n"
    )
    output_path.write_text(html.replace("</head>", f"{injection}</head>", 1), encoding="utf-8")


def random_seeds(count: int, base_seed: int | None) -> list[int]:
    rng = random.Random(base_seed if base_seed is not None else random.SystemRandom().randrange(1, 2**31 - 1))
    seeds: list[int] = []
    seen: set[int] = set()
    while len(seeds) < count:
        candidate = rng.randrange(1, 2**31 - 1)
        if candidate in seen:
            continue
        seen.add(candidate)
        seeds.append(candidate)
    return seeds


def summarize(entries: list[dict[str, Any]]) -> dict[str, Any]:
    if not entries:
        return {
            "episodes": 0,
            "wins": 0,
            "win_rate": 0.0,
            "avg_blinds_passed": 0.0,
            "avg_episode_reward": 0.0,
            "avg_episode_length": 0.0,
        }
    wins = sum(1 for entry in entries if entry["game_won"])
    return {
        "episodes": len(entries),
        "wins": wins,
        "win_rate": wins / len(entries),
        "avg_blinds_passed": sum(entry["blinds_passed"] for entry in entries) / len(entries),
        "avg_episode_reward": sum(entry["episode_reward"] for entry in entries) / len(entries),
        "avg_episode_length": sum(entry["episode_length"] for entry in entries) / len(entries),
    }


def write_episode_artifacts(
    *,
    session_dir: Path,
    batch_session_id: str,
    bundle: dict[str, Any],
    bundle_path: str,
    buffer: EpisodeBuffer,
    render_html: bool,
) -> dict[str, Any]:
    finished_at = datetime.now(timezone.utc).isoformat()
    for record in buffer.behavior_records:
        record["finished_at"] = finished_at
    final_info = buffer.final_info or {}

    replay = {
        "version": bundle["metadata"]["version"],
        "engine": final_info.get("engine_backend", "balatro_native"),
        "seed": buffer.seed,
        "policy": "simple_rule_v1",
        "test_metadata": {
            "session_id": f"{batch_session_id}-seed-{buffer.seed}",
            "test_focus": TEST_FOCUS,
            "started_at": buffer.started_at,
            "finished_at": finished_at,
        },
        "log_metadata": {
            "policy_id": "simple_rule_v1",
            **LOG_METADATA,
        },
        "ruleset_path": bundle_path,
        "asset_root": "../../assets-preview/",
        "sprite_manifest": bundle.get("sprite_manifest", {}),
        "sprite_index": sprite_index(bundle),
        "transitions": buffer.transitions,
        "final_snapshot": final_info["state_snapshot"] if final_info else {},
    }

    replay_path = session_dir / f"seed_{buffer.seed}.replay.json"
    behavior_path = session_dir / f"seed_{buffer.seed}.behavior_log.jsonl"
    replay_path.write_text(json.dumps(replay, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")
    behavior_path.write_text(
        "".join(json.dumps(record, ensure_ascii=True) + "\n" for record in buffer.behavior_records),
        encoding="utf-8",
    )

    html_path: Path | None = None
    if render_html:
        html_path = session_dir / f"seed_{buffer.seed}.replay.html"
        render_replay_html(Path("viewer/index.html"), replay, html_path)

    return {
        "seed": buffer.seed,
        "started_at": buffer.started_at,
        "finished_at": finished_at,
        "game_won": bool(final_info.get("game_won", False)),
        "blinds_passed": int(final_info.get("blinds_passed", 0) or 0),
        "episode_reward": float(final_info.get("episode_reward", 0.0) or 0.0),
        "episode_length": int(final_info.get("step_count", len(buffer.transitions)) or len(buffer.transitions)),
        "paths": {
            "replay": str(replay_path),
            "behavior_log": str(behavior_path),
            "replay_html": str(html_path) if html_path is not None else None,
        },
    }


def main() -> int:
    args = parse_args()
    import balatro_native

    bundle_path = args.ruleset_path or balatro_native.default_ruleset_path()
    bundle = json.loads(Path(bundle_path).read_text())
    agent = SimpleRuleAgent(bundle_path)
    batch_session_id = session_id("coverage")
    session_dir = args.results_dir / batch_session_id
    session_dir.mkdir(parents=True, exist_ok=True)
    seeds = random_seeds(args.episodes, args.base_seed)
    manifest_entries: list[dict[str, Any]] = []
    rendered = 0
    env_config = {
        "env": {
            "seed": 42,
            "stake": args.stake,
            "ruleset_path": bundle_path,
            "force_mock": args.force_mock,
            "disable_reorder_actions": True,
            "max_steps": args.max_steps,
            "include_state_snapshot_in_info": True,
            "include_transition_in_info": True,
        },
        "reward": {
            "use_score_shaping": True,
        },
    }

    started_at = datetime.now(timezone.utc).isoformat()
    for offset in range(0, len(seeds), args.num_envs):
        batch = seeds[offset : offset + args.num_envs]
        vec_env = ParallelBalatroEnvs(
            config=env_config,
            num_envs=len(batch),
            seed=0,
            auto_reset=False,
        )
        obs, infos = vec_env.reset(seeds=batch)
        buffers = [
            EpisodeBuffer(seed=seed, started_at=datetime.now(timezone.utc).isoformat(), start_perf=time.perf_counter())
            for seed in batch
        ]
        active = np.ones(len(batch), dtype=bool)

        while active.any():
            action_masks = vec_env.get_action_masks()
            actions = np.zeros(len(batch), dtype=np.int64)
            plans: list[dict[str, Any] | None] = [None] * len(batch)
            for index in range(len(batch)):
                if not active[index]:
                    continue
                snapshot = infos[index]["state_snapshot"]
                legal_actions = agent.legal_actions_from_mask(action_masks[index])
                action, plan = agent.choose_action(snapshot, legal_actions)
                actions[index] = action
                plans[index] = plan

            next_obs, rewards, terminated, truncated, next_infos = vec_env.step(actions, active_mask=active)

            for index, seed in enumerate(batch):
                if not active[index]:
                    continue
                transition = next_infos[index].get("transition")
                if transition is None:
                    raise RuntimeError(f"missing transition for seed {seed}")
                elapsed_ms = int((time.perf_counter() - buffers[index].start_perf) * 1000)
                transition["step_index"] = len(buffers[index].transitions)
                transition["elapsed_ms"] = elapsed_ms
                decision_log = build_decision_log(
                    transition["snapshot_before"],
                    transition["snapshot_after"],
                    transition.get("events", []),
                    plans[index] or {
                        "policy_id": agent.policy_id,
                        "action_name": transition["action"]["name"],
                        "mode": "fallback",
                        "rationale_tags": ["tempo"],
                    },
                )
                transition["decision_log"] = decision_log
                transition["ui_asset_refs"] = {
                    "blind_name": transition["snapshot_after"]["blind_name"],
                    "joker_ids": [joker["joker_id"] for joker in transition["snapshot_after"]["jokers"]],
                    "shop_joker_ids": [joker["joker_id"] for joker in transition["snapshot_after"]["shop_jokers"]],
                }
                buffers[index].transitions.append(transition)
                buffers[index].behavior_records.append(
                    build_behavior_log_record(
                        seed=seed,
                        step_index=transition["step_index"],
                        elapsed_ms=elapsed_ms,
                        transition=transition,
                        decision_log=decision_log,
                        policy_id=agent.policy_id,
                        started_at=buffers[index].started_at,
                        finished_at=None,
                        test_focus=TEST_FOCUS,
                    )
                )
                buffers[index].episode_reward += float(rewards[index])

                if terminated[index] or truncated[index]:
                    info = dict(next_infos[index])
                    info["episode_reward"] = float(buffers[index].episode_reward)
                    buffers[index].final_info = info
                    manifest_entries.append(
                        write_episode_artifacts(
                            session_dir=session_dir,
                            batch_session_id=batch_session_id,
                            bundle=bundle,
                            bundle_path=bundle_path,
                            buffer=buffers[index],
                            render_html=rendered < args.render_html_count,
                        )
                    )
                    if rendered < args.render_html_count:
                        rendered += 1
                    active[index] = False

            obs = next_obs
            infos = next_infos

    finished_at = datetime.now(timezone.utc).isoformat()
    manifest = {
        "session_id": batch_session_id,
        "started_at": started_at,
        "finished_at": finished_at,
        "policy_id": agent.policy_id,
        "num_envs": args.num_envs,
        "episodes": args.episodes,
        "seeds": seeds,
        "test_focus": TEST_FOCUS,
        "summary_metrics": summarize(manifest_entries),
        "entries": manifest_entries,
    }
    manifest_path = session_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {manifest_path}")
    print(f"  session: {batch_session_id}")
    print(f"  episodes: {len(manifest_entries)}")
    print(f"  seeds: {seeds}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
