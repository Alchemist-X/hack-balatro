#!/usr/bin/env python3
"""Collect Balatro trajectories using an LLM agent.

Plays N games, recording (state, reasoning, action) at each step.
Supports Claude API and a built-in heuristic fallback.

Usage:
    # With Claude API (needs ANTHROPIC_API_KEY):
    python scripts/collect_llm_trajectories.py --agent claude --games 10

    # With built-in heuristic (no API needed, fast):
    python scripts/collect_llm_trajectories.py --agent heuristic --games 50

    # Quick test:
    python scripts/collect_llm_trajectories.py --agent heuristic --games 3 --verbose
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from datetime import datetime, timezone
from itertools import combinations
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import balatro_native
from env.state_serializer import serialize_state, serialize_for_llm_prompt

# ---------------------------------------------------------------------------
# Card / hand utilities
# ---------------------------------------------------------------------------

RANKS = ["2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A"]
RANK_MAP = {"Two": 0, "Three": 1, "Four": 2, "Five": 3, "Six": 4, "Seven": 5,
            "Eight": 6, "Nine": 7, "Ten": 8, "Jack": 9, "Queen": 10, "King": 11, "Ace": 12}
HAND_BASE = {0: (5,1), 1: (10,2), 2: (20,2), 3: (30,3), 4: (30,4),
             5: (35,4), 6: (40,4), 7: (60,7), 8: (100,8)}
HAND_NAMES = ["High Card","Pair","Two Pair","3oK","Straight","Flush","Full House","4oK","Str Flush"]


def _rank_idx(card: dict) -> int:
    r = card.get("rank", "")
    return RANK_MAP.get(r, -1)


def _suit_idx(card: dict) -> int:
    s = card.get("suit", "")
    return {"Spades": 0, "Hearts": 1, "Diamonds": 2, "Clubs": 3}.get(s, -1)


def _classify(indices: list[tuple[int, int]]) -> int:
    """Classify hand from (rank, suit) pairs."""
    if not indices:
        return 0
    ranks = [r for r, _ in indices]
    suits = [s for _, s in indices]
    rc: dict[int, int] = {}
    for r in ranks:
        rc[r] = rc.get(r, 0) + 1
    n = len(indices)
    fl = n >= 5 and len(set(suits)) == 1
    st = False
    if n >= 5:
        ur = sorted(set(ranks))
        for s in range(9):
            if all((s + i) in ur for i in range(5)):
                st = True
                break
        if {0, 1, 2, 3, 12}.issubset(set(ranks)):
            st = True
    mx = max(rc.values())
    pairs = sum(1 for v in rc.values() if v >= 2)
    t3 = any(v >= 3 for v in rc.values())
    if st and fl: return 8
    if mx >= 4: return 7
    if t3 and pairs >= 2: return 6
    if fl: return 5
    if st: return 4
    if t3: return 3
    if pairs >= 2: return 2
    if pairs >= 1: return 1
    return 0


def _est_score(cards: list[dict]) -> tuple[int, int]:
    idx = [(_rank_idx(c), _suit_idx(c)) for c in cards]
    ht = _classify(idx)
    bc, bm = HAND_BASE.get(ht, (5, 1))
    rc: dict[int, int] = {}
    for r, _ in idx:
        rc[r] = rc.get(r, 0) + 1
    chip_val = {"Two": 2, "Three": 3, "Four": 4, "Five": 5, "Six": 6, "Seven": 7,
                "Eight": 8, "Nine": 9, "Ten": 10, "Jack": 10, "Queen": 10, "King": 10, "Ace": 11}
    if ht in (1, 2, 3, 6, 7):
        scoring = [c for c in cards if rc.get(_rank_idx(c), 0) >= 2]
    elif ht in (4, 5, 8):
        scoring = cards
    else:
        scoring = [max(cards, key=lambda c: chip_val.get(c.get("rank", ""), 0))] if cards else []
    total = bc + sum(chip_val.get(c.get("rank", ""), 0) for c in scoring)
    return total * bm, ht


def _best_hand(available: list[dict], max_size: int = 5) -> tuple[list[int], int, int]:
    bs, bi, bht = 0, [], 0
    for sz in range(max(1, min(2, len(available))), min(max_size + 1, len(available) + 1)):
        for combo in combinations(range(len(available)), sz):
            cards = [available[i] for i in combo]
            s, h = _est_score(cards)
            if s > bs:
                bs, bi, bht = s, list(combo), h
    return bi, bs, bht


# ---------------------------------------------------------------------------
# Agent: Heuristic (built-in, no API)
# ---------------------------------------------------------------------------

class HeuristicAgent:
    """Deterministic heuristic agent with CoT-style reasoning."""

    def decide(self, snapshot_dict: dict, legal_actions: list[str]) -> tuple[str, str]:
        """Returns (action_name, reasoning_text)."""
        stage = snapshot_dict.get("stage", "")
        available = snapshot_dict.get("available", [])
        selected_slots = set(snapshot_dict.get("selected_slots", []))
        score = snapshot_dict.get("score", 0)
        required = snapshot_dict.get("required_score", 1)
        plays = snapshot_dict.get("plays", 0)
        discards = snapshot_dict.get("discards", 0)
        money = snapshot_dict.get("money", 0)
        jokers = snapshot_dict.get("jokers", [])

        if stage == "Stage_PreBlind":
            return "select_blind_0", "Enter the blind to start playing."

        if stage == "Stage_Blind":
            return self._decide_blind(available, selected_slots, score, required,
                                       plays, discards, jokers, legal_actions)

        if stage == "Stage_PostBlind":
            return "cashout", f"Cashout to collect reward."

        if stage == "Stage_Shop":
            return self._decide_shop(snapshot_dict, legal_actions)

        # Fallback
        return legal_actions[0] if legal_actions else "select_blind_0", "Fallback action."

    def _decide_blind(self, available, selected_slots, score, required,
                      plays, discards, jokers, legal_actions):
        if not available:
            return legal_actions[0], "No cards available."

        target_indices, est, ht = _best_hand(available)
        target_set = set(target_indices)
        need_select = target_set - selected_slots
        need_deselect = selected_slots - target_set

        hand_desc = HAND_NAMES[ht] if ht < len(HAND_NAMES) else "?"
        remaining = required - score

        # Should we discard instead?
        if plays >= 2 and discards > 0 and est < remaining * 0.3 and not selected_slots:
            # Bad hand, discard worst cards
            sorted_by_value = sorted(range(len(available)), key=lambda i: available[i].get("rank", ""))
            discard_targets = sorted_by_value[:3]
            for idx in discard_targets:
                act = f"select_card_{idx}"
                if act in legal_actions and idx not in selected_slots:
                    reason = (f"Hand quality low ({hand_desc} ~{est}pts vs {remaining} needed). "
                              f"Selecting card {idx} to discard for better draw.")
                    return act, reason
            if "discard" in legal_actions and selected_slots:
                return "discard", f"Discarding {len(selected_slots)} weak cards to draw better ones."

        # Deselect unwanted
        if need_deselect:
            idx = next(iter(need_deselect))
            act = f"select_card_{idx}"
            if act in legal_actions:
                return act, f"Deselecting card {idx} (not part of best {hand_desc})."

        # Select needed
        if need_select:
            idx = next(iter(need_select))
            act = f"select_card_{idx}"
            if act in legal_actions:
                card = available[idx] if idx < len(available) else {}
                rank = card.get("rank", "?")
                suit = card.get("suit", "?")
                return act, f"Selecting {rank} of {suit} for {hand_desc} (~{est}pts)."

        # All selected, play
        if "play" in legal_actions and selected_slots:
            reason = (f"Playing {hand_desc} with {len(selected_slots)} cards. "
                      f"Estimated {est}pts. Need {remaining} more to clear blind. "
                      f"{plays} plays remaining.")
            return "play", reason

        # Edge case: nothing selected but play is legal (shouldn't happen)
        if "play" in legal_actions:
            return "play", "Playing current selection."

        return legal_actions[0] if legal_actions else "play", "Fallback."

    def _decide_shop(self, snap, legal_actions):
        money = snap.get("money", 0)
        jokers = snap.get("jokers", [])
        shop_jokers = snap.get("shop_jokers", [])

        # Buy a joker if affordable and slots available
        if len(jokers) < 5 and shop_jokers:
            for i, j in enumerate(shop_jokers):
                cost = j.get("buy_cost", j.get("cost", 99))
                if cost <= money:
                    act = f"buy_shop_item_{i}"
                    if act in legal_actions:
                        name = j.get("name", "?")
                        return act, f"Buying {name} for ${cost}. Good addition to build."

        if "next_round" in legal_actions:
            return "next_round", f"Nothing worth buying (${money}). Moving to next round."

        return legal_actions[0] if legal_actions else "next_round", "Fallback."


# ---------------------------------------------------------------------------
# Agent: Claude API
# ---------------------------------------------------------------------------

class ClaudeAgent:
    """Uses Claude API to make decisions with CoT reasoning."""

    def __init__(self, model: str = "claude-sonnet-4-20250514"):
        import anthropic
        self.client = anthropic.Anthropic()
        self.model = model
        self.total_tokens = 0

    def decide(self, snapshot_dict: dict, legal_actions: list[str]) -> tuple[str, str]:
        prompt = serialize_for_llm_prompt(snapshot_dict, legal_actions)

        response = self.client.messages.create(
            model=self.model,
            max_tokens=1024,
            messages=[{"role": "user", "content": prompt}],
        )

        text = response.content[0].text
        self.total_tokens += response.usage.input_tokens + response.usage.output_tokens

        # Parse action from response
        action = self._parse_action(text, legal_actions)
        return action, text

    def _parse_action(self, text: str, legal_actions: list[str]) -> str:
        # Look for "ACTION: xxx"
        for line in text.split("\n"):
            if line.strip().upper().startswith("ACTION:"):
                action_str = line.split(":", 1)[1].strip()
                # Exact match
                if action_str in legal_actions:
                    return action_str
                # Fuzzy match
                for la in legal_actions:
                    if la.lower() == action_str.lower():
                        return la
                # Partial match
                for la in legal_actions:
                    if action_str.lower() in la.lower() or la.lower() in action_str.lower():
                        return la

        # Fallback: first legal action
        return legal_actions[0] if legal_actions else "select_blind_0"


# ---------------------------------------------------------------------------
# Game runner
# ---------------------------------------------------------------------------

def play_one_game(agent, seed: int, max_steps: int = 1000, verbose: bool = False) -> dict:
    """Play one full game, returning trajectory data."""
    eng = balatro_native.Engine(seed=seed, stake=1)
    trajectory: list[dict] = []
    step = 0
    selected_set: set[int] = set()

    while not eng.is_over and step < max_steps:
        snap = eng.snapshot()
        snap_dict = json.loads(snap.to_json())
        # Track selected_slots from action state
        snap_dict["selected_slots"] = sorted(selected_set)

        acts = [a for a in eng.legal_actions() if a.enabled]
        legal = [a.name for a in acts]

        if not legal:
            break

        action_name, reasoning = agent.decide(snap_dict, legal)

        # Track selected state for toggle actions
        if action_name.startswith("select_card_"):
            idx = int(action_name.split("_")[-1])
            if idx in selected_set:
                selected_set.discard(idx)
            else:
                selected_set.add(idx)
        elif action_name in ("play", "discard"):
            selected_set.clear()
        elif action_name.startswith("select_blind") or action_name == "cashout" or action_name == "next_round":
            selected_set.clear()

        # Execute
        executed = False
        for a in acts:
            if a.name == action_name:
                eng.step(a.index)
                executed = True
                break
        if not executed:
            eng.step(acts[0].index)
            action_name = acts[0].name

        # Record transition
        new_snap = eng.snapshot()
        new_dict = json.loads(new_snap.to_json())

        state_text = serialize_state(snap_dict, legal)

        record = {
            "step": step,
            "state_text": state_text,
            "reasoning": reasoning,
            "action": action_name,
            "score_before": snap_dict.get("score", 0),
            "score_after": new_dict.get("score", 0),
            "stage_before": snap_dict.get("stage", ""),
            "stage_after": new_dict.get("stage", ""),
            "ante": new_dict.get("ante", 1),
            "money": new_dict.get("money", 0),
        }
        trajectory.append(record)

        if verbose and action_name in ("play", "cashout", "buy_shop_item_0", "buy_shop_item_1",
                                        "next_round", "select_blind_0", "discard"):
            sd = record["score_after"] - record["score_before"]
            if action_name == "play":
                print(f"  Step {step}: PLAY +{sd} ({new_dict['score']}/{new_dict.get('required_score',0)})")
            elif action_name == "cashout":
                print(f"  Step {step}: CASHOUT ${new_dict['money']}")
            elif action_name.startswith("buy_shop"):
                print(f"  Step {step}: BUY (${snap_dict['money']}→${new_dict['money']})")
            elif action_name == "next_round":
                print(f"  Step {step}: NEXT ROUND → Ante {new_dict['ante']}")
            elif action_name == "select_blind_0":
                print(f"  Step {step}: ENTER BLIND (need {new_dict.get('required_score', 0)})")
            elif action_name == "discard":
                print(f"  Step {step}: DISCARD")

        step += 1

    final = json.loads(eng.snapshot().to_json())
    result = {
        "seed": seed,
        "won": bool(eng.is_win),
        "final_ante": final.get("ante", 1),
        "final_money": final.get("money", 0),
        "steps": step,
        "trajectory": trajectory,
    }
    return result


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Collect LLM trajectories for Balatro")
    parser.add_argument("--agent", choices=["claude", "heuristic"], default="heuristic")
    parser.add_argument("--model", type=str, default="claude-sonnet-4-20250514",
                        help="Claude model ID (only used with --agent claude)")
    parser.add_argument("--games", type=int, default=10)
    parser.add_argument("--start-seed", type=int, default=0)
    parser.add_argument("--max-steps", type=int, default=1000)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "results" / "trajectories")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    if args.agent == "claude":
        if not os.environ.get("ANTHROPIC_API_KEY"):
            sys.exit("Error: ANTHROPIC_API_KEY not set. Use --agent heuristic for no-API mode.")
        agent = ClaudeAgent(model=args.model)
        agent_name = f"claude_{args.model.split('-')[1]}"
    else:
        agent = HeuristicAgent()
        agent_name = "heuristic"

    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = args.output_dir / f"{agent_name}_{ts}"
    run_dir.mkdir(parents=True, exist_ok=True)

    print(f"Collecting {args.games} trajectories with {agent_name}")
    print(f"Output: {run_dir}")
    print()

    results: list[dict] = []
    t_start = time.monotonic()

    for i in range(args.games):
        seed = args.start_seed + i
        if args.verbose:
            print(f"--- Game {i+1}/{args.games} (seed={seed}) ---")

        game_result = play_one_game(agent, seed, args.max_steps, args.verbose)
        results.append({
            "seed": game_result["seed"],
            "won": game_result["won"],
            "final_ante": game_result["final_ante"],
            "steps": game_result["steps"],
        })

        # Save individual trajectory
        traj_path = run_dir / f"game_{seed:04d}.json"
        traj_path.write_text(json.dumps(game_result, indent=2, ensure_ascii=False))

        ante = game_result["final_ante"]
        won = "WIN" if game_result["won"] else f"Ante {ante}"
        print(f"  Game {i+1}: seed={seed} → {won} ({game_result['steps']} steps)")

    elapsed = time.monotonic() - t_start

    # Summary
    antes = [r["final_ante"] for r in results]
    wins = sum(1 for r in results if r["won"])
    mean_ante = sum(antes) / len(antes) if antes else 0
    max_ante = max(antes) if antes else 0

    summary = {
        "agent": agent_name,
        "games": args.games,
        "wins": wins,
        "win_rate": wins / args.games if args.games > 0 else 0,
        "mean_ante": round(mean_ante, 2),
        "max_ante": max_ante,
        "ante_distribution": {str(a): antes.count(a) for a in sorted(set(antes))},
        "elapsed_s": round(elapsed, 1),
        "games_per_sec": round(args.games / elapsed, 2) if elapsed > 0 else 0,
        "total_tokens": getattr(agent, "total_tokens", 0),
        "results": results,
    }
    (run_dir / "summary.json").write_text(json.dumps(summary, indent=2))

    print()
    print("=" * 50)
    print(f"COLLECTION COMPLETE: {agent_name}")
    print("=" * 50)
    print(f"  Games:     {args.games}")
    print(f"  Wins:      {wins} ({100*wins/args.games:.1f}%)")
    print(f"  Mean ante: {mean_ante:.2f}")
    print(f"  Max ante:  {max_ante}")
    print(f"  Ante dist: {summary['ante_distribution']}")
    print(f"  Time:      {elapsed:.1f}s ({summary['games_per_sec']:.1f} games/sec)")
    if hasattr(agent, "total_tokens") and agent.total_tokens > 0:
        print(f"  Tokens:    {agent.total_tokens:,}")
    print(f"  Output:    {run_dir}")


if __name__ == "__main__":
    main()
