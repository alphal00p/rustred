# Six-loop single-scale vacuum priority

Status: governing deployment priority and implementation plan, adopted
2026-08-24.  This note refines the order of work without narrowing RustRed's
normative LiteRed scope.  Production algorithms remain topology- and
loop-count independent; concrete four-, five-, and six-loop graphs are test,
campaign, and benchmark inputs only. Implementation status below is reconciled
through the compact atomic application-event checkpoint on 2026-08-26. Its
licensed default-GMP gate passed all 1,658 runnable tests with four Nextest
workers, with 5 configured cases skipped; doctests also passed. A subsequent
worktree slice adds shallow event-bound applicable/exceptional domains; its
focused licensed parallel gates are recorded in the governing port plan.

Internal RustRed owners trust sealed constructors and move semantics. Add
runtime validation only at human/file import, durable artifact loading, and the
final mutation of live reduction state. Internal artifact formats are
disposable and may be replaced without migration during development; detailed
replay/source transcripts are optional audit data unless mathematics itself
requires an independent residual check.

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

Foundry closure and scalability precede optimization of the online runtime.
RustRed does not yet produce a complete closed family rule set: the current
compact event is one internal transition, not a reusable shard. The next
campaign claims therefore require exceptional/subsector fixed-point closure,
multi-start bundle construction, and measured physical six-loop derivation
before high-throughput application is treated as the critical implementation
milestone.

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
kinematics; unit mass is an explicitly declared specialization mode, not a hidden
global assumption.

The denominator sign is a separate declared convention.  GammaLoop's
current Vakint/alphaLoop boundary uses the Minkowski relation
`D_r=q_r^2-m^2`, while several RustRed vacuum fixtures use a Euclidean
`q_{E,r}^2+m^2` convention.  Setting the mass magnitude to one does not erase
that distinction.  The adapter must apply and test one explicit Wick/sign map;
artifacts derived under one convention cannot be loaded as if they belonged to
the other.

## 3. Assessment of the active plan

| Existing work | Six-loop decision | Reason |
|---|---|---|
| Generic parametric IBP/LI generation in [`parametric_ibp.rs`](../../src/parametric_ibp.rs) | Keep and harden | The completed explicit `L=6`, 36-row gate validates topology-independent generation and deterministic regeneration only; it does not validate arity-21 cover construction, solving, or reduction. |
| Generic affine-family map verification in [`symmetry.rs`](../../src/symmetry.rs) | Finish now | Exact family maps are the proof boundary for sector canonicalization, rule transport, and routing equivalence. Delegating its matrix algebra to Symbolica is directly on the high-loop path. |
| [Symbolica-only production algebra compliance](symbolica_only_algebra_compliance_roadmap_2026-08-27.md) | P0 before a production six-loop claim | Reachable older parametric/concrete elimination, generic Feynman-polynomial and remaining family-matrix kernels, case transformations, integer-lattice primitives, and the later tensor path still contain handwritten algebra. The native exact-group database closes one path, not this full call graph. |
| Bounded integer-matrix enumeration in [`symmetry_discovery.rs`](../../src/symmetry_discovery.rs) | Retain only as a small-family fallback/oracle | Radius-one enumeration at six loops has `3^36` candidates before verification. High-loop candidates must come from graph automorphisms, routing equivalences, and sector signatures, then pass through the generic verifier. |
| Eager Boolean-cover/case inventory over every index orthant | Replace on the high-loop path | A genuine `K=21` inactive-family probe requested symbolic case split 65,537 immediately beyond the configured 65,536 cap. The later global MTBDD avoided that partition but retained 268,427 nodes for the all-36 source. The foundry needs direct target-frontier search over owned normalized formulas, with MTBDD compilation admitted only under an explicit measured budget. |
| `GeneratedFamilySymbolicResidualSolveV1`, `WhenBad`, coverage, and provider work | Highest solver priority | This is the missing LiteRed-like bridge from generated identities to reusable guarded parametric rules. Exceptional branches and complete integer-domain coverage cannot be replaced by finite samples. |
| Global eager exact Laporta prototypes and loop-authored finite closures | Oracle only | They validate identities but scale poorly and cannot define production rules. High-loop solving must be sector-local, target-driven, modular-first, and exactly replayed. |
| Vacuum tensor/numerator parsing and scalar-product lowering | Keep on the hot path | GammaLoop supplies many numerator structures. They must be normalized once and converted to integral-key batches before rule application. |
| General external-momentum tensor bases, arbitrary pentagons, broad Feynman-parameter polishing | Defer behind the vacuum critical path | They remain LiteRed-parity requirements but do not unlock the six-loop single-scale campaign. |
| CLI and durable artifacts | Promote | Offline derivation and large campaign application must be separate invocations with inspectable, reproducible artifacts. |

The immediate symmetry migration is therefore not a detour.  The current
affine verifier is scalable once its candidates are supplied intelligently;
the current exhaustive candidate generator is not.

