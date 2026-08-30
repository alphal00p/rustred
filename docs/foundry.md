# Rule foundry

## Status

Five narrow foundry capabilities are now live. The anchored boundary
specializes a chronological `ParametricRelation` slice at one concrete integer
point. The sector-interior boundary instead eliminates that slice directly
over the authenticated Symbolica field `K(n)`, orders shifts hardest-first,
and returns one parametric recurrence only after exact symbolic source replay,
uniform strict-descent proofs, all required retained pivot/denominator guards,
and agreement with an independently derived anchored rule. Its domain is the
largest representable box on which every source shift stays in the anchor's
fixed sector. The target-directed variants request one concrete integral or
free-index shift, require it to be a forward pivot, compute the complete
upper-triangular dependency envelope, and invoke Symbolica's deterministic
serial back-substitution before the same replay, guard, descent, and anchored
agreement gates. Generated tadpole and caller-selected two-loop sunset spans,
a genuine two-source ordinary-IBP/LI elimination, and an all-four-row sunset
target recurrence exercise these paths.

The target-directed parametric path also has a sector-monotone admission. It
retains the exact fixed-sector rule and replay, then admits specialization on
the maximal i64-representable parent-sector box. Each RHS shift first stores a
compact first-pinched proof and can refine it into an exact finite product
partition whose every cell has one fixed target-sector mask. Optional
coordinates are represented in O(K) metadata and cells are materialized
lazily, so constructing the partition does not enumerate its potentially
exponential cell set.

`foundry::dependency` turns those term-local partitions into one preflighted,
stable-ordinal proper-subsector discovery stream. It retains O(R*K) partition
metadata but yields O(1) descriptors bound to the parent rule, ordered RHS
coefficient, guards, and exact target-cell ordinal. Exact base, pivot, and
target domains are allocated only on explicit demand. Aggregate described
cell and obligation counts are admitted before iteration; a process-local
cursor can resume only against the same borrowed rule allocation. The sunset
corner `[1,1,1]` is the first sentinel. This API still requires a nonempty
common fixed-sector interior for the supplied source span; it does not yet
replace source preparation with a fully piecewise reducer.

The general dependency foundry is not yet a generic closure search for an
arbitrary caller-supplied family. A registered `foundry::artifact` installer
does, however, close the canonical unit-mass `K = 1` tadpole and `K = 3`
sunset partitions from freshly generated sources. The sunset owns five exact
application cells, `S3` routing, Lee--Pomeransky zero proofs, a certified
unimodular pinch factorization, and immutable lower-family feedback.
`reduction::Reducer` applies either sealed owner.

A deterministic schema-v2 codec persists exact family inputs, tagged source
plans and semantic witnesses, rules/cells, symmetry and factorization data,
masters, zero terminals, and homogeneity proofs. Bounded loading independently
reconstructs and exactly compares the registered semantics once under the
caller's family/source/rule policies, then returns a sealed owner. The reducer
does not repeat artifact authentication. Three-loop `K = 6` remains open;
successfully deriving one isolated rule for any other family is not a closure
claim. Discovery cursors are not durable artifact identities, and cold proof
replay may perform fallible O(K) allocations outside the streaming work
budget. A test-only `K = 6` pressure fixture now authenticates the exact
six-denominator family, all nine ordinary sources, the order-24 `S4` action,
and the complete eleven-orbit sector partition. It freezes internally checked,
revision-stamped snapshots for all five Vakint classes; live matcher comparison
remains a separate cross-repository gate. Generic factorization tests certify
`K3 x K1` and both inequivalent spanning-tree `K1 x K1 x K1` products. The
first test-only K6 rule cell derives an exact top-sector recurrence from the
complete nine-source span and retains its elimination provenance, guards,
maximal application box, and strict-descent witnesses. A separate residual
projection supplies all nine sources and all 26 exact zero masks to derive the
two canonical positive dotted-edge cells on the five-line face; each retains
its projection replay, guards, application box, and descent proof. Those cells
do not cover negative inactive powers or numerator faces. On the irreducible
four-line face, a target-aligned translation derives a guarded canonical-dot
multi-excess cell and the untranslated span derives the canonical mixed
numerator/dot boundary cell. All four raw dot and eight raw mixed placements
are routed through the exact `S4` canonicalizer. The translated dot rule is
singular at the isolated pure-dot corner. A fixed-corner residual projection
lowers that corner to the scalar corner from two selected ordinary rows and
routes all four raw dot placements. A one-dot translated projection supplies
a strict-descent recurrence for the opposite two-dot orbit from four selected
rows; exact canonicalization routes its two raw placements. Its descendants
remain obligations of the surrounding fixed point.
The inequivalent four-placement adjacent-pair orbit remains an explicit typed
residual after the complete untranslated span reports the target absent and
the natural translated span reports that it is not a pivot. These bounded
negative results are not an exhaustive translated-source search or a terminal
classification. Deeper dot and numerator faces remain open. The fixture
intentionally exposes no closed artifact while its complete rule fixed point
is incomplete. The
[project goal](../GOAL.md) is the authority whenever this design and the
implementation frontier differ.

