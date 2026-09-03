# Exact-lazy support certificates: independent viability audit — 2026-09-03

## Verdict

This is an independent audit of
[`k6_exact_lazy_support_certificate_design_2026-09-03.md`](k6_exact_lazy_support_certificate_design_2026-09-03.md)
against the current modular DAG, exact Janet/Ore implementation, artifact
format, and measured K6 failure surface.

**Proceed with a bounded falsifier. Do not grant production or artifact
authority to the current representation.** The central theorem is sound: for
an owned circuit in `Frac(Z[d,n])`, evaluation at a prime-field point where
every exact-leaf denominator and every explicit inverse operand is defined is
a partial ring homomorphism. A nonzero image therefore proves that the exact
rational function represented by that same circuit root is not identically
zero. One such image is sufficient for sound generic nonzeroness. A zero image
is always inconclusive.

This proves support only on the recorded generic localization. It does not
prove coefficient equality, exact zero, or nonvanishing at every parameter or
lattice point, and it cannot by itself authorize a universally applicable
rule or a closing K6 artifact.

## Mandatory correctness boundaries

### Localization is exact authority

Every certified leader and every denominator or inverse used by its derivation
must retain an exact guard descriptor. Leaf denominators and translated
denominators remain explicit. Creating an inverse requires a consumed exact or
certified-nonzero witness for its operand and records the operand's exact
numerator condition. At the lowering boundary those descriptors are
materialized and canonicalized through the existing Symbolica-backed guard
path. All exceptional guard strata still require independent coverage.

The finite-field witness is not an application guard. It proves only that the
guard polynomial is not identically zero.

### M1 modular traces remain scouts

The current `ModularFrozenNormalFormProblem` begins from a complete exact
`JanetDivisionEpoch`, imports only physical rows and guards, retains no source
provenance roots, returns no coefficient roots, and rolls back every derived
DAG suffix after a probe. Each lane also chooses reductions from its sampled
support. These properties are appropriate for proposal-only scouting but do
not become exact-lazy authority by voting or joining proposals.

An authoritative path needs one serial field-independent circuit row. It must
classify every affected root before choosing the greatest exactly supported
Janet-divisible term. Independent samples may accelerate classification, but
they may not make queue or support decisions independently.

### Inversion must consume evidence

The proposal-only raw DAG can currently construct an inverse of anything that
is not structurally zero, and local simplification can erase a dead invalid
inverse in an expression such as `0/(n-n)`. The authoritative layer must use a
different boundary: inversion consumes a `CertifiedNonzero` or exact-nonzero
proof and records its exact guard descriptor. Raw constructors remain private
and cannot publish a certificate.

### Exact zero remains exact

A nonzero finite-field image is a rigorous one-sided certificate. Any set of
zero images is not a zero certificate. A root whose scheduled images all
vanish must remain unresolved until Symbolica materializes and proves it zero,
or another admitted sample proves it nonzero. Queue exhaustion is forbidden
while any support root is unresolved.

## Representation and scaling risks

The present evaluator is recursive and rejects a depth above 256. K6 histories
already contain tens of thousands of normal-form steps, so both finite-field
evaluation and exact materialization need bounded iterative postorder
evaluators keyed by `(node, accumulated translation)`. Reachability compaction
does not shorten a linear expression history; balanced or n-ary nodes can be
benchmarked later.

Compaction must be a deterministic structural mark/copy operation, not a
second nonzero sample. It preserves exact node, leaf, and translation
structure, returns an authenticated old-to-new root map, rebinds every live
row/provenance/guard root and witness transactionally, and only then drops the
old arena generation. Certificates must retain the actual immutable arena
generation, rather than only the current pointer-like `DagOwner`. Once a
certificate exists, rollback below its root is forbidden.

The retained representation needs aggregate limits for nodes, edges, exact
leaf polynomial payload, translations, row roots, source-provenance roots,
guard descriptors, exact-zero fallbacks, materializer output and scratch,
compaction work, worker caches, and total RSS policy. Current per-probe defaults
are not an aggregate memory contract. Internal row entries should eventually
store raw references under one owner instead of cloning an owner `Arc` into
every coefficient handle.

Parallel workers may only evaluate immutable snapshots. DAG mutation, support
classification, queue mutation, compaction, and proof ordering remain serial.
Commit the lowest deterministic probe-schedule ordinal, never the first worker
to finish.

## Projective interaction

Projective fraction-free arithmetic and exact-lazy circuits are separate
hypotheses. Symbolica's exact polynomial GCD consumes materialized
polynomials; no public API was found for the GCD of opaque RustRed circuit
roots. A lazy `g=1` pseudo-reduction is exact and preserves structural target
cancellation, but delays primitive normalization. Compare three controlled
variants rather than assuming a combined win:

1. monic rational DAG;
2. projective `g=1`, normalized only at admission or epoch checkpoints; and
3. materialized exact-GCD control.

No modular GCD guess may divide an exact row without native Symbolica exact
verification. A polynomial-only circuit needs a type-level exclusion of
inverse/rational leaves, not a convention.

## Publication boundary

The present K6 seed path exports only final leaders and physical row support
as proposal geometry. This makes an in-memory exact-lazy seed falsifier
valuable without changing durable artifacts: a sealed derivation from
authenticated ordinary sources plus rigorous support certificates can remain
generation-only. It still cannot publish executable rules.