The compliance gate is staged: foundry algebra and exceptional-domain closure
precede the derivation-only six-loop benchmark, while tensor expansion and
lowering precede the later GammaLoop numerator campaign. Native retained
reducer trials still make committed and cloned states simultaneously live, so
the 100-core scheduler must charge old plus trial plus scratch and reduce its
effective width under RAM pressure.

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

The production engine accepts any validated typed family. Exhaustiveness is a
property of the current campaign declaration plus a graph enumerator, not a
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
4. record the unit-mass `Q(d)` specialization and all ordering/domain
   policies explicitly;
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

Parallel planning and execution follow the governing
[deterministic campaign-foundry design](parallel_campaign_foundry_design_2026-08-26.md).
Canonical families share one immutable source catalog, unique sectors derive
intrinsic rules concurrently while retaining proper subsectors on their
right-hand sides, and closure later traverses ready dependency-DAG antichains
bottom-up. Target-driven DAG discovery replaces eager `2^K` sector
enumeration. One affine case lane remains a serial, single-owner retained
Symbolica reducer; independent families and sectors, frozen-epoch exceptional
case proposals, modular samples, and exact-verification blocks are the
multicore work units.

The currently implemented campaign layer stops before actual reducer
execution. It has a static multi-root `CampaignPlan`, versioned
resource-estimate and deterministic wave-selection metadata, and a move-only
atomic admission authority. The authority revalidates a frozen selection and
cooperatively charges cores plus estimated bytes to task/resident owners; its
tests include concurrent panic cleanup and exact old/new overlap. The
roots-only CLI authenticates declared inputs but explicitly does not normalize
target numerators. These pieces do not inspect RSS, estimate a physical
six-loop family from native telemetry, construct or hydrate a Symbolica
reducer, dispatch a worker, checkpoint a wave, discover dependencies, or prove
closure. The phase-calibrated estimator, executor, hydration/dehydration
service, and barrier checkpoint runtime are subsequent milestones.

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

The normalized coverage IR, rather than any one Boolean representation, is the
scalable source authority. The sealed normalized-source compiler owns and
replays one exact row-span allocation, every ordered attempt (including dead
suffixes), the normalized IR/locus table, original pre-intersection limits,
and coverage/normalization phase censuses. Common same-allocation row-span
checks are O(1), with exact deep comparison retained for independently
allocated payload-equal proofs. Within normalized-source construction, fresh
normalization rebinds and authenticates the candidate batch once rather than
rebuilding it through each backend.

Pushed checkpoint `c593865` closes the end-to-end ingress gap with a one-pass
candidate-to-normalized-source API and a safe sealed replay token. It performs
`N` construction authentications for `N` candidates rather
than the legacy `2N`. Focused run
`b2ba7679-e7c8-4e64-ba25-c451024843bf` passed 6/6 tests, and independent
affected-suite run `db2a98a5-d473-4cdc-b2b7-fe2f444357e8` passed 44/44.

That checkpoint also uses normalized-source V2 to persist one explicit
`IntegralOrderingPolicy` for every source, including an empty-attempt source,
and authenticate every present candidate's policy. Owner-focused run
`8ad499a3-339e-4e0b-a04f-ccf754406516` passed 21/21 tests, formula/residual
run `6a5267d1-fe75-4854-8b98-9a03b1bb2370` passed 14/14, and independent
audit/validation run `430af297-b806-431e-a169-bd0f19a9f9c8` passed 30/30.
The policy-bound all-36 `L=6`, `K=21` run
`88a73ec1-52c2-4771-8a21-75e1b2a848b6` passed 1/1 with 36 construction
authentications, unchanged 15 Certified/21 Unsupported semantics, and a
1.405-millisecond first-residual search. This is pushed `c593865` evidence,
not a Ready, reduction, or physical-topology claim.

Two residual backends now consume that authority. The V5 MTBDD remains a
compact-case oracle and optional repeated-query classifier under an explicit
node/time/memory budget. Its genuine all-inactive all-36 `K=21` diagnostic has
49 normalized structural loci/atoms, 268,427 rooted nodes, and 18 terminals;
its MTBDD cursor reaches the first Unsupported terminal in 43 decisions only
after that global owner has been built. This is historical scaling evidence,
not the production six-loop route.

At pushed checkpoint `c593865`, the formula-residual cursor instead walks the
authenticated normalized candidate formulas directly. It keeps one dense
three-valued assignment table and a resumable nonzero-first DFS frontier,
prunes a partial assignment as soon as a later certified formula proves it
covered, and constructs no V4 partition, V5 MTBDD, visited set, or materialized
residual-cube inventory. Its focused parallel GMP audit passed 9/9 tests.

