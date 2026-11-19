#!/usr/bin/env bash
# gate8 ns8r.sh SCRIPT ARGS... : fresh private user(root-mapped)+pid+mount namespace, a PRIVATE tmpfs on /dev/shm (the
# intent-log fallback stays inside), private TMPDIR. unshare --kill-child: killing unshare tears the namespace down.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9
BASH_BIN=/home/vhirschi/.nix-profile/bin/bash
: "${CPUS:?CPUS must be set by unit8.sh}"
t=$S/tmp/ns.$$.$RANDOM; mkdir -p "$t"
TMPDIR=$t exec taskset -c "$CPUS" unshare -Urpf --mount-proc --kill-child -- $BASH_BIN -c 'mount -t tmpfs -o size=32m,mode=1777 tmpfs /dev/shm && touch /dev/shm/.gate9-private && exec "$0" "$@"' $BASH_BIN "$@"
