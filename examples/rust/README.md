# Rust library example

[`two_loop_single_mass_vacuum.rs`](two_loop_single_mass_vacuum.rs) calls
`derive_two_loop_unit_mass_sunset`, checks the complete artifact manifest,
encodes durable bytes, and applies the resulting parametric IBPs to
`I(2,2,1)`.

```bash
cargo run --locked -p rustred-app --example two-loop-single-mass-vacuum
```

The defining output is:

```text
algorithm = rustred.generated.two-loop-unit-mass-sunset.v1
ordinary_sources = 4
closing_rule_cells = 5
source = ordinary-ibp:0:0
source = ordinary-ibp:0:1
source = ordinary-ibp:1:0
source = ordinary-ibp:1:1
target = [2, 2, 1]
master [0, 1, 1]: ... mass_squared_power = -3
master [1, 1, 1]: ... mass_squared_power = -2
```

`durable_bytes` and the full exact coefficient strings are also printed.

## SpIRed reference-port case probe

The experimental [`spired_solve_case.rs`](spired_solve_case.rs) exercises the
new compact reference port directly from the Rust library:

```sh
cargo run --release --locked -p rustred --example spired-solve-case -- 3
cargo run --release --locked -p rustred --example spired-solve-case -- 3 '*,*,*,*,*,1'
```

The first argument selects the one-, two-, or three-loop vacuum test family.
Optional comma-separated coordinates fix indices; `*` leaves an index symbolic.
The run reports preparation, polynomial preconditioning, and case-search times,
then prints one exact candidate equation. Generic cases use 1, 4, or 9 ordinary
IBP sources and find a direct leading-row candidate at the first seed.

**This is a single-case search, not a complete sector or closing artifact.**
Denominator exceptions, boundary cases, and finite terminal classification
belong to the subsequent sector driver. The search is capped at seed depth
three and reports an error on a bounded miss. For example, the all-one K6
corner remains a bounded miss; the example does not silently declare it a master.

If the local C++ `vac3` example has been run, append its rule file to compare
the already-generated candidate against the reference using exact Symbolica
arithmetic:

```sh
cargo run --release --locked -p rustred --example spired-solve-case -- \
  3 '*,*,*,*,*,*' \
  vendor/spired/build-release/benchmarks/vac3-serial-artifacts/111111.dat
```

The comparison explicitly sets the reference mass parameter `m=1`. Expected
success is `reference_rhs = exact match after m=1; guards NOT compared`.
The reference is read only after independent generation and never supplies
sources or search hints. This diagnostic checks equations, not complete guards.

## Automatic SpIRed sector traversal

[`spired_solve_sector.rs`](spired_solve_sector.rs) calls
`SectorSolver::solve_sector_with_observer`: it automatically follows the
exceptional coordinate cases and then solves their fully fixed leaves with a
shared numerical system. Unlike the single-case example, it retains the
symbolic common squared mass `m` so that a `vac3` benchmark matches the original
C++ coefficient field.

After preparing the local SpIRed reference inputs, generate all requested
three-loop sectors with:

```sh
cargo run --release --locked -p rustred --example spired-solve-sector -- \
  3 all /tmp/rustred-vac3-new \
  vendor/spired/examples/zeroSectors_vac3.dat \
  vendor/spired/examples/nonZeroSectors_vac3.dat \
  vendor/spired/build-release/benchmarks/vac3-serial-artifacts 3
```

The output directory must be new. Replace `all` with a six-bit sector mask to
inspect one sector, and replace the reference-directory argument with `-` to
run without oracle comparison. The sector manifests contain only the same
zero/nonzero sector input used by C++; no reference equations inform generation.
The optional last argument bounds symbolic seed depth; `unbounded` removes
that diagnostic limit. Numerical search uses depth three.

Outputs include `<sector>.rules.txt` with exact equations and exceptions,
`residuals.txt`, `family.txt`, `stats.tsv`, and `summary.txt`. Per-sector and
aggregate generation timings are separate from the optional native exact RHS
comparison, which happens only after all sectors have been generated. Set
`RUSTRED_SPIRED_PROGRESS=1` for case-level diagnostics. These are source-port
rules and finite bounded-search residuals, not certified family artifacts or
a proof that the residual integrals are independent masters.
