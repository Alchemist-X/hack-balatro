"""Play a full Balatro game with strategic decision-making.

This script simulates an LLM playing Balatro step-by-step with
reasoning in Chinese for each decision. The trajectory is saved
for training smaller models.

Usage:
    python scripts/llm_play_game.py --seed 42
"""
from __future__ import annotations

import sys
import json
import os
import argparse
from collections import Counter

sys.path.insert(0, ".")
import balatro_native
from env.state_serializer import serialize_state

# =============================================================
# Constants
# =============================================================
RANKS_ORDER = [
    "Two", "Three", "Four", "Five", "Six", "Seven", "Eight",
    "Nine", "Ten", "Jack", "Queen", "King", "Ace",
]
RANK_SHORT = {
    "Two": "2", "Three": "3", "Four": "4", "Five": "5",
    "Six": "6", "Seven": "7", "Eight": "8", "Nine": "9",
    "Ten": "10", "Jack": "J", "Queen": "Q", "King": "K", "Ace": "A",
}
RANK_VALUE = {
    "Two": 2, "Three": 3, "Four": 4, "Five": 5, "Six": 6,
    "Seven": 7, "Eight": 8, "Nine": 9, "Ten": 10,
    "Jack": 10, "Queen": 10, "King": 10, "Ace": 11,
}
RANK_ORDER_IDX = {r: i for i, r in enumerate(RANKS_ORDER)}

HAND_SCORES: dict[str, tuple[int, int]] = {
    "High Card": (5, 1),
    "Pair": (10, 2),
    "Two Pair": (20, 2),
    "Three of a Kind": (30, 3),
    "Straight": (30, 4),
    "Flush": (35, 4),
    "Full House": (40, 4),
    "Four of a Kind": (60, 7),
    "Straight Flush": (100, 8),
}

# Consumable types worth buying
VALUABLE_CONSUMABLE_SETS = {"Planet", "Spectral"}


# =============================================================
# Helper functions
# =============================================================

def card_short(c: dict) -> str:
    r = RANK_SHORT.get(c.get("rank", "?"), c.get("rank", "?"))
    s = c.get("suit", "?")[0]
    return f"{r}{s}"


def estimate_score(hand_type: str, selected_cards: list[dict]) -> int:
    base_chips, base_mult = HAND_SCORES.get(hand_type, (5, 1))
    card_chips = sum(RANK_VALUE.get(c.get("rank"), 0) for c in selected_cards)
    return (base_chips + card_chips) * base_mult


