# 模拟环境设计

## 架构概览

```
┌─────────────────────────────────────────────────────┐
│                   Python 层                          │
│                                                     │
│  ┌─────────────┐    ┌──────────────┐                │
│  │ BalatroEnv  │    │ StateEncoder │                │
│  │ (Gymnasium) │◄──►│ (454d obs)   │                │
│  └──────┬──────┘    └──────────────┘                │
│         │                                           │
│         │ PyO3 FFI                                   │
│─────────┼───────────────────────────────────────────│
│         ▼                                           │
│  ┌──────────────┐                                   │
│  │   pylatro    │   Rust 层                          │
│  │  (GameEngine)│                                   │
│  └──────┬───────┘                                   │
│         │                                           │
│  ┌──────▼───────┐                                   │
│  │   core/      │                                   │
│  │  game.rs     │  游戏主循环                        │
│  │  action.rs   │  动作空间 (86 维)                  │
│  │  card.rs     │  扑克牌                            │
│  │  joker.rs    │  48 种 Joker 效果                  │
│  │  stage.rs    │  阶段 + Boss 效果                  │
│  │  shop.rs     │  商店逻辑                          │
│  │  hand.rs     │  牌型判断                          │
│  │  deck.rs     │  牌组管理                          │
│  └──────────────┘                                   │
└─────────────────────────────────────────────────────┘
```

## pylatro Python API

pylatro 是通过 PyO3 从 Rust 编译出的 Python 模块。

### GameEngine

```python
from pylatro import GameEngine

engine = GameEngine()

# 核心方法
engine.gen_action_space()       # -> list[bool], 长度 86, True=合法
engine.handle_action_index(idx) # 执行动作 idx
engine.state                    # -> GameState (当前状态只读快照)
engine.is_over                  # -> bool
engine.is_win                   # -> bool
```

### GameState

```python
state = engine.state

# 游戏状态字段
state.stage            # 当前阶段对象 (Stage_PreBlind, Stage_Blind, ...)
state.round            # 当前回合数
state.score            # 当前得分
state.required_score   # 本盲注所需分数
state.plays            # 剩余出牌次数
state.discards         # 剩余弃牌次数
state.money            # 当前金钱
state.ante             # 当前 Ante (难度等级)
state.chips            # 当前 chips 基数
state.mult             # 当前 mult 乘数

# 牌面信息
state.deck             # list[Card] — 牌组中剩余的牌
state.available        # list[Card] — 当前手牌 (可选择的牌)
state.selected         # list[Card] — 已选中的牌
state.discarded        # list[Card] — 本轮已弃的牌

# Joker 信息
state.jokers           # list[Joker] — 持有的 Joker
state.shop_jokers      # list[Joker] — 商店中的 Joker (本项目新增)

# Boss 与奖励
state.boss_effect      # str — 当前 Boss 效果 (本项目新增)
state.reward           # int — 本轮奖励金钱 (本项目新增)
```

### Card

```python
card = state.available[0]

card.rank_index     # int (0-12: 2,3,...,K,A)  — 本项目新增
card.suit_index     # int (0-3: spade,heart,diamond,club) — 本项目新增
card.chip_value     # int (牌面点数贡献) — 本项目新增
card.card_id        # 唯一标识
```

### Joker

```python
joker = state.jokers[0]

joker.joker_name    # str — Joker 名称 — 本项目新增
joker.joker_cost    # int — 购买价格 — 本项目新增
joker.joker_rarity  # str — 稀有度 (Common/Uncommon/Rare) — 本项目新增
```

## 对原始 balatro-rs 的扩展

### 规则补全

| 功能 | 数量 | 说明 |
|------|------|------|
| Joker 实现 | 48 | 34 Common + 11 Uncommon + 3 Rare |
| Enhancement 计分 | 8 | Bonus, Mult, Glass, Steel, Stone, Gold, Lucky, Wild |
| Edition 计分 | 3 | Foil (+50 chips), Holo (+10 mult), Poly (x1.5 mult) |
| Boss Blind 效果 | 10 | TheClub, TheGoad, TheHead, ThePlant, TheWall, TheWheel, TheArm, ThePillar 等 |
| 手牌升级 | 12 | 每种牌型的 chips/mult 增量不同 |
| 商店稀有度加权 | 3 | Common 70% / Uncommon 25% / Rare 5% |
| 行星牌 | 简化 | cashout 时 30% 概率升级手牌等级 |