The one-pass checkpoint was exercised by honest all-36 `L=6`, `K=21`
primary run `37d85ddb-c356-4c79-a6f4-d428828db039`, which passed 1/1 in
58.109 seconds. It performed 36 construction authentications and preserved the
same census: 49 loci, 36 attempts, 15 Certified outcomes, 21 Unsupported
outcomes, 30 decisions, 19 free loci, and a 1,841-byte peak cursor.
Candidate-to-source construction took 17.4507 seconds, direct cursor
initialization 16.756 microseconds, and first-residual search 832.37
microseconds. The independent semantic oracle exhaustively checked all
524,288 completions. Independent K21 rerun
`e00cdbea-6312-4fb3-9856-0c2f3bf2ef25` also passed in 56.359 seconds.

For comparison, the prior two-stage K21 run
`e7378e6e-5df5-47c3-8fe9-686bbaa8ef30` took 72.935 seconds, spent 17.29 +
16.21 seconds in its two construction phases, and performed 72 construction
authentications. The new fixture also performs explicit source and path
stress-validation replays, taking 18.51 and 17.57 seconds respectively. Those
deliberate reauthentication checks are not part of production direct-search
cost. The production direct path invokes no MTBDD compiler and constructs no
MTBDD owner or DAG.

The allocation-independent terminal stable-value identity emits the
authenticated row span once through typed references and carries Direct
singleton authority through ordering V3, physical frame V2, solve-plan V2, and
source-profiled exact-session recentering without a fake inventory. Stable
value remains distinct from exact terminal/authority/frame/session `Arc`
ancestry. Authenticated lower-arity constrained Direct maps now reach
`ReadyForConditions`; independent default-GMP run
`b60b4fbd-f7b9-4656-ade0-6a476a7b7805` passed 18/18 focused tests. At that
historical checkpoint, the next steps were a transactional temporary Symbolica-
reducer correctness bridge followed by owning exceptional and subsector
scheduling to close the published domains. Both the bridge and its live retained
reducer/catalog successor are now complete, so owning exceptional and subsector
scheduling is the remaining next step.
No arity-21 Direct input has reached Ready,
and no Direct input has reached reduction, closed/durable guarded-rule
publication, or six-loop topology support.

The successful `K=21` fixture stops at the first formula-residual path. It is
not a Ready result, published guarded rule, reduction, or calculation on a
physical vacuum topology.

The direct control layer performs no algebra. Polynomial projection, monic
normalization, GCD/divisibility, matrix work, finite fields, reconstruction,
and affine-map arithmetic continue through public Symbolica APIs. The old
complete product-locus DPLL may remain a differential/fallback oracle, but the
direct high-loop entry and affine adapter must not invoke it.

Native Symbolica dense and sparse solves must also replace the older custom
`exact_sparse_elimination` wherever the public API is applicable. The live
generated-affine exact database now owns the complete easiest-first physical-
key catalog and one clone-on-stage public
`SparseRowReducer<CheckedParametricField>` in `LuLMode::Full`, with the unused
full-rank sentinel. A stage inserts only newly discovered catalog columns and
submits one candidate without replaying historical pivots. Only an independent
trial yields the move-owned reducer/catalog successor that may commit.
Symbolica authoritatively returns the ordered, potentially nonmonotone pivot
factors, normalization, and disposition; RustRed authenticates the complete
historical U/L/pivot prefix and the appended normalized U row coefficient-for-
coefficient while retaining guards and provenance. The exact-database
rebuilding glue/use is now only a `cfg(test)` differential oracle; the generic
legacy adapter remains compiled outside the live path. Licensed default-GMP runs with four test
threads pass 15/15 retained-adapter, 18/18 complete sparse-adapter, and 41/41
exact-database tests.

Committed database telemetry remains outside replay identity. The live path
still deep-clones the complete native reducer at every stage, forward
elimination remains serial, and Symbolica's opaque native heap and scratch are
not byte-censused. This does not establish a complete physical-topology
reduction, Vakint reproduction, or a six-loop memory or throughput result;
clone cost, fill, and physical-family memory must be measured before selecting
later COW, fallible-fork, or scratch-pool work.

The topology-wide canonical sector DAG is a separate foundry layer. It is not
the eager `family_sector_inventory` enumeration, and it need not block the
first one-declared-sector arity-21 Ready gate.

### 4.4 Durable compiled artifact

`PreparedPublication` is a move-owned live-session input and must not become a
serialized interchange type. The durable unit is an immutable, coverage-
closed shard for one topology-neutral `CampaignJobKey = (convention, family,
sector, ordering, coefficient specialization, domain, terminal policy)` job.
The first format is intentionally disposable and carries a simple
RustRed/Symbolica revision tag, not a migration promise. A shard contains:

- explicit family, unit-mass, routing, sector, ordering, domain, and parameter-
  embedding conventions;
- exact rules, guards, packed exceptional routes, and a proof that the declared
  domain is completely routed to descending rules or a finite explicitly
  enumerated terminal-key set (or finite products); no symbolic residual domain
  may be declared terminal;
- a compact sparse source-combination/residual witness sufficient to verify
  every rule exactly against freshly regenerated generic IBPs;
