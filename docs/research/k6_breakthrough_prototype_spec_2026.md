# Executable K6 closure prototype: finite frames, Janet tubes, and block Krylov

## Scope and decision

This note turns the existing closure research into one bounded experiment on RustRed's current
three-loop, unit-mass K4 family. It compares:

- **A:** direct physical-stratum modular finite-frame/Macaulay border completion;
- **B:** targeted Janet/standard-pair tubes; and
- **C:** block-Krylov recovery of a finite quotient's shift actions.

The experiment optimizes for an exact, universal, finite closure with an affordable terminal set.
Master minimality is not an objective. A redundant terminal set is acceptable only when:

1. every all-rank sector and guard stratum has an exact owner;
2. the complement is proved finite rather than sampled to look finite;
3. all terminals are typed integral keys and are independent of a caller's routing;
4. accumulated epsilon-pole debt has a rank-independent bound; and
5. evaluating and shipping all required terminal Laurent values remains realistic.

Stable modular ranks, a bounded census, or a short terminal list are discovery evidence. They are
not closure certificates.

### Recommendation

Implement **A0, a degree-one physical-stratum modular border probe on the canonical four-line
sector**, first. Its physical matrix has exactly 63 rows, 157 columns, and 630 nonzero source
entries. It is the smallest *new* prototype that:

- exercises the missing checked modular matrix plan and finite-field sampling layer;
- reuses one physical matrix while issuing a separate forbidden-set rank query for every admitted
  border target;
- returns exact RuleCell and decorated-stratum complement deltas for candidate B when a relation
  lifts successfully;
- provides the sparse operator and provenance layout needed by candidate C; and
- can lift selected rows through RustRed's existing exact Symbolica reducer and replay verifier.

Candidate B is the immediate control and fallback. RustRed already contains most of its mechanics,
so the experiment should run the concrete one-ray tube below as soon as the common report type
exists. Candidate C must not be implemented first: no finite quotient has yet been certified, and
the exact K6 matrices are too small for Krylov to beat direct sparse elimination plausibly.

This recommendation is about prototype order. Candidate A is not promoted unless it later proves
the full border, guard, descent, and finite-complement obligations. A quotient/action certificate
also needs a complete terminal relation module. A direct descending-rewrite certificate may retain
a finite nonminimal terminal set without constructing that complete module.

## Published inputs and RustRed inferences

The following primary results motivate components of the design.

