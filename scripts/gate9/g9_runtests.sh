#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9
nsrun() { local t=$S/tmp/rt.$$.$RANDOM; mkdir -p $t; TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=32m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
for e in "$@"; do echo "--- $(basename $e)"; nsrun $e --ignored --test-threads=1 2>&1 | grep -E "panicked|exit 0 with|history|test result" | cut -c1-240; done
