# Massless propagator-type integrals (p-integrals)

RustRed reduces two-point ("propagator-type") families with one external
momentum `q` through the same lanes as single-scale vacuum families. This page
states the kinematic model, what is supported, and what is not yet.

## Family model

- Loop momenta `k1..kL`, one external momentum `q`, numeric Gram entry
  `q.q = s0` with `s0 = 1` or `s0 = -1`. Use `gram(q,q,-1)` for Minkowski
  denominators at Euclidean `Q^2 = -q^2 = 1`, or `gram(q,q,1)` for Euclidean
  denominators at `q^2 = 1`. Every coefficient then stays in `Q(d)`.
- Coordinates: the `L(L+1)/2` products `k_i.k_j`, then the `L` products
  `k_i.q`, loop-major (`L(L+3)/2` in total). A complete family has that many
  denominators: the lines of the top graph followed by the ISPs. At `L = 1..5`
  this is 2, 5, 9, 14 and 20 coordinates.
- ISPs are ordinary denominators restricted to nonpositive powers through the
  root (`--nonpositive-indices`). Any scalar products may be used, for example
  `(k1-k3)^2` or `(k3-q)^2`.
- IBP rows: `L(L+1)` ordinary rows per seed (`L` derivatives times `L+1`
  contraction momenta); with one external momentum there are no
  Lorentz-invariance rows.

Example (the two-loop kite, compact input):

```text
I(
  name(p2l_kite),
  loops(k1,k2),
  externals(q),
  dimension(d),
  prop(D1,k1^2,1), prop(D2,k2^2,1), prop(D3,(k1-q)^2,1), prop(D4,(k2-q)^2,1),
  prop(D5,(k1-k2)^2,1),
  gram(q,q,1)
)
```

## Unit scale and homogeneity

`IntegralFamily::unit_scale_kinematics()` classifies a family as
`UnitScaleKinematics::UnitMassVacuum` (no external momenta, every constant
`-1`) or `UnitScaleKinematics::UnitScalePropagator { external_square: s0 }`:

- exactly one external momentum with `q.q = s0 = +-1`;
- `d` is the sole parameter and the dimension;
- no analytic power shifts;
- every denominator constant is rational.

A rational constant is a rational multiple of `q.q`, so every denominator is a
homogeneous quadratic form in `(k, q)`. Masslessness is not required by the
shape check: a constant that is not the `q`-part of a square is read as a
multiple of `q^2`.

With `S = q^2 / s0`, an integral scales as `S^(L d/2 - sum a)`. A reduction
coefficient from target `a` to terminal `b` therefore carries
`S^(sum b - sum a)`, the same law as `m^2` for a unit-mass vacuum.
`CandidateDecomposition::common_mass_squared_power` reports this exponent
unchanged. The caller restores the scale once.

## Admission

- `CandidateFamilyAdmission::UnitScalePropagator` is the publication shape of
  p families. `CandidateFamilyAdmission::unit_scale_for(family)` selects it for
  a family with external momenta and `UnitMassVacuum` otherwise.
- The default preparation paths admit by kinematics. Vacuum families are
  admitted exactly as before. These paths are `CandidateReducer::try_new*`,
  owner libraries, `load_generated_candidate_bundle`, checkpoint loading,
  routed campaigns and saved applications.
- `litered-reduce` defaults to `parametric-vacuum` for vacuum families and to
  `unit-scale-propagator` for families with external momenta. The in-process
  `DynamicLiteRedSession` makes the same choice.
- `reduce_unit_mass` and the other `*_unit_mass*` reducer entry points apply
  to every unit-scale program; the name is historical.

## Closing artifacts

`family-close`, `certify-candidates` and `SourcePortAudit` accept unit-scale
propagator families:
- The source-port audit admits at most one external momentum, so there are no
  Lorentz-invariance rows. It replays the `L(L+1)` ordinary rows per seed:
  for each differentiated loop, the `q` contraction first, then the loop
  contractions.
- A p artifact carries `CommonMassHomogeneityProof::UniformExternalScale`
  (`rustred.homogeneity.uniform-external-scale.v1`, persisted tag 2).
  `CommonMassHomogeneityProof::for_kinematics` selects the proof. Installation
  checks it against the family.
- `reduce_with_common_mass_homogeneity` and the reduction output keep their
  meaning. `common_mass_squared_power` counts powers of `q^2/q.q`.
- The artifact-bound `ScalarNumeratorService` takes external labels through
  `try_new_with_external_momenta`.
