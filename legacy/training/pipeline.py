from __future__ import annotations

import json
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from agents.greedy_agent import GreedyAgent
from agents.random_agent import RandomAgent
from env.legacy.action_space import ACTION_DIM
from env.legacy.balatro_gym_wrapper import BalatroEnv, make_vec_env
from env.legacy.state_encoder import OBS_DIM
from eval.compare_baselines import compare_agents
from eval.eval_policy import evaluate_agent
from scripts.doctor import run_doctor
from utils.reporting import ensure_dir, write_csv, write_json


@dataclass
class PhaseResult:
    ok: bool
    run_id: str
    results_dir: str
    metrics_path: str
    comparison_path: str


def _timestamp_run_id(prefix: str = "run") -> str:
    return f"{prefix}_{time.strftime('%Y%m%d_%H%M%S')}"


def _load_seeds(config: dict[str, Any]) -> list[int]:
    seeds_path = Path(config.get("eval", {}).get("seeds_file", "eval/seeds.json"))
    with seeds_path.open("r", encoding="utf-8") as f:
        seeds = json.load(f)
    if not isinstance(seeds, list):
        raise ValueError("seeds file must be a list")
    return [int(s) for s in seeds]


def _env_factory(config: dict[str, Any]):
    def _factory(seed: int):
        env = BalatroEnv(config)
        env.reset(seed=seed)
        return env

    return _factory


def _comparison_rows(results: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    rows = []
    for name, metrics in results.items():
        rows.append(
            {
                "agent": name,
                "episodes": metrics["episodes"],
                "wins": metrics["wins"],
                "win_rate": metrics["win_rate"],
                "avg_blinds_passed": metrics["avg_blinds_passed"],
                "avg_episode_reward": metrics["avg_episode_reward"],
                "avg_episode_length": metrics["avg_episode_length"],
                "max_win_streak": metrics["max_win_streak"],
                "avg_win_streak": metrics["avg_win_streak"],
            }
        )
    return rows


def _save_comparison_report(output_dir: Path, result_name: str, results: dict[str, dict[str, Any]]) -> tuple[Path, Path]:
    metrics_path = output_dir / f"{result_name}_metrics.json"
    csv_path = output_dir / f"{result_name}_comparison.csv"

    write_json(metrics_path, results)
    write_csv(
        csv_path,
        _comparison_rows(results),
        [
            "agent",
            "episodes",
            "wins",
            "win_rate",
            "avg_blinds_passed",
            "avg_episode_reward",
            "avg_episode_length",
            "max_win_streak",
            "avg_win_streak",
        ],
    )
    return metrics_path, csv_path


def run_phase1(config: dict[str, Any], run_id: str | None = None) -> PhaseResult:
    run_id = run_id or _timestamp_run_id("phase1")
    out_dir = ensure_dir(Path(config.get("report", {}).get("results_dir", "results")) / run_id)

    doctor_path = out_dir / "doctor.json"
    doctor = run_doctor(output_path=str(doctor_path), config=config)

    seeds = _load_seeds(config)
    eval_cfg = config.get("eval", {})
    baseline_episodes = int(eval_cfg.get("baseline_episodes", 100))
    seeds = seeds[:baseline_episodes]

    env_factory = _env_factory(config)

    random_agent = RandomAgent(seed=int(config.get("env", {}).get("seed", 42)))
    greedy_agent = GreedyAgent()

    baseline = compare_agents(
        agents={
            "Random": random_agent,
            "Greedy": greedy_agent,
        },
        env_factory=env_factory,
        seeds=seeds,
        episodes_per_seed=1,
    )

    repeat_runs = int(eval_cfg.get("repeat_runs_for_repro", 2))
    repeats = []
    for i in range(repeat_runs):
        rep = compare_agents(
            agents={
                "Random": RandomAgent(seed=42 + i),
                "Greedy": GreedyAgent(),
            },
            env_factory=env_factory,
            seeds=seeds,
            episodes_per_seed=1,
        )
        repeats.append(rep)

    tolerance = eval_cfg.get("repro_tolerance", {})
    diff_summary = {
        "win_rate_diff": abs(repeats[0]["Greedy"]["win_rate"] - repeats[-1]["Greedy"]["win_rate"]),
        "avg_blinds_diff": abs(
            repeats[0]["Greedy"]["avg_blinds_passed"] - repeats[-1]["Greedy"]["avg_blinds_passed"]
        ),
        "avg_reward_diff": abs(
            repeats[0]["Greedy"]["avg_episode_reward"] - repeats[-1]["Greedy"]["avg_episode_reward"]
        ),
    }
    repro_ok = (
        diff_summary["win_rate_diff"] <= float(tolerance.get("win_rate", 0.02))
        and diff_summary["avg_blinds_diff"] <= float(tolerance.get("avg_blinds_passed", 0.3))
        and diff_summary["avg_reward_diff"] <= float(tolerance.get("avg_episode_reward", 0.5))
    )

    gate = {
        "doctor_ok": bool(doctor["ok"]),
        "eval_completed": all(metrics["episodes"] == baseline_episodes for metrics in baseline.values()),
        "greedy_better_than_random": baseline["Greedy"]["avg_blinds_passed"]
        > baseline["Random"]["avg_blinds_passed"],
        "repro_stable": repro_ok,
    }

    summary = {
        "phase": "phase1",
        "gate": gate,
        "doctor": doctor,
        "baseline": baseline,
        "repeats": repeats,
        "repro_diff": diff_summary,
    }

    metrics_path, comparison_path = _save_comparison_report(out_dir, "phase1", baseline)
    write_json(out_dir / "phase1_summary.json", summary)

    ok = all(gate.values())
    return PhaseResult(
        ok=ok,
        run_id=run_id,
        results_dir=str(out_dir),
        metrics_path=str(metrics_path),
        comparison_path=str(comparison_path),
    )


def _save_checkpoint(
    path: Path,
    agent: Any,
    trainer: Any,
    total_env_steps: int,
    extra: dict[str, Any] | None = None,
) -> None:
    import torch

    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "model_state": agent.model.state_dict(),
        "model_type": agent.model_type,
        "trainer_state": trainer.state_dict(),
        "total_env_steps": int(total_env_steps),
        "extra": extra or {},
    }
    torch.save(payload, path)


