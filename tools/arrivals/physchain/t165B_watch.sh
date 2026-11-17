#!/usr/bin/env bash
# lane arrivals: start T165 top-up part B (T180's file) as soon as T180 is done
until [ -e /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2/T180/done ]; do sleep 30; done
bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/t165_launch2.sh B /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2/T165.B.incoming.txt 16 6
