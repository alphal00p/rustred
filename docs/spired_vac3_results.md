# First complete SpIRed-reference `vac3` sector run

Date: 2026-09-14. This is a reference-port milestone, not completion of the
all-one-through-three-loop PM example goal or publication of a certified
RustRed family artifact.

## Workload and result

Rust independently generated all 38 nonzero sectors of the supplied C++
`vac3.cpp` family, with its symbolic common squared mass `m`, nine ordinary
IBP sources, and numerical seed depth three. Both implementations produced
617 conditional parametric rules and 38 distinct fully fixed residuals.

The current `--release --locked` driver compared **all 617 exact RHS equations,
coordinate guard domains, and sector-sign annotations** using Symbolica,
without specializing the mass. Reference rules were first read after all
generation finished and supplied no seeds or equations to the solver.
Its equations and residual outputs are byte-identical to the first diagnostic
snapshot. An independent output audit also verified:

- identical coordinate cases and within-sector rule order;
- identical exceptional domains on every rule's sector;
- all 1,890 reference symbolic Positive/NonPositive annotations;
- exactly the same 38 residual integral keys;
- the one fully numerical rule in sector `011100`:
  `I(0,1,1,1,0,-1) = I(0,1,1,1,-1,0) - I(-1,1,1,1,0,0)`.

Rust printed 909 raw coordinate guard atoms versus 877 in C++. The extra 32
are outside-sector faces and therefore empty in the relevant domains; the
877 remaining conditions match. The independent audit accepted only literal
monic coordinate equations and rejected unsupported syntax. This is not an
implementation of a general computer algebra system.

The residuals have the same status as the reference's bounded numerical
search output: no claim of independence or minimality. The returned sector
rules have not yet been converted into RustRed's certified artifact format.

## Serial timings

On the same shared AMD EPYC 9754 host, three interleaved Rust/C++ runs gave:

| Metric | Rust port | Original C++ SpIRed |
| --- | --- | --- |
| Process wall times | 0.66, 0.66, 0.66 s | 1.47, 1.47, 1.44 s |
| Median process wall | **0.66 s** | **1.47 s** |
| Generation loop, including output | 663.423, 664.280, 664.344 ms | 1460, 1463, 1438 ms |
| Peak RSS across runs | 6156–6176 KiB | 6200–12368 KiB |

All six runs exited successfully. Oracle comparison was disabled for these
timed repetitions. The observed median process ratio is about 0.45, or a
**2.2× preliminary speedup**. This is not a statistically established bound,
an isolated-core measurement, or proof of superiority across the PM examples.

Both implementations use optimized code and one compute worker. C++ is built
with GCC 14.4.0, `-O3 -DNDEBUG -fopenmp`. Rust's library and driver were built
with Rust 1.97.1 using
`cargo build --release --locked -p rustred --example spired-solve-sector`.
Compilation is outside every timed boundary. Runs use fresh output directories
but are not filesystem-cold; cores were not pinned on the shared host.

Output formats differ: C++ writes both text and a binary rule database;
the Rust driver writes text rules and diagnostics. Consequently these
are application timings, not a precisely matched serialization or solver-core
comparison. The Rust engine is serial here; no Rust six-worker number is
available yet. The original C++ six-worker `vac3` run took 0.31 s.

## Where Rust spent its time

The current full generation-plus-validation run took 0.79 s process wall:
686.680 ms generation, followed by 90.575 ms of exact equation, guard, and
sign validation. Source preparation took 10.048 ms, preconditioning 5.668 ms,
and sector solving 668.992 ms. Validation is excluded from the paired timings.

For phase-level diagnosis, the first snapshot gave the following breakdown;
its generated rules and residuals are identical to the current snapshot.
The initial generation-plus-validation run took 0.77 s process wall. Its
generation loop was 702.776 ms, followed by 59.738 ms of exact RHS validation.

| Phase | Time | Share of sector solving |
| --- | ---: | ---: |
| Symbolic case search | 111.238 ms | 16.3% |
| Exceptional-condition extraction | 7.018 ms | 1.0% |
| Coordinate case geometry | 3.122 ms | 0.5% |
| Bounded numerical-corner search | **557.842 ms** | **81.5%** |
| Other sector-solving overhead | about 5.054 ms | 0.7% |

Source preparation took 3.608 ms; all sector preconditioning took 5.843 ms;
text output took 11.492 ms. The all-positive `111111` sector generated its six
symbolic rules in 0.575 ms of search, then spent 63.022 ms in numerical search.
Do not call that 0.575 ms the complete family-generation time.

The next profiling target is therefore fixed-corner search, not more elaborate
symbolic ordering search or rational reconstruction for this vacuum fixture.
Reducing the numerical depth would change the workload and terminal policy;
it must not be counted as a speedup against the depth-three C++ run.

## Reproduction and evidence

The driver is `examples/rust/spired_solve_sector.rs`. See
[the Rust examples](../examples/rust/README.md) for the current Cargo invocation.
All reference sector manifests are explicit inputs. The current result includes
the LI source-order adapter and full programmatic guard comparator. The solver
suite passed all 77 focused tests before the normal release build.
The release example-comparator suite passed all 12 tests, and the full workspace
check passed. The native Symbolica variable-map migration regression also passed.

Local raw evidence is retained under
`target/spired-vac3-first.ImCaXR/`: `generated/` contains the initial equations,
`stats.tsv`, `residuals.txt`, and `summary.txt`. `cargo-verified/` contains the
current normal-Cargo generation and strict comparison. `cargo-rust-1/` through
`cargo-rust-3/` and `cargo-cpp-1/` through `cargo-cpp-3/` contain the latest
paired repetitions. The earlier `rust-1/` through `rust-3/` and `cpp-1/` through
`cpp-3/` repetitions measured median 0.68 s versus 1.45 s with the first
diagnostic driver. Timing and stdout/stderr logs are adjacent. These generated
files are not committed.
