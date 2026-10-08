# Finite source-support circuit equations: negative control

## Question and authority

Would preparing the existing verified circuit-change-of-variable identities
for ordinary IBP output columns, rather than only declared/canonical terminals,
find additional terminal relations at unchanged ordinary source depth?

The isolated driver uses the native parametric generator to enumerate the
finite structural support of the same depth-zero seeds. It prepares only
eligible L+1-active-line circuit supports. Zero-coefficient endpoints may enter
this finite preparation superset; only actual queried equations enter the
session. Provider right-hand sides do not recursively grow the query set.
There are no new ordinary seeds, terminal declarations or imported equations.
Every prepared equation retains the existing exact transport authority.

An independent audit approved this test with a new provider binding for each
mode: cached empty lookups must not be reused when changing the provider.

## Implementation and reproducibility

Scratch driver (not a production feature):
`TMP/critical-geometry-20261008/circuit_source_support.rs`.
Driver SHA256:
`4acd3b6918b766305b7eb97017d4e7c1ad0871c143620c03fb4606ae5e30f075`.
It links the existing optimized core library whose SHA256 is
`031f8dca8e66f885d2726699b556b8b3077c0bc90bb122412a03ddaa13a9afb5`.
The thin wrapper is compiled at opt-level1; this is not a new full release CLI
benchmark. All pairs use the same wrapper and engine. Other host users were
active. CPU affinity56–59 is disjoint from the observed production64–95.
Rayon, OpenMP and OpenBLAS are set to one thread.

Compilation, excluded from runtime:

```bash
nix develop --command bash -c 'taskset -c 56-59 rustc --edition=2024 -C opt-level=1 \
  TMP/critical-geometry-20261008/circuit_source_support.rs \
  -L dependency=target/release/deps \
  --extern rustred=target/release/deps/librustred-6639687e357815eb.rlib \
  -o TMP/critical-geometry-20261008/circuit_source_support'
```

The driver takes `INPUT_SESSION OUTPUT_SESSION raw|source-support`.
Each output is create-new; choose fresh names when repeating. Four-loop inputs
are `TMP/terminal-compression-20261008/catalog-{h,fg,bmw,x}-wide-circuit-measured.rrbin`.
Five-loop input is the frozen
`TMP/terminal-compression-20261008/five-loop-standard-v2/circuit/state-0000000000000007.rrbin`.
Evidence is `TMP/critical-geometry-20261008/{parent}-{mode}-measured.{log,time,rrbin}`
and `five-{mode}.{log,time,rrbin}`. The failed expanded five-loop mode produces
no output artifact. GNU time is `/run/current-system/sw/bin/time`; an initial
wrapper using absent `/usr/bin/time` failed before invoking the solver.

## Four-loop results

| Family | Remaining, both modes | Ordinary rows, both | Assistance rows raw / expanded | Preparation raw / expanded (s) | Whole driver raw / expanded (s) |
|---|---:|---:|---:|---:|---:|
| H | 19 | 352 | 2 / 346 | 0.051 / 0.556 | 0.394 / 0.911 |
| FG | 15 | 256 | 2 / 389 | 0.046 / 0.547 | 0.214 / 0.732 |
| BMW | 15 | 272 | 2 / 458 | 0.048 / 0.494 | 0.257 / 0.721 |
| X | 16 | 304 | 2 / 419 | 0.052 / 0.555 | 0.446 / 0.957 |

Whole-driver time includes source-session loading, normalization/provider
preparation, search, output and cold verification. Every original terminal's
warm and cold coefficient map agrees exactly and contains only retained keys.
PeakRSS is approximately6–9MiB raw and15MiB expanded. More auxiliary identities
and more sparse pivots did not improve the terminal count.

## Five-loop results and decision

The raw control independently reaches608 from829 raw /651 normalized labels:
16,275 ordinary rows,338 assistance equations,72,118 columns,280,258 nonzeros.
Input15.041s, preparation3.775s, search21.120s, cold verification36.909s,
whole driver77.049s. These boundaries differ from the previous public-CLI
timing; do not compare77s directly with that CLI's29.650s.

Expanded support fails during preparation at the existing aggregate endpoint
budget:4,100,827 requested versus4,000,000 allowed. GNU time records21.74s wall,
21.55s CPU and965,572KiB peakRSS. It never reaches the search, so it is
**resource-censored**, not evidence that an unrestricted expanded solve leaves
the same608 labels. Do not increase the guard merely to finish this experiment.

Decision: park broad circuit-source expansion. A later reopening requires a
selective proposal that targets demonstrably missing equations without eagerly
constructing the large transport closure. The current default and ordinary
source path remain unchanged. No live campaign or shipped artifact was modified.
