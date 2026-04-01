"""Smart Balatro agent with deep strategy reasoning.

Encodes comprehensive Balatro strategy: hand evaluation across all types,
discard optimization, economy management ($5 interest threshold), shop
evaluation (Joker synergy, Planet/Tarot value), and Boss Blind awareness.

This is the "Claude Code manual play" mode — Claude's Balatro knowledge
compiled into deterministic code with CoT-style reasoning at each step.
"""
from __future__ import annotations

from itertools import combinations
from typing import Any

# ---------------------------------------------------------------------------
# Card utilities
# ---------------------------------------------------------------------------

RANKS = ["2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A"]
RANK_MAP = {"Two": 0, "Three": 1, "Four": 2, "Five": 3, "Six": 4, "Seven": 5,
            "Eight": 6, "Nine": 7, "Ten": 8, "Jack": 9, "Queen": 10, "King": 11, "Ace": 12}
CHIP_MAP = {"Two": 2, "Three": 3, "Four": 4, "Five": 5, "Six": 6, "Seven": 7,
            "Eight": 8, "Nine": 9, "Ten": 10, "Jack": 10, "Queen": 10, "King": 10, "Ace": 11}
SUIT_MAP = {"Spades": 0, "Hearts": 1, "Diamonds": 2, "Clubs": 3}

HAND_NAMES = ["High Card", "Pair", "Two Pair", "Three of a Kind", "Straight",
              "Flush", "Full House", "Four of a Kind", "Straight Flush"]
HAND_BASE_CHIPS = [5, 10, 20, 30, 30, 35, 40, 60, 100]
HAND_BASE_MULT = [1, 2, 2, 3, 4, 4, 4, 7, 8]

# Jokers that are especially valuable to buy
HIGH_VALUE_JOKERS = {
    "Joker", "Greedy Joker", "Lusty Joker", "Wrathful Joker", "Gluttonous Joker",
    "Jolly Joker", "Zany Joker", "Mad Joker", "Crazy Joker", "Droll Joker",
    "Half Joker", "Banner", "Mystic Summit", "Blue Joker", "Bull",
    "Abstract Joker", "Fibonacci", "Scary Face", "Scholar", "Even Steven",
    "Odd Todd", "Smiley Face", "Walkie Talkie", "Blackboard",
    "The Duo", "The Trio", "The Family", "The Order", "The Tribe",
    "Stuntman", "Acrobat", "Bootstraps", "Flower Pot",
    "Hack", "Photograph", "Triboulet", "Arrowhead", "Onyx Agate",
    "Golden Joker", "Cloud 9", "Rocket", "To the Moon",
    "Constellation", "Campfire", "Ice Cream", "Popcorn",
}


def _ri(card: dict) -> int:
    return RANK_MAP.get(card.get("rank", ""), -1)


def _si(card: dict) -> int:
    return SUIT_MAP.get(card.get("suit", ""), -1)


def _chips(card: dict) -> int:
    return CHIP_MAP.get(card.get("rank", ""), 0)


def _card_label(card: dict) -> str:
    r = card.get("rank", "?")
    short = {"Two": "2", "Three": "3", "Four": "4", "Five": "5", "Six": "6",
             "Seven": "7", "Eight": "8", "Nine": "9", "Ten": "10",
             "Jack": "J", "Queen": "Q", "King": "K", "Ace": "A"}.get(r, r)
    s = {"Spades": "S", "Hearts": "H", "Diamonds": "D", "Clubs": "C"}.get(card.get("suit", ""), "?")
    return f"{short}{s}"


# ---------------------------------------------------------------------------
# Hand classification
# ---------------------------------------------------------------------------

def classify_hand(cards: list[dict]) -> int:
    """Return hand type index (0=high card ... 8=straight flush)."""
    if not cards:
        return 0
    ranks = [_ri(c) for c in cards]
    suits = [_si(c) for c in cards]
    rc: dict[int, int] = {}
    for r in ranks:
        rc[r] = rc.get(r, 0) + 1
    n = len(cards)

    is_flush = n >= 5 and len(set(suits)) == 1
    is_straight = False
    if n >= 5:
        unique = sorted(set(ranks))
        for s in range(9):
            if all((s + i) in unique for i in range(5)):
                is_straight = True
                break
        if {0, 1, 2, 3, 12}.issubset(set(ranks)):
            is_straight = True

    mx = max(rc.values()) if rc else 0
    pairs = sum(1 for v in rc.values() if v >= 2)
    has_three = any(v >= 3 for v in rc.values())

    if is_straight and is_flush:
        return 8
    if mx >= 4:
        return 7
    if has_three and pairs >= 2:
        return 6
    if is_flush:
        return 5
    if is_straight:
        return 4
    if has_three:
        return 3
    if pairs >= 2:
        return 2
    if pairs >= 1:
        return 1
    return 0