def find_best_hand(cards: list[dict]) -> tuple[str, list[int], int]:
    """Find best poker hand. Returns (hand_type, card_indices, est_score)."""
    n = len(cards)
    if n == 0:
        return "High Card", [], 0

    ranks = [c.get("rank") for c in cards]
    suits = [c.get("suit") for c in cards]
    rank_counts = Counter(ranks)
    suit_counts = Counter(suits)
    candidates: list[tuple[int, int, str, list[int]]] = []

    # -- Four of a Kind --
    for qr in [r for r, cnt in rank_counts.items() if cnt >= 4]:
        indices = [i for i in range(n) if cards[i]["rank"] == qr][:4]
        kickers = sorted(
            [(RANK_VALUE.get(cards[i]["rank"], 0), i) for i in range(n) if i not in indices],
            reverse=True,
        )
        if kickers:
            indices.append(kickers[0][1])
        sel = [cards[i] for i in indices]
        candidates.append((7, estimate_score("Four of a Kind", sel), "Four of a Kind", indices))

    # -- Full House --
    trips = [r for r, cnt in rank_counts.items() if cnt >= 3]
    pairs_all = [r for r, cnt in rank_counts.items() if cnt >= 2]

    for tr in trips:
        for pr in [p for p in pairs_all if p != tr]:
            indices = [i for i in range(n) if cards[i]["rank"] == tr][:3]
            indices += [i for i in range(n) if cards[i]["rank"] == pr and i not in indices][:2]
            if len(indices) == 5:
                sel = [cards[i] for i in indices]
                candidates.append((6, estimate_score("Full House", sel), "Full House", indices))

    # -- Straight Flush (check before regular Flush/Straight) --
    for suit, cnt in suit_counts.items():
        if cnt >= 5:
            suited = sorted(
                [(RANK_ORDER_IDX.get(cards[i]["rank"], -1), i) for i in range(n) if cards[i]["suit"] == suit]
            )
            for sp in range(len(suited) - 4):
                window = [suited[sp + j][0] for j in range(5)]
                if window[-1] - window[0] == 4:
                    indices = [suited[sp + j][1] for j in range(5)]
                    sel = [cards[i] for i in indices]
                    candidates.append((8, estimate_score("Straight Flush", sel), "Straight Flush", indices))

    # -- Flush --
    for suit, cnt in suit_counts.items():
        if cnt >= 5:
            fc = sorted(
                [(RANK_VALUE.get(cards[i]["rank"], 0), i) for i in range(n) if cards[i]["suit"] == suit],
                reverse=True,
            )
            indices = [x[1] for x in fc[:5]]
            sel = [cards[i] for i in indices]
            candidates.append((5, estimate_score("Flush", sel), "Flush", indices))

    # -- Straight --
    unique_ri = sorted(set(RANK_ORDER_IDX.get(r, -1) for r in ranks if r in RANK_ORDER_IDX))
    for s in range(len(unique_ri)):
        if s + 4 < len(unique_ri) and unique_ri[s + 4] - unique_ri[s] == 4:
            tgt = [RANKS_ORDER[unique_ri[s + j]] for j in range(5)]
            indices: list[int] = []
            for t in tgt:
                for i in range(n):
                    if cards[i]["rank"] == t and i not in indices:
                        indices.append(i)
                        break
            if len(indices) == 5:
                sel = [cards[i] for i in indices]
                candidates.append((4, estimate_score("Straight", sel), "Straight", indices))
    # Ace-low straight
    if {0, 1, 2, 3, 12}.issubset(set(unique_ri)):
        tgt = ["Ace", "Two", "Three", "Four", "Five"]
        indices = []
        for t in tgt:
            for i in range(n):
                if cards[i]["rank"] == t and i not in indices:
                    indices.append(i)
                    break
        if len(indices) == 5:
            sel = [cards[i] for i in indices]
            candidates.append((4, estimate_score("Straight", sel), "Straight", indices))

    # -- Three of a Kind --
    for tr in trips:
        indices = [i for i in range(n) if cards[i]["rank"] == tr][:3]
        kickers = sorted(
            [(RANK_VALUE.get(cards[i]["rank"], 0), i) for i in range(n) if i not in indices],
            reverse=True,
        )
        indices += [k[1] for k in kickers[:2]]
        sel = [cards[i] for i in indices]
        candidates.append((3, estimate_score("Three of a Kind", sel), "Three of a Kind", indices))

    # -- Two Pair --
    if len(pairs_all) >= 2:
        sp = sorted(pairs_all, key=lambda r: RANK_VALUE.get(r, 0), reverse=True)
        for ip in range(min(3, len(sp))):
            for jp in range(ip + 1, min(4, len(sp))):
                p1, p2 = sp[ip], sp[jp]
                indices = [k for k in range(n) if cards[k]["rank"] == p1][:2]
                indices += [k for k in range(n) if cards[k]["rank"] == p2 and k not in indices][:2]
                kickers = sorted(
                    [(RANK_VALUE.get(cards[k]["rank"], 0), k) for k in range(n) if k not in indices],
                    reverse=True,
                )
                if kickers:
                    indices.append(kickers[0][1])
                sel = [cards[k] for k in indices]
                candidates.append((2, estimate_score("Two Pair", sel), "Two Pair", indices))

    # -- Pair --
    for pr in pairs_all:
        indices = [i for i in range(n) if cards[i]["rank"] == pr][:2]
        kickers = sorted(
            [(RANK_VALUE.get(cards[i]["rank"], 0), i) for i in range(n) if i not in indices],
            reverse=True,
        )
        indices += [k[1] for k in kickers[:3]]
        sel = [cards[i] for i in indices]
        candidates.append((1, estimate_score("Pair", sel), "Pair", indices))

    # -- High Card --
    v = sorted([(RANK_VALUE.get(cards[i]["rank"], 0), i) for i in range(n)], reverse=True)
    hci = [x[1] for x in v[:5]]
    hc_sel = [cards[i] for i in hci]
    candidates.append((0, estimate_score("High Card", hc_sel), "High Card", hci))

    candidates.sort(key=lambda x: (x[0], x[1]), reverse=True)
    best = candidates[0]
    return best[2], best[3], best[1]


