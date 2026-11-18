#!/usr/bin/env bash
# procfix7 round 10: freeze the host-step package in ONE command (run it from WS/wt/ac-procfix7 on a CLEAN tree):
#   bash scripts/procfix7/freeze_host_step.sh "<superseded directories and why, one line>"
# It: checks both pause flags (implementer one) and DISK_LOW, a clean tree; builds the release binaries of HEAD through
# light_build.sh; refuses a binary that names RR_CAMPAIGN_FAILPOINT or "TEST BUILD"; creates a NEW directory
# WS/bin/frozen/<UTC>-host-step (refuses an existing one: a frozen directory is never edited); copies the host-tests
# (script, grader, p8, tests, fakes, fixture) and the binaries; writes dry-run.txt from INSIDE a private user+pid namespace
# (the dry-run starts nothing anyway); fills the MANIFEST from scripts/procfix7/MANIFEST.host-step.template; writes
# SHA256SUMS over every other file and checks the coverage; makes the directory read-only; runs the MANIFEST's commands
# (sha256sum -c, both tests with -I -B from the read-only directory); checks that no .pyc appeared; prints the directory and
# the MANIFEST's sha256. It starts no campaign, no unit, no scope, signals nothing. The RUNBOOK (1.7 "Current", 3x) is
# updated by hand afterwards (printed at the end).
set -euo pipefail
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
H=$WS/plan/auto-campaign/host-tests
WT=$WS/wt/ac-audit-gate14
SUPERSEDES=${1:?give the superseded directories and why, as one argument}
cd "$WT"
[ -e "$WS/plan/auto-campaign/implementer_ops/PAUSE" ] && { echo "freeze: implementer PAUSE flag present"; exit 2; }
[ -e "$WS/ops/DISK_LOW" ] && { echo "freeze: DISK_LOW present"; exit 2; }
[ -z "$(git status --porcelain)" ] || { echo "freeze: the tree is not clean"; git status --porcelain; exit 2; }
COMMIT=$(git rev-parse --short=8 HEAD)
# the host-tests copies in git must be the installed ones (the frozen files come from host-tests/)
for f in host_step_fake1.sh grade_host_step.py p8_units.py test_grade_host_step.py test_p8_units.py host_scope_check.sh \
         rehearsal-fake-systemd/systemd-run rehearsal-fake-systemd/systemctl; do
  cmp -s "$H/$f" "scripts/procfix7/host-tests/$f" || { echo "freeze: $H/$f differs from the git copy"; exit 2; }
done
# round 12 (gate15 nit): the fixtures too, against the sha256 list kept in git (every file, no extra file)
( cd "$H" && sha256sum -c --quiet "$WT/scripts/procfix7/host-tests/fixtures.sha256" ) \
  || { echo "freeze: $H/fixtures differ from scripts/procfix7/host-tests/fixtures.sha256"; exit 2; }
diff <(cd "$H" && find fixtures -type f | sort) <(awk '{print $2}' scripts/procfix7/host-tests/fixtures.sha256 | sort) \
  > /dev/null || { echo "freeze: $H/fixtures holds files the git list does not name"; exit 2; }
nice -n 10 "$WS/plan/auto-campaign/implementer_ops/light_build.sh" gate17 build -p rustred-campaign --release --bins
RC=$WT/target/release/rr_campaign; FC=$WT/target/release/rr_fake_child; SC=$WT/target/release/rr_scope_check
# round 11 (gate14 m14-freeze): BOTH binaries are checked (rr_fake_child too)
for b in "$RC" "$FC" "$SC"; do
  for s in RR_CAMPAIGN_FAILPOINT "TEST BUILD"; do
    [ "$(grep -a -c "$s" "$b" || true)" = 0 ] || { echo "freeze: $b names '$s': not a release build"; exit 2; }
  done
done
UTC=$(date -u +%Y%m%dT%H%MZ)
# FROZEN_ROOT: only to validate this script into a scratch directory (default: the real WS/bin/frozen)
FR=${FROZEN_ROOT:-$WS/bin/frozen}
F=$FR/$UTC-host-step
# round 11 (gate14 m14-freeze): ONE atomic mkdir (no -p) creates the directory or fails when it exists (a frozen directory
# is never edited, also not by a second freeze in the same minute); only its parent may be created on the way
[ -d "$FR" ] || mkdir -p "$FR"
mkdir "$F" || { echo "freeze: $F exists (never edited): wait a minute"; exit 2; }
mkdir "$F/bin" "$F/fixtures" "$F/rehearsal-fake-systemd"
cp "$H"/host_step_fake1.sh "$H"/grade_host_step.py "$H"/p8_units.py "$H"/test_grade_host_step.py "$H"/test_p8_units.py \
   "$H"/host_scope_check.sh "$F/"
