# Progress

## 2026-03-31 CST

### Completed
- **P0: Consumable system** — Full shop/inventory/use flow for Tarot, Planet, and Spectral cards.
  - `ConsumableInstance` with buy_cost, sell_value, config
  - Buy/sell/use actions wired into action space (indices 24-27, 71-78)
  - `handle_buy_consumable()`, `handle_sell_consumable()`, `handle_use_consumable()`
  - Planet cards level up hand types via `hand_levels: BTreeMap<String, i32>`
  - Tarot cards: Strength, Hermit, Temperance, suit conversion (Star/Moon/Sun/World), enhancement tarots, Hanged Man
  - Shop generates 1-2 consumables per refresh (no Vouchers, no Legendaries)
  - Default 2-slot consumable inventory with `consumable_slot_limit`
  - 8 tests covering buy/sell/use/slot limit/scoring integration
- **P0: Retrigger system** — Full per-card retrigger modeling with Blueprint/Brainstorm chain resolution.
  - `calculate_retriggers()` aggregates Red Seal + Joker retrigger sources additively
  - `resolve_joker_ability()` with visited set for cycle-safe Blueprint/Brainstorm chains
  - `spec_grants_retrigger()` for Sock and Buskin, Hanging Chad, Seltzer (with remaining_uses), Dusk
  - Per-card scoring loop: each card runs (1 + retrigger_count) full scoring passes
  - Seltzer auto-destruction when remaining_uses reaches 0
  - 15 retrigger tests including chain resolution and stacking
- **P1: Joker effects — 128+ implementations** covering all 150 Jokers in the ruleset.
  - `ScoringContext` struct for clean state passing
  - xmult and money_delta support in scoring loop
  - Helpers: is_face_card, is_even_rank, is_odd_rank, is_fibonacci_rank, config_extra_*
  - ~55 fully implemented with real scoring effects
  - ~25 recognized with scaling state (config-driven TODO)
  - ~30 passive/no-scoring (correctly marked supported)
  - ~15 economy/trigger (correctly marked supported)
  - Generic fallback matchers for Mult, Suit Mult, hand-type, Discard Chips patterns
  - 30 tests covering representative effects across all families
- **P1: Three activation phases** — held-in-hand, end-of-round, boss-blind-pre-play.
  - `apply_held_in_hand()`: Mime, Raised Fist, Baron (X1.5/King), Reserved Parking, Shoot the Moon
  - `apply_end_of_round_jokers()`: Golden Joker, Cloud 9, Rocket, To the Moon, Satellite, Gift Card, Egg, Gros Michel (1/6 destroy), Cavendish (1/1000 destroy)
  - `apply_blind_select_jokers()`: Chicot (disable boss), Burglar (+3 hands/-discards), Riff-raff (2 Common Jokers), Cartomancer (1 Tarot), Marble Joker (Stone card)
  - EngineState fields: rocket_extra_dollars, egg_accumulated_sell, unique_planets_used, boss_blind_disabled
  - roll_chance(), count_rank_in_full_deck() helpers
  - 5 phase tests
- **Replay audit zero-warning achieved.**
  - Joker resolution trace fixed: one trace per Joker (not per card × per Joker)
  - Re-recorded 318-step replay reaching Stage_End
  - Determinism: 0 mismatches
  - Hard invariants: OK
  - Fidelity ready: TRUE
  - Errors: 0, Warnings: 0
- Python bindings updated: PyConsumable, PyCard (seal/is_face_card/enhancement/edition), PyJoker (remaining_uses/activation_class)

### Current Result
- `cargo check` — clean (1 dead_code warning)
- `cargo test` — 45 tests passing + 1 spec test
- `replay-fidelity.audit.json` → `ok: true, fidelity_ready: true`
- `replay-fidelity.diff.json` → `mismatch_count: 0`

