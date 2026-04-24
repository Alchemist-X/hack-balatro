# 接口收口计划 — Gym 废弃 / 新主线正式化

_起草_: 2026-04-24  
_触发_: 朋友给的 review（见会话 / 主张单主接口 + 废弃 BalatroEnv）  
_状态_: 完成（2026-04-24 所有 A/B/C/D/E 决策已落地）

## 执行后记（2026-04-24）

| 段 | commit | 状态 |
|---|---|---|
| A1 / A2（主接口 + 废弃 Gym）| `9c6265b` → merge `cd52918` | ✅ `env/legacy/` 等目录就位，DeprecationWarning 生效 |
| B1（拆 serializer + 归档策略）| `7797cea` `560842b` → merge `b67fbf5` | ✅ 5 条硬编码策略已归档到 `docs/archived_strategy_hints_20260424.md` |
| B2（canonical trajectory 扩字段）| `8b5d962` `7cd2460` → merge `2314efe` | ✅ 向后兼容，两份已提交 canonical JSON 重新生成通过 |
| B3 + C（README 收口 + 三层对齐口径）| `f84da21` | ✅ "尚未实现" 更新，新增 schema / value / semantic 三层说明 |

风险记录（由 B2 subagent 报告）：
- 3 份旧 `game_*.json` trajectory 未迁移（shape 不同）。迁移脚本 `scripts/migrate_llm_trajectories_to_canonical.py` 已就位但未运行——等具体使用需求再迁。
- Observer session 的 `legal_actions / parsed_action / executed_action` 为 `None`（录的时候没抓），已标 `info.reconstructed = true` 供下游区分。
- `crates/balatro-py/src/lib.rs:619` 有一句 `py.import("env.state_encoder")`，只有在 `legacy_86x454` observe profile 下被触发。没有主路径依赖它，留作将来复活 legacy 时的已知 TODO。

cargo test: **120 / 120 全绿**；`import env / env.legacy / sim_repl / llm_play_game` 全部 smoke 通过。

## 批注摘要（给 subagent 用作执行规约）

- **A1 / A2**: 废弃 Gym，完全删除 PPO/Gym 路线
- **B1**: 拆 state_serializer，**去掉所有硬编码策略建议并存档**（不是挪到 playbook，是搬到独立 archive 文件）
- **B2**: 只保留一个 canonical trajectory schema，不要搞多个；如果实现有风险（旧 trajectory 不兼容），在 report 里明确标出来
- **B3**: README / docs 按 D 完成后收口
- **C**: README 里要解释清楚 "value 对齐" 和 "semantic 对齐" 是什么意思
- **D**: 按 12 步 checklist 执行

---

## 待你批注的格子

每条建议后面留了 `[ ]` 给你打勾、也可以直接写 `YES / NO / 改成xxx`。

---

## A. 核心决策（请你先拍板这一条）

### A1. 官方指定"唯一主接口"

**提议**：主接口 = **`balatro_native.Engine` + `state_serializer` (pure facts) + canonical trajectory**。

**批注格**：
- [ ] 同意 / 不同意
- 修改意见：肯定同意啊，删掉gym 完全

---

### A2. `BalatroEnv` / `state_encoder.py` / `action_space.py` 处置

**提议**：**废弃（deprecate）**，具体含义——
- 代码物理搬到 `env/legacy/`（保留 git 可追溯）
- `env/__init__.py` 停止公开 `BalatroEnv`
- 导入时 emit `DeprecationWarning`
- README / docs 统一打 🚧 legacy 标签
- 不删除、不破坏——3 个月后需要 RL baseline 可以复活

**对立选项**：**全面修复**（按朋友 B1–B6 + dead config 全做完 ≈ 10–14 天工作量）

**我的倾向**：废弃。理由：
1. PPO 路线正式 pivot（见 `CLAUDE.md` Diagnosis-First Debugging 示例）
2. 当前零依赖者
3. 修 ≈ 重写，不如 3 个月后用新 snapshot 真实需求重新设计
4. 可回溯，不毁资产

**批注格**：
- [ 1 ] 废弃
- [ ] 全面修复
- [ ] 其它：
- 如果选全面修复，指派/时间：

---

## B. 主接口收口动作（如果你同意 A）

### B1. 拆 `state_serializer.py`

**当前问题**：事实序列化 + prompt 组装 + 硬编码中文策略提示混在一个文件。

**提议**：
- `env/state_serializer.py` —— 只留纯事实输出（`serialize_state()`）
- `env/prompt_builder.py` —— 新建，放 `serialize_for_llm_prompt()` + 语言选择 + prompt 策略
- **删掉 `serialize_for_llm_prompt()` 里的硬编码策略建议**（例：_"plays=0 时绝不弃牌"_ _"优先保证 $5 倍数利息"_ _"X Mult 小丑最优先购买"_）——**违反我们自己 CLAUDE.md 的 Objective vs Subjective 强制规则**
- 策略建议挪到 `agents/<agent_name>/playbook.md`（per-agent，不是全局）

**批注格**：
- [ ] 同意
- 要不要保留 playbook 机制：
- 哪些策略建议你想保留、哪些删：去掉所有硬编码建议，并标注，如果需要单独存档

---

### B2. 正式定义 canonical trajectory schema

**当前问题**：三条路（`scripts/llm_play_game.py` / `sim_repl` / real-client adapter）各写各的 trajectory，字段半对齐半不对齐。

