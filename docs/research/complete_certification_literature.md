# Complete certification by mathematical fragment

Primary-source research and pinned-source audit, 2026-09-16. This document is a
proposal, not an implemented complete verifier, a four-loop closure result, or
authorization to add another CAS. It extends
[the semi-numerical experiments](seminumerical_proof_systems.md) and audits the
scope of [the convergence proposal](certification_convergence.md). See also the
complementary [modular-certificate analysis](modular_certificates_and_coverage.md).

## Executive conclusion

There is a principled way out of repeated local guard patches, but not a single
numerical test that makes all remaining proof obligations disappear:

1. Give application domains one exact meaning, including the target, integer
   lattice, fixed faces, whole exclusions, and original poles.
2. Preserve an exhaustive case-split proof during search instead of asking the
   publisher to rediscover the entire partition from independent rules.
3. Use inexpensive modular/numerical discovery to propose small witnesses;
   verify their full identities and domain hypotheses exactly.
4. Provide a complete fallback for an explicitly recognized domain language.
   Presburger arithmetic is the natural complete target for affine integer
   coverage. Algebraic and real-algebraic methods cover different nonlinear
   fragments, not arbitrary nonlinear integer feasibility.
5. Report unsupported language, exhausted resources, genuine counterexamples,
   and missing search rules separately. No finite resource cap supplies a
   mathematical completeness guarantee.

The strongest near-term architecture is consequently a small exact witness
checker plus a domain-aware case DAG and a fragment dispatcher. A low-degree
multiplier finder is a useful accelerator within it, not its completeness
argument. Comprehensive parametric elimination is a credible longer-term way
to preserve exceptional cases, but is not a shortcut around integer geometry
or the infinite shifted IBP problem.

## 1. What can actually be complete?

Here, **complete decision procedure** means termination with the correct answer
for every finite input in its stated language, without a fixed operational cap.
**Refutational completeness** means every false feasibility claim in a stated
class has an eventually discoverable finite certificate; it does not promise
termination on feasible inputs. Neither means uniformly small certificates or
fast execution. The current RustRed bounded paths claim sound acceptance, not
either global completeness property.

| Recognized fragment | Principled complete route | What it does not establish |
| --- | --- | --- |
| Boolean combinations of affine integer equations, inequalities, and constant-modulus congruences | Presburger quantifier elimination; proof-producing Cooper/Omega-style reasoning | Arbitrary polynomial guards or multiplication of unknown indices |
| Polynomial equations over an algebraically closed field | Gröbner decision procedure; Nullstellensatz refutations with sufficient degree | Absence of integer points when complex points remain |
| Polynomial equations with polynomial disequalities over that field | Localization/Rabinowitsch inverse variables, then algebraic decision/refutation | Order, sector signs, integer divisibility |
| Polynomial equations/inequalities over the reals | Real quantifier elimination, CAD/CAC; general Positivstellensatz infeasibility certificates | Integer emptiness when real points remain |
| An explicitly finite integer box | Exhaustive exact enumeration, preferably compressed by stronger proofs | An unbounded sector beyond that box |
| Complete zero-dimensional algebraic solution set with exact filtering | Enumerate every algebraic solution and test integer/domain membership | Positive-dimensional integer feasibility |
| Unrestricted polynomial equations over the integers | No total decision algorithm exists | This says nothing by itself about decidability of the particular IBP-derived subclass |