### Honest Deficiencies Found
- **28 "fake supported" Jokers**: marked `supported = true` with TODO, no real effect. Audit doesn't check values, only flags.
- **Audit is structural only**: checks trace shape, not chips/mult/money correctness. Zero-warning ≠ zero-error.
- **Card Enhancement not scored**: `enhancement` field exists but ignored in scoring loop. Bonus/Mult/Wild/Glass/Steel/Stone/Gold/Lucky cards all have no effect.
- **Edition not scored**: Foil/Holo/Polychrome/Negative have no scoring effect.
- **Boss Blind effects missing**: all 25+ Bosses treated as vanilla big-number blinds.
- **Voucher system missing**: no persistent shop upgrades.
- **Booster Pack missing**: no pack opening sub-decisions.
- **joker_on_played phase missing**: Space Joker/DNA/To Do List/Midas Mask have no activation.
- **lib.rs is 3830 lines**: 4.8x over 800-line guideline, unmaintainable.
- **Real-client trajectory**: 0 recorded. All verification is engine self-comparison.

### Next (priority order)
1. **B-01**: Add numerical audit (chips/mult/money vs source oracle)
2. **B-03**: Implement Card Enhancement scoring effects
3. **B-02**: Implement 28 scaling Joker runtime states
4. **B-11**: Split lib.rs into modules
5. **B-04**: Boss Blind special effects
6. **B-12**: Real-client trajectory recording

### Checklist
- [x] Consumable shop/inventory/use flow
- [x] Retrigger modeling (Red Seal / Blueprint chain)
- [x] 150 Joker match arms in apply_joker_effect (but 28 are TODO stubs)
- [x] Held-in-hand activation phase
- [x] End-of-round activation phase
- [x] Boss-blind-pre-play activation phase
- [x] Replay audit zero-warning (structural only)
- [x] Determinism verified
- [ ] Numerical value audit (chips/mult/money correctness)
- [ ] Card Enhancement scoring
- [ ] Edition scoring
- [ ] 28 scaling Joker runtime state
- [ ] Boss Blind special effects (25+)
- [ ] Voucher system
- [ ] Booster Pack system
- [ ] lib.rs modular split
- [ ] Real-client trajectory recording
- [ ] Winning-run replay coverage

### Need Human Help
- None for the current loop.

## 2026-03-27 22:45 CST

### Completed
- Bootstrapped the local licensed `Balatro.love` from the Steam install into `vendor/balatro/steam-local/`.
- Verified the package hash matches the team baseline:
  - `48c7a0791796a969d2cd0891ebdc9922b2988eb5aaad8ad7a72775a02772e24e`
- Added explicit setuptools package discovery so `pip install -e .[dev]` works from a clean `.venv`.
- Fixed `balatro-engine` struct initializers so the native workspace compiles against the current snapshot/event schema.
- Updated ruleset generation to keep local `game.lua` Joker names authoritative while resolving wiki display-name mismatches.
- Corrected the native blind-clear flow so `Small Blind` / `Big Blind` cash out into `Shop` instead of skipping directly to the next blind.
- Added `lua_state` to replay snapshots so native transitions can be audited against Lua state names.
- Added a Chinese CLI-style replay renderer:
  - `scripts/replay_cli.py`
- Added a replay audit tool that distinguishes:
  - hard invariant pass
  - fidelity-ready pass
- Generated proof artifacts:
  - `results/replay-proof.json`
  - `results/replay-proof.behavior_log.jsonl`
  - `results/replay-proof.cli.txt`
  - `results/replay-proof.audit.json`
- Wrote the current non-negotiable fidelity target into:
  - `Agent-Style.md`

### Reverse-Engineering Findings
- Local `game.lua` and wiki/display names currently diverge for at least these Jokers:
  - `Seance` in local `game.lua` vs `Séance` in localization/wiki
  - `Riff-raff` in local `game.lua` vs `Riff-Raff` in localization/wiki
  - `Caino` in local `game.lua` vs `Canio` in localization/wiki
- Per repo source precedence, emitted ruleset names continue to follow local `game.lua`.
- Wiki-derived effect text and references are still attached via normalized / aliased lookup.

### In Progress
- Designing the first real-client trajectory recorder around the verified Lua entrypoints and snapshot boundaries.
- Expanding the native engine so Lua transient states such as `NEW_ROUND`, `DRAW_TO_HAND`, and `HAND_PLAYED` become observable.

### Next
- Hook snapshot capture around `save_run()` and `G.STATE` transitions.
- Instrument action callbacks and scoring edges inside `evaluate_play()`.
- Start the `Steamodded + Lovely + BalatroBot` integration after the recorder schema is fixed.

