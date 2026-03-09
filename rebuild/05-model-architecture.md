# 模型架构

## 概览

项目实现了两种模型后端，通过 `model_type` 参数切换：

| 模型 | 参数量 | 输入 | 适用场景 |
|------|--------|------|---------|
| BalatroMLP | ~934K (hidden=512) | 454d flat obs | CPU 训练，当前默认 |
| TransformerWrapper → BalatroPolicyValueNet | ~15M | 454d flat → 结构化拆包 | GPU 训练，大规模实验 |

两种模型共享相同的 `forward(obs) -> (logits, value)` 接口。

## BalatroMLP — 默认模型

文件：`agents/ppo_agent.py`

### 结构

```
Input: obs (B, 454)
  │
  ▼
Shared MLP
  Linear(454 → 512) → ReLU
  Linear(512 → 512) → ReLU
  │
  ├──→ Policy Head
  │      Linear(512 → 512) → ReLU
  │      Linear(512 → 86)
  │      Output: logits (B, 86)
  │
  └──→ Value Head
         Linear(512 → 256) → ReLU
         Linear(256 → 1)
         Output: value (B,)
```

### 参数量计算

```
Shared:  454*512 + 512 + 512*512 + 512 = 232,448 + 262,656 = 495,104
Policy:  512*512 + 512 + 512*86 + 86   = 262,654 + 44,118  = 306,772
Value:   512*256 + 256 + 256*1 + 1     = 131,329            = 131,329
                                                     总计 ≈ 933,205
```

### 特点

- 简单直接：flat obs → shared features → 两个 head
- CPU 推理极快：batch=64 时 190K inferences/s
- 适合 <1M 参数规模，CPU 训练场景

## BalatroPolicyValueNet — Transformer 模型

文件：`models/policy_value_net.py`

### 整体结构

```
Input: flat obs (B, 454)
  │
  ▼ unpack_obs_to_structured()
  │
  ├──→ card_features (B, 8, 19)  ──→ CardEncoder ──→ h_cards (B, 64)
  │
  ├──→ joker_ids (B, 5)         ──→ JokerEncoder ──→ h_joker (B, 64)
  │     joker_attrs (B, 5, 1)
  │
  └──→ global_features (B, 169) ──→ Global MLP ───→ h_global (B, 128)
                                                         │
                                              concat ────┘
                                                │
                                          (B, 256) = 64+64+128
                                                │
                                          backbone_proj → (B, backbone_dim)
                                                │
                                          N x ResidualMLP
                                                │
                                     ┌──────────┼──────────┐
                                     ▼          ▼          ▼
                              Intent Head  Action Head  Value Head
                              (B, 8)       (B, 86)     (B,)
```

### CardEncoder

文件：`models/card_encoder.py`

```
Input: card_features (B, 8, 19)
  │
  Linear(19 → 64)            # 投影到 embedding 维度
  + pos_embedding[:, :8, :]  # 可学习位置编码
  │
  TransformerEncoder          # 2 层, 4 heads, FFN=256, GELU
    (src_key_padding_mask=card_mask)
  │
  LayerNorm
  │
  Mean Pooling                # 忽略 padding (card_mask=True 的位置)
  │
Output: h_cards (B, 64)
```

配置参数：
- `input_dim`: 19 (= 13 rank + 4 suit + 1 selected + 1 chip_value)
- `embedding_dim`: 64
- `num_heads`: 4
- `num_layers`: 2
- `max_seq_len`: 10 (预留)
- `dropout`: 0.1

### JokerEncoder

文件：`models/joker_encoder.py`

```
Input: joker_ids (B, 5), joker_attrs (B, 5, attr_dim)
  │
  ├──→ id_embedding(joker_ids)   → (B, 5, 64)   # Embedding(48, 64, padding_idx=0)
  │
  └──→ attr_proj(joker_attrs)    → (B, 5, 64)   # Linear(attr_dim → 64)
       │
       concat → (B, 5, 128)
       │
       combine: Linear(128 → 64)
       + pos_embedding[:, :5, :]
       │
       TransformerEncoder         # 2 层, 2 heads, FFN=256, GELU
         (src_key_padding_mask=joker_mask)
       │
       LayerNorm
       │
       Attention Pooling          # 与 CardEncoder 相同的 mean pooling
       │
Output: h_joker (B, 64)
```

配置参数：
- `num_joker_types`: 47 (当前实现) / 150 (完整游戏)
- `joker_attr_dim`: 1 (TransformerWrapper 中使用 dummy)
- `embedding_dim`: 64
- `num_heads`: 2
- `num_layers`: 1-2
- `max_jokers`: 5
- `dropout`: 0.1

