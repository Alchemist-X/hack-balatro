# Balatro Trajectory 验真计划

## 目标
- 建立一条以真实 Balatro 客户端为金标准的 trajectory 采集与复现流程。
- 用 trajectory 复现一致性来判断模拟环境是否足够贴合原版，为后续 Harness 环境验真提供通过标准。

## Outcome
- 先拿到零漂移的真实轨迹，而不是先手写近似 simulator。
- 用本地 AI 或策略程序对同一状态序列做复现，反向验证模拟环境和动作语义是否正确。
- 最终 acceptance 不是“看起来差不多”，而是 `100%` 复现真实轨迹才算通过。

## Implementation
1. 真实环境优先
- 先解包本机合法持有的 `Balatro.love` 做静态分析，锁定源码入口、状态机、计分链和动作入口。
- 再使用 `Steamodded + Lovely + BalatroBot` 跑真实游戏环境。
- 真实客户端负责提供：
  - 真实状态读取
  - 真实动作执行
  - 真实随机序列与触发顺序

2. 真人录制真实 trajectory
- 第一阶段先不让 AI 自己探索，先由人类在真实客户端中操作。
- 这一阶段的目标不是“自动打游戏”，而是先把真实客户端里的状态和动作可靠地落盘。

### 2.1 当前机器上的真实客户端位置
- 截至 `2026-03-27`，本机 Balatro 安装已经定位到：
  - 游戏目录：`/Users/Aincrad/Library/Application Support/Steam/steamapps/common/Balatro`
  - App Bundle：`/Users/Aincrad/Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app`
  - LOVE 可执行：`/Users/Aincrad/Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/MacOS/love`
  - 原始包：`/Users/Aincrad/Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love`
  - 存档目录：`/Users/Aincrad/Library/Application Support/Balatro`
- 当前真实客户端录制前置条件用脚本探测，结果统一写到：
  - `results/real-client-bootstrap.json`
- 探测命令：
```bash
python3 scripts/probe_real_client_env.py --output results/real-client-bootstrap.json
```

### 2.2 当前机器还差什么
- 当前探测到的缺口不是 Balatro 本体，而是录制链路：
  - `~/Library/Application Support/Balatro/Mods` 还不存在
  - 游戏目录里还没有 `liblovely.dylib`
  - 游戏目录里还没有 `run_lovely_macos.sh`
  - 本机 PATH 里还没有 `uv` / `uvx`
- 这意味着：Balatro 已经在本机，但“真实 trajectory recorder”还没有接到真实客户端上。

### 2.3 macOS 上正确打开带录制能力的 Balatro
- 真实录制链路必须是：`Lovely Injector + Steamodded + BalatroBot + Balatro`
- 官方参考：
  - BalatroBot 安装与 CLI：`https://coder.github.io/balatrobot/`
  - BalatroBot 全文档：`https://coder.github.io/balatrobot/llms-full.txt`
  - Lovely 手工安装：`https://github.com/ethangreen-dev/lovely-injector#manual-installation`
  - Steamodded Wiki：`https://github.com/Steamodded/smods/wiki`
- 安装步骤应按官方链路执行：
  - Lovely 的 macOS 手工安装：把 `liblovely.dylib` 和 `run_lovely_macos.sh` 放进 Balatro 游戏目录
  - Steamodded / BalatroBot：把 `smods/`、`DebugPlus/`、`balatrobot/` 放进 `~/Library/Application Support/Balatro/Mods/`
  - 启动：`uvx balatrobot serve --fast`
- macOS 重点约束：
  - 不要从 Steam 里直接启动做录制。官方说明里明确提到 macOS 有 Steam 启动 bug，应该走 `uvx balatrobot serve` 或 `run_lovely_macos.sh`
- 当前仓库里给了两个本地工具：
  - 环境探针：`scripts/probe_real_client_env.py`
  - 人工录制器：`scripts/record_manual_real_trajectory.py`

