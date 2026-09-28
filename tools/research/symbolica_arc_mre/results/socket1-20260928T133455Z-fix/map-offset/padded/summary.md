
### work = full

| variant [layout] | K | runs | CPU/op ns (median) | min-max | x K=1 | x private | busy % | cycles/op | instr/op | IPC | x-CCX fills/op | GHz(u) |
|---|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| shared [c0/m0] {padded} | 1 | 3 | 14299 | 14296-14389 | 1.00 | - | 61 | 4.44e+04 | 1.24e+05 | 2.80 | 0.0 | 3.10 |
| shared [c0/m0] {padded} | 96 | 3 | 93475 | 93454-94770 | 6.54 | - | 10 | 2.91e+05 | 1.23e+05 | 0.42 | 16.4 | 3.10 |
| shared [c48/m0] {padded} | 1 | 3 | 14280 | 14245-14398 | 1.00 | - | 78 | 4.43e+04 | 1.24e+05 | 2.79 | 0.0 | 3.10 |
| shared [c48/m0] {padded} | 96 | 3 | 94758 | 94719-96170 | 6.64 | - | 9 | 2.95e+05 | 1.23e+05 | 0.42 | 16.1 | 3.10 |

### work = specialize

| variant [layout] | K | runs | CPU/op ns (median) | min-max | x K=1 | x private | busy % | cycles/op | instr/op | IPC | x-CCX fills/op | GHz(u) |
|---|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| shared [c0/m0] {padded} | 1 | 3 | 7668 | 7611-7712 | 1.00 | - | 46 | 2.37e+04 | 5.54e+04 | 2.33 | 0.0 | 3.10 |
| shared [c0/m0] {padded} | 96 | 3 | 37238 | 36838-37735 | 4.86 | - | 7 | 1.15e+05 | 5.52e+04 | 0.48 | 4.9 | 3.10 |
| shared [c48/m0] {padded} | 1 | 3 | 7702 | 7695-7740 | 1.00 | - | 100 | 2.39e+04 | 5.54e+04 | 2.32 | 0.0 | 3.10 |
| shared [c48/m0] {padded} | 96 | 3 | 37153 | 36646-37324 | 4.82 | - | 6 | 1.15e+05 | 5.52e+04 | 0.48 | 4.9 | 3.10 |
| private-ctx [c48/m0] {padded} | 1 | 3 | 7691 | 7646-7700 | 1.00 | - | 71 | 2.39e+04 | 5.56e+04 | 2.33 | 0.0 | 3.12 |
| private-ctx [c48/m0] {padded} | 96 | 3 | 36947 | 36372-37556 | 4.80 | - | 9 | 1.15e+05 | 5.55e+04 | 0.48 | 2.9 | 3.10 |

checksum identity across variants, layouts and repeats: all equal per (work, K)
