# 设计决策 Rationale

本文档记录项目中每个关键设计决策的原因，以及对 rebuild 的具体建议。

## 1. Observation 设计：为什么 454d 而非 344d

**决策**: 从 344 维观测增强到 454 维，新增 poker hand 分类(24d)、joker multi-hot(47d)、弃牌历史(52d)、boss effect(10d)。

**原因**: MLP 模型很难从 rank/suit one-hot 自行推理出当前手牌构成什么牌型。增加 poker hand 分类 one-hot 后，BC 的 loss 降低 7 倍、reward 提升 4.8 倍。这是所有改进中 ROI 最高的。

**rebuild 建议**: 在设计 observation 时，优先把"模型难以自行推理但对决策关键"的信息直接编码进去。Balatro 中最关键的是牌型分类和 Joker 身份。不要让模型浪费容量去做本可以 O(1) 查表完成的推理。

## 2. 训练策略：为什么先 BC 再 PPO

**决策**: 用 GreedyAgent 轨迹做行为克隆（BC）初始化策略网络，再用 PPO 在线微调。

**原因**: Balatro 的奖励极度稀疏（通关率 ~4%），从随机策略出发的 PPO 几乎不可能获得正面信号。BC 提供了一个"能玩下去"的起点。但 BC 有根本的分布漂移问题——99.7% 训练准确率只转化为 2% 通关率，每步 0.3% 错误在 ~60 步中累积。

**rebuild 建议**:
1. 先写好 GreedyAgent（枚举最优牌型 + 简单商店规则），确保其通关率 >3%
2. 收集 5000+ 局轨迹做 BC 预训练
3. BC checkpoint 作为 PPO 的 `--init-weights`
4. 未来可考虑 DAgger（迭代收集 BC 模型运行时的 expert label 重新训练）以缓解分布漂移

## 3. 学习率调度：为什么用 constant LR 而非 cosine

**决策**: PPO 训练推荐使用 constant LR = 1e-4，不要使用 cosine annealing。

**原因**: cosine scheduler 的 `T_max` 必须精确匹配 optimizer step 总数。PPO 的 step 计数很容易算错（`num_epochs * num_mini_batches * num_updates`），导致 LR 周期过短：5M 步训练中 LR 完成了 ~32 个完整 cosine 周期，大部分时间 LR 接近 0。这种错误不会报错，但会静默破坏训练。

**rebuild 建议**: 起步阶段一律使用 constant LR。只在确认训练能正常收敛后，才考虑加入 cosine/linear decay，并务必验证 `T_max` 的计算。

## 4. 设备选择：为什么 CPU 优于 MPS

**决策**: 对 <1M 参数的 BalatroMLP，使用 CPU 训练而非 Apple MPS。

**原因**: Apple M4 CPU 有 NEON/AMX 向量加速器，对小矩阵乘法很高效。CPU→GPU 数据传输开销 (~0.1ms/batch) 在小模型下占总时间 >50%。实测 CPU 推理比 MPS 快 7-12 倍。

**rebuild 建议**:
- 模型 <1M 参数 → 用 CPU
- 模型 >5M 参数 + batch_size >1024 → 用 GPU (MPS/CUDA)
- 始终做一次快速 benchmark 再决定

## 5. Obs 编码性能：为什么 O(n) counting 而非 combinations

**决策**: 牌型分类使用 `rank_counts[13]` + `suit_counts[4]` 固定数组计数，不使用 `itertools.combinations`。

**原因**: `encode_pylatro_state()` 在每个 env step 都被调用，是训练热路径。combinations 版本将训练速度从 18K sps 降到 2.1K sps（9 倍慢）。O(n) counting 恢复到 15.7K sps。

**rebuild 建议**: 在 per-step 函数中：
- 不要用 `itertools.combinations`、`collections.Counter`、`sorted()` 等创建临时对象的操作
- 用固定大小数组代替动态数据结构
- 写完编码器后立即做 micro-benchmark，确保 >50K enc/s

## 6. 奖励塑形：推荐参数

**决策**: 增大 reward shaping 强度。

**原因**: 默认 `score_shaping_scale=0.1` 太弱，每步平均奖励只有 ~0.005，信噪比极低。`blind_pass_reward=0.5` 也不够明确。

**rebuild 建议**:
```yaml
reward:
  win_reward: 10.0
  blind_pass_reward: 2.0          # 从 0.5 提高
  boss_pass_reward: 3.0           # 新增
  score_shaping_scale: 0.5        # 从 0.1 提高
  death_penalty: 0.0              # 保持零——避免过度保守
  entropy_coef: 0.03              # PPO entropy 从 0.01 提高
```

## 7. Action Space：为什么屏蔽 move_left/move_right

**决策**: 动作索引 24-69（move_left/move_right 共 46 个）被永久 mask 为 False。

**原因**: 卡牌排列顺序对 Balatro 计分没有影响。保留这些动作会浪费 53% 的探索预算（46/86），减慢收敛。屏蔽后有效动作空间从 86 降到 40。

**rebuild 建议**: 直接在 `get_action_mask()` 中 `mask[24:70] = False`。如果未来需要牌序（例如某些 Joker 效果依赖位置），再解除。

## 8. 模型选择：MLP vs Transformer

**决策**: 默认使用 BalatroMLP (~934K params)，Transformer (~15M params) 作为备选。

**原因**: MLP 在 CPU 上推理极快（190K inf/s），适合快速迭代。Transformer 的 CardEncoder + JokerEncoder 能更好地处理变长输入和 Joker 间的注意力关系，但需要 GPU 才有速度优势。

**rebuild 建议**: 先用 MLP 跑通全流程（BC + PPO），确认管线正确后再切换 Transformer。过早使用大模型会拖慢调试循环。

## 9. 偏移量管理：不要硬编码

**决策**: 所有从 obs 解析数据的代码必须通过常量引用偏移量，不能写死数字。

**原因**: `NUM_SCALARS` 从 12 改为 14 时，GreedyAgent 中 4 处硬编码的 `offset = 86 + 7 + 12` 全部失效，导致牌面解析错误。这个 bug 使得一整轮实验作废。

**rebuild 建议**:
```python
from env.state_encoder import ACTION_MASK_SIZE, NUM_STAGES, NUM_SCALARS
offset = ACTION_MASK_SIZE + NUM_STAGES + NUM_SCALARS
```
在 `state_encoder.py` 中导出所有段的起始偏移量常量。任何新增 obs 段都要同步更新这些常量。
