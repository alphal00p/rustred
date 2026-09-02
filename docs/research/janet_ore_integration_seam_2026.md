# Janet/Ore proposal integration seam

Status: reviewed design boundary, 2026-09-02. This note defines how an
involutive search engine may enter RustRed's existing requested-domain,
modular-discovery, and exact-replay pipeline. It is not evidence of K6 closure
and does not authorize artifact publication.

## Decision

Janet/Ore completion may contribute only requested geometry and canonical
parent-lattice support. RustRed must regenerate the corresponding ordinary
source translations and independently rediscover, exactly replay, compile, and
admit any useful recurrence:

```text
Janet obligation
  -> requested domain + sorted parent-shift support
  -> deterministic support union by semantic domain
  -> requested-domain plan over the fresh exact uncovered partition
  -> trusted ordinary-source incidence expansion
  -> InitialParentSourceProposal
  -> modular scheduler
  -> canonical exact replay and owner compilation
  -> exact ledger admission
```

This reuses the authority-minimal boundary already implemented by
`OrdinarySourceIncidenceIndex::try_nominate_initial_parent_support` in
`completion/source_discovery/initial_parent_proposal.rs` and
`try_run_interior_replay_task_with_initial_parent_proposal` in
`completion/source_discovery/interior_replay/run.rs`.

The narrow campaign integration belongs beside
`campaign/run.rs::try_run_requested_phase`. Keep
`leader_walk::try_plan_requested_domains` geometry-only and retain proposal
support in a sidecar keyed by semantic domain. A requested-only adapter method
should:

1. validate the task in its plan and bind it to the current opaque ledger
   snapshot;
2. look up support by semantic domain, never by requested ordinal;
3. construct `InitialParentSourceProposal` through the adapter's trusted
   ordinary-source incidence index; and
4. call the existing assisted interior replay path.

The ordinary `ProbeCampaignAdapter::try_evaluate_task` remains the fallback for
external or autonomous requested domains without Janet support.

## Authority boundaries

- The involutive module may own an Ore ranking, sparse search rows, exact or
  modular coefficients, a Janet basis, a prolongation queue, standard-pair
  diagnostics, and search telemetry. None has owner or publication authority.
- The requested-domain planner owns intersection with the current exact
  uncovered partition and opaque geometry identity. It accepts no source row,
  coefficient, circuit, owner, terminal, or closure assertion.
- `OrdinarySourceIncidenceIndex` alone converts parent support into canonical
  `TranslatedSourceRequest` identities and binds them to the complete ordinary
  source barrier.
- Probe-local modular epochs discover support only. Canonical exact replay must
  regenerate all selected rows from `CompletedIbpSourceRows` before the owner
  compiler checks source provenance, guards, and strict descent.
- `CanonicalExactOwnerLedger::try_apply_owner` remains the only owner-cover
  mutation boundary. A Janet queue becoming empty, an involutivity diagnostic,
  or requested `PhaseCompleted` is never `CompilerClosed`.
- Artifact publication remains conditional on the live exact compiler closing
  and the existing durable seal and cold-load validation succeeding. Search
  chronology and Janet coefficients do not enter artifact bytes.

In particular, `OreRow`, `OreConsequence`, coefficients, sampled values,
guards, circuits, owners, terminal declarations, closure flags, and artifact
material must not cross this seam.

## Payload and provenance

The crossing shape should remain structurally authority-minimal. The following
is schematic rather than a prescribed public API:

```rust
struct InvolutiveRequestedProposal {
    domain: RequestedDomainSemanticKey,
    parent_support: Box<[IntegralShift]>,
    provenance: InvolutiveProposalProvenance,
    census: InvolutiveProposalCensus,
}

struct RequestedDomainSemanticKey {
    stable_scope_key: Box<str>,
    sector: Mask,
    point: Box<[u64]>,
    symbolic_axes: Box<[usize]>,
}
```

`parent_support` must be nonempty, in range, sorted, and duplicate-free.
Provenance should retain:

- proposal schema revision and algorithm revision;
- frozen Ore ranking/order and coordinate-chart identifiers;
- sector/orbit and authenticated symmetry-route identifiers;
- guard/localization branch identifier;
- Janet basis revision;
- a semantic obligation key containing the parent basis-row digest,
  nonmultiplicative axis, prolongation exponent or degree, and prospective
  leader;
- the declared ordinary-source-module digest and canonical support digest;
- blind-domain priority metrics and an origin such as `autonomous_janet`; and
- deterministic scalar resource counters.