cp "$H"/rehearsal-fake-systemd/systemd-run "$H"/rehearsal-fake-systemd/systemctl "$F/rehearsal-fake-systemd/"
cp -r "$H/fixtures/host-step-1" "$H/fixtures/host-step-3" "$F/fixtures/"
cp "$RC" "$FC" "$SC" "$F/bin/"
unshare -Urpf --mount-proc -- env -u RR_CAMPAIGN_JOB -u HELPERS REL="$F/bin" bash "$F/host_step_fake1.sh" --dry-run > "$F/dry-run.txt" 2>&1
# round 12: the forking workload (B) too; its spec must carry the helpers
unshare -Urpf --mount-proc -- env -u RR_CAMPAIGN_JOB HELPERS=2 REL="$F/bin" bash "$F/host_step_fake1.sh" --dry-run \
  > "$F/dry-run-fork.txt" 2>&1
grep -q '"honest_helpers":2' "$F/dry-run-fork.txt" || { echo "freeze: dry-run-fork.txt lacks honest_helpers 2"; exit 2; }
grep -q '"honest_helpers":0' "$F/dry-run.txt" || { echo "freeze: dry-run.txt lacks honest_helpers 0"; exit 2; }
RCS=$(sha256sum < "$F/bin/rr_campaign" | cut -c1-16); FCS=$(sha256sum < "$F/bin/rr_fake_child" | cut -c1-16)
SCS=$(sha256sum < "$F/bin/rr_scope_check" | cut -c1-16)
# round 11 (gate14 item 5): the safe host check's binary refuses inside a private namespace (nothing started): checked here
unshare -Urpf --mount-proc -- "$F/bin/rr_scope_check" --unit gate-scopecheck-0123456789abcdef > "$F/scope-check-refusal.txt" 2>&1 \
  && { echo "freeze: rr_scope_check did not refuse inside a namespace"; exit 2; }
grep -q 'refused: inside a private pid namespace' "$F/scope-check-refusal.txt" \
  || { echo "freeze: rr_scope_check's refusal text is missing"; exit 2; }
python3 -I -B - "scripts/procfix7/MANIFEST.host-step.template" "$F/MANIFEST" "$F" "$COMMIT" "$RCS.." "$FCS.." "$SUPERSEDES" \
  "$(date -u +%FT%TZ)" "$SCS.." <<'PY'
import re, sys
tpl, out, d, commit, rc, fc, sup, utc, sc = sys.argv[1:10]
s = open(tpl).read()
for k, v in (("@DIR@", d), ("@COMMIT@", commit), ("@RC_SHA@", rc), ("@FC_SHA@", fc), ("@SC_SHA@", sc), ("@SUPERSEDES@", sup), ("@UTC@", utc)):
    s = s.replace(k, v)
assert not re.search(r"@[A-Z_]+@", s), "an unfilled placeholder in the MANIFEST"
open(out, "w").write(s)
PY
cd "$F"
LIST=$(find . -type f ! -name SHA256SUMS | sort | sed 's|^\./||')
echo "$LIST" | xargs sha256sum > SHA256SUMS
diff <(find . -type f ! -name SHA256SUMS | sed 's|^\./||' | sort) <(awk '{print $2}' SHA256SUMS | sort) \
  || { echo "freeze: SHA256SUMS does not cover every file"; exit 2; }
chmod -R go-rwx,a-w "$F"
chmod u+x,u-w "$F"/rehearsal-fake-systemd/*
sha256sum -c --quiet SHA256SUMS
T=$(mktemp -d "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17/tmp/freeze.XXXXXX")
TMPDIR=$T python3 -I -B test_p8_units.py
TMPDIR=$T python3 -I -B test_grade_host_step.py
case "$T" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17/tmp/freeze.*) rmdir -- "$T" ;; esac
[ "$(find "$F" \( -name '__pycache__' -o -name '*.pyc' \) | wc -l)" = 0 ] || { echo "freeze: a .pyc appeared"; exit 2; }
echo "FROZEN: $F"
echo "MANIFEST sha256: $(sha256sum MANIFEST | cut -c1-64)"
echo "files: $(wc -l < SHA256SUMS) + SHA256SUMS"
echo "BY HAND NOW: RUNBOOK 1.7 'Current' -> $F (supersedes: $SUPERSEDES); RUNBOOK 3x; notes; then a namespace rehearsal"
echo "  (scripts/procfix7/reh_r9b.sh with HTDIR=$F RELDIR=$F/bin, as a detached service unit)."
