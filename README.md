# hack-balatro rebuild

Reproducible Balatro RL rebuild scaffold with two-stage gate:

- `phase1`: doctor + baseline eval (Random vs Greedy) + reproducibility check
- `phase2`: BC pretrain + PPO train + final comparison (Random/Greedy/BC/PPO)

## Quick start

```bash
python scripts/build_ruleset_bundle.py
cargo test
python scripts/repro.py phase1
python scripts/repro.py phase2 --strategy strategy_stable
```

## Native rebuild

This repo now contains a first-party Rust workspace for the Balatro rebuild:

- `crates/balatro-spec`: versioned ruleset bundle schema/loader
- `crates/balatro-engine`: structured snapshot/action/transition engine
- `crates/balatro-py`: PyO3 binding exposed as `balatro_native`

Bundle and asset utilities:

```bash
python scripts/build_ruleset_bundle.py
python scripts/extract_balatro_assets.py --dest results/assets-preview
python scripts/record_replay.py --output results/replay.json
open viewer/index.html
```

## Main CLI

```bash
python scripts/repro.py phase1
python scripts/repro.py phase2 --strategy strategy_stable
python scripts/repro.py phase2 --strategy strategy_reward_boost
python scripts/repro.py phase2 --strategy strategy_transformer
python scripts/repro.py resume --strategy strategy_stable --resume checkpoints/latest.pt
python scripts/repro.py eval --ppo-checkpoint checkpoints/best.pt
python scripts/repro.py report --metrics results/<run_id>/phase2_metrics.json
```

## Project layout

- `env/`: Gym wrapper, action space, state encoder (454d)
- `crates/`: first-party Rust spec/engine/Python binding
- `agents/`: Random/Greedy/PPO
- `training/`: BC, rollout buffer, PPO trainer, curriculum scheduler, phase pipeline
- `eval/`: evaluation and comparison metrics
- `scripts/`: doctor/repro/train/eval entrypoints
- `viewer/`: native replay/state inspector
- `configs/repro.yaml`: single source config with strategy overlays
- `third_party.lock`: pinned upstream repositories/commits
- `vendor/`: cloned upstream dependencies

## Notes

- `fixtures/ruleset/balatro-1.0.1o-full.json` is generated from the local `Balatro.love` plus the Balatro Wiki joker table.
- `BalatroEnv` now prefers `balatro_native` when the extension module is installed, then falls back to `pylatro`, then mock.
- If `pylatro` is unavailable, env falls back to a deterministic mock engine for smoke and CI runs.
- Action/observation contracts are fixed at `86` / `454`.
- Results are written to `results/<run_id>/` as JSON + CSV.
