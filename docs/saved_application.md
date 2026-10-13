# Applying a saved reduction

`rustred-app` can prepare an immutable saved publication, apply its existing
owners with exact coefficient back-substitution, and export a portable runtime
directory. Application performs no IBP generation, campaign walk or finite
master search. It returns symbolic terminal coefficients and applicability
conditions; it does not provide numerical five-loop master values.

The runtime artifact is optional external data. Do not commit or push it with
the code. Host or distribute the complete exported directory separately and
configure its path explicitly in the embedding application. Loading never
downloads an artifact or resumes its source campaign.

The implementation is in
[`saved_application`](../crates/rustred-app/src/application/saved_application/mod.rs).
The small native driver is
[`examples/rust/saved_application.rs`](../examples/rust/saved_application.rs).

## Publication and runtime boundaries

These public APIs are exported by `rustred_app`:

| API | Purpose |
| --- | --- |
| `saved_application_arity(path)` | Read the authenticated physical arity without importing owners. |
| `saved_application_domains::<N>(path)` | Read original query IDs, native geometry and empty-domain flags. |
| `prepare_saved_application::<N>(publication, options)` | Authenticate an explicit publication, check its native terminal substitutions and prepare owners/routes. |
| `export_saved_application::<N>(publication, destination, options)` | Prepare and write a self-contained runtime directory. The destination must not exist. |
| `load_saved_application::<N>(runtime, options)` | Load `application.json` and its runtime payloads. |
| `prepared.export_runtime(destination)` | Export an already prepared handle without changing its mathematical scope. |

For publication input, a directory resolves to `artifact.json`, never the
moving `latest.json` pointer. An explicit manifest path is also accepted.
Runtime loading accepts a directory or its `application.json` file. `N` is a
compiled storage capacity at least as large as the physical arity; integral
keys retain their physical coordinate count.

The five-loop integration uses the fixed THE_ONE generation-54 publication:

```text
campaigns/five-loop-rank-ladder-THE-ONE/master-reduction/scopes/
f7ee869008fccc6ed6cf2f4e3342c78d2b8607f59ac4d523dcae504595c1b0f8/artifact.json
```

Its selection contains 67 owners and 9,982 rule records. It retains 804 query
records and 939 raw terminal maps with 905 remaining family-qualified keys.
Native geometry proves 41 historical queries empty; the other 763 define the
executable union. Empty records remain in provenance and inventories.

Export checks every retained source terminal substitution, including its
conditions, through the native compact-map round trip. The package contains:

- The exact owner selection, verified-route definitions and recorded query union.
- Existing native owner/overlay/preference payloads, transported as standard
  gzip streams split into bounded 32 MiB chunks.
- Compact terminal substitutions using RustRed's existing Symbolica state and
  coefficient-table codec, plus complete native family definitions.
- Sizes, BLAKE3 digests, source identity and inherited assurance metadata.

It excludes campaign ledgers, refinement/search matrices and numerical masters.
Runtime loading verifies compressed chunks and the reconstructed native
payload digests, structural records, coefficient-frame indices and native
family bindings at its boundary. It retains owned encoded bytes, so first use
does not reread mutable files. Each requested owner then uses existing native
rule admission, preferred-program policy and overlay replay exactly once. It needs no source
campaign paths or decompression temporary files. Native Symbolica import is a
trusted-generated-data boundary; integrity checks do not upgrade saved rules
to independently replayed certificates or prove master independence.

## Exact entry scope

The reported maximum rank and power difference are summaries of the recorded
union. They do not authorize every integral below those maxima. For powers
`n`, negative-index degree is `R = sum(max(-n_i, 0))`, positive power is
`A = sum(max(n_i, 0))`, and signed power difference is `D = A - R`.
For a homogeneous product of `r` momentum scalar products the momentum degree
is `2r`. Lowering into massive inverse denominators can also produce terms
with explicit mass factors and smaller negative-index degree. Consequently
momentum degree, tensor rank and each lowered integral's `R` are reported
separately; GeV^10 does not mean scalar negative-index degree ten.

Each original region also fixes positive support, coordinate intervals and
correlated power bounds. Positive coordinates use `n_i - 1`; absent coordinates
use `-n_i`. `saved_application_domains` preserves those predicates and IDs;
`saved_application_boundary_witnesses` proposes representable boundary samples
and verifies them with the native admission service. Sampling is not exhaustive
enumeration or a new closure proof.

