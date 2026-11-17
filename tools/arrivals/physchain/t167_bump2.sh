#!/usr/bin/env bash
until [ -e /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2/T165.A/done ]; do sleep 30; done
systemctl --user set-property --runtime impl-arrivals-physd-T167 CPUQuota=4800% && printf '%s (implementer:arrivals): impl-arrivals-physd-T167 CPUQuota raised to 4800%% (T165.A done; shards 96 + T167 48 = 144 cores)\n' "$(date -u +%FT%TZ)" >> /shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/CPU_LEDGER.md