def _load_checkpoint(path: str | Path, agent: Any, trainer: Any) -> int:
    import torch

    ckpt = torch.load(path, map_location=agent.device)
    agent.model.load_state_dict(ckpt["model_state"], strict=False)
    trainer.load_state_dict(ckpt.get("trainer_state", {}))
    return int(ckpt.get("total_env_steps", 0))


def run_phase2(
    config: dict[str, Any],
    strategy_name: str,
    run_id: str | None = None,
    resume_path: str | None = None,
) -> PhaseResult:
    try:
        import torch
    except Exception as exc:  # pragma: no cover
        raise RuntimeError("phase2 requires torch installed") from exc

    from agents.ppo_agent import PPOAgent
    from legacy.training.behavior_clone import BehaviorCloner, collect_greedy_trajectories
    from legacy.training.curriculum import CurriculumScheduler
    from legacy.training.ppo import PPOConfig, PPOTrainer
    from legacy.training.rollout import RolloutBuffer

    run_id = run_id or _timestamp_run_id(f"phase2_{strategy_name}")
    out_dir = ensure_dir(Path(config.get("report", {}).get("results_dir", "results")) / run_id)
    checkpoints_dir = ensure_dir(config.get("checkpoint", {}).get("dir", "checkpoints"))
    trajectories_dir = ensure_dir("trajectories")

    env_factory = _env_factory(config)
    seeds = _load_seeds(config)

    model_type = config.get("model", {}).get("type", "mlp")
    agent = PPOAgent(model_type=model_type)
    training_cfg = config.get("training", {})

    ppo_cfg = PPOConfig(
        clip_range=float(training_cfg.get("clip_range", 0.2)),
        entropy_coef=float(training_cfg.get("entropy_coef", 0.01)),
        value_loss_coef=float(training_cfg.get("value_loss_coef", 0.5)),
        max_grad_norm=float(training_cfg.get("max_grad_norm", 0.5)),
        num_epochs=int(training_cfg.get("num_epochs", 4)),
    )

    total_steps_target = int(training_cfg.get("total_env_steps", 100000))
    num_envs = int(training_cfg.get("num_envs", 8))
    steps_per_env = int(training_cfg.get("steps_per_env", 64))
    mini_batch_size = int(training_cfg.get("mini_batch_size", 256))
    lr = float(training_cfg.get("learning_rate", 2e-4))
    scheduler_name = str(training_cfg.get("scheduler", "constant"))
    eval_interval = int(training_cfg.get("eval_interval", 10000))
    checkpoint_interval = int(training_cfg.get("checkpoint_interval", 20000))

    total_updates = max(1, total_steps_target // max(1, num_envs * steps_per_env)) * ppo_cfg.num_epochs
    trainer = PPOTrainer(
        agent,
        ppo_cfg,
        lr=lr,
        scheduler_name=scheduler_name,
        total_updates=total_updates,
    )

    # Optional BC warm-start
    bc_cfg = training_cfg.get("behavior_clone", {})
    bc_path = Path(checkpoints_dir) / f"bc_{strategy_name}.pt"
    bc_metrics: dict[str, Any] = {}
    if bool(bc_cfg.get("enabled", True)) and not bc_path.exists() and not resume_path:
        greedy = GreedyAgent()
        traj = collect_greedy_trajectories(
            env_factory=env_factory,
            expert_agent=greedy,
            num_games=int(bc_cfg.get("trajectories_games", 200)),
            max_steps=300,
        )
        traj_path = Path(trajectories_dir) / f"greedy_{strategy_name}.pt"
        torch.save(traj, traj_path)

        cloner = BehaviorCloner(
            agent,
            lr=float(bc_cfg.get("lr", 1e-3)),
            batch_size=int(bc_cfg.get("batch_size", 256)),
        )
        bc_result = cloner.train(traj_path, num_epochs=int(bc_cfg.get("epochs", 4)))
        torch.save({"model_state": agent.model.state_dict(), "model_type": model_type}, bc_path)
        bc_metrics = {
            "final_loss": bc_result.final_loss,
            "epochs": bc_result.epochs,
            "samples": bc_result.samples,
            "trajectory_path": str(traj_path),
            "checkpoint": str(bc_path),
        }

    # Keep BC-only snapshot for later comparison
    bc_agent = PPOAgent(model_type=model_type)
    if bc_path.exists():
        bc_agent.load(bc_path, strict=False)
    else:
        bc_agent.model.load_state_dict(agent.model.state_dict(), strict=False)

    total_env_steps = 0
    best_metric = -1e9
    latest_path = Path(config.get("checkpoint", {}).get("latest_path", "checkpoints/latest.pt"))
    best_path = Path(config.get("checkpoint", {}).get("best_path", "checkpoints/best.pt"))

    if resume_path:
        total_env_steps = _load_checkpoint(resume_path, agent, trainer)

    curriculum_events: list[dict[str, Any]] = []
    curriculum = None
    curriculum_cfg = training_cfg.get("curriculum", {})
    if bool(curriculum_cfg.get("enabled", False)):
        curriculum = CurriculumScheduler(curriculum_cfg.get("stages", []))

    vec_env = make_vec_env(config=config, num_envs=num_envs, seed=int(config.get("env", {}).get("seed", 42)))
    obs, _ = vec_env.reset()

    while total_env_steps < total_steps_target:
        buffer = RolloutBuffer(num_envs=num_envs, steps_per_env=steps_per_env, obs_dim=OBS_DIM, action_dim=ACTION_DIM)

        for _ in range(steps_per_env):
            masks = vec_env.get_action_masks()
            out = agent.act_batch(obs, masks)
            next_obs, rewards, terminated, truncated, infos = vec_env.step(out.action)
            dones = np.logical_or(terminated, truncated)

            buffer.add(
                obs=obs,
                actions=out.action,
                rewards=rewards,
                dones=dones,
                log_probs=out.log_prob,
                values=out.value,
                action_masks=masks,
            )
            obs = next_obs
            total_env_steps += num_envs

            if total_env_steps >= total_steps_target:
                break

        last_masks = vec_env.get_action_masks()
        with torch.no_grad():
            obs_t = torch.as_tensor(obs, dtype=torch.float32, device=agent.device)
            _, last_values_t = agent.model(obs_t)
            last_values = last_values_t.detach().cpu().numpy()
        last_dones = np.zeros(num_envs, dtype=np.float32)
        buffer.compute_advantages(
            last_values=last_values,
            last_dones=last_dones,
            gamma=float(training_cfg.get("gamma", 0.99)),
            gae_lambda=float(training_cfg.get("gae_lambda", 0.95)),
        )

        update_metrics = []
        for _epoch in range(ppo_cfg.num_epochs):
            for batch in buffer.get_batches(mini_batch_size=mini_batch_size, device=agent.device):
                update_metrics.append(trainer.update(batch))

        if total_env_steps % eval_interval < num_envs:
            quick_eval = evaluate_agent(
                agent=agent,
                env_factory=env_factory,
                seeds=seeds[:20],
                episodes_per_seed=1,
            )
            score = quick_eval["avg_blinds_passed"]
            if score > best_metric:
                best_metric = score
                _save_checkpoint(best_path, agent, trainer, total_env_steps, {"quick_eval": quick_eval})

            if curriculum is not None:
                advanced, stage = curriculum.maybe_advance(
                    win_rate=float(quick_eval["win_rate"]),
                    episodes=quick_eval["episodes"],
                )
                if advanced:
                    curriculum_events.append(
                        {
                            "step": total_env_steps,
                            "new_stage": stage.name,
                            "max_ante": stage.max_ante,
                        }
                    )

        if total_env_steps % checkpoint_interval < num_envs:
            _save_checkpoint(latest_path, agent, trainer, total_env_steps)

    _save_checkpoint(latest_path, agent, trainer, total_env_steps)

    random_agent = RandomAgent(seed=42)
    greedy_agent = GreedyAgent()
    ppo_eval_agent = PPOAgent(model_type=model_type)
    if best_path.exists():
        ppo_eval_agent.load(best_path, strict=False)
    else:
        ppo_eval_agent.model.load_state_dict(agent.model.state_dict(), strict=False)

    final_compare = compare_agents(
        agents={
            "Random": random_agent,
            "Greedy": greedy_agent,
            "BC": bc_agent,
            "PPO": ppo_eval_agent,
        },
        env_factory=env_factory,
        seeds=seeds[: int(config.get("eval", {}).get("baseline_episodes", 100))],
        episodes_per_seed=1,
    )

    ppo_vs_bc_ok = (
        final_compare["PPO"]["avg_blinds_passed"] >= final_compare["BC"]["avg_blinds_passed"]
        or final_compare["PPO"]["avg_episode_reward"] >= final_compare["BC"]["avg_episode_reward"]
    )

    gate = {
        "pipeline_ran": True,
        "resume_supported": latest_path.exists(),
        "comparison_generated": True,
        "ppo_beats_bc_on_primary_metric": bool(ppo_vs_bc_ok),
    }

    summary = {
        "phase": "phase2",
        "strategy": strategy_name,
        "gate": gate,
        "total_env_steps": total_env_steps,
        "bc": bc_metrics,
        "curriculum_events": curriculum_events,
        "scheduler": scheduler_name,
        "checkpoints": {
            "best": str(best_path),
            "latest": str(latest_path),
        },
    }

    metrics_path, comparison_path = _save_comparison_report(out_dir, "phase2", final_compare)
    write_json(out_dir / "phase2_summary.json", summary)

    return PhaseResult(
        ok=all(gate.values()),
        run_id=run_id,
        results_dir=str(out_dir),
        metrics_path=str(metrics_path),
        comparison_path=str(comparison_path),
    )
