# 观测空间设计

## 概览

观测空间经历了两个版本的演进：

| 版本 | 维度 | 主要变化 |
|------|------|---------|
| v1 | 344 | 基础版本，缺少牌型分类和完整 Joker 信息 |
| v2 | 454 | 增加 poker hand 分类、Joker multi-hot、弃牌历史、Boss 效果 |

v2 相比 v1，在相同训练数据上 BC loss 降低 7 倍、reward 提升 4.8 倍。

## v2 完整布局 (454 维) — 当前版本

```
偏移量      段             维度    编码方式
─────────────────────────────────────────────────────────
0-85       action_mask     86     bool (1=合法动作)
86-92      stage           7      one-hot (游戏阶段)
93-106     scalars         14     归一化浮点数
107-258    hand_cards      152    8 张牌 x 19 维特征
259-270    selected_hand   12     one-hot (已选牌型)
271-282    best_hand       12     one-hot (最佳可用牌型)
283-334    deck_comp       52     multi-hot (牌组剩余)
335-386    discarded       52     multi-hot (已弃牌)
387-433    joker_held      47     multi-hot (持有 Joker)
434-443    joker_shop      10     2 x 5 维特征
444-453    boss_effect     10     one-hot (Boss 效果)
─────────────────────────────────────────────────────────
总计                       454
```

## 各段详细定义

### 1. Action Mask (86 维, 偏移 0)

每位对应一个动作，1.0 = 合法，0.0 = 非法。详见 [04-action-space.md](04-action-space.md)。

训练时，索引 24-69（move_left/move_right）被永久 mask 为 0，因为牌序对得分无影响。

### 2. Stage One-hot (7 维, 偏移 86)

| 索引 | 阶段 |
|------|------|
| 0 | Stage_PreBlind |
| 1 | Stage_Blind |
| 2 | Stage_PostBlind |
| 3 | Stage_Shop |
| 4 | Stage_End |
| 5 | Stage_CashOut |
| 6 | Stage_Other |

### 3. Scalars (14 维, 偏移 93)

| 索引 | 字段 | 归一化分母 | 说明 |
|------|------|-----------|------|
| 0 | score | 100,000 | 当前得分 |
| 1 | required_score | 100,000 | 本盲注所需分数 |
| 2 | score_ratio | 1.0 | score / required_score |
| 3 | plays | 10.0 | 剩余出牌次数 |
| 4 | discards | 10.0 | 剩余弃牌次数 |
| 5 | money | 100.0 | 当前金钱 |
| 6 | round | 10.0 | 回合数 |
| 7 | num_available | 24.0 | 手牌中可用牌数 |
| 8 | num_selected | 10.0 | 已选中牌数 |
| 9 | num_jokers | 5.0 | 持有 Joker 数 |
| 10 | num_deck | 60.0 | 牌组剩余牌数 |
| 11 | num_valid_actions | 79.0 | 合法动作数 |
| 12 | ante | 8.0 | 当前 Ante 等级 |
| 13 | best_hand_score_norm | 1.0 | 最佳手牌估计得分 / 所需分数 (裁剪至 [0, 1]) |

### 4. Hand Cards (152 维, 偏移 107)

8 个卡牌槽 x 19 维特征。未使用的槽全零填充。

每张牌 19 维:

| 偏移 | 维度 | 编码 |
|------|------|------|
| 0-12 | rank | 13 维 one-hot (2, 3, ..., K, A) |
| 13-16 | suit | 4 维 one-hot (spade, heart, diamond, club) |
| 17 | selected | 1 维 (1.0=已选中, 0.0=未选中) |
| 18 | chip_value | 1 维 (归一化: chip_value / 11.0) |

### 5. Selected Hand Type (12 维, 偏移 259)

当前已选中牌的 poker hand 分类，one-hot 编码。

| 索引 | 牌型 |
|------|------|
| 0 | high_card |
| 1 | pair |
| 2 | two_pair |
| 3 | three_of_kind |
| 4 | straight |
| 5 | flush |
| 6 | full_house |
| 7 | four_of_kind |
| 8 | straight_flush |
| 9 | five_of_kind |
| 10 | flush_house |
| 11 | flush_five |

如果未选中任何牌，则全零。

### 6. Best Hand Type (12 维, 偏移 271)

当前所有可用手牌能构成的最佳牌型，one-hot 编码。索引同上。

这是 v2 最关键的新增特征——让模型无需自行从 rank/suit one-hot 推理牌型。

### 7. Deck Composition (52 维, 偏移 283)

牌组中剩余牌的 multi-hot 编码。每位对应一张特定的牌：

```
索引 = rank_index * 4 + suit_index
```

其中 rank_index 0-12 (2 到 A)，suit_index 0-3。

### 8. Discarded Cards (52 维, 偏移 335)