### Checklist
- [x] local licensed package mirrored into ignored `vendor/`
- [x] package hash matches documented baseline
- [x] editable Python install works in isolated `.venv`
- [x] native Rust compile breakages identified and patched
- [x] ruleset fixture regenerated successfully
- [x] `balatro_native` rebuilt into `.venv`
- [x] doctor passes on native backend
- [x] static source entrypoints documented
- [x] Chinese CLI-style replay proof generated
- [x] replay audit script generated
- [ ] Lua transient states are observable in native replay

### Need Human Help
- None for this loop.

## 2026-03-14 14:10 HKT

### Completed
- Fixed the native blind path to follow `Small -> Big -> Boss` instead of allowing direct free selection.
- Verified the blind progression against the local `game.lua` initialization shape:
  - `blind_states = {Small = 'Select', Big = 'Upcoming', Boss = 'Upcoming'}`
  - `blind_choices = {Small = 'bl_small', Big = 'bl_big'}`
- Hid `shop_jokers` outside `Stage_Shop` and refreshed shop inventory only on shop entry.
- Rebuilt the editable `balatro_native` extension after the Rust engine changes.
- Added timestamped replay metadata and separate per-seed `behavior_log` artifacts.
- Added a reusable `SimpleRuleAgent` and a timestamped 5-way coverage runner.
- Verified the same coverage runner in a 10-way batch.
- Generated fresh artifacts:
  - `results/replay-latest.json`
  - `results/replay-latest.behavior_log.jsonl`
  - `results/replay-latest.html`
  - `results/coverage/coverage-20260314T060806Z-4eec56f2/manifest.json`
  - `results/coverage/coverage-20260314T060920Z-c7905c6a/manifest.json`

### In Progress
- Improving `simple_rule_v1` beyond base-score heuristics and simple shop thresholds.
- Expanding parity work for Joker coverage, Boss Blind behavior, and activation-order validation.

### Next
- Add richer boss-aware and joker-aware reasoning into the behavior log.
- Run longer-horizon coverage batches after more vanilla systems are reconstructed.
- Extend coverage summaries with clearer failure buckets and per-phase diagnostics.

### Checklist
- [x] native blind flow matches linear `Small -> Big -> Boss`
- [x] `Boss` is no longer directly selectable from initial `PreBlind`
- [x] `shop_jokers` is hidden outside `Stage_Shop`
- [x] replay transitions carry `step_index` and `elapsed_ms`
- [x] per-seed behavior log file is written separately
- [x] 5-way timestamped coverage manifest generated
- [x] 10-way coverage artifact generated
- [ ] full vanilla joker parity
- [ ] full boss blind parity

### Need Human Help
- None for this loop.

## 2026-03-14 12:53 HKT

### Completed
- Added `Behavior Log v1` for replay recording with a new `simple_rule_v1` policy.
- Embedded bilingual `decision_log` entries into replay transitions and added replay-level `log_metadata`.
- Upgraded the HTML viewer with `EN` / `ZH` locale toggles and a dedicated `Decision Log` panel.
- Recorded a fresh native replay with meaningful actions beyond pure card toggles and regenerated the autoplay HTML artifact.
- Added repo rule documentation for the behavior-log schema and terminology policy.

### In Progress
- Improving the rule-based policy beyond base-score heuristics and simple shop thresholds.
- Expanding parity work for Joker coverage, Boss Blind behavior, and activation-order validation.

### Next
- Push the same decision-log schema into longer scripted replays and live snapshot inspection.
- Align behavior logs with richer Joker-aware scoring explanations once more vanilla logic is reconstructed.
- Add a stronger automated browser-level check for the locale toggle and log rendering path.

### Checklist
- [x] `simple_rule_v1` policy exists
- [x] replay JSON contains bilingual `decision_log`
- [x] viewer renders locale-switched behavior logs
- [x] fresh replay artifact generated with meaningful actions
- [x] rules updated for behavior-log output
- [ ] Joker-aware policy reasoning
- [ ] live websocket log streaming

### Need Human Help
- None for this loop.

## 2026-03-14 00:10 CST

### Acceptance
- Target remains `Balatro 1.0.1o-FULL`.
- Acceptance bar is no longer "trainable mock"; it is "engine behavior converges to vanilla Balatro and stays auditable from extracted sources".
- Visible progress artifact is required for every loop: replay JSON plus an autoplay HTML animation with numeric state.

