# Numerator-symmetry assistance for finite master refinement

## Scope

Add an opt-in, loop-count-generic provider of exact homogeneous integral
identities to the existing finite terminal-refinement machinery. This is not an
IBP generator, numerical evaluation, or a claim of an independent/minimal master
basis. Existing published keys stay fixed; an eliminated key becomes an exact
dependency in the retained terminal convention.

The motivating P102 identity is a regression oracle only. Neither discovery,
routing nor elimination may dispatch on P names or topology names.

## Existing facilities and the actual gap

`TerminalCircuitEquations` already constructs finite rows `I - T(I) = 0` for
arbitrary numerator powers. It uses verified momentum maps and the shared
Symbolica-backed `sector::symmetry::integral_transport` service, coalesces exact
coefficients, and preserves every offspring as an unresolved column. Its
discovery geometry is deliberately restricted to full-rank `L+1` active-line
unit circuits. Seven active lines at five loops are outside that lane.

`TerminalRelationSession::enable_assistance` binds an immutable provider to an
exact finite inventory. `step_with_provider` validates arity, coefficients and
nonzero conditions, normalizes rows and sends them through the same sparse exact
elimination as ordinary IBPs. Successful provider batches, queried keys and row
cursors are checkpointed. A provider failure leaves the key pending.

`master_reduction::assistance` composes circuit and saved-program providers.
`inherited` freezes equation authority when exporting a partial search, and
`collection::inherit_with_policy` transports existing proved maps without
changing the source artifact. These are the integration seams, not a separate
eliminator or a custom polynomial implementation.

## Proposed native API

The engine supplies a sibling `TerminalNumeratorSymmetryEquations` with:

- `prepare(Arc<IntegralFamily>, &BTreeSet<IntegralKey>, limits)`;
- `family_fingerprint()`;
- `equations(&IntegralKey) -> &[TerminalEquation]`;
- `statistics()`;
- exact witness/certificate accessors for the verified momentum maps and full
  source rows.

Preparation covers the union of raw and normalized retained terminals only.
Unknown lookup keys return no rows. Offspring do not become new terminal
declarations. Denominator masses and positive-power weights must be preserved
before a transformation is admitted as an automorphism of that weighted
integral. The complete numerator is transported, including affine denominator
constants; cancellations preserve masses and do not change loop count merely
because an edge is contracted.

Algebra is delegated to existing exact Symbolica coefficient and transport
services. Combinatorial proposal generation is not an independent CAS kernel.
Limits, if exposed, are caller-selected; no new arbitrary default solver cap is
introduced.

## Application and CLI wiring

Introduce an opt-in `numerator_symmetry_assistance` option and native
`--numerator-symmetry-assistance` flag. Publication/finalization reject a request
to run a new provider; they only transport proved identities. Initially this
provider is supported in the exact refinement lane, consistent with existing
assistance. A modular-circuit request with this flag returns an explicit input
error rather than silently dropping the provider or falling back to a different
strategy.

The provider binding includes the family fingerprint, ordered raw finite
inventory and normalization profile, using its own versioned recipe. Old flag-off
bindings and checkpoints remain unchanged. Source scope and published-refinement
identities acquire the new tag only when the option is enabled.

The report records the option, source authority, per-support discovery counts,
verified automorphisms, exact rows/terms and typed skip reasons. An inherited
authority record receives a default-false field so old publications remain
readable; validation authenticates the versioned binding instead of re-running
discovery over a potentially extended inventory.

The new provider also writes a content-addressed native witness sidecar. Cold
loading independently verifies each stored momentum map and reconstructs its
complete transported row and conditions, without repeating the proposal search.
Portable export and inherited collection capsules copy this sidecar. An
interrupted pre-preparation checkpoint may have no sidecar only while it has no
queried, queued or accepted provider equations. Published numerator-symmetry
authority requires the replayable certificate.

When `--saved-rule-assistance` is also enabled, each complete symmetry identity
is passed through the existing coefficient-carrying routed application before
finite elimination. Explicit finite-point admission is derived from the exact
source row; it does not enlarge the user's guaranteed starting scope, and its
descendants are not clipped. Coefficients and applicability guards are retained.
An unavailable saved application is recorded with a typed failure diagnostic;
the original exact symmetry row is retained with unresolved auxiliary offspring,
never substituted with a fabricated master or an incomplete right-hand side.
This path inherits the supplied candidate programs' existing authority rather
than presenting their consequences as newly regenerated ordinary IBPs.

## Provider-only measurements

Counting new relations must exclude ordinary IBP discovery and collection
diagonal identities. A standalone experiment prepares the same finite baseline
terminal maps, drains only the pending symmetry-provider rows, and compares
exact terminal coefficients before/after while asserting zero newly consumed
ordinary source rows. It must not mark the unsearched ordinary inventory
exhausted merely because the finite symmetry inventory is drained.