The foundry will be an offline service. Ordinary Vakint evaluation will load
precomputed closed artifacts; it will not rerun the search. The implementation
must remain generic in topology and loop count. A specialized high-throughput
lane may be selected only from proved family properties such as vacuum
kinematics, common nonzero mass, and valid unit-mass homogeneity. Stage 1 uses
the current exact methods through three loops; high-loop breakthrough and
extreme-efficiency work are explicitly deferred.

## Input and output contract

A foundry root binds:

- one authenticated integral family, including coefficient/index variable
  order, routing, metric and denominator signs, physical versus auxiliary
  coordinates, cuts, power shifts, kinematics, and ordering policy;
- freshly generated ordinary IBP and, where applicable, LI source rows;
- proved zero, restriction, symmetry, factorization, and cross-family
  transports; and
- an explicit terminal policy, resource policy, and deterministic search
  configuration.

Complete generated source spans can now be recentered through
`ParametricIbpGenerator::translate_completed_source_rows`. The public boundary
accepts sealed `CompletedIbpSourceRows` plus bounded `IntegralShift` values,
rejects foreign family/context scopes before symbolic work, sorts and
deduplicates offsets lexicographically, and emits rows in offset-major then
sealed-source order. Each immutable result retains the original stable
`RowId`, source ordinal, and exact offset. A zero offset copies the sealed
equation exactly while recording zero-offset provenance in the wrapper;
nonzero offsets use Symbolica's checked coefficient translation and checked
integer shift addition. Aggregate row, term, condition, condition-provenance,
and retained-coordinate limits precede construction. Zero translations
re-admit the sealed payload once under the current relation limits; nonzero
translations use a crate-private sealed ingress so translated Symbolica
results are authenticated once rather than rescanned during relation
insertion. This is a generic search-input primitive only; it does not by
itself close the sunset or any other multi-line family.

There are two deliberately different outputs:

- a resumable workspace may contain queues, modular samples, partial
  eliminations, uncovered domains, or resource pauses; and
- an immutable `Closed` artifact contains only a completely discharged root
  with exact regenerated-source replay.

Incomplete state can never deserialize or install as a closed artifact.
Timeout, cancellation, unsupported algebra, an exhausted search depth, and an
uncovered key are results to resume or report, not alternate spellings of a
master integral.

## Strict closure

A root is closed only when every reachable domain has been discharged to one
of the following:

1. a guarded rule whose right-hand side is strictly lower in the persisted
   well-founded order;
2. an already closed proper-subsector or cross-family dependency;
3. an independently certified zero, product, or factorized terminal; or
4. a finite master key selected by an explicit, versioned terminal manifest.

Every rule must retain its exact integer-domain guard, pre-cancellation
polynomial nonzero guards, source-row combination, dependency set, and descent
witness. Reconstructing the rule from freshly generated source rows must give
an exact zero residual. A symbolic residual domain, a successful sample, a
stable structural row count, or failure to find another pivot is not closure.

## Generic fixed point

The foundry is target-driven and lazy. It does not eagerly enumerate all
`2^K` sectors or all integer orthants. In outline:

```text
requested roots
  -> canonical proved symmetry representatives
  -> required proper subsectors and factorization dependencies
  -> zero/restriction classification
  -> exact residual case queues
  -> anchored identity generation and sparse elimination
  -> guarded recurrence candidates
  -> accepted domains plus exceptional children
  -> immutable solved-dependency feedback
  -> repeat until every reachable residual is discharged
```

