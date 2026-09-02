# Certificate-first parametric-IBP closure with blind-domain-guided Janet completion

Status: research recommendation, reviewed 2026-09-02. This document proposes an
experiment and an implementation architecture. It is **not** evidence that the
three-loop `K = 6` family is already closed, and it is not authorization to
start the deferred Stage-2 six-loop campaign.

The companion [Janet/Ore proposal integration seam](janet_ore_integration_seam_2026.md)
records the reviewed authority boundary for connecting these obligations to
RustRed's requested-domain, modular-discovery, and exact-replay pipeline.

## Executive conclusion

The best-supported next closure engine for RustRed is a hybrid of two ideas:

1. use RustRed's exact uncovered-domain geometry, strengthened by the lattice
   feedback loop in the 2026 generating-function method, to decide *where* a
   missing translated consequence is needed; and
2. use Janet-like completion of the linear shift ideal to decide *which*
   translated consequences are mandatory and when the algebraic completion is
   finished.

In one sentence:

> Prioritize Janet nonmultiplicative prolongations by their intersection with
> maximal-dimensional blind domains, batch their modular eliminations, and
> publish closure only after every prolongation reduces to zero and every live
> guard branch has a zero-dimensional leading-monomial ideal.

This is a synthesis, not an algorithm already demonstrated end to end in the
literature. Its attraction is that it separates three questions that the
current lattice walk partially conflates:

- discovery priority comes from the blind geometry;
- algebraic completeness comes from involutive prolongation obligations; and
- finite terminality comes from a zero-dimensional leading-ideal certificate.

Minimality is deliberately not a gate. If the standard complement is finite,
RustRed may publish all of its points as a nonminimal universal terminal basis.
Vakint can attach exact MATAD/FMFT evaluations through four loops, and later
AMFlow values can be attached to a reasonably sized higher-loop basis.

The principal unresolved theoretical risk is guard branching. A generic
operator rule is valid only where its leading coefficient is nonzero. No
primary source located in this review proves that repeated specialization of
index-dependent leading coefficients under shifts always yields a finite
comprehensive guard DAG. RustRed must therefore treat a surviving
positive-dimensional guard branch as a typed incomplete result, never as a
master declaration or sampled proof of closure.

## What the literature establishes

### LiteRed is a successful heuristic, not a closure certificate