Entirely scalar retained bases may need explicitly chosen finite numerator
neighbourhood probes. Their source identities are reduced through existing
programs into the pinned old terminal convention before elimination. R1/R2
probes constitute a declared finite experiment, not unrestricted orbit coverage.

## Acceptance

1. Exact P102 regression with independently checked native signed-index and
   graph/routing witnesses; unit-mass and mass-restored normalization checks.
2. Unequal-mass and unequal-power controls reject the equal-power permutation
   argument. Non-preserving transformations must not be silently reused.
3. A small fixture demonstrates assistance produces finite homogeneous rows,
   retains every auxiliary offspring, coalesces repeated keys, and preserves
   exact conditions through checkpoint round trips.
4. Flag-off preparation does no numerator-symmetry work and retains historical
   binding semantics.
5. Resume with the same provider succeeds; changing provider policy requires a
   new round. Partial finalization retains the provider's frozen authority and
   all previously proved relations, without searching again.
6. Publication-only, modular-with-assistance, malformed map, wrong family and
   mutated inherited binding cases fail explicitly.
7. Run an isolated refinement from the pinned five-loop retained publication.
   Never write to production campaign directories. Report before/after retained
   basis size and exact dependency certificates. No numerical-master claims.

## Using the exact provider

The feature is isolated on `codex/numerator-symmetry-refinement`. With that
branch's Python steering and optimized executable, first stop an active
refinement and finalize its durable exact maps. Then start a relations-only
round, for example:

```bash
python -B examples/python/saved_campaign.py finalize-refinement \
  --campaign "$RUSTRED_CAMPAIGN" --executable "$RUSTRED_BIN"

python -B examples/python/saved_campaign.py refine \
  --campaign "$RUSTRED_CAMPAIGN" --executable "$RUSTRED_BIN" \
  --fresh-search --refinement-strategy exact \
  --seed-depth 2 --containing-sector-depth 0 \
  --numerator-symmetry-assistance --no-saved-rule-assistance \
  --workers 128 --cpus 64-191
```

CPU placement and resource settings are operational choices, not mathematical
search limits. The fresh-search option is a **new-round** action. Omit it when
resuming that same round; the Python adapter restores its bound fresh mode and
provider preferences. Do not finalize a running native process. Finalization
retains only durable proved maps, not in-flight candidates or unfinished rows.

The two assistance flags are separate deliberately. Numerator assistance alone
contributes exact momentum-change source identities; saved-rule assistance
additionally applies existing supplied programs to their complete rows before
finite elimination. It does not generate new parametric IBPs. Both require the
exact refinement backend at present. A modular request is rejected explicitly.
The ordinary default does no numerator-symmetry discovery.

The command above chooses the measured cheap direct-provider lane. It found
31 extra finite dependencies on the pinned five-loop set (904 -> 873), without
saved-rule application. Full saved-rule projection is an advanced opt-in:
an initial lexicographic, high-rank-first assay was stopped after eight hours
inside its first equation. Its test/frontend bodies were opt-0, so that failed
run is not a production release benchmark. Do not promise that adding
`--saved-rule-assistance` is inexpensive. The ordinary `(2,0)` source search
also adds work not included in the direct-provider timing.

For independent optimized experiments, `prepare_saved_integral_equations`
returns a reusable `PreparedSavedIntegralEquations`. Its `reduce_equation`
method uses exactly the existing complete coefficient reducer, contexts,
payload/overlay authentication and guards; `equations` returns directly
applicable saved identities. An exact source row supplies finite roots only
for identity application, not a new guarantee for arbitrary physics inputs.
The caller remains responsible for the validity of a proposed input identity.

The imported previous maps and guards remain authoritative, while old auxiliary
rows, source cursors, modular images/candidates and depth minima are discarded.
The native witness sidecar and source provenance are retained across checkpoint,
partial-finalization and collection-export boundaries. Unsupported cross-family
retained outputs fail explicitly rather than disappearing from the imported
map. The original source artifact remains immutable.

## Measured lower-loop limitation

See [the reproducible lower-loop study](numerator_symmetry_lower_loop_results_2026-10-21.md).
The production provider's input is the raw-plus-normalized retained inventory.
The study additionally supplied declared degree-one/two numerator neighborhoods
through the public Rust provider API. Those extra probes are **not** implicitly
enabled by the CLI flag. Direct raw-only discovery changed neither lower-loop
basis. Explicit degree-one probes recovered one global four-loop dependency;
degree two gave no further gain. The already five-element K6 basis stayed fixed.
This is useful exact evidence, not a minimal-basis or unrestricted-group claim.