### 2.4 启动成功后的验收方式
- 先启动真实客户端服务：
```bash
uvx balatrobot serve --fast
```
- 再做健康检查，只有 `status=ok` 才算真正连上：
```bash
curl -X POST http://127.0.0.1:12346 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"health","id":1}'
```
- 然后启动人工录制器：
```bash
python3 scripts/record_manual_real_trajectory.py \
  --session-dir results/real-client-trajectories/manual-001 \
  --deck RED \
  --stake WHITE \
  --seed 123456
```
- 录制器会做三件事：
  - 先连 `health`
  - 再抓一份 `gamestate`
  - 每次你在真实客户端里执行完一个动作后，按一次回车，它就把动作后的数据落盘

### 2.5 落盘结果必须长什么样
- 每次真实录制都会生成一个 session 目录，例如：
```text
results/real-client-trajectories/manual-001/
├── session_manifest.json
├── session_summary.json
├── rpc/
│   ├── 000.health.rpc.json
│   ├── 001.start.rpc.json
│   ├── 002-initial.gamestate.rpc.json
│   ├── 002-initial.save.rpc.json
│   └── 002-initial.screenshot.rpc.json
├── steps/
│   ├── 002-initial.gamestate.json
│   ├── 003-manual.gamestate.json
│   └── ...
└── screenshots/
    └── 002-initial.png
```
- 文件与需求的对应关系：
  - `seed`：`session_manifest.json > start_request.seed`
  - 初始 `deck / blind / shop`：`steps/002-initial.gamestate.json`
  - 每一步动作输入：`session_manifest.json > steps[].action_label`
  - 每一步动作后的状态 snapshot：`steps/*.gamestate.json`
  - 最终 `round / shop / game over` 结果：`session_summary.json` 以及最后一个 `steps/*.gamestate.json`
- 第一版 recorder 当前能可靠落盘的是：
  - `gamestate`
  - `save` RPC 返回值
  - `screenshot` RPC 返回值
- “关键事件链”分两层：
  - 现在就能录的：动作标签 + 状态切换前后 snapshot + save checkpoint
  - 后续必须补的：通过 Lovely hook 记录 `G.STATE` 变化、`evaluate_play` 内部计分链、Joker before/main/after、blind side effect、RNG 消耗顺序

### 2.6 人工录制时怎么操作
- 推荐流程：
  - 用 BalatroBot `start` 明确指定 `deck / stake / seed`
  - 录制器抓 `002-initial`
  - 你在真实客户端做一个动作
  - 回到终端，输入动作标签，例如：
    - `select_blind_small`
    - `skip_blind`
    - `play_two_pair`
    - `discard_0_1_4`
    - `buy_shop_item_1`
    - `sell_joker_0`
    - `reroll_shop`
    - `use_consumable_0`
    - `cash_out`
    - `next_round`
  - 在真实客户端动作真正落地后按回车，录制器抓动作后的 snapshot
- 这一步一定要保持“先在游戏里做动作，再在终端确认抓取”，不能反过来。

### 2.7 覆盖面要求
- 第一批人工轨迹至少要覆盖这些 session：
  - `blind-select-and-play`
  - `skip-blind`
  - `discard-line`
  - `shop-buy-sell-reroll`
  - `consumable-use`
  - `round-eval-to-shop-to-next-blind`
- 每条 session 都要能在目录里看到：
  - 初始 snapshot
  - 中间步骤 snapshot
  - 最终 snapshot
  - 明确的动作标签序列

### 2.8 这一步如何证明“确实采到数据”
- 不是只看到游戏窗口打开就算成功。
- 只有同时满足以下条件才算成功：
  - `results/real-client-bootstrap.json` 存在，并且 `capture_ready=true`
  - `curl ... health` 返回 `status=ok`
  - `results/real-client-trajectories/<session>/session_manifest.json` 存在
  - `results/real-client-trajectories/<session>/steps/` 里至少有 `initial`、中间步骤、`final` 三类 snapshot
- 如果只打开了游戏，但 `results/real-client-trajectories/` 里没有 step 文件，那就不算录到 trajectory。

