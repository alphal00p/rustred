# Guarded distribution-weighted source identities

The Rust API `solver::guarded` accepts polynomial identities with explicit
integer domains for occupation-weighted reverse-unitarity integrals. It reuses
native modular rule discovery and exact lifting, and adds source-domain replay
and a context-bound application/persistence interface. It does not generate the
physical weighted identities or prove their validity from a measure.

## Integral coordinates and source domains

Flatten an integral `I(a; b)` into one coordinate array, and give every coordinate
an `IndexRole`:

- `Ordinary`: an ordinary propagator or numerator power.
- `RequiredCut`: a required on-shell cut; a nonpositive power defines a zero
  integral. Leave raised cut powers in this role.
- `Occupation`: a distribution coordinate. Zero denotes the bulk theta
  function, positive values denote surface distributions, and negative values
  are undefined.

The flattening is an algebra storage convention. It does not add occupation
coordinates to an `IntegralFamily` as physical propagators or invoke the
ordinary family generator, zero-sector census, symmetries, or master count.
Physical support conventions, orientations, chemical potentials and allowed
symmetries belong to the supplied measure and source corpus.

A `GuardedSource` contains a stable ID, a `PolynomialRow`, an `IndexDomain`, and
optional polynomial nonzero conditions. Integral powers in source rows are
symbolic offsets. Coefficients use one shared Symbolica variable map; pass the
position of each integral's index variable separately. For example, an entry
with shift `[0, 0, 1]` represents the same ordinary powers and the next occupation
order. A source guarded by `b=0` can contain both `I(a;0)` and `I(a;1)`.

`IndexDomain` is a Cartesian product of inclusive integer intervals, with
unbounded endpoints represented by `None`. Separate rows can describe `b=0`,
`b=1`, and `b>=2`. An occupation source domain must exclude negative values.
These bounds are distinct from coefficient nonzero conditions. This initial
interface does not accept arbitrary coupled inequalities as source guards.

`measure_id` must identify the caller's conventions. A saved program also binds
the exact source rows, roles, index-variable map, domains, conditions and supplied
zero domains. Measure-specific zero relations can be supplied through
`with_zero_domains`; no zero relation is inferred from vacuum scalelessness.
Permitted symmetries can be supplied as additional guarded identities only when
they are expressible in the supported shifted-index row language. Arbitrary
index permutations and general momentum transformations are not a separate
symmetry API in this interface.

`native_sources()` exposes the underlying polynomial corpus for inspection and
integration with lower-level tooling. Passing that corpus to the legacy
`SectorSolver` discards the guarded wrapper's domain and role semantics. Use the
`GuardedSourceSystem` search and `GuardedProgram` application paths for weighted
integrals.

## Discovery, application and reuse

After constructing the original polynomial rows, the integration pattern is:

```rust,ignore
use std::sync::Arc;
use rustred::solver::{SearchOptions, SolverError};
use rustred::solver::guarded::{
    GuardedProgram, GuardedSource, GuardedSourceSystem,
    IndexBounds, IndexDomain, IndexRole,
};

// row_bulk and row_surface are PolynomialRow<3> values on one native
// coefficient map: d, mu, a, n, b. Here E^(-n) is an energy numerator.
let domain = |b| IndexDomain::new([
    IndexBounds::new(Some(1), None).unwrap(),
    IndexBounds::new(None, Some(0)).unwrap(),
    b,
]).unwrap();
let sources = Arc::new(GuardedSourceSystem::new(
    "project:massless-positive-energy:occupation-conventions-v1",
    [IndexRole::RequiredCut, IndexRole::Ordinary, IndexRole::Occupation],
    [2, 3, 4],
    vec![
        GuardedSource::new("bulk-euler", row_bulk, domain(IndexBounds::fixed(0))),
        GuardedSource::new("surface-euler", row_surface,
            domain(IndexBounds::new(Some(1), None)?)),
    ],
)?);
let requested = domain(IndexBounds::new(Some(0), Some(3))?);
let found = sources.solve_domains(
    vec![requested],
    SearchOptions { max_depth: Some(2), ..Default::default() },
    128,
)?;
// Retain/report found.unresolved; successful search is not a closure claim.
let program = GuardedProgram::new(sources.clone(), found.rules, [])?;
let result = program.apply(&[2, -2, 1])?;
// Inspect result.status, result.terms and result.nonzero_conditions.
# Ok::<(), SolverError>(())
```

