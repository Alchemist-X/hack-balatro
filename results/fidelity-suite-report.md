# Fidelity Suite Report

## Artifacts

- Source oracle: `results/source-oracle.json`
- Deterministic replay A: `results/replay-fidelity-a.json`
- Deterministic replay B: `results/replay-fidelity-b.json`
- Field-by-field diff: `results/replay-fidelity.diff.json`
- Replay audit: `results/replay-fidelity.audit.json`
- Chinese CLI replay: `results/replay-fidelity.cli.txt`
- Coverage report: `results/fidelity-coverage.json`

## What Passed

- Determinism passed.
  - `results/replay-fidelity.diff.json` reports `mismatch_count = 0`.
- Hard structural replay invariants passed.
  - `results/replay-fidelity.audit.json` reports `hard_invariants_ok = true`.
- Stable and transient Lua-state trace coverage is now observable.
  - Seen transient states: `DRAW_TO_HAND`, `HAND_PLAYED`, `NEW_ROUND`
- RNG order is now observable at the native replay layer.
  - Seen domains include:
    - `deck.shuffle.enter_blind`
    - `deck.shuffle.cashout`
    - `cashout_shop_refresh.*`
    - `boss_blind.select`
- Boss and shop coverage is fully exercised.
  - `results/fidelity-coverage.json` marks:
    - `shop_cashout = true`
    - `shop_buy = true`
    - `shop_reroll = true`
    - `shop_sell = true`
    - `shop_next_round = true`
    - `boss_select = true`
    - `boss_enter = true`
    - `boss_defeat = true`

## What Failed

- Fidelity is not ready yet.
  - `results/replay-fidelity.audit.json` reports `fidelity_ready = false`.
- Consumables are still missing as a real module.
  - `results/fidelity-coverage.json` shows:
    - `consumable_visible = false`
    - `consumable_slot_enabled = false`
    - `consumable_use = false`
- Retrigger is still unsupported.
  - Audit warns: `joker_retrigger_not_implemented`
- Joker implementation coverage is still partial.
  - Current replay explicitly hit unsupported Jokers:
    - `Smiley Face`
    - `Vampire`

## This Round's Important Fix

- Blind clear no longer auto-refills the hand before `ROUND_EVAL`.
  - The updated CLI replay now shows only the surviving hand cards after a winning play, which is closer to vanilla `HAND_PLAYED -> NEW_ROUND -> ROUND_EVAL`.
