# Symbolica feasibility of finite-frame Macaulay closure

## Scope and claim discipline

This note audits whether the finite-frame Macaulay candidate can be implemented in pure Rust with
the vendored Symbolica 2.2.0. It also tests the proposal against known failures of generic
Pfaffian/GKZ restriction at resonant, equal-mass Feynman kinematics.

Facts attributed to Symbolica are verified against the local public Rust API and source. Facts
attributed to papers are claims or examples in the cited primary sources. Sections labelled
**RustRed proposal** are design hypotheses; the cited papers do not establish their six-loop
practicality.

The pressure points are:

- `K = 6`: the complete three-loop scalar-product family;
- `K = 10`: a four-loop scaling proxy; and
- `K = 21`: the eventual complete six-loop scalar-product family.

A nonminimal terminal frame is acceptable only if RustRed proves exact finite closure, ships one
universal frame for every declared family and guard stratum, bounds the accumulated epsilon-pole
depth, and can realistically evaluate the whole independent terminal quotient. A small frame in
sampled boxes or stable modular ranks is not a closure certificate.

## Executive verdict

The pilot is implementable without writing a new computer algebra system, but not by calling one
turnkey Symbolica routine. Symbolica provides the difficult arithmetic kernels: sparse
multivariate polynomials and rational functions, finite fields, sparse and dense row reduction,
commutative F4 Groebner bases, Chinese remaindering, rational reconstruction, and one-variable
Newton interpolation. RustRed must own the finite-frame semantics and proof layer.

The most defensible pilot is a **direct physical-stratum, extensional Macaulay compiler**. It
enumerates translated ordinary IBP sources in sector-chart coordinates and assembles their exact
integral-key coefficients. This realizes the shift/Ore semantics by evaluation at translated
indices and avoids pretending that a commutative polynomial Groebner basis is an IBP module
basis. Modular sparse elimination may discover a row basis; exact ordinary-source replay remains
the certificate.

A generic GKZ/Pfaffian system followed by equal-mass restriction is not the first implementation
choice. Primary examples show that physical restrictions are commonly singular, ranks change on
special strata, resonant systems acquire subsystems, and even the equal-nonzero-mass bubble lacks
a straightforward generic-to-physical restriction. Symbolica also has none of the required
D-module restriction, module-order, or Bernstein--Sato machinery in its public API.

At `K = 21`, a degree-three modular pilot is only a discriminating experiment. Degree four already
has a raw envelope of 455,400 source rows before module components or elimination fill. A lane
that requires degree four, duplicates a filled sparse matrix for many primes, produces a large
terminal quotient, or accumulates unbounded epsilon-pole debt must be killed even if it closes
`K = 6`.

## Audited Symbolica surface

The vendored version is declared at `vendor/symbolica/Cargo.toml:5-17`. The audit found the
following public capabilities.

| Need | Symbolica 2.2.0 | RustRed responsibility |
| --- | --- | --- |
| Sparse multivariate polynomials | Yes | Variable and chart conventions |
| Rational functions | Yes | Guard and denominator-stratum policy |
| Finite fields | Yes | Prime/sample scheduling and rejection |
| Sparse row reduction | Yes | Checked assembly, pivot consensus, certificates |
| Dense exact solving | Yes | Select small exact pivot minors |
| CRT and rational reconstruction | Yes | Multi-entry orchestration and stopping proof |
| Polynomial interpolation | Partial | Adaptive multivariate rational reconstruction |
| Commutative ideal F4 | Yes | Do not mistake it for a shift-module basis |
| Free-module or syzygy Groebner basis | Not found | Relation-module and provenance layer |
| Ore/Weyl/difference algebra | Not found | Extensional shift semantics |
| Macaulay/border-frame builder | Not found | Frame, border, and completion controller |
| Sparse kernel/membership certificate | Not found | Recover and replay source multipliers |
| D-module restriction or `b`-functions | Not found | Avoid or isolate in a research lane |

“Not found” means a repository-wide inspection of the vendored 2.2.0 Rust source and examples did
not expose the named facility. It is not a claim about other Symbolica releases or unpublished
APIs.

### Polynomial and rational-function primitives

**Verified Symbolica facts.**