- Closing artifacts carry raw terminals. They are not compressed by sector
  symmetry; for that, use the census or LiteRed compression.

## Sector symmetry with marked external vertices

Discovery (`sector::symmetry::discovery`) works on line momenta over
`(k, q)`:

- The loop part of a line is the integer square root of its `k.k` form. The
  `q` part follows from the `k.q` coefficients and must reproduce all of them.
- The external momentum is an extra pseudo-line with the unit row `(0,..,0,1)`
  and its own center color.
- A signed permutation of the projector graph therefore maps `q` to `+-q`.
  The recovered change of basis is `k = A k' + B q'`, `q = C q'` with `C = +-1`.
  This is the graph with both external vertices marked: only `q`-preserving
  automorphisms are proposed.
- A line's color is its mass: the constant minus the `q`-part `b^2 s0`.
  Hence `k^2` and `(k-q)^2` share a color.

Every proposal passes the complete map verifier, including the external Gram
congruence, a unit Jacobian and a unit active-line bijection. Vacuum discovery
is unchanged.

This enables `SectorSymmetryCensus`, the terminal sector-symmetry equations
and the LiteRed census compression for p families. Measured (tests below),
census compression reproduces the per-class Lee-Pomeransky master counts:

| family | nonzero sectors | classes | raw terminals | masters |
|---|---:|---:|---:|---:|
| 1L bubble | 1 | 1 | 1 | 1 |
| 2L kite | 8 | 4 | 3 | 2 |
| 3L LA | 47 | 16 | 10 | 5 |
| 3L BE | 58 | 18 | 10 | 4 |
| 3L NO | 70 | 14 | 16 | 5 |
| 4L ladder | 271 | 62 | 36 | 12 |
| 4L crossed rung | 366 | 78 | (3 sectors fail, see below) | |

Only the NO family has a master in its top sector. The LA and BE tops reduce
(triangle rule).

The classes are marked cycle-matroid classes: 2-isomorphic (Whitney-twisted)
sectors merge. At 3L the per-family nauty graph classes sum to 64; the census
finds 48. Every merge is an exactly verified map.

## Routing witnesses

Routed-campaign selections record each route as `source_to_representative`
and `owner_to_representative` matrices. For a family with `E` external momenta
these are `(L+E) x (L+E)` integer matrices over `(k, q)`:

```text
(k, q) = W (k_rep, q_rep),   W = [[A, B], [0, C]]
```

- A loop momentum may absorb an external shift (`k -> M k + s q`).
- An external momentum maps only onto a signed combination of external
  momenta (`q -> +-q`).
- The composed map must be integral, and its external rows must not involve
  loop momenta.
- Vacuum selections keep their `L x L` format.

The routed-campaign and saved-application loaders have no vacuum gate left,
and the witness format is unit-tested. No p publication has been routed end
to end yet; plan item M3 produces the first ones.

## Scalar numerators

`FamilyScalarNumeratorService::try_new_with_external_momenta(family, dot,
loops, externals, limits)` lowers scalarized numerators of a unit-scale
family:

- `dot(k_i, k_j)` and `dot(k_i, q)` become the family coordinates.
- External-external products such as `dot(q, q)` remain scalar spectators.
- The common-scale power counts powers of `S = q^2 / s0`.
- The weighted lane's `common_mass_squared` argument is `S`.
- `try_new` keeps its vacuum contract.
- The artifact-bound `ScalarNumeratorService` has the same
  `try_new_with_external_momenta` for p artifacts.

## Arity

- The finite registry is data-driven. Use `RUSTRED_RUNTIME_ARITIES` with the
  `runtime-arity-selection` feature; the default is `1..=16`.
- The campaign/application layer now accepts every registered arity up to
  `rustred::MAX_CAMPAIGN_ARITY = 32`. That bound comes from its 32-bit owner
  masks and compact walk domains. Before, the layer was capped at 16.
- Four-loop p families (14 coordinates) fit the default registry.
- Five-loop p families (20) need, for example,
  `RUSTRED_RUNTIME_ARITIES=1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,20`.

## Generation failures

`family-candidates --sector-failures report`
(`FamilyCandidatesRequest::sector_failures = SectorFailurePolicy::Report`):

- A sector whose search fails is left out of the bundle. Its integrals stay
  `Uncovered` at application, as for unselected sectors.
- The report lists each such sector under `failed_sectors` with `ordinal`,
  `sector`, a stable `kind` and the error.
