#!/usr/bin/env bash
# lane arrivals: thaw T165.B shard 1 when T165.A is done, shard 2 when T167 is done (memory: <= 200 GB reserved, host floor)
until [ -e /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2/T165.A/done ]; do sleep 30; done
systemctl --user thaw impl-arrivals-physd-T165B1 && echo "$(date -u +%FT%TZ) (implementer:arrivals): thawed impl-arrivals-physd-T165B1 (T165.A done)" >> /shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/CPU_LEDGER.md
until [ -e /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2/T167/done ]; do sleep 30; done
systemctl --user thaw impl-arrivals-physd-T165B2 && echo "$(date -u +%FT%TZ) (implementer:arrivals): thawed impl-arrivals-physd-T165B2 (T167 done)" >> /shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/CPU_LEDGER.md
