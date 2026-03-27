from __future__ import annotations

from scripts.audit_replay import audit_replay
from scripts.build_source_oracle import build_oracle
from scripts.diff_replays import build_report


def test_build_source_oracle_contains_expected_sections() -> None:
    oracle = build_oracle()
    assert oracle["states"]["stable"] == [
        "BLIND_SELECT",
        "SELECTING_HAND",
        "ROUND_EVAL",
        "SHOP",
        "GAME_OVER",
    ]
    assert len(oracle["evaluate_play_order"]["refs"]) >= 5
    assert len(oracle["rng_order"]["refs"]) >= 3


def test_diff_replays_reports_field_level_mismatch() -> None:
    left = {
        "transitions": [
            {
                "snapshot_after": {"score": 10, "money": 4},
            }
        ],
        "final_snapshot": {"score": 10},
    }
    right = {
        "transitions": [
            {
                "snapshot_after": {"score": 11, "money": 4},
            }
        ],
        "final_snapshot": {"score": 10},
    }
    report = build_report(left, right, max_mismatches=20)
    assert not report["ok"]
    assert any(mismatch["path"] == "transitions[0].snapshot_after.score" for mismatch in report["mismatches"])


def test_audit_replay_accepts_transient_trace_for_blind_entry() -> None:
    replay = {
        "transitions": [
            {
                "snapshot_before": {
                    "stage": "Stage_PreBlind",
                    "lua_state": "BLIND_SELECT",
                    "money": 4,
                    "reward": 3,
                    "blind_states": {"Small": "Select", "Big": "Upcoming", "Boss": "Upcoming"},
                    "jokers": [],
                },
                "action": {"name": "select_blind_0"},
                "events": [],
                "trace": {
                    "transient_lua_states": ["NEW_ROUND", "DRAW_TO_HAND"],
                    "rng_calls": [
                        {
                            "order": 0,
                            "domain": "deck.shuffle.enter_blind",
                            "kind": "shuffle",
                            "args": {},
                            "result": {},
                        }
                    ],
                    "joker_resolution": [],
                    "retrigger_supported": False,
                    "notes": [],
                },
                "snapshot_after": {
                    "stage": "Stage_Blind",
                    "lua_state": "SELECTING_HAND",
                    "blind_name": "Small Blind",
                    "blind_states": {"Small": "Current", "Big": "Upcoming", "Boss": "Upcoming"},
                    "jokers": [],
                    "over": False,
                },
            }
        ],
        "final_snapshot": {
            "stage": "Stage_Blind",
            "lua_state": "SELECTING_HAND",
        },
    }
    result = audit_replay(replay)
    assert result["hard_invariants_ok"] is True
    assert result["summary"]["hard_error_count"] == 0
