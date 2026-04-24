from __future__ import annotations

import json
import zipfile
from pathlib import Path

import numpy as np

from agents.simple_rule_agent import SimpleRuleAgent
from env.legacy.balatro_gym_wrapper import ParallelBalatroEnvs


def load_bundle() -> dict:
    return json.loads(Path("fixtures/ruleset/balatro-1.0.1o-full.json").read_text())


def test_game_lua_blind_initialization_matches_source() -> None:
    bundle = load_bundle()
    love_path = Path(bundle["metadata"]["source_paths"]["love_path"])
    game_lua_entry = bundle["metadata"]["source_paths"]["game_lua_entry"]
    with zipfile.ZipFile(love_path) as zf:
        game_lua = zf.read(game_lua_entry).decode("utf-8", errors="ignore")

    assert "blind_states = {Small = 'Select', Big = 'Upcoming', Boss = 'Upcoming'}" in game_lua
    assert "blind_choices = {Small = 'bl_small', Big = 'bl_big'}" in game_lua


def test_simple_rule_agent_selects_current_enabled_blind() -> None:
    agent = SimpleRuleAgent()
    snapshot = {
        "phase": "PreBlind",
        "stage": "Stage_PreBlind",
        "round": 1,
        "ante": 1,
        "stake": 1,
        "blind_name": "Big Blind",
        "boss_effect": "None",
        "score": 0,
        "required_score": 450,
        "plays": 4,
        "discards": 3,
        "money": 7,
        "reward": 4,
        "deck": [],
        "available": [],
        "selected": [],
        "discarded": [],
        "jokers": [],
        "shop_jokers": [],
        "blind_states": {"Small": "Skipped", "Big": "Select", "Boss": "Upcoming"},
        "selected_slots": [],
        "won": False,
        "over": False,
    }
    legal_actions = [
        {"index": 11, "name": "select_blind_1", "enabled": True},
        {"index": 85, "name": "skip_blind", "enabled": True},
    ]
    action, plan = agent.choose_action(snapshot, legal_actions)
    assert action == 11
    assert plan["blind_name"] == "Big Blind"


def test_parallel_env_reset_accepts_explicit_seeds() -> None:
    config = {
        "env": {
            "force_mock": True,
            "disable_reorder_actions": True,
            "include_state_snapshot_in_info": True,
            "max_steps": 32,
        },
        "reward": {"use_score_shaping": True},
    }
    vec_env = ParallelBalatroEnvs(config=config, num_envs=2, seed=0, auto_reset=False)
    _obs, infos = vec_env.reset(seeds=[101, 202])
    assert infos[0]["seed"] == 101
    assert infos[1]["seed"] == 202
    assert infos[0]["state_snapshot"]["blind_name"] == "Small Blind"


def test_parallel_env_active_mask_skips_finished_slots() -> None:
    config = {
        "env": {
            "force_mock": True,
            "disable_reorder_actions": True,
            "include_state_snapshot_in_info": True,
            "max_steps": 32,
        },
        "reward": {"use_score_shaping": True},
    }
    vec_env = ParallelBalatroEnvs(config=config, num_envs=2, seed=0, auto_reset=False)
    obs, infos = vec_env.reset(seeds=[301, 302])
    del obs, infos

    next_obs, rewards, terminated, truncated, infos = vec_env.step(
        np.asarray([10, 10], dtype=np.int64),
        active_mask=np.asarray([True, False], dtype=bool),
    )
    del next_obs, rewards, truncated
    assert not terminated[0]
    assert terminated[1]
    assert infos[1]["skipped_step"] is True
