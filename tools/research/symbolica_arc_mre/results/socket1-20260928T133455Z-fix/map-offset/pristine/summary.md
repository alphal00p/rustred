
### work = full

| variant [layout] | K | runs | CPU/op ns (median) | min-max | x K=1 | x private | busy % | cycles/op | instr/op | IPC | x-CCX fills/op | GHz(u) |
|---|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| shared [c0/m0] {pristine} | 1 | 3 | 14335 | 14321-14534 | 1.00 | - | 100 | 4.46e+04 | 1.24e+05 | 2.78 | 0.0 | 3.10 |
| shared [c0/m0] {pristine} | 96 | 3 | 301673 | 296278-302027 | 21.04 | - | 16 | 9.29e+05 | 1.24e+05 | 0.13 | 36.2 | 3.10 |
| shared [c48/m0] {pristine} | 1 | 3 | 14580 | 14427-14747 | 1.00 | - | 26 | 4.52e+04 | 1.24e+05 | 2.75 | 0.0 | 3.10 |
| shared [c48/m0] {pristine} | 96 | 3 | 124412 | 123527-124986 | 8.53 | - | 10 | 3.85e+05 | 1.24e+05 | 0.32 | 19.4 | 3.10 |

### work = specialize

| variant [layout] | K | runs | CPU/op ns (median) | min-max | x K=1 | x private | busy % | cycles/op | instr/op | IPC | x-CCX fills/op | GHz(u) |
|---|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| shared [c0/m0] {pristine} | 1 | 3 | 7687 | 7669-7742 | 1.00 | - | 17 | 2.38e+04 | 5.55e+04 | 2.33 | 0.0 | 3.10 |
| shared [c0/m0] {pristine} | 96 | 3 | 96049 | 95603-96846 | 12.50 | - | 8 | 2.98e+05 | 5.54e+04 | 0.19 | 11.7 | 3.10 |
| shared [c48/m0] {pristine} | 1 | 3 | 7695 | 7656-7698 | 1.00 | - | 34 | 2.38e+04 | 5.55e+04 | 2.33 | 0.0 | 3.10 |
| shared [c48/m0] {pristine} | 96 | 3 | 84327 | 83129-84604 | 10.96 | - | 3 | 2.6e+05 | 5.54e+04 | 0.21 | 8.0 | 3.10 |
| private-ctx [c48/m0] {pristine} | 1 | 3 | 7725 | 7696-7788 | 1.00 | - | 100 | 2.41e+04 | 5.58e+04 | 2.31 | 0.0 | 3.12 |
| private-ctx [c48/m0] {pristine} | 96 | 3 | 80109 | 78415-81058 | 10.37 | - | 9 | 2.47e+05 | 5.56e+04 | 0.22 | 5.4 | 3.10 |

checksum identity across variants, layouts and repeats: all equal per (work, K)
