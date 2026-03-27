#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = ROOT / "vendor" / "balatro" / "steam-local" / "extracted"


STATE_SNIPPETS = [
    ("globals.lua", "SELECTING_HAND = 1"),
    ("globals.lua", "HAND_PLAYED = 2"),
    ("globals.lua", "DRAW_TO_HAND = 3"),
    ("globals.lua", "SHOP = 5"),
    ("globals.lua", "BLIND_SELECT = 7"),
    ("globals.lua", "ROUND_EVAL = 8"),
    ("globals.lua", "NEW_ROUND = 19"),
]

EVALUATE_PLAY_SNIPPETS = [
    ("functions/state_events.lua", "local text,disp_text,poker_hands,scoring_hand,non_loc_disp_text = G.FUNCS.get_poker_hand_info(G.play.cards)"),
    ("functions/state_events.lua", "local effects = eval_card(G.jokers.cards[i], {cardarea = G.jokers, full_hand = G.play.cards, scoring_hand = scoring_hand, scoring_name = text, poker_hands = poker_hands, before = true})"),
    ("functions/state_events.lua", "mult, hand_chips, modded = G.GAME.blind:modify_hand(G.play.cards, poker_hands, text, mult, hand_chips)"),
    ("functions/state_events.lua", "local eval = eval_card(scoring_hand[i], {repetition_only = true,cardarea = G.play, full_hand = G.play.cards, scoring_hand = scoring_hand, scoring_name = text, poker_hands = poker_hands, repetition = true})"),
    ("functions/state_events.lua", "local eval = eval_card(G.jokers.cards[j], {cardarea = G.play, full_hand = G.play.cards, scoring_hand = scoring_hand, scoring_name = text, poker_hands = poker_hands, other_card = scoring_hand[i], repetition = true})"),
    ("functions/state_events.lua", "local eval = G.jokers.cards[k]:calculate_joker({cardarea = G.play, full_hand = G.play.cards, scoring_hand = scoring_hand, scoring_name = text, poker_hands = poker_hands, other_card = scoring_hand[i], individual = true})"),
    ("functions/state_events.lua", "local eval = eval_card(G.hand.cards[i], {repetition_only = true,cardarea = G.hand, full_hand = G.play.cards, scoring_hand = scoring_hand, scoring_name = text, poker_hands = poker_hands, repetition = true, card_effects = effects})"),
    ("functions/state_events.lua", "local effects = eval_card(_card, {cardarea = G.jokers, full_hand = G.play.cards, scoring_hand = scoring_hand, scoring_name = text, poker_hands = poker_hands, joker_main = true})"),
    ("functions/state_events.lua", "local effect = v:calculate_joker{full_hand = G.play.cards, scoring_hand = scoring_hand, scoring_name = text, poker_hands = poker_hands, other_joker = _card}"),
    ("functions/state_events.lua", "local nu_chip, nu_mult = G.GAME.selected_back:trigger_effect{context = 'final_scoring_step', chips = hand_chips, mult = mult}"),
    ("functions/state_events.lua", "local effects = eval_card(G.jokers.cards[i], {cardarea = G.jokers, full_hand = G.play.cards, scoring_hand = scoring_hand, scoring_name = text, poker_hands = poker_hands, after = true})"),
]

RNG_SNIPPETS = [
    ("functions/misc_functions.lua", "function pseudoshuffle(list, seed)"),
    ("functions/misc_functions.lua", "function pseudorandom_element(_t, seed)"),
    ("functions/misc_functions.lua", "function pseudorandom(seed, min, max)"),
    ("functions/button_callbacks.lua", "G.deck:shuffle('cashout'..G.GAME.round_resets.ante)"),
    ("functions/button_callbacks.lua", "G.jokers.cards[i]:calculate_joker({reroll_shop = true})"),
]

SHOP_AND_CONSUMABLE_SNIPPETS = [
    ("functions/button_callbacks.lua", "G.FUNCS.buy_from_shop = function(e)"),
    ("functions/button_callbacks.lua", "G.FUNCS.reroll_shop = function(e)"),
    ("functions/button_callbacks.lua", "G.FUNCS.cash_out = function(e)"),
    ("functions/button_callbacks.lua", "G.FUNCS.use_card = function(e, mute, nosave)"),
    ("game.lua", "G.FUNCS.use_card({config = {ref_table = v}}, nil, true)"),
]


def locate_snippet(relative_path: str, snippet: str) -> dict[str, Any]:
    path = SOURCE_ROOT / relative_path
    text = path.read_text(encoding="utf-8")
    for line_no, line in enumerate(text.splitlines(), start=1):
        if snippet in line:
            return {
                "file": str(path.relative_to(ROOT)),
                "line": line_no,
                "snippet": snippet,
            }
    raise ValueError(f"could not locate snippet in {relative_path}: {snippet}")


def locate_many(items: list[tuple[str, str]]) -> list[dict[str, Any]]:
    return [locate_snippet(relative_path, snippet) for relative_path, snippet in items]


def build_oracle() -> dict[str, Any]:
    return {
        "source_root": str(SOURCE_ROOT.relative_to(ROOT)),
        "states": {
            "stable": [
                "BLIND_SELECT",
                "SELECTING_HAND",
                "ROUND_EVAL",
                "SHOP",
                "GAME_OVER",
            ],
            "transient": [
                "NEW_ROUND",
                "DRAW_TO_HAND",
                "HAND_PLAYED",
            ],
            "trajectory_path": [
                "BLIND_SELECT",
                "NEW_ROUND",
                "DRAW_TO_HAND",
                "SELECTING_HAND",
                "HAND_PLAYED",
                "DRAW_TO_HAND|NEW_ROUND",
                "ROUND_EVAL",
                "SHOP",
                "BLIND_SELECT",
            ],
            "refs": locate_many(STATE_SNIPPETS),
        },
        "evaluate_play_order": {
            "phases": [
                "hand_classification",
                "joker_before",
                "blind_modify_hand",
                "card_repetition_probe",
                "joker_repetition_probe",
                "joker_individual_card",
                "held_in_hand_repetition_probe",
                "joker_main",
                "joker_on_joker",
                "deck_back_final_scoring",
                "joker_after",
            ],
            "refs": locate_many(EVALUATE_PLAY_SNIPPETS),
        },
        "rng_order": {
            "requirements": [
                "cashout must shuffle deck before entering shop",
                "shop reroll must rebuild offers and then trigger reroll_shop joker hooks",
                "random selection uses pseudorandom/pseudorandom_element, not ad-hoc math.random without seeded context",
            ],
            "refs": locate_many(RNG_SNIPPETS),
        },
        "coverage_targets": {
            "boss": [
                "select boss blind",
                "enter boss blind",
                "defeat boss blind",
            ],
            "shop": [
                "cash_out",
                "buy_from_shop",
                "reroll_shop",
                "sell_card",
                "next_round",
            ],
            "consumable": [
                "buy consumable in shop",
                "use consumable",
            ],
            "refs": locate_many(SHOP_AND_CONSUMABLE_SNIPPETS),
        },
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build a source-derived Balatro fidelity oracle")
    parser.add_argument("--output", type=Path, default=Path("results/source-oracle.json"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    oracle = build_oracle()
    rendered = json.dumps(oracle, ensure_ascii=False, indent=2) + "\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
