from __future__ import annotations

from dataclasses import dataclass

import torch
from torch import nn


class ResidualMLP(nn.Module):
    def __init__(self, dim: int, dropout: float = 0.1) -> None:
        super().__init__()
        self.net = nn.Sequential(
            nn.Linear(dim, dim),
            nn.GELU(),
            nn.Dropout(dropout),
            nn.Linear(dim, dim),
            nn.Dropout(dropout),
        )
        self.norm = nn.LayerNorm(dim)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.norm(x + self.net(x))


@dataclass
class TransformerConfig:
    card_embedding_dim: int = 64
    joker_embedding_dim: int = 64
    global_hidden_dim: int = 128
    backbone_hidden_dim: int = 256
    backbone_layers: int = 2
    action_hidden_dim: int = 256
    card_heads: int = 4
    joker_heads: int = 2
    card_layers: int = 2
    joker_layers: int = 1
    dropout: float = 0.1


class BalatroPolicyValueNet(nn.Module):
    def __init__(self, cfg: TransformerConfig | None = None) -> None:
        super().__init__()
        self.cfg = cfg or TransformerConfig()

        self.card_proj = nn.Linear(19, self.cfg.card_embedding_dim)
        self.card_pos = nn.Parameter(torch.zeros(1, 8, self.cfg.card_embedding_dim))
        card_layer = nn.TransformerEncoderLayer(
            d_model=self.cfg.card_embedding_dim,
            nhead=self.cfg.card_heads,
            dim_feedforward=256,
            dropout=self.cfg.dropout,
            batch_first=True,
            activation="gelu",
        )
        self.card_encoder = nn.TransformerEncoder(card_layer, num_layers=self.cfg.card_layers)

        self.joker_embedding = nn.Embedding(48, self.cfg.joker_embedding_dim, padding_idx=0)
        self.joker_attr_proj = nn.Linear(1, self.cfg.joker_embedding_dim)
        self.joker_combine = nn.Linear(self.cfg.joker_embedding_dim * 2, self.cfg.joker_embedding_dim)
        self.joker_pos = nn.Parameter(torch.zeros(1, 5, self.cfg.joker_embedding_dim))
        joker_layer = nn.TransformerEncoderLayer(
            d_model=self.cfg.joker_embedding_dim,
            nhead=self.cfg.joker_heads,
            dim_feedforward=256,
            dropout=self.cfg.dropout,
            batch_first=True,
            activation="gelu",
        )
        self.joker_encoder = nn.TransformerEncoder(joker_layer, num_layers=self.cfg.joker_layers)

        self.global_mlp = nn.Sequential(
            nn.Linear(169, self.cfg.global_hidden_dim),
            nn.GELU(),
            nn.Linear(self.cfg.global_hidden_dim, self.cfg.global_hidden_dim),
            nn.GELU(),
        )

        fused_dim = self.cfg.card_embedding_dim + self.cfg.joker_embedding_dim + self.cfg.global_hidden_dim
        self.backbone_proj = nn.Linear(fused_dim, self.cfg.backbone_hidden_dim)
        self.backbone = nn.Sequential(
            *[ResidualMLP(self.cfg.backbone_hidden_dim, self.cfg.dropout) for _ in range(self.cfg.backbone_layers)]
        )

        self.intent_head = nn.Sequential(
            nn.Linear(self.cfg.backbone_hidden_dim, self.cfg.action_hidden_dim),
            nn.GELU(),
            nn.Linear(self.cfg.action_hidden_dim, 8),
        )
        self.action_head = nn.Sequential(
            nn.Linear(self.cfg.backbone_hidden_dim, self.cfg.action_hidden_dim),
            nn.GELU(),
            nn.Linear(self.cfg.action_hidden_dim, 86),
        )
        self.value_head = nn.Sequential(
            nn.Linear(self.cfg.backbone_hidden_dim, self.cfg.action_hidden_dim),
            nn.GELU(),
            nn.Linear(self.cfg.action_hidden_dim, 1),
        )

    @staticmethod
    def _masked_mean(x: torch.Tensor, pad_mask: torch.Tensor) -> torch.Tensor:
        valid = (~pad_mask).float().unsqueeze(-1)
        denom = valid.sum(dim=1).clamp_min(1.0)
        return (x * valid).sum(dim=1) / denom

    def forward(
        self,
        card_features: torch.Tensor,
        card_mask: torch.Tensor,
        joker_ids: torch.Tensor,
        joker_mask: torch.Tensor,
        global_features: torch.Tensor,
        joker_attrs: torch.Tensor | None = None,
    ) -> dict[str, torch.Tensor]:
        if joker_attrs is None:
            joker_attrs = torch.zeros(
                joker_ids.shape[0],
                joker_ids.shape[1],
                1,
                device=joker_ids.device,
                dtype=torch.float32,
            )

        card_x = self.card_proj(card_features) + self.card_pos[:, : card_features.size(1)]
        card_h = self.card_encoder(card_x, src_key_padding_mask=card_mask)
        card_pool = self._masked_mean(card_h, card_mask)

        joker_id_emb = self.joker_embedding(joker_ids)
        joker_attr_emb = self.joker_attr_proj(joker_attrs)
        joker_x = self.joker_combine(torch.cat([joker_id_emb, joker_attr_emb], dim=-1))
        joker_x = joker_x + self.joker_pos[:, : joker_x.size(1)]
        joker_h = self.joker_encoder(joker_x, src_key_padding_mask=joker_mask)
        joker_pool = self._masked_mean(joker_h, joker_mask)

        global_h = self.global_mlp(global_features)
        fused = torch.cat([card_pool, joker_pool, global_h], dim=-1)
        shared = self.backbone(self.backbone_proj(fused))

        return {
            "intent_logits": self.intent_head(shared),
            "action_logits": self.action_head(shared),
            "value": self.value_head(shared).squeeze(-1),
        }
