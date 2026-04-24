# Canonical Trajectory Schema

Authoritative spec for every trajectory producer in hack-balatro
(real-client observer, `llm_play_game.py`, `sim_repl`, future online RL).

All producers write the **same JSON shape**. Downstream tools
(training, eval, sim-vs-real diff) should not need to branch on source.

## File layout

```json
{
  "meta":  { ... CanonicalMeta ... },
  "steps": [ { ... CanonicalStep ... }, ... ]
}
```

## CanonicalMeta

| Field         | Type              | Req | Notes                                                       |
|---------------|-------------------|-----|-------------------------------------------------------------|
| `source`      | string            | yes | `"real-client-observer" \| "balatro-native-sim" \| "llm-claude-code" \| ...` |
| `captured_at` | ISO-8601 string   | yes | When the trajectory file was written.                        |
| `seed`        | string \| null    | no  | Game seed if known.                                          |
| `deck`        | string \| null    | no  | e.g. `"RED"`.                                                |
| `stake`       | string \| null    | no  | e.g. `"WHITE"`.                                              |
| `agent_id`    | string \| null    | no  | Producer identity (e.g. `"human"`, `"claude_code"`).         |
| `extra`       | object            | no  | Free-form — source-specific metadata.                        |

## CanonicalStep

| Field              | Type                              | Req | Notes                                                                    |
|--------------------|-----------------------------------|-----|--------------------------------------------------------------------------|
| `step_idx`         | int                               | yes | 0-indexed step counter.                                                  |
| `ts`               | string                            | yes | ISO-8601 if known, else `""`.                                            |
| `state_before`     | `CanonicalState`                  | yes | Snapshot before the action.                                              |
| `legal_actions`    | list[str] \| list[int] \| null    | rec | Null if the producer didn't capture them (e.g. legacy observer).         |
| `requested_action` | string \| null                    | rec | Raw agent output (e.g. `"play"`, `"buy_shop_item_0"`).                   |
| `parsed_action`    | int \| null                       | rec | `action_idx` after parsing; null if unavailable.                         |
| `executed_action`  | int \| null                       | rec | What actually ran (may differ from `parsed_action` on fallback).         |
| `fallback_used`    | `FallbackInfo`                    | yes | `{"used": bool, "reason": str\|null}`. Default `{false, null}`.          |
| `action`           | `CanonicalAction` \| null         | rec | Structured summary (type + params). Human-readable, may duplicate above. |
| `state_after`      | `CanonicalState`                  | yes | Snapshot after the action.                                               |
| `reward`           | number \| null                    | rec | Scalar reward for this step (usually `round_chips_after - round_chips_before`). |
| `terminal`         | bool                              | yes | True on the step that ends the episode (GAME_OVER or final step).        |
| `info`             | object                            | no  | Free-form per-step extension (e.g. `{"reconstructed": true}`).           |

Columns:
- **Req** = always serialized.
- **rec** = strongly recommended but allowed to be `null` when the
  producer genuinely does not have the data (e.g. real-client observer
  cannot recover legal actions post-hoc). Downstream consumers must
  tolerate `null`.

## CanonicalState

Compact state projection. Anything big (full game snapshot) belongs in
`info.raw_snapshot`.

| Field               | Type                | Notes                                |
|---------------------|---------------------|--------------------------------------|
| `state`             | str \| null         | Game state (`SHOP`, `SELECTING_HAND`, ...) |
| `ante`              | int \| null         |                                      |
| `round`             | int \| null         |                                      |
| `hands_left`        | int \| null         | plays remaining in current round     |
| `discards_left`     | int \| null         |                                      |
| `round_chips`       | number \| null      | score accumulated this round         |
| `money`             | number \| null      |                                      |
| `hand_cards`        | list[str]           | Short labels (e.g. `"5D"`, `"AC"`).  |
| `hand_ids`          | list[int]           | Stable card ids when available.      |
| `jokers_count`      | int \| null         |                                      |
| `consumables_count` | int \| null         |                                      |
| `blind_small`       | str \| null         |                                      |
| `blind_big`         | str \| null         |                                      |
| `blind_boss`        | str \| null         |                                      |
| `won`               | bool \| null        |                                      |

## CanonicalAction

```json
{ "type": "<action_type>", "params": { ... } }
```

Allowed `type` values: see `ACTION_TYPES` in `env/canonical_trajectory.py`.

## FallbackInfo

```json
{ "used": bool, "reason": str | null }
```

## Minimum example (one step)

```json
{
  "meta": {
    "source": "balatro-native-sim",
    "captured_at": "2026-04-24T09:00:00Z",
    "seed": "42",
    "deck": "RED",
    "stake": "WHITE",
    "agent_id": "llm-claude-code",
    "extra": {}
  },
  "steps": [
    {
      "step_idx": 0,
      "ts": "2026-04-24T09:00:01Z",
      "state_before": { "state": "SHOP", "money": 4, "hand_cards": [], "hand_ids": [], "jokers_count": 0, "consumables_count": 0 },
      "legal_actions": ["buy_shop_item_0", "next_round"],
      "requested_action": "next_round",
      "parsed_action": 1,
      "executed_action": 1,
      "fallback_used": { "used": false, "reason": null },
      "action": { "type": "next_round", "params": {} },
      "state_after": { "state": "BLIND_SELECT", "money": 4, "hand_cards": [], "hand_ids": [], "jokers_count": 0, "consumables_count": 0 },
      "reward": 0.0,
      "terminal": false,
      "info": {}
    }
  ]
}
```

## Backward compatibility

The schema was expanded on 2026-04-24 to add
`legal_actions`, `requested_action`, `parsed_action`, `executed_action`,
`fallback_used`, `reward`, `terminal`.

Old canonical JSONs lacking these fields still load: missing fields
deserialize to safe defaults (`None` / empty list / `False`).

## Marking reconstructed data

When a producer retrofits a trajectory (e.g. the observer adapter
synthesizes steps from an event stream that lacked legal-action capture),
it MUST set `info.reconstructed = true` and, if applicable,
`info.legal_actions_known = false`. Downstream tools can then tell
real-time captures from retrofits.
