# Real Client Integration — Architecture & Test Plan

_Date_: 2026-04-20 UTC  
_Trigger_: first end-to-end trajectory captured from the real Steam Balatro client via Lovely + Steamodded + BalatroBot on macOS arm64 (session `observer-20260420T223706`, 56 events, 21 snapshots, Ante 2 loss).

## Gaps exposed by the session

| # | Gap | Evidence |
|---|-----|----------|
| 1 | Two disjoint trajectory schemas (simulator vs observer) | `scripts/llm_play_game.py` writes `{step, state_text, reasoning, action, …}`; observer writes JSONL `{ts, kind, summary_after, …}`. No shared schema. |
| 2 | Polling drops fast transitions | 1 Hz observer lost `cards_played` on 2 of 5 hands (`SELECTING_HAND → HAND_PLAYED` in <1s). |
| 3 | Edge-state false positives | At `BLIND_SELECT → DRAW_TO_HAND` round boundary, `discards_left` reset 4→0 triggered a spurious `discard` event. |
| 4 | Launch ritual is lossy | Vanilla Balatro writes `save.jkr`; next modded launch crashes on `cardarea.lua:266`. Discovered via 3 iterative crashes. |
| 5 | No simulator↔real diff tool | We have both paths but no tool to assert `engine.step(seed, actions) == real_client_trajectory`. Fidelity is unmeasured across the boundary. |
| 6 | Action inference is lossy | State-diff reconstruction cannot see: card highlight/deselect, hover, reroll-preview, consumable-preview. "Thinking" actions vanish. |
| 7 | No RPC health watchdog | If BalatroBot crashes mid-session observer silently emits no events; operator won't know until inspection. |

## Design responses

### A. Canonical trajectory schema
Single schema, consumed by simulator, real-client adapter, and LLM play equally.

```
{
  "meta": {
    "source": "real-client-observer" | "balatro-native-sim" | "llm-claude-code",
    "seed", "deck", "stake", "agent_id", "captured_at"
  },
  "steps": [
    {
      "step_idx": int,
      "ts": iso-string,
      "state_before": { "state", "ante", "round", "hands_left",
                        "discards_left", "round_chips", "money",
                        "hand_cards", "hand_ids", "jokers", "consumables",
                        "blind_status" },
      "action": { "type": "play"|"discard"|"buy"|"sell"|"use_consumable"|
                          "skip_blind"|"select_blind"|"reroll"|"cash_out"|
                          "next_round"|"pack_choice"|"observe",
                  "params": { ... } },
      "state_after": { same shape as state_before },
      "info": { "raw_snapshot_path"?, "events"?, "notes"? }
    }
  ]
}
```

Script `scripts/adapt_observer_to_canonical.py` consolidates `events.jsonl + snapshots/` into this form. Simulator trajectory writer migrated incrementally.

### B. Hardened modded launcher
`scripts/launch_modded_balatro.sh` — idempotent:
1. Kill any running `love` Balatro
2. Backup all existing `save.jkr` files under each profile with timestamp suffix
3. Launch via the official `run_lovely_macos.sh` under `nohup`, log to `/tmp/balatro-lovely.log`
4. Poll `127.0.0.1:12346/health` up to 60 s
5. Exit 0 on success, 1 with diagnostic message on failure

### C. Observer upgrade
`scripts/experimental/observe_real_play.py`:
- default `--interval 0.2` (5 Hz)
- gate `hand_played` / `discard` on `prev.state ∈ {SELECTING_HAND, HAND_PLAYED, PLAY_TAROT}` AND `cur.state` same-or-progressed
- two-tick lookback for recovering lost `cards_played` when first tick shows `cards_played=[]` (compare hand at `t-2` against hand at `t+1`)

### D. Real↔Sim diff tool (P2, scaffolded now)
`scripts/diff_real_vs_sim.py` (stub): takes a canonical real trajectory, replays `actions` through `balatro_native` with the same `seed`, emits per-step diff report against `state_after`. First version only asserts initial-state equivalence after `start`; per-step diff is P2.

## Test plan

### Phase P0 — NOW (machine)
- [x] Lovely + Steamodded + BalatroBot working (Ante 2 loss captured)
- [ ] Harden launch ritual
- [ ] Upgrade observer to 5 Hz + state gate
- [ ] Ship canonical schema + observer adapter
- [ ] Stub real↔sim diff

### Phase P1 — HUMAN PLAY (you, 3–5 hours, 5–10 games)

Coverage checklist per session:

- Hands: High Card, Pair, Two Pair, Three of a Kind, Straight, Flush, Full House, Four of a Kind, Straight Flush (9 rows) — check off once each across all sessions.
- Consumables: used Tarot / Planet / Spectral at least once each.
- Shop: bought Joker, sold Joker, rerolled, bought each booster pack type (Standard / Buffoon / Arcana / Celestial / Spectral), opened each.
- Bosses: faced 3+ different boss types (The Window already done; try The Hook, The Wall, The Psychic, …).
- Progression: reached Ante 3+ in at least one run; one win + one loss.
- Blind choices: skipped small once, played small once.

Volume rationale: ~50–150 steps/game × ~10 games ≈ 1000 human-generated ground-truth actions. Enough to:
1. exercise every `action.type` branch in the observer adapter,
2. seed imitation-learning corpus for a small model,
3. give the sim-diff tool ≥10 samples per action type for fidelity regression.

### Phase P2 — DIFF & FIX

- Implement per-step real↔sim diff.
- Run on P1 corpus, collect mismatches grouped by action type and boss effect.
- Fix engine bugs surfaced (expect boss-blind scaling, joker interactions, enhancement propagation to be top offenders).
- Target: zero mismatches on first-hand outcome across corpus.

### Phase P3 — SELF-PLAY SCALE

- Headless self-play via `uvx balatrobot serve --headless --fast`, random/scripted policies.
- 500+ games, unattended.
- Grows corpus by ~50k steps. Becomes the RL-finetuning / BC dataset.

### Phase P4 — ITERATION

- Diff tool regressions become part of CI (`scripts/fidelity_regression.py`).
- Update README + CLAUDE.md each time the fidelity bar moves.

## Success criteria

- Observer loses **zero** `hand_played` details on 5 Hz (cross-check against `round.hands_played` counter).
- Launcher script: `bash scripts/launch_modded_balatro.sh` green in <30 s from any state.
- Canonical adapter: round-trips `observer-20260420T223706/events.jsonl` → canonical → re-extracted event list with 1:1 play / discard / buy / sell events.
- Real↔sim init diff: `deck + stake + seed` → identical starting hand on both sides.