Restrictions and analytic zero proofs remain distinct. A cut or user pattern
may exclude a sector without proving that its integral is zero. Likewise, a
canonical graph signature or polynomial signature may propose a symmetry, but
only an exact momentum/denominator transformation with the correct Jacobian
may quotient work. A verified symmetry maps a query to a representative; it
cannot turn an unresolved representative into a closed one.

Solved proper subsectors feed back into parent work through immutable,
fingerprint-bound dependencies. The scheduler repeats sector solving,
exceptional refinement, and dependency substitution to a deterministic fixed
point. Arrival order or worker count must not change the semantic artifact.

## Residual recentering

The unit of unresolved work is an exact symbolic case, not just the original
sector corner. Cases are ordered deterministically, grouped by their fixed and
free coordinates, and assigned authenticated search anchors. A bounded lattice
diamond is generated around the current anchor; the depth can grow or reset
when the first remaining anchor changes.

For an affine residual locus

\[
F(t)=b+A t,
\]

an ambient source relation with shifts `s`, searched at offset `delta`, must
be recentered as

\[
R_{\delta,F}(t)=\sum_s c_s(F(t)+\delta)
                 J(F(t)+\delta+s).
\]

Translation therefore happens before affine substitution. Substituting first
would generally construct `F(t + delta)`, a different locus. The source case,
anchor construction, depth, candidate ordinal, and generated-row span remain
available to replay; covering one concrete anchor does not prove its symbolic
parent case.

## `WhenBad` and exceptional domains

Candidate elimination and domain coverage are separate proofs. A useful
algebraic pivot is generalized, then its exact applicability is derived:

- a denominator depending on free parameters is identically bad only when all
  of its parameter-coefficient polynomials vanish;
- equality of a product is a disjunction of factor equalities, while
  nonvanishing of a product is a conjunction, modulo proved coefficient-field
  units;
- only activation of an inactive sector index is a containment leak; pinching
  an active index to zero is allowed; and
- a leaking right-hand-side alternative matters only where its coefficient
  numerator also survives.

For a current case `C` and exact bad locus `B`, the published domain is
`C && !B` and the exceptional child is `C && B`. If `B` is identically true,
nothing is published for that case. The algebraic pivot may remain useful to
the local elimination database, but neither rejection nor search exhaustion
certifies a master. Predicates that cannot be represented or decided remain
explicit typed residuals.

RustRed owns this Boolean/integer-domain semantics and provenance. Symbolica
owns coefficient projection, substitution, exact polynomial arithmetic,
factorization or GCD when used, and sparse row algebra through its public Rust
API. Cancellation in a fraction field never erases the stored exceptional
locus.

## Exact solving and reconstruction

Integral keys are mapped once to columns in hardest-first physical order.
Symbolica's sparse row reducer is the primary exact row-algebra primitive;
RustRed owns column meaning, source chronology, guard construction, and replay.
Modular images, support discovery, CRT, interpolation, and rational
reconstruction may accelerate candidate discovery. Prime/point schedules are
deterministic, bad samples are rejected, and every reconstructed rule is
verified over the exact authenticated coefficient domain. Finite-field
agreement alone cannot publish a rule.

The live anchored and sector-interior boundaries augment their integral/shift
matrices with chronological identity columns. Forward reduction therefore
exposes both a normalized row and its exact source combination in `U`, while
`L` identifies the complete chronological chain of pre-normalization pivot
coefficients. The parametric path passes Symbolica's native rational-polynomial
coefficients directly to the same reducer; it does not sample and fit an
ansatz or round-trip through `Atom`. Neither path uses sparse `solve` or sparse
`inv` as an oracle. Visible input/output structure and exact replay work
are limited with typed errors. Aggregate structural budgets separately census
live integral/shift coordinate cells, guard provenance, fixed-sector bounds,
and the coordinate buffers owned by prepared and strict-descent ordering keys.
Symbolica 2.2.0 still exposes no hard scratch-memory census or cancellation
hook for the reducer.

A missing public Symbolica primitive is a typed unsupported boundary. In
particular, the pinned Symbolica revision has no complete public integer
normal-form/affine-lattice service or multivariate rational-function
reconstruction service. RustRed may own orchestration around public algebraic
primitives, but must not grow a parallel CAS.

## Artifact boundary

