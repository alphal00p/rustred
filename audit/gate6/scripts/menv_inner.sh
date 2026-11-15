# M2 (gate5 item 4), run INSIDE ns(): every rr_campaign verb started with RR_CAMPAIGN_JOB in its environment must refuse
# (exit 1) and create / signal nothing; then the execve environments of a supervisor's non-job helpers (strace -v).
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/menv; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
CASE_MS='[200,400]' P_CRASH=0 make_spec $R/spec.json $R/c $FC 3 3
# a real campaign directory to aim the read verbs at (made from a clean environment)
env -u RR_CAMPAIGN_JOB $RC run --spec $R/spec.json --dir $R/c --plain > $R/ref.out 2>&1; echo "reference run (clean env) rc=$?"
ID=$(campid $R/c)
before=$(find $R -newer $R/ref.out | wc -l)
for val in "$ID:N0.i0000.L0-t0:a1" "other.0123456789abcdef:x:a1" ""; do
  for v in "run --spec $R/spec.json --dir $R/c2 --plain" "run --spec $R/spec.json --dir $R/c --plain" "preflight --spec $R/spec.json" \
           "control $R/c stop" "control $R/c resolve N0.i0000.L0-t0" "control $R/c pause" "verify $R/c" "status $R/c" "watch $R/c --once" \
           "dashboard $R/c --once" "result $R/c" "rebuild-manifest $R/c" "help" "nosuchverb" ""; do
    RR_CAMPAIGN_JOB="$val" timeout -s KILL 30 $RC $v > $R/o.txt 2>&1; rc=$?
    printf '%-4s %-22s %s | %s\n' "$rc" "${val:0:22}" "${v%% *}${v:+ ...}" "$(head -c 90 $R/o.txt | tr '\n' ' ')"
  done
done > $R/verbs.txt
echo "verbs tried: $(wc -l < $R/verbs.txt); exit codes: $(cut -c1-4 $R/verbs.txt | sort | uniq -c | tr '\n' ' ')"
grep -v '^1 ' $R/verbs.txt | head
echo "files created/changed by the refused verbs: $(find $R -newer $R/ref.out ! -name o.txt ! -name verbs.txt | wc -l); c2 exists: $([ -e $R/c2 ] && echo YES || echo no)"
# the hidden barrier verb with the marker set: runs only with a released fd (here: a pipe with 'G')
RR_CAMPAIGN_JOB="$ID:N0:a1" $RC __spawn-barrier 3 -- /bin/echo released 3< <(printf G); echo "barrier verb with 'G': rc=$?"
RR_CAMPAIGN_JOB="$ID:N0:a1" $RC __spawn-barrier 3 -- /bin/echo released 3< /dev/null; echo "barrier verb with EOF: rc=$?"
RR_CAMPAIGN_JOB="$ID:N0:a1" $RC __spawn-barrier 3 -- $RC status $R/c 3< <(printf G) > $R/o.txt 2>&1; echo "barrier verb exec-ing 'rr_campaign status' with the marker: rc=$? ($(head -c 60 $R/o.txt))"
# helpers' environments: every execve made by the supervisor's forks
: gate6 L18: no removal of c3
env -u RR_CAMPAIGN_JOB strace -f -v -qq -s 600 -e trace=execve -e signal=none -o $R/execve.txt $RC run --spec $R/spec.json --dir $R/c3 --plain > $R/c3.out 2>&1; echo "traced run rc=$?"
python3 -I - $R/execve.txt <<'PY'
import sys, re, collections
c = collections.Counter()
for l in open(sys.argv[1]):
    m = re.search(r'execve\("([^"]*)", \[(.*?)\], \[(.*)\]\) = 0', l)
    if not m: continue
    prog, argv, env = m.group(1), m.group(2), m.group(3)
    kind = 'barrier shim' if '__spawn-barrier' in argv else ('supervisor' if '"run"' in argv and prog.endswith('rr_campaign') else prog.split('/')[-1] + ' ' + argv[:40])
    c[(kind, 'MARKED' if 'RR_CAMPAIGN_JOB=' in env else 'clean')] += 1
for k, v in sorted(c.items()): print(f"  execve {k[0]!r}: {k[1]} x{v}")
PY
