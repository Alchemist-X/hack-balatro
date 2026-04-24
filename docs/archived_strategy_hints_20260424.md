# Archived Strategy Hints (removed 2026-04-24)

Source: previously lived in `env/state_serializer.py::serialize_for_llm_prompt()`,
removed on 2026-04-24 to comply with `CLAUDE.md` "Objective vs Subjective —
Content Rule (MANDATORY)". Strategy must emerge from training data, not be
baked into model-facing text.

These hints are preserved here **only** for provenance / future reference.
They are **not** automatically wired into any agent. See
`todo/20260424_interface_consolidation_plan.md` (section B1, user
annotation: _"去掉所有硬编码建议，并标注，如果需要单独存档"_) for the
decision record.

---

## Hint 1: plays=0 时绝不弃牌

- **Context**: Step 3 of the "请决策" chain-of-thought rubric, rendered on
  every prompt regardless of stage.
- **Original text** (zh, verbatim): _"plays=0时绝不弃牌"_
- **Why it violates the rule**: encodes a rigid judgment ("never discard")
  when the factual rule (`plays=0` makes `play` the only progress action,
  and `discard` is already gated out of `legal_actions` in most scenarios)
  is already expressed in the objective state. The engine's legality check
  is authoritative; a hardcoded directive is a subjective playstyle.

## Hint 2: 优先保证 $5 倍数利息

- **Context**: Same "关键提醒" line as Hint 1, sent on every shop/pre-blind
  prompt.
- **Original text** (zh, verbatim): _"优先保证$5倍数利息"_
- **Why it violates the rule**: "优先" (prioritize) is an opinion word
  flagged by the Objective vs Subjective audit grep. The fact — "Interest:
  \$1 per \$5 held, max \$5" — already lives in the rules guide. Whether
  to optimize for interest vs. joker purchases is a strategic trade-off
  that should emerge from training, not be hardcoded.

## Hint 3: X Mult 小丑最优先购买

- **Context**: Same "关键提醒" line; applied to every shop decision.
- **Original text** (zh, verbatim): _"X Mult小丑最优先购买"_
- **Why it violates the rule**: "最优先" (highest priority) is a judgment.
  Cavendish (X3 Mult, 1/1000 destroy chance) is a factual stat, but whether
  X Mult jokers are the best purchase depends on deck composition, ante,
  money, joker slots, and existing synergies — precisely the reasoning we
  want the model to learn, not inherit.

## Hint 4 (ambient): "选择最佳动作"

- **Context**: Prompt framing line ("分析当前局面并选择**最佳**动作").
- **Original text** (zh, verbatim): _"分析当前局面并选择最佳动作"_
- **Why it violates the rule**: "最佳" (best) is an opinion word; there
  is no objective "best" without a defined reward/horizon, and encoding
  it in the prompt nudges the model toward greedy single-step thinking.
  Replacement prompt in `env/prompt_builder.py` simply asks for "a single
  legal action identifier" without qualitative framing.

## Hint 5 (ambient): "应该出牌、弃牌还是其他操作？为什么？"

- **Context**: Prompt chain-of-thought step 3.
- **Original text** (zh, verbatim): _"应该出牌、弃牌还是其他操作？为什么？"_
- **Why it violates the rule**: "应该" (should) is on the Objective vs
  Subjective audit wordlist. The question-form scaffolding encodes a fixed
  reasoning trace ("what should you do and why") — fine as a training
  trajectory style, wrong as a frozen system prompt. Per-agent playbooks
  can reintroduce this if desired.

---

## Reactivation guidance

If a specific agent wants opinionated strategy, create
`agents/<agent_name>/playbook.md` and paste the relevant hint there —
**never** back into `state_serializer` or `prompt_builder`. Per-agent
scope is acceptable; model-global scope is not.

Callers that want a custom system prompt can pass
`system_prompt=<string>` to `env.prompt_builder.build_prompt`; that
string is the agent's responsibility, not the framework's.
