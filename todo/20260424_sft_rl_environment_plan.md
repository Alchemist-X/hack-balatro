# SFT / RL 训练环境搭建计划

_起草_: 2026-04-24  
_触发_: 接口收口完成后（plan `20260424_interface_consolidation_plan.md`），开始走 README "训练路线图" Phase 0 → Phase 1  
_状态_: 草案，等批阅

---

## 待你批注的格子

每条建议后留 `[ ]` 给你打勾或写 `YES / NO / 改成xxx`。可以在"批注格"自由写长文。我等你批完再动手。

---

## 现状回顾（数据 / 代码 / 阻塞）

| 资产 | 状态 | 备注 |
|---|---|---|
| Engine + 120 tests | ✅ | balatro_native 直用 |
| 文本接口 (`state_serializer` + `prompt_builder`) | ✅ | 无硬编码策略 |
| `canonical_trajectory` schema | ✅ | 7 字段：legal_actions / requested_action / parsed_action / executed_action / fallback_used / reward / terminal |
| `action_inference` | ✅ | observer 数据 100% legal_actions |
| sim_repl | ✅ | 人手玩调试 |
| Real-client trajectory | 🟡 | 仅 2 份（round2 win 232 步 + observer-20260420 Ante2 loss 20 步）|
| **Sim 自玩 trajectory** | ❌ | 0 份（旧的全删）|
| LLM 自玩 harness | ❌ | 旧 `llm_play_game.py` 输出老 shape，已不能用 |
| Sim↔real value/semantic 对齐 | ❌ | schema 一层完成，value/semantic 待做 |
| PPO 历史 | 🚧 | 6 次失败，已 deprecate |

**阻塞最严重的是"零 sim 自玩 trajectory"**——SFT 没数据训。

---

## A. 整体路线（请你先拍板）

### A1. 是否走 README 既定 Phase 0→4 路线

README 写的：
- Phase 0: 数据准备 (100+ 局，30+ 过 Ante 4)
- Phase 1: SFT on (state, reasoning, action) → mean ante ≥ 3
- Phase 2: GRPO/DPO/PPO RL → win rate ≥ 30%
- Phase 3: MCTS + LM → win rate ≥ 60%
- Phase 4: Beyond Human

**我的建议**：保持，但**先只搭 Phase 0 和 Phase 1 的环境**，先不动 RL。理由：
1. sim 还没 value/semantic 对齐，RL 现在做就是优化错误目标
2. PPO 已失败 6 次，再上 RL 必须先 SFT 给个强冷启动
3. 数据是当前唯一阻塞

**批注格**：
- [ ] 同意（先做 Phase 0 + 1 的环境，RL 等数据齐后再说）
- [ ] 不同意 / 改成：

---

### A2. SFT 数据生成的"作者"

谁来玩 sim 产 (state, reasoning, action) trajectory？

| 选项 | 优 | 缺 |
|---|---|---|
| **Claude API**（外部 SDK 调）| 质量高、能并行、成本可控 | 按 token 付费、依赖网络 |
| **Claude Code CLI**（你包月）| 包月免费、和当前会话同款模型 | 串行、要走 cli wrapper |
| **本地 LM**（Qwen2.5/Llama）| 完全离线、可大规模 | 推理质量低、长 horizon 差 |
| **混合 A+C**（Claude 出种子 + 本地批量自玩）| 兼顾质量+量 | harness 复杂 |

**我的建议**：先用 **Claude API**（如有预算）或 **Claude Code CLI**（如想包月），目标 100 局，**reasoning 必须落盘**。

**批注格**：
- [ ] Claude API
- [ ] Claude Code CLI
- [ ] 本地 LM（说哪个）：
- [ ] 混合：
- 如果用 API，预算上限：

---

### A3. 数据规模目标

| 量级 | 用处 | 代价（按 Claude API ~$0.5/局粗估）|
|---|---|---|
| 30 局 | 看是否能走通管线 | ~$15 |
| 100 局 | README Phase 0 目标，最小可 SFT | ~$50 |
| 500 局 | 比较像样的 SFT 数据集 | ~$250 |
| 5000 局 | 接近 RL 自洽规模 | ~$2500 |

**我的建议**：分 3 阶段——先 **30 局** smoke 验证管线 → 看效果决定 **100 局** Phase 0 → 训完看 SFT 模型表现决定要不要 **500 局**。

**批注格**：
- [ ] 同意三阶段：30 → 100 → 500
- [ ] 改成：

---

### A4. Sim 还是 Real-client 作为训练环境

