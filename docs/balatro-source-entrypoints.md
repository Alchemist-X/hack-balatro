# Balatro Source Entrypoints

## Scope

- Local package source:
  - `vendor/balatro/steam-local/original/Balatro.love`
- Verified package hash:
  - `48c7a0791796a969d2cd0891ebdc9922b2988eb5aaad8ad7a72775a02772e24e`
- Extracted Lua mirror:
  - `vendor/balatro/steam-local/extracted/`

## Boot Chain

1. `main.lua`
   - Requires engine modules, gameplay modules, and callback tables.
   - Seeds Lua RNG with `math.randomseed(G.SEED)`.
   - Hooks the Love2D loop:
     - `love.load() -> G:start_up()`
     - `love.update(dt) -> G:update(dt)`
     - input events -> `G.CONTROLLER`
2. `game.lua`
   - `Game:init()` binds the global `G` singleton and initializes globals.
   - `Game:start_up()` loads settings, save manager, HTTP manager, localization, prototypes, shared sprites, and the event manager.
3. `globals.lua`
   - Defines the authoritative numeric `G.STATES` and `G.STAGES` enums.

## Run-State Machine

The run loop is not hidden in UI callbacks. The core state dispatch lives in `Game:update()` and selects one `update_*` function based on `G.STATE`.

Key run states from `globals.lua`:

- `SELECTING_HAND = 1`
- `HAND_PLAYED = 2`
- `DRAW_TO_HAND = 3`
- `GAME_OVER = 4`
- `SHOP = 5`
- `PLAY_TAROT = 6`
- `BLIND_SELECT = 7`
- `ROUND_EVAL = 8`
- `NEW_ROUND = 19`

Primary trajectory path in vanilla play:

1. `BLIND_SELECT`
2. `NEW_ROUND`
3. `DRAW_TO_HAND`
4. `SELECTING_HAND`
5. `HAND_PLAYED`
6. `DRAW_TO_HAND` or `NEW_ROUND`
7. `ROUND_EVAL`
8. `SHOP`
9. `BLIND_SELECT`

## Action Entrypoints

### Start Run

- UI callback:
  - `functions/button_callbacks.lua`
  - `G.FUNCS.start_run`
- Runtime entry:
  - `game.lua`
  - `Game:start_run(args)`

Important behavior:

- `prep_stage(G.STAGES.RUN, G.STATES.BLIND_SELECT)`
- loads save state if present
- resets `STATE_COMPLETE`
- sets blind background using the current blind
- initializes deck/back/run modifiers

### Blind Selection

- UI callback:
  - `G.FUNCS.select_blind`
- Skip callback:
  - `G.FUNCS.skip_blind`
- Round transition:
  - `new_round()`

Important behavior:

- selecting a blind writes `G.GAME.round_resets.blind`
- marks the current blind slot as `Current`
- `skip_blind` mutates:
  - `blind_states`
  - `blind_on_deck`
  - tag effects
  - joker `{skip_blind = true}` triggers
- both paths call `save_run()` around the transition boundary

### Draw / Select / Play / Discard

- draw entry:
  - `Game:update_draw_to_hand`
  - `G.FUNCS.draw_from_deck_to_hand`
- play callback:
  - `G.FUNCS.play_cards_from_highlighted`
- discard callback:
  - `G.FUNCS.discard_cards_from_highlighted`
- action gating:
  - `G.FUNCS.can_play`
  - `G.FUNCS.can_discard`

Important behavior:

- `can_play` blocks if:
  - no highlighted cards
  - more than 5 highlighted cards
  - `G.GAME.blind.block_play`
- `can_discard` blocks if:
  - no highlighted cards
  - no discards left
- `play_cards_from_highlighted`:
  - sorts highlighted cards by x-position
  - moves them from `G.hand` to `G.play`
  - decrements hands
  - lets the blind react via `press_play()`
  - schedules `evaluate_play()`
  - schedules move from play area to discard

## Scoring Pipeline

The scoring chain is centered in `functions/state_events.lua`, not in `game.lua`.

### Hand Classification

