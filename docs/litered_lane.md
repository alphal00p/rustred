# LiteRed lane: modular DAG application, census compression, exact coefficients

Status: **uncertified**. The lane applies saved `family-candidates` formulas,
which carry no source-replay provenance or cover proof. Its basis is not proved
minimal. Use it as a cross-check and as an interim backend. Never feed its
output to closing-artifact installation or publication.

Code: `rustred::solver::litered` (core) and `rustred litered-reduce` (app).

## Admission: `CandidateFamilyAdmission::ParametricVacuum`

`CandidateReducer` normally admits only the publication shape: a unit-mass,
`d`-only vacuum family (`validate_unit_mass_family`). The
`ParametricVacuum` policy bypasses only that predicate. It accepts any uncut,
unshifted vacuum family, including:

- deformed lines, e.g. a constant `-1-eta`;
- extra parameters, e.g. `eta`;
- a numeric dimension.

Entry points:

- `CandidateReducer::try_new_with_admission`;
- `load_generated_candidate_bundle_with_admission`.

The default admission and every certified path are unchanged.

## Pipeline

1. **DAG** (`dag.rs`). The DAG follows the ordinary `CandidateEvaluator`
   dispatch exactly:
   - ordered `Partition` rules, then whole-piece alternatives;
   - fixed coordinates, affine equalities, AND-branch exclusions and every RHS
     denominator;
   - zero-sector children dropped, strict descent checked, exact coalescing.

   Every test runs on Zp images at one sample (prime `2^61-1`, seeded point).
   **Every zero image is confirmed by exact specialization**, and a nonzero
   image is an exact nonzero. The structure is therefore exact and the same
   for every prime and point. Other samples only re-evaluate the edges.
2. **Sweeps** (`modular.rs`). The forward sweep reduces every node onto the
   terminals, with memoization. The adjoint sweep reduces all targets at once.
   The two must agree.
3. **Compression** (`compress.rs`). Rows are exact RustRed census identities
   `I - T(I) = 0`, built for every candidate terminal:
   - `GroupOrbit` (default): the route composed with every element of the
     representative group, obtained by orbit closure;
   - or only the generators.

   The rows are reduced through the DAG, and elimination pivots on the
   **hardest** column. Masters are the non-pivot columns, at the easy end of
   `ColumnOrder::{LinesDotsRank (default), Program}`. Preferred integrals are
   forced to the easy end.
4. **Coefficients** (`session.rs`, `reconstruct.rs`):
   - **at a rational point:** multi-prime CRT plus maximal-quotient rational
     reconstruction;
   - **in Q(x) for one free parameter**, e.g. `eta` at a sampled epsilon:
     early-terminating Thiele interpolation per prime, then CRT and MQRR of
     the normalized coefficients.

   Either way, a result is accepted only after a further, unused prime
   reproduces every entry. If the pivots at a sample differ from the
   structure sample's, the result is `UnluckySample` and the sample is
   skipped.
5. **In-process generation** (`generate.rs`). The configuration equals the
   `family-candidates` defaults. An optional coordinate priority has the
   `--permutation` semantics. `isps_at_ends_priority` puts the first ISP
   first and the remaining ISPs last.
   - On RustFlow's native P14 spoke family, the default ISPs-last order
     stalls in sector 217 (exceptional geometry, more than 15 min,
     single-threaded).
   - With the ISPs-at-ends priority the same family generates in about 1 min,
     and rim in 77 s instead of about 200 s.

   `DynamicLiteRedSession` adds runtime-arity dispatch for hosts such as
   RustFlow.

## CLI

```
rustred litered-reduce --spec SPEC.json [--output OUT.json] [--force]
```

The spec has schema `rustred.litered-reduce-spec.v1`. Its fields:

| Field | Meaning |
|---|---|
| `bundle` | Path of the candidate bundle. |
| `admission` | `parametric-vacuum` (default) or `unit-mass-vacuum`. |
| `targets` | Integrals to reduce. |
| `extra_targets` | Further integrals reduced onto the same basis, e.g. a foreign basis for a basis-change check. |
| `preferred` | Integrals forced to the easy end of the column order. |
| `census` | Whether to build the census and use its rows. |
| `rows` | `group-orbit` (default) or `generators`. |
| `order` | `lines-dots-rank` (default) or `program`. |
| `seed` | Seed of the structure sample. |
| `points` | Rational parameter points, by parameter name. |
| `random_points` | Number of extra random rational points. |
| `random_point_seed` | Seed of the random rational points. |
| `univariate` | One map per reduction: every parameter but one fixed. |
| `max_primes`, `workers` | Prime budget and concurrent samples. |
| `exact_cross_check` | Number of targets also reduced by the exact applier and compared. |
| `compare_generator_rows` | Also report the compression with generator rows. |
| `bundle_max_bytes` | Raised bundle input limit. |

## P14 validation (2026-10-24)

Kira was used only as the oracle. The runs, scripts and the acceptance JSON
are in the pilot scratchpad: `rustred-dev/litered-lane`.

- **DAG.** For the 74 rim final requests (manifest indexing) the DAG has
  26,993 nodes, 201,871 edges and 0 uncovered points. This equals the Python
  design prototype.
- **Compression with the RustRed census.**

  | Family | Terminals | Masters |
  |---|---:|---:|
  | Rim | 216 | 42 |
  | Spoke | 246 | 42 |
  | Undeformed | 179 | 14 |

  - Both column orders give these totals.
  - Generator rows and group-orbit rows give the same masters.
  - Per-unique-sector counts equal Kira's in 27/27, 26/26 and 11/11 classes.
- **Coefficients against Kira.** The coefficients of the 74 rim and 66 spoke
  requests equal the Kira tables after the basis change. 42 requests of each
  family are Kira masters, for which this comparison only checks the basis
  change itself; the non-trivial rows are the 32 rim and 24 spoke requests
  that Kira reduces, together with the full rank (42) of the basis change.
  The rows agree:
  - exactly at 3 rational `(d, eta)` points;
  - in `Q(eta)` at `d = 4-2/2700` and at `d = 37/11`, checked at 4 rational
    `eta` points each.
- **RustFlow's native family.** On the native P14 family (ISPs `k0.k1`,
  `k0.k2`) the rim compression is also 216 → 42, and the coefficients again
  equal the Kira tables.
- **Timing.** About 6 to 14 s per family on 8 cores, including the census,
  compression and all reconstructions.
- **RustFlow integration.** With RustFlow's `LiteRedRustRedBackend` (RustRed
  only), `PreparedFlow::new_at_epsilon(1/2700)` closes for rim and spoke with
  basis 42 in 5 rounds.
  - The first sample takes 90 s (rim) and 80 s (spoke); each further epsilon
    sample takes about 13 s.
  - All 1764 DE-matrix entries equal the Kira-derived ones after the exact
    basis change.