- Lee's sector-basis construction and LiteRed algorithms organize symbolic rules by sectors,
  symmetries, and lower-sector feedback
  ([arXiv:0804.3008](https://arxiv.org/abs/0804.3008),
  [arXiv:1212.2685](https://arxiv.org/abs/1212.2685),
  [arXiv:1310.1145](https://arxiv.org/abs/1310.1145)).
- Macaulay multiplication can recover first-border/Pfaffian relations in a known
  zero-dimensional differential quotient
  ([arXiv:2204.12983](https://arxiv.org/abs/2204.12983)).
- Stable-span border-basis algorithms certify a finite quotient once a valid order ideal and
  its border relations are supplied
  ([Kehrein--Kreuzer](https://doi.org/10.1016/j.jpaa.2005.07.006)).
- Janet completion makes nonmultiplicative prolongations explicit for its stated difference
  systems
  ([arXiv:1206.3463](https://arxiv.org/abs/1206.3463)).
- Standard pairs finitely describe complements of monomial ideals
  ([arXiv:2005.10968](https://arxiv.org/abs/2005.10968)).
- Wiedemann and block Wiedemann replace filled elimination by repeated sparse products, subject
  to probabilistic projection and reconstruction
  ([Wiedemann](https://doi.org/10.1109/TIT.1986.1057137),
  [Coppersmith](https://doi.org/10.2307/2153413)).
- Block-Krylov sparse FGLM methods recover structure only after a zero-dimensional quotient is
  present
  ([arXiv:1712.04177](https://arxiv.org/abs/1712.04177)).
- FiniteFlow demonstrates modular sampling, reconstruction, and dataflow techniques, but not an
  all-rank IBP closure proof
  ([arXiv:1905.08019](https://arxiv.org/abs/1905.08019)).

Everything below labelled as an experiment rule, resource budget, matrix layout, or acceptance
condition is a **RustRed proposal**. None of the cited papers proves that degree three closes K6,
let alone that the same bound scales to K21.

## Frozen K6 experiment input

### Family and ownership state

The experiment consumes the existing exact objects, not a second description of the topology:

- the equal-mass K4 family in
  crates/rustred-core/src/foundry/artifact/three_loop/family.rs;
- its nine ordinary momentum-space IBP rows;
- the exact order-24 S4 action and the six full-rank sector orbits in
  crates/rustred-core/src/foundry/artifact/three_loop/manifest.rs;
- the immutable zero and factorization owners;
- the 46 current RuleCells; and
- the current exact guard-blind carrier complements.

The present census is deliberately finite. It has 115 submitted roots, 44 canonical roots,
89 discovered nodes, 53 rule applications, 27 zero/factorization terminals, and nine uncovered
nodes. The uncovered canonical witnesses are:

    (0,-1,1,2,2,1)
    (0,-2,2,2,1,1)
    (0,1,1,1,1,0)
    (0,1,1,2,4,0)
    (0,1,1,2,5,0)
    (0,1,2,3,3,0)
    (0,1,3,2,3,0)
    (0,1,1,1,1,1)
    (1,1,1,1,1,1)

They occupy four representative sector charts:

| label | sector corner | current role |
| --- | --- | --- |
| S6 | (1,1,1,1,1,1) | top corner |
| S5 | (0,1,1,1,1,1) | five-line corner |
| S4a | (0,1,1,1,1,0) | four-line dot complement |
| S4b | (0,0,1,1,1,1) | factorized scalar face with numerator coupling |

For S4a, 19 structural cover boxes leave 32 guard-blind boxes after 114 splits. For S4b,
seven cover boxes leave 20 guard-blind boxes after 42 splits. Both complements contain boxes of
varying dimension six. Their finite i64 carrier cardinality is tautological; it is not evidence
that the mathematical all-rank complement is finite.

The 46 cells carry 205 guard occurrences and 126 distinct guard polynomials. Of the occurrences,
119 contain an explicitly nonzero constant equation. The remaining 86 residual systems currently
depend on one index coordinate and contain one to three equations; their enumerated integer roots
lie outside the corresponding installed application domains. This finite fact simplifies the
first K6 experiment, but every new elimination pivot can introduce a new multivariate guard.

The other two full-rank sector orbits must enter the production closure pass even though this
bounded census does not expose one of the nine witnesses in them. The five Vakint graph classes
also do not replace the sixth RustRed sector obligation.

### Exact ordinary-source support

A read-only enumeration of the sealed K6 source rows gives term counts

    (8, 11, 11, 11, 8, 11, 11, 11, 8),

so one complete translation layer has 9 rows and exactly 90 nonzero symbolic term entries. The
union of their relative integral shifts has 31 elements. Translation preserves the term count.

This agrees with the resource accounting in
crates/rustred-core/src/identity/generator/translated_source/construction.rs, which computes
translated rows and term entries before constructing them.

### Sector-chart frame

For a sector S, define a sign vector sigma by:

    sigma_i = +1 for an active denominator,
    sigma_i = -1 for an inactive numerator coordinate.

The degree-D one-sided chart frame is:

    M_D(S) = { sigma .* t : t in N^6 and sum(t_i) <= D }.

Its offset counts at degrees one through three are 7, 28, and 84. Translating every ordinary
source therefore produces 63, 252, and 756 rows, with 630, 2,520, and 7,560 physical entries.

The following column counts are exact unions of translated integral shifts before residual
projection, symmetry transport, lower-sector discharge, or coefficient specialization:

| sector | degree | rows R | physical columns C | entries Z | C + provenance R | Z + R |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| S6 | 1 | 63 | 136 | 630 | 199 | 693 |
| S6 | 2 | 252 | 396 | 2,520 | 648 | 2,772 |
| S6 | 3 | 756 | 936 | 7,560 | 1,692 | 8,316 |
| S5 | 1 | 63 | 153 | 630 | 216 | 693 |
| S5 | 2 | 252 | 464 | 2,520 | 716 | 2,772 |
| S5 | 3 | 756 | 1,115 | 7,560 | 1,871 | 8,316 |
| S4a | 1 | 63 | 157 | 630 | 220 | 693 |
| S4a | 2 | 252 | 488 | 2,520 | 740 | 2,772 |
| S4a | 3 | 756 | 1,191 | 7,560 | 1,947 | 8,316 |
| S4b | 1 | 63 | 161 | 630 | 224 | 693 |
| S4b | 2 | 252 | 500 | 2,520 | 752 | 2,772 |
| S4b | 3 | 756 | 1,215 | 7,560 | 1,971 | 8,316 |

The provenance columns form an identity augmentation. They let an exact elimination result retain
the combination of original translated source rows. They are never physical matrix columns and
must not enter a forbidden, target, or allowed partition or any modular rank query. A0 discovery
therefore uses the 63 by 157 physical matrix, while exact circuit recovery may use the 63-column
identity augmentation. These counts are structural envelopes, not claims about modular rank or
fill.

## Common experiment data model

All candidates must consume one immutable matrix plan and emit the same evidence envelope.
Proposed foundry-private records are:

- FamilyStratumId: family fingerprint, sector mask, fixed/free coordinates, exact application box,
  and guard atoms;
- TranslationMonomial: chart-oriented integral offset and total degree;
- SourceInstanceId: ordinary RowId plus translation;
- ColumnKey: raw decorated-sector integral shift, without symmetry quotienting;
- ColumnRole: forbidden, requested border, admitted terminal, or immutable descendant;
- SparsePattern: checked CSR structure with sorted columns, combined duplicates, and no zeros;
- PrimeSample: finite-field prime, values of d and free indices, and rejected denominators;
- PivotFingerprint: rank, independent source rows, pivots, and fill;
- ExactSourceCertificate: source multipliers and exact regenerated-row replay;
- CompletionDelta: new leaders, standard pairs, guards, and remaining obligations;
- A0TargetStatus: ExactRelation, ExactNoRelationInDeclaredFrame, or ModularNoHit;
- TerminalBudget: raw terminals, S4 orbits, quotient rank, and evaluation metadata; and
- PoleDebtCertificate: local epsilon valuations and the global all-rank bound.

The sparse pattern is shared read-only. Prime workers construct only modular values and their own
bounded reducer state. A worker streams fingerprints or compact selected rows back to the
coordinator. It never sends a whole filled matrix to another worker.

### Deterministic row and column order

Rows are ordered by:

1. total translation degree;
2. lexicographic chart translation; and
3. ordinary RowId chronology.

Columns are first assigned by exact integral shift. The same physical matrix is reused, but it is
partitioned and queried separately for each requested target b:

    [ F_b | b | A_b ].

F_b contains every column forbidden by the declared strict reduction order. A_b contains only
strictly lower same-sector terms or immutable lower-sector, zero, and factorization descendants.
Other unresolved border columns stay forbidden.

For a sampled row matrix E, a candidate relation for b exists over that finite field exactly when:

    rank([E_Fb | E_b]) > rank(E_Fb).

The forbidden set depends on b, so one global RREF is not automatically a simultaneous answer for
all targets. An implementation may use a deterministic nested-column sweep only after proving it
is equivalent to every target-specific query.

The rank test is only support discovery. Exact source multipliers and all coefficient guards must
still be reconstructed. A modular rank equality is not an obstruction. It remains ModularNoHit
unless exact elimination proves rank equality over Q(d,x) in the declared finite frame. Even that
ExactNoRelationInDeclaredFrame status says nothing about higher translation degrees or closure.

A0 performs no S4 quotient of rows or columns. Only the stabilizer of the complete decorated
stratum could act within one matrix problem, and exploiting it would be a separate experiment
requiring an exact span and permutation-similarity proof. S4 may transport an already certified
RuleCell only together with its application box, fixed/free restrictions, guards, coefficients,
RHS, and source provenance. It may not erase independent rows or merge physical columns during
discovery.

## Candidate A: physical-stratum finite-frame border completion

### A1. Matrix semantics

For sector S and degree D, construct:

    E_(S,D)(d,x) =
      rows:    all nine ordinary sources translated by M_D(S),
      columns: the union of resulting integral shifts,
      entries: exact coefficients in Q(d,x_1,...,x_6).

Here x_i is n_i - 1 on active lines and -n_i on inactive lines. Equal unit mass is built into the
family before elimination. The experiment never derives a generic-mass system and then naively
restricts it to the potentially singular equal-mass locus.

Translation evaluates the coefficient at the shifted indices before registering shifted integral
columns. This extensional construction respects the Ore relation between number operators and
shifts without adding a new noncommutative CAS.

Negative chart shifts produced by an ordinary source are not silently converted to Laurent
monomials in a commutative ideal. They are classified by the exact sector boundary:

- a same-sector lower shift is an allowed descendant only when the ordering proves it;
- a pinched shift goes through an immutable child owner; and
- a positive inactive-line shift is an explicit refinement boundary.

### A2. Border schedule

At each degree:

1. partition the obligation queue by complete FamilyStratumId;
2. map only the RuleCell leaders valid on that decorated stratum into its formal sector chart;
3. construct a separate minimal leading antichain and exact complement for that stratum;
4. enumerate its non-owned first border and the minimal faces of that exact complement;
5. run a target-specific modular forbidden-set rank query for every border target;
6. select deterministic independent source rows and one compact target circuit;
7. lift only those rows over the exact indexed field;
8. replay the exact relation and build its guard strata;
9. add accepted leaders only to their proved strata, recompute those complements, and queue every
   boundary, transverse, and exceptional stratum; and
10. stop only at a fixed point of the exact decorated-stratum obligation queue.

A leader valid only on a fixed face or bounded application box never generates an ideal over the
whole sector chart. For example, a leader in direction x_1 valid only at x_2 = 0 owns that face;
it does not own the unbounded x_2 direction. Guard-blind carrier boxes are candidate structural
regions, not mathematical ownership outside their proved RuleCell domains.

The formal complement must support mathematical unbounded endpoints. The current i64 carrier is a
useful arithmetic boundary test, but touching i64::MAX is never interpreted as infinity.

Degree one on S4a is A0. It processes the exact 63 by 157 physical matrix. If a target produces
ModularNoHit or ExactNoRelationInDeclaredFrame, proceed to degree two and then degree three under
the registered budgets. Neither status is a closure obstruction. Degree four is a new experiment
and is not an automatic retry.

### A3. Modular discovery and exact lift

Use a deterministic schedule of:

- three discovery primes and four nonsingular parameter/index samples per prime;
- two held-out primes and two held-out samples per prime; and
- exact Symbolica replay for every promoted relation.

Different pivots across nonsingular samples indicate a missing guard stratum. They are not resolved
by majority vote. A denominator-zero sample is rejected with its exact guard provenance.

At K6, do not reconstruct a full modular RREF. Use modular elimination to select independent
original rows and pivot columns for each target-specific forbidden set, then feed the selected
minor to the existing exact indexed reducer. Row identities remain sidecar metadata during modular
discovery; provenance identity columns are appended only in exact circuit recovery. If exact
elimination disagrees with the modular support, quarantine the prime/sample and split the guard
domain as needed.

Only if the selected exact minor becomes too large should the experiment use CRT and rational
reconstruction for individual source multipliers. Fresh-prime agreement remains screening; exact
ordinary-source replay is the proof.

Every target query returns exactly one of:

- ExactRelation: exact source multipliers cancel every forbidden column, have nonzero target
  coefficient, and replay over Q(d,x);
- ExactNoRelationInDeclaredFrame: exact symbolic elimination proves rank equality for that target,
  stratum, and translation frame; or
- ModularNoHit: the sampled ranks do not expose a relation, with no exact negative claim.

A nonzero exact full-row-rank forbidden minor is one valid
ExactNoRelationInDeclaredFrame certificate. Otherwise exact column membership or an equivalent
symbolic rank certificate is required. Held-out finite-field agreement alone never promotes a
negative result.

### A4. Output and completion criterion

One accepted border relation emits:

- its target and strictly lower typed integral-key RHS;
- the exact source-row combination;
- every pivot and denominator guard;
- a strict-descent witness;
- its maximal valid RuleCell domain;
- exact concrete replay at independent boundary points; and
- the resulting decorated-stratum leader/standard-pair or exact-box-cover delta.

A descending-rewrite artifact is complete only when, on every decorated sector, fixed/free,
application-box, and guard stratum:

1. every first-border element reduces exactly;
2. every nonmultiplicative/overlap obligation has a consistent normal form;
3. the structural standard-pair complement has no free coordinate;
4. every finite survivor is declared as a typed terminal or immutable child;
5. all reductions are strictly descending;
6. every accepted relation replays from the nine regenerated ordinary sources;
7. the finite terminal set is routing-independent, evaluable within budget, and has bounded
   epsilon-pole debt; and
8. deterministic precedence is installed and actual overlaps have identical exact normal forms.

The descending-rewrite certificate may keep every finite typed survivor as a nonminimal terminal.
It needs only the exact terminal relations required to reconcile actual overlaps; it does not need
the complete terminal relation module or multiplication actions.

As an alternative, a quotient/action certificate on a redundant frame O must additionally prove
its complete exact relation module R, show that every shift action preserves R, and show that all
actions commute modulo R. Those are candidate-specific obligations for quotient-based A and C,
not universal gates for a direct rewrite artifact.

A finite Macaulay degree by itself proves neither certificate path.

## Candidate B: targeted Janet and standard-pair tubes

### B1. Queue semantics

Candidate B consumes the exact standard-pair or uncovered-face queue produced by the common
geometry. For one pair (u,F), where u is its finite base and F its free coordinate set:

1. Janet division marks multiplicative and nonmultiplicative directions;
2. a deterministic path follows the free directions from u;
3. a bounded transverse halo supplies source rows;
4. the same forbidden/target/allowed rank test discovers a candidate recurrence;
5. selected support is lifted symbolically in the free variables;
6. every coefficient-zero branch and boundary face returns to the queue; and
7. an accepted all-rank RuleCell removes an exact region from the complement.

The tube is a row-selection policy, not a certificate. Completion comes from exhausting the exact
Janet prolongation and standard-pair queue.

### B2. First concrete tube

The first target is the current uncovered ray:

    T_N = I(0,1,1,2,N,0),  N >= 4.

It contains the independent census witnesses N = 4 and N = 5. Positions 0, 1, 2, 3, and 5 are
fixed; position 4 is symbolic. Let e_4 be its active chart direction.

The axial tube of length D is:

    T_D^0 = { q e_4 : 0 <= q <= D }.

The width-one Janet halo adds one inward chart step in each of the five transverse directions at
every q:

    T_D^1 = T_D^0 union
            { q e_4 + sigma_j e_j : 0 <= q <= D, j != 4 }.

Before residual projection, their exact structural envelopes are:

| tube | D | offsets | rows | columns | entries | C + R | Z + R |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| axial | 1 | 2 | 18 | 57 | 180 | 75 | 198 |
| axial | 2 | 3 | 27 | 83 | 270 | 110 | 297 |
| axial | 3 | 4 | 36 | 109 | 360 | 145 | 396 |
| halo one | 1 | 12 | 108 | 246 | 1,080 | 354 | 1,188 |
| halo one | 2 | 18 | 162 | 350 | 1,620 | 512 | 1,782 |
| halo one | 3 | 24 | 216 | 454 | 2,160 | 670 | 2,376 |

Residual projection should lower these counts and leave N symbolic. The existing
SourceViewBatch residual projection and exact target reducer are reused; no bespoke recurrence
solver is introduced.

Run axial D = 1, 2, 3, followed by width-one D = 1, 2, 3. If all six fail, report
ExactNoRelationInDeclaredFrame only for tubes that received exact symbolic rank certificates;
otherwise report ModularNoHit. Return those bounded-frame results to candidate A, but do not call
them closure obstructions or widen an ad hoc tube in the same registered experiment.

The symbolic rule, not the samples, must own every N in its guarded domain. N values
4, 5, 8, 16, and 32 are held-out regressions. Boundary N = 3 must either agree with an existing
owner or receive deterministic precedence and an identical exact normal form.

### B3. Promotion and kill condition

Promote B only if:

- the rule replays into ordinary sources with its selected translations;
- its target coefficient is nonzero on a completely stratified domain;
- every RHS is uniformly lower for all N in that domain;
- all transverse faces are owned or queued; and
- the exact formal complement loses the intended positive-dimensional pair.

Kill fixed-width tubes as a K21 architecture if required width grows with tested rank, if the
number of queued tubes grows faster than the number of standard pairs removed, or if source
support approaches the full degree-three finite-frame matrix without gaining a larger exact
coverage region.

## Candidate C: block-Krylov action recovery

### C1. What C may do

Candidate C is an arithmetic accelerator after A or B has proposed a finite typed frame O and its
exact relation module R. It may:

- estimate modular rank;
- recover selected nullspace or border-membership witnesses;
- recover a generic shift-action minimal generator; and
- avoid storing a filled sparse U when direct elimination demonstrably fills.

It may not infer that O is universal, decide guard strata, or replace ordinary-source replay.

Do not form A-transpose times A over a finite field. For example, over F5 the nonzero row
(1,2) has rank one but its product with its transpose is zero. If a square diagnostic operator is
needed, use the bipartite embedding:

    H = [ 0   E  ]
        [ E^T 0  ].

Its kernel separates the left and right kernels of E without the isotropic rank loss of
E-transpose times E.

### C2. Exact K6 diagnostic envelope

For the four unresolved sector matrices as one block diagonal H, the exact dimensions are:

| degree | H dimension N | H entries | CSR floor at 12 bytes/entry | row-pointer floor |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 859 | 5,040 | 60,480 bytes | 6,880 bytes |
| 2 | 2,856 | 20,160 | 241,920 bytes | 22,856 bytes |
| 3 | 7,481 | 60,480 | 725,760 bytes | 59,856 bytes |

These are diagnostic embeddings, not quotient action matrices. Once a terminal presentation is
known, let:

    t = number of typed terminal keys,
    q = exact rank of terminal relations,
    r = t - q.

Each recovered shift action must be an r by r map on span(O)/R, but its stored columns must retain
representatives in typed integral keys. Dense Krylov vectors are not acceptable production
terminals.

With block size b = 8 and a conservative sequence length

    L = ceil(2N/b) + 16,

the block-diagonal diagnostic estimates are:

| degree | L | H-by-block multiply-adds | block-sequence bytes | live vector bytes |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 231 | 9,313,920 | 118,272 | 54,976 |
| 2 | 730 | 117,734,400 | 373,760 | 182,784 |
| 3 | 1,887 | 913,006,080 | 966,144 | 478,784 |

The counts ignore block-minimal-generator work and multi-prime repetition. They show why C is not
the first K6 implementation: the matrices fit direct sparse elimination, while Krylov replaces
small fill risk by almost a billion degree-three field operations.

### C3. Certificate obligations

A recovered action is promoted only after:

1. every action column has exact original-source multipliers;
2. those multipliers replay over Q(d,x);
3. the action preserves R;
4. all pairwise shift commutators vanish modulo R;
5. the original nine-source module annihilates the quotient;
6. all guard strata are separately owned; and
7. the action induces the same strictly descending normal form as the artifact.

Projected minimal polynomials, stable modular ranks, and matching sampled sequences are not
certificates.

## Symbolica boundary

### Existing primitives to use

RustRed already stores exact coefficients as Symbolica
RationalPolynomial<IntegerRing,u16> in
crates/rustred-core/src/algebra/coefficient/model.rs. The public Symbolica surface supplies:

- sparse multivariate polynomials:
  vendor/symbolica/src/poly/polynomial.rs, MultivariatePolynomial;
- exact rational polynomial fields:
  vendor/symbolica/src/domains/rational_polynomial.rs, RationalPolynomialField;
- 32- and 64-bit finite fields:
  vendor/symbolica/lib/numerica/src/domains/finite_field.rs, Zp and Zp64;
- public CSR matrices and incremental sparse row reduction:
  vendor/symbolica/lib/numerica/src/tensors/sparse.rs, SparseMatrix and SparseRowReducer;
- dense solve_any for selected small exact minors:
  vendor/symbolica/lib/numerica/src/tensors/matrix.rs;
- integer CRT:
  vendor/symbolica/lib/numerica/src/domains/integer.rs;
- scalar rational reconstruction:
  vendor/symbolica/lib/numerica/src/domains/rational.rs; and
- one-variable Newton interpolation:
  vendor/symbolica/src/poly/gcd.rs.

RustRed's exact parametric reducer already constructs an identity-augmented
SparseRowReducer with LuLMode::Full in
crates/rustred-core/src/foundry/parametric/sparse.rs. Its exact replay and RuleCell admission
should remain the promotion boundary.

### Infrastructure RustRed must own

No public Symbolica facility found in the vendored version supplies:

- a sector-aware Macaulay/border-frame builder;
- free-module, Ore, Janet, or standard-pair completion;
- checked shared-pattern multi-prime matrix assembly;
- pivot consensus across parameter samples;
- adaptive multivariate rational-function reconstruction;
- original-source certificate recovery from a modular kernel;
- block Wiedemann, block minimal generators, or Scalar-FGLM;
- guard-stratum completion; or
- the terminal and epsilon-pole proof.

RustRed must own those semantics. It should not wrap Symbolica polynomial or matrix primitives more
deeply than needed for checked dimensions, provenance, deterministic ordering, and artifact
serialization.

The public CSR constructors do not remove all malformed-input risk. The matrix planner must sort
columns, combine duplicate entries, drop exact zeros, check every u32 conversion, and enforce
row-pointer and allocation limits before calling Symbolica.

## Resource and kill budgets

These are experiment limits on the recorded reference machine, not performance claims. A retry
with larger limits is a new registered experiment.

| phase | scope | wall-time limit | process RSS | arithmetic kill |
| --- | --- | ---: | ---: | --- |
| A0 | S4a, degree 1, one sample | 30 s | 256 MiB | fill over 20 Z |
| A2 | one sector, degree 2, one sample | 120 s | 512 MiB | fill over 30 Z |
| A3 | one sector, degree 3, one sample | 600 s | 2 GiB | fill over 50 Z |
| A K6 | all discovery and held-out samples | 45 min | 4 GiB | unstable support |
| exact lift | one accepted relation | 120 s | 1 GiB | over 256 source rows |
| B tube | all six registered tube plans | 30 min | 2 GiB | no exact region removed |
| C control | four-sector degree-three block | 15 min | 2 GiB | slower than 2x direct |

For A, fill means nonzeros retained in modular L plus U divided by physical input Z. A support that
changes at held-out nonsingular primes is rejected. A candidate whose largest exact source
certificate uses more than 256 of the 756 degree-three rows fails the K21 scaling gate even if it
can remain useful as a K6-only discovery result.

C is not launched unless direct degree-three elimination exceeds tenfold fill or 50 percent of its
RSS budget. It is killed if provenance recovery is denser than direct elimination or if it fails
to recover every selected border witness found by direct elimination.

## Universal terminal and AMFlow budget

### Measurements

Every completion iteration records:

- T_raw: all finite typed terminal keys before symmetry;
- T_orbit: terminals after exact S4 transport;
- r: exact quotient rank when terminal relations are known;
- T_orbit/r: optional redundancy diagnostic when r is known, reported but not minimized;
- sector and guard-stratum counts;
- maximal terminal dot rank and numerator rank;
- number and support of exact terminal relations;
- required terminal Laurent depth;
- estimated serialized table size; and
- measured offline evaluation throughput and peak memory.

A bounded-frame unpivoted column is not a terminal. A terminal is admitted only after every
positive-dimensional standard pair or unbounded exact-box-cover component has disappeared on
every decorated application-box and guard stratum.

### Provisional K6 gate

The first registered K6 experiment permits:

- at most 4,096 raw terminals;
- at most 512 S4-orbit terminals;
- a serialized numerical terminal table no larger than 256 MiB; and
- a one-off offline evaluation projection no longer than seven node-days.

These bounds deliberately tolerate a very nonminimal basis. They are evaluation and distribution
limits, not master-count expectations. If the exact complement is finite but exceeds one of them,
closure is mathematically successful and operationally rejected for the Vakint artifact.

For three loops, MATAD/Vakint may be used offline to generate approximately 20,000-digit Laurent
values for RustRed's typed terminals. An exact map to the conventional MATAD basis is optional.
When bases differ, numerical Laurent parity can be the cross-backend parity gate. Exact RustRed
source replay, complement completeness, descent, and guard ownership remain mandatory.

No FORM-derived recurrence or reduction equation enters RustRed. Offline numerical terminal
tables may enter Vakint as immutable data. AMFlow is the fallback when a typed terminal batch is
not covered conveniently by the existing three-loop evaluator.

### Epsilon-pole debt

After substituting d = 4 - 2 epsilon, attach:

    debt(c) = max(0, -v_epsilon(c))

to every exact rule coefficient. A generic valuation is insufficient when its leading coefficient
can vanish at an integer-index guard.

Collapse recurrent RuleCells into a finite stratum graph. A repeatable edge with positive debt
creates unbounded all-rank debt unless an exact potential bounds the number of traversals. Such a
cycle rejects the orientation.

For an acyclic condensed graph, compute the maximum weighted path B_epsilon. The provisional K6
gate requires:

- no repeatable positive-debt cycle; and
- B_epsilon at most 16.

For a requested Laurent window epsilon^-3 through epsilon^4, terminal data must extend through at
least epsilon^(4 + B_epsilon). At the maximal terminal and debt budgets this is 12,288
high-precision Laurent coefficients before sparsity or shared constants. The actual serialized
size, not that count alone, must pass the 256 MiB gate.

Candidate scores penalize terminal growth and pole debt even though they do not penalize
nonminimality by itself.

## Candidate scorecard

This is a design score before running the experiment. Each category is scored from zero to five;
the weighted total is a hypothesis, not evidence.

| criterion | weight | A | B | C |
| --- | ---: | ---: | ---: | ---: |
| exact completeness path | 30 | 4 | 4 | 1 |
| universality and routing independence | 15 | 5 | 5 | 3 |
| terminal-growth observability | 15 | 5 | 3 | 1 |
| epsilon-pole control | 15 | 4 | 5 | 2 |
| typed-terminal/AMFlow feasibility | 15 | 5 | 4 | 2 |
| plausible K21 arithmetic scaling | 10 | 3 | 4 | 3 |
| **weighted total / 100** | 100 | **87** | **83** | **36** |

Interpretation:

- A scores highest because it asks directly whether the complete border closes and exposes the
  finite terminal presentation. Its risk is degree and sparse-fill growth.
- B can beat A when the exact complement has a few low-dimensional standard pairs. It loses
  points because terminal growth is visible only after the whole tube queue terminates.
- C can improve arithmetic after a frame exists. It has no independent finite-complement,
  guard, or typed-terminal certificate.

Minimal terminal count is intentionally absent from the rubric. Terminal *affordability* is not.

## Negative controls

Every control has a required failure mode.

1. **Bounded-miss terminals.** Declare the nine current uncovered witnesses to be terminals.
   The rank-8, rank-16, and formal outer-border tests must expose the remaining
   positive-dimensional complement.
2. **Leave one certificate source out.** Remove one translated source used by a selected exact
   certificate. The solver must find a different exact certificate or report the target hole; it
   may not reproduce the old provenance.
3. **Drop one border owner.** Remove one accepted border RuleCell and recompute standard pairs.
   A corresponding prolongation or uncovered face must return.
4. **Singular sample.** Deliberately choose a sample on a pivot or denominator guard. The modular
   scheduler must reject and record it rather than accept its lower rank.
5. **Bad prime.** Choose a prime that annihilates a reconstructed integer coefficient. Held-out
   consensus and exact replay must quarantine it.
6. **Physical-stratum rank change.** Compare a direct unit-mass matrix with a generic-mass matrix
   naively specialized to unit mass. Any difference is an explicit restriction obligation, not a
   reason to prefer the generic result.
7. **Unsafe symmetry quotient.** Remove non-stabilizer source images as though graph orbits were
   row equivalences, or merge their columns before rank discovery. Exact span, matrix-plan
   permutation similarity, or replay must fail when those rows or decorated strata are distinct.
8. **Krylov projection miss.** Use a deliberately deficient block projection. A held-out
   projection, commutator check, or exact border replay must expose the missing quotient direction.
9. **Finite-field normal equation.** Use the F5 row E = (1,2). The harness must show that
   E times E-transpose loses rank and must reject normal-equation squaring.
10. **Unbounded pole cycle.** Install a synthetic repeatable edge weighted by
    1/(d-4). The pole graph must report unbounded debt.

The source-deletion control does not assume every one of the nine ordinary sources is globally
indispensable. An exact alternative certificate is a valid outcome.

## Acceptance sequence

### Gate 0: reproducible matrix plan

Implementation status (2026-08-30): the test-only physical-frame planner is
green for all twelve `S6`/`S5`/`S4a`/`S4b`, degree-one through degree-three
envelopes. It preserves the required row chronology and source provenance,
builds a checked raw-physical-column CSR with no provenance augmentation, and
passes repeat-build byte-determinism plus exact CSR/source-entry checks.

The first modular kernel slice is also green. It samples the exact 63 by 157
`S4a` degree-one frame over a validated Symbolica `Zp64`, rejects zero source
conditions and coefficient denominators before division, and retains the
original row ordinals selected by each sparse elimination. A nonempty
target-specific physical partition agrees with an independent dense reference
for both ranks, pivot columns, and chronological independent rows. Symbolica
records coefficient-free `L` patterns and coefficient-valued `U`; the kernel
preflights `r(R+C)` total fill and enforces the registered 20-times-input fill
gate. The finite-field normal-equation negative control passes.

Gate 0 is not yet declared complete: the exact decorated-stratum owner has not
yet produced the full physical target/forbidden registry, so all registered
targets and supported worker counts have not been compared. Gate 1 additionally
still needs the multi-prime/held-out schedule and exact lift/replay.

- Regenerate the exact row, column, and entry counts in this note.
- Produce byte-identical row/column registries at every supported worker count.
- Verify checked CSR construction against a dense K6 reference.
- Preserve the raw source and translation identity for every row.
- Verify that adding provenance identity columns changes no physical target rank query because
  those columns are absent from its matrix.
- Verify that every target-specific forbidden-set query agrees with a dense reference.

### Gate 1: A0

- Run S4a degree one under all discovery and held-out samples.
- Emit rank, pivots, fill, border hits, and exactly one typed status per target:
  ExactRelation, ExactNoRelationInDeclaredFrame, or ModularNoHit.
- Lift every border hit exactly and replay it.
- Recompute only the affected decorated-stratum complements and state which exact boxes or formal
  pairs changed.

A0 passes as a prototype even if it finds no new rule, provided it reports ModularNoHit without a
negative claim or supplies an ExactNoRelationInDeclaredFrame certificate, and the negative
controls work. Neither outcome passes as closure.

### Gate 2: degree ladder and B control

- Run A at degrees two and three only while the previous resource gate passes.
- Run the six concrete axial/halo tube plans for T_N.
- Compare accepted-rule support, guard count, complement removed, and pole debt.
- Prefer the candidate that removes a larger exact formal region per retained source row; do not
  prefer a shorter RHS merely because it resembles a conventional master basis.

### Gate 3: terminal presentation

- Continue the winning candidate over all six full-rank sector orbits.
- Close every decorated application-box and coefficient-guard branch and every overlap.
- Prove that all structural standard pairs are zero-dimensional and that no unbounded exact box
  remains.
- Compute T_raw, T_orbit, and B_epsilon.
- For the direct descending-rewrite path, reconstruct only terminal relations needed to prove
  identical exact normal forms on actual overlaps.
- For the alternative quotient/action path, compute the complete R and r, prove that every shift
  action preserves R, and prove all action commutators vanish modulo R.
- Run the offline terminal-evaluation pilot.

### Gate 4: conditional C

Run C only if direct sparse fill triggers its gate. Compare rank and every recovered border
certificate with direct elimination at K6. Retain C only if it reduces measured memory or time and
keeps exact provenance compact.

### Gate 5: production eligibility

The K6 artifact is eligible only after:

- zero uncovered mathematical regions, not merely zero uncovered census roots;
- at least one complete owner on every decorated sector, fixed/free, application-box, and guard
  stratum;
- deterministic precedence and identical exact normal forms on overlaps, using exact
  overlap-specific terminal relations when needed;
- exact ordinary-source replay of every generated rule;
- strict descent and termination;
- finite terminals within the operational budget;
- bounded epsilon-pole debt;
- identical artifacts across worker counts; and
- numerical Laurent parity for all five Vakint graph classes plus the extra RustRed sector orbit.

## K10 and K21 implications

Passing K6 does not approve six-loop work. At K21, 36 ordinary sources and degree-three
one-sided translations already give 72,864 rows before sectors, guards, or fill. Candidate A must
therefore show low completion degree, reusable sparse patterns, and compact exact certificates.

Candidate B has the better possible scaling when the leader complement contains a small number of
low-dimensional standard pairs. It fails if tube width or pair count grows with target rank.

Candidate C becomes attractive only when its resident sparse products cost less than filled
elimination and its action frame is already certified. It trades fill for repeated memory
bandwidth; it does not make a large terminal quotient affordable to AMFlow.

The next scaling study should carry forward exactly the same metrics:

- rows, columns, entries, fill, and bytes read;
- standard-pair count and free dimensions;
- exact certificate support and coefficient degree;
- T_raw and T_orbit, plus optional r when a complete terminal relation module is known;
- B_epsilon and required Laurent depth;
- offline terminal evaluation cost; and
- exact complement and guard certificates.

No degree, memory, terminal, or pole budget is increased after observing a failure without
registering a new experiment.
