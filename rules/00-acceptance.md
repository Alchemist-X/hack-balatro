# Acceptance Rules

## Primary standard

- The acceptance target is **vanilla Balatro `1.0.1o-FULL`**.
- "Done" means the project behaves like Balatro, not merely like a trainable card-game approximation.
- Every implementation loop must leave behind a **human-checkable artifact**:
  - structured replay JSON
  - autoplay HTML animation or equivalent visual playback
  - visible numeric state (score, required score, ante, money, hands, discards)

## Runtime hierarchy

1. `balatro-engine` is the primary execution engine.
2. `balatro-py` exposes the engine to Python.
3. `BalatroEnv` may fall back to `pylatro` or mock only for bring-up or testing.
4. Mock behavior does **not** count toward final acceptance.

## Evidence requirements

- Ruleset bundle version/hash must match the local extracted game files.
- Replay artifacts must show real engine state, not fabricated UI-only data.
- Progress updates must include:
  - completed items
  - current checklist
  - explicit human-help requests, if any