- strict lower-sector, factorization, and verified rank-decreasing cross-family
  dependencies; and
- explicitly selected master terminals or independently certified zero and
  factorized terminals, never masters inferred from an uncovered, unsupported,
  resource-limited, or timed-out frontier.

The planned `CampaignPlan` precedes this artifact and is intentionally weaker:
it will be a non-durable scheduling value containing roots, exact job
identities, dependencies, and deterministic ready-job antichains, but no rules
and no `Closed` claim. The first topology-neutral slice will use exact family-
representation identity and identity ingress, share one strict proper-subsector
child between parents, and reject cycles or non-descending edges.
Verified routing and cross-family transports are later extensions of the same
plan rather than prerequisites for testing its scheduler.

A whole calculation uses a lightweight multi-start campaign bundle. Its root
table maps every user topology/family/start domain through a verified ingress
map; its object table deduplicates canonical shards; and its dependency DAG
shares subsectors and factorized lower-loop components between otherwise
independent roots. Routing/permutation-equivalent roots are aliases, not
duplicated rule sets. Incompatible coefficient contexts or convention sets
remain distinct unless an explicit transport is generically verified.
The bundle canonical family ID is computed only after verified routing,
denominator-order, and parameter canonicalization and excludes root names and
momentum-label aliases. The existing label-sensitive family fingerprint remains
a representation/session identity and is not a cross-root dedup key. Verified
same-rank family/routing maps are collapsed into ingress aliases before DAG
construction; a cross-family dependency is admitted only with a strict
well-founded job-rank decrease.

Campaign merge is deterministic and transactional: an equal job key with an
equal payload deduplicates, while the same key with a different payload is a
conflict; the same root ID with a different ingress map also conflicts. A
shared child is one object with multiple incoming edges. Incompatible
conventions or coefficient contexts remain distinct unless an exact transport
is verified, and same-rank equivalences remain aliases rather than dependency
edges. A cycle, non-descending edge, or incomplete shard rejects the proposed
closed-manifest update.

Closed shards are written atomically and the campaign manifest is installed
last. Incomplete or interrupted work lives in a separate resumable workspace
that the reducer cannot open as `Closed`. Finalization, explicit
`verify --exact`, and first trust of an external artifact check each unique
shard's exact rule residuals from compact witnesses and verify dependency
descent. Success may write a local receipt bound to the disposable artifact
checksum and exact RustRed/Symbolica revisions. Ordinary loading of a locally
finalized artifact performs lightweight schema/revision, convention, format-
local checksum, and DAG structural checks and may reuse that receipt. Full
source transcripts, modular samples, reconstruction traces, content addressing,
canonical cross-revision byte serialization, signatures, and detailed
derivation replay remain optional. A CLI should expose at least `plan`,
`derive --resume`, `verify --exact`, `inspect`, and later `reduce` phases. Single
`I(...)` input remains the convenient one-root special case; TOML or one
`Campaign(root(...),root(...))` Symbolica expression supplies multiple starts.

Exact `Closed` admission reconstructs every family and coefficient context,
replays ingress and dependency maps, regenerates the generic IBP/LI sources and
proves every rule residual exactly zero in Symbolica, proves strict RHS and
dependency descent, proves complete declared-domain routing to rules or a
finite selected/certified terminal set (including finite products), and rejects
all cycles and unresolved routes. It runs at finalization, explicit exact
verification, or first external trust; a later local load may use the valid
receipt. One-worker and multi-worker evaluation must have identical
mathematical semantics. Full chronological replay is optional.

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

- Do not enumerate every sector/seed or all `2^K` orthants globally. Work from
  the requested topology manifest and target frontier, retain only reachable
  normalized-formula search states, compile a global MTBDD only under an
  explicit measured budget, and schedule bottom-up through the sector DAG.
- Do not enumerate bounded `GL(L,Z)` matrices as the primary symmetry search.
  Generate graph/routing candidates and certify them generically.
- Do not use normalized exact rational-function elimination as the exploratory
  backend.  Discover modular pivot structure and reconstruct/replay exact
  reachable rules.
- Do not rebuild lower-loop products in every parent.  Factorize once, retain
  the transformation proof, and depend on the canonical lower-loop artifact.
- Do not perform quadratic pairwise associate tests for every repeated
  condition locus. Project each unique locus once to Symbolica `K[n]`, use
  Symbolica's public monic normalization as the exact associate-class
  representative, and cache it under an owner-held measured budget.
- Do not mix discovery state with online reductions.  Persist and version the
  former with a disposable revision tag until an external format is declared
  stable; keep the latter deterministic and restartable.
- Do not copy the complete chronological event list or complete target-
  disposition vector on every published recurrence. Use chunked/persistent
  event storage, shared or paged copy-on-write target state, and one ordered
  packed route array per event with shallow rule/residual handles.
