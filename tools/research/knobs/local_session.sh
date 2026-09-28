#!/usr/bin/env bash
# Sequential knob_run.py invocations on the lane's own CPUs (no socket-1 lock;
# every plan line must pin to CPUs within 118-127,256-263 and use < 24 threads).
# Usage: local_session.sh <session-name> <plan-file>
set -uo pipefail
NAME=${1:?name}; PLAN=${2:?plan}
LOG=/common/dev/rustred/TMP/w0/knobs/sessions/$NAME.log
RUNNER=/common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75/tools/research/knobs/knob_run.py
exec >>"$LOG" 2>&1
echo "$(date -u +%FT%TZ) local session start"
while IFS= read -r line; do
  [[ -z "$line" || "$line" == \#* ]] && continue
  echo "$(date -u +%FT%TZ) run: $line"
  eval "nix develop /common/dev/rustred --command python $RUNNER $line"
  echo "$(date -u +%FT%TZ) exit $?"
done < "$PLAN"
echo "$(date -u +%FT%TZ) local session done"
