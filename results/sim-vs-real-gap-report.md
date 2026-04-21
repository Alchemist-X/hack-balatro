# Simulator vs Real-Client — Schema Alignment Report

- **real source**: `results/real-client-trajectories/observer-20260420T223706/snapshots/tick-000010.json`
- **sim build**: seed=42 stake=1
- **total fields compared**: 148

## Summary

| Status | Count | % |
|---|---:|---:|
| `aligned` | 94 | 63.5% |
| `value_mismatch` | 34 | 23.0% |
| `missing_in_sim` | 20 | 13.5% |
| `missing_in_real` | 0 | 0.0% |
| `shape_mismatch` | 0 | 0.0% |

## Value mismatches (same field, different value) — 34

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
| `hands.Flush.played` | `3` | `0` |
| `hands.Flush.played_this_round` | `2` | `0` |
| `hands.Full House.played` | `1` | `0` |
| `hands.High Card.played` | `1` | `0` |
| `hands.Pair.played` | `2` | `0` |
| `hands.Pair.played_this_round` | `2` | `0` |
| `hands.Straight.played` | `1` | `0` |
| `hands.Two Pair.played` | `1` | `0` |
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

## Missing in simulator (real has it, sim doesn't) — 20

| Path | Real | Sim |
|---|---|---|
| `blinds.big.tag_effect` | `'添加一张优惠券 到下一个商店'` | `None` |
| `blinds.big.tag_name` | `'Voucher Tag'` | `None` |
| `blinds.boss.tag_effect` | `''` | `None` |
| `blinds.boss.tag_name` | `''` | `None` |
| `blinds.small.tag_effect` | `'商店里的下一张 基础版本小丑牌 将会免费且变为负片'` | `None` |
| `blinds.small.tag_name` | `'Negative Tag'` | `None` |
| `hands.Five of a Kind.example` | `[['S_A', True], ['H_A', True], ['H_A', True], ['C_A', True]…` | `None` |
| `hands.Flush.example` | `[['H_A', True], ['H_K', True], ['H_T', True], ['H_5', True]…` | `None` |
| `hands.Flush Five.example` | `[['S_A', True], ['S_A', True], ['S_A', True], ['S_A', True]…` | `None` |
| `hands.Flush House.example` | `[['D_7', True], ['D_7', True], ['D_7', True], ['D_4', True]…` | `None` |
| `hands.Four of a Kind.example` | `[['S_J', True], ['H_J', True], ['C_J', True], ['D_J', True]…` | `None` |
| `hands.Full House.example` | `[['H_K', True], ['C_K', True], ['D_K', True], ['S_2', True]…` | `None` |
| `hands.High Card.example` | `[['S_A', True], ['D_Q', False], ['D_9', False], ['C_4', Fal…` | `None` |
| `hands.Pair.example` | `[['S_K', False], ['S_9', True], ['D_9', True], ['H_6', Fals…` | `None` |
| `hands.Straight.example` | `[['D_J', True], ['C_T', True], ['C_9', True], ['S_8', True]…` | `None` |
| `hands.Straight Flush.example` | `[['S_Q', True], ['S_J', True], ['S_T', True], ['S_9', True]…` | `None` |
| `hands.Three of a Kind.example` | `[['S_T', True], ['C_T', True], ['D_T', True], ['H_6', False…` | `None` |
| `hands.Two Pair.example` | `[['H_A', True], ['D_A', True], ['C_Q', False], ['H_4', True…` | `None` |
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
- `cards.highlighted_limit`
- `cards.limit`
- `consumables.highlighted_limit`
- `consumables.limit`
- `deck`
- `hand.cards`
- `hand.count`
- `hand.highlighted_limit`
- `hand.limit`
- `hands.Five of a Kind.chips`
- `hands.Five of a Kind.level`
- `hands.Five of a Kind.mult`
- `hands.Five of a Kind.order`
- `hands.Five of a Kind.played`
- `hands.Five of a Kind.played_this_round`
- `hands.Flush.chips`
- `hands.Flush.level`
- `hands.Flush.mult`
- `hands.Flush.order`
- `hands.Flush Five.chips`
- `hands.Flush Five.level`
- `hands.Flush Five.mult`
- `hands.Flush Five.order`
- `hands.Flush Five.played`
- `hands.Flush Five.played_this_round`
- `hands.Flush House.chips`
- `hands.Flush House.level`
- `hands.Flush House.mult`
- `hands.Flush House.order`
- `hands.Flush House.played`
- `hands.Flush House.played_this_round`
- … and 54 more