def get_discard_indices(cards: list[dict]) -> list[int]:
    """Choose cards to discard. Keep pairs, flush draws, and high cards."""
    ranks = [c.get("rank") for c in cards]
    suits = [c.get("suit") for c in cards]
    rc = Counter(ranks)
    sc = Counter(suits)
    keep: set[int] = set()

    # Keep paired/tripled cards
    for r, cnt in rc.items():
        if cnt >= 2:
            for i, c in enumerate(cards):
                if c["rank"] == r:
                    keep.add(i)

    # Keep flush draws (4+ of same suit)
    for s, cnt in sc.items():
        if cnt >= 4:
            for i, c in enumerate(cards):
                if c["suit"] == s:
                    keep.add(i)

    # Keep Aces and Kings
    for i, c in enumerate(cards):
        if c["rank"] in ("Ace", "King") and len(keep) < 4:
            keep.add(i)

    # Discard non-kept, lowest value first
    disc = sorted(
        [(RANK_VALUE.get(cards[i]["rank"], 0), i) for i in range(len(cards)) if i not in keep]
    )
    count = min(5, len(disc))
    if count == 0:
        av = sorted([(RANK_VALUE.get(cards[i]["rank"], 0), i) for i in range(len(cards))])
        return [x[1] for x in av[: min(5, len(av))]]
    return [x[1] for x in disc[:count]]


def has_flush_draw(cards: list[dict]) -> bool:
    sc = Counter(c.get("suit") for c in cards)
    return any(cnt >= 4 for cnt in sc.values())


def has_straight_draw(cards: list[dict]) -> bool:
    ri = sorted(set(RANK_ORDER_IDX.get(c.get("rank"), -1) for c in cards if c.get("rank") in RANK_ORDER_IDX))
    for i in range(len(ri)):
        for j in range(i + 1, len(ri)):
            if ri[j] - ri[i] <= 4:
                cnt = sum(1 for r in ri if ri[i] <= r <= ri[j])
                if cnt >= 4:
                    return True
    return False


# =============================================================
# Game Engine Wrapper
# =============================================================