The prepared handle provides two explicit application paths:

- `reduce_linear_combination` checks every original key against the executable
  union before cancellation or cached reuse, including zero input coefficients.
- `reduce_routed_linear_combination` first uses installed, verified entry
  transports and admits every nonzero owner image to that same union. Exact
  certified-zero images can be removed with their conditions retained.
  Entry transport and downstream reduction share one operational budget.

`route_entry_to_owner` exposes the scope-checked transport without applying
IBPs. `routed_entry_is_admitted` probes the same union without formula
preparation: out-of-union images return `false`, while malformed keys, routing
and resource failures remain typed errors. Literal owners use identity transport. Missing routes, missing owners,
uncovered rules, invalid coefficients and cycles produce errors; they are not
converted into new terminal declarations. Descendants of an admitted request
follow saved rules and are not clipped to starting-domain bounds.

## Prepared Rust use

This example chooses a native-verified witness from the supplied package rather
than assuming that an arbitrary power tuple belongs to its scope:

```rust
use std::path::Path;
use rustred_app::{
    SavedApplicationOptions, load_saved_application,
    saved_application_boundary_witnesses, saved_application_domains,
};

fn apply_one(runtime: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let domain = saved_application_domains::<15>(runtime)?
        .into_iter()
        .find(|domain| !domain.is_empty)
        .ok_or("publication has no nonempty entry domain")?;
    let witness = saved_application_boundary_witnesses(&domain.region)?
        .into_iter()
        .next()
        .ok_or("domain produced no witness")?;

    let mut options = SavedApplicationOptions::default();
    // Explicit native-session allowance; not a change to coverage.
    options.reduction_limits.max_cached_coefficient_bytes =
        usize::try_from(32_u64 * 1024 * 1024 * 1024)?;
    // Make bytes the retained coefficient bound for this explicit session.
    options.reduction_limits.max_cached_coefficient_terms = usize::MAX;
    let mut prepared = load_saved_application::<15>(runtime, options)?;
    let one = prepared.family().coefficient_context().one();
    let result = prepared.reduce_linear_combination([
        (witness.integral, one),
    ])?;

    for (terminal, coefficient) in result.terms() {
        println!("{} {:?}: {coefficient}",
            terminal.family_fingerprint(), terminal.integral().powers());
    }
    println!("required nonzero conditions: {}",
        result.nonzero_conditions().len());
    println!("cache: {:?}", prepared.cache_statistics());
    prepared.clear_cache()?;
    Ok(())
}
```

Coefficients are native exact rational polynomials in the authenticated family
context. `SavedApplicationIntegral` always carries both a family fingerprint
and integral powers. `output_family` borrows its definition;
`output_family_shared` clones the existing `Arc` for another native service.
Do not reconstruct algebra from printed coefficient or family strings.

The result uses the stored unit-mass, common formal loop-measure convention.
The embedding frontend retains source mass homogeneity and any external
measure factors. Product display must not apply the original five-loop mass
or measure normalization a second time to each factor.

## Guards and product normalization

Every coefficient result includes sufficient generic-parameter nonzero
conditions. Original source conditions, denominator poles and selected
exception-branch witnesses are retained even when algebraic coefficients
cancel. Conditions describe applicability, not a maximal exceptional-parameter
classification. Consumers must preserve them instead of inferring validity
only from the final simplified denominators.

`prepare_terminal_products(VacuumProductLimits)` optionally prepares verified
products of the unchanged remaining terminal keys. `terminal_product` returns
a coefficient and sorted family-qualified lower-loop factors; repeated factors
remain repeated. `terminal_product_statistics` reports retained/factored and
numerator counts, lower-family count and first preparation time. Repeated
preparation reuses the immutable result. Unsupported or connected terminals
stay flat. This presentation step does not independently reduce lower factors
or alter the package's 905-key terminal identity.

