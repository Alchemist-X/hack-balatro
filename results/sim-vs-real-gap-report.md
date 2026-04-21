# Simulator vs Real-Client — Schema Alignment Report

- **real source**: `results/real-client-trajectories/observer-20260420T223706/snapshots/tick-000010.json`
- **sim build**: seed=42 stake=1
- **total fields compared**: 76

## Summary

| Status | Count | % |
|---|---:|---:|
| `aligned` | 28 | 36.8% |
| `value_mismatch` | 26 | 34.2% |
| `missing_in_sim` | 22 | 28.9% |
| `missing_in_real` | 0 | 0.0% |
| `shape_mismatch` | 0 | 0.0% |

## Value mismatches (same field, different value) — 26

| Path | Real | Sim |
|---|---|---|
| `ante_num` | `2` | `1` |
| `blinds.big.score` | `1200` | `450` |
| `blinds.boss.effect` | `'初始弃牌 次数为0'` | `'None'` |
| `blinds.boss.name` | `'The Water'` | `'Small Blind'` |
| `blinds.boss.score` | `1600` | `600` |
| `blinds.small.score` | `800` | `300` |
| `blinds.small.status` | `'UPCOMING'` | `'SELECT'` |
| `cards.cards` | `list[52]` | `list[0]` |
| `cards.count` | `52` | `0` |
| `consumables.cards` | `list[1]` | `list[0]` |
| `consumables.count` | `1` | `0` |
| `jokers.cards` | `list[1]` | `list[0]` |
| `jokers.count` | `1` | `0` |
| `money` | `12` | `4` |
| `packs.cards` | `list[2]` | `list[0]` |
| `packs.count` | `2` | `0` |
| `round.discards_left` | `4` | `3` |
| `round.discards_used` | `4` | `1` |
| `round.hands_played` | `4` | `0` |
| `round.reroll_cost` | `5` | `0` |
| `round_num` | `3` | `1` |
| `shop.cards` | `list[2]` | `list[0]` |
| `shop.count` | `2` | `0` |
| `state` | `'SHOP'` | `'BLIND_SELECT'` |
| `vouchers.cards` | `list[1]` | `list[0]` |
| `vouchers.count` | `1` | `0` |

## Missing in simulator (real has it, sim doesn't) — 22

| Path | Real | Sim |
|---|---|---|
| `blinds.big.tag_effect` | `'添加一张优惠券 到下一个商店'` | `None` |
| `blinds.big.tag_name` | `'Voucher Tag'` | `None` |
| `blinds.boss.tag_effect` | `''` | `None` |
| `blinds.boss.tag_name` | `''` | `None` |
| `blinds.small.tag_effect` | `'商店里的下一张 基础版本小丑牌 将会免费且变为负片'` | `None` |
| `blinds.small.tag_name` | `'Negative Tag'` | `None` |
| `cards.highlighted_limit` | `5` | `None` |
| `cards.limit` | `52` | `None` |
| `hands.Five of a Kind` | `{'played': 0, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.Flush` | `{'played': 3, 'played_this_round': 2, 'level': 1, 'order': …` | `None` |
| `hands.Flush Five` | `{'played': 0, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.Flush House` | `{'played': 0, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.Four of a Kind` | `{'played': 0, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.Full House` | `{'played': 1, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.High Card` | `{'played': 1, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.Pair` | `{'played': 2, 'played_this_round': 2, 'level': 1, 'order': …` | `None` |
| `hands.Straight` | `{'played': 1, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.Straight Flush` | `{'played': 0, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.Three of a Kind` | `{'played': 0, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `hands.Two Pair` | `{'played': 1, 'played_this_round': 0, 'level': 1, 'order': …` | `None` |
| `packs.highlighted_limit` | `1` | `None` |
| `packs.limit` | `2` | `None` |

## Missing in real (sim-only extension) — 0

_none_

## Shape mismatches (type differs) — 0

_none_

## Aligned (for reference)

- `blinds.big.effect`
- `blinds.big.name`
- `blinds.big.status`
- `blinds.big.type`
- `blinds.boss.status`
- `blinds.boss.type`
- `blinds.small.effect`
- `blinds.small.name`
- `blinds.small.type`
- `consumables.highlighted_limit`
- `consumables.limit`
- `deck`
- `hand.cards`
- `hand.count`
- `hand.highlighted_limit`
- `hand.limit`
- `jokers.highlighted_limit`
- `jokers.limit`
- `round.chips`
- `round.hands_left`
- `seed`
- `shop.highlighted_limit`
- `shop.limit`
- `stake`
- `used_vouchers`
- `vouchers.highlighted_limit`
- `vouchers.limit`
- `won`