3. 本地 AI 复现
- 在有了一批真实 trajectory 之后，再让本地 AI 或 rule-based agent 读取这些真实状态与动作序列。
- 复现方式分两层：
  - 动作级复现：在同一观测下是否能做出与真人一致的动作
  - 环境级复现：在同一 seed 与动作序列下，模拟环境是否能得到与真实客户端完全一致的状态转移
- 本阶段不是为了“打得好”，而是为了验证环境是否“演得对”。

4. 用复现一致性判断 simulator fidelity
- 之后 Harness 要复现环境时，核心判据不是单个规则点，而是 trajectory end-to-end 一致性。
- 对比和 replay 阶段允许关闭动画或尽量压缩动画时长，以提升渲染效率和整条流水线吞吐。
- 关闭动画只应影响视觉表现和等待时间，不应改变状态推进、事件顺序、RNG 调用或动作语义。
- 验证维度至少包括：
  - 下一状态是否一致
  - chips / mult / dollars 是否一致
  - hand / deck / discard / jokers / consumeables 是否一致
  - blind / shop / reroll / skip side effects 是否一致
  - joker 触发顺序与 retrigger 结果是否一致
  - RNG 结果是否一致
- 只要任意一步偏离，环境就视为未通过。

5. 通过标准
- `100%` 复现真实 trajectory 才算通过。
- 这里的 `100%` 指：
  - 全部动作合法且一致
  - 全部关键状态字段一致
  - 全部阶段切换一致
  - 全部随机结果一致
- 不接受“最终分数差不多”或“绝大多数 case 正确”作为通过标准。

6. 推荐执行顺序
- 第一步：静态分析本机 `Balatro.love`
- 第二步：接通 `Steamodded + Lovely + BalatroBot`
- 第三步：设计并实现真实 trajectory recorder
- 第四步：人工录制一批高质量 trajectory
- 第五步：实现本地 AI / policy replay 复现器
- 第六步：用 replay diff 框架逐步打磨 simulator，并在对比阶段默认关闭动画以提高跑通效率
- 第七步：只有达到 `100%` trajectory match，才允许把环境作为 Harness 的可信后端

## User Decisions
- 决策：trajectory recorder 是否只记录 `save_run()` 风格 snapshot，还是同时记录更细的事件链。
- 为什么重要：只录 snapshot 足够做状态比对，但不一定足够定位 Joker 触发顺序错误。
- Recommended default：同时记录 snapshot 和关键事件链。

- 决策：第一批 trajectory 是否只覆盖常规开局，还是从一开始就覆盖复杂 Joker / boss / shop case。
- 为什么重要：常规样本容易起步，但复杂 case 才能暴露 simulator 的真实偏差。
- Recommended default：先做一批常规样本，再补一批高复杂度样本。

- 决策：Harness 的准入条件是否允许分阶段通过。
- 为什么重要：这会决定团队是否接受“部分 fidelity”的中间环境。
- Recommended default：不允许。只有 `100%` trajectory 复现才通过。

## Risks and Assumptions
- 假设：真实客户端轨迹是唯一金标准，模拟环境只能向它对齐，不能反过来定义规则。
- 风险：如果只做静态规则抽取，不接真实客户端，很容易遗漏事件顺序和 RNG 调用顺序。
- 风险：如果太早让 AI 自己探索，会把错误环境当成正确环境放大。
- 风险：只比较最终结果而不比较中间状态，会掩盖大量 phase-order bug。

## Execution Gate
- 当前仓库已经补了：
  - 真实客户端环境探针：`scripts/probe_real_client_env.py`
  - 人工录制脚本：`scripts/record_manual_real_trajectory.py`
- 当前门槛不再是“设计 recorder”，而是：
  - 先把 `Lovely + Steamodded + BalatroBot` 真正装到本机 Balatro 上
  - 让 `results/real-client-bootstrap.json` 从 `capture_ready=false` 变成 `true`
  - 录出第一条真实 session 到 `results/real-client-trajectories/`
- 在这之前，不继续把近似 simulator 包装成“已经有真实金标准”。