The final distinction is essential. Hilbert's tenth problem concerns unrestricted
Diophantine input. No reduction from that problem to RustRed's actual IBP guard
language is established here. Conversely, no theorem currently shows that all
future IBP guards reduce to one of the complete fragments above.
[Matiyasevich's own account](https://logic.pdmi.ras.ru/~yumat/H10Pbook/par_1_1.htm).

The sensible promise is therefore: complete coverage for a precisely specified
affine-integer domain sublanguage, and exact accepted certificates plus explicit
inconclusive results for richer obligations. It is not yet a complete promise
for every denominator guard appearing in the vacuum lane.

## 2. Affine integer coverage: use the lattice, not only the real hull

Presburger arithmetic allows addition, order, integer constants, Boolean
connectives, and quantification; multiplication by a fixed integer is linear.
Divisibility by a fixed modulus is expressible and naturally appears during
elimination. Cooper's procedure supplies quantifier elimination, and subsequent
work gives proof-producing and mechanically verified versions. These are
particularly relevant because coverage is a universal statement whose negation
is an existential uncovered-point query.
[Cooper, 1972](https://www.cs.cmu.edu/~emc/spring06/home1_files/Cooper.pdf),
[Chaieb and Nipkow, proof synthesis](https://www21.in.tum.de/~nipkow/pubs/presburger.html),
[Chaieb and Nipkow, verified reflection](https://www.proof.cit.tum.de/~nipkow/pubs/lpar05.html),
[Nipkow, formalized linear quantifier elimination](https://isa-afp.org/entries/LinearQuantifierElim.html).

Pugh's Omega test handles integer equalities and inequalities using exact
projection, tightening, and necessary splitting; it is not merely real
Fourier–Motzkin elimination. Its favorable practical behavior does not remove
worst-case growth. It is a model for a complete integer fallback beneath cheap
affine checks, not a justification for silently replacing integer domains by
rational charts.
[Pugh, original paper](https://homes.luddy.indiana.edu/achauhan/Teaching/B629/2006-Fall/CourseMaterial/1992-cacm-pugh-omega_test.pdf),
[author's Omega project](https://www.cs.umd.edu/projects/omega/).

Examples exposing the gap in a real-only checker are `2*x=1` and
`1 <= 3*x <= 2`: both have real solutions but no integer solutions. Eliminating
`x` from `2*x=y` must preserve `y ≡ 0 (mod 2)`. A rational parameterization
without this condition enlarges the set; real emptiness of the enlarged set is
a sound sufficient proof, but its nonemptiness is not an integer witness.

For RustRed, a complete affine query must retain the Boolean structure of
`target AND bounds AND NOT(OR whole-exclusion-ANDs)`. Negating a conjunction
produces a disjunction, not an arbitrary selected sibling. Descent after a
fixed shift can often be expressed by finite affine/sign cases; the dispatcher
must establish that translation rather than assume every order comparison is
linear. Sector activation and terminal membership also belong in the query.

Exact Farkas witnesses are excellent for rational linear contradictions.
Integer cuts, rounding, congruence arguments, and branch coverage are needed
beyond them. Cheung–Gleixner–Steffy's certificate architecture separates a large
branch-and-cut solver from a smaller exact checker; its VIPR implementation is
a useful engineering precedent. VeriPB is a related cutting-planes checker for
pseudo-Boolean, hence bounded Boolean, problems—not a ready unbounded integer
domain service.
[Verifying Integer Programming Results](https://arxiv.org/abs/1611.08832),
[VIPR](https://github.com/scipopt/vipr), [VeriPB](https://veripb.org/).

Recommended completeness boundary: accept a normalized affine-integer formula
with explicit congruences, supply a proof-producing complete procedure for it,
and permit faster RREF/Farkas/box certificates to terminate early. A numerical
LP solver can propose a dual witness; exact coefficients and every bound must
be checked. Heuristic branch-and-bound without a termination argument is not
the fallback completeness theorem.

## 3. Nonlinear equalities: small ideals, localization, and their limits

For polynomial equations `C_j=0`, a certificate

```text
1 = Σ_j U_j C_j
```

proves algebraic infeasibility. NulLA searches increasing multiplier degrees by
linear algebra. The Nullstellensatz supplies finite witnesses for systems
infeasible over the algebraic closure; known degree bounds yield a theoretical
decision route, although their size can be prohibitive. Sparse supports and low
degrees are optimizations, not universal bounds.
[De Loera, Lee, Malkin and Margulies, §§1–2](https://www.math.ucdavis.edu/~deloera/RECENT_WORK/jsc09_issac08.pdf).

Our guard variant is often smaller:

```text
T = Σ_j U_j C_j + Σ_k V_k A_k,     D entails T != 0,
```

where `A_k=0` are authenticated target/fixed-face equations and `D` is the
actual domain. The exact identity forces `T=0` if all coefficients vanish.
The separate nonvanishing proof may be affine, a certified factor product,
or another accepted witness. This is a derivation for RustRed's obligations,
not a claim that NulLA automatically chooses a useful `T`.

The coefficient interpretation matters: for `Q(n,d)=sum_j C_j(n)*d^j`, at a
fixed admitted index point the polynomial is identically zero in generic `d`
exactly when every coefficient vanishes. Proving that this never happens does
not prove `Q(n,4)!=0`; isolated dimensional poles and Laurent expansion remain
separate semantics. For several independent base parameters, use the complete
monomial coefficient vector, not one numerical specialization.

Polynomial disequalities can be encoded by auxiliary field variables:
`S!=0` becomes `y*S-1=0`; equivalently work in a localization/saturated ideal.
A certificate using a product of declared nonzero polynomials is valid only
where all those hypotheses hold. The zero branches must remain explicit.
Clearing denominators in an identity cannot authorize evaluation at an original
pole. For a whole excluded variety, implication may require radical membership
(`E^k` in an ideal), not plain degree-one ideal membership. Each equation of the
same exclusion must be established; equations from different exclusions cannot
be combined into a fictional covered branch.
[Ballarin and Kauers, §3](https://wwwbroy.in.tum.de/publ/papers/rwca02.pdf),
[practical algebraic and Nullstellensatz proof checkers](https://link.springer.com/article/10.1007/s10703-022-00391-x).

Completeness is exact but narrower than the application: increasing all
multiplier degrees eventually refutes a localized algebraically inconsistent
system. It need not refute `x^2+1=0` over complex numbers, although that equation
has no real solution; nor can it decide integer emptiness of a variety with
real/complex points. Restricting forever to affine multipliers or one selected
coefficient pair is an additional incomplete heuristic.

Full Gröbner computation is a possible algebraic fallback, not the necessary
first step. If a proposed basis is untrusted, proving its own S-polynomials
reduce to zero does not prove it generates the original ideal. Original-to-basis
and basis-to-original membership, or an authenticated computation, is needed.
For a small desired consequence, directly checking its original-generator
multiplier identity avoids that larger provenance obligation.

## 4. Real certificates: powerful sufficient integer proofs, not integer decisions

For `g_i>=0` and `h_j=0`, the general real infeasibility certificate has the form

```text
-1 = Σ_alpha sigma_alpha * product_i g_i^(alpha_i) + Σ_j v_j h_j,
```

with sums of squares `sigma_alpha` and products from the full preordering.
Strict inequalities and disequalities require their proper encodings or the
general monoid form. This is the Stengle/Positivstellensatz setting, not merely
one chosen SOS relaxation. Exact checking consists of polynomial identity and
positivity evidence. Real emptiness implies integer emptiness; the converse
fails.
[Stengle, 1974](https://doi.org/10.1007/BF01362149),
[Parrilo and Lall, explicit certificate formulations](https://www.mit.edu/~parrilo/cdc03_workshop/10_positivstellensatz_2003_12_07_02_screen.pdf).

Bounded-degree semidefinite searches form useful hierarchies. Completeness of
the full real certificate family does not imply that a floating-point SDP
solver reliably finds an exact certificate at its boundary. Exact real
algebraic decision methods can provide a theoretical fallback for the finite
coefficient feasibility question, but that is a substantial service, not a
rounding option.
[Parrilo, 2003](https://doi.org/10.1007/S10107-003-0387-5).

Important distinctions:

- Schmüdgen concerns strict positivity on compact basic closed sets and a
  preordering. Putinar's smaller quadratic module needs an Archimedean
  hypothesis, stronger than merely observing compactness of the set. Powers
  proves rational representations under stated hypotheses, including an
  explicit bounding-ball generator in the rational Putinar result. Infinite
  RustRed sign boxes do not automatically satisfy these assumptions.
  [Powers](https://arxiv.org/abs/0911.1331).
- Numerical Gram matrices can be projected/rounded to exact rational SOS
  certificates under the paper's standing strictly feasible Gram-matrix
  assumption. A tiny floating negative eigenvalue
  cannot simply be ignored; the final matrix or explicit square decomposition
  must prove positivity exactly.
  [Peyrl and Parrilo](https://www.mit.edu/~parrilo/pubs/files/PeyrlParrilo-ComputingSumOfSquaresDecompositionsWithRationalCoefficients.pdf).
- Rational polynomials can be SOS over the reals but not SOS over the rationals.
  Thus a fixed rational Gram ansatz need not contain the desired certificate.
  This does not refute broader Positivstellensatz alternatives with different
  degree, products, denominators, or algebraic coefficients.
  [Scheiderer](https://arxiv.org/abs/1209.2976).
- Recent work handles degenerate SDPs without assuming rational feasible
  matrices: Kolmogorov–Naldi–Zapata's 2025 version uses a maximum-rank target
  assumption and polynomial equations with an isolated correct solution.
  It is a serious hybrid exact-certification direction, not a general cheap
  rational-rounding guarantee.
  [Certifying solutions of degenerate semidefinite programs](https://arxiv.org/abs/2405.13625).

CAD supplies a complete real-algebraic decision route; conflict-driven
cylindrical algebraic coverings aim to avoid constructing unnecessary cells.
Their sample points are backed by exact generalized regions and full coverage,
not interpreted as experimental proof. Recent work extends CAC to Boolean
structure and quantifiers. Lower-dimensional sections, singular roots, and
boundaries remain indispensable. Full formal certificate export/checking is a
separate engineering problem; early CAC implementation literature explicitly
does not supply a finished general formal checker.
[Ábrahám et al., CAC](https://arxiv.org/abs/2003.05633),
[implementation discussion](https://pure.coventry.ac.uk/ws/files/54490968/Post_Print.pdf),
[Nalbach and Kremer, 2025 version](https://arxiv.org/abs/2411.03070),
[Babatunde, England and Sadeghimanesh, 2026 optimization](https://arxiv.org/abs/2601.14424).

Recommendation: do not make a broad CAD/SOS implementation the next local
patch. Keep a future exact real backend as a declared extension. It can prove
many nonlinear integer obligations by real relaxation, but must return a
real-feasible/integer-unresolved result when that implication is insufficient.

## 5. Semi-numerical discovery: exact output is the useful certificate

For a finite multiplier ansatz, construct a native sparse coefficient system
`M*u=t`. Modular row reduction can propose pivots/support, and reconstruction
can propose rational coefficients. Verify the complete polynomial residual
over the original rational/integer coefficient ring. That last check is
deterministic and does not require believing a reconstructed rank or a random
matrix challenge. It is often much cheaper than symbolic discovery.

The modular Gröbner literature demonstrates useful prime-image learning and
lifting, but bad primes, changing rank/support, and reconstruction validation
remain explicit concerns. msolve is a relevant modern primary implementation
paper, not a proposed dependency here.
[Arnold, modular Gröbner algorithms](https://www3.risc.jku.at/research/theorema/Groebner-Bases-Bibliography/gbbib_files/publication_355.pdf),
[Berthomieu, Eder and Safey El Din, msolve](https://arxiv.org/abs/2104.03572).

Fast interactive certificates for sparse determinants/minimal polynomials and
polynomial-matrix properties are useful comparison points. The cited protocols
are probabilistically sound; the polynomial-matrix work largely concerns the
univariate ring `F[x]` and explicitly distinguishes its module from its fraction
field. They do not by themselves provide exact integer-domain coverage.
[Dumas et al.](https://arxiv.org/abs/1602.00810),
[Lucas, Neiger, Pernet, Roche and Rosenkilde](https://arxiv.org/abs/1807.01272).

Sampling can choose an ansatz or expose a counterexample. A candidate
counterexample must replay exactly against the complete target, bounds,
exclusions, and original denominators. Agreement on finite samples does not
exclude thin exceptional loci. Modular sign tests do not represent order;
`S!=0` over the integers does not imply `S mod p != 0`. A complete residue
obstruction to necessary integer equalities is sound, but lack of such an
obstruction is not completeness: the intersective polynomial
`(x^2-13)(x^2-17)(x^2-221)` has a root modulo every positive modulus and no
integer root.
[Lê and Spencer, intersective-polynomial discussion](https://home.olemiss.edu/~leth/papers/intersective_polynomials_II_2.pdf).

A fair complete search within the algebraic-refutation fragment must eventually
expand beyond its preferred sparse supports and selected witnesses. Keep the
fast mode explicitly bounded; schedule the complete fallback separately. Merely
trying more random points or more primes forever does not provide that fallback.

## 6. Preserve exceptional cases: comprehensive parametric elimination

Comprehensive Gröbner systems partition parameter space into constructible
strata with specialization-valid bases. Manubens–Montes describe a finite
dichotomic tree: each uncertain parameter polynomial is zero or nonzero,
and each terminal specification has a valid specialized basis. This addresses
the exact defect of retaining only a generic basis and its final denominators;
exceptional rank changes can disappear from that generic result.
[Manubens and Montes, §§1–3](https://mat.upc.edu/en/people/antonio.montes/cgbdiscriminant.pdf/@@display-file/file/CGBDiscriminant.pdf).

Parametric Gaussian elimination is an even closer precedent for a fixed source
matrix. Ballarin–Kauers carry a constraint context, split uncertain pivots,
and use algebraic reasoning to justify zero/nonzero decisions. They warn of
exponential regimes and expression swell. This is evidence for the design,
not a performance guarantee at RustRed's scale.
[Ballarin and Kauers, §§3–4](https://wwwbroy.in.tum.de/publ/papers/rwca02.pdf).

Proposed RustRed adaptation, not an existing feature:

- Record each split as the tautological pair `P=0` / `P!=0`, with the original
  full domain inherited by both children. A skipped branch needs an exact
  emptiness certificate. Shared subgraphs may avoid duplicate work, but the
  graph must be acyclic or otherwise have an independently justified proof
  interpretation.
- Attach source-combination identities and every pivot denominator to each
  applicable leaf. Keep algebraic row transformations distinct from domain
  simplifications. Integer signs/congruences are additional constraints, not
  properties inherited from algebraic closure.
- Attach rule/descent/zero/terminal evidence to all live leaves. A complete
  finite parametric matrix analysis may instead expose a genuinely unresolved
  leaf; the certificate must not turn it into an assumed master.

This construction can make partition coverage local and auditable instead of
recomputing a global Boolean cover. It does **not** prove that a finite shifted
source frame generates every needed IBP reduction, that every positive-dimensional
stratum has a decidable integer emptiness problem, or that every leaf has a
descending rule. New source generation and a termination/coverage argument for
the infinite integral family remain distinct obligations. Implementing CGS
would also require a native specialization-safe service or approved upstream
work; it is not permission to add a home-grown Gröbner engine.

## 7. Actual RustRed captures as acceptance tests

These examples explain why neither uniform local patches nor one universal
algebraic relaxation is sufficient. They are exact diagnostic deductions, not
claims that later rules have been verified.

### FG115, rule 76: an empty application domain, not a live upward step

The saved candidate's target is `T=1+n2+n9=0`. Its exclusions are the union of
four complete branches: `1+n8+n9=0`, `3+n8+2*n9=0`, `n8=0`, and `n2=0`.
Fixed coordinates are `n0=n1=n4=n5=n6=1`, `n3=n7=0`. Term zero shifts
`[0,0,0,0,0,0,0,-1,2,-1]` with coefficient

```text
-(1+n8) / ((1+n8+n9)*(3+n8+2*n9)).
```

The rejected piece fixes `n8=-1,n9=0`, with `n2<=-1`. Its first exclusion is
identically zero; the target additionally forces `n2=-1`. Therefore the actual
application domain is empty. The coefficient is `0/0`, not zero. The current
whole-rectangle containment path unnecessarily asks the target to hold over
the rectangular hull; target-relative implication is the needed generic proof.
The nearby `n2=-1,n8=-2,n9=0` control is outside all saved exclusions and has
coefficient `-1`; it prevents indiscriminate removal of the whole rule.

Evidence: `/tmp/rustred-fg115-bundle-diagnostic.QLM96W/DIAGNOSIS.md` and
`TARGET-RELATIVE-EXCLUSION-PROPOSAL.md`. The saved bundle is
`/tmp/rustred-split-candidates-release.qCn7z3/fg.candidates.toml`, SHA-256
`8079c94855ab3940b2aaaee52eb9265a0a3d0662c9dd0a847afb69d4eed22de6`.
The bundle preserves source seeds, not every regenerated source condition;
the control above is not a claim of complete source replay. Exclusion emptiness
already suffices for the failing piece.

Published-source integration points at milestone `725d756` are
`source_port/lower/verification.rs::application_is_proved_empty`,
`parametric/affine/box_containment.rs`,
`source_port/predicate_cover/consistency/implication.rs`, and
`solver/case/affine/chart.rs::canonical_equalities`. Exact rank implication can
certify an entire affine exclusion relative to the target and singleton face;
an inconsistent augmented system is **not** an implication proof. Integer
singleton values must remain native integers, not truncated compact powers.

### H229: singleton semantics and conservative resource estimates

On the captured piece let `z=n1=0`, `a=n3<=-1`, `b=n4<=-2`. The guard has
coefficients including

```text
C2 = 3*(1+b)*(-1-b-2*a+z)*(1+b-a+z)
C1 | (a=1+b+z) = -(1+b)^2*(3+3*b+z)*(z-2-2*b)
C1 | (z=0,a=1+b) = 6*(1+b)^4.
```

The first two nonconstant factors of `C2` that are not `1+b-a+z` are nonzero
by the captured signs. Thus simultaneous coefficient vanishing forces
`a=1+b`, and the restricted `C1` is nonzero. The widened point
`z=0,a=-1,b=-1` is a genuine zero and must not inherit that conclusion.

The failure is a prospective bit-budget bound after the singleton `z=0` is
not retained in the transient chart; substituting an affine expression for
`z` expands powers unnecessarily. This motivates canonical domain/singleton
alignment before algebraic work, not a higher default budget or permission
to assume cancellations. See
`/tmp/rustred-factor-policy-release.QSclTI/H229-FG115-NEXT-FAILURES.md`.

### FG83: small nonlinear certificate, supplied witness ansatz

With `a=n2`, `b=n7=n8`, the captured domain has `a<=-2,b<=-1` and

```text
C0 = 2*b*(b^2+(a+3)*b-a^2-2*a)
C1 = b*(2*a-3*b)
b^3 = 2*C0+(2*a+b+4)*C1.
```

Exact polynomial checking plus `b!=0` proves that the generic-`d` guard cannot
vanish. Widening to `b=0` gives real zeros. A bounded native Matrix experiment
recovered the multiplier coefficients from sample values and verified the
full identity; the coefficient pair, affine ansatz, and target witness `b^3`
were supplied. This was not autonomous witness discovery or a coverage proof
from sampling. The detailed experiment is recorded in
[the earlier research report](seminumerical_proof_systems.md).

These diagnostic polynomial calculations used installed Python Symbolica
`symbolica-77c1374`, independently of the pinned production runtime. The pinned
Rust API audit below is separate; no production build or code change was made
for this literature task.

## 8. Pinned Symbolica 3: reusable primitives and missing services

The workspace pins `symbolica = "=3.0.0"` and patches Symbolica, Numerica, and
Graphica to `vendor/symbolica` in `Cargo.toml`. The following are direct source
observations, not claims inferred from a newer online manual:

| Need | Inspected native API/source | Authority limitation |
| --- | --- | --- |
| Exact dense witness solve | Numerica `tensors/matrix.rs`: `Matrix::solve`, `solve_any`, `row_reduce`, `solve_fraction_free` | A solved ansatz is not proof that all possible certificates were searched |
| Sparse/modular discovery | Numerica `tensors/sparse.rs`: `SparseMatrix::solve`, `solve_parallel`, `SparseRowReducer` with pivots/back substitution; native `Z`, `Q`, `Zp64` | A field solution alone says nothing about integer-domain coverage |
| Rational reconstruction | `poly/reconstruction/rational.rs::reconstruct_rational_function_over_q` | Its documentation explicitly labels the unused-prime acceptance test probabilistic, not a coefficient-height proof; require exact original residual afterward |
| Polynomial/factor/restriction arithmetic | Existing `parametric/affine/restriction.rs`, guard factor services, native polynomial arithmetic | Preserve variable maps, coefficient ring, original poles and all admission gates |
| Algebraic fallback | `poly/groebner.rs::GroebnerBasis`, `is_groebner_basis`, `solve`, `solve_parametric` | Public basis object contains a system, not an original-generator multiplier certificate; its basis test is not original-ideal provenance |
| Exact algebraic root handling | `poly/univariate/roots.rs::isolate_roots`, `isolate_real_roots`, `isolate_real_root_intervals` | Certified univariate roots are not a complete multivariate integer solver |
| Domain/solution coverage metadata | `solve/solution_set.rs::SolveCoverage`, `coverage_guard`, `is_empty` | `is_empty` rejects generic coverage or entirely conditional branches; never discard those guards |
| Rational affine normalization | RustRed `solver/case/intersection/native.rs`, `solver/geometry/normalization.rs`, `solver/case/affine/chart.rs` | Native RREF is not Presburger elimination or integer-lattice completeness |

Relevant exact locations in this snapshot include `matrix.rs:1937`,
`sparse.rs:1040,1570`, `reconstruction/rational.rs:46`,
`groebner.rs:134,1241`, `univariate/roots.rs:1223,1254,2126`, and
`solve/solution_set.rs:289,347`. Lines are convenience references, not stable API
identifiers. The general solve API exposes an `Integers` domain and unsupported
outcomes; its name does not override its coverage/conditional-solution contract
or establish decidability of all polynomial integer input.

No ready public proof-producing Presburger, comprehensive integer case-cover,
general CAD certificate checker, or SOS/SDP pipeline was identified in the
inspected native services. This is an audited integration gap, not proof that
every such capability is absent from every Symbolica component. Before adopting
a fallback, verify its actual supported fragment, completeness status, native
error handling, and cancellation behavior upstream. Do not reimplement these
CAS algorithms in RustRed under the label of a proof helper.

Resource admission must precede native allocation: checked monomial counts,
matrix dimensions/nonzeros, coefficient bits, substitution growth, and a shared
cumulative work allowance. A precharge is not interruptibility of a single
native call. Discovery and cold verification need separate measured budgets;
certificate size can dominate verification. Resource settings remain caller
policy, never artifact authority.

## 9. Proposed proof architecture and honest eventual-coverage statement

### Small checker contract

One canonical domain record should own the original index map, sector, bounds,
target equations, fixed coordinates, congruences, whole exclusions, and pole
conditions. Temporary charts must carry exact reconstruction maps and preserve
the original record. Proof IDs and caches must be bound to all those inputs.

A minimal extensible certificate vocabulary is:

- exact source-combination identity and justified denominator use;
- exhaustive Boolean split with inherited domain;
- affine equality consequence, rational inequality/Farkas consequence, integer
  divisibility/rounding/split consequences;
- polynomial multiplier identity with separately proved nonzero target;
- optionally an exact real certificate with an explicitly supported coefficient
  representation;
- a leaf's rule applicability, strict descent, zero-sector proof, or explicit
  finite terminal membership.

Numerical discovery never adds a new axiom. A leaf that has no witness remains
unresolved. A live, fully replayed point violating descent is a counterexample;
an empty application domain is not. An algebraically feasible relaxation is
not automatically either a valid integral counterexample or a missing IBP.

### A conditional completeness theorem worth aiming to prove

For a finite set of admitted candidate rules, suppose:

1. All domain, coverage, terminal, and descent conditions translate exactly into
   a declared Presburger language after verified algebraic simplifications.
2. Every algebraic simplification and source identity has an accepted exact
   certificate, with original denominators retained.
3. The remaining coverage formula is evaluated by a terminating complete
   Presburger procedure whose proof can be checked, without a fixed resource cap.

Then the checker can decide coverage of that candidate rule set, including
detecting holes. On the successful side, if every reachable leaf is a valid
descending rule, proved zero, or a declared finite terminal, and the descent
order is proved well-founded on the admitted integral domain, the certificate
also establishes termination of the reduction. This is
coverage completeness for the stated fragment and candidate set, **not** a
theorem that the generator will find such a set for every topology. A finite
candidate set can genuinely leave uncovered points.

For algebraically empty guard branches, fair increasing-degree exact multiplier
search is refutationally complete in its localized algebraic fragment. For
real-empty branches, an exact complete real procedure is another fallback.
Neither closes the integer-real gap. Operationally, bounded scheduling returns
`inconclusive/resource exhausted`; a theoretical eventual-completeness claim
requires that no fixed degree, support, coefficient-size, or branch cap forever
excludes the necessary proof and that heuristics cannot starve the fallback.

### Practical priority

| Phase | Bounded deliverable | Gate before claiming more |
| --- | --- | --- |
| 1: semantics | Common target-relative domain and singleton handling; existing exact native implication services | FG115/H229 controls, original poles, mixed exclusions, same results in publication/cold load; no completeness claim |
| 2: compact witnesses | Exact multiplier certificate checker, then configurable low-degree native sparse/modular discovery | Wrong-witness rejection, genuine-zero negatives, original-generator identity, cumulative budget tests; separately measure proposal and checking |
| 3: coverage ledger | Persist exact split provenance and independently check all leaves | Every omitted branch has evidence; finite terminals explicit; no generic-only specialization holes |
| 4: complete declared fragment | Written affine-integer grammar plus upstream/native proof-producing complete backend | Congruence/parity/inequality tests and a termination/completeness argument for that grammar; bounded runs still may be inconclusive |
| 5: nonlinear expansion | Only a demonstrated need justifies exact real or comprehensive algebraic fallback | Specify field/domain, exceptional strata, supported certificate coefficients and cancellation/resource behavior before integration |

Current bounded factor/affine guard proofs, source replay, finite terminals,
and cold checks are implemented services. The general multiplier-certificate
language, split ledger, complete Presburger fallback, comprehensive parametric
elimination, and general real-certificate backend described here are proposals.
The existing sample-fit research prototype does not change that status.

Each phase should have an independently reviewable proof contract and an
on/off workload comparison with exact output checks. Measure exact certificate
checking separately from discovery; compare reconstruction on/off at identical
inputs, worker counts, and resource limits. Changing both the certificate
language and the search workload at once obscures the result.

This sequence is a general architecture improvement, not a promise that FG115
and H229 exhaust the remaining problems. Current four-loop durable closure and
full numerical acceptance remain open.

## 10. Adversarial acceptance suite and convergence-draft audit

Required controls include: affine parity obstructions; rational solutions with
no integer point; negative literals with inconsistent augmented equalities;
mixed siblings from different exclusions; target-relative empty pieces;
original `0/0` poles after apparent cancellation; zero branches of every pivot;
positive-dimensional exceptional loci; genuine widened-domain guard zeros;
real-feasible/integer-empty nonlinear systems; unlucky modular primes; a
reconstructed wrong witness rejected by exact residual; degenerate SOS boundary
data; resource exhaustion at each precharge; and charts that introduce new
singleton coordinates. Full infinite coverage must not depend on sample density.

Read-only audit of `certification_convergence.md`: the draft correctly separates
soundness, fragment completeness, and performance; its Hilbert-tenth caveat is
appropriately limited, and its FG115 pole/domain diagnosis is consistent with
the saved bundle. No blocking overclaim was found. The independently suggested
qualification is now explicit in that draft: the affine-integer completeness
goal is the **domain/coverage sublanguage**. Nonlinear generic-parameter
coefficient guards already exist and need their own certified reduction to that
language or a separate backend.
Native API availability and small successful examples are not completeness
theorems. The proposed architecture above deliberately keeps these qualifications
explicit.

Read-only audit of `modular_certificates_and_coverage.md`: no theoretical blocker
was found. The exact modular identity proposal correctly requires a proven
coefficient bound and polynomial-zero tests, not sampled zeros; the intersective
example, randomized protocol qualifications, and conditional split-DAG coverage
statement are correctly scoped. Its implementation/performance hypotheses
remain labeled as proposals, not measured improvements.