The current durable schema-v2 format records the semantics needed to load and
apply the sealed `K = 1` and `K = 3` artifacts:

- schema/algorithm identifiers and canonical bounded sparse binary Symbolica
  rational-polynomial payloads;
- exact `IntegralFamily` constructor inputs, family/indexed-context
  fingerprints, and the ordering identifier;
- a tagged complete-ordinary source derivation plan plus a full semantic
  witness retaining source terms and pre-cancellation condition provenance;
- ordered guarded rule-cell domains and proofs, fixed-index source views,
  proof-bearing residual projections, rule snapshots, strict-descent and exact
  replay evidence;
- exact canonical symmetry actions, immutable lower-artifact dependencies,
  factorization projections, and installer-compiled typed master-product
  embeddings; and
- stable master keys, proved zero-sector terminals, common-mass homogeneity
  proofs, and deterministic tagged-section/rule-plan metadata.

Encoding and decoding have explicit total, collection, string, per-payload and
aggregate coefficient, aggregate semantic-witness, family, source-generation,
relation, and rule-replay resource policies. Coefficients carry expanded
sparse numerator/denominator terms on the family context's authenticated
ordered variable map; bounded counts, magnitudes, and `u16` exponents are
checked before Symbolica construction. No expression parser, process-local
symbol identifier, or foreign symbol can enter this boundary. Source and rule
witnesses are opaque exact-comparison bytes with their own shared monotonic
budget, rather than being reparsed as algebra. Untrusted bytes are
authenticated once at load/installation, and hot-path reduction does not
repeat schema round trips or whole-artifact replay. Atomic filesystem
publication remains an application-layer responsibility. The `K = 3`
installer independently validates its complete five-cell projection,
symmetry, factorization, and terminal layout at this cold boundary. No such
authentication is repeated by recursive hot-path reduction. Schema v2 records
the generic typed dependency-master product embeddings needed by `K3 x K1`;
the future `K = 6` codec will extend the registered algorithm payload without
changing those semantics.

## RAM-aware deterministic parallelism

The parallel design is intentionally coarse grained:

- one coordinator owns deterministic work ordinals and merge barriers;
- one active case lane owns one coefficient context and one mutable sparse
  reducer, whose ordered forward mutation remains serial;
- independent families, sectors, frozen residual proposals, modular sample
  ordinals, source preparation, and verification blocks may run in parallel;
  and
- immutable family/source data is shared rather than cloned per worker.

Execution width is selected from both the requested core ceiling and admitted
RAM. Admission accounts for coordinator and worker state, Symbolica
thread-local/native headroom, GMP payloads, predecessor/trial/successor reducer
overlap, result staging, and bounded communication buffers. Cores and memory
are reserved together before constructing a lane. There are no nested pools,
per-task process forks, unbounded worker queues, or full symbolic-state copies
sent between workers.

Workers return compact references or framed bounded chunks. Merges occur only
at stable sorted barriers, so `n_cores = 1, 2, 4` produces the same semantic
artifact. Thread, process, or hybrid execution is a measured implementation
choice; peak RSS, communication volume, coefficient growth, ready-job width,
and wall/CPU scaling decide it.

## Stage 1 pressure families

The active manifest contains three sector-complete unit-mass families:

| loops | `K` | ordinary sources | Vakint graph classes covered |
|---:|---:|---:|---:|
| 1 | 1 | 1 | 1 |
| 2 | 3 | 4 | 2 (sunset and pinch) |
| 3 | 6 | 9 | 5 (K4/Mercedes parent and four contractions) |

All eight graph classes must map through proved routing, symmetry, pinching, or
factorization semantics to these artifacts. Merely constructing the indicated
sources, closing one sampled seed, or fitting a finite recurrence table is not
the milestone.

Unit common mass reduces concrete coefficient work to `Q(d)` only after an
authenticated homogeneity specialization. For squared mass `s`, the installed
reducer restores a target-to-master coefficient by
`s^(sum(master) - sum(target))`. The specialization does not reduce the index
dimension or establish closure by itself.

Four- through six-loop closure, dedicated high-loop reconstruction, and
extreme parallel scaling belong to Stage 2 and must not start without explicit
new guidance. Stage 1 algorithms nevertheless remain topology- and
loop-count-generic and share the same proof and publication path.
