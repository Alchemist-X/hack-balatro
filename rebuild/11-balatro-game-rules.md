# Balatro 游戏规则与数据获取

## 信息获取方式总览

| 信息类型 | 首选来源 | URL |
|---------|---------|-----|
| 全部 Joker 数据 (结构化) | Balatro Wiki Lua 表 | https://balatrowiki.org/w/Module:Jokers/data |
| Joker 效果描述 | Balatro Wiki | https://balatrowiki.org/w/Jokers |
| Blinds 与 Boss 效果 | Balatro Wiki | https://balatrowiki.org/w/Blinds |
| 计分公式 | Balatro Wiki | https://balatrowiki.org/w/Scoring |
| 扑克牌型 | Balatro Wiki | https://balatrowiki.org/w/Poker_Hands |
| 塔罗牌效果 | Balatro Wiki | https://balatrowiki.org/w/Tarot_Cards |
| 行星牌效果 | Balatro Wiki | https://balatrowiki.org/w/Planet_Cards |
| 幽灵牌效果 | Balatro Wiki | https://balatrowiki.org/w/Spectral_Cards |
| 优惠券系统 | Balatro Wiki | https://balatrowiki.org/w/Vouchers |
| 牌组变体 | Balatro Wiki | https://balatrowiki.org/w/Decks |
| 难度等级 | Balatro Wiki | https://balatrowiki.org/w/Stakes |
| 权威源码 (ground truth) | 游戏 Lua 文件 | 见下方"游戏源码获取" |
| 第三方 Joker 表格 | balatro.wiki | https://balatro.wiki/Jokers |

### 游戏源码获取

Balatro 使用 LOVE2D 引擎，游戏逻辑以 Lua 编写。源码是所有数值和效果的**权威定义**。

- **macOS**: 右键 `Balatro.app` → Show Package Contents → `Contents/Resources/Game/game.lua`
- **Windows**: `Steam/steamapps/common/Balatro/Balatro.exe`（用 7zip 解压 `.exe` 得到 `.love` 文件，再解压得到 Lua 源码）
- **GitHub 提取版**: https://github.com/vibezfire/balatro-game.lua-files

---

## 核心游戏循环

来源: https://balatrowiki.org/w/Balatro_Wiki

```
选择盲注 (Small/Big/Boss)
    │
    ▼
出牌阶段 ──→ 选牌 → 出牌 (得分) / 弃牌 (改善手牌)
    │         │
    │         ▼
    │       得分 >= 所需分数?
    │         ├── 是 → 通过盲注 → 商店
    │         └── 否 → 继续出牌/弃牌 (或失败)
    │
    ▼
商店阶段 ──→ 购买 Joker / 行星牌 / 塔罗牌
    │
    ▼
下一个 Ante ──→ 3 个盲注 (Small → Big → Boss)
    │
    ▼
Ante 8 通关 = 胜利
```

## 计分公式

来源: https://balatrowiki.org/w/Scoring

```
最终得分 = (base_chips + card_chips) * (base_mult + card_mult) * joker_multipliers
```

### 基础牌型分数

来源: https://balatrowiki.org/w/Poker_Hands

| 牌型 | Base Chips | Base Mult |
|------|-----------|-----------|
| High Card | 5 | 1 |
| Pair | 10 | 2 |
| Two Pair | 20 | 2 |
| Three of a Kind | 30 | 3 |
| Straight | 30 | 4 |
| Flush | 35 | 4 |
| Full House | 40 | 4 |
| Four of a Kind | 60 | 7 |
| Straight Flush | 100 | 8 |
| Five of a Kind | 120 | 12 |
| Flush House | 140 | 14 |
| Flush Five | 160 | 16 |

每张参与计分的牌贡献其 chip_value: 2-10 面值, J/Q/K = 10, A = 11

### 手牌升级

来源: https://balatrowiki.org/w/Planet_Cards

通过行星牌升级手牌等级，每次升级增加 chips 和 mult：
```
Level N 的 Pair = (10 + N*15) chips, (2 + N*1) mult
```

## Joker 系统

来源: https://balatrowiki.org/w/Jokers

### 数据规模

- **游戏总计**: 150 种 Joker（105 初始解锁 + 45 需要解锁条件）
- **当前实现**: 48 种（34 Common + 11 Uncommon + 3 Rare）
- **未实现**: 102 种（含全部 Legendary）

### 完整数据获取

结构化 Lua 表: https://balatrowiki.org/w/Module:Jokers/data

每条记录包含: `order`, `name`, `cost`, `rarity`, `effect`, `config`（效果参数）, `blueprint_compat`, `eternal_compat` 等。

### 稀有度分布

| 稀有度 | 数量 | 商店出现概率 |
|--------|------|-------------|
| Common | ~70 | 70% |
| Uncommon | ~45 | 25% |
| Rare | ~25 | 5% |
| Legendary | ~10 | 0.3% (仅通过 The Soul 幽灵牌) |

### Joker 类别

| 类别 | 说明 | 示例 |
|------|------|------|
| +Chips | 增加 chips | Scary Face (+30 per face card) |
| +Mult | 增加 mult | Jolly Joker (+8 mult if pair) |
| xMult | 乘法 mult | Joker Stencil (x1 per empty slot) |
| Economy | 赚钱 | Bull (+2$ per hand above $5) |
| Retrigger | 重新触发 | Dusk (retrigger last card) |
| Other | 特殊效果 | Marble (adds Stone card) |

## 盲注与 Ante

来源: https://balatrowiki.org/w/Blinds

### 盲注类型

每个 Ante 包含 3 个盲注：

| 盲注 | 所需分数倍数 | Boss 效果 |
|------|-------------|-----------|
| Small Blind | 1x | 无 |
| Big Blind | 1.5x | 无 |
| Boss Blind | 2x | 有（每次不同） |

