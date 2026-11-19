# REG (d), run INSIDE ns(): the release binary has no test hooks, refuses the failpoint variable, ignores RR_HUNT_TRACE;
# which environment variables does it read (strings), and does any of them skip a check?
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
REL=$S/bin/release/rr_campaign; R=$S/runs/reg_d; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
for w in RR_CAMPAIGN_FAILPOINT RR_HUNT_TRACE failpoint "TEST BUILD" signal_after_intent signal_after_send after_spawn no_spawn_barrier after_launch_record before_settle_commit final_sweep_fails ledger_enospc; do
  printf '  strings %-22s %s\n' "$w" "$(strings $REL | grep -c -- "$w")"; done
CASE_MS='[200,400]' P_CRASH=0 make_spec $R/spec.json $R/c $S/bin/release/rr_fake_child 21 4
RR_CAMPAIGN_FAILPOINT=after_spawn:1 $REL run --spec $R/spec.json --dir $R/c --plain > $R/fp.out 2>&1; echo "release with RR_CAMPAIGN_FAILPOINT=after_spawn:1: rc=$? created=$([ -e $R/c ] && echo yes || echo no) $(head -c 120 $R/fp.out)"
RR_CAMPAIGN_FAILPOINT= $REL run --spec $R/spec.json --dir $R/c --plain > $R/fp2.out 2>&1; echo "release with RR_CAMPAIGN_FAILPOINT='': rc=$?"
RR_HUNT_TRACE=1 $REL run --spec $R/spec.json --dir $R/c --plain > $R/ht.out 2>&1; echo "release with RR_HUNT_TRACE=1: rc=$? HUNT lines=$(grep -c HUNT $R/ht.out $R/c/log/supervisor.log 2>/dev/null | awk -F: '{s+=$2} END{print s}')"
echo "MARKED_LEFT=$(marked "RR_CAMPAIGN_JOB=$(campid $R/c):" | wc -w)"
echo "env names referenced by the release binary (getenv candidates):"
strings $REL | grep -oE '^(RR_[A-Z_]+|XDG_[A-Z_]+|DBUS_[A-Z_]+|TERM|NO_COLOR|LC_ALL|LC_CTYPE|LANG|USER|PATH|HOME|SYMBOLICA_LICENSE|COLUMNS|LINES|TMPDIR|SHELL)$' | sort -u | tr '\n' ' '; echo