**提议**：`env/canonical_trajectory.py` 已有雏形，扩字段到强制：

```
step_idx
ts
state_before       # 冻结事实
legal_actions      # legal action 列表
requested_action   # agent 原始输出（可能是字符串 "play"）
parsed_action      # 解析到 action_idx 或 None
executed_action    # 真实执行的（可能经过 fallback）
fallback_used      # bool + 原因
state_after
reward
terminal
info               # 自由扩展
```

所有 collector（LLM / sim REPL / real-client / 未来 online RL）必须产出这个 schema。

**批注格**：
- [ ] 同意
- 字段增减：只需要保留一个有效的，有风险点在跟我说

---

### B3. README + docs 收口

**当前问题**：README 前面"尚未实现"说 real-client trajectory 没做，后面有完整章节；Gym/PPO 标 ✅ ready 但实际没收口。

**提议**（派 subagent 干）：
- 删 README "尚未实现"里关于 real-client 的项
- Gym/PPO readiness 从 ✅ 改 🚧 legacy，加一句"本路线已退役，主接口见下文"
- `docs/project-overview.md` 已经把 state_encoder / balatro_gym_wrapper 标为 "PPO 遗留" — 同步到 README
- 新增一小节「主接口」清晰指向：`Engine + state_serializer + canonical trajectory`

**批注格**：
- [ 1] 同意
- 有要额外加/删的段落：

---

## C. sim↔real 差分的诚实口径

**当前问题**：对外说 "98/148 aligned"，听上去"接近对齐"，实际上还有 **38 value_mismatch**——真要说"对齐"差远了。

**提议**：
- 报告头部加明确**语义阶段**分层：
  - _schema 对齐_: 98/148 ✅ （字段存在、类型对）
  - _value 对齐_: 待做（需要 seed-aligned 真实轨迹 + P2 step-by-step diff）
  - _semantic 对齐_: 待做（tag 效果 / voucher 效果 / joker 交互都要真执行）
- README / todo 里**禁止**不加限定地说"已对齐"

**批注格**：
- [ ] 同意
- 要加别的口径区分：这些要做啥解释下 在readme里加上   - _value 对齐_: 待做（需要 seed-aligned 真实轨迹 + P2 step-by-step diff）
  - _semantic 对齐_: 待做（tag 效果 / voucher 效果 / joker 交互都要真执行）

---

## D. 废弃 Gym 的具体执行单（如果 A2 选"废弃"）

按顺序：

1. `git mv env/balatro_gym_wrapper.py env/legacy/balatro_gym_wrapper.py`
2. `git mv env/state_encoder.py env/legacy/state_encoder.py`
3. `git mv env/action_space.py env/legacy/action_space.py`
4. `git mv training/ legacy/training/`
5. `git mv scripts/train_smoke.py scripts/legacy/train_smoke.py`（如果用）+ `train_ppo.py` / `train_bc.py` / `train_full_pipeline.py`
6. `env/__init__.py` 改为：
   ```python
   # Legacy PPO/Gym interface (deprecated 2026-04-24)
   # See todo/20260424_interface_consolidation_plan.md for rationale.
   # To resurrect, see env/legacy/README.md
   ```
7. `env/legacy/__init__.py` 里 emit `DeprecationWarning`
8. 新建 `env/legacy/README.md` 写"复活指引"
9. README 章节：`## LLM / RL 训练环境接口` → 删 "Gym 风格" 段；或改写成"（已退役）"
10. `configs/repro.yaml` / `configs/train_smoke.yaml` 等——移到 `configs/legacy/` 或注明废弃
11. `cargo test` + 跑一次 `sim_repl` + `llm_play_game.py` smoke 验证没打坏主路
12. Commit: `chore: deprecate BalatroEnv / PPO training scaffolding (moved to env/legacy/)`

**批注格**：
- [ ] 同意执行顺序
- 要保留哪些不搬：

---

## E. 优先级与时间盘

| 项 | 工期 | 依赖 |
|---|:---:|---|
| A1 / A2 决策 | 你批阅 5 分钟 | — |
| D 废弃执行 | subagent 1–2 小时 | A2 同意"废弃" |
| B1 拆 serializer + 删硬编码策略 | subagent 1–2 小时 | — |
| B2 canonical trajectory 字段扩充 | subagent 2–3 小时 | — |
| B3 README / docs 收口 | subagent 30 分钟 | D 完成 |
| C sim↔real 口径修正 | 10 行改动 | — |

**建议执行顺序**：
1. 你先批阅此文档（5–10 分钟）
2. 我派 1 个 subagent 做 D（废弃）
3. 并行派 1 个 subagent 做 B1（拆 serializer）
4. 两个回来后合并 + 派 B2 / B3 / C 最终收尾

---

## 你的整体结论 / 备注格子

（你可以在这里任意写长 text，我下一步按你写的执行）

> 

---

## 参考

- 朋友原始反馈（见会话）
- `CLAUDE.md` → _Objective vs Subjective — Content Rule (MANDATORY)_
- `CLAUDE.md` → _Diagnosis-First Debugging — 2026-04-11 PPO 失败案例_
- `README.md` → 现版本的"尚未实现"段 + Real client integration 段
- `results/sim-vs-real-gap-report.md` → 98/148 的具体分布
