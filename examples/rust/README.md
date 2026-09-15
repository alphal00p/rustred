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

[`spired_solve_sector.rs`](spired_solve_sector.rs) uses `SectorExecutor` to run
`SectorSolver::solve_sector_with_observer`: each sector automatically follows the
exceptional coordinate and admitted affine cases and then solves their fully
fixed leaves with a shared numerical system. Unlike the single-case example, it retains the
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
The optional argument after the reference directory bounds symbolic seed depth;
`unbounded` removes that diagnostic limit. The next worker count defaults to one;
an optional ordering-file path follows it, then an optional schedule:
`active-first` (default) or `input-order`. Use `-` for the ordering-file slot
when selecting a schedule without ordering overrides. Numerical search uses
depth three for vacuum inputs. For six workers, append `6` to the command
above. Use a valid Symbolica license for multicore execution.

The executor shares immutable source polynomials and the zero-sector census,
while every live sector owns its preconditioning, case queue, and GPLU state.
A reusable private Rayon pool bounds compute workers; the one-worker path
runs inline. Default `active-first` scheduling prioritizes sectors with more
active coordinates; `input-order` submits them in manifest order. Neither policy
changes a sector's integral ordering, seed sequence, or internal search. The
selected policy is recorded as `schedule=...` in `family.txt` and `summary.txt`.
Sector files are written as workers finish, while aggregate outputs retain
manifest order. Without reference
comparison, only compact summaries and residual keys survive completed tasks.

For an oracle-free six-worker `vac3` run in manifest order:

```sh
cargo run --release --locked -p rustred --example spired-solve-sector -- \
  3 all /tmp/rustred-vac3-input-order-new \
  vendor/spired/examples/zeroSectors_vac3.dat \
  vendor/spired/examples/nonZeroSectors_vac3.dat - unbounded 6 - input-order
```

Outputs include `<sector>.rules.txt` with exact equations and exceptions,
`residuals.txt`, `family.txt`, `stats.tsv`, and `summary.txt`. Per-sector and
aggregate generation timings are separate from the optional native exact RHS,
guard, and sign comparison, which happens only after all sectors have been generated. Set
`RUSTRED_SPIRED_PROGRESS=1` for case-level diagnostics. These are source-port
rules and finite bounded-search residuals, not certified family artifacts or
a proof that the residual integrals are independent masters.

## Independent vacuum artifact diagnostics

Generate every nonzero sector of a vacuum family and independently check the
original IBP sources, guards, descent, finite terminals, and whole-integer-sector
coverage:

```sh
cargo run --release --locked -p rustred --example spired-artifact-audit -- 3 6
```

The first argument is the loop count (`1`, `2`, or `3`); the second is the
worker budget. The census is derived from the family, without reference rule
files. Expected generation counts are respectively 1, 18, and 617 rules across
1, 4, and 38 nonzero sectors. The table distinguishes stored-guard coverage
from independently replayed, descending coverage and prints every unresolved
obligation. All three full-family audits now pass at one and six workers: every
rule is replayed and descending, `issues=0`, and every uncovered-region column
is zero. A failed audit exits nonzero. **This is a diagnostic, not a shipped
artifact generator:** it neither writes a `ClosedArtifact` nor installs one in
Vakint. See the [artifact integration report](../../docs/spired_vakint_artifact_plan.md)
for the current full-run results and publication boundary.

## Linear-cut PM example

The same executable accepts the supplied two-loop `fam1_11` family:

```sh
cargo run --release --locked -p rustred --example spired-solve-sector -- \
  fam1_11 all /tmp/rustred-fam1-11-new \
  vendor/spired/examples/zeroSectors_1_11.dat \
  vendor/spired/examples/nonZeroSectors_1_11.dat \
  vendor/spired/build-release/benchmarks/serial-artifacts unbounded 6
```

The family data live in [`support/spired_families.rs`](support/spired_families.rs),
not in engine dispatch. It keeps `d,x` symbolic, generates ordinary and Lorentz
identities, derives the two linear-cut preliminary rules, and then specializes
the improved sources to cut powers one. It requests all 40 supplied nonzero
sectors with the reference's numerical depth two. Replace the reference path
with `-` for independent generation without comparison, and `6` with `1` for
serial timing. Reference equations are first read after generation finishes.

