#!/usr/bin/env bash
# gate9 matrix.sh CELLS OUT P : each line "<ns8r.sh|ns8u.sh> <inj|live> TAG MODE G8_k=v..." -> one namespaced run; RESULT lines to OUT
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
cells=$1; out=$2; P=${3:-8}
one() { set -- $1; ns=$1; kind=$2; shift 2
  r=$(timeout -s KILL 700 scripts/$ns scripts/g9_${kind}_inner.sh "$@" 2>&1 | grep -E '^RESULT' | tail -1)
  [ -n "$r" ] || r="RESULT tag=$1 mode=$2 rc=HARNESS daemon_alive=?"
  echo "$r cell=[$*]" >> $out; }
export -f one; export S out
grep -v '^#' $cells | grep . | xargs -d '\n' -P $P -I{} bash -c 'one "$@"' _ {}
echo "=== matrix done $(date -u +%FT%TZ) $(wc -l < $out) results" >> $out