- `PolynomialRing<R, E>` is public in
  `vendor/symbolica/src/poly/polynomial.rs:32` and implements the generic ring interface.
- `MultivariatePolynomial<F, E, O>` is public at
  `vendor/symbolica/src/poly/polynomial.rs:290`. Its representation is sparse in terms but stores
  one dense exponent vector per term.
- Public construction, variable unification, coefficient mapping, differentiation, evaluation,
  quotient/remainder, and monic normalization live in that same file. This is sufficient for
  exact coefficient manipulation and replay.
- The built-in positive exponent choices are `u8`, `u16`, and `u32` in
  `vendor/symbolica/src/poly.rs:607-635`. The default `u16` is adequate for a deliberately
  low-degree Macaulay pilot; RustRed does not need a new exponent type for it.
- Public monomial orders include lexicographic and graded reverse lexicographic order in
  `vendor/symbolica/src/poly.rs:677-721`. The `MonomialOrder` trait permits custom orders, but no
  position-over-term or term-over-position free-module order was found.
- `RationalPolynomialField<R, E>` is public in
  `vendor/symbolica/src/domains/rational_polynomial.rs:40`. Its elements expose exact numerator
  and denominator polynomials and implement a field when the coefficient domain supports the
  required Euclidean and GCD operations.
- `MultivariatePolynomial::newton_interpolation` is public in
  `vendor/symbolica/src/poly/gcd.rs:343`. It interpolates one variable from polynomial-valued
  samples. It is a useful kernel, not an adaptive sparse multivariate rational reconstructor.

RustRed already uses a Symbolica rational polynomial over integers as its coefficient model in
`crates/rustred-core/src/algebra/coefficient/model.rs`. The finite-frame lane should therefore add
small foundry-private adapters, not another general symbolic-expression wrapper.

**RustRed proposal.** Evaluate rational coefficients modulo a prime by evaluating the public
numerator and denominator separately, rejecting a point when the denominator vanishes, and only
then dividing in the finite field. Track every rejected prime and sample as guard evidence. A
successful value at one generic sample says nothing about an exceptional denominator stratum.

### Finite fields, CRT, and reconstruction

**Verified Symbolica facts.**

- `Zp = FiniteField<u32>` and `Zp64 = FiniteField<u64>` are public in
  `vendor/symbolica/lib/numerica/src/domains/finite_field.rs:30-32`. The implementation uses
  Montgomery arithmetic and provides prime iterators.
- `Integer::chinese_remainder` is public in
  `vendor/symbolica/lib/numerica/src/domains/integer.rs:1697`.
- `Rational::maximal_quotient_reconstruction` and scalar
  `Rational::rational_reconstruction` are public in
  `vendor/symbolica/lib/numerica/src/domains/rational.rs:1066-1196`. The example
  `vendor/symbolica/examples/rational_reconstruction.rs` demonstrates the scalar interface.

These routines supply exact scalar kernels. No public controller was found for reconstructing a
large sparse vector of multivariate rational functions with common support, shared denominator,
bad-prime quarantine, adaptive degree bounds, and an exact stopping certificate.

**RustRed proposal.** RustRed owns:

1. deterministic prime and sample streams;
2. denominator-prime and singular-sample rejection;
3. stable pivot and support consensus across independent samples;
4. CRT accumulation for all selected entries;
5. nested Newton or sparse interpolation scheduling;
6. rational reconstruction and fresh-prime validation; and
7. exact replay over Symbolica rational polynomials.

Fresh-prime agreement is still probabilistic screening. Exact replay against regenerated ordinary
IBP sources is the proof.

### Sparse and dense linear algebra

**Verified Symbolica facts.**

- `SparseMatrix<F>` is a public CSR matrix at
  `vendor/symbolica/lib/numerica/src/tensors/sparse.rs:234`. Its column identifiers and row count
  are `u32`; row pointers use `usize`.
- Sparse APIs are publicly reachable through `symbolica::tensors::sparse`, via
  `vendor/symbolica/src/tensors.rs` and
  `vendor/symbolica/lib/numerica/src/tensors.rs`, although the sparse types are not in the main
  prelude.
- `SparseRowReducer<F>` and `LuLMode` are public at
  `vendor/symbolica/lib/numerica/src/tensors/sparse.rs:1456-1513`. Rows can be added
  incrementally; public accessors expose `L`, `U`, and pivots.
