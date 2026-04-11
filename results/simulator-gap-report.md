# 模拟环境缺口报告

## 1. 当前结论

- 当前 native 环境已经不是纯 mock。
- 当前已经能跑通：
  - `BLIND_SELECT -> SELECTING_HAND -> ROUND_EVAL -> SHOP -> BLIND_SELECT`
  - Small/Big Blind 清关后进入 Shop
  - Shop 内购买、reroll、next_round 的基础路径
- 但当前 **还不能宣称与原版 Balatro 一致**。
- 当前最新 replay 审计结果是：
  - `hard_invariants_ok = true`
  - `fidelity_ready = false`

直接证据：

- `results/replay-proof.json`
- `results/replay-proof.cli.txt`
- `results/replay-proof.audit.json`

## 2. 还差什么

### 2.1 状态机可观测性还不完整

当前 snapshot 里已经能看到这些 Lua 对齐状态：

- `BLIND_SELECT`
- `SELECTING_HAND`
- `ROUND_EVAL`
- `SHOP`
- `GAME_OVER`

但还缺这几个 Lua 中间态的可观测 snapshot：

- `NEW_ROUND`
- `DRAW_TO_HAND`
- `HAND_PLAYED`

这意味着：

- 现在能证明主路径大框架没完全错
- 但还不能逐步对齐原版的每个中间推进点

### 2.2 Joker 模块远远不够

当前 Rust engine 的 Joker 逻辑还是非常有限，核心入口在：

- `crates/balatro-engine/src/lib.rs`
- `apply_joker_effect(...)`

目前只覆盖了少量模式：

- `Mult`
- `Suit Mult`
- `Discard Chips`
- 少数按名字写死的 Joker，例如：
  - `Abstract Joker`
  - `Scary Face`

还没达到原版要求的部分：

- 大部分 Joker 的完整触发语义
- `before / individual / repetition / after / end_of_round` 的完整触发层次
- Joker on Joker
- retrigger 顺序
- edition / seal / enhancement 与 Joker 的耦合

### 2.3 Boss Blind / Blind side effect 还不完整

从 Lua 静态分析看，原版 blind 逻辑不只是“目标分数不同”。

还缺的能力包括：

- `press_play`
- `debuff_hand`
- `modify_hand`
- `drawn_to_hand`
- boss disable / triggered / debuff 文本与状态
- Boss 特殊规则的逐条实现

目前环境只实现了非常简化的 blind 路径，不足以证明 boss fidelity。

### 2.4 Shop 生命周期还只是第一层

现在已经修到了：

- 清掉 Small/Big Blind 后进入 Shop
- Shop 中基础买、卖、reroll、next_round

但和原版还有明显缺口：

- vouchers
- boosters / packs
- tag 对商店生成和最终商店状态的影响
- free reroll / inflation / reroll cost 的完整规则
- shop enter / shop end 的 Joker 钩子

### 2.5 Consumables / Booster / Voucher / Tag 基本未完成

这部分在 Lua 里分布在：

- `functions/button_callbacks.lua`
- `functions/state_events.lua`
- `card.lua`
- `functions/common_events.lua`

当前 native engine 还没有做到原版级别的：

- consumable 使用链
- booster pack 中断与恢复
- voucher 获取与生效
- tag 的即时效果 / shop_start / new_blind_choice / eval

### 2.6 RNG 调用顺序还没有被真实客户端证明

现在 native engine 是 deterministic 的，但 deterministic 不等于 vanilla-consistent。

还缺：

- 与真实客户端同 seed、同动作序列下的 RNG 对齐
- shuffle / boss 选择 / shop 生成 / effect roll 的调用顺序证明

### 2.7 真实客户端 recorder 还没接通

这是当前阶段最关键的剩余工作。

还没完成：

- `Steamodded + Lovely + BalatroBot` 真实客户端接通
- 真实 trajectory recorder
- `save_run()` 风格 snapshot 抓取
- 关键事件链抓取
- replay diff

没有这一层，native engine 还只能说“按 Lua 静态分析在靠近原版”，不能说“已被真实客户端验真”。

## 3. 还差哪些模块

按工程拆分，当前还差这些模块：

### A. 状态机模块

- Lua 中间态可观测化：
  - `NEW_ROUND`
  - `DRAW_TO_HAND`
  - `HAND_PLAYED`
- 更细粒度的 state transition 日志

### B. Blind / Boss 模块

- Boss debuff 规则
- blind hook 对应：
  - `press_play`
  - `modify_hand`
  - `debuff_hand`
  - `drawn_to_hand`
- boss disable / trigger / boss-specific side effects

### C. Joker 执行模块

- table-driven Joker execution
- trigger stage 分类
- retrigger / repetition
- Joker-on-card / Joker-on-Joker / end_of_round

### D. Shop 模块

- voucher
- booster
- shop tag hooks
- reroll cost fidelity
- shop start / shop end hooks

### E. Consumable 模块

- tarot / planet / spectral / booster use
- interrupt / pack-return flow

### F. 真实轨迹模块

- real-client snapshot recorder
- real-client event-chain recorder
- replay diff / mismatch reporter

## 4. 当前已有检查

### 环境与构建检查

- 本机 Steam `Balatro.love` 存在
- 本地镜像 hash 已校验
- `.venv` + `balatro_native` 构建通过
- `cargo test` 通过
- `pytest` 通过
- `scripts/doctor.py` 通过

### 规则与路径检查

- blind linear path
- Boss 初始不可直选
- shop inventory outside shop hidden
- Small/Big Blind 清关后进入 Shop
- Boss 清关后才升 ante

### replay 检查

- 中文 CLI replay 可读回放
- replay audit 可检查：
  - stage -> lua_state 映射
  - `cashout` 后进 `Shop`
  - `Shop -> next_round` 的 ante 行为
  - 是否还缺 Lua 中间态

## 5. 还缺的检查

这些检查现在还没有闭环：

- 真实客户端同 seed + 同动作序列 replay compare
- 中间 snapshot 字段逐项比对
- Joker 触发顺序比对
- retrigger 比对
- RNG 调用顺序比对
- boss blind 特殊 case 覆盖
- shop / voucher / booster / tag 全路径覆盖
- consumable 使用全路径覆盖

## 6. 我在哪里定位到了你的游戏

我定位到的本机原始游戏包路径是：

- `/Users/Aincrad/Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love`

我在仓库里建立的本地镜像路径是：

- 原包镜像：
  - `/Users/Aincrad/dev-proj/hack-balatro/vendor/balatro/steam-local/original/Balatro.love`
- 提取后的 Lua 源码：
  - `/Users/Aincrad/dev-proj/hack-balatro/vendor/balatro/steam-local/extracted/`
- manifest：
  - `/Users/Aincrad/dev-proj/hack-balatro/vendor/balatro/steam-local/manifest.json`

当前镜像 hash：

- `48c7a0791796a969d2cd0891ebdc9922b2988eb5aaad8ad7a72775a02772e24e`

## 7. 你现在应该先看哪几个文件

- `results/replay-proof.cli.txt`
- `results/replay-proof.audit.json`
- `results/game-location.json`
- `docs/balatro-source-entrypoints.md`
- `Agent-Style.md`
