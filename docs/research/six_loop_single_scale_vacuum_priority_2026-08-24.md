# Six-loop single-scale vacuum priority

Status: governing deployment priority and implementation plan, adopted
2026-08-24.  This note refines the order of work without narrowing RustRed's
normative LiteRed scope.  Production algorithms remain topology- and
loop-count independent; concrete four-, five-, and six-loop graphs are test,
campaign, and benchmark inputs only.

## 1. Deployment objective

The priority deployment is a world-scale six-loop QCD beta-function
calculation.  GammaLoop already owns the general BPHZ/forest operation.  After
that operation, the integration problem of interest is a collection of
massive, single-scale vacuum integrals with many concrete numerator
structures.  RustRed should become the exact reduction engine at that
boundary.

This changes the implementation priority, not the mathematical contract:

- RustRed still derives fully parametric IBPs and guarded reduction rules from
  a caller-supplied family, as LiteRed does;
- no topology name, loop count, expected recurrence, or master coefficient may
  select production behavior;
- Vakint and existing loop-specific RustRed modules remain validation oracles;
  and
- non-vacuum LiteRed parity remains required, but it is no longer on the
  immediate performance-critical path.

The campaign naturally separates into two products:

1. an **offline rule foundry** that canonicalizes vacuum families/sectors and
   derives, verifies, compiles, and persists their parametric reductions; and
2. an **online reduction runtime** that maps large batches of concrete
   GammaLoop numerator integrals into canonical integral keys and applies the
   compiled rules with shared caches.

Discovery must never run implicitly in the hot application path.  Conversely,
a fast finite reducer is not evidence that the parametric rules were derived
generically or that their integer domain is covered.

## 2. What unit mass changes

For an `L`-loop vacuum family there are

```text
K = L(L+1)/2
```

independent loop scalar products and `L^2` ordinary momentum-space IBPs per
seed.  At six loops this is 21 scalar-product coordinates and 36 raw
parametric IBPs.  There are no external-momentum LI identities.

All physical denominators in the intended campaign carry one common nonzero
mass.  Rescaling loop momenta lets the campaign set `m^2 = 1`; the overall
mass dimension can be restored by homogeneity if ever needed.  The MS-bar
measure and renormalization-scale factors are presentation/evaluation data,
not IBP variables.

The high-performance campaign domain should therefore be explicit:

```text
derivation coefficients:  Q(d, n_1, ..., n_K)
concrete-rule coefficients after integer specialization: Q(d)
mass magnitude: 1
```

This removes a reconstruction variable and mass powers from every hot
coefficient, permits univariate sampling/reconstruction in `d`, and makes
rule artifacts reusable for every numerical choice of the auxiliary UV mass.
The generic RustRed family API must still support symbolic masses and other
kinematics; unit mass is a fingerprinted specialization mode, not a hidden
global assumption.

The denominator sign is a separate fingerprinted convention.  GammaLoop's
current Vakint/alphaLoop boundary uses the Minkowski relation
`D_r=q_r^2-m^2`, while several RustRed vacuum fixtures use a Euclidean
`q_{E,r}^2+m^2` convention.  Setting the mass magnitude to one does not erase
that distinction.  The adapter must apply and test one explicit Wick/sign map;
artifacts derived under one convention cannot be loaded as if they belonged to
the other.

## 3. Assessment of the active plan

| Existing work | Six-loop decision | Reason |
|---|---|---|
| Generic parametric IBP/LI generation in [`parametric_ibp.rs`](../../src/parametric_ibp.rs) | Keep and harden | This is the source of all 36 six-loop vacuum rows; it is already topology independent. |
| Generic affine-family map verification in [`symmetry.rs`](../../src/symmetry.rs) | Finish now | Exact family maps are the proof boundary for sector canonicalization, rule transport, and routing equivalence. Delegating its matrix algebra to Symbolica is directly on the high-loop path. |
| Bounded integer-matrix enumeration in [`symmetry_discovery.rs`](../../src/symmetry_discovery.rs) | Retain only as a small-family fallback/oracle | Radius-one enumeration at six loops has `3^36` candidates before verification. High-loop candidates must come from graph automorphisms, routing equivalences, and sector signatures, then pass through the generic verifier. |
| `GeneratedFamilySymbolicResidualSolveV1`, `WhenBad`, coverage, and provider work | Highest solver priority | This is the missing LiteRed-like bridge from generated identities to reusable guarded parametric rules. Exceptional branches and complete integer-domain coverage cannot be replaced by finite samples. |
| Global eager exact Laporta prototypes and loop-authored finite closures | Oracle only | They validate identities but scale poorly and cannot define production rules. High-loop solving must be sector-local, target-driven, modular-first, and exactly replayed. |
| Vacuum tensor/numerator parsing and scalar-product lowering | Keep on the hot path | GammaLoop supplies many numerator structures. They must be normalized once and converted to integral-key batches before rule application. |
| General external-momentum tensor bases, arbitrary pentagons, broad Feynman-parameter polishing | Defer behind the vacuum critical path | They remain LiteRed-parity requirements but do not unlock the six-loop single-scale campaign. |
| CLI and durable artifacts | Promote | Offline derivation and large campaign application must be separate invocations with inspectable, reproducible artifacts. |