- Do not create topology-specific Rust reducers.  Concrete topology names may
  occur in manifests, tests, benchmark labels, and oracle adapters only.
- Do not let worker completion order select pivots, modular samples, rules, or
  dependency edges. Stable work keys and canonical merges must make 1/2/4-
  worker artifacts semantically identical.
- Do not share one checked-field controller between active reducer workers.
  Share immutable coefficient contexts and family-source catalogs, but give
  every concurrently active case lane its own controller and reducer owner.
- Keep global estimated-memory admission separate from per-job algebra limits.
  Backpressure may delay work; it must not create worker-count-dependent
  derivation-contract failures and never has master-discovery semantics.

### 6.1 100-core EPYC memory-first execution contract

The intended large-node campaign target is approximately 100 cores and 1 TiB
of RAM on an EPYC-class NUMA host. `--n-cores` is a ceiling for the complete
invocation, not a command to activate that many reducer owners. Every wave is
admitted against cores and estimated memory conjunctively; RAM pressure may
therefore leave most cores idle, and that is the correct outcome rather than a
utilization failure.

Before constructing a pool, the campaign selects the largest feasible
effective execution width `E` with `1 <= E <= --n-cores`. `E=1` runs inline on
the coordinator with no worker thread; `E>1` creates `E` workers, while the
coordinator remains a separate Symbolica Workspace owner. The fixed baseline
charges coordinator stack/TLS plus every possible worker stack/TLS and any
explicitly admitted inner thread. If that baseline plus the minimum runnable
task cannot fit even at `E=1`, the run returns a typed memory-capacity pause
before pool construction. Requested/effective widths, worker-thread count,
and estimator revision are physical metadata excluded from mathematical
hashes.

The admitted live-set model is explicit:

```text
fixed runtime and shared Symbolica/RustRed catalogs
+ old hydrated resident reducers
+ new retained successor reducers/results
+ transient clone, algebra, ingress, and serialization scratch
+ bounded staged-result/checkpoint buffers
<= configured campaign memory ceiling
```

Old and successor reducers coexist during clone-on-stage and commit. Shared
immutable payloads are charged once, but a process-local or native allocation
is never declared shared merely because its mathematical value is equal.
`--max-memory` is configured below physical RAM with declared headroom for the
OS, allocator fragmentation and arenas, Symbolica's opaque native heap,
thread stacks/TLS, filesystem cache, and checkpoint I/O. A cgroup or outer
supervisor remains necessary for a hard RSS limit.

The ready DAG and formula frontiers remain compact metadata. Only the bounded
selected wave is hydrated, and inactive lanes are checkpointed and
dehydrated by a deterministic policy when the next wave otherwise cannot fit.
Each frozen wave settles or durably stages its results before the canonical
merge/checkpoint barrier exposes the next frontier. RustRed must not fork a
process per family, sector, or job: duplicating the Symbolica runtime,
catalogs, allocator state, and thread-local caches would erase the memory
benefit of one shared campaign process. Any later distributed mode shards
durable jobs explicitly between nodes and applies this resource contract
independently on each node.

NUMA topology is execution metadata, not mathematics. The first executor
should use first-touch placement for newly hydrated owners, avoid migrating a
live reducer between nodes, and measure remote-memory traffic and bandwidth
saturation. Later socket-aware affinity or packing is permitted only if it
preserves the same stable wave, merge, and artifact semantics; it may alter
timing but never pivots, rules, or hashes.

The estimator starts conservative and versioned. Campaign telemetry will
record predicted versus observed phase peaks, old/new overlap, native U/L
stored-entry fill, coefficient-limb growth, staged bytes, RSS/allocator deltas,
NUMA locality, worker utilization, and the fraction of time limited by cores
or memory. Calibration may update coefficients and safety margins only in an
explicit new estimator revision at a canonical barrier or in a later run;
instantaneous RSS and worker completion order cannot adapt policy inside a
frozen revision.

The 2026-08-26 licensed whole-tree regression exposed a concrete profiling
target: `equality_target_commits_only_into_a_sealed_refined_epoch_suspension`
took 4,069.531 seconds in an otherwise passing 1,651-test run. This is not a
physics capability failure, but it is unacceptable as an unexamined cost model
for the six-loop campaign. Profile the equality-refinement fixture, replay, and
full-vector successor preparation separately before treating the current exact
session path as performance evidence.

At six loops a complete family has 21 indices and each seed emits 36 rows.
These counts make allocation policy, sparse interning, symmetry/factorization,
and reachability filtering first-order requirements rather than later
optimizations.

## 7. Validation and benchmark ladder

Correctness gates remain ordered even though engineering effort is focused on
the high-loop path. The derivation/foundry lane comes first:

1. coverage-closed one- through three-loop rule shards with exact source
   residuals, exceptional recursion, solved-subsector feedback, and explicit
   terminals;
2. complete derived replacement systems for every frozen Vakint four-loop
   H/X/BMW/FG family, without FORM or copied authored recurrence tables;
