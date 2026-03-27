# Behavior Log

## Goal

- Replay artifacts must be able to explain what the current AI or rule-based policy did.
- `Behavior Log v1` is attached to replay transitions and rendered in the HTML viewer.
- This is an inspection layer, not hidden chain-of-thought. Logs must describe actual heuristics and actual outcomes.

## Schema

- Replay top-level metadata:
  - `test_metadata.session_id`
  - `test_metadata.test_focus`
  - `test_metadata.started_at`
  - `test_metadata.finished_at`
  - `log_metadata.policy_id`
  - `log_metadata.locales`
  - `log_metadata.default_locale`
  - `log_metadata.terminology_mode`
- Per-transition payload:
  - `step_index`
  - `elapsed_ms`
  - `decision_log.policy_id`
  - `decision_log.rationale_tags`
  - `decision_log.context`
  - `decision_log.en`
  - `decision_log.zh`
- Separate per-seed artifact:
  - append-only `behavior_log.jsonl`
  - each record carries `seed`, `started_at`, `finished_at`, `step_index`, `elapsed_ms`, `action`, localized text, and `test_focus`

## Terminology

- Canonical Balatro nouns stay in English in both locales:
  - `Small Blind`
  - `Big Blind`
  - `Boss Blind`
  - `Pair`
  - `Flush`
  - `Straight`
  - `chips`
  - `Mult`
  - `Cash Out`
  - `Ante`
  - `Stake`
  - `Joker`
  - `reroll`
- Chinese mode may translate surrounding explanation, but should not invent unofficial local names for canonical game nouns.

## Minimum Output

- `simple_rule_v1` must emit a decision log for every transition.
- Every replay and coverage artifact must preserve run-level timestamps.
- Every log must include:
  - chosen action
  - reason
  - outcome
  - numeric context
- Viewer must support `EN` and `ZH` locale toggle and fall back gracefully when a replay has no `decision_log`.

## Checklist

- Completed:
  - replay JSON embeds bilingual decision logs
  - viewer renders locale-switched decision logs
  - simple rule-based policy produces meaningful actions beyond raw card toggles
- In progress:
  - improve policy quality beyond base-score heuristics
  - align future logs with fuller Joker and Boss Blind parity
- Next:
  - add live log streaming for websocket snapshots
  - promote the policy/logging layer into richer RL debugging tools
- Need human help:
  - none for v1