These are diagnostic claims, not acceptance seals. The trusted campaign must
attach and verify the family and coefficient-context fingerprints, exact
completed-source identity and chronology, live ledger revision, and opaque
ledger snapshot. Process-local identities must not be serialized.

Reports may retain scalar counters and stable digests. Exact artifacts retain
only the regenerated source provenance admitted by the existing compiler.

## Same-domain support union and resource accounting

`campaign/run.rs::merge_requested_domains` currently deduplicates a domain by
its point and symbolic axes, and the requested planner rejects duplicate
domains. Two Janet obligations can therefore name the same domain while
nominating different parent shifts. Attaching support after the existing
deduplication would silently discard algebraically relevant proposals.

Aggregate Janet proposals first in a deterministic ordered map keyed by
`RequestedDomainSemanticKey`:

- preserve the first domain's established external/blind chronology;
- merge its sorted support slices into one sorted unique union;
- retain canonically ordered obligation digests as detached telemetry; and
- attach the same union to every residual task produced for that domain.

Preflight the complete union before allocation. Dedicated involutive proposal
limits should bound:

- proposed domains, origin records, origin bytes, and retained transcript
  bytes;
- raw and unique parent-support entries;
- raw and unique support coordinate cells;
- union comparisons or logical canonicalization work; and
- retained support bytes.

Then preflight downstream work using the existing source-discovery limits:

- `unique support entries * ordinary source-term occurrences` incidence visits;
- raw and unique translated requests and their coordinate cells; and
- source chronology and translated-row limits.

The scheduler must additionally charge the union with the target-unit bootstrap
against per-probe request/cell limits and aggregate epoch, materialized-source,
modular-entry, and exact-lift work. If one domain intersects multiple exact
parent boxes and the proposal is reconstructed for each residual task, charge
that work per task rather than amortizing it invisibly.

Every count uses checked arithmetic and every allocation is reserved only after
the relevant cap passes. Exhaustion is a typed resource stop with no partial
prefix and no ledger mutation; it never means that no recurrence exists.

## Revision and determinism rules

- Any exact owner mutation invalidates the requested plan and all attached
  domain/support sidecars. Rebuild them from the fresh exact partition.
- Bind a task and its proposal to the opaque ledger snapshot, not only the
  scalar revision. Delayed results must fail even if a mutation leaves the
  geometric cover structurally equal.
- Keep Janet-local epoch, basis revision, planner geometry identity, and ledger
  revision as distinct types and fields.
- Use the original requested pivot for `target_shift` after residual
  replanning. The residual lower endpoint must not translate the recurrence or
  double its fixed-coordinate displacement.
- Freeze the Ore ranking, coordinate chart, guard branch, and modular-prime
  portfolio before a proof run. Changing the order requires a fresh run or an
  explicit recertification boundary.
- Canonicalize obligations, support unions, prime results, and worker results
  independently of hash iteration and completion order. A fastest worker or
  fastest prime cannot decide the accepted support.
- A proposal from one localization branch, sector, authenticated symmetry
  route, family, context, or completed source chronology cannot be reused in
  another.
- Empty prolongation queues and finite-complement diagnostics remain research
  evidence until exact guard, descent, owner-cover, durable-codec, and cold-load
  gates all succeed.

## Minimum integration gate

1. A compile-time exhaustive-destructure test proves that the crossing payload
   contains only domain, parent support, provenance, and scalar census.
2. Withhold one known K3 or K6 rule. Janet-assisted replay must produce the same
   exact source provenance, guards, descent semantics, and `RuleCell` as the
   established exact baseline; no Janet coefficient survives.
3. Feed disjoint supports for the same domain in every relevant input and
   worker order. The canonical union, requested schedule, exact result, and
   report digest must be identical, with no support silently dropped.
4. Build a proposal and plan at revision `r`, mutate the ledger, and prove bind
   or apply rejects the stale work, including when the cover remains
   geometrically equal. Fresh replanning must succeed.
5. Exercise tight union, incidence, scheduler, and report caps. Each returns the
   expected typed stop, retains no partial prefix, and leaves the ledger
   unchanged.
6. Reject foreign family, context, completed chronology, order, chart, sector,
   symmetry route, and guard branch before modular work begins.
7. Replan a request near the integer carrier edge and prove that the original
   pivot shift survives residual intersection without overflow or doubled
   displacement.
8. Show that an empty Janet queue and requested `PhaseCompleted` cannot publish
   an artifact. Only the live exact ledger's `CompilerClosed` path may do so.

Passing this gate authorizes a proposal lane, not a K6 closure claim. Full K6
campaigns and independent artifact validation remain subsequent work.