- In full `L` mode, the implementation records `L U = A`; its exact-rational test is near
  `vendor/symbolica/lib/numerica/src/tensors/sparse.rs:2630`.
- Each row reducer owns dense scratch storage proportional to the column count. Parallel back
  substitution performs extra work and may permute output rows relative to serial execution, as
  documented near `vendor/symbolica/lib/numerica/src/tensors/sparse.rs:2397`.
- The public sparse `solve` rejects underdetermined systems. No public sparse `solve_any`, kernel,
  or direct row-membership-with-source-certificate API was found.
- The public dense `Matrix` in
  `vendor/symbolica/lib/numerica/src/tensors/matrix.rs:706` provides field row reduction,
  `solve`, `solve_any`, and rank. It is appropriate for selected small exact minors, not a dense
  representation of a `K = 21` Macaulay matrix.

The CSR constructors deserve a hard boundary. `from_triplets` relies on debug assertions for some
ordering assumptions, while `from_csr` does not canonicalize duplicate or explicit-zero entries.
RustRed must validate dimensions, sort columns, combine duplicates, drop zeros, and check `u32`
conversion before handing a matrix to Symbolica.

The public sparse matrix owns its pattern and values. No public shared-pattern view was found, so
naively constructing one reducer per prime duplicates the CSR pattern, values, elimination state,
and fill. Prime-level parallelism is still the most deterministic parallel lane, but it requires a
strict memory semaphore and bounded result streaming.

**RustRed proposal.** Use modular sparse reduction to choose independent original source rows and
pivot columns. For a requested border relation, solve one selected square pivot minor exactly,
using the dense exact API when it remains small, then replay the recovered source multipliers over
all exact columns. `L` can accelerate the mapping from a dependent row through `U`, but RustRed
must still map that relation back to original source instances. A pivot list alone is not source
provenance.

### Groebner and module capabilities

**Verified Symbolica facts.** `GroebnerBasis<R, E, O>` is public in
`vendor/symbolica/src/poly/groebner.rs:118`. Its constructor runs an internal F4-style algorithm,
and public normal-form reduction is available in the same file. The optimized finite-field
echelonization is an arithmetic asset for commutative polynomial ideals.

No public API was found for free-module Groebner or syzygy bases, module monomial orders,
Ore/Weyl/difference algebras, Janet completion, standard pairs, D-module restriction,
Bernstein--Sato polynomials, or characteristic varieties. The internal F4 matrices do not expose
source transformations.

**RustRed proposal.** Use commutative F4 only as an optional discovery aid for coefficient ideals,
guard decomposition, or a proposed commutative order ideal. It cannot certify membership in the
ordinary-IBP shift module. Do not copy or fork Symbolica's private F4 implementation into RustRed.

## Direct extensional finite-frame design

### Semantics and data ownership

An ordinary source has the form

```text
sum_s c_s(a, D) I(a + s) = 0.
```

For a source translated by `t`, RustRed regenerates it at `a + t`. This evaluates coefficients at
the translated index before registering columns `I(a + t + s)` and therefore realizes the Ore
identity `E_i c(a) = c(a + e_i) E_i` extensionally. No symbolic Ore-algebra type is required.

RustRed should own the following bounded, foundry-local records:

- `FrameKey`: the canonical sector-chart integral key;
- `ColumnRegistry`: deterministic integral-key-to-column assignment;
- `SourceInstance`: base source, translation, sector, and guard provenance;
- `SparsePattern`: validated CSR pattern, shareable before Symbolica matrix construction;
- `PrimeSample`: prime, coefficient assignment, and rejected denominators;
- `PivotFingerprint`: independent source rows, pivot columns, and rank;
- `ExactSourceCertificate`: reconstructed multipliers and exact replay digest;
- `TerminalRelationModule`: exact relations among redundant terminals; and
- `PoleDebtCertificate`: epsilon valuations and a global depth bound.

These are algorithmic records, not replacements for Symbolica polynomial, rational, finite-field,
or matrix primitives.

### Two implementation lanes

