# Fidelity And Coverage

## Blind Path

- Blind progression must be linear within an ante:
  - `Small Blind`
  - `Big Blind`
  - `Boss`
- `Boss` is mandatory before ante advancement.
- `skip_blind` may bypass the currently selectable `Small Blind` or `Big Blind`, but must not permit skipping `Boss`.
- The local `game.lua` initialization shape is a regression guard:
  - `blind_states = {Small = 'Select', Big = 'Upcoming', Boss = 'Upcoming'}`
  - `blind_choices = {Small = 'bl_small', Big = 'bl_big'}`

## Shop Lifecycle

- Shop inventory must not be visible outside `Stage_Shop`.
- Engine/runtime may keep internal shop state, but snapshots and replay artifacts must expose `shop_jokers = []` unless the current stage is actually `Stage_Shop`.
- Shop inventory should be refreshed when entering shop, not during ante initialization.

## Coverage Runner

- The first coverage runner uses the synchronous vector wrapper.
- Supported first-pass batch sizes are `5` and `10`.
- Seeds must be randomized by default and stored in the manifest.
- Every seed in a coverage batch writes:
  - replay JSON
  - behavior-log JSONL
  - optional replay HTML
- Every coverage batch writes one manifest with:
  - `session_id`
  - `started_at`
  - `finished_at`
  - `policy_id`
  - `num_envs`
  - `seeds`
  - `summary_metrics`
  - per-seed artifact paths
