#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import urllib.request
import zipfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_LOVE = Path.home() / "Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love"
DEFAULT_OUTPUT = ROOT / "fixtures/ruleset/balatro-1.0.1o-full.json"
WIKI_JOKERS_URL = "https://balatrowiki.org/w/Module:Jokers/data?action=raw"
GAME_LUA_ENTRY = "game.lua"
VERSION = "1.0.1o-FULL"

ANTE_BASE_SCORES = [300, 800, 2800, 6000, 11000, 20000, 35000, 50000]
HAND_SPECS = [
    {"key": "high_card", "name": "High Card", "base_chips": 5, "base_mult": 1, "level_chips": 10, "level_mult": 1},
    {"key": "pair", "name": "Pair", "base_chips": 10, "base_mult": 2, "level_chips": 15, "level_mult": 1},
    {"key": "two_pair", "name": "Two Pair", "base_chips": 20, "base_mult": 2, "level_chips": 20, "level_mult": 1},
    {"key": "three_of_kind", "name": "Three of a Kind", "base_chips": 30, "base_mult": 3, "level_chips": 20, "level_mult": 2},
    {"key": "straight", "name": "Straight", "base_chips": 30, "base_mult": 4, "level_chips": 30, "level_mult": 3},
    {"key": "flush", "name": "Flush", "base_chips": 35, "base_mult": 4, "level_chips": 15, "level_mult": 2},
    {"key": "full_house", "name": "Full House", "base_chips": 40, "base_mult": 4, "level_chips": 25, "level_mult": 2},
    {"key": "four_of_a_kind", "name": "Four of a Kind", "base_chips": 60, "base_mult": 7, "level_chips": 30, "level_mult": 3},
    {"key": "straight_flush", "name": "Straight Flush", "base_chips": 100, "base_mult": 8, "level_chips": 40, "level_mult": 4},
    {"key": "five_of_a_kind", "name": "Five of a Kind", "base_chips": 120, "base_mult": 12, "level_chips": 35, "level_mult": 3},
    {"key": "flush_house", "name": "Flush House", "base_chips": 140, "base_mult": 14, "level_chips": 40, "level_mult": 4},
    {"key": "flush_five", "name": "Flush Five", "base_chips": 160, "base_mult": 16, "level_chips": 50, "level_mult": 4},
]

SPRITE_DEFAULTS = {
    "Joker": {"atlas": "resources/textures/1x/Jokers.png", "frame_w": 71, "frame_h": 95},
    "Blind": {"atlas": "resources/textures/1x/BlindChips.png", "frame_w": 68, "frame_h": 68},
    "Stake": {"atlas": "resources/textures/1x/chips.png", "frame_w": 58, "frame_h": 58},
    "Tarot": {"atlas": "resources/textures/1x/Tarots.png", "frame_w": 71, "frame_h": 95},
    "Planet": {"atlas": "resources/textures/1x/Tarots.png", "frame_w": 71, "frame_h": 95},
    "Spectral": {"atlas": "resources/textures/1x/Tarots.png", "frame_w": 71, "frame_h": 95},
    "Voucher": {"atlas": "resources/textures/1x/Vouchers.png", "frame_w": 71, "frame_h": 95},
}

SPRITE_MANIFEST = {
    "resources/textures/1x/Jokers.png": "resources/textures/1x/Jokers.png",
    "resources/textures/1x/BlindChips.png": "resources/textures/1x/BlindChips.png",
    "resources/textures/1x/chips.png": "resources/textures/1x/chips.png",
    "resources/textures/1x/Tarots.png": "resources/textures/1x/Tarots.png",
    "resources/textures/1x/Vouchers.png": "resources/textures/1x/Vouchers.png",
}

SHOP_WEIGHTS = {"common": 70.0, "uncommon": 25.0, "rare": 5.0, "legendary": 0.3}


@dataclass
class Token:
    kind: str
    value: str