class BalatroPlayer:
    def __init__(self, seed: int = 42, stake: int = 1, verbose: bool = False):
        self.eng = balatro_native.Engine(seed=seed, stake=stake)
        self.trajectory: list[dict] = []
        self.step_count = 0
        self.seed = seed
        self.verbose = verbose

    def get_state(self) -> tuple[dict, list[str], str]:
        snap = json.loads(self.eng.snapshot().to_json())
        acts = [a.name for a in self.eng.legal_actions() if a.enabled]
        state_text = serialize_state(snap, acts)
        return snap, acts, state_text

    def do_action(self, action_name: str, reasoning: str) -> dict:
        snap_before, acts, state_text = self.get_state()
        score_before = snap_before.get("score", 0)

        for a in self.eng.legal_actions():
            if a.enabled and a.name == action_name:
                self.eng.step(a.index)
                break
        else:
            raise ValueError(f"Action '{action_name}' unavailable. Enabled: {acts}")

        snap_after = json.loads(self.eng.snapshot().to_json())
        score_after = snap_after.get("score", 0)

        self.trajectory.append({
            "step": self.step_count,
            "state_text": state_text,
            "reasoning": reasoning,
            "action": action_name,
            "score_before": score_before,
            "score_after": score_after,
        })
        self.step_count += 1
        return snap_after

    def step_by_name(self, name: str) -> bool:
        for a in self.eng.legal_actions():
            if a.enabled and a.name == name:
                self.eng.step(a.index)
                return True
        return False

    def try_use_consumables(self, snap: dict, acts: list[str]) -> bool:
        """Try to use Planet or Spectral consumables. Returns True if used."""
        consumables = snap.get("consumables", [])
        for i, cons in enumerate(consumables):
            act_name = f"use_consumable_{i}"
            if act_name not in acts:
                continue
            name = cons.get("name", cons.get("consumable_name", "?"))
            cset = cons.get("set", "?")
            if cset in ("Planet", "Spectral"):
                self.do_action(
                    act_name,
                    f"使用消耗品'{name}'[{cset}]。行星牌升级牌型等级增加得分，幽灵牌有特殊加强效果。",
                )
                return True
        return False

    def handle_pre_blind(self, snap: dict, acts: list[str]) -> None:
        blind = snap.get("blind_name", "?")
        money = snap.get("money", 0)
        ante = snap.get("ante", 0)
        jokers = snap.get("jokers", [])
        joker_names = [j.get("name", j.get("joker_name", "?")) for j in jokers]

        # Use consumables before entering blind
        for _ in range(5):
            snap2, acts2, _ = self.get_state()
            if snap2["stage"] != "Stage_PreBlind":
                return
            if not self.try_use_consumables(snap2, acts2):
                break

        snap, acts, _ = self.get_state()
        if snap["stage"] != "Stage_PreBlind":
            return

        # Select the appropriate blind
        for i in range(3):
            act = f"select_blind_{i}"
            if act in acts:
                jk_str = ", ".join(joker_names) if joker_names else "无"
                self.do_action(
                    act,
                    f"Ante {ante}，进入{blind}。当前${money}。小丑: [{jk_str}]。"
                    f"不跳过盲注，需要积累资金和通过盲注推进游戏进度。",
                )
                return

        if "skip_blind" in acts:
            self.do_action("skip_blind", f"无法选择盲注，跳过{blind}。")

    def handle_blind(self, snap: dict, acts: list[str]) -> None:
        """Play through a blind phase with strategic hand selection."""
        for _ in range(50):
            snap, acts, _ = self.get_state()
            if snap["stage"] != "Stage_Blind":
                return
            if snap.get("over"):
                return

            hand = snap.get("available", [])
            selected_slots = snap.get("selected_slots", [])
            score = snap.get("score", 0)
            required = snap.get("required_score", 0)
            plays = snap.get("plays", 0)
            discards = snap.get("discards", 0)
            score_needed = required - score

            if not hand:
                return

            # Deselect any currently selected cards
            if selected_slots:
                for slot in selected_slots:
                    act = f"select_card_{slot}"
                    if act in acts:
                        self.do_action(act, "取消选择，重新分析手牌。")
                continue

            # Try using consumables during blind
            if self.try_use_consumables(snap, acts):
                continue

            hand_str = " ".join(card_short(c) for c in hand)
            ht, bi, est = find_best_hand(hand)

            if plays <= 0 and discards <= 0:
                return

            if plays <= 0 and discards > 0:
                disc_indices = list(range(min(5, len(hand))))
                disc_str = " ".join(card_short(hand[i]) for i in disc_indices)
                for idx in disc_indices:
                    self.do_action(f"select_card_{idx}", f"选择弃牌(消耗弃牌次数)")
                self.do_action(
                    "discard",
                    f"出牌次数耗尽，消耗弃牌结束回合。当前{score}/{required}。弃: {disc_str}",
                )
                continue

            # Decide: play or discard?
            should_disc = False
            disc_reason = ""

            if discards > 0 and plays > 1:
                if ht == "High Card":
                    should_disc = True
                    disc_reason = "仅高牌，弃牌希望改善手牌"
                elif ht == "Pair" and plays > 2:
                    if has_flush_draw(hand):
                        should_disc = True
                        disc_reason = "有对子但有同花抽牌(4张同花色)，弃牌搏同花"
                    elif has_straight_draw(hand):
                        should_disc = True
                        disc_reason = "有对子但有顺子抽牌，弃牌搏顺子"

            if should_disc:
                di = get_discard_indices(hand)
                disc_str = " ".join(card_short(hand[i]) for i in di)
                reasoning = (
                    f"手牌: {hand_str}。牌型: {ht}(预估{est})。"
                    f"{disc_reason}。弃: {disc_str}。"
                    f"剩余出牌{plays}次，弃牌{discards}次，还差{score_needed}分。"
                )
                for idx in di:
                    self.do_action(f"select_card_{idx}", f"选择弃牌: {card_short(hand[idx])}")
                self.do_action("discard", reasoning)
            else:
                sel_str = " ".join(card_short(hand[i]) for i in bi)
                reasoning = (
                    f"手牌: {hand_str}。最佳牌型: {ht}(预估{est}分)。"
                    f"打出: {sel_str}。当前{score}/{required}，还差{score_needed}。"
                    f"剩余出牌{plays}次。"
                )
                for idx in bi:
                    self.do_action(f"select_card_{idx}", f"选择出牌: {card_short(hand[idx])}")
                self.do_action("play", reasoning)

    def handle_post_blind(self, snap: dict, acts: list[str]) -> None:
        if "cashout" in acts:
            money = snap.get("money", 0)
            score = snap.get("score", 0)
            required = snap.get("required_score", 0)
            self.do_action(
                "cashout",
                f"盲注通过！{score}/{required}。兑现奖励。当前${money}。",
            )
        elif acts:
            self.do_action(acts[0], f"PostBlind: {acts[0]}")

    def handle_shop(self, snap: dict, acts: list[str]) -> None:
        """Smart shopping: buy jokers, planet cards, spectral cards, then use them."""
        for _ in range(10):
            snap, acts, _ = self.get_state()
            if snap["stage"] != "Stage_Shop":
                return

            money = snap.get("money", 0)
            jokers = snap.get("jokers", [])
            consumables = snap.get("consumables", [])
            shop_jokers = snap.get("shop_jokers", [])
            shop_cons = snap.get("shop_consumables", [])
            cons_limit = snap.get("consumable_slot_limit", 2)

            bought = False

            # Priority 1: Buy jokers
            if len(jokers) < 5 and shop_jokers:
                for i, sj in enumerate(shop_jokers):
                    cost = sj.get("buy_cost", sj.get("cost", 999))
                    name = sj.get("name", sj.get("joker_name", "?"))
                    act_name = f"buy_shop_item_{i}"
                    if cost <= money and act_name in acts:
                        jk_str = ", ".join(
                            j.get("name", j.get("joker_name", "?")) for j in jokers
                        )
                        self.do_action(
                            act_name,
                            f"购买小丑'{name}'(${cost})。当前小丑: [{jk_str}]。"
                            f"购买后${money - cost}。小丑是核心得分增强，优先购买。",
                        )
                        bought = True
                        break

            # Priority 2: Buy valuable consumables
            if not bought and len(consumables) < cons_limit:
                for i, sc in enumerate(shop_cons):
                    cost = sc.get("buy_cost", sc.get("cost", 999))
                    name = sc.get("name", sc.get("consumable_name", "?"))
                    cset = sc.get("set", "?")
                    act_name = f"buy_consumable_{i}"
                    if (
                        cset in VALUABLE_CONSUMABLE_SETS
                        and cost <= money
                        and act_name in acts
                    ):
                        self.do_action(
                            act_name,
                            f"购买消耗品'{name}'[{cset}](${cost})。"
                            f"行星牌升级牌型等级，幽灵牌提供特殊加强效果。",
                        )
                        bought = True
                        break

            if not bought:
                break

        # Use consumables we have
        for _ in range(5):
            snap, acts, _ = self.get_state()
            if snap["stage"] != "Stage_Shop":
                return
            if not self.try_use_consumables(snap, acts):
                break

        # Exit shop
        snap, acts, _ = self.get_state()
        if snap["stage"] == "Stage_Shop":
            if "next_round" in acts:
                money = snap.get("money", 0)
                interest = min(money // 5, 5)
                self.do_action(
                    "next_round",
                    f"商店完成。当前${money}(利息+${interest})。进入下一轮。",
                )

    def play_game(self) -> dict:
        """Play a full game, returning the result."""
        for _ in range(5000):
            snap, acts, _ = self.get_state()
            stage = snap.get("stage", "?")

            if snap.get("over", False):
                if self.verbose:
                    won = snap.get("won", False)
                    ante = snap.get("ante", 0)
                    print(f"GAME OVER! Won: {won}, Ante: {ante}")
                break

            if stage == "Stage_PreBlind":
                self.handle_pre_blind(snap, acts)
            elif stage == "Stage_Blind":
                self.handle_blind(snap, acts)
            elif stage == "Stage_PostBlind":
                self.handle_post_blind(snap, acts)
            elif stage == "Stage_Shop":
                self.handle_shop(snap, acts)
            elif stage == "Stage_End":
                break
            else:
                if acts:
                    self.do_action(acts[0], f"未知阶段'{stage}'，执行: {acts[0]}")
                else:
                    break

        final_snap = json.loads(self.eng.snapshot().to_json())
        return {
            "seed": self.seed,
            "agent": "claude_code",
            "won": final_snap.get("won", False),
            "final_ante": final_snap.get("ante", 0),
            "steps": self.step_count,
            "trajectory": self.trajectory,
        }


def print_summary(result: dict) -> None:
    """Print a human-readable summary of the game."""
    print(f"Game complete!")
    print(f"  Won: {result['won']}")
    print(f"  Final Ante: {result['final_ante']}")
    print(f"  Steps: {result['steps']}")

    print(f"\nBlind-by-blind summary:")
    for t in result["trajectory"]:
        action = t["action"]
        if action.startswith("select_blind_"):
            for line in t["state_text"].split("\n"):
                if "[STAGE]" in line or "[ANTE]" in line:
                    print(f"  {line}")
        elif action == "play":
            delta = t["score_after"] - t["score_before"]
            r = t["reasoning"]
            ht = "?"
            if "牌型:" in r:
                idx_s = r.find("牌型:") + 3
                idx_e = r.find("(", idx_s)
                if idx_e > idx_s:
                    ht = r[idx_s:idx_e].strip()
            print(f"    Play: {ht:20s} +{delta:6d}")
        elif action == "discard":
            print(f"    Discard")
        elif action == "cashout":
            print(f"    => CASHOUT")
        elif action.startswith("buy_shop_item"):
            r = t["reasoning"]
            # Extract joker name
            if "'" in r:
                name = r.split("'")[1]
                print(f"    Buy Joker: {name}")
        elif action.startswith("buy_consumable"):
            r = t["reasoning"]
            if "'" in r:
                name = r.split("'")[1]
                print(f"    Buy Consumable: {name}")
        elif action.startswith("use_consumable"):
            r = t["reasoning"]
            if "'" in r:
                name = r.split("'")[1]
                print(f"    Use: {name}")


def main():
    parser = argparse.ArgumentParser(description="Play Balatro with LLM strategy")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--stake", type=int, default=1)
    parser.add_argument("--output", type=str, default=None)
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    player = BalatroPlayer(seed=args.seed, stake=args.stake, verbose=args.verbose)
    result = player.play_game()

    output_path = (
        args.output
        or f"results/trajectories/llm_claude_code/game_{args.seed:04d}.json"
    )
    os.makedirs(os.path.dirname(output_path), exist_ok=True)

    with open(output_path, "w") as f:
        json.dump(result, f, indent=2, ensure_ascii=False)

    print(f"Saved to: {output_path}")
    print_summary(result)

    return result


if __name__ == "__main__":
    main()
