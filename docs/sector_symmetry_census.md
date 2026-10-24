# Vacuum sector-symmetry census

Status (2026-10-24): `rustred::sector::symmetry::SectorSymmetryCensus` is a
public Rust service. On RustFlow's native four-loop wheel (P14) it reproduces
Kira's independent sector partitions, in-sector symmetry counts and (with a
caller preference list) Kira's representatives exactly for the undeformed
family and for symbolic-`eta` deformations of a rim and a spoke line. On the
five-loop P1 top sector it reproduces Kira's partitions for the undeformed
family and three single-line deformations. It is a symmetry proof service
only: it makes no IBP-completeness, master-independence or minimality claim,
and no solver consumes it yet.

## Contract

`SectorSymmetryCensus::discover(family, sectors, options, control)` classifies
caller-declared sectors of one vacuum family without analytic power shifts;
`discover_subsectors(family, top, analyzer, options, control)` first keeps
every nonempty subsector of `top` that the exact `sector::zero::Analyzer`
neither proves zero nor excludes, and retains the analyzer's zero decisions.

- The sector-owned projector-graph discovery (`sector::symmetry::discovery`)
  colors each line by its complete denominator constant, a context
  `Coefficient`, so `-1 - eta` and `-1` lines are never interchanged. Its
  native canonical graph is each sector's `CanonicalSupportKey`.
- Sectors are visited in `RepresentativePolicy` order: `LowestSectorId`
  (default) or `HighestSectorId` of the sector ids `sum_{i active} 2^i`, or
  `Preferred(list)`, where listed sectors come first in list order and the
  rest follow by lowest id. The first sector of a class is its
  representative; the result is independent of input order, hashing and
  process. `Preferred` with an external tool's representatives (Kira, AMFlow)
  makes master lists directly comparable.
- A sector joins a class only through a route `k_sector = A k_representative`
  that `symmetry::verify` replays exactly (all denominators, constants and
  ISP rows) with a unit Jacobian and that
  `integral_transport::compile_with_affine_constants` admits as a unit
  bijection of the active lines. Equal canonical keys with only
  non-unit-Jacobian maps stay separate classes
  (`statistics().shared_canonical_keys`).
- Every representative retains the verified self-maps of its native
  canonical-graph generators and the order of the active-line permutation
  group they generate (bounded by `max_group_order`).
- A sector-local failure (support preparation, map recovery, route or
  generator compilation) never aborts the census. The sector keeps its own
  class, or the class drops the generator and reports no group order, and
  `skips()` records the sector, the stage (`Support`, `Match`, `Route`,
  `Automorphisms`, `AutomorphismGenerator`) and the typed reason. Skips can
  only split classes or drop rows; no unverified relation is ever produced.
  Cancellation, family-level ineligibility and a failing identity transport
  remain errors.

### Affine constants

`integral_transport::compile` keeps its strict contract: inactive-row affine
constants must be rational, and a parameter-dependent constant fails with
`NonconstantCoefficient { column: None }`. The opt-in
`compile_with_affine_constants(.., AffineConstants::Parametric)` admits
constants that depend on context parameters (an inactive unit line expressed
through a deformed `-1 - eta` target line); linear coefficients must stay
rational. Such a `Prepared` reports `has_parametric_constants()`. Its exact
`transport` expands the constants as virtual native-Q axes under the same
prospective budgets and substitutes them exactly in the coefficient context;
the substitution work is charged to shared budgets as well. The native-Q
trace-support lane refuses such a map (`ParametricConstantSupport`), and
`RoutedCandidateReducer` refuses it at admission.

The census uses `options.affine_constants`, `Parametric` by default.
`Rational` restricts it to maps every transport lane accepts; maps that need
parametric constants then appear as `Route` or `AutomorphismGenerator` skips.

### Routing for solvers

- `transport_to_representative(key)` returns the exact transported integral.
  Numerators on inactive lines and ISPs are expanded through the verified
  affine rows; pinched endpoints lie in subsectors of the representative.
- `route_relation(key)` returns `I - T(I) = 0` for a non-representative key.
- `automorphism_relations(key)` returns the nonzero rows `I - T_g(I) = 0`
  for every representative generator `g`. Applied to a seed set closed under
  the generated images they span the relations of the full finite group.
