# Original-source certificate proposal: measured bottleneck and next slice

Status: research/design only, 2026-09-16. No prototype, implementation, speedup
measurement, or new closure claim accompanies this note. The proposed change
must retain the complete original-source replay and every existing guard,
descent, ownership and cold-load gate.

## Actual corrected FG run

After conservative symbolic zero projection fixed the selected sector-106
replay obstruction, the full physical FG release campaign used the external
`examples/input/four_loop_fg.toml`, natural ordering, nonpositive coordinates
8 and 9, and two workers pinned to CPUs 34 and 35. Its native census contains
281 global zero proofs; the physical scope contains 132 zero and 124 nonzero
sectors.

All 124 searches finish by **57.1 s**, returning **9,272 candidate rules and
145 finite terminal candidates**. Exact checking passes 32 sectors, including
all 99 rules of sector 106. At **80.9 s** it begins sector **214**, which has
161 rules, and remains there until the **600 s** bound. The final process
measurements are **600.13 s wall, 651.21 s CPU** (647.10 user + 4.11 system),
**551,840 KiB peak RSS**, and exit status **124**. No artifact is published.

A live **15-second** profiling window while checking sector 214 records
**724 CPU-cycle samples**, with no reported lost samples:

| Inclusive sampled stack | Share |
| --- | ---: |
| Native `SparseRowReducer::add_row` | 97.90% |
| `source_port::ordinary::native::propose` | 95.67% |
| Native multivariate polynomial `gcd` | 64.90% |

These are nested inclusive percentages, not additive phase timings. They
identify the bottleneck in that window, not its share of the entire campaign.
This was a shared-host diagnostic, not an isolated benchmark. The monitor
does not identify which of sector 214's 161 rules was active; the profile
does not justify inventing that ordinal or its matrix dimensions.

Evidence: `/tmp/rustred-safe-projection.DMp3co/fg-generate.stderr`,
`fg-generate.time`, `fg-perf-children.txt`, `fg.perf.data`, and `fg.sh`.
The frozen CLI SHA256 is
`7cd40693f4ac25cb7cbcc43ee3fbbb42757e70acb5fbc61a13a6365aa5508e56`.

## Why the current proposal is expensive

`foundry/artifact/source_port/ordinary.rs::weights` regenerates every ordinary
IBP at each distinct seed in a candidate's retained source trace. For four
loops that is 16 ordinary rows per seed. It asks
`ordinary/native.rs::propose` to express the desired identity in this corpus.

The proposal currently reduces an augmented matrix `[A | I]`: physical
integral columns followed by a distinct identity column for **every** source
row, and a separate desired-row marker. Even physically dependent source rows
become independent in the augmented matrix. Exact elimination therefore keeps
processing them and propagates expanded rational source-combination tails.
This is a structural reason for coefficient/GCD swell; it is not evidence of
a slow or incorrect Symbolica GCD implementation.

This work is separate from modular discovery and selected-rule exact lifting.
Selecting `SemiNumerical` or `SparseTargetOnly` for those discovery phases does
not currently accelerate this independent original-source membership proposal.
The full unprojected replay in `native::verify` remains essential: the earlier
FG failure showed why success in an assumed-sector quotient is not authority.

## Smallest recommended implementation slice

Use a bounded modular support proposal before the existing exact proposal.
Do not change source generation, coefficient translations, mathematical
zero predicates, ordering, or rule payloads in this slice.

1. Build one exact projected corpus for the current proposal quotient, keeping
   the original unprojected rows and the exact source-index mapping intact.
   Preserve every physical column in the registry even when its coefficient
   happens to vanish at the chosen sample.
2. Evaluate coefficients with Symbolica's native finite-field conversion and
   `evaluate_with_coeff_map`. A zero denominator is an unlucky sample, not a
   zero term. Use a fixed, bounded prime/sample schedule for reproducibility.
3. Stream only the **physical** rows into native
   `SparseRowReducer<Zp64>` with `LuLMode::Pattern`. Keep a permanent zero
   sentinel so full physical rank does not suppress the next native L row.
   Track original-row IDs, accepted-U-row IDs and native-L-row IDs separately:
   dependent inputs append L patterns, while empty inputs append neither.
4. Append the desired physical row. Only if it reduces to zero modularly,
   inspect that row's native L dependencies and walk their accepted-row
   ancestors. This produces a candidate source subset, not a certificate.
   Handle an empty desired image explicitly; do not read a stale previous
   L row or conclude that an exactly nonzero desired row is zero.
5. Keep selected original rows in a deterministic order, initially their
   original order. Run the existing exact proposal on this compact subset.
   Expand the resulting weights back into the original request vector, with
   exact zero weights elsewhere.
6. Run **unchanged full unprojected weighted replay** against the full desired
   identity. Continue through original-row normalization, source conditions,
   certificate-pole extraction, exact exceptional-domain coverage, descent,
   and the existing publication/load pipeline.

This needs only structural sparse bookkeeping around native algebra. Existing
code already supplies the critical patterns:

- `solver/discovery.rs::Discovery` tracks accepted U rows versus native L
  input rows and traces dependencies. Its target-pivot-specific API must not
  be abused by inventing a synthetic physical integral. A narrow shared CSR
  dependency helper is preferable to another discovery engine.
- `foundry/completion/spired/modular/kernel.rs::capture_dependent_dependencies`
  handles the dependent-final-row case, including L-row-count assertions and
  the zero sentinel. The new membership trace needs the complete ancestor
  closure, not a target-pivot shortcut imported without proof.
