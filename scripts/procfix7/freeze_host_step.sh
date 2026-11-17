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
WT=$WS/wt/ac-procfix7
SUPERSEDES=${1:?give the superseded directories and why, as one argument}
cd "$WT"
[ -e "$WS/plan/auto-campaign/implementer_ops/PAUSE" ] && { echo "freeze: implementer PAUSE flag present"; exit 2; }
[ -e "$WS/ops/DISK_LOW" ] && { echo "freeze: DISK_LOW present"; exit 2; }
[ -z "$(git status --porcelain)" ] || { echo "freeze: the tree is not clean"; git status --porcelain; exit 2; }
COMMIT=$(git rev-parse --short=8 HEAD)
# the host-tests copies in git must be the installed ones (the frozen files come from host-tests/)
for f in host_step_fake1.sh grade_host_step.py p8_units.py test_grade_host_step.py test_p8_units.py \
         rehearsal-fake-systemd/systemd-run rehearsal-fake-systemd/systemctl; do
  cmp -s "$H/$f" "scripts/procfix7/host-tests/$f" || { echo "freeze: $H/$f differs from the git copy"; exit 2; }
done
nice -n 10 "$WS/plan/auto-campaign/implementer_ops/light_build.sh" procfix7 build -p rustred-campaign --release --bins
RC=$WT/target/release/rr_campaign; FC=$WT/target/release/rr_fake_child
for s in RR_CAMPAIGN_FAILPOINT "TEST BUILD"; do
  [ "$(grep -a -c "$s" "$RC" || true)" = 0 ] || { echo "freeze: $RC names '$s': not a release build"; exit 2; }
done
UTC=$(date -u +%Y%m%dT%H%MZ)
# FROZEN_ROOT: only to validate this script into a scratch directory (default: the real WS/bin/frozen)
F=${FROZEN_ROOT:-$WS/bin/frozen}/$UTC-host-step
[ -e "$F" ] && { echo "freeze: $F exists (never edited): wait a minute"; exit 2; }
mkdir -p "$F/bin" "$F/fixtures" "$F/rehearsal-fake-systemd"
cp "$H"/host_step_fake1.sh "$H"/grade_host_step.py "$H"/p8_units.py "$H"/test_grade_host_step.py "$H"/test_p8_units.py "$F/"
cp "$H"/rehearsal-fake-systemd/systemd-run "$H"/rehearsal-fake-systemd/systemctl "$F/rehearsal-fake-systemd/"
cp -r "$H/fixtures/host-step-1" "$F/fixtures/"
cp "$RC" "$FC" "$F/bin/"
unshare -Urpf --mount-proc -- env -u RR_CAMPAIGN_JOB REL="$F/bin" bash "$F/host_step_fake1.sh" --dry-run > "$F/dry-run.txt" 2>&1
RCS=$(sha256sum < "$F/bin/rr_campaign" | cut -c1-16); FCS=$(sha256sum < "$F/bin/rr_fake_child" | cut -c1-16)
python3 -I -B - "scripts/procfix7/MANIFEST.host-step.template" "$F/MANIFEST" "$F" "$COMMIT" "$RCS.." "$FCS.." "$SUPERSEDES" \
  "$(date -u +%FT%TZ)" <<'PY'
import re, sys
tpl, out, d, commit, rc, fc, sup, utc = sys.argv[1:9]
s = open(tpl).read()
for k, v in (("@DIR@", d), ("@COMMIT@", commit), ("@RC_SHA@", rc), ("@FC_SHA@", fc), ("@SUPERSEDES@", sup), ("@UTC@", utc)):
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
T=$(mktemp -d "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/procfix7/tmp/freeze.XXXXXX")
TMPDIR=$T python3 -I -B test_p8_units.py
TMPDIR=$T python3 -I -B test_grade_host_step.py
case "$T" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/procfix7/tmp/freeze.*) rmdir -- "$T" ;; esac
[ "$(find "$F" \( -name '__pycache__' -o -name '*.pyc' \) | wc -l)" = 0 ] || { echo "freeze: a .pyc appeared"; exit 2; }
echo "FROZEN: $F"
echo "MANIFEST sha256: $(sha256sum MANIFEST | cut -c1-64)"
echo "files: $(wc -l < SHA256SUMS) + SHA256SUMS"
echo "BY HAND NOW: RUNBOOK 1.7 'Current' -> $F (supersedes: $SUPERSEDES); RUNBOOK 3x; notes; then a namespace rehearsal"
echo "  (scripts/procfix7/reh_r9b.sh with HTDIR=$F RELDIR=$F/bin, as a detached service unit)."
