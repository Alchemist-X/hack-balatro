# Legacy Gym/PPO interface

_Retired 2026-04-24._ See [`../../todo/20260424_interface_consolidation_plan.md`](../../todo/20260424_interface_consolidation_plan.md)
for the decision that moved this code here.

## Why retired

The PPO research direction failed after **6 experiments / 930K total steps**.
Vanilla PPO could not cold-start on Balatro's toggle-based action space; 500K
steps of PPO cleared **zero** blinds, while zero-training Claude/LLM agents
reached the Ante 2 boss. Root cause (documented in `CLAUDE.md` Diagnosis-First
Debugging section): the agent spent ~98% of steps cycling `select_card_*`
toggles, which neither entropy-coef tuning nor reward shaping could break.

The research pivoted to the LLM-facing direct path:

- **Primary interface**: `balatro_native.Engine` + `env.state_serializer` +
  `env.canonical_trajectory`.
- **Collectors**: `scripts/llm_play_game.py`, `scripts/sim_repl.py`,
  `scripts/adapt_observer_to_canonical.py`.

This `env/legacy/` tree (plus `legacy/training/`, `scripts/legacy/`,
`agents/legacy/`, `tests/legacy/`, `configs/legacy/`) is **frozen, not
deleted** — git history is preserved so an RL baseline can be resurrected
in the future with a clean slate.

## What's frozen here

Files in this directory as of the move commit:

- `balatro_gym_wrapper.py` — `BalatroEnv`, `ParallelBalatroEnvs`, `make_vec_env`
- `state_encoder.py` — 576-dim observation encoder (`encode_pylatro_state`)
- `action_space.py` — 86-dim discrete action labels / offsets

Sibling legacy trees:

- `legacy/training/` — PPO trainer, GAE rollout buffer, BC pipeline, curriculum
- `scripts/legacy/` — `train_smoke`, `train_ppo`, `train_bc`,
  `train_full_pipeline`, `eval_run`, `repro`, `run_simple_rule_coverage`,
  `collect_greedy_trajectories`, `test_greedy`, `doctor`
- `agents/legacy/` — `greedy_agent`, `simple_rule_agent`, `ppo_agent`
- `tests/legacy/` — `test_action_mask_and_fallback`, `test_fidelity_and_coverage`,
  `test_greedy_offsets`, `test_state_encoder`, `test_rollout_smoke`
- `configs/legacy/` — `repro.yaml` (PPO hyperparams)

## Revival recipe

If a future RL baseline is wanted, do **not** just unmove the files. The six
issues surfaced in the 2026-04-24 friend-review (captured in chat that day,
not committed) must be addressed first:

1. **`git log env/legacy/`** to locate the last known-good commit and review
   the diff against the move commit.
2. **Fix the six bugs** from the review:
   - Action-space labels mismatched observation offsets
   - Default action-mask incorrect on reset
   - `info` contract unstable across phase transitions
   - Mock backend silently diverges from `balatro_native`
   - Observation coverage missing several snapshot fields
   - Dead config knobs (`curriculum.*`, `behavior_clone.*`) still present
3. **Rewire to the current `Snapshot` schema**. The schema has evolved and now
   carries fields the encoder never saw: `seed_str`, `deck_name`, `stake_name`,
   `hand_stats`, `small_tag`, `big_tag`, `boss_tag`, `deck_limit`,
   `play_card_limit`, `pack_limit`, `pack_highlighted_limit`. Any resurrection
   must regenerate `OBS_DIM` with these included, or explicitly justify
   dropping them.
4. **Pick a modern RL baseline** — vanilla PPO failed; consider IMPALA,
   Muesli, or sample-efficient model-based methods. Don't re-run the same
   PPO config "with more steps".

## Pointers

- Decision log: [`todo/20260424_interface_consolidation_plan.md`](../../todo/20260424_interface_consolidation_plan.md)
- Post-mortem anchor: `CLAUDE.md` → _Diagnosis-First Debugging — 2026-04-11 PPO 失败案例_
- Friend-review feedback: captured in chat on 2026-04-24 (not committed to
  repo — the interface-consolidation plan was the committed artifact).
