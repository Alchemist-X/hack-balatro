# CLAUDE.md

## Project

hack-balatro: high-fidelity Balatro 1.0.1o simulator + AI agent research platform.

## Build & Test

```bash
cargo check                # Rust workspace compile check
cargo test                 # All Rust tests
unset CONDA_PREFIX && source .venv/bin/activate && maturin develop --manifest-path crates/balatro-py/Cargo.toml  # rebuild native extension
python scripts/record_replay.py --seed 42 --policy simple_rule_v1 --max-steps 500 --output results/replay-fidelity-a.json
python scripts/audit_replay.py --replay results/replay-fidelity-a.json --output results/replay-fidelity.audit.json
```

## Conventions

- Follow `Agent-Style.md` as the fidelity bar. Follow `agents.md` for progress persistence rules.
- Commit messages: `<type>: <description>` (feat, fix, refactor, docs, test, chore, perf, ci, checkpoint).
- Do not commit `vendor/`, `.venv/`, or files containing secrets.
- `game.lua` is authoritative for mechanical fields; Wiki is supplementary.

## Subagent-First Execution Policy (MANDATORY)

**The main conversation must stay free for communication with the user.**
Whenever a task involves actual work (code changes, long shell commands,
data collection, training, analysis, debugging, etc.), delegate it to a
subagent via the `Agent` tool. The main process should only:

1. Receive the user's request
2. Plan and decompose it into subtasks
3. Dispatch subagent(s) to execute
4. Report results back to the user in concise form

### When to dispatch subagents

**Always dispatch** for these kinds of work:
- Any task that reads or writes more than a couple of files
- Any task involving a long-running command (tests, training, builds, data collection)
- Any exploratory research across the codebase
- Any task requiring heavy tool use (10+ tool calls expected)
- Any task that could be done in parallel (always prefer parallel subagents)

**Dispatch guidelines**:
- Prefer **parallel** subagents over sequential when subtasks are independent
- Use `run_in_background: true` for long-running subagents so the main
  conversation can keep responding to the user
- Use `isolation: "worktree"` when multiple subagents modify the same files
- Give each subagent a **complete, self-contained prompt** with all context
  it needs (it cannot see our conversation)
- After dispatching, inform the user briefly what was launched and continue

### When NOT to dispatch

- Trivial single-file edits (< 3 tool calls)
- Quick status checks (reading one file, running one fast command)
- Clarifying questions back to the user
- Summarizing results the user already has

### Communication priority

While subagents are running, the main conversation MUST remain responsive.
Never block on a subagent when the user asks a question. Use `TaskList` or
check output files to report progress without waiting for completion.

## Objective vs Subjective — Content Rule (MANDATORY)

**Never mix objective mechanics with subjective strategy in files that
feed the model** — rule docs, prompts, training context, system messages, etc.

Before adding any content to a model-facing document, ask:
- Is this a **fact** (always true regardless of model/situation)?
- Or is this a **judgment** (my opinion on how to play well)?

If it's a judgment, it does **not** belong in the rule doc / prompt. It should
emerge from training data, not be hard-coded as context.

### Examples

| ✅ Objective (keep) | ❌ Subjective (remove) |
|---------------------|------------------------|
| "Pair gives 10 chips × 2 mult base" | "Always prioritize Pair in early Ante" |
| "Interest: $1 per $5 held, max $5" | "Keep money in $5 multiples to maximize interest" |
| "Cavendish: X3 mult, 1/1000 destroy chance" | "X Mult jokers are the most valuable purchases" |
| "plays=0 makes play action illegal" | "Save last play for the best hand" |
| "Boss The Goad: all Spades don't score" | "Avoid building Spade-heavy decks" |

### Rationale

Hardcoding a single strategy into the model's context forces one playstyle.
Different models / training regimes may discover better strategies we haven't
thought of. The rule doc teaches the game; the data teaches how to play it.

### Where to put strategy instead

- **Training trajectories** (CoT reasoning from strong agents)
- **Evaluation notes** (human analysis, kept separate from model input)
- **README / research notes** (for collaborators, not for the model)
- **Agent-specific playbooks** (e.g. `agents/<name>/playbook.md`) — explicitly
  scoped to one agent, not a global rule

Files like `rules/balatro_guide_for_llm.md` are model-facing and must stay
100% objective. Audit them periodically by grep-ing for opinion words:
"should", "best", "prefer", "优先", "最好", "应该".

## Diagnosis-First Debugging (MANDATORY)

**Do not change parameters/code in response to a symptom without first
writing down a hypothesis and a verification method.**

When a metric stalls, a test fails, or an agent behaves unexpectedly,
the first action is **diagnosis**, not **tuning**.

### Required ritual before any "fix"

State explicitly (in chat or as a comment):

```
SYMPTOM:        [what's wrong, with specific numbers]
HYPOTHESIS:     [specific guess at the cause]
VERIFICATION:   [exact command/check that confirms or rejects hypothesis]
```

If you cannot name a verification step, **stop and diagnose first**.

### Standard diagnostic tools (use these before tuning)

