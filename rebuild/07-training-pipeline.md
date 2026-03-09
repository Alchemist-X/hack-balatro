# 训练流水线

## 总体流程

```
Step 1: 收集 GreedyAgent 轨迹       (collect_greedy_trajectories.py)
   │
Step 2: 行为克隆 (BC) 预训练          (train_bc.py / run_bc_greedy.py)
   │     GreedyAgent 轨迹 → 监督学习 → 初始化策略网络
   │
Step 3: PPO 在线强化学习              (train_ppo.py)
   │     从 BC checkpoint 出发 → 并行环境交互 → PPO 更新
   │
Step 4: 评估与对比                    (eval_run.py)
         PPO-BC vs BC-only vs GreedyAgent vs Random
```

一键完整流程：`scripts/train_full_pipeline.py`

## 行为克隆 (BC)

文件：`training/behavior_clone.py`

### 原理

通过监督学习模仿 GreedyAgent 的动作选择：
- 输入：GreedyAgent 产生的 `(observation, action)` 对
- 损失：CrossEntropyLoss
- 目标：初始化策略网络，为 PPO 提供良好起点

### 数据收集

```bash
python scripts/collect_greedy_trajectories.py --num-games 5000 --workers 10
# 产出: trajectories/greedy_5000.pt
# 格式: {"observations": Tensor(N, 454), "actions": Tensor(N,)}
```

每局 GreedyAgent 游戏约产生 ~100 步 transition，5000 局 ≈ 500K transitions。

### 训练流程

```python
cloner = BehaviorCloner(agent, lr=1e-3, batch_size=256)
cloner.train(
    trajectories_path="trajectories/greedy_5000.pt",
    num_epochs=100,
)
agent.save("checkpoints/bc_pretrained.pt")
```

### 关键实现细节

1. **Action Masking**: 从 obs 前 86 维提取合法动作掩码，训练时对 logits mask
   ```python
   action_mask = batch_obs[:, :86] > 0.5
   logits.masked_fill(~action_mask, -inf)
   loss = CrossEntropyLoss(logits, actions)
   ```

2. **数据加载**: `torch.load` → `TensorDataset` → `DataLoader(shuffle=True)`

3. **模型接口**: `logits, _values = agent.model(batch_obs)`

### BC 超参数

| 参数 | 值 | 说明 |
|------|-----|------|
| lr | 1e-3 | 学习率 |
| batch_size | 256-1024 | 批量大小 |
| num_epochs | 50-100 | 训练轮数 |
| loss | CrossEntropyLoss | 分类损失 |
| action_mask | 是 | 从 obs[:86] 提取 |

### BC 局限性

- **分布漂移 (distribution shift)**: 每步 0.3% 错误率在 ~60 步 episode 中累积 → 83.5% 概率至少犯一次错
- 一旦犯错 → 进入未见过的状态 → 连锁错误
- BC 模型的 episode length 只有 GreedyAgent 的 ~60%
- **结论**: BC 适合作为初始化，但不能替代 RL 训练

## PPO 训练

文件：`training/ppo.py`, `training/rollout.py`

### PPO 算法

Proximal Policy Optimization with Clipped Surrogate Objective:

```
L_clip = -min(ratio * A_t, clip(ratio, 1-ε, 1+ε) * A_t)
L_value = 0.5 * (V(s) - R_t)^2
L_entropy = -entropy_coef * H(π)
L_total = L_clip + value_loss_coef * L_value + L_entropy
```

### RolloutBuffer

文件：`training/rollout.py`

收集并行环境的 rollout 数据，计算 GAE (Generalized Advantage Estimation):

```python
buffer = RolloutBuffer(
    num_envs=64,
    steps_per_env=256,
    obs_dim=454,
    action_dim=86,
)

# 收集阶段
for step in range(steps_per_env):
    actions, log_probs, values = agent.act_batch(obs, masks)
    next_obs, rewards, dones, infos = envs.step(actions)
    buffer.add(obs, actions, rewards, dones, log_probs, values, masks)

# 计算 GAE
buffer.compute_advantages(last_values, last_dones, gamma=0.99, gae_lambda=0.95)

# 生成 mini-batch
for batch in buffer.get_batches(mini_batch_size=512):
    # PPO 更新
```

### PPO Trainer

文件：`training/ppo.py`

```python
config = PPOConfig(
    clip_range=0.2,
    entropy_coef=0.01,
    value_loss_coef=0.5,
    max_grad_norm=0.5,
    num_epochs=4,
)

trainer = PPOTrainer(agent, config, lr=2e-4)

for batch in buffer.get_batches(mini_batch_size=512):
    metrics = trainer.update(batch)
    # metrics: policy_loss, value_loss, entropy, approx_kl, clip_fraction
```

### PPO 超参数

