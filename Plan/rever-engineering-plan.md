# Balatro 逆向分析与 RL 环境计划

## 目标
- 基于本机正版 Balatro 安装，建立一份可复用、可验证的源码快照与运行时骨架分析。
- 明确原版客户端中最适合做高保真 RL 封装的状态、动作、序列化与阶段切换入口。

## Outcome
- 当前已确认唯一可提取源码入口是 Steam 版 `Balatro.love`，不是 `~/Applications/Balatro.app` 的 launcher stub。
- 当前已完成本地复制：
  - 原包：`vendor/balatro/steam-local/original/Balatro.love`
  - 抽取源码：`vendor/balatro/steam-local/extracted`
- 当前已确认源码快照：
  - 原始包 SHA-256：`48c7a0791796a969d2cd0891ebdc9922b2988eb5aaad8ad7a72775a02772e24e`
  - 原包体积约 `53M`
  - 抽取后的 Lua/本地化源码约 `3.8M`
  - 已抽取 `47` 个文件，核心 Lua 总行数约 `33,773`
- 非目标：
  - 不再从 Python/Rust 重新手写一套“近似 Balatro”规则
  - 不在 Steam 安装目录内直接改文件

## 当前发现
- 实际源码启动链：
  - `main.lua` 先 `require` 引擎、函数库、`game.lua`、`globals.lua`、`card.lua`、`blind.lua`
  - `love.load()` 调 `G:start_up()`
  - `Game:start_up()` 初始化窗口、音频、存档线程、HTTP 线程、controller、原型表、事件管理器，最后进入 `self:splash_screen()`
  - 运行开始入口是 `G.FUNCS.start_run()` -> `G:start_run(args)`
- 全局状态不是分散对象，而是单一 `G = Game()`：
  - `G.STATES` / `G.STAGE` / `G.STATE` 定义顶层状态机
  - `G.FUNCS` 是 UI 与动作回调命名空间
  - `G.I` / `G.MOVEABLES` / `G.ANIMATIONS` 管所有运行时实例
  - `G.GAME` 是 run 级持久状态
  - `G.E_MANAGER` 是事件队列与延迟逻辑核心
  - `G.CONTROLLER` 是输入抽象层
- run 主循环是 `Game:update()`，它按 `G.STATE` 分发到：
  - `update_selecting_hand`
  - `update_shop`
  - `update_hand_played`
  - `update_draw_to_hand`
  - `update_new_round`
  - `update_blind_select`
  - `update_round_eval`
  - 各类 pack 状态
- blind/shop 核心阶段已经明确：
  - `select_blind` 设置 `G.GAME.round_resets.blind` 后调用 `new_round()`
  - `new_round()` 重置回合资源、设置 blind、洗牌，然后切到 `DRAW_TO_HAND`
  - `DRAW_TO_HAND` 发牌后切到 `SELECTING_HAND`
  - `play_cards_from_highlighted` 把状态切到 `HAND_PLAYED`，随后进入 `evaluate_play`
  - `HAND_PLAYED` 结束后按胜负切到 `NEW_ROUND` 或 `DRAW_TO_HAND`
  - `end_round()` / `ROUND_EVAL` / `cash_out()` 后切到 `SHOP`
  - `toggle_shop()` 会从 shop 回到 `BLIND_SELECT`
- 最适合做 observation 序列化的原生入口不是 UI，而是 `save_run()`：
  - 它会把 `cardAreas`、`tags`、`GAME`、`STATE`、`BLIND`、`BACK`、`VERSION` 统一裁剪后写入 `G.ARGS.save_run`
  - 这意味着可以直接复用其结构做 RPC snapshot，而不是自己重新遍历所有 UI 节点

## Implementation
1. 源码与快照治理
   - 固定使用本机 Steam 源：`~/Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love`
   - 保留 `vendor/balatro/steam-local/original/Balatro.love` 作为只读基线
   - 后续新增 `manifest.json` 或等价结构，记录路径、hash、抽取时间、版本号

