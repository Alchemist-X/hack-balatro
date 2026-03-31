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