`prepare_terminal_product_charts(VacuumProductChartLimits)` separately groups
negative-power terminals by source family and positive support. A
`terminal_product_chart` preserves the verified momentum map, including all
numerators. Its `project` method uses an explicit existing tensor-projector
callback only for cross-component couplings, then performs native exact scalar
lowering. Residual cross-component powers or foreign coefficients fail closed.
Chart statistics count prepared geometry, not completed tensor projections.
Identical settings reuse the chart plans; changed settings replace them only
after successful preparation. Component families are available through the
same `output_family` and `output_family_shared` accessors. Flat terminal rows,
entry scope and reduction caches remain unchanged.

For independent factor reduction and exact sum-of-products comparison, the core
`VacuumProductReducer` accepts an explicit `VacuumFactorProvider`. The provider
must use existing selected programs, report per-call work and preserve their
guards under the remaining limits. Same-loop declared terminals stop; verified
product recursion strictly lowers loop count while preserving its total.
Products are sorted, multiplied and coalesced with native Symbolica algebra.

Before requesting a lower program, the core reuses its native sector-zero
analyzer. Only a replayed `ProvedZero` certificate can remove a scaleless
factor, including polynomial numerators in that sector; an inconclusive test
still requires the provider. All sibling factors remain checked and their
guards retained even if one factor is zero. Decisions are memoized by exact
family and positive support under shared cache limits, with native work bounded
by `VacuumProductReductionLimits.zero` and `max_zero_analyses`. Zero-analysis,
support-cache and proved-zero counts are reported separately; `clear_cache`
clears both support decisions and factor expansions.

Existing candidate reducers provide
`reduce_unit_mass_guarded(target, remaining_limits)`. Closed artifacts provide
`Reducer::reduce_artifact_unit_mass_guarded(artifact, target, limits)` without
allocating a redundant receiver cache. Both return the existing typed
decomposition, nonzero conditions and per-call statistics. The instance closed
method additionally deducts its receiver's retained cache from the allowance.
These interfaces generate no IBPs and do not silently fall back to another CAS.

For a verified transport into an existing candidate program,
`reduce_transported_candidate_factor` retains one native memoization cache
across the transport's distinct endpoints. Shared subreductions are evaluated
once within that batch. The native cache and the retained weighted output and
guards share the caller's cache allowance; every endpoint consumes the same
rule/coalescing work budget. The scoped native cache is cleared at entry and
on success or error, while the outer product reducer can still cache the
completed factor result. No new rules or terminal identities are discovered.

## Operational budgets and reuse

`SavedApplicationOptions` keeps mathematical scope separate from operational
limits. Default `ReductionLimits` include a 1 GiB retained coefficient-byte
allowance and a 16-million retained coefficient-term allowance. Raising only
the byte allowance does not remove the independent term or work limits.
Input count, operational-node count, transport work, rule applications,
coalescing work and pending frames also have explicit bounds.

`cache_representation` defaults to `CandidateCacheRepresentation::Sparse`.
The opt-in `Factorized` setting uses the existing Symbolica-native factorized
denominator cache; returned coefficients remain ordinary exact coefficients.
Its retained-term accounting includes a conservative expanded-size estimate.
Factorized storage is not an assurance upgrade or a guaranteed speedup. Select
it before loading a prepared handle; use a separate handle for a matched
representation comparison.

One handle reuses the same immutable owner/route selection and its mutable
coefficient cache. `cache_statistics` reports that retained cache;
`clear_cache` releases reduction memoization while retaining preparation and
optional terminal-product presentation. A new owner selection or coefficient
context requires a new handle. Core per-call statistics and
`terminal_substitution_additions` together describe composition work.

`SavedApplicationOptions.cache_policy` selects native completed-result reuse.
The saved runtime defaults to `CandidateRoutedCachePolicy::EvictLeastRecentlyUsed`:
when a retained-entry, coefficient-term or coefficient-byte allowance is reached,
it discards older completed entries that are not needed by a pending combine or
the current output. Pending dependencies are reference-counted and protected;
recomputation uses the same exact rules, guards and per-request work allowances.
`FailOnLimit` selects the previous retention behavior and remains the default
for the lower-level `RoutedCandidateApplication` API. Neither policy changes
artifact bytes, admission, terminal identities or formula precedence.

`cache_work_statistics()` reports cumulative evicted entries/terms/bytes and
same-request recomputed nodes, including work in failed requests. Per-request
receipts expose the same fields under `statistics().cache`. Cache clearing
preserves cumulative work counters. Eviction is not an unlimited-memory promise:
a single oversized result or the protected active working set can still exceed
the explicit allowance. An oversized entry is rejected before discarding useful
old entries. Native arithmetic scratch space and the returned expression are
not the completed-result cache.