| 参数 | 值 | 来源 |
|------|-----|------|
| gamma | 0.99 | configs/train.yaml |
| gae_lambda | 0.95 | configs/train.yaml |
| clip_range | 0.2 | configs/train.yaml |
| clip_range_vf | null | 无 value clipping |
| entropy_coef | 0.01 | configs/train.yaml |
| value_loss_coef | 0.5 | configs/train.yaml |
| max_grad_norm | 0.5 | configs/train.yaml |
| num_epochs | 4 | 每次 PPO update 的 epoch 数 |
| mini_batch_size | 512 | configs/train.yaml |
| num_envs | 64 | configs/train.yaml |
| steps_per_env | 256 | configs/train.yaml |
| lr | 2e-4 | configs/train.yaml |
| lr (init-weights) | 1e-4 | 从 BC 微调时自动减半 |
| scheduler | cosine (有 bug) | 推荐改为 constant |
| warmup_steps | 1000 | configs/train.yaml |
| total_env_steps | 100M | configs/train.yaml |
| checkpoint_interval | 500K | configs/train.yaml |
| eval_interval | 100K | configs/train.yaml |

### 训练脚本

```bash
# 从零开始
python scripts/train_ppo.py --config configs/train.yaml

# 从 BC 预训练权重开始
python scripts/train_ppo.py --init-weights checkpoints/bc_pretrained.pt

# 恢复训练
python scripts/train_ppo.py --resume checkpoints/latest.pt

# 使用 Transformer 模型
python scripts/train_ppo.py --model-type transformer --init-weights checkpoints/bc_transformer.pt
```

### 训练循环伪代码

```python
# 初始化
agent = PPOAgent(model_type="mlp", hidden_dim=512)
if init_weights:
    agent.load(init_weights)
trainer = PPOTrainer(agent, ppo_config, lr=lr)
envs = ParallelBalatroEnvs(num_envs=64)

# 主循环
total_steps = 0
while total_steps < total_steps_target:
    # 1. 收集 rollout
    buffer = RolloutBuffer(num_envs, steps_per_env, obs_dim, action_dim)
    for step in range(steps_per_env):
        actions, log_probs, values = agent.act_batch(obs, masks)
        next_obs, rewards, dones, infos = envs.step(actions)
        buffer.add(obs, actions, rewards, dones, log_probs, values, masks)
        obs = next_obs
        total_steps += num_envs

    # 2. 计算 GAE
    last_values = agent.get_values(obs)
    buffer.compute_advantages(last_values, last_dones)

    # 3. PPO 更新
    for epoch in range(num_epochs):
        for batch in buffer.get_batches(mini_batch_size):
            trainer.update(batch)
            scheduler.step()

    # 4. 评估与 checkpoint
    if total_steps % eval_interval == 0:
        metrics = evaluate_agent(agent, seeds[:20])
        if metrics["win_rate"] > best_win_rate:
            agent.save("checkpoints/best.pt")
    if total_steps % checkpoint_interval == 0:
        save_checkpoint(agent, trainer, total_steps)
```

## 课程学习

文件：`training/curriculum.py`

### 设计

按难度分 3 个阶段：

| 阶段 | max_ante | 进阶条件 |
|------|---------|----------|
| easy | 4 | win_rate >= min_win_rate 且 episodes >= min_episodes |
| standard | 8 | win_rate >= min_win_rate 且 episodes >= min_episodes |
| hard | 8 | 终极阶段 |

```python
scheduler = CurriculumScheduler(stages=[
    {"name": "easy", "max_ante": 4, "min_win_rate": 0.1, "min_episodes": 1000},
    {"name": "standard", "max_ante": 8, "min_win_rate": 0.05, "min_episodes": 5000},
    {"name": "hard", "max_ante": 8},
])
```

### 当前状态

课程学习的逻辑已实现，但 `train_ppo.py` 中尚未实际接入。需要在环境重置时设置 `max_ante` 参数。

## 完整流程脚本

文件：`scripts/train_full_pipeline.py`

```bash
python scripts/train_full_pipeline.py
```

自动执行：
1. 收集 GreedyAgent 轨迹 (5000 局)
2. BC 预训练
3. PPO 微调 (从 BC checkpoint)
4. 最终多 agent 对比评估

## Checkpoint 管理

| 文件 | 用途 |
|------|------|
| `checkpoints/bc_pretrained.pt` | BC 预训练权重 |
| `checkpoints/bc_greedy.pt` | BC (Greedy 轨迹) 权重 |
| `checkpoints/best.pt` | eval win rate 最高的 checkpoint |
| `checkpoints/latest.pt` | 最近一次 checkpoint |
| `checkpoints/step_{N}.pt` | 按步数保存 |
| `checkpoints/final.pt` | 训练结束最终 checkpoint |