- `solver/search.rs::Probe::evaluate` already uses native numerator and
  denominator evaluation and rejects sampled poles.
- `solver/discovery/variables.rs::FrameVariables` validates coefficient maps
  and performs native context compaction/restoration. It can be narrowly
  factored if needed, without introducing a new coefficient representation.

### Deterministic fallback and guard risks

A failed modular rank test, an unlucky sample, or a compact exact frame that
does not lift is **inconclusive**. Retry within a fixed budget, then use the
existing complete exact proposal. The strict quotient and the speculative
assumed-parent-sign quotient must remain distinct.

Crucially, failure of a pruned proposal's unprojected replay must permit
fallback within the **same quotient** before rejecting the rule or moving
to the other quotient. Different span solutions can behave differently on
coefficient-zero or activation faces. Projection of an individually zero
source product does not authorize multiplying it by an arbitrary pole and
discarding the resulting product; full weighted replay checks precisely this.

A different valid certificate can also introduce new denominator poles even
when its final RHS is unchanged. Those poles must take the existing exact
guard/exception path. They may not be silently dropped, sampled away, or
treated as covered because the old certificate had no such pole. If the new
certificate creates unsupported or uncovered exceptional faces, try a
deterministically bounded alternative or the old unpruned certificate; do not
turn an old success into a false closure or hide missing faces. Equally, do
not misreport one failed certificate as proof that the candidate identity is
invalid.

## Follow-on if compact exact proposals remain expensive

### Recover only one exact source-weight vector

After modularly selecting an independent physical source subset, exact GPLU
can use `LuLMode::Full` on those physical rows and a desired-only marker.
The source rows have marker zero; the desired row has marker one. If exact
membership holds, the desired row reduces to that marker alone. Recover its
source weights by solving `L^T w = e_last`, rather than propagating an identity
tail for every row throughout physical elimination. The desired component
must be checked and normalized; the source weights have the opposite sign.

`solver/discovery/target_only.rs::solve_transposed_lower` already implements
this structural transpose plus native sparse normalization/back-substitution
with square-L, diagonal and pivot checks. Factor/reuse that narrowly rather
than implement another solver. Its current materializer returns a physical
target row, not an original-generator certificate, so the surrounding
membership contract still needs explicit integration and tests.

Dependent exact prefixes, changed pivots, or a surviving physical remainder
must fail the compact proposal. Native GPLU/triangular solving and full replay
remain responsible for all algebra. No speedup is claimed before measurement.

### Native semi-numerical reconstruction of certificate weights

The pinned local Symbolica 3 API now provides
`poly::reconstruction::reconstruct_rational_function_over_q`, taking a
finite-field black box, native variable map, `ReconstructionMethod`,
`ReconstructionOptions`, and `max_primes`. It owns interpolation, CRT,
rational lifting and its cross-prime checks. RustRed must not reimplement
these algorithms.

The existing `solver/discovery/semi_numerical.rs` demonstrates the reusable
integration: cache the complete native GPLU output vector for each
`(prime, point)` while reconstructing scalar coefficients, and restore the
original variable map. For this bottleneck, the reconstructed quantities must
be **original-source weights**, not merely the candidate's physical RHS.
Freeze the compact row support, column ordering and generic pivot contract
across samples; return `None` for poles or unlucky rank/pivot changes rather
than interpolate different underdetermined solutions at different points.

Reconstruct every needed weight, including ones zero at the first probe;
a sampled zero is not exact support authority. Native reconstruction's final
cross-prime acceptance is probabilistic, so exact unprojected replay remains
the decisive authority. Preserve every reconstructed denominator condition.
Budget the shared cache and total work: native `max_probes` is per prime
image, and separate scalar coefficients each have their own reconstruction
budget. It is not automatically an aggregate certificate-work limit.

## Native API audit and acceptance for the next slice

The audit checked workspace resolution (`symbolica = 3.0.0`, local vendor
patch), public implementation, and existing RustRed consumers:

- `vendor/symbolica/lib/numerica/src/tensors/sparse.rs`: `SparseRowReducer`,
  `LuLMode`, `add_row`, `u`, `l`, `pivots`, `back_substitute`, native sparse
  multiplication, and `SparseMatrix::solve`. L patterns and solver operations
  are already present; no custom elimination is needed.
- `vendor/symbolica/src/poly/reconstruction.rs` and
  `poly/reconstruction/rational.rs`: deterministic options, native Q-valued
  reconstruction, budgets, retries and the stated probabilistic final check.
- Existing discovery, target-only and semi-numerical consumers listed above.

Required regressions include unlucky samples/primes, sampled-zero desired
rows, empty/dependent-input L bookkeeping, source-index remapping, repeated
integral columns, full-replay rejection of quotient-only relations, poles on
activation faces, same-quotient fallback, unchanged fixed/affine restrictions,
and deterministic results across worker counts. Keep the sector-106 boundary
reproducer as an adversarial gate. Compare compact versus full exact proposals
by unprojected equality and complete guard coverage, not by assuming identical
certificate weights.

Instrument proposal input rows/columns, modular rank, retained support, L/U
nonzeros, probe attempts, fallback reason, exact-proposal time, full-replay
time and new guard count. First run a bounded sector-214 diagnostic, then
K1/K3/K6 regressions, and then the full physical FG publication/cold-reload
attempt. Do not infer a whole-campaign speedup from a faster modular kernel.
