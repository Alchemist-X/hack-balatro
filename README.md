# hack-balatro rebuild

Reproducible Balatro RL rebuild scaffold with two-stage gate:

- `phase1`: doctor + baseline eval (Random vs Greedy) + reproducibility check
- `phase2`: BC pretrain + PPO train + final comparison (Random/Greedy/BC/PPO)

## Quick start

```bash
python scripts/repro.py phase1
python scripts/repro.py phase2 --strategy strategy_stable
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
- `agents/`: Random/Greedy/PPO
- `training/`: BC, rollout buffer, PPO trainer, curriculum scheduler, phase pipeline
- `eval/`: evaluation and comparison metrics
- `scripts/`: doctor/repro/train/eval entrypoints
- `configs/repro.yaml`: single source config with strategy overlays
- `third_party.lock`: pinned upstream repositories/commits
- `vendor/`: cloned upstream dependencies

## Notes

- If `pylatro` is unavailable, env falls back to a deterministic mock engine for smoke and CI runs.
- Action/observation contracts are fixed at `86` / `454`.
- Results are written to `results/<run_id>/` as JSON + CSV.