### 动作空间扩展 (79 → 86)

原始 balatro-rs 提供 79 维动作空间，本项目新增 7 个动作：

| 索引 | 动作 | 新增 |
|------|------|------|
| 79 | reroll_shop | 是 |
| 80-84 | sell_joker (5 槽) | 是 |
| 85 | skip_blind | 是 |

## BalatroEnv — Gymnasium 封装

文件：`env/balatro_gym_wrapper.py`

### 接口

```python
env = BalatroEnv(config=config)
obs, info = env.reset(seed=42)

# 每步循环
action_mask = env.get_action_mask()  # np.ndarray[bool], shape=(86,)
obs, reward, terminated, truncated, info = env.step(action)
```

### 观测空间

- **类型**: `Box(low=-inf, high=inf, shape=(454,), dtype=float32)`
- **编码**: 由 `StateEncoder.encode_pylatro_state()` 生成
- **详见**: [03-observation-space.md](03-observation-space.md)

### 动作空间

- **类型**: `Discrete(86)`
- **掩码**: `get_action_mask()` 返回 86 维 bool 数组
- **详见**: [04-action-space.md](04-action-space.md)

### 容错机制

`step()` 方法实现了多重容错：

1. **无效动作回退**: 如果传入的 action 在 mask 中为 False，自动选择第一个合法动作
2. **引擎异常重试**: 最多重试 4 次，每次尝试不同的合法动作
3. **全部失败终止**: 如果所有动作都失败，返回 `terminated=True, reward=-1.0`
4. **安全步数上限**: `_step_count >= 2000` 时强制 `truncated=True`

### info 字典

```python
info = {
    "stage": "Stage_Blind",        # 当前游戏阶段
    "score": 150,                   # 当前得分
    "required_score": 300,          # 所需分数
    "plays": 3,                     # 剩余出牌次数
    "discards": 2,                  # 剩余弃牌次数
    "money": 12,                    # 金钱
    "round": 3,                     # 回合数
    "num_available": 8,             # 手牌数
    "num_selected": 3,              # 已选牌数
    "num_jokers": 2,                # 持有 Joker 数
    "num_deck": 35,                 # 牌组剩余
    "step_count": 42,               # 当前步数
    "blinds_passed": 5,             # 已通过盲注数
    "game_won": False,              # 是否通关
    "is_over": False,               # 游戏是否结束
}
```

## 并行环境

```python
from env.balatro_gym_wrapper import make_vec_env

# 创建 64 个并行环境
envs = make_vec_env(config=config, num_envs=64, seed=0)
```

使用 `gymnasium.vector.AsyncVectorEnv` 实现异步并行。

注：训练中的并行环境 (`ParallelBalatroEnvs`) 使用同步方式（单进程 for 循环），这是当前的性能瓶颈（占训练时间 ~45%）。

## 性能基准

测试环境：Apple M4, 10 核, 16 GB RAM

| 指标 | 数值 |
|------|------|
| 单环境 step (Rust pylatro) | 6,698 steps/s |
| GreedyAgent 完整游戏 (含 obs 编码) | 72 games/s |
| 轨迹收集 (10 workers) | 526 games/s |
| 端到端 PPO 训练 (454d, 934K, O(n) 编码) | 15,700 sps |

### 瓶颈分解

PPO update 时间分解（64 envs x 256 步）:

| 组件 | 耗时占比 | 说明 |
|------|---------|------|
| Env step (含 obs 编码) | ~45% | 64 个 env 串行 step |
| Model inference (act_batch) | ~20% | CPU batch=64 推理 |
| PPO gradient update | ~30% | 4 epochs x 32 mini-batches |
| Buffer / GAE / overhead | ~5% | numpy 操作 |

## 游戏阶段状态机

```
PreBlind → Blind (出牌/弃牌) → PostBlind → Shop → PreBlind (下一盲注)
                                                        │
                                                   CashOut (Ante 通过)
                                                        │
                                                   End (通关或失败)
```

7 种阶段类型：
1. `Stage_PreBlind` — 盲注选择
2. `Stage_Blind` — 出牌/弃牌阶段
3. `Stage_PostBlind` — 盲注通过后
4. `Stage_Shop` — 商店阶段
5. `Stage_CashOut` — Ante 结算
6. `Stage_End` — 游戏结束
7. `Stage_Other` — 其他
