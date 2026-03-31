#!/usr/bin/env python3
"""Minimal PPO training smoke test.

Runs a small number of PPO updates against parallel BalatroEnv instances
to verify the full training loop works end-to-end.  Designed to complete
in under 5 minutes on a Mac with CPU only.

Usage:
    python scripts/train_smoke.py
    python scripts/train_smoke.py --num-envs 8 --num-updates 20 --steps-per-rollout 128
"""
from __future__ import annotations

import argparse
import sys
import time
from collections import deque
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


# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="PPO training smoke test")
    parser.add_argument("--num-envs", type=int, default=4,
                        help="Number of parallel environments (default: 4)")
    parser.add_argument("--steps-per-rollout", type=int, default=256,
                        help="Steps collected per env per update (default: 256)")
    parser.add_argument("--num-updates", type=int, default=50,
                        help="Number of PPO updates to run (default: 50)")
    parser.add_argument("--hidden-dim", type=int, default=256,
                        help="Hidden dimension for MLP model (default: 256)")
    parser.add_argument("--lr", type=float, default=3e-4,
                        help="Learning rate (default: 3e-4)")
    parser.add_argument("--seed", type=int, default=42,
                        help="Random seed (default: 42)")
    parser.add_argument("--mini-batch-size", type=int, default=512,
                        help="Mini-batch size for PPO epochs (default: 512)")
    parser.add_argument("--gamma", type=float, default=0.99,
                        help="Discount factor (default: 0.99)")
    parser.add_argument("--gae-lambda", type=float, default=0.95,
                        help="GAE lambda (default: 0.95)")
    parser.add_argument("--log-interval", type=int, default=10,
                        help="Print metrics every N updates (default: 10)")
    return parser.parse_args()


# ---------------------------------------------------------------------------
# Environment helpers
# ---------------------------------------------------------------------------

