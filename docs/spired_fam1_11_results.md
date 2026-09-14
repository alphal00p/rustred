# Complete `fam1_11` reference-port result

Date: 2026-09-14. RustRed independently reproduces the supplied two-loop PM
example at every tested worker count. This accepts a complete `solveSector`
workload, **not** a certified closing artifact or the remaining PM examples.

## Exact workload and result

The family has two loops, three external momenta, nine propagator/ISP
coordinates, two linear cuts, and symbolic `d,x`. Its Gram matrix includes
`q²=-1`, `u1²=u2²=1`, and `u1·u2=(1+x²)/(2x)`. The
[input census](spired_pm_acceptance.md) specifies all denominators and the
40 nonzero/88 zero sectors with both cuts present. Exact rules retain all
parameters; no kinematic sampling substitutes for exact arithmetic.

RustRed generates ten ordinary and three Lorentz identities. Generic cut
preparation derives two five-term preliminary rules and leaves nine improved
sources at cut powers one. Symbolic depth is unbounded; numerical depth is two,
as in C++. Only family data and sector manifests enter generation. Reference
equations are first read afterward.

The complete result is **802 sector rules** (798 symbolic and four numerical),
both preliminary cut rules with their power-one exclusions, and **16 finite
residuals**. Every RHS, coordinate guard domain, and sector-sign annotation
matches C++ using native exact Symbolica comparison at 1/2/4/6 workers. Separate
native binary reading confirms every residual integral key, including two
noncorner keys with a power two. Agreement does not prove master independence
or minimality. Unsupported coupled/nonlinear geometry still fails explicitly;
none was silently dropped in this run.

## Release timing

Seven fresh-process pairs per setting alternate Rust/C++ launch order on the
same CPU prefixes of `2,3,4,5,6,8`: distinct physical cores of this shared AMD
EPYC 9754 host. The machine is not isolated. No samples were discarded.

| Workers | Rust campaign median | C++ campaign median | Rust process wall median | C++ process wall median |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 264.459 ms | 739 ms | 290 ms | 750 ms |
| 2 | 151.490 ms | 373 ms | 170 ms | 380 ms |
| 4 | 84.767 ms | 188 ms | 110 ms | 200 ms |
| 6 | 66.119 ms | 131 ms | 90 ms | 140 ms |

Rust is approximately 2.8× faster serially and 2.0× faster at six workers by
campaign medians; its six-worker speedup is 4.0×. Median paired Rust/C++ ratios
are 0.358, 0.395, 0.451, and 0.515. Rust wins all 28 campaign pairs.

Campaign timers include sector solving and output. C++ additionally emits
compressed binary databases; Rust emits text and statistics. These are close,
not byte-for-byte identical workloads. Rust includes pool setup in its campaign
but excludes family/source preparation. Reference comparisons, compilation,
and dependency downloads are excluded. Rust uses `--release --locked`; the
unchanged C++ executable uses GCC 14.4, `-O3 -DNDEBUG`, and OpenMP.

Rust family/source preparation has medians of 24–26 ms across worker settings,
including native initialization (individual samples span roughly 21–27 ms).
GNU time median process CPU times at 1/2/4/6 workers are 260/290/280/290 ms
for Rust and 740/750/750/790 ms for C++. Maximum reported process RSS across
timing samples is 6,228 KiB for Rust and 9,312 KiB for C++. These process
measurements are not an allocation proof or general memory-scaling bound.
GNU time's wall/CPU readings are quantized to 0.01 seconds.
Workers share immutable sources/manifests and consume completed outputs without
retaining full solutions unless oracle comparison was explicitly requested.

## Phase profiling

Median summed serial sector solving is 249.763 ms. Symbolic search accounts
for 179.276 ms (about 72%), numerical search 46.716 ms (19%), exceptions
11.992 ms (5%), and coordinate geometry 6.302 ms (3%). Preconditioning adds
3.843 ms; writes add 9.536 ms. Phase medians need not sum to the median total.
There are 15,875 seeded symbolic rows and 20 numerical cases, four of which
reduce. Several slowest sectors take roughly 18–20 ms each.

Unlike `vac3`, this workload is symbolic-search dominated. The counters do not
identify GCD, translation, GPLU, or exact replay as the deepest hotspot; native
primitive-level profiling should precede further optimization. No rational
polynomial reconstruction algorithm was implemented.

## Validation and evidence

All 108 focused solver tests pass, including independent derivative fixtures,
fixed-coordinate seeding, native denominator clearing, and ordering. The
ordering tests include 120,000 comparisons against compiled C++.
All 16 example reference-comparator tests also pass in release mode, including
pre-rule guard validation and exact comparison with fixed-source variable maps.
`cargo check --locked --workspace` passes for the Rust library, application,
and Python extension crates; the existing unused-method warning is unchanged.

All 60 PM processes—four validation runs and 28 timing pairs—exit zero with
empty stderr. Across 32 Rust runs, 1,280 sector files, 32 pre-rule files, and
32 residual files are byte-identical. All 512 residual records match the native
C++ census. All 2,324 new C++ output files match the saved reference byte-for-byte.

Release regressions also pass at serial and requested six workers: K1 gives
one rule/one residual, K3 gives 18 rules/four residuals, and all 617 symbolic-mass
`vac3` equations still match C++. These are not a new paired vacuum timing study.

Local ignored evidence is in `target/spired-fam11-first.thOZj8/`: outputs, timing
logs, `benchmark.sh`, and `INDEPENDENT_OUTPUT_AUDIT.md`. The native C++ residual
reader and canonical keys are in `target/spired-fam11-residuals.qPXTP0/`.
Reference source and binary artifacts are not committed. See the
[Rust examples](../examples/rust/README.md) for reproduction instructions.

## Remaining scope

The other supplied PM families, selected three-loop sectors, special orderings,
and ordering sweeps remain required by the active assignment. Generic affine
cases and artifact/Vakint integration remain separate work. Cut preparation
currently rejects noninteger offsets when cuts are removed; `fam1_11` has none.