- Kinds: `unsupported-nonlinear-equalities`, `case-intersection`,
  `affine-geometry`, `search`, `exceptions`, `non-progress`, `case-budget`,
  `numeric` and `finite-retention`.
- No rank cap or other scope change is applied.
- The default `abort` keeps the earlier behavior.
- The policy cannot be combined with checkpoints.

The four-loop crossed-rung family is the measured case. It has three sectors
with unsupported nonlinear exceptional equalities; plan M3 discharges them
(closure by construction, locus ledger).

## Not yet supported for p families

These lanes keep their vacuum gates. They return an error or skip p families
explicitly; they never return a wrong result.

- `ClosedArtifact::is_complete_unit_mass_vacuum` and the hand-written
  1L/2L/3L vacuum artifacts (K1, sunset, K6) stay vacuum-only.
- The vacuum-only terminal lanes:
  - tadpole and vacuum products, product charts and the vacuum factor
    provider;
  - the U-polynomial parametric aliases;
  - corank-one and circuit equations;
  - the weighted projection;
  - the numerator-symmetry supplement;
  - the vacuum terminal collections.
- p products (chains of p-subintegrals) and one-loop insertion
  (G-function) lanes are plan items M2/M3.
- Several external momenta and power shifts.

## Tests

- `crates/rustred-core/tests/p_integrals.rs`, with the exact G-function
  evaluator in `tests/support/p_integrals.rs`:
  - unit-scale classification;
  - the marked census of the kite (8 sectors, 4 classes, top line group
    order 4, a `q`-shifted route) and of the bubble;
  - affine routes with `q -> -q`, verified and transported;
  - exact reductions at `d = 37/10, 43/10, 59/10`:
    - the bubble;
    - the kite top against the triangle-rule closed form
      `K = 2/(d-4) G(1,1) [G(2,1) - G(2,3-d/2)]`;
    - every recursively one-loop target of LA/BE/NO (241/231/212 identities
      per `d`);
  - master counts against Lee-Pomeransky counts;
  - the terminal sector-symmetry equations relating the two kite sunsets;
  - the 4L crossed-rung census;
  - ignored: the 4L ladder generation, about 150 s on 8 threads.
- `crates/rustred-app/tests/p_integral_closing.rs`: `family-close` of the
  bubble and the kite, with reductions and scale exponents.
- `crates/rustred-app/tests/p_integral_candidates.rs`:
  - `family-candidates` of the kite and the 3L ladder;
  - application under `UnitScalePropagator` with the scale exponent;
  - the failure-policy input check;
  - ignored: the 4L crossed-rung failure report.
- `routed_campaign/prepare/routes/tests.rs`: p witnesses with shifts and
  `q -> -q`; malformed witnesses are rejected.

## p-library conventions

Status: normative for the p library of milestone M2 (`propagator/`), which
reduces 1L-3L p-integrals on Forcer's rule cascade. The design is in
`docs/p_integral_generation_architecture.md`. These conventions are
implemented by the M2 tasks; this section is their single statement. Any
change goes through the coordinator and is recorded here.

RustRed owns the normalisation. A reduction onto masters is directly the
quantity vakint calls `reduced`, so vakint needs no translation table. The
conventions are C1-C4 below, pinned by exact anchors.