The immediate symmetry migration is therefore not a detour.  The current
affine verifier is scalable once its candidates are supplied intelligently;
the current exhaustive candidate generator is not.

## 4. Offline rule foundry

### 4.1 Campaign input and topology catalog

"All topologies at a loop count" needs a declared finite graph class.  Fixed
loop count alone does not define a useful catalog when arbitrary multiedges,
vertices, powers, and disconnected products are allowed.  A campaign manifest
must state at least:

- graph source/corpus and graph-class restrictions;
- connected-component and one-particle-irreducibility policy;
- allowed QCD vertex valences and edge multiplicities;
- common-mass normalization and denominator sign convention; and
- maximum numerator/dot demand emitted by the BPHZ calculation.

The production engine accepts any authenticated family.  Exhaustiveness is a
property of a versioned campaign manifest plus a graph enumerator, not a
loop-count branch in RustRed.

GammaLoop's existing `to_vakint_integrand` boundary already performs the
right upstream graph work: subgraph shrinking, equal-mass two-bond fusion,
contiguous edge rebuilding, loop-routing solution, and consistent numerator
rewriting.  See
[`integrated.rs:482-1018`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L482).
RustRed should first consume this normalized semantic output through a typed
adapter instead of duplicating GammaLoop's BPHZ or graph surgery.

Two build/API prerequisites are explicit.  GammaLoop's useful
`to_vakint_integrand` function is currently crate-private, so the normalized
term handoff needs a small public GammaLoop-side engine interface.  RustRed and
the GammaLoop workspace also currently resolve Symbolica from different
sources; a zero-copy `Atom`/`VakintTerm` boundary requires one pinned shared
Symbolica package revision.  Text serialization is acceptable as a diagnostic
fallback, not as the six-loop hot path.

### 4.2 Canonical family and sector DAG

For each normalized connected graph, the foundry must:

1. construct the physical denominator rows and prove their loop rank;
2. handle duplicate/dependent physical denominators through the generic
   partial-fraction/overcomplete-family layer;
3. complete the `K=L(L+1)/2` scalar-product basis with deterministic ISPs;
4. fingerprint the unit-mass `Q(d)` specialization and all ordering/domain
   policies;
5. build the physical-sector dependency DAG;
6. prove zero sectors and factorized lower-loop components before row
   generation; and
7. quotient sectors/families by certified routing maps.

Symmetry candidates should be generated from graph automorphisms, canonical
edge signatures, and explicit routing solutions.  Every candidate remains
untrusted until [`verify_affine_family_map`](../../src/symmetry.rs) replays the
complete scalar-product and denominator action.  ISP images may be affine
linear combinations rather than permutations, so certificates must retain
the full family map.

### 4.3 Parametric rule derivation

Within each canonical sector, the foundry should follow LiteRed's broad
`SolvejSector` strategy:

```text
generated parametric IBPs
-> zero/symmetry/factorization canonicalization
-> persistent sector-local elimination over ordered nearby points
-> symbolic recurrence candidate
-> exact identity replay
-> pivot and boundary guards
-> WhenBad exceptional-domain partition
-> proved descending coverage of the requested integer cylinder
```

The primary high-loop exploration backend should use Symbolica finite fields
for pivot/rank discovery, sample `d` at several fresh primes, stabilize the
pivot skeleton, and reconstruct only rules reachable from the campaign's
target/numerator domain.  A reconstructed rule is publishable only after
exact `Q(d,n)` replay against freshly generated source relations and held-out
prime checks.  Modular agreement alone is not a proof.

Parametric recurrences are preferable to repeating a large finite Laporta
solve for every graph numerator.  Finite sector solves remain important for
candidate discovery, fallback coverage, and independent validation.

### 4.4 Durable compiled artifact

One artifact unit should be a canonical `(family, sector, order, domain)`
job.  It must include:

- schema and RustRed/Symbolica revision;
- family, unit-mass, routing, sector, symmetry, zero, factorization, and order
  fingerprints;
- source parametric-row identities and provenance roots;
- rules, guards, exceptional branches, descent witnesses, and coverage;
- dependencies on lower sectors/families;
- modular samples, reconstruction metadata, and independent replay status;
- master candidates or explicitly user-selected masters, never inferred from
  an uncovered timeout; and
- exact resource/benchmark census.

Artifacts should be atomically written, content addressed, resumable, and
loadable without rerunning discovery.  A CLI should expose at least `derive`,
`verify`, `inspect`, and `reduce` phases.

## 5. Online batch reduction runtime

