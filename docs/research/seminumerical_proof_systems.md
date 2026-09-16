# Semi-numerical proof systems for parametric-IBP closure

Research and bounded experiments, 2026-09-16. This report combines independent
modular-algebra and geometric-proof literature investigations with native
Symbolica experiments on actual four-loop failures. The recommendations below
are not a newly implemented production proof engine or a closure claim.

## Conclusion

The best near-term approach is **cheap discovery of a small witness, followed
by exact verification of that witness**. Finite fields, point evaluations and
numerical optimization can guide discovery without requiring large symbolic
eliminations. They cannot by themselves establish coverage of infinitely many
integer powers. Exact checking can nevertheless be much smaller than exact
discovery; the two current H/FG failures already admit degree-one multipliers.

For the immediate captures, reuse of already-computed native factors and
affine charts remains the simplest implementation. A bounded, low-degree
certificate discoverer is a promising complementary generalization. Neither
route needs a new CAS implementation inside RustRed.

| Approach | Appropriate use | Exact authority / principal limitation |
| --- | --- | --- |
| Modular or sampled low-degree multiplier discovery | Joint guard coefficients; selected source identities | Verify the full polynomial combination with Symbolica, then prove its target nonzero on the domain |
| Sample-guided affine conflicts and local covers | Affine targets, excluded faces and rectangular integer bounds | Exact equation/bound certificates and a complete domain partition; not a bag of sampled points |
| Complete small-prime congruence obstruction | Necessary polynomial equalities in integer unknowns | Entire residue system must be impossible; integer disequalities cannot silently become modular inverses |
| Native modular Gröbner discovery | Small stubborn guard systems after cheaper paths fail | A reconstructed basis needs original-ideal membership evidence, not just its own basis check |
| Numerical SOS / Positivstellensatz proposals | Nonlinear sign/positivity conditions | Exact rational identities and positivity checks; conditioning, degree and matrix growth are substantial risks |
| Randomized matrix/identity certificates | Fast rejection, diagnostics, very large modular discovery frames | Monte Carlo guarantees do not satisfy the current exact publication contract on their own |

## What is actually blocked today?

RustRed milestone `6feaad1` is pushed. Its release runs pass every sector audit
for H, FG and BMW: respectively 314/21,360, 124/9,272 and 134/9,024
sectors/rules, zero reported gaps/issues. Durable publication subsequently
rejects guards in H58, FG83 and BMW230. X passes 144 complete sector audits
and then rejects preparation of a 33rd distinct predicate under its independent
32-atom limit. No four-loop artifact is written or cold validated.

The observed failures are verification limitations, not demonstrated missing
IBPs. Exact identities below resolve the mathematical questions in the captured
guards. They do not show that every later unchecked cell will pass, that all
remaining X sectors are covered, or that four-loop Vakint numerical acceptance
already works. The detailed release evidence is in the
[four-loop guard report](four_loop_predicate_consistency.md).

## 1. Small ideal-membership certificates: the strongest immediate candidate

