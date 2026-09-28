#!/usr/bin/env bash
# fix-inputs 2026-09-28: r2 verifier on the 8 C-5F witness arms, 4 at a time on disjoint CPU sets.
cd /common/dev/rustred
V=TMP/w0/inputs/oracle-verify; C=TMP/w0/inputs/i2/c5f
$V/verify.sh 88-97,344-353 20 orig-ord1=$C/orig-ord1/five-finite orig-1=$C/orig-1/five-finite > $V/c5f-a.log 2>&1 &
$V/verify.sh 98-107,354-363 20 i2-ord1=$C/i2-ord1/five-finite i2-1=$C/i2-1/five-finite > $V/c5f-b.log 2>&1 &
$V/verify.sh 108-117,364-373 20 orig-ord2=$C/orig-ord2/five-finite orig-2=$C/orig-2/five-finite > $V/c5f-c.log 2>&1 &
$V/verify.sh 118-127,374-383 20 i2-ord2=$C/i2-ord2/five-finite i2-2=$C/i2-2/five-finite > $V/c5f-d.log 2>&1 &
wait
echo "c5f_all done $(date -u +%FT%TZ)"
