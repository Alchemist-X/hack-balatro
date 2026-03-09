# 智能体实现

## 概览

| 智能体 | 文件 | 通关率 | 用途 |
|--------|------|--------|------|
| RandomAgent | `agents/random_agent.py` | 0% | 环境验证基线 |
| RuleBasedAgent | `agents/rule_based_agent.py` | ~0% | 规则基线 + 轨迹记录 |
| GreedyAgent | `agents/greedy_agent.py` | ~4% | 当前最强基线 + BC 数据源 |
| PPOAgent | `agents/ppo_agent.py` | 0-2% | RL 训练主体 |
| MCTSAgent | `agents/mcts_agent.py` | N/A | 搜索增强（未完成） |

所有智能体共享接口：`act(obs, info, action_mask) -> action` 或 `(action, log_prob, value)`

## RandomAgent

文件：`agents/random_agent.py` (48 行)

在 action mask 中为 True 的动作中均匀随机采样。

```python
def act(self, obs, info=None, action_mask=None):
    valid = np.where(action_mask)[0]
    return self.rng.choice(valid)
```

用途：验证环境是否正常工作，确认游戏能正确推进和终止。

## RuleBasedAgent

文件：`agents/rule_based_agent.py` (247 行)

按游戏阶段执行硬编码规则：

### 决策逻辑

| 阶段 | 策略 |
|------|------|
| play | 在 `valid_actions["play"]` 中选得分最高的牌型 |
| discard | 选第一个合法 discard |
| shop | 有钱就买第一个可买物品，否则 skip |
| blind_select | 固定选第一个盲注 |

牌型评估优先级：straight_flush > four_of_a_kind > full_house > flush > straight > three_of_kind > two_pair > pair > high_card

### 轨迹记录

```python
agent.start_recording()
# ... 运行若干局 ...
trajectories = agent.stop_recording()
# trajectories: list of (obs, action, phase, info)
```

依赖 `info` 中的 `phase`, `valid_actions`, `shop_items` 等结构化数据。如果 `BalatroEnv._get_info()` 未提供这些字段，会退化为选第一个合法动作。

## GreedyAgent — 当前最强基线

文件：`agents/greedy_agent.py` (366 行)

GreedyAgent 是项目中性能最好的非学习型智能体，通关率约 4%。它直接从 obs 向量中解析牌面信息做决策。

### 决策逻辑

#### 盲注阶段 (Blind)

1. **枚举所有合法牌组合**，用 `_classify_hand()` 评估每个组合
2. **选择得分最高的牌型**打出
3. **如果最高分 < 80** 且还有 discards/plays 剩余，优先**弃掉最差的牌**
4. 弃牌策略：找出贡献最低的牌（不参与任何 pair/flush/straight）

#### 商店阶段 (Shop)

1. 如果 Joker 槽未满，优先**买第一个可买的 Joker**
2. 否则尝试 **reroll shop**
3. 再否则 **next round**

#### 其他阶段

按动作优先级处理：SELECT_BLIND → CASHOUT → NEXT_ROUND

### 关键实现

- **`_extract_cards(obs)`**: 从 obs 偏移量 `ACTION_MASK_SIZE + NUM_STAGES + NUM_SCALARS` 处解析手牌的 rank/suit
- **`_classify_hand()`**: 使用 O(n) counting 算法分类牌型
- **`_find_best_hand()`**: 枚举可用牌组合，找最佳牌型
- **`_find_worst_cards()`**: 找出贡献最低的牌用于弃牌

### 历史 Bug

曾因硬编码 obs 偏移量 `offset = 86 + 7 + 12 = 105`，在 scalars 从 12 维扩展到 14 维后偏移错位，导致整个实验作废。修复后改为动态导入常量。

### 接口

```python
action, _, _ = agent.act(obs, info, action_mask)
# 返回 (action, 0.0, 0.0)，兼容 eval 的 (action,) 或 (action, log_prob, value) 形式
```

## PPOAgent

文件：`agents/ppo_agent.py` (384 行)

PPO 策略智能体，支持 MLP 和 Transformer 两种网络后端。

### 模型选择

```python
agent = PPOAgent(model_type="mlp")          # BalatroMLP (~934K params)
agent = PPOAgent(model_type="transformer")  # TransformerWrapper (~15M params)
```

### 动作选择

```python
def act(self, obs, info=None, action_mask=None):
    logits, value = self.model(obs_tensor)
    logits.masked_fill(~action_mask, -inf)
    dist = Categorical(logits=logits)
    action = dist.sample()
    return action, log_prob, value
```

### 批量推理

```python
actions, log_probs, values = agent.act_batch(obs_batch, mask_batch)
# obs_batch: (num_envs, 454), mask_batch: (num_envs, 86)
```

### PPO 更新评估

```python
log_probs, values, entropy = agent.evaluate(obs_batch, action_batch, mask_batch)
```

### 设备自动选择

```python
# 优先级: CUDA → MPS → CPU
# 当前结论: 对 <1M 参数的 MLP, CPU 更快
```

## MCTSAgent — 搜索增强（未完成）

文件：`agents/mcts_agent.py` (283 行)

在关键决策点使用 MCTS 增强 PPO 策略。

### 搜索触发条件

```python
def _should_search(self, info):
    # 1. Boss 盲注
    if info.get("is_boss_blind"):
        return True
    # 2. 剩余手数 ≤ 2 且分数差距 > 50%
    if plays <= 2 and score_gap > 0.5:
        return True
    # 3. 商店中有高价物品 (> 50% 预算)
    if is_shop and expensive_item:
        return True
    return False
```

### 决策流程

- **触发搜索**: 运行 MCTS，用访问次数构造改进策略并选动作
- **不触发**: 直接使用 PPO 策略

### 已知 Bug

`_get_priors()` 调用 `self.policy_agent._obs_to_tensors()` 和 `out["intent_logits"]`，但 PPOAgent 的 BalatroMLP 返回 `(logits, value)` 元组，没有这些方法/key。需要修复接口适配。

### 待完成

- 环境状态快照（用于 MCTS 展开）
- PPO 接口适配
- AlphaZero 风格训练目标