NulLA expresses polynomial infeasibility through bounded-degree multipliers;
fixing their support gives a linear-algebra problem. Finite fields and sparse
structure help the search, although degree growth can make the general method
impractical. This motivates a bounded witness search here, not an unrestricted
new elimination engine. [De Loera, Lee, Malkin and Margulies](https://www.math.ucdavis.edu/~deloera/RECENT_WORK/jsc09_issac08.pdf).

Our application-specific variant seeks

```text
T(n) = sum_j U_j(n) C_j(n),
```

where the Cj are the guard's coefficients in the symbolic base parameters,
and T is already known nonzero on the actual integer domain. Simultaneous
vanishing of every Cj would contradict this exact identity. T need not be 1;
using a known nonzero coordinate power can avoid inverse variables and a much
larger unit-ideal calculation. This variant and the examples below are our
derivations from the captured RustRed data, not benchmark claims from NulLA.

### FG83: a complete two-polynomial witness

Set `a=n2`, `b=n7=n8`. The actual domain has `a<=-2`, `b<=-1`.
The guard is `Q=C0+d*C1`, with

```text
C0 = 2*b*(b^2 + (a+3)*b - a^2 - 2*a)
C1 = b*(2*a-3*b)

b^3 = 2*C0 + (2*a+b+4)*C1.
```

Both coefficients cannot vanish: the right-hand side would be zero, whereas
`b^3 != 0`. Symbolica verifies the displayed polynomial identity exactly.
The conclusion holds for the entire captured infinite domain, not only tested
powers. Widening the domain to include b=0 invalidates this conclusion, as it
must: the original guard then genuinely vanishes.

### H58: the same small-certificate shape

Set `a=n0`, `b=n6=n7`, again with `a<=-2`, `b<=-1`. The two highest
coefficients of its quadratic-in-d guard are

```text
C2 = 2*b*(2*a-3*b)
C1 = 24*b^2 + 22*b^3 - 16*a*b - 2*a*b^2 - 8*a^2*b

2*b^3 = 2*C1 + (4*a+7*b+8)*C2.
```

This is another exact contradiction if all guard coefficients vanish. A
witness may use a subset of coefficients, but original input admission and
the complete guard/domain ownership must still be retained. A zero at one
special numerical value of d is not the vanishing of the generic-d guard.

### BMW230: preserve the excluded branches

Its new guard is `(n3-n9-1)` times the previous guard. Under the target
`2*n0=n3+n9+1`, zeros of the extra factor lie in one entire retained exclusion;
joint zeros of the previous coefficient pair lie in another. These are two
complete excluded conjunctions, never one artificial conjunction assembled
from equations chosen from different exclusions.

### Proposed discovery workflow

1. Use the actual target chart to expose the small set of relevant variables.
   Preserve the chart's conditions, integer interpretation and denominator data.
2. Propose T from simple factors already proved nonzero on this domain, and
   start with a small bounded multiplier support. This choice is heuristic.
3. Stream native finite-field evaluations into Symbolica/Numerica sparse linear
   algebra. Try independent primes and points; failed/rank-deficient probes
   do not delete branches.
4. Reconstruct only the winning multipliers through Symbolica's API, or solve
   a compact exact frame if that is cheaper. Do not implement reconstruction,
   CRT or elimination independently.
5. Verify `T-sum(Uj*Cj)` is the zero polynomial exactly, then apply the native
   nonvanishing/domain proof for T. A fit or held-out sample is not this check.
6. On failure or a budget limit, return inconclusive and retain the existing
   exact fallback. Do not turn a missed certificate into a new master.

For k active variables and degree r, a dense multiplier support already has
`binomial(k+r,r)` monomials per coefficient. Use local support, native factor
information and tight degree/row budgets. Nothing in this proposal guarantees
small certificates or efficient six-loop closure for arbitrary inputs.

## 2. Affine conflicts and cover compilation

For X's predicate cover, the current equations are affine; nonlinear guard
polynomials are handled separately. A full nonlinear real solver would be a
large first response to a representation/preparation cap.

The earlier X394 contradiction illustrates the right kind of small proof.
Its true equations include

```text
E0 = -1-n6+n0 = 0
E1 = -4-4*n6-n4+2*n2 = 0.
```

The box has `n0<=-2`, `n4<=0`, `-1<=n2<=0`. Consequently

```text
E1 - 4*E0 = 6 + 4*(-n0-2) + (-n4) + 2*(n2+1) >= 6.
```

The left side must be zero; the right side is strictly positive. This is an
exact affine/bound certificate. A numerical or modular proposal can identify
useful equation combinations, but coefficient signs and domain bounds must
be checked over the ordered exact field, not inferred modulo a prime.

Conflict-driven cylindrical algebraic covering generalizes a failed sample
into a certified region and accumulates regions until the space is covered.
That distinction—sample to find a conflict, exact mathematics to generalize
it—is useful here. Its full nonlinear machinery is not yet warranted for
RustRed's affine predicate problem. [Ábrahám et al.](https://arxiv.org/abs/2003.05633).

Recent work minimizes the sets of reasons needed by such coverings. Its
relevance is to keeping local explanations small; an excluded region can
require several conditions together, so selecting a single convenient
condition is unsound. [Babatunde, England and Sadeghimanesh](https://arxiv.org/abs/2601.14424).

Our inference is to investigate face-local predicate compilation and reusable
small conflicts, with exact domain binding. A caller-owned atom allowance is
the smaller diagnostic step; it does not eliminate separate matrix, Boolean
node, clause or cumulative-work limits. Sampling cannot make a 33-atom input
fit an unchanged 32-atom preparation limit.

## 3. Other approaches and their boundaries

### Modular Gröbner discovery

Modular computations and learned reduction traces can avoid repeating expensive
symbolic discovery. The msolve paper describes this within an F4-based system.
This is useful inspiration, not permission to add msolve or another CAS to
RustRed. Native Symbolica already supplies F4. [Berthomieu, Eder and
Safey El Din](https://arxiv.org/abs/2104.03572).

A lifted set passing its own Gröbner-basis test is not enough: it could generate
the wrong ideal. Exact provenance or both required ideal containments must
connect it to the original coefficients. For our pressure targets, a directly
checked consequence witness is simpler than authenticating a full basis.
Modular arithmetic controls coefficient size, not monomial count or matrix fill.

### Complete congruence obstructions

If original necessary integer-polynomial equalities have no solution modulo
some prime, they have no integer solution. This can be an exact proof, not a
Monte Carlo argument. The premise is **complete modular infeasibility**, not
failure to find a point. Finite-field elimination can certify such statements,
including field equations `x^p-x`. [Gao, Platzer and Clarke](https://arxiv.org/abs/1104.0746).

Crucial counterexample: `x=p` and `x!=0` is feasible over integers. Adding
`u*x=1` modulo p incorrectly deletes it. A nonzero integer may become zero
modulo p, and finite fields have no integer ordering. Likewise, b<=-1 admits
b=-p in the H/FG cases. Their original coefficient equalities also retain
b=0 roots modulo every prime, so this route alone does not solve those guards.
An inverse-variable certificate reconstructed and checked over Q is a different,
potentially sound argument; its modular discovery is not final authority.

### SOS / Positivstellensatz

Numerical semidefinite solutions can sometimes be rounded and projected to
exact rational sum-of-squares certificates. Strict feasibility matters;
singular boundary cases are substantially harder. This offers a possible
future nonlinear-sign fallback, but not the shortest solution to today's
linear-factor captures. [Peyrl and Parrilo](https://www.mit.edu/~parrilo/pubs/files/PeyrlParrilo-ComputingSumOfSquaresDecompositionsWithRationalCoefficients.pdf).

Exact checking would require the full rational identity and a valid positivity
certificate, with complete box/exclusion semantics. Dense Gram matrices and
degree growth can be worse than the original elimination. We found no reason
to implement an SDP solver or independent SOS algebra in RustRed now.

### Randomized matrix and identity verification

Interactive linear-algebra certificates can reduce verification to a few
matrix-vector operations, with Monte Carlo or computational assumptions.
They are attractive for fast rejection and large modular discovery frames,
but they are not deterministic exact artifact proofs under our current
contract. [Dumas and Kaltofen](https://arxiv.org/abs/1401.4567),
[Dumas et al.](https://arxiv.org/abs/1909.05692).

Proved degree and coefficient-height bounds can support deterministic modular
identity checks, but a full interpolation grid or conservative circuit-height
bound may be prohibitively large. A compact circuit is not automatically a
cheap deterministic zero-identity proof. A polynomial-identity probability
bound also does not bound the chance of missing a thin exceptional integer
ray in a guard-coverage search.

## 4. Symbolica services already available

The current vendored public Rust APIs were inspected, not inferred from the
older local Python binding used for diagnostic experiments:

- `poly::reconstruction::reconstruct_rational_function_over_q` already provides
  rational reconstruction. Its documentation explicitly describes held-out
  verification as probabilistic, not a coefficient-height proof. Results are
  suitable proposals for an exact final witness check.
- Native `SparseRowReducer`, matrix solves and finite-field domains support
  the discovery linear algebra; native polynomial evaluation, factorization,
  multiplication and division support discovery and exact verification.
- `poly::groebner::GroebnerBasis::new`, native reduction and basis checking
  provide F4 functionality. A directly exported source-transformation witness
  was not found in the inspected result interface; audit upstream services
  before introducing any local provenance mechanism.
- `AtomCore::solve().over(Integers/Reals).wrt(...)` returns solution branches
  and explicit complete/generic coverage. `SolutionSet::is_empty()` rejects
  unresolved coverage rather than treating an empty-looking generic result
  as proof. Branch conditions still require attention; this is not a general
  certificate that arbitrary box inequalities have been solved.
- Certified native univariate real-root isolation and Numerica ball arithmetic
  are relevant to future geometric fallbacks. Root/interval operations and
  unsupported cases need a fresh task-specific API audit before integration.

No ready-made LP/SMT/CAD/SOS proof service was found in the inspected public
modules, examples and tests. This is a scoped search result, not a reason to
reimplement numerical algebra or a claim about all future Symbolica APIs.

## 5. Experiments completed in this research pass

Native Symbolica sampling of the actual H58/FG83/BMW230 captures used seed
2026091601, 256 random integer points and 32 highest-coefficient-zero points
per family. All 864 admitted integer probes had no simultaneous coefficient
zero. Separate finite-field probes at 101, 103 and 107 admitted 3,220 points,
again with no joint zero; 11 whole-exclusion points were rejected. Primes 2
and 3 were excluded by the chart/leading-coefficient sampling policy.

The finite-field lanes deliberately cannot retain integer inequalities and
are **not** surjective reductions of every admissible integer point. They are
diagnostic probes of chosen strata. Widened b=0 domains and excluded BMW points
produce genuine exact zeros; single-d zeros are also checked as adverse controls.
Those controls distinguish generic-d guard failure from specialization.

Both small H/FG identities above were independently checked against coefficients
extracted from the final release logs. A further automatic sampled experiment
then discovered their multipliers without receiving the winning coefficients:
two multipliers each in the span `{1,a,b}`, 12 seeded integer training points,
native exact-Q matrix solve, and 16 disjoint held-out points per capture.
Both six-unknown systems have rank six, and both resulting full polynomial
residuals are exactly zero. The coefficient pair and target b³/2b³ are supplied
templates: this is not autonomous choice of a certificate target or an artifact
generator. Successful point fits alone would not have been accepted.

This experiment uses exact rational point values, not floating-point arithmetic.
The installed diagnostic extension identifies itself as `symbolica-77c1374`
(distribution metadata 2.2.0), separate from pinned Rust Symbolica 3. Its Python
matrix binding did not accept native finite-field polynomial entries, so no
finite-field matrix-fit experiment is claimed. The pinned Rust sparse/matrix
field interfaces do provide that route. No modular elimination, interpolation
or CRT was reimplemented to work around the old binding.

All experiment scripts and machine-readable results are retained in
`/tmp/rustred-native-refinement-release.YaRtcr/`, including
`sampled-guards-results.json`, `discovered-multipliers-results.json`, and the
independent `SEMINUMERICAL-MODULAR-RESEARCH.md` and geometry research report.
No research prototype changes production code or the artifact trust boundary.

## Recommended next work

1. Finish the small native-factor/affine consequence handoff for current guards.
2. Resolve X's predicate representation/policy limit separately and rerun all
   four full release campaigns; retain precise first failures.
3. Prototype bounded small-witness discovery through native Rust finite fields,
   compact exact lifting/reconstruction and full identity verification. Measure
   total cost against the cheaper factor/affine path, not only modular solve time.
4. For cover scaling, prioritize small exact affine conflicts and face-local
   explanations before a general nonlinear covering engine.
5. Keep SOS/full nonlinear solving and randomized-only verification as distinct
   research directions, not silent replacements for the exact closing contract.

Benchmark search time, witness size, exact verification time, memory, failed
proposals and complete end-to-end artifact time. Claims of four-loop closure
still require a cold-reloadable artifact and FORM-less Vakint numerical parity;
neither is established by these research results.