For training / agent debugging:
- **Action distribution** — which actions are being taken? (catches toggle loops, stuck agents)
- **Reward breakdown** — which reward components dominate? (catches reward hacking)
- **Episode length distribution** — are episodes too short/long? (catches early termination bugs)
- **Manual trajectory inspection** — read 5-10 actual trajectories end-to-end (catches anything automated checks miss)

For engine / test failures:
- **Minimal reproduction** — smallest seed/input that triggers it
- **State dump before/after** — what changed vs what should have changed
- **Git bisect** — if it worked yesterday, find the breaking commit

### Anti-patterns (ban these)

- ❌ "Let me try increasing X" without knowing why X matters
- ❌ Running another experiment with a slightly tweaked hyperparameter when
  the previous one's failure mode is not yet understood
- ❌ "Maybe the reward is too small, let me double it"
- ❌ Multi-parameter tuning without isolating one variable
- ❌ Copying a fix from another project without verifying the root cause matches

### Real example from 2026-04-11

6 PPO experiments (930K steps) were wasted tuning entropy/reward/play_bonus.
30 seconds of printing the action distribution would have revealed the agent
was spending 98% of steps in `select_card` toggle loops — making the entire
reward-shaping direction irrelevant. The fix was a routing change, not a
tuning change.

Always diagnose first.

## Workspace Hygiene

Keep experimental artifacts separate from long-term assets.

### File placement rules

- **Long-term code** → `crates/`, `env/`, `agents/`, `training/`, `scripts/`
- **One-off experiments / throwaway scripts** → `scripts/experimental/`
  or `.tmp/` (both gitignored by convention)
- **Documentation** → `rules/`, `docs/`, `todo/`
- **Generated artifacts** → `results/` (selectively gitignored)

### Before every commit

1. Run `git status` and explicitly account for every untracked file:
   - **Add it** — if it's a real asset
   - **Delete it** — if it's a throwaway
   - **Move it** — if it's in the wrong place
   - **Gitignore it** — if it's generated output

   Never leave untracked files "for later" — they become stale noise.

2. Stage with specific paths, never `git add -A`. This forces you to think
   about what's being committed.

### Background processes

When launching a background command or subagent, explicitly decide:
- **One-shot**: will complete and be discarded → OK to run in background
- **Long-lived**: needs monitoring → use `TaskList` / output file polling
- **Legacy**: from previous tasks → kill it with `TaskStop` before starting new work

Old background tasks that are no longer relevant generate notification noise
and confuse future diagnosis. Clean them up at the end of every task.

### Worktrees

When using `isolation: "worktree"` for parallel subagents:
- Each worktree = ephemeral. Nothing in it survives beyond the merge.
- After merging subagent results, **delete the worktree** with
  `git worktree remove` and the branch with `git branch -D`
- Never leave worktrees around "just in case" — they compound merge pain

## README-For-Collaborators Update Policy (MANDATORY)

Whenever a **major update** lands — new capability, new integration, breaking
schema change, or fidelity milestone — append a dated section to `README.md`
written in **plain language for non-developer collaborators** (game designers,
analysts, playtesters who may not read Python).

Every such section must cover, in order:

1. **What we did** (1 short paragraph, no jargon)
2. **Why it matters** (2-3 bullets, business/research value)
3. **How to try it** (numbered, copy-pasteable, <5 min recipe)
4. **What we learned** (1 honest paragraph, including limits)
5. **What we need from you** (explicit asks + coverage checklist)
6. **Known limitations** (short bullet list)
7. Pointer line to the architecture/test-plan doc.

Trigger example: "first end-to-end real-client capture works" -> append the
dated section *before* merging the feature branch. If the update changes how
collaborators interact with the repo, README must reflect it the same day.

## Auto Commit & Push

When a task is complete (tests pass, no regressions), **automatically commit and push** without asking:

1. Stage only relevant changed files (no `git add -A`).
2. Write a concise conventional-commit message summarizing the change.
3. `git push origin HEAD` to the current branch.
4. If working on a feature branch, create a PR via `gh pr create` and **auto-merge** it:
   ```bash
   gh pr merge --auto --squash
   ```
5. If on `main` directly, just push. No PR needed.

### When to auto-commit

- After `cargo test` passes following a code change.
- After a checkpoint-worthy amount of progress (new feature, bug fix, refactor complete).
- Before switching to a different task area.
- If the last push was >12h ago, push a checkpoint commit even if work is in-progress.

### Commit scope

- Prefer atomic commits (one logical change per commit).
- For large changes, split into sequential commits (e.g., refactor first, then feature, then tests).
- Always verify `cargo check && cargo test` before committing.

## Key Paths

- `crates/balatro-engine/src/lib.rs` — core engine (scoring, shop, phases, joker effects)
- `crates/balatro-spec/src/lib.rs` — ruleset schema and loader
- `crates/balatro-py/src/lib.rs` — Python bindings (PyO3)
- `fixtures/ruleset/balatro-1.0.1o-full.json` — generated ruleset bundle
- `scripts/` — replay, audit, coverage, oracle tools
- `progress.md` — progress log with timestamps
- `todo/20260331_backlog.md` — current backlog and honest assessment