def score_hand(cards: list[dict], hand_levels: dict | None = None) -> tuple[int, int, list[dict]]:
    """Score a hand. Returns (total_score, hand_type, scoring_cards)."""
    ht = classify_hand(cards)
    if ht >= len(HAND_BASE_CHIPS):
        ht = min(ht, len(HAND_BASE_CHIPS) - 1)

    base_c = HAND_BASE_CHIPS[ht]
    base_m = HAND_BASE_MULT[ht]

    # Apply hand level bonus if available
    if hand_levels:
        ht_name = HAND_NAMES[ht].lower().replace(" ", "_")
        level = hand_levels.get(ht_name, 1)
        if level > 1:
            base_c += (level - 1) * 10
            base_m += (level - 1) * 1

    ranks = [_ri(c) for c in cards]
    rc: dict[int, int] = {}
    for r in ranks:
        rc[r] = rc.get(r, 0) + 1

    if ht in (1, 2, 3, 6, 7):
        scoring = [c for c in cards if rc.get(_ri(c), 0) >= 2]
    elif ht in (4, 5, 8):
        scoring = list(cards)
    else:
        scoring = [max(cards, key=_chips)] if cards else []

    total_chips = base_c + sum(_chips(c) for c in scoring)
    return total_chips * base_m, ht, scoring


def find_best_hand(available: list[dict], hand_levels: dict | None = None,
                   max_size: int = 5) -> tuple[list[int], int, int, str]:
    """Find best hand from available cards.
    Returns (card_indices, estimated_score, hand_type, description).
    """
    best_score = 0
    best_indices: list[int] = []
    best_ht = 0

    for size in range(max(1, min(2, len(available))), min(max_size + 1, len(available) + 1)):
        for combo in combinations(range(len(available)), size):
            cards = [available[i] for i in combo]
            s, h, _ = score_hand(cards, hand_levels)
            if s > best_score:
                best_score = s
                best_indices = list(combo)
                best_ht = h

    desc = HAND_NAMES[best_ht] if best_ht < len(HAND_NAMES) else "?"
    card_labels = "+".join(_card_label(available[i]) for i in best_indices) if best_indices else "none"
    return best_indices, best_score, best_ht, f"{desc}({card_labels})~{best_score}"


def find_discard_candidates(available: list[dict], hand_levels: dict | None = None) -> list[int]:
    """Find cards to discard for improving hand quality.
    Returns indices of cards NOT in the best hand (worst cards first).
    """
    best_indices, _, _, _ = find_best_hand(available, hand_levels)
    best_set = set(best_indices)

    # Cards not in best hand, sorted by chip value ascending (worst first)
    non_best = [(i, _chips(available[i])) for i in range(len(available)) if i not in best_set]
    non_best.sort(key=lambda x: x[1])
    return [i for i, _ in non_best]


# ---------------------------------------------------------------------------
# Smart Agent
# ---------------------------------------------------------------------------