class LuaTokenizer:
    def __init__(self, text: str):
        self.text = text
        self.length = len(text)
        self.index = 0

    def tokens(self) -> list[Token]:
        out: list[Token] = []
        while self.index < self.length:
            ch = self.text[self.index]
            if ch.isspace():
                self.index += 1
                continue
            if ch == "-" and self.text[self.index : self.index + 2] == "--":
                self._consume_comment()
                continue
            if ch in "{}[]=(),":
                out.append(Token(ch, ch))
                self.index += 1
                continue
            if ch in "+-*/":
                out.append(Token("op", ch))
                self.index += 1
                continue
            if ch in "'\"":
                out.append(Token("string", self._consume_string(ch)))
                continue
            if ch.isdigit() or (ch == "-" and self._peek_is_number()):
                out.append(Token("number", self._consume_number()))
                continue
            if ch.isalpha() or ch == "_":
                out.append(Token("ident", self._consume_identifier()))
                continue
            raise ValueError(f"unsupported lua token near: {self.text[self.index:self.index+32]!r}")
        return out

    def _consume_comment(self) -> None:
        while self.index < self.length and self.text[self.index] != "\n":
            self.index += 1

    def _consume_string(self, quote: str) -> str:
        self.index += 1
        result: list[str] = []
        while self.index < self.length:
            ch = self.text[self.index]
            self.index += 1
            if ch == "\\" and self.index < self.length:
                result.append(self.text[self.index])
                self.index += 1
                continue
            if ch == quote:
                break
            result.append(ch)
        return "".join(result)

    def _peek_is_number(self) -> bool:
        return self.index + 1 < self.length and self.text[self.index + 1].isdigit()

    def _consume_number(self) -> str:
        start = self.index
        self.index += 1
        while self.index < self.length and (self.text[self.index].isdigit() or self.text[self.index] == "."):
            self.index += 1
        return self.text[start:self.index]

    def _consume_identifier(self) -> str:
        start = self.index
        self.index += 1
        while self.index < self.length and (self.text[self.index].isalnum() or self.text[self.index] == "_"):
            self.index += 1
        return self.text[start:self.index]


class LuaParser:
    def __init__(self, tokens: list[Token]):
        self.tokens = tokens
        self.index = 0

    def parse(self) -> Any:
        return self._parse_value()

    def _current(self) -> Token:
        return self.tokens[self.index]

    def _accept(self, kind: str) -> Token | None:
        if self.index < len(self.tokens) and self.tokens[self.index].kind == kind:
            token = self.tokens[self.index]
            self.index += 1
            return token
        return None

    def _expect(self, kind: str) -> Token:
        token = self._accept(kind)
        if token is None:
            raise ValueError(f"expected {kind}, got {self.tokens[self.index:self.index+4]}")
        return token

    def _parse_value(self) -> Any:
        token = self._current()
        if token.kind == "{":
            return self._parse_table()
        if token.kind == "string":
            self.index += 1
            return token.value
        if token.kind == "number":
            if self._looks_like_expression():
                return self._parse_expression()
            self.index += 1
            return int(token.value) if "." not in token.value else float(token.value)
        if token.kind == "ident":
            if token.value == "true":
                self.index += 1
                return True
            if token.value == "false":
                self.index += 1
                return False
            if token.value == "nil":
                self.index += 1
                return None
            if self._looks_like_expression():
                return self._parse_expression()
            return self._parse_ident_or_call()
        if token.kind == "op" and token.value == "-":
            return self._parse_expression()
        raise ValueError(f"unexpected token {token}")

    def _looks_like_expression(self) -> bool:
        offset = self.index
        if offset >= len(self.tokens):
            return False
        if self.tokens[offset].kind == "op" and self.tokens[offset].value == "-":
            return True
        if self.tokens[offset].kind not in {"number", "ident"}:
            return False
        offset += 1
        return offset < len(self.tokens) and self.tokens[offset].kind == "op"

    def _parse_expression(self) -> Any:
        depth = 0
        parts: list[str] = []
        while self.index < len(self.tokens):
            token = self.tokens[self.index]
            if depth == 0 and token.kind in {",", "}"}:
                break
            if token.kind == "(":
                depth += 1
            elif token.kind == ")":
                depth = max(0, depth - 1)
            parts.append(token.value if token.kind != "string" else repr(token.value))
            self.index += 1
        expression = "".join(parts)
        if re.fullmatch(r"[0-9\.\+\-\*/ ()]+", expression):
            return eval(expression, {"__builtins__": {}}, {})
        return expression

    def _parse_ident_or_call(self) -> Any:
        ident = self._expect("ident").value
        if self._accept("("):
            args: list[Any] = []
            depth = 1
            current: list[Token] = []
            while depth > 0:
                token = self.tokens[self.index]
                self.index += 1
                if token.kind == "(":
                    depth += 1
                elif token.kind == ")":
                    depth -= 1
                    if depth == 0:
                        break
                if depth > 0:
                    current.append(token)
            if current:
                parser = LuaParser(current)
                try:
                    while parser.index < len(parser.tokens):
                        args.append(parser._parse_value())
                        parser._accept(",")
                except Exception:
                    args = [" ".join(token.value for token in current)]
            if ident == "HEX" and args:
                return args[0]
            if len(args) == 1:
                return f"{ident}({args[0]!r})"
            return f"{ident}({', '.join(repr(arg) for arg in args)})"
        return ident

    def _lookahead_is_assignment(self) -> bool:
        if self.index >= len(self.tokens):
            return False
        if self.tokens[self.index].kind == "ident":
            return self.index + 1 < len(self.tokens) and self.tokens[self.index + 1].kind == "="
        if self.tokens[self.index].kind == "[":
            offset = self.index + 1
            depth = 1
            while offset < len(self.tokens) and depth > 0:
                if self.tokens[offset].kind == "[":
                    depth += 1
                elif self.tokens[offset].kind == "]":
                    depth -= 1
                offset += 1
            return offset < len(self.tokens) and self.tokens[offset].kind == "="
        return False

    def _parse_key(self) -> Any:
        if self._accept("["):
            key = self._parse_value()
            self._expect("]")
            self._expect("=")
            return key
        key = self._expect("ident").value
        self._expect("=")
        return key

    def _parse_table(self) -> Any:
        self._expect("{")
        mapping: dict[str, Any] = {}
        array: list[Any] = []
        while not self._accept("}"):
            if self._lookahead_is_assignment():
                key = self._parse_key()
                mapping[str(key)] = self._parse_value()
            else:
                array.append(self._parse_value())
            self._accept(",")
        if mapping and array:
            mapping["__array__"] = array
            return mapping
        if mapping:
            return mapping
        return array


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load_love_entry(love_path: Path, entry: str) -> bytes:
    with zipfile.ZipFile(love_path) as archive:
        return archive.read(entry)