| 方面 | sim | real-client |
|---|---|---|
| 速度 | ~10 局/分钟 headless | 1 局 ~10-30 分钟 |
| 一致性 | 100% 决定性，可重放 | 需要游戏窗口活跃 |
| 准确性 | schema OK；value/semantic ❌ | 100%（金标准）|
| 自动化 | 完全 | 需 BalatroBot RPC + 游戏开着 |

**我的建议**：**sim 跑量 + real-client 抽样校准**。具体：
- Phase 0/1 SFT 数据 100% 来自 sim（量需要）
- 每个 SFT checkpoint 在 5–10 局 real-client（agent 通过 BalatroBot 驱动）上抽样测，看 sim 训出来的能力在真实环境是否 transfer

**批注格**：
- [ ] 同意（sim 训 + real-client 抽测）
- [ ] 全 sim
- [ ] 全 real-client
- [ ] 其它：

---

### A5. SFT 模型尺寸

| 选项 | 本地能跑 | 推理力 | 训练成本 |
|---|---|---|---|
| 3B (Qwen 2.5-3B / Llama 3.2-3B) | ✅ | 弱 | 低 |
| 7B/8B (Qwen 2.5-7B / Llama 3.1-8B) | ✅ Mac Studio 64G | 中 | 中 |
| 23B (Qwen 2.5-32B 量化 / Mistral 22B) | 🟡 量化勉强 | 强 | 中-高 |
| 70B+ | ❌ 必须云 | 最强 | 高 |

README 写 "23B"。

**我的建议**：实际起步用 **7B/8B**——Mac 本地能 inference + LoRA SFT，迭代快。证明管线 work 后再往 23B 升。

**批注格**：
- [ ] 7B/8B（说哪个具体模型）：
- [ ] 23B
- [ ] 其它：
- 训练硬件：Mac / 云 / 其它：

---

### A6. RL 时机

**我的建议**：**先不搭**。理由：
- 现状 PPO 失败案例新鲜（CLAUDE.md 记着）
- sim 对齐没收口，RL 优化错的目标
- 数据 0 → SFT 还没影
- 等 SFT 跑出"能玩 Ante 3"的 baseline，且 sim value 对齐做完，再讨论 RL

**对立**：现在就把 RL harness（GRPO/DPO 框架接入）搭好，等 SFT 一好就能上。

**批注格**：
- [ ] 同意：先不搭 RL，等 SFT 出 baseline + sim 对齐做完
- [ ] 现在就搭 RL 框架（说选 GRPO / DPO / PPO 哪个）：

---

## B. Phase 0 数据收集 harness（如果 A 同意走 SFT 路线）

### B1. 模块设计

新建 `agents/llm_player.py`（如不存在）+ `scripts/collect_sft_trajectories.py`：

```
collect_sft_trajectories.py
  for game_idx in range(N):
    seed = next_seed()
    eng = Engine(seed=seed, deck="red", stake=1)
    traj = CanonicalTrajectory(meta=..., steps=[])
    while not eng.is_over():
      snap = eng.snapshot()
      legal = eng.legal_actions()
      prompt = build_prompt(snap, legal, lang="en")
      response = llm.complete(prompt)        # 调 Claude / 本地 LM
      reasoning, action_str = parse_response(response)
      action_idx, fallback = parse_action_to_idx(action_str, legal)
      transition = eng.step(action_idx)
      traj.steps.append(CanonicalStep(
        step_idx=...,
        state_before=snap_to_canonical(snap),
        legal_actions=[a.index for a in legal],
        requested_action=action_str,
        parsed_action=action_idx,
        executed_action=action_idx if not fallback else fallback_idx,
        fallback_used=FallbackInfo(used=fallback, reason=...),
        action=CanonicalAction(...),
        state_after=snap_to_canonical(eng.snapshot()),
        reward=transition.reward if hasattr(transition,'reward') else None,
        terminal=eng.is_over(),
        info={"reasoning": reasoning, "raw_response": response},
      ))
    traj.to_json(out_path)
```

**批注格**：
- [ ] 同意整体设计
- [ ] reasoning 字段单独叫 `info.reasoning` 还是单独提到顶层 `CanonicalStep.reasoning`：
- 改进：

### B2. Prompt 设计

- **System prompt**：`prompt_builder.DEFAULT_SYSTEM_EN`（已有，无策略）
- **User prompt**：当前 state 文本 + 合法动作列表
- **Output 约定**：要求模型先输出 `# <reasoning>` 再输出动作字符串（一行一个）

**风险**：模型输出格式不稳，需要 lenient parser + fallback 到随机合法动作。

