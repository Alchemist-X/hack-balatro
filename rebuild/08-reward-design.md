# 奖励设计

## 奖励结构

奖励由三层组成，从密集到稀疏：

```
总奖励 = score_shaping + blind_pass_reward + win_reward
```

文件：`env/balatro_gym_wrapper.py` 中的 `_compute_reward()`

## 各层奖励详情

### 1. Score Shaping — 密集信号

```python
if score_delta > 0 and required_score > 0:
    reward += score_shaping_scale * log1p(score_delta / required_score)
```

| 参数 | 值 | 说明 |
|------|-----|------|
| score_shaping_scale | 0.1 | 缩放系数 |
| score_delta | state.score - prev_score | 本步得分增量 |
| required_score | state.required_score | 当前盲注所需分数 |

**设计理由**: 使用 `log1p` 压缩大得分，避免高分 hand 对策略的过度影响。归一化到 `required_score` 让不同 Ante 下的奖励具有可比性。

**问题**: `scale=0.1` 太小，每步平均奖励只有 ~0.005，信噪比极低。建议提高到 0.5。

### 2. Blind Pass — 中频信号

```python
stage_name = type(state.stage).__name__
if "PostBlind" in stage_name:
    reward += blind_pass_reward
```

| 参数 | 值 | 说明 |
|------|-----|------|
| blind_pass_reward | 0.5 | 通过盲注奖励 |

**设计理由**: 通过一个盲注是有意义的进度标志，提供比纯 score shaping 更明确的阶段性信号。

**问题**: 0.5 可能太小。建议提高到 2.0。

### 3. Win Reward — 稀疏终端信号

```python
if engine.is_over and engine.is_win:
    reward += win_reward
```

| 参数 | 值 | 说明 |
|------|-----|------|
| win_reward | 10.0 | 通关奖励 |

**设计理由**: 通关是最终目标，给予最大奖励。

**问题**: 当通关率接近 0% 时，这个信号几乎不存在。需要依赖其他层的奖励引导学习。

## 配置文件

来源：`configs/train.yaml`

```yaml
reward:
  win_reward: 10.0
  blind_pass_reward: 0.5
  boss_pass_reward: 1.0    # 配置存在，实现中未使用
  death_penalty: 0.0       # 死亡无额外惩罚
  use_score_shaping: true
  score_shaping_scale: 0.1
  streak_bonus: 0.2        # 配置存在，实现中未使用
```

## 未实现的奖励组件

| 组件 | 配置值 | 状态 | 设计意图 |
|------|--------|------|---------|
| boss_pass_reward | 1.0 | 未实现 | Boss 盲注通过应给更高奖励 |
| death_penalty | 0.0 | 已实现但为零 | 失败不额外惩罚，避免过度保守 |
| streak_bonus | 0.2 | 未实现 | 鼓励连续通过盲注 |

## 稀疏奖励问题分析

### 问题量化

在 E3 的 5M 步 / 137K episodes PPO 训练中：
- `win_reward=10.0` → **0 次 win** → 永远没有 win 信号
- `blind_pass_reward=0.5` → agent 很少通过 blind
- `score_shaping_scale=0.1` 的 log-scale 奖励太微弱
- 每步平均奖励只有 ~0.005

### 对 PPO 的影响

1. **梯度信号弱**: 低奖励 → 低 advantage 方差 → 策略几乎不更新
2. **Value 估计困难**: V(s) 几乎恒为 ~0，无法区分好坏状态
3. **Entropy 坍缩**: 缺乏正面反馈 → 策略趋向确定性（但不是好的确定性）

### 建议改进

| 改进 | 建议值 | 理由 |
|------|--------|------|
| score_shaping_scale | 0.5 | 增强密集信号强度 |
| blind_pass_reward | 2.0 | 更明确的阶段性奖励 |
| boss_pass_reward | 3.0 | Boss 盲注是关键决策点 |
| streak_bonus | 0.5 | 鼓励连续进步 |
| entropy_coef | 0.03-0.05 | 防止策略过早坍缩 |

### Reward Shaping 的理论考虑

当前的 reward shaping 不满足 potential-based shaping 的条件（不是状态势函数的差），因此可能改变最优策略。但在实践中，由于通关率极低，密集奖励对引导探索至关重要。

## 奖励信号的时间分布

一局典型游戏（~100 步）的奖励分布：

```
Step  1-10:  select_blind, 选牌    → 奖励 ~0
Step 11-20:  出牌得分               → score shaping ~0.01-0.05
Step 21-25:  通过盲注              → blind_pass 0.5
Step 26-30:  商店                   → 奖励 ~0
...
Step 90-100: 最终盲注              → 失败: 0 / 通关: 10.0
```

大部分步骤的奖励接近 0，奖励信号高度集中在少数关键时刻。
