# 动作空间设计

## 概览

动作空间为 `Discrete(86)`，使用 **action masking** 确保智能体只选择合法动作。

文件：`env/action_decoder.py`

## 完整动作映射表

| 索引 | 动作类型 | 数量 | 说明 |
|------|---------|------|------|
| 0-23 | select_card | 24 | 选中/取消选中手牌槽 0-23 |
| 24-46 | move_left | 23 | 将牌向左移动（训练时屏蔽） |
| 47-69 | move_right | 23 | 将牌向右移动（训练时屏蔽） |
| 70 | play | 1 | 打出选中的牌 |
| 71 | discard | 1 | 弃掉选中的牌 |
| 72 | cashout | 1 | 结算 (Ante 通过后) |
| 73-76 | buy_joker | 4 | 购买商店中的 Joker (4 个槽) |
| 77 | next_round | 1 | 进入下一回合 |
| 78 | select_blind | 1 | 选择盲注 |
| 79 | reroll_shop | 1 | 刷新商店 (花费 $5) |
| 80-84 | sell_joker | 5 | 出售持有的 Joker (5 个槽) |
| 85 | skip_blind | 1 | 跳过当前盲注 |

## Action Masking 机制

### 原理

每步中，环境通过 `get_action_mask()` 返回一个 86 维 bool 数组，True 表示该动作在当前状态下合法。

智能体必须只从合法动作中选择。实现方式：

```python
# 在 action logits 上施加 mask
logits[~action_mask] = -inf
action = Categorical(logits=logits).sample()
```

### move_left/move_right 被屏蔽

索引 24-69 共 46 个 move 动作在训练时被**永久屏蔽**：

```python
# env/balatro_gym_wrapper.py
mask[24:70] = False
```

原因：卡牌排列顺序对 Balatro 计分没有影响，但保留这些动作会：
1. 浪费探索预算（46/86 = 53% 的动作空间）
2. 增加学习难度（模型需要学习"这些动作无用"）
3. 减慢收敛速度

屏蔽后有效动作空间从 86 降到 40 个。

### 不同阶段的合法动作

| 游戏阶段 | 常见合法动作 |
|---------|-------------|
| PreBlind | select_blind, skip_blind |
| Blind | select_card, play, discard |
| PostBlind | next_round, cashout |
| Shop | buy_joker, sell_joker, reroll_shop, next_round |
| CashOut | cashout |
| End | (无合法动作，游戏结束) |

### 容错回退

如果智能体选择了非法动作，`BalatroEnv.step()` 会自动回退到第一个合法动作：

```python
if not mask[action]:
    valid = np.where(mask)[0]
    if len(valid) > 0:
        action = int(valid[0])
```

`ActionDecoder.decode()` 也提供了同样的回退逻辑。

## 动作空间常量

文件 `env/action_decoder.py` 定义了所有索引常量：

```python
ACTION_SPACE_SIZE = 86

SELECT_CARD_START = 0
SELECT_CARD_END = 24
MOVE_LEFT_START = 24
MOVE_LEFT_END = 47
MOVE_RIGHT_START = 47
MOVE_RIGHT_END = 70
PLAY_IDX = 70
DISCARD_IDX = 71
CASHOUT_IDX = 72
BUY_JOKER_START = 73
BUY_JOKER_END = 77
NEXT_ROUND_IDX = 77
SELECT_BLIND_IDX = 78
REROLL_SHOP_IDX = 79
SELL_JOKER_START = 80
SELL_JOKER_END = 85
SKIP_BLIND_IDX = 85
```

## 辅助函数

```python
action_type(action_idx)       # -> str, 返回动作名称
is_select_card(action_idx)    # -> bool
is_play(action_idx)           # -> bool
is_discard(action_idx)        # -> bool
is_shop_action(action_idx)    # -> bool
```

## 与 pylatro 引擎的映射

`engine.gen_action_space()` 返回长度为 86 的 `list[bool]`，直接对应上述索引。`engine.handle_action_index(idx)` 接受同样的索引执行动作。

动作索引在 Rust 端 (`core/src/action.rs`) 定义，Python 端完全复用。