Expected output includes `reference_pre_rule_matches=2`,
`reference_rule_matches=802`, `rules=802`, and `finite_residuals=16`.
The [complete comparison](../../docs/spired_fam1_11_results.md) records repeated
serial/multicore release timings and independent residual-key validation.

`pre_rules.txt` records the additional cut rules and their excluded power-one
faces. The exact preliminary-rule, sector-rule, guard, and sign comparisons are
separate from generation timing; residuals remain explicitly bounded-search
residuals. `unbounded` removes the symbolic search-depth cap, not the numerical
depth bound or the requirement for subsequent artifact certification.

## Additional supplied reference families

The same driver accepts `fam1_12`, `fam1_111`, `fam1_112`, `bc4PMRad1`, and
`fam_cosmo`. Their family definitions are ordinary Rust example data; no
topology-name dispatch or stored rule is introduced in the solver. See the
[input census](../../docs/spired_pm_acceptance.md) for the exact workloads.

For the two-loop example with a noninteger power offset, use:

```sh
cargo run --release --locked -p rustred --example spired-solve-sector -- \
  bc4PMRad1 all /tmp/rustred-bc4pm-new \
  vendor/spired/examples/bc4PM_Rad1_zeroSectors.dat \
  vendor/spired/examples/bc4PM_Rad1_nonZeroSectors.dat - unbounded 6
```

Its first physical denominator power is `n0 + ep2`, while sector signs and
seed crossings use the integer coordinate `n0`, as in the reference. The
offset remains symbolic in source coefficients and is recorded in `family.txt`.
There are no removed cuts.
Expected summary: `sectors=38`, `rules=856`, and `finite_residuals=16`.

The cosmology example keeps all three masses and the external square symbolic:

```sh
cargo run --release --locked -p rustred --example spired-solve-sector -- \
  fam_cosmo 10101 /tmp/rustred-cosmo-new - - - unbounded 1
```

When its optional reference directory is supplied, the oracle comparison
explicitly identifies the C++ notation `dot[p,p]` with the independent Rust
parameter `s`. This is notation mapping, not a numerical substitution, and
happens only after Rust generation. One selected sector uses one worker even
if a larger sector-worker budget is requested.
Expected summary: `sectors=1`, `rules=15`, and `finite_residuals=4`.
The [additional-fixture report](../../docs/spired_additional_fixtures.md)
records exact validation, repeated serial/parallel timings, and remaining gaps.

The `fam1_112` reference requires 16 per-sector ordering overrides. Supply the
checked input file after the worker count (and before any optional schedule):

```sh
cargo run --release --locked -p rustred --example spired-solve-sector -- \
  fam1_112 all /tmp/rustred-fam1-112-new \
  vendor/spired/examples/zeroSectors_1_112.dat \
  vendor/spired/examples/neededSectors_1_112.dat - unbounded 6 \
  examples/rust/support/fam1_112_orderings.txt
```

This `fam1_112` command is an input/diagnostic example, **not a claim that its
436 selected sectors already pass**. It and the supplied ordering sweeps
remain part of the outstanding acceptance scope.

The full `fam1_12` and `fam1_111` manifests now pass, including their coupled
exceptional faces. Reproduce them with the same driver:

```sh
cargo run --release --locked -p rustred --example spired-solve-sector -- \
  fam1_12 all /tmp/rustred-fam1-12-new \
  vendor/spired/examples/zeroSectors_1_12.dat \
  vendor/spired/examples/nonZeroSectors_1_12.dat - unbounded 6
cargo run --release --locked -p rustred --example spired-solve-sector -- \
  fam1_111 all /tmp/rustred-fam1-111-new \
  vendor/spired/examples/zeroSectors_1_111.dat \
  vendor/spired/examples/neededSectors_1_111.dat - unbounded 6
```

Expected summaries are `sectors=40`, `rules=1104`, `finite_residuals=32` and
`sectors=132`, `rules=10333`, `finite_residuals=26`. Optional reference paths
replace `-` only for post-generation verification. Affine rule files include
`required = ...` equalities; these constrain applicability without merging
physical integral indices. One empty C++ `fam1_12` branch is deliberately
absent, with an exact divisibility proof. See the
[complete results](../../docs/spired_affine_results.md) for comparisons and limits.
