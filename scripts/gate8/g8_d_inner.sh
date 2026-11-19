. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8/scripts/lib.sh
in_ns_guard6; shm_guard
R=$S/runs/d8; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
RR=$S/bin/release/rr_campaign; FR=$S/bin/release/rr_fake_child
P_CRASH=0 CASE_MS='[300,600]' make_spec $R/s1.json $R/c1 $FR 3 4
RR_CAMPAIGN_FAILPOINT=settle_after_commit:1 $RR run --spec $R/s1.json --dir $R/c1 --plain > $R/a.out 2>&1; echo "release, failpoint var set: rc=$? files: $(find $R/c1 -type f 2>/dev/null | sed "s#$R/c1/##" | tr '\n' ' ')"
RR_CAMPAIGN_FAILPOINT= $RR run --spec $R/s1.json --dir $R/c1 --plain > $R/b.out 2>&1; echo "release, failpoint var EMPTY: rc=$?"
RR_HUNT_TRACE=1 $RR run --spec $R/s1.json --dir $R/c1 --plain > $R/c.out 2>&1; echo "release, RR_HUNT_TRACE=1: rc=$? trace lines: $(grep -ci 'hunt trace\|^hunt' $R/c.out)"
RR_CAMPAIGN_JOB=x.0123456789abcdef:J:a1 $RR status $R/c1 > $R/d.out 2>&1; echo "release status with RR_CAMPAIGN_JOB set: rc=$?"
RR_CLOSELOOP_TEST_PASS_DELAY_MS=1 $RR --help > /dev/null 2>&1; echo "release: RR_CLOSELOOP_TEST_PASS_DELAY_MS is read by closeloop/pass.rs run_pass (a delay only)"