- `G.FUNCS.get_poker_hand_info(G.play.cards)`
- resolves:
  - scoring hand type
  - display text
  - scoring card subset

### Main Evaluation

- `G.FUNCS.evaluate_play`

Observed order:

1. classify the played hand
2. update hand usage / visibility counters
3. extend scoring hand with pure bonus cards such as `Stone Card`
4. highlight scoring cards
5. reject or debuff via `G.GAME.blind:debuff_hand(...)`
6. set base `mult` and `hand_chips`
7. run joker `before = true` effects
8. let blind mutate hand totals via `modify_hand(...)`
9. score each scoring card
   - per-card effects
   - seal repetitions
   - joker repetitions
   - chip / mult / xmult / dollar / edition side effects
10. score held-in-hand effects on remaining hand cards
11. score joker-main and joker-on-joker effects
12. apply deck/back final scoring effect
13. resolve destruction and shatter side effects
14. ease total chips into `G.GAME.chips`
15. run joker `after = true` effects

This order is the critical source of truth for trajectory fidelity. End-score parity alone is not enough.

## Round / Shop Flow

### End Of Blind

- `Game:update_hand_played`
  - if blind target reached or no hands left:
    - `G.STATE = G.STATES.NEW_ROUND`
  - else:
    - `G.STATE = G.STATES.DRAW_TO_HAND`

### End Of Round

- `Game:update_new_round -> end_round()`
- `end_round()` handles:
  - game-over decision
  - end-of-round joker effects
  - boss-win and ante-up logic
  - hand/discard cleanup
  - defeated blind state mutation
  - transition to `ROUND_EVAL`

### Round Evaluation

- `Game:update_round_eval`
- `G.FUNCS.evaluate_round()`

Important behavior:

- awards blind reward dollars
- awards extra money from hands/discards/jokers/tags
- defeats the blind
- prepares the cash-out UI

### Cash Out And Shop

- `G.FUNCS.cash_out`
- `Game:update_shop`
- `G.FUNCS.reroll_shop`
- `G.FUNCS.buy_from_shop`
- `G.FUNCS.sell_card`
- `G.FUNCS.use_card`

Important behavior:

- `cash_out`:
  - moves from `ROUND_EVAL` to `SHOP`
  - shuffles deck with `cashout..ante`
  - pays out `current_round.dollars`
  - resets blind ladder when boss is defeated
- `update_shop`:
  - materializes jokers, vouchers, boosters
  - applies tag hooks such as `shop_start`, `voucher_add`, `shop_final_pass`
  - persists the state with `save_run()`
- `reroll_shop`:
  - decrements free rerolls
  - recalculates reroll cost
  - destroys and recreates shop jokers
  - triggers joker `{reroll_shop = true}`

## Snapshot Boundary Already Present In Vanilla

`save_run()` in `functions/misc_functions.lua` already serializes a useful snapshot boundary:

- `cardAreas`
- `tags`
- `GAME`
- `STATE`
- `ACTION`
- `BLIND`
- `BACK`
- `VERSION`

This is a strong candidate for recorder snapshots, but it is not enough by itself for activation-order debugging.

## Implications For Trajectory Recorder

The first recorder should hook both snapshots and event edges.

Recommended capture points:

- before and after:
  - `select_blind`
  - `skip_blind`
  - `play_cards_from_highlighted`
  - `discard_cards_from_highlighted`
  - `buy_from_shop`
  - `sell_card`
  - `use_card`
  - `reroll_shop`
  - `cash_out`
- state transitions:
  - every `G.STATE` change in `Game:update_*`
- scoring detail:
  - within `evaluate_play`
  - joker `before`
  - blind `modify_hand`
  - per-card scoring loop
  - held-in-hand loop
  - joker-main loop
  - joker `after`
- persistence checkpoints:
  - every `save_run()`

Recorder design consequence:

- `save_run()` snapshots can anchor replay comparison.
- event-chain instrumentation is still required to explain any mismatch in:
  - trigger order
  - retriggers
  - blind side effects
  - RNG consumption order
