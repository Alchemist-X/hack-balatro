# 260316 Handoff

## 1. 当前任务目标
当前任务是把仓库中的 Balatro 原生环境继续推进到更接近 `Balatro 1.0.1o-FULL` 的规则一致性，重点不是继续做 mock 或训练脚手架，而是修正核心规则层。

本轮明确要解决的问题：
- 统一规则来源并把 `Joker` 全量数据接入 ruleset
- 修正 `Blind` 线性流程和 `skip_blind` 规则
- 修正 `Shop` 经济规则，尤其是 `reroll` 价格和 `sell_value`
- 把 `Joker` 触发顺序从“散落在 score 逻辑里的简化分支”重构为显式激活阶段
- 让 replay / behavior log / viewer 能看到激活链和更真实的 shop/joker 状态

预期产出：
- 可重新生成的 ruleset bundle，包含 150 个 `Joker` 的机械字段和英文效果文本
- 可编译的 Rust engine，支持新的 `JokerInstance` / `Snapshot` / `Event` 结构
- 通过测试的 blind / shop / activation-order 行为
- 刷新的 replay / behavior log / viewer 产物

完成标准：
- `cargo check` / `cargo test` 通过
- 规则 bundle 重新生成成功，且 150 个 `Joker` 三方对齐
- `skip_blind`、`reroll_cost`、`sell_value`、`activation order` 至少有针对性测试
- replay 中能看到更结构化的 `Joker` / shop / activation 信息

## 2. 当前进展
已完成的分析和确认：
- 已确认外部权威源：
  - 本地 `Balatro.love` / `game.lua`
  - `https://balatrowiki.org/w/Module:Jokers/data?action=raw`
  - `https://balatrowiki.org/w/Common_Jokers?action=raw`
  - `https://balatrowiki.org/w/Uncommon_Jokers?action=raw`
  - `https://balatrowiki.org/w/Rare_Jokers?action=raw`
  - `https://balatrowiki.org/w/Legendary_Jokers?action=raw`
  - `https://balatrowiki.org/w/Guide:_Activation_Sequence?action=raw`
  - `https://balatrowiki.org/w/The_Shop?action=raw`
  - `https://balatrowiki.org/w/Glossary?action=raw`
  - `https://balatrowiki.org/w/Skip?action=raw`
- 已验证可以从 Wiki 的四个 rarity 页面解析出完整 `150` 条 `Joker` 的 `Name / Cost / Effect`
- 已验证以下规则事实：
  - `Small Blind` 和 `Big Blind` 可以跳过
  - `Boss Blind` 不能跳过
  - `reroll` 默认从 `$5` 开始，每次 `+1`，进入新 shop 重置
  - `sell value = floor(buy_cost / 2)`，并受 edition/discount/special cases 影响
  - `Joker` 不是统一“先结算”；应按 `Activation Sequence` 阶段执行，同阶段内按 Joker 槽位从左到右

已存在且可用的仓库能力：
- 原生 Rust 路径已存在：
  - `crates/balatro-spec`
  - `crates/balatro-engine`
  - `crates/balatro-py`
- Python 包装、replay、viewer、rule-based agent、coverage runner 已经有基础实现：
  - `env/balatro_gym_wrapper.py`
  - `agents/simple_rule_agent.py`
  - `scripts/behavior_log.py`
  - `scripts/record_replay.py`
  - `scripts/run_simple_rule_coverage.py`
  - `viewer/index.html`
- 前一轮已经实现过：
  - blind 线性推进 `Small -> Big -> Boss`
  - `shop_jokers` 只在 `Stage_Shop` 显示
  - timestamped replay / behavior log / coverage artifacts

本轮已动手但未收尾的修改：
- `crates/balatro-spec/src/lib.rs`
  - 已给 `JokerSpec` 增加：
    - `base_cost`
    - `wiki_effect_text_en`
    - `activation_class`
    - `source_refs`
- `scripts/build_ruleset_bundle.py`
  - 已加入 HTML 解析骨架，用来解析 Wiki rarity 页面
  - 已加入 `fetch_wiki_rendered_html()`、`JokerTableParser`、`parse_wiki_joker_table()`、`load_wiki_joker_display_specs()`
  - 已把 `SHOP_WEIGHTS["legendary"]` 从 `0.3` 改成 `0.0`
  - 已准备把每个 Joker 的 display spec 合并进 ruleset
- `crates/balatro-engine/src/lib.rs`
  - 已开始扩展：
    - `JokerInstance`
    - `Snapshot`
    - `Event`
    - engine state 内的 shop reroll 相关字段
  - 但 engine 构造逻辑尚未全部补齐，当前编译失败

## 3. 关键上下文
重要背景信息：
- 用户目标一直不是“做一个大概类似 Balatro 的环境”，而是把规则逼近原版，并且明确强调：
  - 必须全量获取 `Joker`
  - 要注意结算顺序
  - `Joker` 的顺序是从左到右按槽位叠加