1. **Exact `K = 6` reference lane.** Assemble exact rational-polynomial coefficients. Use sparse
   reduction only when coefficient growth remains modest, otherwise select a modularly discovered
   pivot minor and solve it exactly. This lane validates chart coordinates, source translation,
   and certificate replay.
2. **Modular production candidate.** Reuse one canonical source/column plan across bounded prime
   workers. Establish pivot consensus, reconstruct only border certificates and terminal relations,
   and replay each exactly. Never reconstruct a full echelon form merely because it exists
   modulo several primes.

The exact lane is a reference and falsification tool. Direct exact rational-function elimination
is not a credible `K = 21` default because expression swell and polynomial GCD work can dominate.

### Finite-frame completion certificate

Let `O` be a finite connected frame of proposed terminals and let `R` be the exact relation module
among them. A production artifact may claim closure only after proving all of the following:

1. every element of the complete first shift border of `O`, in every supported direction, has an
   exact ordinary-source certificate into `O`, lower sectors, zeros, or factorizations;
2. every coefficient guard and every nonempty guard intersection has at least one complete owner
   with deterministic precedence;
3. all applicable owners have identical exact normal forms on overlaps;
4. each induced shift action preserves `R` and annihilates the original source module;
5. shift actions commute modulo `R` wherever the discrete shifts commute;
6. lower-sector maps agree with immutable lower-sector artifacts;
7. reduction is strictly descending under the declared well-founded order; and
8. the terminal manifest is universal for the declared family, sectors, and guard strata.

For a redundant frame, exact rank and generators of `R` distinguish a finite quotient from a
finite sample that simply missed another independent direction. A conventional master count,
critical-point count, flat numerical connection, or stable modular rank is a diagnostic only.

## Why generic restriction is a high-risk lane

### Published failure evidence