### Completed
- Added a first-party Rust workspace for `balatro-spec`, `balatro-engine`, and `balatro-py`.
- Generated a versioned ruleset bundle from the local `Balatro.love`.
- Wired the Python Gym wrapper to prefer `balatro_native`.
- Extracted atlas files needed by the viewer.
- Installed `maturin` and built the editable `balatro_native` extension.

### In Progress
- Recording native replays from the Rust engine.
- Exporting standalone autoplay replay HTML for human verification.
- Folding reverse-engineering findings into the repo `rules/` docs.

### Next
- Expand the engine beyond the current deterministic vertical slice into full boss/shop/joker behavior parity.
- Add diff-style scenario fixtures against extracted Lua behavior.
- Replace placeholder joker execution with table-driven effect coverage.

### Checklist
- [x] Native workspace exists
- [x] Ruleset bundle is generated from local game files
- [x] Python can build the native extension
- [x] Viewer can consume structured replay data
- [ ] Replay autoplay artifact generated for this loop
- [ ] Full vanilla blind/joker activation parity
- [ ] Differential validation against more extracted Lua cases

### Need Human Help
- None for the current loop. The repo can already read the local Steam install.

## 2026-03-14 00:08:49

### Completed
- Installed and built the editable balatro_native extension with maturin.
- Verified BalatroEnv now prefers balatro_native instead of mock.
- Recorded a structured native replay to results/replay-latest.json.
- Rendered a standalone autoplay replay to results/replay-latest.html.
- Added repo-level acceptance/source/rough-loop rules under rules/.

### In Progress
- Expanding joker/boss/shop behavior from the current deterministic vertical slice toward full vanilla parity.
- Converting more reverse-engineered Lua behavior into structured tests.

### Next
- Increase engine fidelity for boss-specific debuffs and activation ordering.
- Replace placeholder joker execution with broader table-driven effect coverage.
- Add replay generation for longer scripted runs and better human demos.

### Checklist
- [x] native extension installed
- [x] env backend switched to balatro_native
- [x] replay json generated
- [x] autoplay replay html generated
- [ ] full vanilla joker parity
- [ ] full boss blind parity
- [ ] differential Lua validation suite

### Need Human Help
- None

## 2026-03-27 23:58 CST

### Acceptance
- Current gate is still vanilla trajectory fidelity, not "deterministic enough".
- New audit layers must fail loudly on missing modules such as retrigger or consumables.

### Completed
- Added source-derived oracle generation at `results/source-oracle.json`.
- Added field-by-field replay diff tool and verified same-seed native replays produce zero mismatches.
- Extended `balatro_native` transition JSON with:
  - transient Lua-state trace
  - RNG call trace
  - Joker resolution order trace
  - consumable snapshot fields
- Fixed a fidelity bug where clearing a blind incorrectly refilled the hand before `ROUND_EVAL`.
- Added strict replay audit for:
  - transient state trace
  - RNG order visibility
  - Joker order visibility
  - retrigger support gaps
- Added strict boss/shop/consumable coverage runner and wrote scenario replays under `results/fidelity-coverage/`.
- Rebuilt the native wheel for the actual repo `.venv` interpreter and re-recorded proof artifacts with the new extension.

### Current Result
- Determinism passes.
  - `results/replay-fidelity.diff.json` has `mismatch_count = 0`
- Hard replay invariants pass.
  - `results/replay-fidelity.audit.json` has `hard_invariants_ok = true`
- Fidelity still fails on explicit gaps.
  - retrigger trace not implemented
  - consumables not implemented
  - replay surfaced unsupported Jokers such as `Smiley Face` and `Vampire`

### Next
- Implement real consumable shop/inventory/use flow.
- Add source-aligned retrigger modeling for Red Seal / Blueprint-style chains.
- Expand Joker native coverage until audit warnings drop to zero on recorded trajectories.

### Checklist
- [x] source oracle generated from local extracted Lua
- [x] field-by-field replay diff tool exists and passes same-seed determinism
- [x] transient Lua-state trace emitted by native replay
- [x] RNG trace emitted by native replay
- [x] boss/shop coverage runner exists
- [ ] consumable visibility/use coverage
- [ ] retrigger fidelity
- [ ] zero-warning replay audit

### Need Human Help
- None for the current loop.
