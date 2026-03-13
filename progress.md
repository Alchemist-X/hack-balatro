# Progress

## 2026-03-14 00:10 CST

### Acceptance
- Target remains `Balatro 1.0.1o-FULL`.
- Acceptance bar is no longer "trainable mock"; it is "engine behavior converges to vanilla Balatro and stays auditable from extracted sources".
- Visible progress artifact is required for every loop: replay JSON plus an autoplay HTML animation with numeric state.

### Completed
- Added a first-party Rust workspace for `balatro-spec`, `balatro-engine`, and `balatro-py`.
- Generated a versioned ruleset bundle from the local `Balatro.love`.
- Wired the Python Gym wrapper to prefer `balatro_native`.
- Extracted atlas files needed by the viewer.
- Installed `maturin` and built the editable `balatro_native` extension.

### In Progress
- Recording native replays from the Rust engine.
- Exporting standalone autoplay replay HTML for human verification.
- Folding reverse-engineering findings into the repo `rules/` docs.

### Next
- Expand the engine beyond the current deterministic vertical slice into full boss/shop/joker behavior parity.
- Add diff-style scenario fixtures against extracted Lua behavior.
- Replace placeholder joker execution with table-driven effect coverage.

### Checklist
- [x] Native workspace exists
- [x] Ruleset bundle is generated from local game files
- [x] Python can build the native extension
- [x] Viewer can consume structured replay data
- [ ] Replay autoplay artifact generated for this loop
- [ ] Full vanilla blind/joker activation parity
- [ ] Differential validation against more extracted Lua cases

### Need Human Help
- None for the current loop. The repo can already read the local Steam install.

## 2026-03-14 00:08:49

### Completed
- Installed and built the editable balatro_native extension with maturin.
- Verified BalatroEnv now prefers balatro_native instead of mock.
- Recorded a structured native replay to results/replay-latest.json.
- Rendered a standalone autoplay replay to results/replay-latest.html.
- Added repo-level acceptance/source/rough-loop rules under rules/.

### In Progress
- Expanding joker/boss/shop behavior from the current deterministic vertical slice toward full vanilla parity.
- Converting more reverse-engineered Lua behavior into structured tests.

### Next
- Increase engine fidelity for boss-specific debuffs and activation ordering.
- Replace placeholder joker execution with broader table-driven effect coverage.
- Add replay generation for longer scripted runs and better human demos.

### Checklist
- [x] native extension installed
- [x] env backend switched to balatro_native
- [x] replay json generated
- [x] autoplay replay html generated
- [ ] full vanilla joker parity
- [ ] full boss blind parity
- [ ] differential Lua validation suite

### Need Human Help
- None