The Macaulay/Pfaffian construction of
[*Macaulay Matrix for Feynman Integrals: Linear Relations and Intersection Numbers*
](https://arxiv.org/abs/2204.12983) assumes a supplied standard-monomial basis and a
zero-dimensional ideal. Its algorithm raises the Macaulay degree until required border rows enter
the row span; the termination argument does not give a small a priori degree. Its six-point
example already has a `945 x 958` block at degree two, and complete functional reconstruction is
left as future work. Finite-field rank tests in that work are probabilistic discovery checks.

[*Restrictions of Pfaffian Systems for Feynman Integrals*
](https://arxiv.org/abs/2305.01585) states that physical restrictions are typically singular and
may require gauge/Moser reduction. Its direct restriction algorithm assumes a regular holonomic
ideal and a restricted basis; its rational restriction route needs free-module Groebner bases,
position-over-term orders, `b`-function/truncation information, and the restricted rank. Examples
in the paper reduce generic ranks `9`, `33`, and `115` to physical ranks `3`, `7`, and `7`. Another
example has generic rank `238`, Euler-characteristic expectation `12`, and known physical rank `8`
after an additional invariant-symmetry restriction.

[*Resonance and Differential Reduction of Feynman Integrals*
](https://arxiv.org/abs/2606.09978) emphasizes that integer propagator powers are resonant and can
produce reducible subsystems. It identifies physical-locus restriction as a central difficulty.
For the equal, nonzero-mass bubble it explicitly reports no straightforward restriction from the
generic GKZ system to physical kinematics. For higher sunrise/banana systems, Euler equations alone
do not eliminate the nonphysical derivatives and the state vector must be enlarged.

Rank jumps are not merely hypothetical. Matusevich and Walther give an explicit `A`-hypergeometric
system with generic rank four and rank five at a special parameter in
[*Arbitrary rank jumps for A-hypergeometric systems*
](https://arxiv.org/abs/math/0404183), and construct arbitrarily large rank-volume gaps.

### Consequence for RustRed

These results do not disprove direct finite-frame completion from ordinary equal-mass IBPs. They do
disprove the inference that a generic GKZ standard frame or rank can simply be specialized to the
resonant equal-mass locus. A generic-to-physical lane would need its own exact restriction and
rank-stratification implementation before it could become a closure authority.

The direct extensional lane starts from the physical unit-mass ordinary sources and therefore
avoids that particular restriction problem. It can still encounter coefficient-zero strata,
rank jumps in `D`, or an unbounded completion degree; its guard-completion machinery must expose
rather than sample past them.

## Adversarial six-loop scaling

With 36 ordinary momentum-space sources at six loops, the number of commutative multiplier
monomials through total degree `d` is

```text
M(K, d) = binomial(K + d, d).
```

At `K = 21` the raw source-row envelope is:

| degree `d` | `M(21, d)` | 36-source rows |
| ---: | ---: | ---: |
| 1 | 22 | 792 |
| 2 | 253 | 9,108 |
| 3 | 2,024 | 72,864 |
| 4 | 12,650 | 455,400 |

If a row has an illustrative average of 50 nonzeros, degrees three and four contain about 3.64
million and 22.77 million input nonzeros. A finite-field value plus a `u32` column identifier has
an eight-byte theoretical floor, giving about 29 MiB and 182 MiB before row pointers, allocation,
dense scratch, duplicated prime workers, `U`, or elimination fill. At a more realistic 12--16
bytes per stored input nonzero, the inputs alone are roughly 44--58 MiB and 273--364 MiB.

Those figures are envelopes, not measurements: source degrees can reduce the multiplier set, while
module components and fill can increase it drastically. Sparse elimination fill of 10--100 times
the input is entirely capable of turning degree four into a multi-gigabyte or multi-node failure.
The public Symbolica API does not let independent reducers share one immutable CSR pattern.

### Candidate ranking by likely `K = 21` cost

1. **Direct physical, modular, compressed sources at `d <= 2`.** Best prospect. It still needs
   exact border replay, terminal relations, and all guards. Failure to close at degree two is useful
   evidence, not permission to declare new masters.
2. **The same lane at `d = 3`.** Researchable with bounded prime workers, row streaming, aggressive
   source preconditioning, and reconstruction of selected certificates only. Measure fill before
   committing to functional reconstruction.
3. **Generic GKZ/Pfaffian then exact physical restriction.** Worse despite potentially compact
   equations: auxiliary coefficient directions, a larger generic rank, singular restriction, and
   missing D-module APIs can dominate the Macaulay solve.
4. **Full exact rational-function elimination or a full `d >= 4` modular matrix.** Worst default.
   Kill unless an earlier structural experiment shows exceptional sparsity and a small quotient.

Gram/logarithmic sources, commutative F4, learned pivot orders, numerical rank, and critical-point
counts may compress or guide discovery. None is a closure certificate.

## Terminal evaluation and epsilon-pole debt

Finite algebraic closure is insufficient when its universal terminal quotient cannot be evaluated.
Let `t = |O|` and `r = dim(O/R)`. RustRed must give exact generators for `R`, select an independent
quotient basis of size `r`, and map every redundant terminal into it. AMFlow or an offline evaluator
should solve/evaluate the independent basis simultaneously; evaluating `t` unrelated objects loses
the purpose of the quotient certificate.

Before accepting a nonminimal frame, record:

- `t`, exact quotient rank `r`, and relation sparsity;
- coupled differential/difference-system dimension and singular points;
- required epsilon order and precision loss for every terminal;
- peak memory, wall time, and checkpoint/storage volume for simultaneous evaluation; and
- whether the evaluator itself requires the unavailable reduction, which would be circular.

Vakint's shipped MATAD values through three loops are a useful high-precision offline diagnostic.
Most currently shipped FMFT four-loop constants have only about 26--50 digits, with only a few near
20,000 digits; generic four-loop high-precision terminal data would require regeneration or AMFlow.
Numerical Laurent parity may validate a different basis, but it does not certify RustRed closure.

After substituting `D = 4 - 2 epsilon`, annotate each rule coefficient with its exact epsilon-adic
valuation. The artifact must prove a finite upper bound on accumulated negative valuation along
every reduction path. A repeatable transition with growing pole debt means that no fixed Laurent
depth is universal for arbitrary rank, even when the algebraic terminal set is finite. Exact
rational arithmetic postpones expansion but does not remove genuine coefficient poles.

A bounded Vakint input envelope may declare and prove a finite depth for that envelope. It must not
be advertised as an all-rank universal numerical artifact.

## Falsification programme

### P0: API and certificate smoke test

Build a tiny sunset frame twice: once with exact coefficients and once at at least three independent
finite-field samples. Append a known dependent border row, recover its original-source
multipliers through a selected pivot minor, and replay every exact column. Remove one source and one
border row in separate negative controls; both holes must be reported.

Reject the design if the public sparse API cannot support deterministic row selection without
unbounded copying, if exact replay cannot recover the original-source provenance, or if modular
pivots do not stabilize after excluding demonstrably singular samples.

### P1: complete `K = 6` kill test

For completion degrees zero through three:

1. cover the current exceptional corners and numerator rays together with the complete first border;
2. compare ordinary sources with optional Gram/logarithmic source compression;
3. require multi-prime pivot stability and exact rational-polynomial reconstruction;
4. deliberately sample every discovered denominator-zero stratum and close it separately;
5. prove exact terminal relations, action invariance, commutation, and lower-sector compatibility;
6. compare deterministic artifacts and reductions at one and several supported worker counts; and
7. measure pole debt at target ranks `1, 2, 4, 8, 16` and preflight simultaneous terminal
   evaluation against the available MATAD oracle.

Kill the finite-frame hypothesis if the completion degree keeps growing with the probed border,
the exact frame relation module does not stabilize, or pole debt grows without a provable bound.

### P2: `K = 10` scaling proxy

Attempt degrees one through three. Record generated rows, canonical nonzeros, pivot count, `U` fill,
peak resident memory per prime, reconstruction support and degree, exact replay time, `t`, `r`, and
terminal-evaluation cost. Use FMFT only as an offline numerical oracle, recognizing its current
precision limits.

Stop before larger completion if degree-three fill or reconstruction projects beyond the agreed
single-node budget, or if a universal terminal quotient cannot be evaluated affordably.

### P3: `K = 21` structural proxies

Run structure-only degrees one through three on both a six-loop banana and a representative
15-propagator/six-ISP family. Do not launch a full degree-four calculation by default. Forecast
rows, nonzeros, elimination fill, per-prime duplication, exact-certificate support, terminal rank,
pole debt, and simultaneous AMFlow cost.

The lane fails its six-loop purpose if any of the following occurs:

- no stable finite frame closes the complete first border;
- completion requires `d > 3` without a source-compression breakthrough;
- `d = 3` fill or prime duplication exceeds the bounded RAM/I/O design;
- pivots vary for reasons not captured by exact guard strata;
- selected exact certificates suffer prohibitive coefficient growth;
- generic-to-physical restriction becomes necessary for correctness;
- the universal terminal quotient is too large for simultaneous offline or AMFlow evaluation;
- accumulated epsilon-pole debt is unbounded; or
- any exact regenerated-source replay fails.

Passing modular rank, flatness, master-count, or numerical-value tests cannot override one of these
failures.

## Implementation boundary and priority

The recommended priority is:

1. implement the checked CSR assembler and tiny exact/modular certificate smoke test;
2. complete the direct physical-stratum `K = 6` experiment through degree three;
3. add exact redundant-frame relations and epsilon-pole accounting before enlarging the frame;
4. measure the `K = 10` scaling proxy; and
5. permit a `K = 21` degree-three structural pilot only if the prior gates pass.

Do not first implement generic GKZ restriction, a new polynomial engine, a new finite-field layer,
a copied F4 kernel, or an all-purpose Ore algebra. Symbolica already owns the arithmetic primitives;
RustRed's missing value is the source-aware finite-frame compiler and its exact closure certificate.

## Primary sources

- T. Chestnov et al., *Macaulay Matrix for Feynman Integrals: Linear Relations and Intersection
  Numbers*, [arXiv:2204.12983](https://arxiv.org/abs/2204.12983).
- T. Chestnov et al., *Restrictions of Pfaffian Systems for Feynman Integrals*,
  [arXiv:2305.01585](https://arxiv.org/abs/2305.01585).
- R. Britto, T. W. Grimm, and A. Hoefnagels,
  *Resonance and Differential Reduction of Feynman Integrals*,
  [arXiv:2606.09978](https://arxiv.org/abs/2606.09978).
- L. F. Matusevich and U. Walther,
  *Arbitrary rank jumps for A-hypergeometric systems through Laurent polynomials*,
  [arXiv:math/0404183](https://arxiv.org/abs/math/0404183).
