"""Interactive Balatro simulator REPL.

Boots the ``balatro_native.Engine`` with the requested seed/deck/stake and
drops into a turn-by-turn REPL that prints the serialized state, shows legal
actions with indices, and reads commands from stdin.

Usage:
    python scripts/sim_repl.py --seed 42 --deck red --stake 1 --lang en
    python scripts/sim_repl.py --seed-str DEMO42 --lang zh

Commands (typed at the `>` prompt):
    <N>            apply the N-th legal action (0-indexed)
    s / state      print the raw JSON snapshot
    r / rand       pick a random legal action
    l / lang       toggle language (en <-> zh)
    h / help       show this help
    q / quit       exit cleanly

The CLI only uses stdlib + ``balatro_native`` + ``env.state_serializer`` +
``env.locale`` — no third-party deps.
"""
from __future__ import annotations

import argparse
import json
import random
import sys
from pathlib import Path
from typing import Any

# Make ``env`` importable when running this file directly.
REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

import balatro_native  # type: ignore[import-not-found]

from env.locale import label as _label  # noqa: E402
from env.state_serializer import serialize_state  # noqa: E402


VALID_DECKS = {
    "red", "blue", "yellow", "green", "black", "magic", "nebula", "ghost",
    "abandoned", "checkered", "zodiac", "painted", "anaglyph", "plasma",
    "erratic",
}


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Interactive Balatro simulator REPL.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("--seed", type=int, default=42, help="numeric seed (u64)")
    p.add_argument(
        "--seed-str",
        type=str,
        default="",
        help="alphabetical seed string (overrides numeric seed inside the engine)",
    )
    p.add_argument(
        "--deck",
        type=str,
        default="red",
        help=f"deck key (default red). One of: {sorted(VALID_DECKS)}",
    )
    p.add_argument("--stake", type=int, default=1, help="stake level 1..8 (default 1)")
    p.add_argument(
        "--lang",
        type=str,
        default="en",
        choices=["en", "zh"],
        help="UI language (default en)",
    )
    p.add_argument(
        "--max-steps",
        type=int,
        default=500,
        help="safety cap on total actions (default 500)",
    )
    return p.parse_args(argv)


def _snap_dict(engine: Any) -> dict:
    return json.loads(engine.snapshot().to_json())


def _enabled_actions(engine: Any) -> list[Any]:
    return [a for a in engine.legal_actions() if a.enabled]


def _print_section_break(step: int, stage: str, lang: str) -> None:
    bar = "=" * 72
    print(bar)
    print(f"step {step:>4}  |  {_label('stage', lang=lang)}: {stage}")
    print(bar)


def _print_legal(actions: list[Any], lang: str) -> None:
    header = _label("legal_actions", lang=lang)
    print(f"-- {header} --")
    for i, a in enumerate(actions):
        print(f"  [{i:>2}] {a.name}")


def _help(lang: str) -> None:
    if lang == "zh":
        msg = [
            "命令:",
            "  <N>         选择第 N 个合法动作",
            "  s / state   打印原始 JSON 快照",
            "  r / rand    随机选择一个合法动作",
            "  l / lang    切换语言 (en <-> zh)",
            "  h / help    显示帮助",
            "  q / quit    退出",
        ]
    else:
        msg = [
            "Commands:",
            "  <N>         apply the N-th legal action",
            "  s / state   print raw JSON snapshot",
            "  r / rand    pick a random legal action",
            "  l / lang    toggle language (en <-> zh)",
            "  h / help    show this help",
            "  q / quit    exit cleanly",
        ]
    print("\n".join(msg))


def _print_summary(snap: dict, steps: int, lang: str) -> int:
    """Print end-of-game summary. Returns shell exit code."""
    won = bool(snap.get("won"))
    score = snap.get("score", 0)
    ante = snap.get("ante", 0)
    money = snap.get("money", 0)
    bar = "#" * 72
    print(bar)
    if lang == "zh":
        verdict = "胜利 🎉" if won else "失败"
        print(f"游戏结束: {verdict}")
        print(f"  大关: {ante} | 分数: {score} | 资金: ${money} | 步数: {steps}")
    else:
        verdict = "WIN" if won else "LOSS"
        print(f"Game over: {verdict}")
        print(f"  Ante: {ante} | Score: {score} | Money: ${money} | Steps: {steps}")
    print(bar)
    return 0 if won else 1


def _read_command(prompt: str) -> str | None:
    """Read one line from stdin. Return None on EOF."""
    try:
        return input(prompt)
    except EOFError:
        return None


def run_repl(args: argparse.Namespace) -> int:
    deck_key = args.deck.lower()
    if deck_key not in VALID_DECKS:
        print(f"warning: unknown deck '{deck_key}'; passing through anyway")

    try:
        engine = balatro_native.Engine(
            seed=args.seed,
            stake=args.stake,
            deck=deck_key,
            seed_str=args.seed_str,
        )
    except Exception as err:  # pragma: no cover - defensive
        print(f"failed to construct engine: {err}", file=sys.stderr)
        return 2

    lang = args.lang
    rng = random.Random(args.seed ^ 0xBA1A7)

    step = 0
    last_stage: str | None = None

    while step < args.max_steps:
        if engine.is_over:
            break

        snap = _snap_dict(engine)
        stage = snap.get("stage", "?")
        actions = _enabled_actions(engine)

        if stage != last_stage:
            _print_section_break(step, stage, lang)
            last_stage = stage

        print(serialize_state(snap, [a.name for a in actions], lang=lang))
        _print_legal(actions, lang)

        if not actions:
            print("no legal actions; ending session")
            break

        cmd = _read_command("> ")
        if cmd is None:
            print()  # newline on EOF
            break
        cmd = cmd.strip()
        if not cmd:
            continue

        if cmd in ("q", "quit", "exit"):
            print("quit")
            break
        if cmd in ("h", "help", "?"):
            _help(lang)
            continue
        if cmd in ("s", "state"):
            print(json.dumps(snap, indent=2, ensure_ascii=False))
            continue
        if cmd in ("l", "lang"):
            lang = "zh" if lang == "en" else "en"
            print(f"-> lang={lang}")
            continue
        if cmd in ("r", "rand"):
            choice = rng.choice(actions)
            print(f"-> random: {choice.name}")
            try:
                engine.handle_action_index(choice.index)
                step += 1
            except Exception as err:
                print(f"error applying action: {err}")
            continue

        # Numeric action index.
        try:
            idx = int(cmd)
        except ValueError:
            print(f"unknown command: {cmd!r} (try 'h' for help)")
            continue
        if idx < 0 or idx >= len(actions):
            print(f"action index out of range: {idx} (valid: 0..{len(actions)-1})")
            continue
        chosen = actions[idx]
        try:
            engine.handle_action_index(chosen.index)
        except Exception as err:
            print(f"error applying action {chosen.name}: {err}")
            continue
        step += 1
    else:
        print(f"max-steps ({args.max_steps}) reached; stopping")

    final = _snap_dict(engine)
    return _print_summary(final, step, lang)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        return run_repl(args)
    except KeyboardInterrupt:
        print("\ninterrupted")
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