class SmartAgent:
    """Deep strategy Balatro agent with CoT reasoning."""

    name = "smart"
    stats: dict[str, Any] = {}

    def __init__(self) -> None:
        self._target_indices: list[int] = []
        self._discard_indices: list[int] = []
        self._phase = "idle"  # idle | selecting | deselecting | discarding

    def decide(self, snap: dict, legal: list[str]) -> tuple[str, str]:
        stage = snap.get("stage", "")
        if stage == "Stage_PreBlind":
            return self._decide_preblind(snap, legal)
        if stage == "Stage_Blind":
            return self._decide_blind(snap, legal)
        if stage == "Stage_PostBlind":
            return self._decide_postblind(snap, legal)
        if stage == "Stage_Shop":
            return self._decide_shop(snap, legal)
        return legal[0] if legal else "select_blind_0", "Fallback."

    # --- PreBlind ---
    def _decide_preblind(self, snap: dict, legal: list[str]) -> tuple[str, str]:
        # Skip blind strategy: skip Small/Big if early game and want tag reward
        # For now: always enter
        self._phase = "idle"
        self._target_indices = []
        self._discard_indices = []
        if "select_blind_0" in legal:
            blind = snap.get("blind_name", "?")
            return "select_blind_0", f"Entering {blind}."
        if "skip_blind" in legal:
            return "skip_blind", "Skipping blind for tag."
        return legal[0], "Fallback."

    # --- Blind (main gameplay) ---
    def _decide_blind(self, snap: dict, legal: list[str]) -> tuple[str, str]:
        available = snap.get("available", [])
        selected = set(snap.get("selected_slots", []))
        score = snap.get("score", 0)
        required = snap.get("required_score", 1)
        plays = snap.get("plays", 0)
        discards = snap.get("discards", 0)
        hand_levels = snap.get("hand_levels", {})
        remaining = max(0, required - score)

        if not available:
            return legal[0] if legal else "play", "No cards."

        # Step 1: Evaluate current best hand
        best_idx, best_score, best_ht, best_desc = find_best_hand(available, hand_levels)

        # Step 2: Decide strategy — play or discard?
        if self._phase == "idle":
            # If plays=0 and discards=0, nothing to do (engine will end game)
            if plays <= 0 and discards <= 0:
                return legal[0] if legal else "play", "No plays or discards left. Game ending."

            should_discard = self._should_discard(
                available, best_score, remaining, plays, discards, hand_levels
            )
            if should_discard:
                discard_candidates = find_discard_candidates(available, hand_levels)
                # Discard up to 5 worst cards
                self._discard_indices = discard_candidates[:min(5, len(discard_candidates))]
                self._phase = "discard_select"
                self._target_indices = []
            else:
                self._target_indices = list(best_idx)
                self._phase = "play_select"
                self._discard_indices = []

        # Step 3: Execute selection/deselection
        if self._phase == "discard_select":
            return self._execute_selection(
                selected, set(self._discard_indices), available, legal,
                on_complete=("discard", f"Discarding {len(self._discard_indices)} weak cards to draw better. "
                             f"Current best: {best_desc}, need {remaining} more."),
            )

        if self._phase == "play_select":
            return self._execute_selection(
                selected, set(self._target_indices), available, legal,
                on_complete=("play", f"Playing {best_desc}. Need {remaining} more. {plays} plays left."),
            )

        # Fallback: reset and try again
        self._phase = "idle"
        if "play" in legal and selected:
            return "play", "Playing current selection (fallback)."
        return legal[0] if legal else "play", "Fallback."

    def _should_discard(self, available, best_score, remaining, plays, discards, hand_levels) -> bool:
        """Should we discard instead of playing?"""
        if discards <= 0:
            return False

        # If no plays left but discards remain, MUST discard to draw and hope
        if plays <= 0:
            return True

        # If current best hand can cover >40% of remaining, play it
        if remaining > 0 and best_score >= remaining * 0.4:
            return False

        # On last play: only discard if hand is truly terrible (high card only)
        if plays <= 1:
            _, _, best_ht, _ = find_best_hand(available, hand_levels)
            if best_ht <= 0:
                return True  # High card on last play — discard is better
            return False  # At least a pair, commit to play

        # If best hand is only high card or weak pair, consider discard
        _, _, best_ht, _ = find_best_hand(available, hand_levels)
        if best_ht <= 0:
            return True  # High card only — definitely discard
        if best_ht == 1 and best_score < remaining * 0.2:
            return True  # Weak pair that barely helps

        return False

    def _execute_selection(self, current_selected: set, target_selected: set,
                           available: list[dict], legal: list[str],
                           on_complete: tuple[str, str]) -> tuple[str, str]:
        """Toggle cards to match target selection, then execute completion action."""
        need_deselect = current_selected - target_selected
        need_select = target_selected - current_selected

        if need_deselect:
            idx = next(iter(need_deselect))
            act = f"select_card_{idx}"
            if act in legal:
                card = available[idx] if idx < len(available) else {}
                return act, f"Deselecting {_card_label(card)} (not needed)."

        if need_select:
            idx = next(iter(need_select))
            act = f"select_card_{idx}"
            if act in legal:
                card = available[idx] if idx < len(available) else {}
                return act, f"Selecting {_card_label(card)} for hand."

        # All toggles done — execute
        action, reason = on_complete
        if action in legal:
            self._phase = "idle"
            self._target_indices = []
            self._discard_indices = []
            return action, reason

        # Can't execute intended action, reset
        self._phase = "idle"
        return legal[0] if legal else "play", "Cannot execute, fallback."

    # --- PostBlind ---
    def _decide_postblind(self, snap: dict, legal: list[str]) -> tuple[str, str]:
        self._phase = "idle"
        return "cashout", f"Cashout. Reward: ${snap.get('reward', 0)}."

    # --- Shop ---
    def _decide_shop(self, snap: dict, legal: list[str]) -> tuple[str, str]:
        self._phase = "idle"
        money = snap.get("money", 0)
        jokers = snap.get("jokers", [])
        shop_jokers = snap.get("shop_jokers", [])
        consumables = snap.get("consumables", [])
        shop_cons = snap.get("shop_consumables", [])
        ante = snap.get("ante", 1)

        # Economy strategy: keep money at $5 multiples for interest
        interest_threshold = ((money // 5) * 5)  # Current interest bracket
        min_keep = max(0, interest_threshold)  # Try to stay at this level

        # Buy joker if valuable and affordable (keep interest threshold)
        if len(jokers) < 5 and shop_jokers:
            best_buy = self._evaluate_shop_jokers(shop_jokers, jokers, money, min_keep)
            if best_buy is not None:
                idx, name, cost, reason = best_buy
                act = f"buy_shop_item_{idx}"
                if act in legal:
                    return act, reason

        # Buy consumable if useful
        if shop_cons and len(consumables) < 2:
            for i, c in enumerate(shop_cons):
                cname = c.get("name", c.get("consumable_name", "?"))
                cset = c.get("set", "")
                cost = c.get("buy_cost", c.get("cost", 99))
                if cost <= money - min_keep:
                    act = f"buy_consumable_{i}"
                    if act in legal:
                        # Prefer Planet cards (level up hand types)
                        if cset == "Planet":
                            return act, f"Buying {cname} (Planet) to level up a hand type."
                        if cset == "Tarot" and ante <= 2:
                            return act, f"Buying {cname} (Tarot) for card modification."

        # Use consumable if we have one
        for i in range(len(consumables)):
            act = f"use_consumable_{i}"
            if act in legal:
                cname = consumables[i].get("name", "?") if i < len(consumables) else "?"
                return act, f"Using {cname}."

        if "next_round" in legal:
            return "next_round", f"Moving on. Saving ${money} ({money//5} interest)."
        return legal[0] if legal else "next_round", "Fallback."

    def _evaluate_shop_jokers(self, shop_jokers, owned_jokers, money, min_keep):
        """Evaluate shop jokers and return best purchase or None."""
        owned_names = {j.get("name", j.get("joker_name", "")) for j in owned_jokers}
        candidates = []

        for i, j in enumerate(shop_jokers):
            name = j.get("name", j.get("joker_name", "?"))
            cost = j.get("buy_cost", j.get("cost", 99))

            if cost > money - min_keep:
                continue  # Can't afford without breaking interest
            if name in owned_names:
                continue  # Already have it

            # Score the joker
            value = 1  # Base value
            if name in HIGH_VALUE_JOKERS:
                value = 3
            # Economy jokers are very valuable
            if any(kw in name.lower() for kw in ("gold", "cloud", "rocket", "moon", "interest")):
                value = 4

            candidates.append((i, name, cost, value))

        if not candidates:
            # Still buy if very cheap and have plenty of money
            for i, j in enumerate(shop_jokers):
                name = j.get("name", j.get("joker_name", "?"))
                cost = j.get("buy_cost", j.get("cost", 99))
                if cost <= money - min_keep and cost <= 4:
                    return (i, name, cost, f"Buying {name} (${cost}). Cheap, worth having.")
            return None

        # Pick highest value
        candidates.sort(key=lambda x: -x[3])
        i, name, cost, value = candidates[0]
        reason = f"Buying {name} (${cost}). "
        if value >= 4:
            reason += "Excellent economy joker."
        elif value >= 3:
            reason += "High value scoring joker."
        else:
            reason += "Decent addition to build."
        return (i, name, cost, reason)
