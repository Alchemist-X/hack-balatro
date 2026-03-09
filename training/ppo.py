from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import torch

from training.rollout import RolloutBatch


@dataclass
class PPOConfig:
    clip_range: float = 0.2
    entropy_coef: float = 0.01
    value_loss_coef: float = 0.5
    max_grad_norm: float = 0.5
    num_epochs: int = 4


class PPOTrainer:
    def __init__(
        self,
        agent,
        config: PPOConfig,
        lr: float = 2e-4,
        scheduler_name: str = "constant",
        total_updates: int = 1000,
    ) -> None:
        self.agent = agent
        self.config = config
        self.optimizer = torch.optim.Adam(self.agent.model.parameters(), lr=lr)
        self.scheduler = self._build_scheduler(scheduler_name, total_updates)
        self.update_count = 0

    def _build_scheduler(self, name: str, total_updates: int):
        if name == "cosine":
            return torch.optim.lr_scheduler.CosineAnnealingLR(
                self.optimizer,
                T_max=max(1, total_updates),
                eta_min=1e-6,
            )
        return torch.optim.lr_scheduler.LambdaLR(self.optimizer, lambda _: 1.0)

    def update(self, batch: RolloutBatch) -> dict[str, float]:
        self.agent.train()

        log_probs, values, entropy = self.agent.evaluate(
            batch.obs,
            batch.actions,
            batch.action_masks,
        )

        ratio = (log_probs - batch.old_log_probs).exp()
        unclipped = ratio * batch.advantages
        clipped = torch.clamp(ratio, 1.0 - self.config.clip_range, 1.0 + self.config.clip_range) * batch.advantages
        policy_loss = -torch.min(unclipped, clipped).mean()

        value_loss = 0.5 * torch.nn.functional.mse_loss(values, batch.returns)
        entropy_loss = -entropy.mean()

        total_loss = (
            policy_loss
            + self.config.value_loss_coef * value_loss
            + self.config.entropy_coef * entropy_loss
        )

        self.optimizer.zero_grad(set_to_none=True)
        total_loss.backward()
        grad_norm = torch.nn.utils.clip_grad_norm_(
            self.agent.model.parameters(),
            self.config.max_grad_norm,
        )
        self.optimizer.step()

        approx_kl = 0.5 * ((log_probs - batch.old_log_probs) ** 2).mean()
        clip_fraction = ((ratio - 1.0).abs() > self.config.clip_range).float().mean()

        self.update_count += 1
        self.scheduler.step()

        lr = self.optimizer.param_groups[0]["lr"]
        return {
            "loss": float(total_loss.item()),
            "policy_loss": float(policy_loss.item()),
            "value_loss": float(value_loss.item()),
            "entropy": float(entropy.mean().item()),
            "entropy_loss": float(entropy_loss.item()),
            "approx_kl": float(approx_kl.item()),
            "clip_fraction": float(clip_fraction.item()),
            "grad_norm": float(grad_norm.item() if hasattr(grad_norm, "item") else grad_norm),
            "lr": float(lr),
        }

    def state_dict(self) -> dict[str, Any]:
        return {
            "optimizer": self.optimizer.state_dict(),
            "scheduler": self.scheduler.state_dict(),
            "update_count": self.update_count,
        }

    def load_state_dict(self, state: dict[str, Any]) -> None:
        if not state:
            return
        if "optimizer" in state:
            self.optimizer.load_state_dict(state["optimizer"])
        if "scheduler" in state:
            self.scheduler.load_state_dict(state["scheduler"])
        self.update_count = int(state.get("update_count", 0))