2. 运行时骨架映射
   - 以 `main.lua` / `game.lua` / `globals.lua` 建立启动与状态机图
   - 以 `functions/state_events.lua` 建立 round 内动作与计分链
   - 以 `functions/button_callbacks.lua` 建立 UI 动作到内部调用的映射
   - 以 `save_run()` 为状态快照基线，补充原结构未覆盖的临时字段

3. RL 封装优先动作面
   - 开局/重置：`G.FUNCS.start_run`
   - 选盲注：`G.FUNCS.select_blind`
   - 跳过盲注：`G.FUNCS.skip_blind`
   - 出牌：`G.FUNCS.play_cards_from_highlighted`
   - 弃牌：`G.FUNCS.discard_cards_from_highlighted`
   - 购物：`G.FUNCS.buy_from_shop`
   - 刷新商店：`G.FUNCS.reroll_shop`
   - 使用商品/消耗牌：`G.FUNCS.use_card`
   - 结算进入商店：`G.FUNCS.cash_out`
   - 结束商店进入下一盲注：`G.FUNCS.toggle_shop`

4. RL 封装优先观测面
   - 顶层：`G.STATE`, `G.STAGE`, `G.STATE_COMPLETE`
   - run：`G.GAME`, `G.GAME.current_round`, `G.GAME.round_resets`, `G.GAME.shop`
   - 区域：`G.hand`, `G.play`, `G.deck`, `G.discard`, `G.jokers`, `G.consumeables`
   - shop：`G.shop_jokers`, `G.shop_vouchers`, `G.shop_booster`
   - blind：`G.GAME.blind`, `G.GAME.blind_on_deck`, `G.GAME.round_resets.blind_states`

5. 训练环境推荐路线
   - 第一步做只读 snapshot mod，先证明 `save_run()` 派生状态足够支持离线监督与 replay
   - 第二步再做动作 RPC 层，直接调用 `G.FUNCS.*` 或等价底层函数
   - 第三步增加 step barrier，等待状态稳定在 `SELECTING_HAND`、`BLIND_SELECT`、`SHOP`、pack 状态或 `GAME_OVER`

6. 验证
   - 验证 reset 后状态一定落在 `BLIND_SELECT`
   - 验证 blind 流程只能 `Small -> Big -> Boss`
   - 验证 `skip_blind` 只推进当前可跳过 blind
   - 验证 `reroll_shop` 使用原生 `calculate_reroll_cost()`
   - 验证 snapshot 与原生 `save_run()` 结构一致

## User Decisions
- 决策：后续是否只做只读分析，还是直接进入 injected RPC mod。
  - 为什么重要：这决定我们先做静态抽取，还是直接做可执行环境。
  - Recommended default：先做 injected RPC mod，因为高保真 RL 最终必须以原版状态机为准。
- 决策：是否把完整抽取资源继续扩展到纹理/音频，而不止 Lua 与 localization。
  - 为什么重要：完整 UI 对齐、视觉 replay、卡图编码可能需要贴图资源。
  - Recommended default：先保持 Lua + localization，纹理按需提取。
- 决策：是否把 `save_run()` 作为官方 snapshot schema 基线。
  - 为什么重要：这会直接决定 observation schema 的兼容性与实现成本。
  - Recommended default：是，优先复用 `save_run()`，只补临时 UI/事件字段。

## Risks and Assumptions
- 假设：当前本机 Steam 包就是仓库目标版本 `1.0.1o-FULL`
- 假设：`~/Applications/Balatro.app` 不参与逆向，因为它只是 `open steam://run/2379780` 的启动器
- 风险：Balatro 逻辑大量依赖 `G.E_MANAGER` 的延迟事件，简单同步调用容易拿到“半过渡态”
- 风险：很多动作依赖高亮、焦点和 card area 上下文，不适合只传一个离散 action id
- 风险：直接改 Steam 安装目录会破坏升级路径；后续应走 mod 注入或外置 hook

## Execution Gate
- 当前回合已完成本地定位、复制与骨架分析。
- 下一阶段是“真实客户端 + snapshot/RPC 注入”的实现，不应在未确认设计前直接改原游戏包。
- 等你确认后，再继续把这个计划落成具体的 RPC mod、Gym wrapper 和 step barrier。