GammaLoop currently normalizes an integrated counterterm to a
`VakintExpression`, canonicalizes it, tensor-reduces it, and evaluates it at
[`integrated.rs:280-343`](../../vendor/gammaloop/crates/gammalooprs/src/uv/approx/integrated.rs#L280).
The RustRed integration should replace the reduction/evaluation middle with a
typed native boundary while preserving GammaLoop's normalization, trace, and
MS-bar conventions.

The online pipeline is:

```text
GammaLoop normalized vacuum term
-> tensor/metric contraction and spectator separation
-> scalar products in a complete family
-> propagator cancellation and integral-key collection
-> certified family/sector canonicalization
-> iterative compiled parametric-rule application
-> factorized lower-loop dependencies
-> sparse master-coefficient map over Q(d)
```

The hot path should operate on interned integral IDs and typed sparse
coefficient maps, not repeatedly pattern-match whole Symbolica expressions.
Symbolica remains the owner of coefficient algebra.  Parsing/rendering occurs
at the boundary; rule dispatch uses compiled integer predicates, indexed shift
templates, and cached coefficient specializations.

Performance requirements include:

- batch all terms sharing a family/artifact;
- canonicalize and hash-cons integral keys once;
- memoize normal forms across numerator terms and across BPHZ forest outputs;
- cache parametric coefficient templates specialized at repeated index tuples;
- schedule independent canonical topologies/sectors in parallel while
  respecting the lower-sector DAG;
- share immutable artifacts across workers and keep mutable caches sharded;
- stream/collect output masters so one large numerator expression is not
  expanded repeatedly; and
- report uncovered keys as typed failures, never silently call them masters.

Representation-closure tests are mandatory in this path: an explicit
numerator factor `q_{E,r}^2+1` (or the selected sign-convention equivalent) must
reduce identically to the input in which the matching propagator power was
cancelled before RustRed was called.

## 6. Scaling policies

The following policies are critical at five and six loops:

- Do not enumerate every sector/seed globally.  Work from the requested
  topology manifest and target frontier, bottom-up through the sector DAG.
- Do not enumerate bounded `GL(L,Z)` matrices as the primary symmetry search.
  Generate graph/routing candidates and certify them generically.
- Do not use normalized exact rational-function elimination as the exploratory
  backend.  Discover modular pivot structure and reconstruct/replay exact
  reachable rules.
- Do not rebuild lower-loop products in every parent.  Factorize once, retain
  the transformation proof, and depend on the canonical lower-loop artifact.
- Do not mix discovery state with online reductions.  Persist and version the
  former; keep the latter deterministic and restartable.
- Do not create topology-specific Rust reducers.  Concrete topology names may
  occur in manifests, tests, benchmark labels, and oracle adapters only.

At six loops a complete family has 21 indices and each seed emits 36 rows.
These counts make allocation policy, sparse interning, symmetry/factorization,
and reachability filtering first-order requirements rather than later
optimizations.

## 7. Validation and benchmark ladder

Correctness gates remain ordered even though engineering effort is focused on
the high-loop path:

1. generated one-loop parametric rules, tensor/numerator lowering, Vakint
   comparisons, and cancellation closure;
2. complete two- and three-loop connected/factorized vacuum corpora with exact
   source replay;
3. every Vakint four-loop H/X/BMW/FG contraction and numerator fixture, with
   masters left unsubstituted;
4. multiple general five-loop families, including ISP-rich and
   duplicate-denominator cases rather than only the banana;
5. a versioned six-loop GammaLoop/BPHZ-derived QCD vacuum corpus; and
6. fresh graph routings, edge permutations, loop-basis changes, primes, and
   held-out numerator shells at every rung.

Each performance report must separate offline derivation from online
application and record at least:

- number of raw/canonical graphs, sectors, target keys, and rules;
- zero/factorization/symmetry reduction ratios;
- derivation wall time, CPU time, peak memory, prime samples, and artifact
  size;
- reconstruction and exact-replay time;
- online integrals/second, terms/second, peak memory, cache hit rate, and
  parallel scaling; and
- uncovered keys and the exact frontier that produced them.

The first six-loop milestone is not a claimed beta-function result.  It is a
replay-certified reduction of a declared GammaLoop-derived corpus to an
unsubstituted master basis with reproducible artifacts and measured batch
throughput.

## 8. Revised implementation order

1. Finish the Symbolica-native affine-family/symmetry verifier and independent
   matrix oracle; push it as a standalone milestone.
2. Finish the generic `GeneratedFamilySymbolicResidualSolveV1` rule-publication
   path, including `WhenBad`, subsector feedback, and durable artifacts.
3. Add unit-mass `Q(d)` family specialization and modular/reconstruction
   services through public Symbolica finite-field and polynomial APIs.
4. Add topology-generic graph ingestion, deterministic ISP completion,
   factorization, graph-lifted symmetry candidates, and a canonical sector
   DAG; validate through the complete Vakint four-loop corpus.
5. Implement the separate batch rule-application runtime and GammaLoop
   `VacuumIntegralEngine`-style adapter at the existing normalized-integrand
   seam.
6. Expand to general five-loop families, then execute and profile the declared
   six-loop QCD vacuum campaign.

Broad non-vacuum tensor bases, arbitrary one-loop pentagons, and
Feynman-parameter API cleanup resume after the vacuum foundry/runtime can
derive and consume reusable rules.  Algebra migrations needed by the active
vacuum path remain mandatory: RustRed does not gain permission to implement a
parallel CAS for performance.