- `is_zero_sector(mask)`, `zero_sector_kind(mask)` and `zero_sectors()`
  expose the retained zero set of `discover_subsectors` (proved zero or
  excluded, including the empty sector). `discover` has an empty zero set.
  Relation and transport requests for a zero-set key are `ZeroSector`
  errors; undeclared nonzero sectors stay `UnknownSector`.
- `normalize(key)` applies member routes recursively until every endpoint
  lies in a representative sector and drops zero-set endpoints. Endpoints are
  processed by decreasing line count, so every key is transported at most
  once and the recursion terminates (routed line counts strictly decrease).
  Automorphisms are not applied. `normalize_many(keys)` shares transported
  routes across a batch. Both check the inherited Symbolica RSS/time budget
  at every route application; the count thresholds in
  `options.normalization` are opt-in diagnostics, as elsewhere in RustRed.
- `route_relation_with` and `automorphism_relations_with` take a
  `RelationEndpoints`: `Transported` (the default rows), `DropZero` (drop
  zero-set endpoints) or `Representatives` (every endpoint normalized, so the
  row lives on representative sectors apart from a routed source).

Unsupported geometry (non-square active lines) and rank-deficient supports
become singleton classes with a `SupportSkipReason`.

## Validation

`crates/rustred-core/tests/sector_symmetry_census.rs` embeds RustFlow's exact
native P14 family (`D0..D7` physical, `D8 = k0.k1`, `D9 = k0.k2`) and Kira's
oracle partitions, group orders and representatives. Independently of the
census code, every route and automorphism is re-checked:

- integer line arithmetic: `q_s A = +-q_t`, equal complete constants,
  `|det A| = 1`;
- the exact integrand identity `f_I(A k') = sum_j c_j f_j(k')` for every
  probe integral (dots, ISP numerators, inactive physical and deformed-line
  numerators), evaluated over `Q(eta)` at random rational Gram matrices
  `G' = (k'_a . k'_b)` with `G = A G' A^T`, from the test's own denominator
  table. This detects a consistently wrong affine constant (for example
  `eta -> -eta`), which a round trip cannot; the opposite map convention is
  confirmed to fail;
- an exact numerator round trip `T^-1(T(I)) = I` through an independently
  inverted and verified map;
- relation rows equal `I - T(I)` exactly;
- normal forms equal a plain recursion over the public API and land on
  representative sectors; `Representatives` route rows equal
  `I - normalize(I)`;
- with `Preferred(Kira representatives)` the census picks exactly Kira's
  representatives and the same partition.

| Family | Nonzero | Classes (Kira) | sum(\|G\|-1) (Kira) | Integrand identities (opposite convention fails) | Round trips with `eta` |
|---|---:|---:|---:|---:|---:|
| Undeformed | 134 | 11 (11) | 333 (333) | 986 (649) | 0 |
| Rim `D0 - eta` | 134 | 27 (27) | 377 (377) | 1226 (529) | 379 |
| Spoke `D1 - eta` | 134 | 26 (26) | 394 (394) | 1223 (537) | 371 |

The class partitions equal Kira's sector-to-representative maps member by
member. Of the transported probe endpoints, 54-57% lie in zero sectors and
8-11% in non-representative sectors; normal forms need up to 108 recursive
route applications. A release census of each P14 variant, including the
256-mask zero census, takes well under a second on one core.

`tests/numeric_coefficient_context.rs` checks a parameter-free context
(`eps = 1/2700`, `eta = 1/3`) end to end: a P14 rim Laporta solve of
`[1,1,1,1,1,0,0,0,-1,0]` at depth 3 gives the same rule and residual
structure as symbolic `eta` and symbolic `(d, eta)`, and each solution
replays exactly.

Five-loop family `44b030da` (15 unit-mass lines), P1 top sector
(slots 1, 2, 5, 6, 7, 9, 10, 12; 66 nonzero sectors), one line `D_s - eta`
with symbolic `eta`:

| Deformed slot | Classes | Routed | sum(\|G\|-1) | Kira |
|---|---:|---:|---:|---|
| none | 9 | 57 | 1171 | same partition and orders |
| 1, 2, 5 | 13 | 53 | 455 | slot 1: same |
| 6, 7, 9, 10 | 18 | 48 | 1456 | slot 6: same |
| 12 | 11 | 55 | 665 | slot 12: same |

All 32,767 subsectors of `44b030da` give 27,202 nonzero sectors in 339
classes (280-350 s on one core, release). Exact map verification over `Q(d)`
dominates the large census; graph canonization is about 2% of the profile.
Sectors are processed serially.