本轮已弃牌的 multi-hot 编码，格式同 Deck Composition。

这个特征帮助模型理解"哪些牌已经不在牌组中"，对弃牌决策和概率推算至关重要。

### 9. Joker Held Multi-hot (47 维, 偏移 387)

47 种 Joker 的 multi-hot 编码。每位对应一种 Joker 类型。

```
bit 0  = TheJoker
bit 1  = JollyJoker
bit 2  = ZanyJoker
...
bit 46 = BullJoker
```

替代了 v1 中的 per-slot 特征编码（5 slots x 5 features = 25 维），后者由于 type_id 归一化到 [0,1]，47 种 Joker 的区分度极低。

### 10. Joker Shop (10 维, 偏移 434)

商店中 2 个 Joker 槽 x 5 维特征:

| 偏移 | 维度 | 说明 |
|------|------|------|
| 0 | 1 | type_id / 47 (归一化类型 ID) |
| 1 | 1 | is_chips (是否提供 chips) |
| 2 | 1 | is_mult (是否提供 mult) |
| 3 | 1 | is_xmult (是否提供 x_mult) |
| 4 | 1 | is_economy (是否经济类) |

### 11. Boss Effect (10 维, 偏移 444)

当前 Boss Blind 效果的 one-hot 编码:

| 索引 | Boss 效果 |
|------|-----------|
| 0 | None (非 Boss 盲注) |
| 1 | TheClub (黑桃花色被禁) |
| 2 | TheGoad (红心花色被禁) |
| 3 | TheHead (方块花色被禁) |
| 4 | ThePlant (梅花花色被禁) |
| 5 | TheWall (大盲注) |
| 6 | TheWheel (随机翻转) |
| 7 | TheArm (降级手牌等级) |
| 8 | ThePillar (重复牌被禁) |
| 9 | Other (其他未知效果) |

## v1 布局 (344 维) — 已弃用

```
偏移量      段              维度    编码方式
─────────────────────────────────────────────────────────
0-85       action_mask      86     bool
86-92      stage            7      one-hot
93-104     scalars          12     归一化浮点 (无 ante, best_hand_score_norm)
105-256    hand_cards       152    8 x 19
257-308    deck_composition 52     multi-hot
309-343    joker_slots      35     7 x 5 (per-slot 特征)
─────────────────────────────────────────────────────────
总计                        344
```

v1 缺失的信息:
- 不知道当前手牌构成什么 poker hand
- 不知道哪些牌已弃掉
- Joker 的具体效果丢失（47 种压缩到 per-slot 5 维特征）
- 不知道当前 ante 和 boss effect

## 编码实现

### StateEncoder

文件：`env/state_encoder.py`

核心方法 `encode_pylatro_state(state, action_mask)`:

1. 预分配 454 维零向量
2. 按段依次填入：action_mask → stage → scalars → hand_cards → hand_types → deck → discarded → jokers → shop → boss
3. 对每个 scalar 字段按预定义分母归一化
4. 牌面编码：rank one-hot + suit one-hot + selected flag + chip_value 归一化
5. 牌型分类使用 O(n) counting 算法

### O(n) 牌型分类

`classify_hand_direct(ranks, suits)` 实现:

```python
rank_counts = [0] * 13
suit_counts = [0] * 4
for r in ranks:
    rank_counts[r] += 1
for s in suits:
    suit_counts[s] += 1

max_rank_count = max(rank_counts)
num_pairs = sum(1 for c in rank_counts if c >= 2)
has_flush = n >= 5 and max(suit_counts) >= 5
has_straight = _check_straight_fast(rank_counts)
```

然后按优先级判断：flush_five > flush_house > five_of_kind > straight_flush > four_of_kind > full_house > flush > straight > three_of_kind > two_pair > pair > high_card

性能对比:
| 版本 | 方法 | 编码速度 | 训练 SPS |
|------|------|---------|---------|
| v1 (combinations) | C(8,5)=56 组合枚举 | 5K enc/s | 2,100 |
| v2 (O(n) counting) | 固定数组计数 | 68K enc/s | 15,700 |

### unpack_obs_to_structured

文件：`env/state_encoder.py`

将 454 维 flat observation 拆包为 Transformer 所需的结构化输入：

```python
result = {
    "card_features":   (B, 8, 19),   # 手牌特征
    "card_mask":       (B, 8),       # True = padding
    "joker_ids":       (B, 5),       # Joker 类型 ID (1-indexed, 0=pad)
    "joker_mask":      (B, 5),       # True = padding
    "global_features": (B, 169),     # 所有其余特征
    "action_mask":     (B, 86),      # True = 非法动作
}
```

global_features (169 维) = stage(7) + scalars(14) + selected_hand(12) + best_hand(12) + deck(52) + discarded(52) + joker_shop(10) + boss(10)
