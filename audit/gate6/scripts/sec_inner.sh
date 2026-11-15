# SEC (gate6 F): a second supervisor on the SAME directory, and a copy taken WHILE the original runs started with
# --adopt-moved-campaign: both must be refused while the original lives, and signal nothing. INSIDE ns().
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts/lib.sh
in_ns_guard6
R=$S/runs/sec; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
make_wrapper $R/wrap.sh daemon
CASE_MS='[3000,5000]' P_CRASH=0 make_spec $R/spec.json $R/c $R/wrap.sh 21 6
$RC run --spec $R/spec.json --dir $R/c --plain > $R/s1.out 2>&1 & S1=$!
for i in $(seq 1 200); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 3 ] && break; sleep 0.1; done
ID=$(campid $R/c); before=$(marked "RR_CAMPAIGN_JOB=$ID:" | sort | tr '\n' ' ')
timeout -s KILL 60 $RC run --spec $R/spec.json --dir $R/c --plain > $R/s2.out 2>&1; echo "second supervisor, same dir: rc=$? $(grep -h -o 'lock[^"]\{0,120\}' $R/s2.out | head -1)"
cp -a $R/c $R/c2
timeout -s KILL 60 $RC run --spec $R/spec.json --dir $R/c2 --plain --adopt-moved-campaign > $R/s3.out 2>&1; echo "copy taken while live, --adopt-moved-campaign: rc=$? $(tail -1 $R/s3.out | cut -c1-160)"
timeout -s KILL 60 $RC run --spec $R/spec.json --dir $R/c2 --plain > $R/s4.out 2>&1; echo "copy taken while live, no adopt: rc=$? $(tail -1 $R/s4.out | cut -c1-160)"
after=$(marked "RR_CAMPAIGN_JOB=$ID:" | sort | tr '\n' ' ')
echo "S1's marked processes unchanged by the refused starts: $([ "$before" = "$(echo $after | tr ' ' '\n' | grep -xF -f <(echo $before | tr ' ' '\n') | tr '\n' ' ')" ] && echo yes || echo "CHECK: before=[$before] after=[$after]")"
echo "signal_sent records in c2 not present in c (copy): $(comm -13 <(grep -h '"signal_sent"' $R/c/ledger/*.jsonl | sort) <(grep -h '"signal_sent"' $R/c2/ledger/*.jsonl | sort) | wc -l)"
wait $S1; echo "S1 rc=$?; MARKED_LEFT=$(marked "RR_CAMPAIGN_JOB=$ID:" | wc -w)"