3. deterministic multi-start bundles proving routing aliases, shared
   subsectors/factorizations, incremental reuse, and equivalent one-worker and
   multi-worker semantics;
4. multiple general five-loop families, including ISP-rich and
   duplicate-denominator cases rather than only the banana;
5. a pre-run-frozen, structurally representative QCD-valid quartic/cubic six-
   loop corpus from family construction through closed dependency DAGs, then a
   small versioned GammaLoop/BPHZ-derived multi-root corpus; and
6. fresh graph routings, edge permutations, loop-basis changes, primes, and
   held-out specializations at every rung.

Only closed shards enter the application/oracle lane. A minimal generic seam
must reproduce scalar and tensor/numerator reductions against Vakint through
four loops with RustRed's raw masters left unsubstituted and must pass
numerator/denominator cancellation closure. The optimized batch runtime is
prioritized only after the physical six-loop derivation gate; it is not needed
to make that gate meaningful.

Vakint acceptance compares the final normalized expression over those
unsubstituted master/terminal symbols, together with its semantic guard domain
after the explicit convention map. It does not require RustRed to rediscover
the same authored FORM recurrence identities, pivot order, or intermediate
rules.

Vakint is not a generic derivation oracle: its four-loop FMFT-backed outputs
are frozen compatibility/end-to-end data, and it provides no five- or six-loop
oracle. Every RustRed rule must still have zero exact residual against freshly
generated generic IBPs before publication; neither a topology name nor a
frozen master-substituted number may select production behavior.

Specialization closure is a campaign gate: derive/reduce with symbolic `m^2`
and then set `m^2=1`, and compare with specializing the declared family and
input before reduction. The results must agree after restoring the overall
mass dimension by homogeneity, with the explicit Wick/sign convention checked
independently.

All campaign suites run with licensed, GMP-enabled Symbolica and no `no_gmp`,
FORM, or Mathematica. Shard in parallel by family/sector/corpus with isolated
artifact directories, and require equivalent loaded bundle semantics between
one-worker and multi-worker runs. Byte checksums are required only if canonical
serialization is deliberately defined.

Each performance report must separate offline derivation from online
application and record at least:

- number of raw/canonical graphs, sectors, target keys, and rules;
- zero/factorization/symmetry reduction ratios;
- derivation wall time, CPU time, peak memory, prime samples, and artifact
  size;
- reconstruction and exact residual-verification time;
- online integrals/second, terms/second, peak memory, cache hit rate, and
  parallel scaling; and
- uncovered keys and the exact frontier that produced them.

The first physical six-loop milestone is derivation-only, not a claimed beta-
function result and not an online-throughput result. Its topology manifest is
frozen before execution and uses actual GammaLoop/BPHZ roots when available.
The inaugural fallback corpus includes a QCD-valid connected 1PI quartic `K5`
root (10 physical lines, 11 ISPs) and a cubic 10-vertex/15-line representative
such as Petersen or a lower-symmetry graph (6 ISPs), with multiple non-
factorizing reachable sectors. Each 21-coordinate family processes all 36
sources and closes every reachable exceptional, subsector, factorization, and
rank-decreasing cross-family dependency onto a finite enumerated set of user-
selected or independently certified terminal keys or products. Every rule has
zero exact residual against freshly regenerated generic IBPs; every dependency
strictly descends; no reachable `Unsupported`, resource, timeout, uncovered, or
unresolved exceptional leaf is accepted. A subsequent small multi-root corpus
must reuse shared shards deterministically.

That milestone records named hardware, release/GMP configuration, wall and CPU
time, peak RSS, rule/event/target/locus/case counts, queue peak, coefficient
growth, dependency/deduplication counts, artifact bytes, and 1/2/4-worker
scaling. “Reasonable” means a numerical resource envelope frozen before the
run, not an undocumented or post-hoc timeout. The provisional dedicated-host
target is at most 48 GiB peak RSS, 24 hours wall time per root, and 48 hours for
a three-root bundle. If the ready-job antichain exposes at least four
independent jobs, four workers must reach at least 2.5x one-worker speedup;
otherwise the manifest must predeclare the measured critical-path exception.
Exceeding a threshold fails the gate and never discovers a master. The later
online milestone reduces a declared GammaLoop-derived numerator corpus to
unsubstituted terminals with reproducible artifacts and measured batch
throughput.

## 8. Revised implementation order

Checkpoint update: the affine-family-map V2 milestone is complete, and the
first non-publishing slice of generic rule derivation is implemented. Its
authenticated exact-Ready phase accepts selector-independent compact affine
maps, proves physical-key descent inside the source chamber, and retains
arbitrary-precision inactive-orthant hazard intervals for later partitioning.
Condition mapping, relative bad-domain partitioning, and compact move-bound
route preparation are now implemented. The route stage performs one linear
pass and stores one byte per leaf; it trusts its sealed Ready owner rather than
adding another schema/replay/binding layer. The atomic target-consuming compact
application-event commit is also implemented. Shallow rule/residual views,
exceptional scheduling, subsector feedback, a coverage fixed point, durable
closed shards, and application remain unfinished. This status does not claim a
complete family reduction or any six-loop result.

