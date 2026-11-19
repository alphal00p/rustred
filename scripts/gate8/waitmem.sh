#!/usr/bin/env bash
# waitmem.sh UNIT8-ARGS... : wait until host MemAvailable >= 100 GB, then run unit8.sh with the arguments
until [ "$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)" -ge 100 ]; do sleep 20; done
exec /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8/scripts/unit8.sh "$@"
