# 仓库、依赖与信息源

## 核心仓库（实际使用）

### 1. evanofslack/balatro-rs — Rust 游戏引擎（主要后端）

- **仓库**: https://github.com/evanofslack/balatro-rs
- **语言**: Rust
- **使用方式**: fork 到 `vendor/balatro-rs/`，通过 PyO3 暴露 Python 绑定 (`pylatro`)
- **本项目的修改**:
  - 为 `Card` 添加 getter：`rank_index`, `suit_index`, `chip_value`
  - 为 `Joker` 添加 getter：`joker_name`, `joker_cost`, `joker_rarity`, `joker_categories`
  - 暴露 `GameState` 的 `shop_jokers`, `boss_effect`, `ante`, `reward` 字段
  - 将动作空间从 79 扩展到 86（新增 RerollShop, SellJoker x5, SkipBlind）
  - 补全 48 个 Joker 实现（原始仓库只有部分）
  - 实现 8 种 Enhancement、3 种 Edition、10 种 Boss Blind、手牌升级系统
  - Joker 稀有度加权商店（70% Common / 25% Uncommon / 5% Rare）

### 2. cassiusfive/balatro-gym — Gymnasium 环境参考

- **仓库**: https://github.com/cassiusfive/balatro-gym
- **语言**: Python
- **角色**: 参考了 Gymnasium 接口设计（obs/action/reward 结构），但最终因功能不足未直接使用

## 参考仓库（设计启发）

### 3. coder/balatrobot — JSON-RPC 接口

- **仓库**: https://github.com/coder/balatrobot
- **语言**: Go
- **价值**: 展示如何通过 JSON-RPC 连接真实 Balatro 游戏客户端，可用于未来在真实游戏中验证 AI

### 4. vivasvan1/balatro-dqn-agent — DQN 训练参考

- **仓库**: https://github.com/vivasvan1/balatro-dqn-agent
- **语言**: Python
- **价值**: DQN 方法参考，启发了 observation 编码和 reward shaping 思路

### 5. xwkya/RLatro — 确定性模拟器

- **仓库**: https://github.com/xwkya/RLatro
- **语言**: Python
- **价值**: 提供确定性 headless Balatro 模拟器设计参考

### 6. pjpuzzler/python-balatro — 纯 Python 模拟器

- **仓库**: https://github.com/pjpuzzler/python-balatro
- **语言**: Python (111 commits)
- **价值**: 可作为备选后端或交叉验证引擎逻辑。纯 Python 实现，无需 Rust 编译

### 7. proj-airi/game-playing-ai-balatro — Vision+LLM 方法

- **仓库**: https://github.com/proj-airi/game-playing-ai-balatro
- **语言**: Jupyter Notebook / Python
- **价值**: 完全不同的技术路线（YOLO + PaddleOCR + LLM），可对比 RL 方法的优劣

## 游戏数据与 Modding 仓库

### 8. VibezFire/Balatro-Game.lua-files — 游戏 Lua 源码