def extract_assignment_block(source: str, anchor: str) -> str:
    anchor_index = source.find(anchor)
    if anchor_index < 0:
        raise ValueError(f"could not find anchor {anchor!r}")
    brace_start = source.find("{", anchor_index)
    if brace_start < 0:
        raise ValueError(f"could not find opening brace after {anchor!r}")
    depth = 0
    quote: str | None = None
    escape = False
    for index in range(brace_start, len(source)):
        ch = source[index]
        if quote is not None:
            if escape:
                escape = False
            elif ch == "\\":
                escape = True
            elif ch == quote:
                quote = None
            continue
        if ch in "\"'":
            quote = ch
            continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return source[brace_start:index + 1]
    raise ValueError(f"unterminated block for {anchor!r}")


def parse_lua_table(source: str) -> Any:
    tokenizer = LuaTokenizer(source)
    parser = LuaParser(tokenizer.tokens())
    return parser.parse()


def fetch_url(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "hack-balatro/0.1"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read()


def sprite_for(set_name: str, pos: Any) -> dict[str, Any] | None:
    defaults = SPRITE_DEFAULTS.get(set_name)
    if defaults is None or not isinstance(pos, dict):
        return None
    return {
        "atlas": defaults["atlas"],
        "x": int(pos.get("x", 0)),
        "y": int(pos.get("y", 0)),
        "frame_w": defaults["frame_w"],
        "frame_h": defaults["frame_h"],
    }


def consume_item_map(raw_map: dict[str, Any], expected_set: str) -> list[dict[str, Any]]:
    out = []
    for item_id, item in raw_map.items():
        if not isinstance(item, dict) or item.get("set") != expected_set:
            continue
        out.append(
            {
                "id": item_id,
                "name": item.get("name", item_id),
                "set": item.get("set", expected_set),
                "order": int(item.get("order", 0) or 0),
                "cost": int(item.get("cost", 0) or 0),
                "config": item.get("config", {}) if isinstance(item.get("config", {}), dict) else {},
                "sprite": sprite_for(expected_set, item.get("pos")),
            }
        )
    return sorted(out, key=lambda item: (item["order"], item["id"]))


def build_bundle(love_path: Path, output_path: Path) -> dict[str, Any]:
    love_bytes = love_path.read_bytes()
    game_lua = load_love_entry(love_path, GAME_LUA_ENTRY).decode("utf-8")
    wiki_raw = fetch_url(WIKI_JOKERS_URL).decode("utf-8")

    stakes = parse_lua_table(extract_assignment_block(game_lua, "self.P_STAKES ="))
    blinds = parse_lua_table(extract_assignment_block(game_lua, "self.P_BLINDS ="))
    centers = parse_lua_table(extract_assignment_block(game_lua, "self.P_CENTERS ="))
    center_jokers = {key: value for key, value in centers.items() if isinstance(value, dict) and value.get("set") == "Joker"}
    wiki_joker_count = len(re.findall(r"^\s*j_[a-z0-9_]+\s*=", wiki_raw, flags=re.MULTILINE))
    if len(center_jokers) != wiki_joker_count:
        raise ValueError(f"wiki joker count {wiki_joker_count} != game.lua joker count {len(center_jokers)}")

    joker_specs = []
    for joker_id, joker in center_jokers.items():
        if not isinstance(joker, dict):
            continue
        joker_specs.append(
            {
                "id": joker_id,
                "order": int(joker.get("order", 0) or 0),
                "name": joker.get("name", joker_id),
                "set": joker.get("set", "Joker"),
                "cost": int(joker.get("cost", 0) or 0),
                "rarity": int(joker.get("rarity", 0) or 0),
                "effect": joker.get("effect"),
                "config": joker.get("config", {}) if isinstance(joker.get("config", {}), dict) else {},
                "unlocked": bool(joker.get("unlocked", False)),
                "blueprint_compat": bool(joker.get("blueprint_compat", False)),
                "perishable_compat": bool(joker.get("perishable_compat", False)),
                "eternal_compat": bool(joker.get("eternal_compat", False)),
                "sprite": sprite_for("Joker", joker.get("pos")),
            }
        )
    joker_specs.sort(key=lambda item: (item["order"], item["id"]))

    blind_specs = []
    for blind_id, blind in blinds.items():
        if not isinstance(blind, dict):
            continue
        boss_meta = blind.get("boss", {}) if isinstance(blind.get("boss"), dict) else {}
        blind_specs.append(
            {
                "id": blind_id,
                "name": blind.get("name", blind_id),
                "order": int(blind.get("order", 0) or 0),
                "dollars": int(blind.get("dollars", 0) or 0),
                "mult": float(blind.get("mult", 0) or 0),
                "boss": bool(boss_meta),
                "showdown": bool(boss_meta.get("showdown", False)),
                "min_ante": int(boss_meta["min"]) if boss_meta.get("min") is not None else None,
                "max_ante": int(boss_meta["max"]) if boss_meta.get("max") is not None else None,
                "debuff": blind.get("debuff", {}) if isinstance(blind.get("debuff", {}), dict) else {},
                "sprite": sprite_for("Blind", blind.get("pos")),
            }
        )
    blind_specs.sort(key=lambda item: (item["order"], item["id"]))

    stake_specs = []
    for stake_id, stake in stakes.items():
        if not isinstance(stake, dict):
            continue
        stake_specs.append(
            {
                "id": stake_id,
                "name": stake.get("name", stake_id),
                "order": int(stake.get("order", 0) or 0),
                "stake_level": int(stake.get("stake_level", 0) or 0),
                "unlocked": bool(stake.get("unlocked", False)),
                "sprite": sprite_for("Stake", stake.get("pos")),
            }
        )
    stake_specs.sort(key=lambda item: (item["stake_level"], item["id"]))

    consumables = []
    for set_name in ("Tarot", "Planet", "Spectral", "Voucher"):
        consumables.extend(consume_item_map(centers, set_name))
    consumables.sort(key=lambda item: (item["set"], item["order"], item["id"]))

    bundle = {
        "metadata": {
            "version": VERSION,
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "source_hashes": {
                "game_lua_sha256": sha256_bytes(game_lua.encode("utf-8")),
                "wiki_jokers_sha256": sha256_bytes(wiki_raw.encode("utf-8")),
                "love_sha256": sha256_bytes(love_bytes),
            },
            "source_paths": {
                "love_path": str(love_path),
                "game_lua_entry": GAME_LUA_ENTRY,
                "wiki_jokers_url": WIKI_JOKERS_URL,
            },
            "sprite_defaults": SPRITE_DEFAULTS,
        },
        "hand_specs": HAND_SPECS,
        "ante_base_scores": ANTE_BASE_SCORES,
        "blinds": blind_specs,
        "stakes": stake_specs,
        "jokers": joker_specs,
        "consumables": consumables,
        "sprite_manifest": SPRITE_MANIFEST,
        "shop_weights": SHOP_WEIGHTS,
    }

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(bundle, ensure_ascii=True, indent=2) + "\n")
    return bundle


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build versioned Balatro ruleset bundle from local .love resources")
    parser.add_argument("--love", type=Path, default=DEFAULT_LOVE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.love.exists():
        print(f"Balatro .love not found: {args.love}", file=sys.stderr)
        return 1
    bundle = build_bundle(args.love, args.output)
    print(f"Wrote {args.output}")
    print(f"  version: {bundle['metadata']['version']}")
    print(f"  jokers: {len(bundle['jokers'])}")
    print(f"  blinds: {len(bundle['blinds'])}")
    print(f"  stakes: {len(bundle['stakes'])}")
    print(f"  consumables: {len(bundle['consumables'])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
