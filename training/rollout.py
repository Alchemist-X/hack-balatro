from __future__ import annotations

from dataclasses import dataclass
from typing import Iterator

import numpy as np
import torch


@dataclass
class RolloutBatch:
    obs: torch.Tensor
    actions: torch.Tensor
    old_log_probs: torch.Tensor
    old_values: torch.Tensor
    returns: torch.Tensor
    advantages: torch.Tensor
    action_masks: torch.Tensor


class RolloutBuffer:
    def __init__(self, num_envs: int, steps_per_env: int, obs_dim: int, action_dim: int) -> None:
        self.num_envs = num_envs
        self.steps_per_env = steps_per_env
        self.obs_dim = obs_dim
        self.action_dim = action_dim

        shape = (steps_per_env, num_envs)
        self.obs = np.zeros((steps_per_env, num_envs, obs_dim), dtype=np.float32)
        self.actions = np.zeros(shape, dtype=np.int64)
        self.rewards = np.zeros(shape, dtype=np.float32)
        self.dones = np.zeros(shape, dtype=np.float32)
        self.log_probs = np.zeros(shape, dtype=np.float32)
        self.values = np.zeros(shape, dtype=np.float32)
        self.action_masks = np.zeros((steps_per_env, num_envs, action_dim), dtype=bool)

        self.advantages = np.zeros(shape, dtype=np.float32)
        self.returns = np.zeros(shape, dtype=np.float32)

        self._ptr = 0

    def add(
        self,
        obs: np.ndarray,
        actions: np.ndarray,
        rewards: np.ndarray,
        dones: np.ndarray,
        log_probs: np.ndarray,
        values: np.ndarray,
        action_masks: np.ndarray,
    ) -> None:
        if self._ptr >= self.steps_per_env:
            raise RuntimeError("RolloutBuffer overflow")

        i = self._ptr
        self.obs[i] = obs
        self.actions[i] = actions
        self.rewards[i] = rewards
        self.dones[i] = dones.astype(np.float32)
        self.log_probs[i] = log_probs
        self.values[i] = values
        self.action_masks[i] = action_masks
        self._ptr += 1

    def compute_advantages(
        self,
        last_values: np.ndarray,
        last_dones: np.ndarray,
        gamma: float = 0.99,
        gae_lambda: float = 0.95,
    ) -> None:
        gae = np.zeros(self.num_envs, dtype=np.float32)
        for step in reversed(range(self.steps_per_env)):
            if step == self.steps_per_env - 1:
                next_non_terminal = 1.0 - last_dones.astype(np.float32)
                next_values = last_values
            else:
                next_non_terminal = 1.0 - self.dones[step + 1]
                next_values = self.values[step + 1]
            delta = self.rewards[step] + gamma * next_values * next_non_terminal - self.values[step]
            gae = delta + gamma * gae_lambda * next_non_terminal * gae
            self.advantages[step] = gae

        self.returns = self.advantages + self.values

    def get_batches(self, mini_batch_size: int, device: torch.device) -> Iterator[RolloutBatch]:
        total = self.steps_per_env * self.num_envs
        idx = np.arange(total)
        np.random.shuffle(idx)

        obs = self.obs.reshape(total, self.obs_dim)
        actions = self.actions.reshape(total)
        log_probs = self.log_probs.reshape(total)
        values = self.values.reshape(total)
        returns = self.returns.reshape(total)
        advantages = self.advantages.reshape(total)
        masks = self.action_masks.reshape(total, self.action_dim)

        advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)

        for start in range(0, total, mini_batch_size):
            batch_idx = idx[start : start + mini_batch_size]
            yield RolloutBatch(
                obs=torch.as_tensor(obs[batch_idx], dtype=torch.float32, device=device),
                actions=torch.as_tensor(actions[batch_idx], dtype=torch.long, device=device),
                old_log_probs=torch.as_tensor(log_probs[batch_idx], dtype=torch.float32, device=device),
                old_values=torch.as_tensor(values[batch_idx], dtype=torch.float32, device=device),
                returns=torch.as_tensor(returns[batch_idx], dtype=torch.float32, device=device),
                advantages=torch.as_tensor(advantages[batch_idx], dtype=torch.float32, device=device),
                action_masks=torch.as_tensor(masks[batch_idx], dtype=torch.bool, device=device),
            )