`owner_preparation_statistics()` distinguishes declared, prepared and failed
owners, preparation attempts and cumulative preparation time. Route presence
and transport lookup do not prepare formulas. A first-use preparation error is
cached and fails before any result is returned. Clearing coefficient memoization
preserves prepared owners and their immutable identity.

A cache-budget error is an operational failure, not proof of missing rules or
unsupported scope. A failed request can leave valid completed subresults in
the cache. Clear between independent cases when measuring separate resource
requirements. Retained coefficient-byte counters are not process RSS; owner
programs, family data, native arithmetic scratch space and output materialization
also consume memory.

Runtime loading defers per-owner formula preparation through the existing
native lazy coefficient-table decoder. Family/context preparation, zero
certificates and route verification remain eager. Full native formula validity
is checked on that owner's first use, before dispatch; loading a handle alone
does not assert that every untouched formula has been prepared or certified.
Source-publication preparation remains eager for offline export checks.

`preparation_workers` defaults to one and controls the existing deterministic
route-verification pool. It must be positive and pass the native core-budget
preflight. It does not parallelize formula application or widen coverage.
Measure cold loading, first application including any owner initialization,
and warm reuse separately; do not label a warm reduction as a cold start.

`preparation_statistics()` exposes disjoint cumulative wall durations for
metadata, terminal maps, owner payload read/decompression/authentication,
shared owner-context admission, route preparation and remaining assembly work, plus
the overall total. Source preparation includes native terminal verification
and compact-map checking in its terminal-map phase; runtime preparation includes
only compact-map import. Deferred owner initialization is reported separately
by `owner_preparation_statistics`. These receipts do not attribute process RSS
to phases.

## Native driver

Build the example with a runtime configuration supporting the physical arity:

```sh
nix develop --command cargo build --release --locked \
  -p rustred-app --example saved_application
```

Its argument forms are:

```text
saved_application inventory PUBLICATION_OR_RUNTIME
saved_application terminals RUNTIME
saved_application export PUBLICATION NEW_RUNTIME_DIRECTORY
saved_application suite RUNTIME INVENTORY_JSON [OFFSET] [COUNT] [REPEATS] [CACHE_MODE] [CACHE_BYTES] [CACHE_REPRESENTATION] [LIMITS_JSON]
saved_application compare-cache RUNTIME INVENTORY_JSON CASE_ID CACHE_BYTES [REFERENCE_BYTES] [LIMITS_JSON]
```

`inventory` writes JSON to standard output. `terminals` reports family-qualified
remaining keys once loaded, without generating equations. `suite` writes JSONL receipts and
checks exact coefficient/guard equality on repeated applications. Cache mode is
`independent` (default, clear between cases) or `shared`; representation is
`sparse` (default) or `factorized`. The driver's explicit `CACHE_BYTES` override
also removes its aggregate coefficient-term ceiling while preserving its other
limits. The Rust API does not make that second change automatically.
An optional JSON object overrides named operational counts without recompiling,
for example `{"max_transport_endpoints":40000000,"preparation_workers":8}`.
The same object accepts `"cache_policy":"evict-lru"` (default) or
`"cache_policy":"fail-on-limit"`. Unknown names, rank/scope keys and invalid
values are rejected. Complete selected limits appear in
the preparation receipt. After a failed case the driver records the cache
census and clears partial memoization before the next case, preventing an
exhausted shared cache from manufacturing repeated unrelated failures.

`compare-cache` compares one inventory key with a fail-on-limit reference cache
(1 GiB by default) and a smaller evicting cache. It requires exact coefficient
and ordered-condition equality, first/warm agreement and actual eviction, and
checks every output against the declared remaining terminals. Other selected
limits are identical. Preparation and application are timed separately; the
two handles run serially in one process, so these are not independent cold
process measurements. It writes no artifacts and does not discover new rules.

Export to a new local directory outside tracked source. Keep that entire
directory together when hosting it. The current measurements and incomplete
acceptance items are recorded separately in [CODEX_PROGRESS.md](../CODEX_PROGRESS.md);
successful sampled reductions do not establish unrestricted family closure.