- **仓库**: https://github.com/vibezfire/balatro-game.lua-files
- **价值**: 包含 Balatro 游戏的 Lua 源码提取，是所有 Joker 效果、Boss 效果、计分公式的**权威定义**
- **获取方式**: 也可从游戏安装目录提取
  - macOS: 右键 Balatro.app → Show Package Contents → `Contents/Resources/Game/game.lua`
  - Windows: `Steam/steamapps/common/Balatro/`
  - 解包工具: [7zip](https://www.7-zip.org/) 解压 `.exe` 或 `.love` 文件

### 9. jie65535/awesome-balatro — Mod 与工具策展

- **仓库**: https://github.com/jie65535/awesome-balatro
- **价值**: Balatro Mod 和工具的策展列表，了解社区生态

## 同类游戏 RL 参考仓库

### 10. KrystianRusin/Slay-The-Spire-RL — Maskable PPO

- **仓库**: https://github.com/krystianrusin/slay-the-spire-rl
- **语言**: Python
- **价值**: 在 Slay the Spire（同类 roguelike 卡牌游戏）上实现 Maskable PPO，支持多并行环境，是最直接的同类参考

### 11. xaved88/bottled_ai — Slay the Spire 自动化

- **仓库**: https://github.com/xaved88/bottled_ai
- **语言**: Python
- **价值**: Slay the Spire 自动化 AI，参考其游戏抽象和决策框架设计

---

## 结构化数据源

### Balatro Wiki (社区维护，推荐首选)

| 页面 | URL | 内容 |
|------|-----|------|
| 首页 | https://balatrowiki.org/ | 社区维护的 Balatro Wiki |
| Joker 完整数据 (Lua 表) | https://balatrowiki.org/w/Module:Jokers/data | 全 150 Joker 的结构化数据：name, cost, rarity, effect, config |
| Joker 列表页 | https://balatrowiki.org/w/Jokers | 全 150 Joker 的效果描述、解锁条件 |
| Joker 模板数据 | https://balatrowiki.org/w/Template:Joker_data | Joker 数据模板定义 |
| Blinds 完整列表 | https://balatrowiki.org/w/Blinds | 全部 ~30 种 Boss Blind 效果 |
| 计分公式 | https://balatrowiki.org/w/Scoring | 精确计分公式 |
| Tarot Cards | https://balatrowiki.org/w/Tarot_Cards | 塔罗牌效果 |
| Planet Cards | https://balatrowiki.org/w/Planet_Cards | 行星牌效果 |
| Spectral Cards | https://balatrowiki.org/w/Spectral_Cards | 幽灵牌效果 |
| Vouchers | https://balatrowiki.org/w/Vouchers | 优惠券系统 |
| Decks | https://balatrowiki.org/w/Decks | 初始牌组变体 |
| Stakes | https://balatrowiki.org/w/Stakes | 难度等级 |
| Poker Hands | https://balatrowiki.org/w/Poker_Hands | 扑克牌型定义 |

### 其他数据源

| 来源 | URL | 说明 |
|------|-----|------|
| Balatro Wiki (Fandom, 旧版) | https://balatrogame.fandom.com/ | 内容较旧但仍有参考价值 |
| 第三方 Joker 表格 | https://balatro.wiki/Jokers | 全 150 Joker 的可排序表格 |
| Fandom CDN 素材 | `static.wikia.nocookie.net/balatrogame/images` | 卡牌/Joker/Blind 图片素材 |
| Modded Wiki Vanilla 数据 | https://balatromods.miraheze.org/wiki/Modded_Balatro_Wiki:Data/Vanilla_Jokers | 模组 Wiki 中的原版 Joker 数据 |

### Joker 数据结构（来自 Module:Jokers/data）

每个 Joker 在 Lua 表中的结构：

```lua
j_joker = {
    order = 1,
    name = "Joker",
    cost = 2,
    rarity = 1,                    -- 1=Common, 2=Uncommon, 3=Rare, 4=Legendary
    set = "Joker",
    effect = "Mult",
    config = {mult = 4},           -- 效果参数
    pos = {x = 0, y = 0},
    blueprint_compat = true,
    eternal_compat = true,
    perishable_compat = true,
    unlocked = true,
    discovered = true,
}
```

---

## 社区资源

| 来源 | URL | 价值 |
|------|-----|------|
| Reddit r/balatro | https://www.reddit.com/r/balatro/ | 策略讨论、通关率统计、构筑理论 |
| Steam 社区指南 | https://steamcommunity.com/app/2379780/guides/ | 高分策略、Joker 组合推荐 |
| Balatro 官方网站 | https://www.playbalatro.com/ | 游戏介绍、更新日志 |
| Steam 商店页 | https://store.steampowered.com/app/2379780/Balatro/ | 玩家评价、统计 |

---

## Python 依赖

来源：`pyproject.toml`，Python >= 3.12

### 运行时依赖

| 包 | 版本要求 | 用途 |
|----|----------|------|
| gymnasium | >=0.29 | Gymnasium 环境接口 |
| torch | >=2.0 | PyTorch 深度学习框架 |
| numpy | >=1.26 | 数值计算 |
| pyyaml | >=6.0 | YAML 配置文件解析 |
| wandb | >=0.16 | 实验追踪 (Weights & Biases) |
| tensorboard | >=2.15 | 训练日志可视化 |
| tqdm | >=4.66 | 进度条 |
| psutil | >=5.9 | 系统资源监控 |

### 开发依赖

| 包 | 版本要求 | 用途 |
|----|----------|------|
| pytest | >=7.0 | 单元测试 |
| ruff | >=0.1 | Python linter |

## Rust 工具链

编译 `vendor/balatro-rs/pylatro` 需要：

| 工具 | 用途 | 文档 |
|------|------|------|
| Rust (rustc + cargo) | 编译器 | https://www.rust-lang.org/tools/install |
| PyO3 | Rust-Python 互操作 | https://pyo3.rs/ |
| maturin | 构建 PyO3 Python 包 | https://www.maturin.rs/ |

```bash
cd vendor/balatro-rs/pylatro
pip install maturin
maturin develop --release
```

## vendor/balatro-rs 结构

```
vendor/balatro-rs/
├── Cargo.toml              # Rust workspace 配置
├── core/                   # 游戏核心逻辑
│   └── src/
│       ├── game.rs         # 主游戏循环
│       ├── action.rs       # 动作空间定义
│       ├── card.rs         # 扑克牌结构
│       ├── hand.rs         # 牌型判断
│       ├── deck.rs         # 牌组管理
│       ├── joker.rs        # Joker 效果实现
│       ├── shop.rs         # 商店逻辑
│       ├── stage.rs        # 游戏阶段与 Boss 效果
│       ├── effect.rs       # Enhancement/Edition 效果
│       └── ante.rs         # Ante 进度管理
├── pylatro/                # Python 绑定
│   ├── src/lib.rs          # PyO3 接口定义
│   ├── gym/env.py          # Python 环境封装
│   └── examples/           # 使用示例
└── cli/                    # 命令行交互工具
```
