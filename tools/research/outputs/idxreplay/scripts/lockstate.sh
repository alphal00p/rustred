#!/usr/bin/env bash
# Receipt: socket1.lock holder/waiters and a socket-1 load sample (read-only; touches no process).
# usage: lockstate.sh OUT [SECONDS]
OUT=$1; S=${2:-30}
LOCK=/common/dev/rustred/TMP/locks/socket1.lock
INO=$(stat -c %i $LOCK)
{
echo "# lockstate receipt, start $(date -u +%FT%TZ); lock $LOCK inode $INO (device 00:2d)"
echo "## /proc/locks lines for the inode (first line = holder, '->' = queued waiters)"
grep -E ":$INO " /proc/locks
echo "## processes (readable to this user) with the lock file open"
for d in /proc/[0-9]*; do p=${d#/proc/}; for f in $d/fd/*; do [ "$(readlink $f 2>/dev/null)" = "$LOCK" ] && echo "$p ${f##*/}"; done; done 2>/dev/null | sort -u |
while read p fd; do echo "pid $p fd $fd | $(ps -o user=,lstart=,args= -p $p | cut -c1-260)"; done
echo "## socket-1 load over ${S} s (busy = total - idle - iowait, in CPUs; this lane runs nothing on these CPUs)"
snap(){ awk -v f=$1 -v l=$2 '/^cpu[0-9]+ /{id=substr($1,4)+0; if(id>=f&&id<=l){t=0;for(i=2;i<=9;i++)t+=$i; B+=t-$5-$6; T+=t}} END{print B, T}' /proc/stat; }
read b1 t1 < <(snap 128 255); read c1 u1 < <(snap 128 227); T0=$(date +%s.%N)
sleep $S
read b2 t2 < <(snap 128 255); read c2 u2 < <(snap 128 227); T1=$(date +%s.%N)
awk -v b=$((b2-b1)) -v c=$((c2-c1)) -v t0=$T0 -v t1=$T1 'BEGIN{d=t1-t0; printf "CPUs 128-255: %.1f busy of 128 (%.0f%%); CPUs 128-227: %.1f busy of 100 (%.0f%%); window %.1f s\n", b/d/100, b/d/100/128*100, c/d/100, c/d/100, d}'
echo "## running threads (state R) on CPUs 128-255 by user, snapshot $(date -u +%FT%TZ)"
ps -eLo user:16,psr,stat --no-headers | awk '$2>=128 && $2<=255 && $3 ~ /^R/ {n[$1]++} END{for(u in n) print n[u], u}' | sort -rn
echo "# end $(date -u +%FT%TZ)"
} > "$OUT" 2>&1