- 用户之前明确指出过当前实现错误：
  - 不能自由选择 `Small / Big / Boss`
  - `Boss` 不能跳过
  - `Shop` 不应该一直存在
  - `reroll` 价格不是固定 `$1`

已知约束：
- 当前仓库是 dirty worktree，不要覆盖用户已有改动
- 必须优先使用 `apply_patch` 做文件编辑
- 当前环境有网络，可以访问 Balatro Wiki
- 当前日期为 `2026-03-16`，handoff 文件名按 `260316-handoff.md`

已做出的关键决定：
- 规则优先级：
  - `game.lua` 决定机械字段
  - Wiki 的 `Activation Sequence` / `Shop` / `Skip` / `Glossary` 决定解释性顺序和价格规则补充
  - Wiki rarity 页面决定人类可读 `Joker effect text`
- 不再接受“Joker 只有少量 hardcode 特例”的方向；要先把 150 个 Joker 全量纳入 bundle
- 不再让 rule-based policy 主导规则；policy 只能消费 engine 提供的合法动作和状态

重要假设：
- 这一轮先把核心规则层做对，不追求一次性补齐所有 UI 交互
- 即使某些 voucher / edition / sticker 机制还没有全部跑通，价格模型也要先按照完整公式留出结构
- `Legendary Jokers` 虽然不能正常在 shop 中购买，但必须出现在 bundle 中

## 4. 关键发现
1. 当前最大的技术阻塞不是“找不到规则”，而是代码处于半重构状态。
   - 已经给 Rust 类型加了新字段，但 engine 里旧的构造点还没全部更新，导致整个 crate 编不过。

2. 当前 `cargo check` 的直接失败点只有两个，但它们是全局 blocker：
   - `crates/balatro-engine/src/lib.rs:987`
     - `JokerInstance` 初始化缺少新字段
   - `crates/balatro-engine/src/lib.rs:1171`
     - `Event` 初始化缺少新字段

3. 这两个报错说明：
   - 类型扩展已经生效
   - 但 `refresh_shop()` 和 `event()` helper 仍是旧逻辑
   - 所以现在还没到“调行为”的阶段，先得把 build 恢复

4. Wiki 数据抓取是可行的，不需要再重复论证：
   - `Module:Jokers/data` 能给到结构化机械字段
   - 四个 rarity 页面通过 MediaWiki parse API 可以拿到表格 HTML
   - 已实测解析出：
     - `Common 61`
     - `Uncommon 64`
     - `Rare 20`
     - `Legendary 5`
     - 合计 `150`

5. `Activation Sequence` 的关键点已经查实，不建议再猜：
   - `Boss blind` 效果先
   - 再 `On played Jokers`
   - 再牌面 scoring，从左到右
   - `On scored Jokers` 同一张牌按 Joker 槽位左到右
   - retrigger 复制前一段激活序列，不是简单重复加分
   - `held in hand` 之后才到 `independent Jokers`
   - `independent Jokers` 也按 Joker 槽位左到右

6. `Shop` 规则的关键事实已经查实，不建议再猜：
   - `reroll` 初始 `$5`
   - 每次 `+1`
   - 新 shop 重置
   - `sell value` 需要基于当前 buy-cost 重算

7. 当前 worktree 状态很重要：
   - 已跟踪修改：
     - `crates/balatro-engine/src/lib.rs`
     - `crates/balatro-spec/src/lib.rs`
     - `env/balatro_gym_wrapper.py`
     - `progress.md`
     - `results/replay-latest.html`
     - `results/replay-latest.json`
     - `scripts/build_ruleset_bundle.py`
     - `scripts/record_replay.py`
     - `viewer/index.html`
   - 未跟踪文件：
     - `agents/simple_rule_agent.py`
     - `rules/03-behavior-log.md`
     - `rules/04-fidelity-and-coverage.md`
     - `scripts/behavior_log.py`
     - `scripts/run_simple_rule_coverage.py`
     - `tests/test_behavior_log.py`
     - `tests/test_fidelity_and_coverage.py`
     - `todo/`

## 5. 未完成事项
按优先级排序：

1. 恢复编译状态
- 补齐 `crates/balatro-engine/src/lib.rs` 中所有 `JokerInstance` 和 `Event` 构造点
- 让 `cargo check` 先通过

2. 完成 ruleset 生成管线
- 让 `scripts/build_ruleset_bundle.py` 真正输出：
  - `base_cost`
  - `wiki_effect_text_en`
  - `activation_class`
  - `source_refs`
- 重新生成 `fixtures/ruleset/balatro-1.0.1o-full.json`
- 为 bundle 增加测试，验证 150 条 `Joker` 三方一致