### Global MLP

```
Input: global_features (B, 169)
  │
  Linear(169 → 128) → GELU
  Linear(128 → 128) → GELU
  │
Output: h_global (B, 128)
```

global_features (169 维) 由 `unpack_obs_to_structured` 组装：
- stage one-hot: 7
- scalars: 14
- selected hand type: 12
- best hand type: 12
- deck composition: 52
- discarded cards: 52
- joker shop: 10
- boss effect: 10

### Shared Backbone

```
Input: h_fused (B, 256) = concat(h_cards, h_joker, h_global)
  │
  backbone_proj: Linear(256 → backbone_hidden_dim)
  │
  N x ResidualMLP(backbone_hidden_dim)
    每个 ResidualMLP:
      x → Linear(dim → dim) → GELU → Dropout
        → Linear(dim → dim) → Dropout
        → LayerNorm(x + residual)
  │
Output: h_shared (B, backbone_hidden_dim)
```

配置参数：
- `backbone_hidden_dim`: 512 (config) / 256 (TransformerWrapper 默认)
- `backbone_layers`: 3 (config) / 2 (TransformerWrapper 默认)
- `dropout`: 0.1

### Output Heads

#### Intent Head (8 类)
```
Linear(backbone_dim → action_hidden_dim) → GELU
Linear(action_hidden_dim → 8)
```

8 种高层意图分类（play_best_hand, trigger_joker, discard, shop_buy 等）。当前训练中未直接使用，作为辅助信号。

#### Flat Action Head (86 维)
```
Linear(backbone_dim → action_hidden_dim) → GELU
Linear(action_hidden_dim → 86)
```

固定大小离散动作空间的 logits，经 action mask 后采样。

#### Per-Candidate Action Head (可选)
```
concat(h_shared.expand(B, C, -1), action_features) → (B, C, backbone_dim + feature_dim)
Linear(backbone_dim + feature_dim → action_hidden_dim) → GELU
Linear(action_hidden_dim → 1)
```

当提供 `action_features` 时使用，为每个候选动作独立打分。当前训练中未使用。

#### Value Head
```
Linear(backbone_dim → action_hidden_dim) → GELU
Linear(action_hidden_dim → 1)
```

## TransformerWrapper — 适配层

文件：`agents/ppo_agent.py`

`TransformerWrapper` 将 `BalatroPolicyValueNet` 封装为与 `BalatroMLP` 相同的接口：

```python
def forward(self, obs: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
    s = unpack_obs_to_structured(obs)       # flat → 结构化
    joker_attrs = torch.zeros(B, 5, 1, ...) # placeholder
    out = self.net(card_features=s["card_features"], ...)
    return out["action_logits"], out["value"]
```

TransformerWrapper 默认参数（比 configs/model.yaml 中的配置更小）：
- `card_embedding_dim`: 64
- `joker_embedding_dim`: 64
- `backbone_hidden_dim`: hidden_dim // 2 (= 256)
- `backbone_layers`: 2
- `action_hidden_dim`: hidden_dim // 2 (= 256)

## 模型选择依据

| 考虑因素 | BalatroMLP | TransformerWrapper |
|---------|------------|-------------------|
| 参数量 | ~934K | ~15M |
| CPU 推理速度 | 190K inf/s (batch=64) | 显著更慢 |
| MPS/GPU 优势 | 无 (CPU 更快) | 有 (参数 >5M) |
| 结构化理解 | 需从 flat obs 自行学习 | 专门编码 card/joker 结构 |
| 当前推荐 | 本地训练/快速迭代 | 云端 H100 大规模训练 |

## 检查点格式

### BC 预训练保存
```python
torch.save(model.state_dict(), "checkpoints/bc_pretrained.pt")
```

### PPO 完整 checkpoint
```python
{
    "model_state": agent.model.state_dict(),
    "model_type": "mlp" | "transformer",
    "trainer_state": {
        "optimizer": optimizer.state_dict(),
        "scheduler": scheduler.state_dict(),
        "update_count": int,
    },
    "total_env_steps": int,
    "episode_wins": [...],
}
```

### 加载方式
```python
# 仅权重
agent.load(path)  # → model.load_state_dict(..., strict=False)

# 完整恢复
ckpt = torch.load(path)
agent.model.load_state_dict(ckpt["model_state"])
trainer.load_state_dict(ckpt["trainer_state"])
```
