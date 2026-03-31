#!/usr/bin/env python3
"""PPO training with experiment report.

Runs PPO updates against parallel BalatroEnv instances and writes a
structured experiment report to results/training/.

Usage:
    python scripts/train_smoke.py
    python scripts/train_smoke.py --num-envs 8 --num-updates 100 --exp-name baseline_v1
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import sys
import time
from collections import deque
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import numpy as np

from env.action_space import ACTION_DIM
from env.balatro_gym_wrapper import BalatroEnv
from env.state_encoder import OBS_DIM
from training.rollout import RolloutBuffer

try:
    import torch
except ImportError:
    sys.exit("torch is required -- install it first (pip install torch)")

from agents.ppo_agent import PPOAgent
from training.ppo import PPOConfig, PPOTrainer

try:
    import psutil
    HAS_PSUTIL = True
except ImportError:
    HAS_PSUTIL = False


# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="PPO training with experiment report")
    parser.add_argument("--exp-name", type=str, default=None,
                        help="Experiment name (default: auto-generated timestamp)")
    parser.add_argument("--num-envs", type=int, default=4)
    parser.add_argument("--steps-per-rollout", type=int, default=256)
    parser.add_argument("--num-updates", type=int, default=50)
    parser.add_argument("--hidden-dim", type=int, default=256)
    parser.add_argument("--lr", type=float, default=3e-4)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--mini-batch-size", type=int, default=512)
    parser.add_argument("--gamma", type=float, default=0.99)
    parser.add_argument("--gae-lambda", type=float, default=0.95)
    parser.add_argument("--entropy-coef", type=float, default=0.01)
    parser.add_argument("--log-interval", type=int, default=10)
    parser.add_argument("--save-checkpoint", action="store_true",
                        help="Save model checkpoint at the end")
    return parser.parse_args()


# ---------------------------------------------------------------------------
# System info
# ---------------------------------------------------------------------------

def collect_system_info(device: str) -> dict[str, Any]:
    info: dict[str, Any] = {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "cpu": platform.processor() or platform.machine(),
        "cpu_count": os.cpu_count(),
        "torch_version": torch.__version__,
        "device": str(device),
    }
    if HAS_PSUTIL:
        mem = psutil.virtual_memory()
        info["ram_total_gb"] = round(mem.total / (1024**3), 1)
        info["ram_available_gb"] = round(mem.available / (1024**3), 1)
    if torch.cuda.is_available():
        info["gpu"] = torch.cuda.get_device_name(0)
        info["gpu_memory_gb"] = round(torch.cuda.get_device_properties(0).total_mem / (1024**3), 1)
    elif hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        info["gpu"] = "Apple MPS"
    return info


def collect_memory_snapshot() -> dict[str, float]:
    snap: dict[str, float] = {}
    if HAS_PSUTIL:
        proc = psutil.Process()
        mem = proc.memory_info()
        snap["rss_mb"] = round(mem.rss / (1024**2), 1)
        snap["vms_mb"] = round(mem.vms / (1024**2), 1)
    if torch.cuda.is_available():
        snap["gpu_allocated_mb"] = round(torch.cuda.memory_allocated() / (1024**2), 1)
        snap["gpu_reserved_mb"] = round(torch.cuda.memory_reserved() / (1024**2), 1)
    return snap


# ---------------------------------------------------------------------------
# Environment helpers
# ---------------------------------------------------------------------------

def make_envs(num_envs: int, seed: int) -> list[BalatroEnv]:
    envs: list[BalatroEnv] = []
    for i in range(num_envs):
        config = {
            "env": {"seed": seed + i, "force_mock": False, "max_steps": 2000},
            "reward": {
                "use_score_shaping": True,
                "score_shaping_scale": 0.1,
                "blind_pass_reward": 1.0,
                "win_reward": 10.0,
                "death_penalty": 1.0,
            },
        }
        envs.append(BalatroEnv(config))
    return envs


# ---------------------------------------------------------------------------
# Rollout collection
# ---------------------------------------------------------------------------

def collect_rollout(
    envs: list[BalatroEnv],
    agent: PPOAgent,
    rollout_buf: RolloutBuffer,
    current_obs: np.ndarray,
    current_masks: np.ndarray,
    episode_tracker: dict[str, Any],
    steps_per_rollout: int,
) -> tuple[np.ndarray, np.ndarray]:
    num_envs = len(envs)
    rollout_buf._ptr = 0

    for _step in range(steps_per_rollout):
        out = agent.act_batch(current_obs, current_masks)
        actions = out.action
        log_probs = out.log_prob
        values = out.value

        next_obs_list: list[np.ndarray] = []
        rewards = np.zeros(num_envs, dtype=np.float32)
        dones = np.zeros(num_envs, dtype=np.float32)
        next_masks_list: list[np.ndarray] = []

        for i in range(num_envs):
            obs_i, reward_i, terminated_i, truncated_i, info_i = envs[i].step(int(actions[i]))
            done_i = terminated_i or truncated_i
            rewards[i] = reward_i
            dones[i] = float(done_i)

            episode_tracker["step_rewards"][i] += reward_i
            episode_tracker["step_lengths"][i] += 1

            if done_i:
                episode_tracker["episode_rewards"].append(
                    episode_tracker["step_rewards"][i]
                )
                episode_tracker["episode_lengths"].append(
                    episode_tracker["step_lengths"][i]
                )
                ante = int(info_i.get("ante", info_i.get("round", 1)))
                episode_tracker["episode_antes"].append(ante)
                episode_tracker["episode_wins"].append(
                    1 if info_i.get("won", False) else 0
                )

                obs_i, _reset_info = envs[i].reset()
                episode_tracker["step_rewards"][i] = 0.0
                episode_tracker["step_lengths"][i] = 0

            next_obs_list.append(obs_i)
            next_masks_list.append(envs[i].get_action_mask())

        next_obs = np.stack(next_obs_list, axis=0)
        next_masks = np.stack(next_masks_list, axis=0)

        rollout_buf.add(
            obs=current_obs,
            actions=actions,
            rewards=rewards,
            dones=np.array(dones, dtype=np.float32),
            log_probs=log_probs,
            values=values,
            action_masks=current_masks,
        )

        current_obs = next_obs
        current_masks = next_masks

    return current_obs, current_masks


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    args = parse_args()
    t_start = time.monotonic()
    wall_start = datetime.now(timezone.utc)

    np.random.seed(args.seed)
    torch.manual_seed(args.seed)

    # Experiment ID
    ts = wall_start.strftime("%Y%m%dT%H%M%SZ")
    exp_name = args.exp_name or f"ppo_{ts}"
    exp_dir = ROOT / "results" / "training" / exp_name
    exp_dir.mkdir(parents=True, exist_ok=True)

    print(f"Experiment: {exp_name}")
    print(f"  output_dir: {exp_dir}")
    print(f"  num_envs={args.num_envs}  steps_per_rollout={args.steps_per_rollout}"
          f"  num_updates={args.num_updates}")
    print(f"  hidden_dim={args.hidden_dim}  lr={args.lr}  entropy_coef={args.entropy_coef}"
          f"  seed={args.seed}")
    print(f"  OBS_DIM={OBS_DIM}  ACTION_DIM={ACTION_DIM}")

    # --- Create components ---
    envs = make_envs(args.num_envs, args.seed)
    backend = envs[0]._engine.backend
    print(f"  engine_backend={backend}")

    agent = PPOAgent(model_type="mlp", hidden_dim=args.hidden_dim)
    print(f"  device={agent.device}")
    print()

    system_info = collect_system_info(agent.device)
    mem_before = collect_memory_snapshot()

    ppo_config = PPOConfig(
        clip_range=0.2,
        entropy_coef=args.entropy_coef,
        value_loss_coef=0.5,
        max_grad_norm=0.5,
        num_epochs=4,
    )
    trainer = PPOTrainer(
        agent=agent,
        config=ppo_config,
        lr=args.lr,
        scheduler_name="constant",
        total_updates=args.num_updates,
    )

    rollout_buf = RolloutBuffer(
        num_envs=args.num_envs,
        steps_per_env=args.steps_per_rollout,
        obs_dim=OBS_DIM,
        action_dim=ACTION_DIM,
    )

    # Model param count
    param_count = sum(p.numel() for p in agent.model.parameters())
    trainable_count = sum(p.numel() for p in agent.model.parameters() if p.requires_grad)

    # --- Initial reset ---
    obs_list: list[np.ndarray] = []
    mask_list: list[np.ndarray] = []
    for env in envs:
        obs_i, _info = env.reset()
        obs_list.append(obs_i)
        mask_list.append(env.get_action_mask())
    current_obs = np.stack(obs_list, axis=0)
    current_masks = np.stack(mask_list, axis=0)

    # --- Episode tracking ---
    episode_tracker: dict[str, Any] = {
        "episode_rewards": deque(maxlen=100),
        "episode_lengths": deque(maxlen=100),
        "episode_antes": deque(maxlen=100),
        "episode_wins": deque(maxlen=100),
        "step_rewards": np.zeros(args.num_envs, dtype=np.float64),
        "step_lengths": np.zeros(args.num_envs, dtype=np.int64),
    }

    # --- Metrics history ---
    history: list[dict[str, Any]] = []
    total_env_steps = 0
    total_episodes = 0

    # --- Timing ---
    env_time = 0.0
    train_time = 0.0

    # --- Training loop ---
    for update_idx in range(1, args.num_updates + 1):
        agent.train()

        t0 = time.monotonic()
        current_obs, current_masks = collect_rollout(
            envs=envs,
            agent=agent,
            rollout_buf=rollout_buf,
            current_obs=current_obs,
            current_masks=current_masks,
            episode_tracker=episode_tracker,
            steps_per_rollout=args.steps_per_rollout,
        )
        t1 = time.monotonic()
        env_time += t1 - t0

        steps_this_update = args.steps_per_rollout * args.num_envs
        total_env_steps += steps_this_update

        # Compute bootstrap values
        with torch.no_grad():
            last_out = agent.act_batch(current_obs, current_masks)
            last_values = last_out.value
        last_dones = np.zeros(args.num_envs, dtype=np.float32)
        rollout_buf.compute_advantages(
            last_values=last_values,
            last_dones=last_dones,
            gamma=args.gamma,
            gae_lambda=args.gae_lambda,
        )

        # PPO update
        t2 = time.monotonic()
        epoch_metrics: list[dict[str, float]] = []
        for _epoch in range(ppo_config.num_epochs):
            for batch in rollout_buf.get_batches(args.mini_batch_size, agent.device):
                metrics = trainer.update(batch)
                epoch_metrics.append(metrics)
        t3 = time.monotonic()
        train_time += t3 - t2

        agg: dict[str, float] = {}
        if epoch_metrics:
            for key in epoch_metrics[0]:
                agg[key] = sum(m[key] for m in epoch_metrics) / len(epoch_metrics)

        ep_rewards = episode_tracker["episode_rewards"]
        ep_lengths = episode_tracker["episode_lengths"]
        ep_antes = episode_tracker["episode_antes"]
        ep_wins = episode_tracker["episode_wins"]

        total_episodes = len(ep_rewards)

        record = {
            "update": update_idx,
            "total_env_steps": total_env_steps,
            "total_episodes": total_episodes,
            "wall_time_s": round(time.monotonic() - t_start, 2),
            "loss": round(agg.get("loss", 0.0), 6),
            "policy_loss": round(agg.get("policy_loss", 0.0), 6),
            "value_loss": round(agg.get("value_loss", 0.0), 6),
            "entropy": round(agg.get("entropy", 0.0), 4),
            "approx_kl": round(agg.get("approx_kl", 0.0), 6),
            "clip_fraction": round(agg.get("clip_fraction", 0.0), 4),
            "lr": agg.get("lr", args.lr),
            "mean_reward": round(float(np.mean(ep_rewards)) if ep_rewards else 0.0, 4),
            "mean_ante": round(float(np.mean(ep_antes)) if ep_antes else 0.0, 2),
            "max_ante": int(max(ep_antes)) if ep_antes else 0,
            "mean_ep_len": round(float(np.mean(ep_lengths)) if ep_lengths else 0.0, 1),
            "win_rate": round(float(np.mean(ep_wins)) if ep_wins else 0.0, 4),
            "throughput_steps_per_sec": round(steps_this_update / (t1 - t0 + t3 - t2), 0),
        }
        history.append(record)

        if update_idx % args.log_interval == 0 or update_idx == 1:
            elapsed = time.monotonic() - t_start
            print(
                f"Update {update_idx:>{len(str(args.num_updates))}}/{args.num_updates}"
                f" | loss: {record['loss']:.4f}"
                f" | entropy: {record['entropy']:.3f}"
                f" | kl: {record['approx_kl']:.5f}"
                f" | reward: {record['mean_reward']:.3f}"
                f" | ante: {record['mean_ante']:.1f}"
                f" | win: {record['win_rate']:.2%}"
                f" | ep_len: {record['mean_ep_len']:.0f}"
                f" | steps: {total_env_steps}"
                f" | {record['throughput_steps_per_sec']:.0f} sps"
                f" | {elapsed:.1f}s"
            )

    # --- Final summary ---
    elapsed = time.monotonic() - t_start
    mem_after = collect_memory_snapshot()

    summary = {
        "experiment_name": exp_name,
        "started_at": wall_start.isoformat(),
        "finished_at": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": round(elapsed, 2),
        "env_time_s": round(env_time, 2),
        "train_time_s": round(train_time, 2),
        "overhead_time_s": round(elapsed - env_time - train_time, 2),
        "total_env_steps": total_env_steps,
        "total_updates": args.num_updates,
        "total_episodes": total_episodes,
        "steps_per_second": round(total_env_steps / elapsed, 0),
        "final_mean_reward": round(float(np.mean(ep_rewards)) if ep_rewards else 0.0, 4),
        "final_mean_ante": round(float(np.mean(ep_antes)) if ep_antes else 0.0, 2),
        "final_max_ante": int(max(ep_antes)) if ep_antes else 0,
        "final_win_rate": round(float(np.mean(ep_wins)) if ep_wins else 0.0, 4),
        "final_entropy": round(agg.get("entropy", 0.0), 4),
        "final_loss": round(agg.get("loss", 0.0), 6),
    }

    report = {
        "summary": summary,
        "config": {
            "num_envs": args.num_envs,
            "steps_per_rollout": args.steps_per_rollout,
            "num_updates": args.num_updates,
            "hidden_dim": args.hidden_dim,
            "lr": args.lr,
            "seed": args.seed,
            "mini_batch_size": args.mini_batch_size,
            "gamma": args.gamma,
            "gae_lambda": args.gae_lambda,
            "entropy_coef": args.entropy_coef,
            "obs_dim": OBS_DIM,
            "action_dim": ACTION_DIM,
            "engine_backend": backend,
        },
        "model": {
            "type": "mlp",
            "hidden_dim": args.hidden_dim,
            "param_count": param_count,
            "trainable_param_count": trainable_count,
        },
        "system": system_info,
        "memory": {
            "before_training": mem_before,
            "after_training": mem_after,
        },
        "throughput": {
            "total_steps_per_sec": round(total_env_steps / elapsed, 0),
            "env_steps_per_sec": round(total_env_steps / env_time, 0) if env_time > 0 else 0,
            "train_updates_per_sec": round(args.num_updates / train_time, 2) if train_time > 0 else 0,
            "env_pct": round(100 * env_time / elapsed, 1),
            "train_pct": round(100 * train_time / elapsed, 1),
            "overhead_pct": round(100 * (elapsed - env_time - train_time) / elapsed, 1),
        },
        "history": history,
    }

    # --- Write report ---
    report_path = exp_dir / "report.json"
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False))

    # Write condensed metrics CSV
    csv_path = exp_dir / "metrics.csv"
    if history:
        keys = list(history[0].keys())
        lines = [",".join(keys)]
        for row in history:
            lines.append(",".join(str(row.get(k, "")) for k in keys))
        csv_path.write_text("\n".join(lines) + "\n")

    # Save checkpoint
    if args.save_checkpoint:
        ckpt_path = exp_dir / "checkpoint.pt"
        agent.save(str(ckpt_path))
        print(f"  Checkpoint saved: {ckpt_path}")

    # --- Print summary ---
    print()
    print("=" * 60)
    print(f"EXPERIMENT COMPLETE: {exp_name}")
    print("=" * 60)
    print()
    print(f"  Duration:           {summary['elapsed_s']:.1f}s")
    print(f"    env collection:   {summary['env_time_s']:.1f}s ({report['throughput']['env_pct']}%)")
    print(f"    PPO training:     {summary['train_time_s']:.1f}s ({report['throughput']['train_pct']}%)")
    print(f"    overhead:         {summary['overhead_time_s']:.1f}s ({report['throughput']['overhead_pct']}%)")
    print()
    print(f"  Env steps:          {summary['total_env_steps']:,}")
    print(f"  Episodes:           {summary['total_episodes']}")
    print(f"  Throughput:         {summary['steps_per_second']:.0f} steps/sec")
    print(f"    env only:         {report['throughput']['env_steps_per_sec']:.0f} steps/sec")
    print()
    if mem_after:
        print(f"  Memory (RSS):       {mem_after.get('rss_mb', '?')} MB")
    print(f"  Model params:       {param_count:,} ({trainable_count:,} trainable)")
    print()
    print(f"  Mean reward:        {summary['final_mean_reward']:.4f}")
    print(f"  Mean ante:          {summary['final_mean_ante']:.1f}")
    print(f"  Max ante:           {summary['final_max_ante']}")
    print(f"  Win rate:           {summary['final_win_rate']:.2%}")
    print(f"  Entropy:            {summary['final_entropy']:.4f}")
    print()
    print(f"  Report: {report_path}")
    if args.save_checkpoint:
        print(f"  Checkpoint: {exp_dir / 'checkpoint.pt'}")
    print()


if __name__ == "__main__":
    main()