3. 修正 shop 价格模型
- 新增 runtime 的：
  - `buy_cost`
  - `sell_value`
  - `shop_base_reroll_cost`
  - `shop_current_reroll_cost`
  - `shop_reroll_count`
- 改掉当前：
  - `reroll_shop = $1`
  - `sell_joker = cost / 2`

4. 修正 `refresh_shop()` 语义
- `Legendary` 不应该进入普通 shop 抽取池
- shop 中 Joker 的 runtime 字段应完整填充
- 买下商品后应从 shop 中移除或以正确方式更新，不要继续保留旧对象语义

5. 重构激活顺序
- 不再把 Joker 效果统一塞在 `play_selected()` 末尾
- 拆成显式阶段
- 增加结构化 activation 事件，供 replay 和 log 使用

6. 完善 Joker 覆盖
- 至少先按 family 分批实现：
  - additive / xmult
  - suit / hand-type dependent
  - scaling
  - copy / retrigger / destroy
  - economy / shop / blind / end-of-round

7. 同步 Python / replay / viewer
- `crates/balatro-py/src/lib.rs`
- `env/balatro_gym_wrapper.py`
- `scripts/record_replay.py`
- `scripts/behavior_log.py`
- `viewer/index.html`

8. 回归测试和产物刷新
- `cargo test`
- `cargo check -p balatro-py`
- `python -m py_compile ...`
- 重录 replay、刷新 HTML、刷新 coverage

## 6. 建议接手路径
优先查看的文件：
- `crates/balatro-engine/src/lib.rs`
- `scripts/build_ruleset_bundle.py`
- `crates/balatro-spec/src/lib.rs`
- `crates/balatro-py/src/lib.rs`
- `tests/test_fidelity_and_coverage.py`
- `tests/test_behavior_log.py`

优先验证的事情：
1. 先跑 `cargo check`
   - 确认仍然只卡在 `JokerInstance` / `Event` 构造点
2. 看 `crates/balatro-engine/src/lib.rs` 当前 diff
   - 重点检查：
     - `refresh_shop()`
     - `handle_shop()`
     - `event()`
3. 看 `scripts/build_ruleset_bundle.py`
   - 确认 HTML 解析骨架已经存在，不要重写一套新的 parser

推荐的下一步动作：
1. 在 `balatro-engine` 内先补齐类型构造，恢复 build
2. 在 `build_ruleset_bundle.py` 完成 display spec 合并并重建 fixture
3. 为新的 bundle 和 pricing 写最小测试
4. 再回头重构 activation stages

可以直接复用的命令：
- 查看工作树：
  - `git status --short`
- 验证当前 blocker：
  - `cargo check`
- 重新生成 bundle：
  - `python scripts/build_ruleset_bundle.py`
- 运行 Rust 测试：
  - `cargo test`

## 7. 风险与注意事项
1. 不要重复做已经验证过的外部规则研究。
- `skip_blind`
- `reroll`
- `sell value`
- `activation order`
这些事实已经从 Wiki raw / parse 页面拿到，继续停留在“再确认一次”会浪费时间。

2. 当前最容易跑偏的方向是：
- 继续给 `behavior_log` 或 viewer 加表面功能
- 继续扩充 rule-based agent
- 继续录 replay
这些都不是 blocker。现在 blocker 是 engine 编不过。

3. 不要把“bundle 全量 Joker”误解为“本轮必须把 150 个 Joker 效果全部完整实现后才能提交任何中间成果”。
- 正确做法是：
  - 先让 150 个 Joker 元数据完整进入 bundle
  - 再逐批实现 effect family
  - 同时明确哪些 Joker 仍是 smoke-only / 未完成

4. 小心 worktree 里已有的未跟踪文件。
- `agents/simple_rule_agent.py`
- `scripts/behavior_log.py`
- `scripts/run_simple_rule_coverage.py`
- `tests/test_behavior_log.py`
- `tests/test_fidelity_and_coverage.py`
这些是前一轮已经写出的有效内容，不要误删或当成垃圾文件。

5. `SHOP_WEIGHTS["legendary"]` 已经在 `scripts/build_ruleset_bundle.py` 被改成 `0.0`。
- 如果后续重新设计 shop 抽样逻辑，要注意不要又把 `Legendary` 放回普通 shop 池。

6. 当前 `results/replay-latest.*` 和 `progress.md` 都是旧产物。
- 在 engine 编不过的情况下，不要相信这些产物仍然反映最新代码状态。

## 下一位 Agent 的第一步建议
先不要继续“设计”，也不要先碰 UI。第一步直接在仓库根目录运行 `cargo check`，然后按报错去补 `crates/balatro-engine/src/lib.rs` 中 `JokerInstance` 和 `Event` 的所有构造点，让工程恢复到可编译状态。只有 build 恢复后，后面的 ruleset 重建、shop 定价和 activation-order 重构才有意义。