def make_envs(num_envs: int, seed: int) -> list[BalatroEnv]:
    """Create N independent BalatroEnv instances with distinct seeds."""
    envs: list[BalatroEnv] = []
    for i in range(num_envs):
        config = {
            "env": {
                "seed": seed + i,
                "force_mock": False,
                "max_steps": 2000,
            },
            "reward": {
                "use_score_shaping": True,
                "score_shaping_scale": 0.1,
                "blind_pass_reward": 0.5,
                "win_reward": 10.0,
                "death_penalty": 0.0,
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
    episode_tracker: dict[str, deque],
    steps_per_rollout: int,
) -> tuple[np.ndarray, np.ndarray]:
    """Collect *steps_per_rollout* steps from each env, storing into rollout_buf.

    Returns updated (current_obs, current_masks) after rollout.
    """
    num_envs = len(envs)
    rollout_buf._ptr = 0  # reset buffer pointer for this rollout

    for _step in range(steps_per_rollout):
        # Batch inference
        out = agent.act_batch(current_obs, current_masks)
        actions = out.action          # (num_envs,)
        log_probs = out.log_prob      # (num_envs,)
        values = out.value            # (num_envs,)

        # Step all envs
        next_obs_list: list[np.ndarray] = []
        rewards = np.zeros(num_envs, dtype=np.float32)
        dones = np.zeros(num_envs, dtype=np.float32)
        next_masks_list: list[np.ndarray] = []

        for i in range(num_envs):
            obs_i, reward_i, terminated_i, truncated_i, info_i = envs[i].step(int(actions[i]))
            done_i = terminated_i or truncated_i
            rewards[i] = reward_i
            dones[i] = float(done_i)

            # Track episode stats
            episode_tracker["step_rewards"][i] += reward_i
            episode_tracker["step_lengths"][i] += 1

            if done_i:
                episode_tracker["episode_rewards"].append(
                    episode_tracker["step_rewards"][i]
                )
                episode_tracker["episode_lengths"].append(
                    episode_tracker["step_lengths"][i]
                )
                ante = int(info_i.get("round", 1))
                episode_tracker["episode_antes"].append(ante)

                # Reset this env
                obs_i, _reset_info = envs[i].reset()
                episode_tracker["step_rewards"][i] = 0.0
                episode_tracker["step_lengths"][i] = 0

            next_obs_list.append(obs_i)
            next_masks_list.append(envs[i].get_action_mask())

        next_obs = np.stack(next_obs_list, axis=0)
        next_masks = np.stack(next_masks_list, axis=0)

        # Store transition
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
# Main training loop
# ---------------------------------------------------------------------------

def main() -> None:
    args = parse_args()
    t_start = time.monotonic()

    # Seed numpy/torch for reproducibility
    np.random.seed(args.seed)
    torch.manual_seed(args.seed)

    print(f"PPO Smoke Test")
    print(f"  num_envs={args.num_envs}  steps_per_rollout={args.steps_per_rollout}"
          f"  num_updates={args.num_updates}")
    print(f"  hidden_dim={args.hidden_dim}  lr={args.lr}  seed={args.seed}")
    print(f"  OBS_DIM={OBS_DIM}  ACTION_DIM={ACTION_DIM}")

    # --- Create components ---
    envs = make_envs(args.num_envs, args.seed)
    backend = envs[0]._engine.backend
    print(f"  engine_backend={backend}")
    print()

    agent = PPOAgent(model_type="mlp", hidden_dim=args.hidden_dim)
    print(f"  device={agent.device}")

    ppo_config = PPOConfig(
        clip_range=0.2,
        entropy_coef=0.01,
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
        "step_rewards": np.zeros(args.num_envs, dtype=np.float64),
        "step_lengths": np.zeros(args.num_envs, dtype=np.int64),
    }

    total_env_steps = 0
    latest_metrics: dict[str, float] = {}

    # --- Training loop ---
    for update_idx in range(1, args.num_updates + 1):
        agent.train()

        # Collect rollout
        current_obs, current_masks = collect_rollout(
            envs=envs,
            agent=agent,
            rollout_buf=rollout_buf,
            current_obs=current_obs,
            current_masks=current_masks,
            episode_tracker=episode_tracker,
            steps_per_rollout=args.steps_per_rollout,
        )
        total_env_steps += args.steps_per_rollout * args.num_envs

        # Compute bootstrap values for GAE
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

        # PPO update (multiple epochs over mini-batches)
        epoch_metrics: list[dict[str, float]] = []
        for _epoch in range(ppo_config.num_epochs):
            for batch in rollout_buf.get_batches(args.mini_batch_size, agent.device):
                metrics = trainer.update(batch)
                epoch_metrics.append(metrics)

        # Aggregate metrics across all mini-batch updates this rollout
        agg: dict[str, float] = {}
        if epoch_metrics:
            for key in epoch_metrics[0]:
                agg[key] = sum(m[key] for m in epoch_metrics) / len(epoch_metrics)
        latest_metrics = agg

        # Log progress
        if update_idx % args.log_interval == 0 or update_idx == 1:
            ep_rewards = episode_tracker["episode_rewards"]
            ep_lengths = episode_tracker["episode_lengths"]
            ep_antes = episode_tracker["episode_antes"]

            mean_reward = float(np.mean(ep_rewards)) if ep_rewards else 0.0
            mean_ante = float(np.mean(ep_antes)) if ep_antes else 0.0
            mean_ep_len = float(np.mean(ep_lengths)) if ep_lengths else 0.0
            loss = latest_metrics.get("loss", 0.0)
            entropy = latest_metrics.get("entropy", 0.0)
            approx_kl = latest_metrics.get("approx_kl", 0.0)

            elapsed = time.monotonic() - t_start
            print(
                f"Update {update_idx:>{len(str(args.num_updates))}}/{args.num_updates}"
                f" | loss: {loss:.4f}"
                f" | entropy: {entropy:.3f}"
                f" | approx_kl: {approx_kl:.5f}"
                f" | mean_reward: {mean_reward:.3f}"
                f" | mean_ante: {mean_ante:.1f}"
                f" | mean_ep_len: {mean_ep_len:.0f}"
                f" | steps: {total_env_steps}"
                f" | time: {elapsed:.1f}s"
            )

    # --- Summary ---
    elapsed = time.monotonic() - t_start
    ep_rewards = episode_tracker["episode_rewards"]
    ep_antes = episode_tracker["episode_antes"]
    final_reward = float(np.mean(ep_rewards)) if ep_rewards else 0.0
    final_ante = float(np.mean(ep_antes)) if ep_antes else 0.0
    total_episodes = len(episode_tracker["episode_rewards"])

    print()
    print("=== SMOKE TEST COMPLETE ===")
    print(f"Total env steps: {total_env_steps}")
    print(f"Total updates: {args.num_updates}")
    print(f"Total episodes completed: {total_episodes}")
    print(f"Final mean reward: {final_reward:.4f}")
    print(f"Final mean ante: {final_ante:.1f}")
    print(f"Final loss: {latest_metrics.get('loss', 0.0):.4f}")
    print(f"Final entropy: {latest_metrics.get('entropy', 0.0):.3f}")
    print(f"Engine backend: {backend}")
    print(f"Device: {agent.device}")
    print(f"Time elapsed: {elapsed:.1f}s")


if __name__ == "__main__":
    main()
