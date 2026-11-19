# gate7 (g/h) m2-a: can the JOB PROGRAM (which runs after the release) fake "the spawn barrier refused: the program never
# ran"? It writes RUN/sup/BARRIER_REFUSED and exits 125 (BARRIER_NOT_RELEASED) after doing visible work. INSIDE ns().
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
tag=$1; mode=${2:-always}   # always: every attempt fakes; once: only attempt a1 of each job
R=$S/runs/barrier/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R/ran
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ]; then
  run=\$(dirname "\$2"); n=\$(basename "\$run")
  if [ "$mode" = always ] || case "\$n" in *.a1) true;; *) false;; esac; then
    echo "ran \$n \$(date +%s.%N) pid \$\$" >> $R/ran/log      # visible work: the program DID run
    printf 'eof: no release token (faked by the program)\n' > "\$run/sup/BARRIER_REFUSED"
    exit 125
  fi
fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[300,600]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 4 2
timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1; rc=$?
echo "rc=$rc  program runs that faked the refusal: $(wc -l < $R/ran/log)"
echo "ledger job.started records: $(grep -h -c '"ev":"started"' $R/c/ledger/*.jsonl | paste -sd+ | bc)"
grep -h "spawn barrier refused\|never ran\|row 15s\|crash\|attention\|OPERATOR\|FINISHED" $R/run.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-230 | sort | uniq -c | sort -rn | head -8 | sed 's/^/  log: /'
grep -h -o '"reason":"[^"]*row 15s[^"]*"' $R/c/ledger/*.jsonl | head -2 | cut -c1-300 | sed 's/^/  ledger: /'
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
