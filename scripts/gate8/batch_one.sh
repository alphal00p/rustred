#!/usr/bin/env bash
# gate8 (a) B1 CLASS one-at-a-time sweep over the recovery / settle / sweep / final-sweep FILE reads (root-mapped ns,
# readable marked daemon of a crashed session) and the lineage /proc reads (host-like ns, non-dumpable daemon).
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8; cd $S
FM='@CDIR@/ledger|@CDIR@/log/|@CDIR@/jobs|@CDIR@/LOCK|@CDIR@/CAMPAIGN|@CDIR@/control|@CDIR@/accepted|@CDIR@/lock|/dev/shm/rr-campaign|/tmp/rustred-campaign|/proc/sys/kernel/random/boot_id|/proc/self/ns|/proc/self/stat'
ALL=open,read,readdir,opendir,stat,readlink
{
for n in $(seq 1 660); do echo "r f24-$n $ALL 24 $n"; done
for n in $(seq 1 2 660); do echo "r f5-$n $ALL 5 $n"; done
for n in $(seq 2 3 660); do echo "r f13-$n $ALL 13 $n"; done
for n in $(seq 3 3 660); do echo "r f2-$n $ALL 2 $n"; done
for n in $(seq 1 200); do echo "r fs-$n read short $n"; done
for n in $(seq 1 20); do echo "r ft-$n readdir trunc $n"; done
} | while read mode tag ops err n; do echo "$mode $tag $ops $err $n $FM"; done > $S/tmp/one_jobs.txt
LM='/proc/@DP@/|/proc/1/'
for n in 1 2 3 4 5 6 7 8 9 10 12 15 20 30 50 80 120 200 300 500 800 1300 2000 3000 5000 8000 12000 16000 20000 23000 25000 26000; do echo "u l24-$n open,read,stat,readdir 24 $n $LM"; done >> $S/tmp/one_jobs.txt
for n in 1 2 3 5 8 13 21 34 55 89 144 233 377 610 987 1597 2584 4181 6765 10946 17711; do echo "u l5-$n open,read,stat,readdir 5 $n $LM"; done >> $S/tmp/one_jobs.txt
echo "jobs: $(wc -l < $S/tmp/one_jobs.txt) start $(date -u +%FT%TZ)"
xargs -P ${PAR:-8} -L 1 $S/scripts/one_job.sh < $S/tmp/one_jobs.txt
echo "=== done $(date -u +%FT%TZ)"
