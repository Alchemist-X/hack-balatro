from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

import numpy as np
import torch
from torch import nn
from torch.utils.data import DataLoader, TensorDataset


@dataclass
class BCTrainResult:
    final_loss: float
    epochs: int
    samples: int


class BehaviorCloner:
    def __init__(self, agent, lr: float = 1e-3, batch_size: int = 256):
        self.agent = agent
        self.batch_size = batch_size
        self.optimizer = torch.optim.Adam(self.agent.model.parameters(), lr=lr)
        self.loss_fn = nn.CrossEntropyLoss()

    def train(
        self,
        trajectories_path: str | Path,
        num_epochs: int = 10,
    ) -> BCTrainResult:
        data = torch.load(trajectories_path, map_location="cpu")
        obs = data["observations"].float()
        actions = data["actions"].long()

        dataset = TensorDataset(obs, actions)
        loader = DataLoader(dataset, batch_size=self.batch_size, shuffle=True)

        self.agent.train()
        final_loss = 0.0
        for _epoch in range(num_epochs):
            for batch_obs, batch_actions in loader:
                batch_obs = batch_obs.to(self.agent.device)
                batch_actions = batch_actions.to(self.agent.device)

                action_mask = batch_obs[:, :86] > 0.5
                logits, _ = self.agent.model(batch_obs)
                masked_logits = logits.masked_fill(~action_mask, torch.finfo(logits.dtype).min)
                loss = self.loss_fn(masked_logits, batch_actions)

                self.optimizer.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_(self.agent.model.parameters(), max_norm=1.0)
                self.optimizer.step()

                final_loss = float(loss.item())

        return BCTrainResult(final_loss=final_loss, epochs=num_epochs, samples=int(obs.shape[0]))


def collect_greedy_trajectories(
    env_factory: Callable[[int], Any],
    expert_agent: Any,
    num_games: int,
    max_steps: int = 400,
) -> dict[str, torch.Tensor]:
    observations: list[np.ndarray] = []
    actions: list[int] = []

    for seed in range(num_games):
        env = env_factory(seed)
        obs, info = env.reset(seed=seed)
        terminated = False
        truncated = False
        step = 0

        while not (terminated or truncated) and step < max_steps:
            mask = env.get_action_mask()
            action = int(expert_agent.act(obs, info, mask))
            observations.append(obs.copy())
            actions.append(action)

            obs, _, terminated, truncated, info = env.step(action)
            step += 1

    obs_tensor = torch.as_tensor(np.asarray(observations, dtype=np.float32), dtype=torch.float32)
    action_tensor = torch.as_tensor(np.asarray(actions, dtype=np.int64), dtype=torch.long)
    return {
        "observations": obs_tensor,
        "actions": action_tensor,
    }
