#!/usr/bin/env bash
# gate12 item 6: the process-layer part of the supervisor suite on 4435f6db (debug): each test binary as pid 1 of a private
# user+pid+mount namespace (private /dev/shm, private TMPDIR), --test-threads=4; the lib with the process-layer filters.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12
source $WS/ops/env.sh >/dev/null 2>&1
while read -r name exe; do
  t=$S/tmp/suite-$name-$$; mkdir -p $t
  args=(--test-threads=4); [ $name = lib ] && args=(resources:: launch:: cpu_policy:: --test-threads=4)
  s=$(date +%s)
  out=$(cd $WS/wt/ac-audit-gate12/crates/rustred-campaign && TMPDIR=$t timeout -s KILL 3600 unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=64m tmpfs /dev/shm && exec "$0" "$@"' $exe "${args[@]}" 2>&1); rc=$?
  echo "$out" > $S/logs/suite12_$name.txt
  echo "SUITE $name rc=$rc $(( $(date +%s) - s ))s $(echo "$out" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed; [0-9]* ignored' | tail -1)"
  echo "$out" | grep -E '^test .* FAILED$' | head -5 | sed 's/^/   /'
  case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12/tmp/suite-*) rm -r -- "$t";; esac
done < $S/scripts/suite12_targets.txt
echo "=== suite12 done $(date -u +%FT%TZ)"
