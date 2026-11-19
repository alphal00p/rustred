# TWOSUP (gate6 m2 consequence): two campaigns of the same uid at once, host-like ns. A starts first; B (another
# directory, another id) starts 3 s later and runs longer. Since procfix6 every hosting supervisor is non-dumpable:
# does B's supervisor make A's sweeps inconclusive (A's final sweep unproven -> exit 8)? INSIDE innsu.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/twosup; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
P_CRASH=0 P_MEM=0 CASE_MS='[3000,4000]' MAXJOBS=2 CAMP=g6a make_spec $R/a.json $R/a $FC 5 4
P_CRASH=0 P_MEM=0 CASE_MS='[30000,30000]' MAXJOBS=2 CAMP=g6b make_spec $R/b.json $R/b $FC 6 2
$RC run --spec $R/a.json --dir $R/a --plain > $R/a.out 2>&1 & A=$!
sleep 3
$RC run --spec $R/b.json --dir $R/b --plain > $R/b.out 2>&1 & B=$!
sleep 1; echo "B supervisor $B: /proc dir owner $(stat -c %u /proc/$B), environ readable by A's uid: $(head -c1 /proc/$B/environ >/dev/null 2>&1 && echo yes || echo NO)"
wait $A; echo "A rc=$? (B still running: $(alive $B && echo yes || echo no))"
grep -h "unreadable\|unproven\|FINISHED\|OPERATOR" $R/a.out | sed 's/^[0-9TZ:-]* //' | cut -c1-200 | sort | uniq -c | head -6 | sed 's/^/  A log: /'
wait $B; echo "B rc=$?"
grep -h "unreadable\|unproven\|FINISHED" $R/b.out | sed 's/^[0-9TZ:-]* //' | cut -c1-160 | sort | uniq -c | head -4 | sed 's/^/  B log: /'
echo "marked A left: $(marked "RR_CAMPAIGN_JOB=$(campid $R/a):" | wc -w), B left: $(marked "RR_CAMPAIGN_JOB=$(campid $R/b):" | wc -w)"