### Ante 进度（所需分数）

| Ante | Small | Big | Boss |
|------|-------|-----|------|
| 1 | 300 | 450 | 600 |
| 2 | 800 | 1200 | 1600 |
| 3 | 2800 | 4200 | 5600 |
| 4 | 6000 | 9000 | 12000 |
| 5 | 11000 | 16500 | 22000 |
| 6 | 20000 | 30000 | 40000 |
| 7 | 35000 | 52500 | 70000 |
| 8 | 50000 | 75000 | 100000 |

### Boss Blind 效果

来源: https://balatrowiki.org/w/Blinds

**已实现 (10 种)**:

| Boss | 效果 |
|------|------|
| The Club | 所有梅花牌被禁用 |
| The Goad | 所有黑桃牌被禁用 |
| The Head | 所有红心牌被禁用 |
| The Plant | 所有方块牌被禁用 |
| The Wall | 所需分数翻倍 |
| The Wheel | 1/7 概率翻转手牌 |
| The Arm | 降级打出的牌型等级 |
| The Pillar | 之前打出过的牌被禁用 |

**未实现 (~20 种)**: 包括 The Eye (每手牌型不能重复)、The Mouth (只能打一种牌型)、The Fish (手牌面朝下)、The Serpent (弃牌后抽 3 张)、The Ox (打出某牌型扣钱) 等。完整列表见 https://balatrowiki.org/w/Blinds

## Enhancement 与 Edition

来源: https://balatrowiki.org/w/Card_Modifiers

### Enhancement (8 种) — 已全部实现

| Enhancement | 效果 |
|-------------|------|
| Bonus | +30 chips |
| Mult | +4 mult |
| Glass | x2 mult，有概率碎裂 |
| Steel | x1.5 mult（在手牌中时） |
| Stone | +50 chips，不计入牌型 |
| Gold | 回合结束 +$3 |
| Lucky | 1/5 概率 +20 mult，1/15 概率 +$20 |
| Wild | 可视为任意花色 |

### Edition (3 种) — 已全部实现

| Edition | 效果 |
|---------|------|
| Foil | +50 chips |
| Holographic | +10 mult |
| Polychrome | x1.5 mult |

## 商店系统

来源: https://balatrowiki.org/w/Shop

每通过一个盲注后进入商店：
- 2 个 Joker 槽 (可用 $5 刷新)
- 消耗品 (塔罗牌、行星牌)
- Joker 最多持有 5 个

---

## 尚未实现但数据可获取的游戏机制

以下机制在游戏中存在，当前项目未实现，但所有数据可从上述信息源获取：

| 机制 | 数量 | 数据来源 | 实现优先级 |
|------|------|---------|-----------|
| 完整 Joker | 150 (缺 102) | https://balatrowiki.org/w/Module:Jokers/data | 高 |
| 完整 Boss Blind | ~30 (缺 ~20) | https://balatrowiki.org/w/Blinds | 高 |
| 塔罗牌消耗品 | 22 种 | https://balatrowiki.org/w/Tarot_Cards | 中 |
| 行星牌消耗品 | 12 种 | https://balatrowiki.org/w/Planet_Cards | 中（当前仅简化处理） |
| 幽灵牌 | 18 种 | https://balatrowiki.org/w/Spectral_Cards | 低 |
| 优惠券系统 | 32 种 | https://balatrowiki.org/w/Vouchers | 低 |
| Seal 封印效果 | 4 种 | https://balatrowiki.org/w/Seals | 低 |
| 牌组变体 | 15 种 | https://balatrowiki.org/w/Decks | 低 |
| 难度等级 (Stakes) | 8 种 | https://balatrowiki.org/w/Stakes | 低 |

## 为何 Balatro 是有挑战性的 RL 问题

1. **多阶段决策**: 选牌、出牌、弃牌、购买，每种需要不同策略
2. **巨大组合空间**: C(52,5) = 2,598,960 种组合（简化为 86 维 + action masking）
3. **长期规划**: Joker 协同效应、牌组构筑、金钱管理
4. **高随机性**: 洗牌、Joker 出现、Boss 效果
5. **稀疏奖励**: 只有通过盲注和通关才有明确奖励
6. **不完全信息**: 不知道下一张抽到什么、商店出什么

## 项目内文档

`docs/wiki/` 下的 12 份规则文档（从上述在线源整理）：

| 文件 | 内容 | 对应在线源 |
|------|------|-----------|
| 01-game-overview.md | 核心循环 | https://balatrowiki.org/w/Balatro_Wiki |
| 02-poker-hands.md | 扑克牌型 | https://balatrowiki.org/w/Poker_Hands |
| 03-blinds-and-antes.md | 盲注与 Ante | https://balatrowiki.org/w/Blinds |
| 04-jokers.md | Joker 分类与效果 | https://balatrowiki.org/w/Jokers |
| 05-card-modifiers.md | Enhancement/Edition/Seal | https://balatrowiki.org/w/Card_Modifiers |
| 06-tarot-cards.md | 塔罗牌 | https://balatrowiki.org/w/Tarot_Cards |
| 07-planet-cards.md | 行星牌 | https://balatrowiki.org/w/Planet_Cards |
| 08-spectral-cards.md | 幽灵牌 | https://balatrowiki.org/w/Spectral_Cards |
| 09-vouchers.md | 优惠券 | https://balatrowiki.org/w/Vouchers |
| 10-decks.md | 牌组变体 | https://balatrowiki.org/w/Decks |
| 11-stakes.md | 难度等级 | https://balatrowiki.org/w/Stakes |
| 12-scoring-formula.md | 计分公式 | https://balatrowiki.org/w/Scoring |
