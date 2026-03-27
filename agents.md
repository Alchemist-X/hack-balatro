# Agents

## Working Baseline

- Follow `Agent-Style.md` as the primary project behavior and fidelity bar.

## Progress Persistence

- Save progress regularly instead of waiting for a "fully done" state.
- Record a timestamp for every progress checkpoint or handoff update.
- Use an explicit timestamp format with timezone, for example `2026-03-28 14:30 CST`.
- Write checkpoint notes into `progress.md` or another user-requested handoff file when the work meaningfully changes state.
- If the latest recorded or pushed checkpoint is older than 12 hours, prioritize syncing the current work to the remote before starting more large changes.
- Remote sync should prefer a small checkpoint commit and push of the current branch, even if the work is still in progress, as long as the state is coherent.