The first genuine arity-21 attempt exposed the eager-case blocker before
Ready: Boolean-cover construction requested split 65,537 beyond its 65,536
limit. The cap was intentionally not raised. A fast separate generator gate
checks the generic `L=6` formula produces 36 parametric IBPs. The subsequent
global-MTBDD experiment avoided the explicit orthant partition, and the new
MTBDD cursor reaches its first residual without flattening paths, but the all-36
source still retains 49 atoms and 268,427 nodes. Arity-21 Ready and condition
stress therefore required direct normalized-formula frontier search rather
than larger caps. The generator gate and all 11 tests in the
independent Ready/publication validation module passed licensed `--lib -j4`
Nextest run `a06d5558-e404-4048-a2e9-5407277a95d6`.

The MTBDD cursor's five compact replay/filter/resource tests passed independent
licensed parallel Nextest run `6fa17e71-f9ec-4fdb-9be0-434e8119977f` (5/5,
998 skipped). The final post-audit licensed parallel run
`d1b3d6f2-70fe-4da2-ba36-9a671f48080a` included those five tests plus the
explicitly ignored real all-36 stress and passed 6/6 with 997 skipped; the
stress itself took 132.985 seconds. That timing includes source construction,
authenticated replay, first-path traversal, and cheap exact/one-below repeats;
it must not be quoted as an online reduction benchmark.

The shared normalized-source checkpoint passed independent licensed GMP
Nextest run `1cd1cd6f-b282-489e-b1d6-3bf2088f635a` (15/15 focused owner,
MTBDD-certificate, and polarity tests) and residual-compatibility run
`ce3d7162-ba19-45bb-9c36-9f087ef0de48` (5/5, with the K21 stress excluded).
The stable normalized-source SHA-256 was
`f74ccd89ce1755d7672393a169dbd0e2586a2675c9643f77196697154ad3629e`.

The sealed fresh-normalization seam and direct formula-residual cursor are now
implemented at pushed checkpoint `c593865`. The direct cursor's focused
parallel GMP audit passed 9/9 tests, including exhaustive small-IR differential
comparison, source-backed MTBDD comparison, routing/filtering, tamper
rejection, and exact resource boundaries.

The same pushed checkpoint additionally implements one-pass sealed candidate
ingress/replay. Focused run
`b2ba7679-e7c8-4e64-ba25-c451024843bf` passed 6/6 and independent affected
run `db2a98a5-d473-4cdc-b2b7-fe2f444357e8` passed 44/44. Primary honest
all-36 `K=21` run `37d85ddb-c356-4c79-a6f4-d428828db039` passed 1/1 in
58.109 seconds with 36, rather than 72, construction authentications. Its
candidate-to-source/direct-initialization/first-residual timings were 17.4507
seconds, 16.756 microseconds, and 832.37 microseconds. It retained the same
49/36/15/21 locus/attempt/Certified/Unsupported census, used 30 decisions with
19 free loci and a 1,841-byte cursor, and exhaustively checked 524,288
completions. Independent run `e00cdbea-6312-4fb3-9856-0c2f3bf2ef25` also
passed in 56.359 seconds. Its explicit 17.95/17.26-second source/path stress
replays are validation checks rather than production search phases. The direct
path constructs no MTBDD compiler, owner, or DAG. This checkpoint stops at a
certified formula-residual path; it has not produced an arity-21 affine
inventory, exact Ready outcome, published guarded rule, complete reduction,
or physical-topology calculation.

1. **Completed:** Symbolica-native affine-family/symmetry verifier and
   independent matrix oracle, pushed as a standalone milestone.
2. **Completed generation-only checkpoint:** topology-neutral `L=6`, `K=21`
   generation of all 36 ordinary IBPs. It stops before Boolean cover and Ready.
3. **Completed separate lower-arity checkpoint:** exact compact-affine Ready
   geometry, fixed-chamber descent, and lazy hazards. No arity-21 input has
   reached Ready yet.
4. **Completed traversal checkpoint:** an authenticated bounded `O(depth)`
   cursor over the rooted MTBDD, including compact replay/resource tests and
   an ignored honest all-36 `K=21` scaling oracle. This does not solve global
   MTBDD construction and has not reached Ready.
5. **Completed direct-search checkpoint:** the replayable authenticated
   normalized-source owner, sealed fresh-normalization seam, and bounded direct
   formula-residual cursor are implemented. The direct cursor bypasses V4, V5,
   and the residual Boolean/DPLL owner; the 10/10 audit includes a successful
   direct all-36 `K=21` formula-residual search without an MTBDD owner or DAG.
