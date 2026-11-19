#!/usr/bin/env bash
# procfix7 round 11 (gate14 item 5, the auditor's design): THE SAFE HOST CHECK of the scope belt's cgroup.kill path on a
# real cgroupfs, for the COORDINATOR (prepared by the implementer, never run by it with --run). Run from the frozen
# directory, on the HOST (the binary refuses inside a private pid namespace), outside any job of the campaign:
#   bash host_scope_check.sh --dry-run     # the refusals + the plan; nothing is started
#   bash host_scope_check.sh --run         # the check (about 10-30 s)
# Optional: REL=<dir with rr_scope_check> (default: bin/ next to this script), OUT=<report dir> (default: a new mktemp
# directory under TMPDIR or /tmp). It uses a NEW unit name gate-scopecheck-<16 hex from /dev/urandom> each time.
# What --run does (rustred_campaign::launch::host_check): ONE scope of that name whose processes all live in their own
# private pid namespace; read-only checks of the belt's record and of live_members(); a safety gate (every pid in the
# scope is the scope's own unshare or lives in its private namespace, else ABORT without writing); ONE cgroup.kill of that
# scope; `systemctl --user stop` / `reset-failed` of that exact unit if anything is left. Nothing else is signalled.
# Afterwards this script checks that the unit is no longer loaded. Exit 0: PASS; 1: FAIL (the report says which step);
# 2: usage.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
REL=${REL:-$here/bin}
case "${1:-}" in
  --dry-run) arg=(); mode="dry run" ;;
  --run) arg=(--run); mode="run" ;;
  *) echo "usage: host_scope_check.sh --dry-run | --run"; exit 2 ;;
esac
[ -x "$REL/rr_scope_check" ] || { echo "no executable $REL/rr_scope_check"; exit 2; }
U=gate-scopecheck-$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')
OUT=${OUT:-$(mktemp -d "${TMPDIR:-/tmp}/scopecheck.XXXXXX")}
echo "unit $U.scope; report $OUT/report.txt"
set +e
"$REL/rr_scope_check" --unit "$U" "${arg[@]}" 2>&1 | tee "$OUT/report.txt"
rc=${PIPESTATUS[0]}
set -e
ls=$(systemctl --user show -p LoadState --value "$U.scope" 2>/dev/null || echo unknown)
echo "after: $U.scope LoadState $ls; rr_scope_check exit $rc" | tee -a "$OUT/report.txt"
if [ "$rc" = 0 ] && [ "$ls" = not-found ]; then
  echo "HOST SCOPE CHECK: PASS ($mode)" | tee -a "$OUT/report.txt"
  exit 0
fi
echo "HOST SCOPE CHECK: FAIL (exit $rc, LoadState $ls)" | tee -a "$OUT/report.txt"
exit 1