The retained membership proof should be a shared whole-consequence operation
DAG—such as authenticated `Source`, `Translate`, `Scale`, and `Add` nodes—rather
than a fully expanded sparse provenance map or a set of naked coefficient
roots. Final conversion consumes an opaque `ExactSupportEpochView` binding the
source/action owner, arity, revision, leaders, and certified sorted physical
support. Raw shifts never cross that boundary. Because the support proposal is
only a downstream search nomination, its algorithm revision must change when
this new derivation path is selected.

Current runtime artifacts contain exact rational rule/source coefficients and
exact polynomial guards, and cold loading regenerates exact relations.
Selected publishable cells must therefore be materialized and replayed
sequentially at the existing authority boundary. Generation arenas should be
dropped as soon as safe. Final lowering, exact source replay, encoding, cold
decoding, and reducer RSS/time are required gates; otherwise laziness merely
moves the original peak to publication.

Persisting lazy checkpoints is deferred. If later required, bytes must contain
topologically ordered nodes, canonical exact leaves and translations, root
tables, deterministic probe schedule data, and source ordinals under strict
preflight limits. A fresh process must rebuild action/source owners from
authenticated inputs and re-evaluate every certificate. A checksum or pointer
identity is never semantic replay.

## Smallest falsifiable implementation slice

1. Implement iterative, bounded finite-field evaluation and exact
   materialization for coefficient roots. Differential-test active and
   inactive translations and chains deeper than 4,096 against direct
   Symbolica evaluation.
2. Introduce opaque `SupportClass::{KnownZero, CertifiedNonzero, Unresolved}`.
   The nonzero constructor owns a root, re-evaluates it in the lowest usable
   deterministic probe, verifies all denominator/inverse conditions and the
   nonzero residue, and binds the immutable DAG generation, coefficient
   context, and Ore action. Authoritative inverse construction consumes this
   proof and emits a localization descriptor.
3. Add one retained `LazyOreConsequence` with sorted physical roots, a
   whole-consequence derivation root, and guard descriptors.
   Run one complete multi-step normal form built from the real canonical K=3
   ordinary sources against a frozen exact monic epoch. Selection is serial
   from classified support. Compact once, materialize row/provenance/guards in
   the differential harness, and compare exactly with the current rational
   normal form. Production seed conversion instead consumes only the sealed
   `ExactSupportEpochView`.

Only after this passes should a lazy Janet epoch or bounded completion loop be
implemented.

## Go/no-go gates

### K=3

- Exercise all four sunset ordinary sources and active/inactive charts.
- Commit no unresolved term.
- Prove every removed nonsyntactic coefficient exactly zero with Symbolica;
  replay a nonzero witness for every retained term.
- Match the exact path's greatest-reducible target sequence, divisor ordinals,
  support, leaders, and final remainder.
- Match the fully materialized physical row, source provenance, canonical
  guards, and regenerated-source replay at admission/checkpoints.
- Inject unlucky-zero and pole probes, a nonsyntactic exact zero, and an
  invalid inverse; all must defer or reject without changing support.
- Pass structural compaction replay and a depth-above-4,096 stress test.
- Worker counts 1, 2, and 4 must yield identical transcripts, chosen witness
  ordinals, and exact final content.

### K=6

Use the natural-order orbit-4 release baseline and corrected seven-variable
coefficient-cell accounting. Match exact trajectory checkpoints before
crediting a speedup. The established deep prefix is basis 88, revision 118,
4,097 attempted prolongations, and 44,168 normal-form steps, with a baseline
of 794.26 seconds and 1,770,948 KiB RSS. A performance go decision requires at
most 885,474 KiB RSS and no more than 794.26 seconds to the matched prefix,
with exact fallback below 25% and compaction below 20% of wall time.

Repeat the selected ordering `[5,3,4,2,0,1]` through its established prefix:
basis 100, revision 141, 5,232 attempts, and 139,945 normal-form steps. The
baseline is 2,489.84 seconds and 909,372 KiB RSS. Report the unrelated logical
divisor-visit limit separately.

The candidate must then advance beyond both old prefixes without merely
raising caps. Decisive acceptance remains queue exhaustion in all six K6
orbits, finite pure-power complement, no unresolved support, complete
exceptional-guard coverage, exact selected-cell lowering and source replay,
canonical artifact encoding/cold decoding, and reducer-ready output.

## Additional measured evidence

A frozen release executable with SHA-256
`8700d10972a82bec98812d3ccf326879c1c0b5992405f5fee2154bbc9bf1be7d`
ran natural-order orbit 4 with a deliberate 1,048,576-term consequence cap.
It stopped after 101.33 seconds at 433,156 KiB RSS, basis 83/revision 108,
3,579 completion attempts, 35,720 normal-form steps, and 152,382,806
historical logical divisor visits. The first rejected consequence contained
1,826,367 numerator-plus-denominator terms:

| Component | Numerator terms | Denominator terms | Coefficients |
|---|---:|---:|---:|
| physical row | 649,437 | 43,335 | 187 |
| source provenance | 1,007,770 | 125,825 | 615 |

The largest single coefficient contained 21,569 terms. Of the first 256
exactly tracked nonunit denominator instances, 131 reused an already confirmed
representative and 125 were distinct; the remaining 545 instances were
deliberately outside the bounded exact-tracking sample. The outcome shows that
provenance expansion is the largest component, physical-row expansion is also
substantial, and denominator payload is secondary. It supports the exact-lazy
falsifier as the primary next lane while retaining projective arithmetic as a
controlled comparison.