6. **Completed at pushed checkpoint `c593865`:** one-pass
   candidate-to-normalized-source construction ahead of V4 plus a safe sealed
   replay token. Focused and independent affected suites pass; the all-36
   `K=21` comparison reduces construction authentication from 72 to 36.
7. **Completed at pushed checkpoint `c593865`:** normalized-source V2 binds
   `IntegralOrderingPolicy` for every source, including empty-attempt sources,
   and authenticates all present candidate policies. Focused 21/21 and 14/14
   suites, an independent 30/30 audit/validation run, and the policy-bound
   all-36 K21 1/1 gate pass.
8. **Completed:** terminal stable identity through Direct ordering V3, frame
   V2, solve-plan V2, and source-profiled exact-session ingress, with no fake
   inventory and exact `Arc` ancestry separately authenticated. Authenticated
   constrained compact maps now reach `ReadyForConditions`; the production
   regression replays six RHS descent witnesses. This has not reached rule
   publication, reduction, or six-loop topology support.
9. **Completed through shallow event-bound domains and live native sparse
   authority:** current-lineage condition
   mapping, canonical-locus relative `WhenBad` partitioning, one-pass
   move-bound Ready/route preparation, and atomic database/target/event commit.
   The compact event now exposes shallow applicable/exceptional leaves and the
   full parent-premise-plus-relative-predicate domain. The exact database now
   owns the complete easiest-first physical-key catalog plus a clone-on-stage
   Full-L `SparseRowReducer`, commits only independent move-owned reducer/
   catalog successors, and coefficient-authenticates each appended normalized
   U row after the historical prefix. The exact-database rebuilding glue/use is a `cfg(test)`
   oracle. Focused licensed default-GMP four-thread runs pass 15/15 retained-
   adapter, 18/18 complete sparse-adapter, and 41/41 exact-database tests.
   Export native telemetry to campaign benchmarks and profile full-clone cost,
   serial forward elimination, fill, and opaque native memory. The
   non-durable topology-neutral `CampaignPlan` slice and its stateless
   resource-estimate/wave-selection companion are now implemented with exact
   representation-level deduplication, identity ingress, shared proper-
   subsector children, cycle/non-descent rejection, and a deterministic
   ready-job antichain. A separate invocation-wide move-only core-plus-memory
   admission authority now atomically charges selected waves and retained
   successors. Its first stable indexed executor and resident-transform seam
   move a complete exact session through a genuine Symbolica dependent-row
   transition with old/new/transient overlap charged. This remains a
   cooperative low-level primitive, not the campaign runtime: there is no
   calibrated physical estimator, effective-execution-width selector, frontier
   coordinator, reducer hydration policy, or checkpoint barrier yet. On a
   100-core node, choose `1 <= E <= --n-cores` before pool construction and
   separately charge the coordinator plus every potentially warmed worker's
   Symbolica thread-local cache in the fixed baseline. A no-fit `E=1` run
   returns a typed memory-capacity pause without building a pool. Implement that bounded
   coordinator before finishing
   `GeneratedFamilySymbolicResidualSolveV1` with
   exceptional scheduling, solved-subsector feedback, a proved coverage fixed
   point, exact residual verification, and the distinct 36-source session
   batch.
10. Replace quadratic session event/target storage and add unit-mass `Q(d)`
    specialization plus modular/reconstruction services through public
    Symbolica finite-field and polynomial APIs.
11. Extend the earlier `CampaignPlan` with topology-generic graph ingestion,
    deterministic ISP completion, factorization, graph-lifted symmetry
    candidates, verified routing, and canonical job identity; then build the
    canonical lazy physical-sector dependency DAG and compile immutable closed
    job shards into deterministic multi-start campaign bundles.
12. Derive the complete Vakint one- through four-loop replacement-system
    corpus without FORM or copied recurrences. Use a minimal generic
    application seam to compare normalized reductions with unsubstituted
    terminals against Vakint as an external oracle. This is the authoritative
    compatibility lane for the generic foundry, not production recurrence
    input.
13. After the one- through four-loop oracle lane closes, close and profile
    multiple general five-loop families, then one physical nontrivial six-loop
    root and a small multi-root GammaLoop/BPHZ-derived derivation corpus under
    declared time, memory, and parallel-scaling budgets. Vakint supplies no
    five- or six-loop derivation oracle; exact regenerated-IBP residuals and
    closure/resource manifests are authoritative there.
14. Only after those foundry gates, optimize the separate batch rule-
    application runtime and GammaLoop `VacuumIntegralEngine`-style adapter at
    the existing normalized-integrand seam.
15. Execute the later declared six-loop numerator-reduction campaign and
    publish online throughput separately from foundry derivation metrics.

Broad non-vacuum tensor bases, arbitrary one-loop pentagons, and
Feynman-parameter API cleanup resume after the vacuum foundry/runtime can
derive and consume reusable rules.  Algebra migrations needed by the active
vacuum path remain mandatory: RustRed does not gain permission to implement a
parallel CAS for performance.