**批注格**：
- [ ] 同意（prompt 留通用，强制 reasoning 在前）
- [ ] 加结构化（要求 JSON {reasoning, action}）：
- 其它：

### B3. 失败 / 卡死保护

- 单局 max_steps（500 默认）
- 模型连续 3 次输出非法动作 → fallback 随机合法
- API 失败 → 重试 3 次后跳过该局
- 全局 timeout

**批注格**：
- [ ] 同意默认值
- 改：

---

## C. SFT 训练环境（如果 A 同意 7B/8B 本地路线）

### C1. 数据集 builder

`scripts/build_sft_dataset.py`：读 N 个 canonical trajectory → 转 HuggingFace dataset 格式 → 每条样本 `{prompt, completion}` 或 `{messages: [...]}`，落盘到 `results/sft_dataset/<run>/train.jsonl` + `eval.jsonl`。

**批注格**：
- [ ] 同意
- 数据集格式偏好（chatml / instruct / 自定义）：
- 比例 train/eval：

### C2. 训练脚本

不在仓内训（否则 git 跟踪 checkpoints 太大）——**`scripts/sft_train.py` 只做配置生成 + 调用外部框架**（HuggingFace TRL / Axolotl / Llama-Factory）。

**批注格**：
- [ ] 同意（用外部 framework，仓里只配置）
- [ ] 仓里写完整训练 loop：
- 偏好的 framework：

### C3. 评估 harness

`scripts/eval_sft_model.py`：加载 SFT 后的模型 → 在 N 局 sim 上跑 → 报 mean ante / win rate / score。

**批注格**：
- [ ] 同意
- 主指标选哪些：
- 评估局数：

---

## D. 仓库结构变化

预期会新增：
```
agents/
  llm_player.py            # 通用 LLM agent (Claude API / Claude Code CLI / local)
scripts/
  collect_sft_trajectories.py
  build_sft_dataset.py
  sft_train.py             # 配置生成器 + 外部 framework caller
  eval_sft_model.py
results/
  sft_runs/                # 已有 results/training/ 但 deprecated；新加 sft_runs/
    <run_id>/
      trajectories/        # canonical JSONs
      dataset/             # HF format
      checkpoints/         # gitignored
      eval/                # metrics
configs/
  sft/<run>.yaml
```

**批注格**：
- [ ] 同意结构
- 改：

---

## E. 优先级 / 时间盘

| 项 | 工期 | 依赖 |
|---|:---:|---|
| A1–A6 决策（你批阅） | 你批阅 10–15 分钟 | — |
| B1+B2+B3 数据收集 harness | subagent 2–3 小时 | A2/A4 决策 |
| 跑 30 局 smoke 验证 | 跑 ~30 分钟 + 看一遍 | B 完成 |
| 跑 100 局 Phase 0 数据 | API 自跑 ~3-5 小时 | smoke 通过 |
| C1+C2+C3 SFT 环境 | subagent 2–3 小时 | 100 局数据落地 |
| 实际 SFT 训练 | 外部硬件、~6-12 小时（7B）| C 完成 |
| Eval | 1 小时 | 模型有 |

---

## F. 关键风险

1. **数据生成质量**：Claude 不一定能玩到 Ante 4——你之前一局通关也是有 enhanced cards / seal / tag 加成。如果 30 局 smoke 后 mean ante < 2，可能要先**升级 prompt**或**找更强的 generator**
2. **Sim 不准**：Tag 效果只 8/24 实现、joker 交互简化 → SFT 训出来"sim-only 强"的模型，到 real-client 失效
3. **Reasoning 是否对训练有用**：CoT SFT 比 plain action SFT 强，但 Balatro reasoning 多是数字计算（chip×mult），LM 算数学不行
4. **本地 LoRA fine-tune 7B 在 Mac**：Apple Silicon 的 MLX 支持还有限，可能要走云
5. **Checkpoint 体积**：7B LoRA adapter 几十 MB ok，全权重 GB 级——必须 gitignore

**批注格**：
- 你最担心的风险：
- 想对哪个加额外检查/防护：

---

## G. 你的整体结论 / 备注格子

（任意写长 text，我下一步按你写的执行）

> 

---

## 参考

- README → `## 训练路线图` Phase 0–4 表
- `CLAUDE.md` → `Diagnosis-First Debugging` 里 6 PPO 实验失败案例
- `todo/20260424_interface_consolidation_plan.md` → 接口收口（前置）
- `env/canonical_trajectory.py` → trajectory schema 当前形态
- `env/prompt_builder.py` → 无策略 prompt 默认模板
- `env/action_inference.py` → 合法动作推断（用于 collector + observer）
