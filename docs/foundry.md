# Rule foundry

## Status

The first two narrow foundry capabilities are now live. The anchored boundary
specializes a chronological `ParametricRelation` slice at one concrete integer
point. The sector-interior boundary instead eliminates that slice directly
over the authenticated Symbolica field `K(n)`, orders shifts hardest-first,
and returns one parametric recurrence only after exact symbolic source replay,
uniform strict-descent proofs, all required retained pivot/denominator guards,
and agreement with an independently derived anchored rule. Its domain is the
largest representable box on which every source shift stays in the anchor's
fixed sector. Generated tadpole and caller-selected two-loop sunset spans, plus
a genuine two-source ordinary-IBP/LI elimination, exercise this path.

This is not yet a closing foundry. The API does not authenticate that its
caller-supplied relation slice is a fresh complete source set, split guard-zero
exceptional loci, feed back proper subsectors, select terminals, publish a
closed artifact, or reduce an integral. The
[project goal](../GOAL.md) is the authority whenever this design and the
implementation frontier differ.

The foundry will be an offline service. Ordinary Vakint evaluation will load
precomputed closed artifacts; it will not rerun the search. The implementation
must remain generic in topology and loop count. A specialized high-throughput
lane may be selected only from proved family properties such as vacuum
kinematics, common nonzero mass, and valid unit-mass homogeneity.

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

A closed artifact records only durable semantics needed to load and apply it:

- schema/algorithm and Symbolica identities;
- the canonical family, routing, convention, variable-order, source-set, and
  ordering fingerprints;
- guarded rules, strict-descent evidence, selected terminals, and stable
  master keys;
- zero, symmetry, factorization, and lower-sector dependencies;
- source-row provenance and exact replay material; and
- deterministic dependency and integrity metadata.

Untrusted bytes are bounded and authenticated once at load/installation. A
sealed in-process owner then carries those invariants; hot-path functions do
not repeat schema round trips, full hashes, or whole-artifact replay. Atomic
publication prevents a partial workspace from appearing under the final
artifact name.

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

## Six-loop pressure lane

For a complete six-loop vacuum family, `K = 21` and raw generation yields 36
ordinary IBPs. Unit common mass can reduce concrete coefficient work to
`Q(d)` after an authenticated homogeneity specialization, but it does not
reduce the 21-dimensional index domain or prove any sector closed. The fast
lane must be selected from sealed semantic evidence, never a topology name or
`loop_count == 6` branch, and must share the generic proof and publication
path.

The scientific milestone is a frozen, replayable topology-domain manifest in
which every entry maps to a closed artifact. Merely constructing 36 sources,
closing one sampled seed, or fitting a finite recurrence table is not that
milestone.
