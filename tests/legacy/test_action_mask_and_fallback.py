from __future__ import annotations

from env.legacy.balatro_gym_wrapper import BalatroEnv


def test_invalid_action_fallback_executes_legal_action() -> None:
    config = {
        "env": {
            "seed": 7,
            "force_mock": True,
            "max_steps": 200,
            "disable_reorder_actions": True,
        },
        "reward": {
            "use_score_shaping": True,
        },
    }
    env = BalatroEnv(config)
    obs, info = env.reset(seed=7)
    del obs, info

    # Intentionally illegal at Stage_PreBlind
    _obs, _reward, terminated, truncated, info = env.step(0)
    assert not terminated
    assert not truncated
    assert info["fallback_used"] is True
    assert info["executed_action"] != 0