LiteRed motivates the target representation: symbolic rules are compact,
reusable, and cheap to apply. It also states explicitly that its rule search is
heuristic, ordering-sensitive, and not proved to terminate. Its required
integral order has finitely many predecessors and is shift-invariant within a
sector. Those are necessary design constraints, but `SolvejSector` does not
supply a general completion certificate. See Lee's
[LiteRed paper](https://arxiv.org/abs/1212.2685) and
[LiteRed 1.4 update](https://arxiv.org/abs/1310.1145).

This means that reproducing the LiteRed2 search chronology more faithfully can
improve K6, but cannot by itself justify the word “closing.”

### Blind-lattice feedback is the right discovery signal

The 2026 generating-function algorithm organizes reduction into equation
generation, symbolic rule extraction, global update, and lattice completeness
checking. When the rule set is incomplete, the remaining irreducible lattice
region guides the next descendants. A rule leader covers an upward orthant;
the intersection of the complementary regions is the unresolved set. See Feng
et al.,
[An Algorithm for the Symbolic Reduction of Multi-loop Feynman Integrals via Generating Functions](https://arxiv.org/abs/2605.09541).

That geometry is extremely close to RustRed's exact `BoxCover` and standard
blind-domain representation. The paper's unknown-master stopping proposal,
however, compares reductions along selected paths. Path agreement is valuable
validation but is not an algebraic proof that every untested critical
composition vanishes. The proposed hybrid keeps the lattice feedback and
replaces the final completeness authority.

### Janet-like completion supplies finite mandatory obligations

For a finite linear difference system under an admissible ranking,
Gerdt--Robertz give a terminating Janet-like algorithm. A set is Janet-like
complete precisely when every prescribed difference power/prolongation has
zero Janet normal form. Their implementation applies the construction to a
simple Feynman recurrence and also records the practical warnings that matter
here: at-least-exponential complexity, rapid coefficient growth with
parameters, and substantial sensitivity to selection strategy. See
[Computation of Difference Gröbner Bases](https://arxiv.org/abs/1206.3463).

The theorem applies to the linear difference ideal over the chosen difference
field and ranking. It does **not** erase the distinction between the generic
coefficient localization and exceptional integer-index loci. RustRed's guard
compiler remains part of the proof.

### Finite master spaces exist, but the theorem is not constructive enough

Smirnov--Petukhov prove finiteness of the number of masters for a fixed graph
family, while Lee--Pomeransky relate the number to critical points of the
Lee--Pomeransky polynomial. These results justify looking for a finite quotient:

- [The number of master integrals is finite](https://arxiv.org/abs/1004.4199)
- [Critical points and number of master integrals](https://arxiv.org/abs/1308.6676)

They do not prove that a chosen order, a chosen coefficient localization, or a
completion restricted to the nine ordinary K6 IBP generators will expose that
finite quotient efficiently.

## Formal closure certificate

Fix one sector and map its physical integer indices to nonnegative local
coordinates:

\[
x_i=n_i-1\quad\text{for active denominator slots},\qquad
x_i=-n_i\quad\text{for inactive/numerator slots}.
\]

Translate each ordinary IBP so its shift support lies in one nonnegative shift
monoid. Let `E_i` raise `x_i`. The `E_i` commute with one another, but act on
coefficient functions by

\[
E_i\,a(x,d)=a(x+e_i,d)\,E_i.
\]

Consequently this is a linear Ore/difference problem, not an ordinary
commutative polynomial ideal in the combined coefficients and shifts.

For one admissible ranking and one guard branch, let `B` be the completed
linear difference basis and let `M` be the monomial ideal generated by its
leading shift exponents. Two independent statements are required:

1. **Involutivity:** every Janet nonmultiplicative prolongation of every member
   of `B` has zero Janet normal form. This certifies that no difference-critical
   obligation remains under the fixed ranking and localization.
2. **Finite complement:** for every free coordinate `i`, there is an `m_i > 0`
   such that `E_i^(m_i)` belongs to `M`.

The second condition is the usual zero-dimensional monomial-ideal criterion.
Equivalently, the standard monomials fit inside a finite box; equivalently, no
standard pair retains a free axis. It is exactly the distinction needed for
the user's “walk upward” clue:

- a pointwise walk that keeps finding new reducible points is not closure;
- an infinite standard pair identifies a whole ray still lacking a leader;
- a pure-power leader for every axis proves that only finitely many points can
  remain.

The finite standard complement may be adopted as explicit terminals after
symmetry, zero-sector, and predecessor ownership have been removed. It need
not coincide with MATAD's preferred basis.

## Guard-comprehensive completion

Suppose an exact candidate has pivot coefficient `q(x,d)`. The generic rule
belongs to the branch `q != 0`. RustRed must then inspect the integer-relevant
exceptional locus `q = 0`:

1. factor `q` with Symbolica;
2. discard factors already certified nonzero on the application domain;
3. specialize each remaining branch exactly;
4. regenerate/reorient the affected difference basis on that branch;
5. recompute involutivity and the pure-power certificate; and
6. merge only branches with identical authenticated owner semantics.

The guard tree closes only if every reachable branch is one of:

- covered by a descending rule;
- routed to an immutable predecessor, exact symmetry image, or zero sector;
- a finite explicit terminal point; or
- proved empty over the integer application domain.

A branch with a free standard-pair axis is an exact obstruction. A depth cap,
prime portfolio, or sample agreement may pause the campaign, but may not
convert that obstruction into a terminal.

## Recommended RustRed architecture

### 1. Preserve the existing proof boundary

The new engine should propose `RuleCell` material only. The existing exact
source provenance, guard authentication, strict-descent compiler,
`CanonicalExactOwnerLedger`, immutable predecessor snapshots, durable codec,
and cold-load replay remain publication authority.

### 2. Add a thin typed shift/Ore layer

Implement only the missing semantics:

- sparse shift exponent vectors;
- coefficient shift action;
- admissible block rankings;
- leading term and Ore multiplication;
- Janet multiplicative/nonmultiplicative variable sets;
- Janet normal form and prolongation queue;
- sparse provenance combinations of the nine ordinary sources; and
- standard-pair or equivalent leading-antichain bookkeeping.

Do not implement polynomial, rational-function, finite-field, GCD,
factorization, or reconstruction engines in RustRed.

### 3. Make blind geometry a priority, not an authority

Intersect each pending Janet obligation's prospective leader region with the
current exact uncovered partition. Order work by a deterministic tuple such as:

1. largest surviving standard-pair dimension;
2. largest exact blind volume when finite;
3. smallest prolongation degree;
4. predicted sparse fill;
5. canonical sector/orbit/coordinate tie breakers.

Tube, boundary, and requested-domain probes can improve this priority. They do
not decide which completion obligations may be skipped.

### 4. Batch modular elimination

Group same-degree prolongations into sparse Macaulay-style batches. Use several
deterministically selected finite fields to discover stable support and rank,
then reconstruct only rows that can become descending rules or new basis
leaders. Exact source combinations must be replayed over the rational
coefficient field before admission.

F4/F5-style batching is an accelerator, not the proof: the proof is the exact
normal-form census plus the leading-ideal terminal certificate.

### 5. Freeze an order only after a bounded tournament

Ordering can change intermediate growth by orders of magnitude. RustRed should
screen a bounded, deterministic set of admissible block orders on shallow
modular completion. Useful metrics are:

- maximum standard-pair dimension after each bucket;
- number and degree of pending Janet obligations;
- leading-antichain size;
- matrix rows, nonzeros, fill, and peak retained bytes;
- support agreement across primes;
- guard-factor count and branch fan-out; and
- worst-sector rather than aggregate progress.

The selected proof run then starts from fresh ordinary sources under one frozen
order. Changing the order midway requires a full restart or explicit
recertification. MCTS or bandit selection is a plausible later scheduler for
the bounded tournament, but must never be part of the mathematical authority.

## Symbolica public-API audit

The current vendored Symbolica `v2.2.0` source was searched before defining the
custom layer.

Available and to be reused:

- public `MultivariatePolynomial` and `RationalPolynomial` types;
- finite fields and rational-to-finite-field conversion;
- polynomial GCD, square-free factorization, and factorization;
- integer Chinese remaindering and rational reconstruction machinery;
- public `GroebnerBasis::new`, whose implementation uses F4 and has a
  specialized finite-field echelonization path; and
- native exact expression rewriting and the matrix/field operations already
  wrapped at RustRed's algebra boundary.

Not found in the public Rust API:

- an Ore or shift-operator algebra;
- Janet/Janet-like division or involutive completion;
- standard-pair decomposition for shift-leading ideals; or
- a comprehensive shifted-coefficient guard-basis engine.

The public Symbolica Gröbner implementation is for commutative polynomial
ideals. It must not be presented with Ore operators as though they commute.
Use it for valid commutative subproblems—guard ideals, leading monomial ideals,
and coefficient algebra—or after a mathematically justified commutative
encoding. Sparse operator-row scheduling and Janet obligations are the thin
RustRed-specific layer.

## Ranked alternatives

| Rank | Candidate | Closure strength | Scaling outlook | Recommended role |
|---:|---|---|---|---|
| 1 | Blind-domain-guided Janet/Ore completion | Terminating linear completion under a fixed admissible ranking; finite complement separately certified | Serious exponential risk, but locality, symmetry and batches can control K6 | Primary K6 experiment |
| 2 | Full rational double-shift Gröbner basis | Strong normal-form authority and finite-standard-monomial test | Existing implementations failed on the two-loop on-shell kite | Reference model and small-family oracle |
| 3 | Rational-Weyl border basis | Finite chosen order ideal plus integrability certificate | Discovering the right order ideal and membership rows may be as hard as completion | Fallback when no useful term order orients desired rules |
| 4 | Tube/syzygy/seedless triangular search | Strong targeted rule discovery; excellent high-rank scouting | No general all-rank finite-complement proof in the reviewed implementations | Priority oracle feeding candidate 1 |
| 5 | Full parametric-annihilator `D`-module then Mellin completion | Can enlarge the authorized relation module; finite-rank theory is strong | Annihilator/Weyl Gröbner work is likely enormous | Research fallback if nine-source completion stalls algebraically |

### Full double-shift Gröbner basis

Barakat et al. formulate IBP reduction in a rational double-shift algebra and
derive reusable normal-form relations. This is mathematically clean, but the
paper reports that existing implementations could not compute the basis for
the on-shell kite, motivating a linear-algebra ansatz. See
[Feynman integral reduction using Gröbner bases](https://arxiv.org/abs/2210.05347).

### Border bases

Border bases need not be tied to one leading-term order. Given a finite order
ideal `O`, one seeks a relation for each border monomial and verifies
integrability of the multiplication/connection matrices. This is attractive
if the useful RustRed rules resist one global order, but it moves the hard
problem to discovering `O` and proving every border relation lies in the IBP
ideal. See Rodriguez--Sattelberger,
[Border Bases in the Rational Weyl Algebra](https://arxiv.org/abs/2510.23411).

### Target-neighborhood and tube scouts

Smith--Zeng explicitly analyze the currently irreducible integrals, fix as many
indices as possible, and solve small systems in neighborhoods of the remaining
targets. This closely formalizes the collaborator's “pick from the blind
sector, walk upward, and rinse and repeat” clue. The method also exposes the
same guard issue: a generic rule can fail at special integer values. See
[Feynman Integral Reduction using Syzygy-Constrained Symbolic Reduction Rules](https://arxiv.org/abs/2507.11140).

Related recent work supplies useful scouts and accelerators:

- [Untangling the IBP Equations](https://arxiv.org/abs/2512.05923) develops
  diagonalized/triangular recurrence systems;
- [Seedless Reduction of Feynman Integrals](https://arxiv.org/abs/2602.22111)
  constructs generic lowering operators from small systems; and
- [Tube Seeding](https://arxiv.org/abs/2606.10698) demonstrates thin-path
  finite-field reductions through rank 20 at two loops.

These results support sparse, target-directed work selection. They do not
replace the involutivity and finite-complement gates.

### Parametric annihilators

Bitoun et al. use the Mellin transform to turn parametric annihilators into
shift relations and show that these contain momentum-space IBPs. This could
provide genuinely new relations if the ordinary-source completion remains
positive-dimensional. The cost and implementation surface are much larger,
and equality of the practically generated ordinary IBP module with the full
annihilator module is not a general implementation theorem. See
[Feynman integral relations from parametric annihilators](https://arxiv.org/abs/1712.09215).

## First K6 experiment

Do not begin with the six-index parent. Begin with the exposed four-line blind
ray represented in physical powers by

```text
[0, 1, 1, 2, N, 0]
```

and include held-out witnesses such as

```text
[0, 1, 1, 2, 4, 0]
[0, 1, 1, 2, 5, 0]
[0, 1, 2, 3, 3, 0]
[0, 1, 3, 2, 3, 0]
```

Install the production terminal/factorization predecessor first so scalar
corner images are removed before Janet scheduling.  This does **not** close the
corresponding dotted and numerator lattices: the 2026-09-02 pre-Janet release
baseline still left the first two sector orbits with 59 and 58 unbounded boxes
after 190 exact owners and 4,096 requested tasks.  Consequently the production
driver must derive its Janet targets from the live exact uncovered partition
and may skip an orbit only when that ledger is actually compiler-closed.  The
four-line ray above remains the first focused diagnostic, while the release
harness covers all six registered sector representatives.

The experiment has four phases:

1. **Self-test.** Withhold one known K3 or known K6 four-line rule and require
   the new completion engine to rediscover it with identical exact provenance,
   guard, and strict-descent semantics.
2. **Order screen.** Run two or three bounded modular order candidates. Select
   by worst-sector leading-ideal progress and fill, then discard every screen
   basis.
3. **Fresh proof run.** Complete mandatory Janet prolongations, prioritizing
   those intersecting the displayed blind ray. Reconstruct and replay exact
   rows from the nine ordinary K6 sources.
4. **Certificate.** Require zero pending prolongations and a pure shift power
   for each free coordinate on every reachable guard branch.

### Orbit-three ingress measurement (2026-09-02)

The first frozen release diagnostic lifted the nine completed ordinary sources
for sector representative `[0,1,1,1,1,0]`.  The head
`[1,1,1,1,1,1]` occurred twice, for source rows `ordinary-ibp:1:2` and
`ordinary-ibp:2:1`; both lifted rows had eleven terms.  This exposed an initial
input-normalization obstruction in the first Janet prototype, not a surviving
standard pair or evidence that a ray had escaped involutive completion.

Deterministic same-head Ore elimination is now implemented before Janet masks
or obligations are built.  It cancels coincident leaders over the exact
Symbolica-backed rational-function field, preserves source-module provenance
and localization guards, and consumes the completion's cumulative work
budget.  On orbit three, exact lift plus preprocessing took 0.002095 seconds;
preprocessing alone took 0.000194 seconds.  All nine input rows were retained
with nine distinct heads after one equal-head elimination produced the lower
head `[0,2,1,1,0,2]`.

The preprocessing census was: one nonzero remainder, no zero remainder, no
cascading collision, maximum collision-chain length one, maximum head-class
size two, 25 sort comparisons over 137,050 bounded payload visits, 23 pivot-head
comparisons over 138 coordinates, and 26 pivot insertion moves.  Its shared
work ledger recorded one normal-form step, one divisor visit, and 30 exact
coefficient operations.  Equal-head ingress is therefore no longer the K6
blocker.  The remaining question is whether full completion can exhaust the
Janet queue within practical coefficient and divisor-search resources; the
companion release study records those measurements without treating a bounded
stop as complement evidence.

Record at least:

- completed and pending prolongation counts by degree;
- leading-antichain size and maximum standard-pair dimension;
- modular rows/nonzeros/fill and support agreement by prime;
- exact reconstruction attempts and failures;
- coefficient term/byte peaks;
- guard factors, integer-relevant roots, and branch count;
- exact blind-box count after every admitted owner; and
- deterministic transcript/hash across supported worker counts.

Success means a cold-reloadable exact artifact or an exact finite-terminal
sector layer. Failure must be typed as one of resource exhaustion,
positive-dimensional leading complement, unresolved guard branch, inconsistent
modular support, or exact replay/descent rejection.

## Scaling toward the eventual six-loop family

For a vacuum family at loop count `L`, the complete scalar-product basis has
`K = L(L+1)/2` coordinates and the ordinary momentum-space IBP source count is
`L^2`. Thus the eventual six-loop pressure target has `K = 21` and 36 ordinary
sources. This does not imply that completion complexity scales like 36; the
number and degree of prolongations, guard branches, and standard pairs are the
real risk.

The route most likely to survive is:

- exact graph/symmetry canonicalization before algebra;
- immutable lower-sector and factorized-product predecessors;
- sector-local completion rather than one global signed lattice;
- worst-sector order selection;
- sparse degree-bucket modular batches;
- support discovery before exact reconstruction;
- shared read-only source/basis data with compact worker-local rows;
- deterministic merge and exact replay on the coordinator; and
- willingness to stop at a finite nonminimal terminal basis.

The four-, five-, and six-loop structural studies should track extrapolation
metrics without claiming closure. Stage 2 should not begin until the K6 pilot
demonstrates that the guard DAG stays finite and the standard-pair dimension
falls monotonically under mandatory prolongations.

## Negative evidence and open questions

1. LiteRed and s-bases are not proved to terminate in all cases. See also
   Smirnov,
   [An Algorithm to Construct Gröbner Bases for Solving Integration by Parts Relations](https://arxiv.org/abs/hep-ph/0602078).
2. A finite number of masters does not imply that a particular localized
   nine-source basis has a finite standard complement under every order.
3. Full double-shift Gröbner computation is presently too risky as the primary
   engine; the published on-shell-kite failure is a strong warning.
4. Janet termination over the generic difference field does not prove that
   index-dependent exceptional branches form a finite comprehensive system.
5. Path consistency and finite sampling can falsify a candidate rule set but
   cannot certify that all critical compositions vanish.
6. F4/F5, finite fields, reconstruction, tube seeding, MCTS ordering, and
   multicore execution improve cost only; none is a closure theorem.
7. The large SPIDER calculation mentioned in
   [arXiv:2604.25916](https://arxiv.org/abs/2604.25916) is evidence for large
   dependency navigation and back-substitution, not a published generic proof
   of all-rank parametric-rule construction.

The decisive research question for RustRed is now precise:

> Can the ordinary K6 shift module be made Janet-involutive under a practical
> order while every coefficient-specialized branch acquires a
> zero-dimensional leading ideal before guard or coefficient growth becomes
> prohibitive?

That question is experimentally answerable and produces exact intermediate
certificates even when the final answer is “not yet.”
