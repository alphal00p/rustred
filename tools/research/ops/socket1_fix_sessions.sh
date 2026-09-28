#!/usr/bin/env bash
# Fix-round socket-1 sessions of the W1.4 ops lane (each <= 1 h under
# TMP/locks/socket1.lock, walks on CPUs 128-177 at W50, oracle verifiers on
# CPUs 192-223 at 32 threads; foreign load is recorded per run, not avoided:
# socket 1 is shared, owner answer 2026-09-27).
#   S1: C-5F Ordered W50, release / campaign / campaign+mimalloc, 2 interleaved
#       repeats, strict identity vs TMP/w0/oracle/runs/c5f-ordered/five-finite and
#       the oracle gate on every run (ab_session.sh, ORACLE_ASYNC=1).
#   S2: C-5F under --frontier-policy stop (campaign+mimalloc; 0 frontiers
#       expected, records strict-identical, oracle with the binding substituted
#       from S1's record checkpoint of the same binary); a mid-walk pause/resume
#       under stop (same binary); two cross-binary mid-walk resumes
#       4a17f9c7 -> campaign+mimalloc and 4a17f9c7 -> campaign (record).
#   S3: N3 RSS pilot (directive 0.1.2): C-HOT (hot owner 011101110111000, the
#       full hot box) Ordered W48, campaign on CPUs 128-175 and campaign+mimalloc
#       on 176-223 concurrently, cooperative stop at 45 min; marginal RSS per
#       discovered domain over the second half (rss_fit.py). Ordered makes the
#       two arms' domain sequences identical, so RSS is compared at matched
#       domain counts; concurrent arms perturb CPU timing, not RSS.
#   S2 also runs the release app lib test executable APP_EXE once on 64
#   socket-1 CPUs, so its W50 worker-loop variants (vacuous skips on the 16-CPU
#   lane mask) execute.
# usage: socket1_fix_sessions.sh S1|S2|S3 RELEASE CAMPAIGN MIMALLOC [APP_EXE]
set -u
S=$1; REL=$2; CAM=$3; MI=$4; APP=${5:-}
ROOT=/common/dev/rustred
OPS=$ROOT/.claude/worktrees/agent-ade877816b107b1cf/tools/research/ops
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
REF=$ROOT/TMP/w0/oracle/runs/c5f-ordered/five-finite
LEGACY=$ROOT/TMP/fable51-controls/bin/rustred-4a17f9c7
LOG=$ROOT/TMP/w1-ops/runs/socket1-fix-$S.log
export TMPDIR=$ROOT/TMP
echo "$(date -u +%FT%TZ) $S queued on socket1.lock" >> $LOG
flock -w 14400 $ROOT/TMP/locks/socket1.lock bash -c '
  S='"$S"'; REL='"$REL"'; CAM='"$CAM"'; MI='"$MI"'; APP='"$APP"'; OPS='"$OPS"'; PY='"$PY"'; REF='"$REF"'; LEGACY='"$LEGACY"'
  ROOT=/common/dev/rustred
  echo "$(date -u +%FT%TZ) $S socket1.lock acquired; MemAvailable $(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo) GiB"
  end=$(( $(date +%s) + 3500 ))
  left() { echo $(( end - $(date +%s) )); }
  if [ "$S" = S1 ]; then
    ORACLE=all ORACLE_ASYNC=1 ORACLE_CPUS=192-223 ORACLE_THREADS=32 timeout $(left) \
      $OPS/ab_session.sh n3r2c5f 2 "release=$REL campaign=$CAM campaignmi=$MI" five-finite 128-177 50
    echo "$(date -u +%FT%TZ) S1 ab_session exit $?"
  elif [ "$S" = S3 ]; then
    R=$ROOT/TMP/w1-ops/runs
    HQ=$ROOT/TMP/qcd-feynman-d9d10-pilot-hot-owner/queries.json
    $PY $OPS/knob_run.py --binary $CAM --family hot-sub --queries $HQ --label n3r2rss-campaign --cpus 128-175 \
      --policy ordered --workers 48 --timeout-seconds 2700 --keep-result &
    $PY $OPS/knob_run.py --binary $MI --family hot-sub --queries $HQ --label n3r2rss-campaignmi --cpus 176-223 \
      --policy ordered --workers 48 --timeout-seconds 2700 --keep-result &
    wait
    $PY $OPS/rss_fit.py $R/n3r2rss-campaign/hot-sub $R/n3r2rss-campaignmi/hot-sub
  else
    R=$ROOT/TMP/w1-ops/runs
    if [ -n "$APP" ]; then
      (cd $ROOT/.claude/worktrees/agent-ade877816b107b1cf/crates/rustred-app && \
        taskset -c 128-191 nice -n 5 $APP > $R/app-lib-socket1-64cpu.log 2>&1)
      echo "APP-LIB 64 CPUs exit $? $(grep -E "^test result:" $R/app-lib-socket1-64cpu.log | tail -1) skip_markers=$(grep -c "^SKIPPED" $R/app-lib-socket1-64cpu.log)"
    fi
    # (a) C-5F under the stop policy (no frontier expected).
    timeout $(left) $PY $OPS/knob_run.py --binary $MI --family five-finite --label n3r2c5f-stop-campaignmi \
      --cpus 128-177 --policy ordered --workers 50 --keep-result --native-args "--frontier-policy stop"
    out=$R/n3r2c5f-stop-campaignmi/five-finite
    nice -n 19 taskset -c 192-223 $PY $ROOT/.claude/worktrees/agent-ade877816b107b1cf/examples/python/compare_walk_records.py \
      --mode strict $REF/result.json $out/result.json --ignore-top frontier_policy --output $out/strict-vs-4a17f9c7.json > /dev/null 2>&1
    echo "STRICT stop: $(grep -o "\"verdict\": *\"[A-Z]*\"" $out/strict-vs-4a17f9c7.json) $(grep -o "\"differing_records\": *[0-9]*" $out/strict-vs-4a17f9c7.json)"
    $PY $OPS/oracle_check.py $out --binding-from $R/n3r2c5f-campaignmi-r1/five-finite/checkpoint \
      --cpus 192-223 --threads 32 | cut -c1-400 &
    # (b) mid-walk pause/resume under stop, same binary.
    timeout $(left) $PY $OPS/pause_resume.py --first $MI --second $MI --family five-finite --label n3r2c5f-stop-pause \
      --cpus 128-177 --workers 50 --stop-at-committed 600000 --native-args "--frontier-policy stop" \
      --reference $REF/result.json --ignore-top uncommitted_inspections frontier_policy \
      --oracle --oracle-cpus 192-223 --oracle-threads 32 \
      --binding-from $R/n3r2c5f-campaignmi-r1/five-finite/checkpoint | cut -c1-600
    wait
    # (c, d) cross-binary mid-walk resumes from the frozen legacy binary (record).
    for pair in campaignmi=$MI campaign=$CAM; do
      [ $(left) -gt 900 ] || { echo "skip $pair: $(left) s left"; continue; }
      arm=${pair%%=*}; bin=${pair#*=}
      timeout $(left) $PY $OPS/pause_resume.py --first $LEGACY --second $bin --family five-finite \
        --label n3r2c5f-resume-4a17f9c7-to-$arm --cpus 128-177 --workers 50 --stop-at-committed 600000 \
        --reference $REF/result.json --oracle --oracle-cpus 192-223 --oracle-threads 32 | cut -c1-600
    done
  fi
  echo "$(date -u +%FT%TZ) $S done; $(left) s of the hour left"
' >> $LOG 2>&1
echo "$(date -u +%FT%TZ) $S exit $?" >> $LOG