Notation: `d = 4 - 2 eps`. A slot is a line or an ISP of a presentation
family, lines first. The exponent of slot `j` is `x_j = n_j + j_j eps`, where
`n_j` is an integer and `j_j` is the number of eps units of an eps line
(Forcer's crossed line; `j_j = 0` on every other slot). An eps line stores
its eps part as `power_shift = j (4-d)/2`.

### C1: the frontier is Minkowski, and the eps-line sign rule

- The frontier is made of Minkowski family integrals of the presentation
  family:
  - numeric Gram entry `q.q = -1`, so Euclidean `Q^2 = 1`;
  - measure `Prod_l d^dk_l / (i pi^{d/2})`;
  - no `(4 pi)^eps` factor.
- With the Euclidean measure `d^dk / pi^{d/2}`, the two are related by
  `I_M(x) = (-1)^{Sum_j n_j} I_E(x)`.
  - The sum runs over ALL slots: lines and ISPs, including ISPs at negative
    powers.
  - `n_j` is the INTEGER part of the slot exponent. On an eps line,
    `x_j = n_j + j_j eps` contributes only `(-1)^{n_j}`.
- Eps-line sign rule. For an eps family the formula above is the definition
  of `I_M`. The dropped phase `(-1)^{j eps}` is common to a whole family:
  every integral of that family carries the same eps units on the same lines.
  Hence every IBP row of the family, as generated by SpIRed or extracted by
  the Triangle template, holds for this `I_M` unchanged.

### C2: kernels are Euclidean; the Wick ratio uses integer parts

- The Insertion, SeriesParallel and Carpet kernels are Euclidean: Ruijl's
  thesis, eqs. 15-18.
- A kernel step converts back to C1 per output monomial. After the
  child-slot rewrite, it multiplies each output monomial by the Wick ratio
  `(-1)^{Sum n_parent - Sum n_child}`.
  - The sums run over the integer parts of ALL slot exponents (lines and
    ISPs) of the parent key and of that child key.
  - The ratio therefore depends on the monomial, not only on the step.
- Alternative: evaluate the kernel natively in Minkowski variables, using
  `(Y.Y')_M = -(Y.Y')_E`. The kernel module (`propagator/kernel/`) states
  which of the two it implements.
- A line absorbed into `q`, and the line of the trivial node, contribute
  `(-1)^n` only, with `n` the integer part of their power. Their eps part
  contributes no phase, by the rule of C1.
- IBP-based steps (Triangle, Custom) are convention-free: their rows hold for
  `I_M` as defined in C1.

### C3: the G-scheme normalisation of `reduce`

- `PLibrary::reduce` with `onto_masters = true` returns
  `(eps G(1,1))^{-L} Sum_inputs c I_M(x)` at `Q^2 = 1`.
  - This is Forcer's G-scheme, with Minkowski signs.
  - It equals vakint's `reduced = (-1)^{Sum a + r} C_G`
    (gammaloop `crates/vakint/src/p_integral.rs:315-327`).
- The normalisation factor is
  `eps G(1,1) = Gamma(1+eps) Gamma(1-eps)^2 Gamma(1-2eps)^{-1} / (d-3)`.
  - As a Gamma tag times a rational function of `d`: the tag
    `{(+1,+1), (-1,+2), (-2,-1)}` times `1/(d-3)`.
  - A tag pair `(m, e)` stands for `Gamma(1 + m eps)^e`.
- Its inverse is the tag `{(+1,-1), (-1,-2), (-2,+1)}` times `(d-3)`.

### C4: how vakint renders a reduction

- `PBasis::Unit` renders as `1`.
- `PBasis::Master(n)` renders as `PMaster(n)`. `n` is a Forcer master name
  from `Masters.prc`, for example `no` or `t105`.
- A tag renders as `Prod PGamma(m)^e`, with `PGamma(m) = Gamma(1 + m eps)`.
- `PBasis::Residual` never reaches vakint: it is a typed error (fail closed).
- `PPart.wick_sign` is applied on the Forcer path only.
- vakint's finalize step then applies:
  - the configured normalisation;
  - the scale factor `(Q^2)^w`;
  - the Oep guard.

### Anchors

These exact values pin C1-C4. Tests use them as anchors.

- `P1L(1,1) = 1/eps`, which is
  `PTerm { basis: Unit, tag: one, coefficient: 2/(4-d) }`.
- The 1L bubble with the numerator `k.q` (odd rank), for `q` and for `-q`.
- The 2L kite at powers `(1,1,1,1,1)`: its finalized eps^0 coefficient is
  vakint's kite value (gammaloop
  `crates/vakint/tests/p_integral_forcer_tests.rs:51-60`). Also a rank-3 kite.
- The 3L non-planar top `NO(1^8, 0)`.
- The 3L parents of `t1star05`, including the plain 3L kite with a doubled
  central line. It integrates to `t1star05` with the eps line at `n = 0`
  (the common chart-).

### Pinned master definitions

From Forcer's sources (benruijl/forcer):

- `Master(t105) = Zt1star5(1,1,1,1,1) Q^2` (`forcer-rules.h:15`, `:242`;
  `forcer.h:277`).
- `Master(no) = Zno(1^8, 0) Q^4` (`forcer-rules.h:16`; `forcer.h:276`).

Under C1 at `Q^2 = 1`:

- `I_M(t1star05; 1,1,1,1,1+eps) = -(eps G(1,1))^2 Master(t105)`;
- `I_M(NO; 1^8, 0) = +(eps G(1,1))^3 Master(no)`.

The signs are part of the convention: `PReduction` onto these masters must
reproduce them.