`solve_domain` uses a default domain budget; `solve_domains` takes it explicitly.
A finite seed depth is required. Broad positive and nonpositive ordinary sectors
are partitioned as needed, while occupation zero remains a legitimate fixed
bulk face. Exhausted cases are refined at source-guard endpoints reachable
within the seed budget, allowing boundary-only identities to be discovered
inside a broader request. For targeted boundary checks, requesting separate
fixed occupation orders is also useful.

Domain bounds use `i64` and can describe unbounded parametric rays. Concrete
native powers and symbolic displacements still use RustRed's compact range
`-64..=63`; a wider domain is not a promise that every point can be materialized.
Out-of-range concrete application, fixed search coordinates and unrepresentable
seed shifts report `UnsupportedPower` as unresolved work, preserving any rules
already found on supported parts of the request.

Discovery keeps original guarded rows, avoiding global preconditioning across
incompatible domains and loss of original rows at exceptional specializations.
Admitted source seeds may cross a bulk/surface boundary. If source `i` has guard
`G_i`, was seeded by `s_i`, and the winning head was recentered by `-t`, the
resulting rule retains `G_i(n+s_i-t)`. Exact replay recomputes source use and
retains source conditions and uncancelled denominator obligations. Strict
integral descent is checked before publishing and when applying a concrete rule.

`GuardedRule` exposes its applicability domain, nonzero conditions and candidate
RHS. The native exception extractor and case-intersection machinery are reused.
Coordinate exceptional cases can be searched further; coupled exceptional loci
that cannot be represented by the box interface remain guarded and explicitly
unresolved. Search exhaustion and unproved descent also remain unresolved.

`GuardedProgram::apply` returns `Applied`, `Terminal`, `Zero`, or `Unresolved`.
`reduce` performs bounded recursive application and retains the exact unresolved
frontier. Its terminal list is an explicit caller-selected stopping set, with no
claim of master independence. Remaining parameter conditions must be respected
when specializing dimension, masses or chemical potentials.

For reuse, `encode_native(BinaryIoLimits)` produces native bytes.
`GuardedProgram::decode_generated(bytes, expected_sources, limits)` binds them to
the expected source context and replays each rule before application. This is a
matching RustRed/Symbolica native format, not a general untrusted interchange
format. Limits and mismatched contexts are rejected explicitly.

## Independent physics controls and their scope

The test module `crates/rustred-core/src/solver/guarded/tests.rs` uses independently
integrated moments, in addition to source replay. The relevant physical
references are:

- Ghisoiu et al., [On high-order perturbative calculations at finite
  density](https://arxiv.org/html/1609.04339), sections 2 and 5: finite-density
  cutting produces occupied on-shell phase-space measures; numerator factors
  and repeated propagator powers require their corresponding extensions.
- Österman, Schicho and Vuorinen, [Integrating by parts at finite
  density](https://arxiv.org/html/2304.05427v2), identities (ibp.1a–c),
  (ibp.2a–d), and equation (5.22): distribution derivatives enter one- and
  two-loop IBPs, and their zero-temperature surface contributions cannot be
  discarded. Their contour prescription also explains why taking the
  zero-temperature limit before manipulating singular integrands needs care.
- Kärkkäinen et al., [Quark matter at four loops: hardships and how to overcome
  them](https://arxiv.org/html/2501.17921v2), equations (31)–(34): the connected
  four-loop C topology has a nonzero two-cut contribution. Its two vacuum
  integrations leave a known scalar kernel in two occupied phase-space
  integrals. This supplies the four-loop topology used by the bounded pilot.

The one-loop fixture is the positive-energy massless cut

\[
J(a,n;b)=\int_0^\infty dE\int d^d p\,
 C_a(E^2-p^2)E^{-n}W_b(\mu-E),\qquad
 C_a(x)=\frac{(-1)^{a-1}}{(a-1)!}\delta^{(a-1)}(x).
\]

With `mu=1`, direct spatial integration gives, after dividing out the common
angular factor, `J=c_a R(d-2a-n,b)`, where

\[
c_a=\frac{(-1)^{a-1}}{(a-1)!}
 \left(\frac d2-1\right)_{\underline{a-1}},\quad
R(q,0)=\frac1{q+1},\quad
R(q,b\ge1)=\frac{(-1)^{b-1}}{(b-1)!}(q)_{\underline{b-1}}.
\]

Here the underlined symbol is a falling factorial. These formulas follow by
integrating the cut delta and evaluating distribution derivatives on the radial
power; they are not obtained from solver reductions. The tests keep `d` symbolic
and use analytic continuation from a convergent radial integral. They cover
raised required-cut powers 1–3, polynomial energy numerators through degree 3,
and bulk/surface orders through 4. Bulk and surface values are explicitly
nonzero. Missing cuts and undefined negative occupation orders are checked
separately.

The connected two-, three-, and four-loop controls use two cut massless
momenta `P,Q` and exchanged momentum `L=P-Q`, with remaining kernels

| Loops | Connected kernel before vacuum subintegration | Provenance |
| --- | --- | --- |
| 2 | `1/L^2` | Doubly cut sunset, 1609.04339 equation (31), massless specialization |
| 3 | `(1/L^2) integral_R 1/[R^2(R+L)^2]` | Derived connected bubble-insertion control |
| 4 | `(1/L^2) integral_RS 1/[R^2 S^2(R+S+L)^2]` | Published C cut, 2501.17921v2 equation (31) |

Vacuum subintegration produces a power of `L^2`. Since on shell
`L^2=2pq(1-cos(theta))`, its angular factor is independent of occupation order;
separate radial moments therefore provide an exact oracle for support shifts.
The tests retain the individual physical denominator and cut indices. A
polynomial energy numerator is an ordinary negative index.

For these controls the supplied IBP is simultaneous momentum scaling:

\[
0=\left[L(d+1)-2\sum_i a_i-n-
 \sum_{j:b_j\ge1}b_j\right]I
 +\mu\sum_j c(b_j)I(b_j+1),\qquad
 c(0)=-1,\quad c(b\ge1)=b.
\]

It follows directly from momentum homogeneity and
`(k dot derivative)(mu-u dot k)=(mu-u dot k)-mu`, together with the supplied
support multiplication identities. The one-loop corpus also contains
`f W_1=0` and `f W_b=W_(b-1)` on their distinct domains.

These checks exercise bounded native discovery, exact replay and concrete
application on physical weighted integral families through four loops. They do
not evaluate the full published four-loop master, reproduce the full three-loop
QCD Mercedes diagram, search a complete set of independent IBP vectors, or
establish arbitrary-family closure. Normalizations common to all terms of an
identity are divided out. The three- and four-loop checks are correctness pilots;
large-family scalability and a project-specific independently generated weighted
IBP corpus remain separate acceptance work.

Running these tests with `--nocapture` emits one receipt per loop order with the
number of generated rules, unresolved domains, tested targets and successful
applications. Target counts include deliberate unreduced bulk controls in the
connected pilots. They describe this bounded suite and must not be interpreted
as a family-wide coverage or master-count measurement.

## Bounded one- through four-loop pilots

The implementation was checked on branch `fermi`, based on RustRed
`c0b642c9246e2f55da89f4c1e01d453056da5981`, using the checkout's pinned Symbolica
revision `ef0db494533c87adb40356c996241680dc5a7bff`. This is not a backport or a
verification of the original requester pin `7c1ed03722b8c05daf60c89ba4ecc79457ed2ada`.

Each pilot below ran in a fresh process with one test thread, in the unoptimized
native test build. Wall time includes test-process startup; peak RSS is the whole
test process, recorded using Linux `wait4`. These short runs share a busy host and
do not establish a scalability trend.

| Loops | Rules | Unresolved domains | Applied / sampled targets | Wall time (s) | Peak RSS (KiB) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 9 | 0 | 36 / 36 | 0.0209 | 37120 |
| 2 | 6 | 6 | 18 / 27 | 0.0486 | 37116 |
| 3 | 6 | 6 | 18 / 27 | 0.5551 | 37148 |
| 4 | 6 | 6 | 18 / 27 | 0.1068 | 37116 |

Every applied reduction was compared exactly with the independent radial-moment
formula. Numerators in these tests are supplied as ordinary negative powers;
automatic conversion of arbitrary polynomial expressions is outside this API.

## Supplied contour reference (2026-10-09)

The exact user-supplied equations are preserved in
`crates/rustred-core/tests/fixtures/finite_density_ibp_reference.tex`
(SHA-256 `8cddbb43eb38ee7aa2a29b0d0c5d023487a70569f119b9cb031de36eadac85de`).
The test labels R1–R7 follow its seven displayed groups of relations. Counting
the two pairs of zeros and four factorization cases separately gives twelve
scalar equalities. This reference has at most three loops; the preceding
four-loop control remains a separate, limited pilot.

### Conversion to occupation and surface distributions

The reference uses shifted **Euclidean** energies, with `d=D-1`. On a positively
oriented occupied cut, substitute `P0=iE`, not `P0=E`. A negatively oriented
original momentum has the corresponding minus sign. Energy powers remain
ordinary polynomial numerator coordinates. They are not occupation indices.

Writing `J(a,k;b)` with an explicit numerator `E^k` (so `k=-n` in the earlier
index notation), the massless one-loop medium contribution obeys

\[
\mathcal D_5^{(\alpha)}(\sigma;a)
  =(-1)^a(i\sigma)^\alpha J(a,\alpha;0),\qquad \sigma=\pm1.
\]

Here `J` uses the reference spatial measure `d^d p/(2 pi)^d` and the shell
measure `dE`; the earlier moment comparisons divide out the common angular
normalization.

Its vacuum term is scaleless in dimensional regularization. In particular,

\[
\mathcal D_5^{(1)}(+;2)=-\frac{i}{2}J(1,0;1),\qquad
\mathcal D_4^{(0,0)}(++B;111)
 =\frac{[J(1,0;1)]^2}{2(d-2)(d-3)}.
\]

Thus the first reference relation contains a nonzero Fermi-surface moment.
Both its Euclidean phase and its surface contribution matter. Tests compare
the energy-first residue calculation with independent spatial-first raised-cut
moments for powers `a=1..3`, energy degrees `0..4`, and both contour signs.
The sunset is evaluated independently by its double-cut angular and radial
integrals. These checks use the contour conventions and analytical formulas in
[Integrating by parts at finite density, Appendix A and D.3](https://arxiv.org/html/2304.05427v2).

For a multiloop term, let `H` denote the entire remaining vacuum kernel,
including its routed polynomial numerator. Each simple occupied cut has weight
`-W0(mu-E)/(2E)`. Raising a selected propagator to power `a` applies
`(-partial_(m^2))^(a-1)/(a-1)!` before taking its massless limit. With
`J1[E^k H^(r) W_b]` denoting a simple positive-energy cut, the derivative is

\[
\partial_{m^2}J_1[E^k H^{(r)}W_b]
 =\frac{k-1}{2}J_1[E^{k-2}H^{(r)}W_b]
 +\frac12J_1[E^{k-1}H^{(r+1)}W_b]
 +\frac{t_b}{2}J_1[E^{k-1}H^{(r)}W_{b+1}],
\quad t_0=-1,\quad t_{b\ge1}=b.
\]

The second term differentiates the **full** kernel, not just its numerator.
With multiple cut legs, mixed kernel derivatives are retained. Raised uncut
propagators remain inside `H`; independent masses for the original lines avoid
an implicit mass dependence being discarded. This implements the numerator and
raised-power extensions of the
[finite-density cutting rules, sections 2, 4 and 5](https://arxiv.org/html/1609.04339).

`reference_cut_plan.rs` constructs explicit formal cut recipes for every
three-loop term in the supplied equations. It enumerates independent charged
line covectors, orients them, constructs an exact unimodular loop basis, routes
all six denominators and the energy numerator, and expands all raised-cut
kernel derivatives and surface distributions. Cut independence refers to the
physical diagram; deleting edges of the momentum-coordinate tetrahedron is
not the appropriate test. Zero-cut and partial-cut kernels are retained:
declaring an on-shell massless kernel scaleless *before* differentiation can
incorrectly remove nonzero derivatives.

The conversion tests include nonconstant symbolic kernels and a coupled
two-energy polynomial kernel. They compare to independently integrated shell
distributions and detect omitted surface or mixed-derivative terms. The cut
recipes retain unevaluated vacuum kernels and their continuation prescription.
They are not a vacuum-kernel evaluator or an automatic arbitrary-polynomial
front end.

The supplied D1 corpus contains 30 term occurrences, which expand into 229
formal cut contributions and 435 kernel-derivative terms. Of those terms, 117
contain surfaces and 117 contain differentiated kernels; these categories
overlap. Surface and derivative orders reach two, with seven terms containing
surfaces on multiple legs and seven containing mixed kernel derivatives.
An additional source-level check converts actual generated one-loop spatial
IBPs and compares their coefficient rows with independently differentiated
cut/occupation distributions. It includes raised cuts, both contour signs,
energy degrees 0, 1, 2 and 4, and a nonzero surface contribution. This is a
one-loop source check, not a complete three-loop weighted-source replay.

`reference_weighted_spatial.rs` also compares the native D1 source rows with
independently differentiated weighted integrands for the supplied raised-cut
and numerator seeds, across all nine spatial IBP vectors and their admissible
cut masks. In this compact representation, selected propagators become
`C_a(g)` with `g=E^2-p^2`; their positive-energy and occupation factors remain
spatial constants at fixed independent energies. The derivative and
multiplication laws are checked with nonpositive required-cut powers removed.
The ordinary numerator and occupation coordinates retain separate meanings.

For this comparison the uncut Minkowski vacuum energies use `dE/(2 pi i)`
and the cut energies use `dE`. With the inherited Feynman continuation, the
conversion phase is `(-1)^sum(a) i^sum(alpha)`, independent of cut count.
The compact raised-cut representation can then be lowered by the full-kernel
derivative expansion above. This source comparison occurs before graph
routing; the remaining vacuum kernels are unevaluated.
The measured correctness run checked 1,242 rows across 138 cut configurations
for 16 seeds and all nine spatial vectors: 15 supplied tensor/polynomial seeds
and one odd-phase control. It compared 10,795 integral terms and explicitly
removed 1,108 nonzero native terms at required-cut boundaries. This validates
the source conversion; it does not assert independent replay of the complete
three-loop weighted reduction certificates after routing.

### Independent source proofs and performance method

`reference_spatial.rs` derives the `L^2` spatial IBPs directly from
`partial_(p_i) dot p_j`, including `p_i dot p_j = P_i dot P_j - P0_i P0_j`.
Every source row is also compared exactly with RustRed's ordinary parametric
generator after projection out of the energy direction and substitution
`D=d+1`. The supplied equalities are queries only; no reference equality is
inserted into the source matrix.

The three-loop proof harness preserves the contour class in every column.
Its change-of-variable identities preserve the supplied measure, including
bubble reflections that legitimately connect different contour classes.
Only explicitly proved scaleless vacuum factors or free polynomial
integrations are discarded. No generic vacuum zero-sector or master-count
assumption is imported.

The bounded probe seeds neighborhoods of the reference terms. Shells add
inverse source shifts, denominator/energy-power transfers, and bubble
reflections, with seed denominator powers in `-1..3` and energy powers in
`0..4`. Native finite-field elimination at `d=17` selects a dependency trace;
that trace is recomputed over exact rational functions in `d`. A final
independent multiplication of the original source rows by the recovered
coefficients must reproduce the query exactly. A modular match alone is never
counted as a proof. A modular miss is reported as unresolved at that bound,
not as a contradiction of the reference. Displayed denominators such as
`d-2`, `d-3`, and `d-4` retain their applicability restrictions; this does not
evaluate the relations at their singular specializations.

For the exact step, back substitution through native modular `L` proposes the
nonzero original-source support. If that smaller subset fails exact replay,
the verifier retries the full dependency trace. This is a support-selection
optimization, not rational reconstruction or a probabilistic final check.
An unlucky specialization can remove necessary sources from both proposed
supports. If both miss exact membership, the bounded probe reports the query
as unresolved; it does not automatically expand to the entire source matrix.
Regression tests cover cancellation in the modular source coefficients and an
actual modular specialization that loses both supports despite a valid exact
identity. Exact membership followed by a failed original-source multiplication
is a verifier error and fails the test.
Short constant source rows precede longer symbolic rows, and exact replay uses
the native sparse reducer to solve `A^T w = target^T` for the certificate
coefficients. Short constant equations are processed first using incoming-row
back-substitution. Once every weight has a pivot, the verifier can stop
elimination and check **all** original columns by multiplying `w^T A` back.
An inconsistent column outside the solved subsystem is therefore still a
support miss. This controls intermediate expression growth without changing
the identity being proved or accepting a finite-field result as final.

The proof harness benchmarks finite source-row reduction using the native
sparse reducer. It does not benchmark full three-loop parametric rule
discovery. The first reference conversion separately exercises guarded
parametric discovery, exact replay and application. Formal conversion of the
complete reference corpus and exact contour proofs are distinct from an
independent native reduction of every resulting weighted cut sector.

The reproducible runner is `tools/research/guarded_reference_benchmark.py`.
It runs fresh processes with one test thread and records stage output, wall
time, per-child peak RSS, timeouts and exit codes in JSON. Compile the native
test executable with:

```sh
cargo test -p rustred --lib --no-default-features --features native \
  solver::guarded::reference --no-run
python3 tools/research/guarded_reference_benchmark.py \
  --binary /absolute/path/to/the/native/test/executable \
  --output /tmp/reference-benchmark.json --shells 1 2 --repeats 3
```

The ignored probe can also be run directly using test filter
`solver::guarded::reference_validation::bounded_supplied_reference_probe_reports_outcomes`,
`--exact --ignored --nocapture --test-threads=1`, and
`RUSTRED_REFERENCE_SHELLS=2`. Set `RUSTRED_REFERENCE_REQUIRE_CLOSURE=1` to make
unresolved queries fail the test; otherwise the probe prints them explicitly.
`RUSTRED_REFERENCE_FILTER` optionally selects one reference label.

### Reference verification receipt (2026-10-09)

The updated native solver regression suite passed **905 tests**, with the
existing frozen-schedule benchmark and the explicit supplied-reference probe
ignored in the routine run. All 28 nonignored reference checks passed. The
probe is measured separately below. Changed reference modules pass formatting.

The environment used the unoptimized native test profile and the same pinned
Symbolica revision stated above. Timings exclude compilation and include
fresh-process startup on a shared, busy Linux host; they are observations,
not production throughput or an arbitrary-family scaling estimate.

All twelve supplied scalar equalities have exact checks in the contour
convention. The conversion operator and the weighted source checks above are
separate evidence; this is not a claim of complete native three-loop weighted
rule discovery or independent replay of every routed weighted certificate.

| Reference | Equalities | Exact check |
| --- | ---: | --- |
| R1, sunset | 1 | Independent double-cut angular/radial integral and native surface conversion |
| R2–R3, zero cases | 4 | Explicit vacuum-factor or free-integration witnesses respecting contour shifts |
| R4, factorization chain | 4 | Measure-preserving factorization into the checked sunset and tadpole |
| R5, negative denominator power | 1 | Exact spatial-IBP/routing certificate |
| R6, first long equation | 1 | Exact certificate with 745 nonzero original-source weights |
| R7, final long equation | 1 | Exact certificate with 2,209 nonzero original-source weights |

The final wide run (`shells=2`, one fresh process, 600-second limit) completed
in **93.20 seconds**, with **534.08 MiB** peak RSS and no unresolved reference
queries. Its complete machine-readable receipt is
`crates/rustred-core/tests/fixtures/finite_density_reference_benchmark.json`.

| Query at shell 2 | Seeds | Source rows | Integral columns | Generation (s) | Modular (s) | Exact certificate including multiplication back (s) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| R5 | 417 | 3,305 | 1,527 | 1.163 | 0.079 | 0.00044 |
| R6 | 3,571 | 30,313 | 14,160 | 14.003 | 5.560 | 0.322 |
| R7 | 4,428 | 47,350 | 12,632 | 27.714 | 30.166 | 13.317 |

The exact R6 and R7 checks solve 1,093/1,099 and 2,926/2,928 coefficient
equations respectively, then verify every original column, including those
not needed for the solve. Both residuals are exactly zero over rational
functions of `d`. No reference equation is used as a source for another.

At shell 1 the two long equations remain explicitly unresolved; increasing
the finite source neighborhood to shell 2 is required by this harness.
Consequently, a successful shallow-probe process is not a closure result.
The native parametric discovery timing belongs to the separate R1 conversion
test; the three-loop table measures finite source-row proof construction.

Three additional fresh-process samples are saved in
`crates/rustred-core/tests/fixtures/finite_density_reference_short_benchmark.json`:

| Check | Median wall time (s) | Observed range (s) | Maximum peak RSS (MiB) | Outcome |
| --- | ---: | --- | ---: | --- |
| D5 conversion and R1 native surface conversion | 0.01828 | 0.01769–0.01974 | 36.28 | Exact checks pass |
| Shell-1 reference probe | 7.13524 | 7.12916–7.14176 | 90.37 | R6 and R7 explicitly unresolved |

Within the R1 test, median native discovery took **1.136 ms**, exact replay
**0.194 ms**, and application **0.373 ms**. It discovered one guarded rule,
reported no unresolved requested domains, and applied it to both sampled
raised-cut targets. These millisecond figures describe that small conversion
rule, not complete two- or three-loop reduction. Only one wide-run sample was
taken; no repeatability or scaling claim is inferred from that single result.
