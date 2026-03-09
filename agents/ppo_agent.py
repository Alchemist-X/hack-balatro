from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from env.action_space import ACTION_DIM
from env.state_encoder import OBS_DIM, unpack_obs_to_structured

try:
    import torch
    from torch import nn
    from torch.distributions import Categorical

    from models.policy_value_net import BalatroPolicyValueNet, TransformerConfig

    TORCH_AVAILABLE = True
except Exception:  # pragma: no cover
    TORCH_AVAILABLE = False


@dataclass
class PPOInferenceOutput:
    action: np.ndarray
    log_prob: np.ndarray
    value: np.ndarray


if TORCH_AVAILABLE:

    class BalatroMLP(nn.Module):
        def __init__(self, hidden_dim: int = 512) -> None:
            super().__init__()
            self.shared = nn.Sequential(
                nn.Linear(OBS_DIM, hidden_dim),
                nn.ReLU(),
                nn.Linear(hidden_dim, hidden_dim),
                nn.ReLU(),
            )
            self.policy_head = nn.Sequential(
                nn.Linear(hidden_dim, hidden_dim),
                nn.ReLU(),
                nn.Linear(hidden_dim, ACTION_DIM),
            )
            self.value_head = nn.Sequential(
                nn.Linear(hidden_dim, hidden_dim // 2),
                nn.ReLU(),
                nn.Linear(hidden_dim // 2, 1),
            )

        def forward(self, obs: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
            h = self.shared(obs)
            logits = self.policy_head(h)
            value = self.value_head(h).squeeze(-1)
            return logits, value


    class TransformerWrapper(nn.Module):
        def __init__(self, hidden_dim: int = 512) -> None:
            super().__init__()
            cfg = TransformerConfig(
                backbone_hidden_dim=max(hidden_dim // 2, 128),
                action_hidden_dim=max(hidden_dim // 2, 128),
            )
            self.net = BalatroPolicyValueNet(cfg)

        def forward(self, obs: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
            structured = unpack_obs_to_structured(obs.detach().cpu().numpy())
            device = obs.device
            card_features = torch.as_tensor(structured["card_features"], dtype=torch.float32, device=device)
            card_mask = torch.as_tensor(structured["card_mask"], dtype=torch.bool, device=device)
            joker_ids = torch.as_tensor(structured["joker_ids"], dtype=torch.long, device=device)
            joker_mask = torch.as_tensor(structured["joker_mask"], dtype=torch.bool, device=device)
            global_features = torch.as_tensor(structured["global_features"], dtype=torch.float32, device=device)
            joker_attrs = torch.zeros(joker_ids.shape[0], joker_ids.shape[1], 1, device=device)

            out = self.net(
                card_features=card_features,
                card_mask=card_mask,
                joker_ids=joker_ids,
                joker_mask=joker_mask,
                global_features=global_features,
                joker_attrs=joker_attrs,
            )
            return out["action_logits"], out["value"]


    class PPOAgent:
        def __init__(
            self,
            model_type: str = "mlp",
            hidden_dim: int = 512,
            device: str | None = None,
        ) -> None:
            self.model_type = model_type
            self.device = torch.device(device or self._auto_device())

            if model_type == "transformer":
                self.model: nn.Module = TransformerWrapper(hidden_dim=hidden_dim)
            elif model_type == "mlp":
                self.model = BalatroMLP(hidden_dim=hidden_dim)
            else:
                raise ValueError(f"Unknown model type: {model_type}")

            self.model.to(self.device)

        @staticmethod
        def _auto_device() -> str:
            if torch.cuda.is_available():
                return "cuda"
            if getattr(torch.backends, "mps", None) and torch.backends.mps.is_available():
                return "mps"
            return "cpu"

        def _obs_tensor(self, obs: np.ndarray | torch.Tensor) -> torch.Tensor:
            if isinstance(obs, torch.Tensor):
                out = obs.float()
            else:
                out = torch.as_tensor(obs, dtype=torch.float32)
            if out.ndim == 1:
                out = out.unsqueeze(0)
            return out.to(self.device)

        def _mask_tensor(self, action_mask: np.ndarray | torch.Tensor) -> torch.Tensor:
            if isinstance(action_mask, torch.Tensor):
                mask = action_mask.bool()
            else:
                mask = torch.as_tensor(action_mask, dtype=torch.bool)
            if mask.ndim == 1:
                mask = mask.unsqueeze(0)
            return mask.to(self.device)

        def _dist(self, logits: torch.Tensor, action_mask: torch.Tensor) -> Categorical:
            masked_logits = logits.masked_fill(~action_mask, torch.finfo(logits.dtype).min)
            return Categorical(logits=masked_logits)

        @torch.no_grad()
        def act(
            self,
            obs: np.ndarray,
            info: dict[str, Any] | None = None,
            action_mask: np.ndarray | None = None,
        ) -> tuple[int, float, float]:
            del info
            if action_mask is None:
                action_mask = obs[:ACTION_DIM] > 0.5

            obs_t = self._obs_tensor(obs)
            mask_t = self._mask_tensor(action_mask)
            logits, value = self.model(obs_t)
            dist = self._dist(logits, mask_t)
            action = dist.sample()
            log_prob = dist.log_prob(action)

            return int(action.item()), float(log_prob.item()), float(value.item())

        @torch.no_grad()
        def act_batch(self, obs_batch: np.ndarray, mask_batch: np.ndarray) -> PPOInferenceOutput:
            obs_t = self._obs_tensor(obs_batch)
            mask_t = self._mask_tensor(mask_batch)

            logits, values = self.model(obs_t)
            dist = self._dist(logits, mask_t)
            actions = dist.sample()
            log_probs = dist.log_prob(actions)

            return PPOInferenceOutput(
                action=actions.detach().cpu().numpy(),
                log_prob=log_probs.detach().cpu().numpy(),
                value=values.detach().cpu().numpy(),
            )

        def evaluate(
            self,
            obs_batch: torch.Tensor,
            action_batch: torch.Tensor,
            mask_batch: torch.Tensor,
        ) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
            logits, values = self.model(obs_batch)
            dist = self._dist(logits, mask_batch)
            log_probs = dist.log_prob(action_batch)
            entropy = dist.entropy()
            return log_probs, values, entropy

        def save(self, path: str | Path) -> None:
            output = Path(path)
            output.parent.mkdir(parents=True, exist_ok=True)
            torch.save(
                {
                    "model_state": self.model.state_dict(),
                    "model_type": self.model_type,
                },
                output,
            )

        def load(self, path: str | Path, strict: bool = False) -> None:
            ckpt = torch.load(path, map_location=self.device)
            state = ckpt.get("model_state", ckpt)
            self.model.load_state_dict(state, strict=strict)

        def train(self) -> None:
            self.model.train()

        def eval(self) -> None:
            self.model.eval()

else:

    class PPOAgent:  # pragma: no cover
        def __init__(self, *args: Any, **kwargs: Any) -> None:
            del args, kwargs
            raise RuntimeError("PPOAgent requires torch installed. Install torch to use phase2/ppo.")
