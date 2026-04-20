#!/usr/bin/env bash
# Idempotent modded-Balatro launcher.
#
#   bash scripts/launch_modded_balatro.sh
#
# Behaviour:
#   1. kill any running Balatro (SIGKILL)
#   2. back up every profile/*.jkr `save.jkr` under $BACKUP_DIR
#   3. launch via the official Lovely launcher under nohup
#   4. poll http://127.0.0.1:12346 health up to 60 s
#   5. exit 0 on success (prints PID + health), 1 on failure (prints reason)
set -euo pipefail

GAME_DIR="${BALATRO_GAME_DIR:-/Users/$USER/Library/Application Support/Steam/steamapps/common/Balatro}"
SAVE_ROOT="${BALATRO_SAVE_DIR:-/Users/$USER/Library/Application Support/Balatro}"
BACKUP_DIR="${BALATRO_BACKUP_DIR:-$HOME/.balatro-save-backup}"
RPC_HOST="${BALATROBOT_HOST:-127.0.0.1}"
RPC_PORT="${BALATROBOT_PORT:-12346}"
HEALTH_TIMEOUT="${BALATRO_HEALTH_TIMEOUT_S:-60}"
LOG_FILE="${BALATRO_LOG_FILE:-/tmp/balatro-lovely.log}"

step() { printf "\033[1;34m[%s]\033[0m %s\n" "$1" "$2"; }
ok()   { printf "\033[1;32m[%s]\033[0m %s\n" "OK" "$1"; }
fail() { printf "\033[1;31m[FAIL]\033[0m %s\n" "$1" >&2; exit 1; }

# 1. prereq checks
[ -x "$GAME_DIR/run_lovely_macos.sh" ] || fail "missing $GAME_DIR/run_lovely_macos.sh (install Lovely first)"
[ -f "$GAME_DIR/liblovely.dylib" ]     || fail "missing $GAME_DIR/liblovely.dylib"
[ -d "$SAVE_ROOT/Mods/smods" ]         || fail "missing Mods/smods (install Steamodded)"
[ -d "$SAVE_ROOT/Mods/balatrobot" ]    || fail "missing Mods/balatrobot (install BalatroBot mod)"

# 2. kill existing
step "1" "kill any running Balatro"
if pkill -9 -f "Balatro.app/Contents/MacOS/love" 2>/dev/null; then
  sleep 1
fi
pgrep -f "Balatro.app/Contents/MacOS/love" >/dev/null && fail "balatro still running after kill"
ok "no balatro running"

# 3. backup saves
step "2" "backup save.jkr files"
mkdir -p "$BACKUP_DIR"
backed_up=0
for pdir in "$SAVE_ROOT"/[0-9]*; do
  [ -d "$pdir" ] || continue
  f="$pdir/save.jkr"
  if [ -f "$f" ]; then
    p=$(basename "$pdir")
    ts=$(date +%Y%m%dT%H%M%S)
    mv "$f" "$BACKUP_DIR/profile${p}-save-${ts}.jkr"
    echo "    backed up profile $p -> $BACKUP_DIR/profile${p}-save-${ts}.jkr"
    backed_up=$((backed_up+1))
  fi
done
ok "$backed_up save.jkr file(s) backed up"

# 4. launch
step "3" "launch via run_lovely_macos.sh"
: > "$LOG_FILE"
cd "$GAME_DIR"
bash ./run_lovely_macos.sh > "$LOG_FILE" 2>&1 &
launch_pid=$!
disown "$launch_pid" 2>/dev/null || true
sleep 1
bal_pid=$(pgrep -f "Balatro.app/Contents/MacOS/love" | head -1 || true)
[ -n "$bal_pid" ] || fail "balatro process did not start (see $LOG_FILE)"
ok "balatro pid=$bal_pid (launcher pid=$launch_pid)"

# 5. wait for health
step "4" "poll RPC health on $RPC_HOST:$RPC_PORT (timeout ${HEALTH_TIMEOUT}s)"
deadline=$(( $(date +%s) + HEALTH_TIMEOUT ))
while :; do
  now=$(date +%s)
  if [ "$now" -ge "$deadline" ]; then
    fail "RPC never became ready; last balatro pid=$bal_pid; last 40 lines of $LOG_FILE: $(tail -40 "$LOG_FILE" 2>/dev/null)"
  fi
  if ! pgrep -f "Balatro.app/Contents/MacOS/love" >/dev/null; then
    fail "balatro died during boot; see $LOG_FILE"
  fi
  body=$(curl -sS --max-time 2 -X POST "http://$RPC_HOST:$RPC_PORT" \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","method":"health","id":1}' 2>/dev/null || true)
  if echo "$body" | grep -q '"status":"ok"'; then
    ok "RPC healthy: $body"
    break
  fi
  sleep 1
done

# 6. report
step "5" "ready"
echo "  GAME_DIR   = $GAME_DIR"
echo "  SAVE_ROOT  = $SAVE_ROOT"
echo "  BACKUP_DIR = $BACKUP_DIR"
echo "  LOG_FILE   = $LOG_FILE"
echo "  PID        = $bal_pid"
echo "  RPC        = http://$RPC_HOST:$RPC_PORT"
exit 0
