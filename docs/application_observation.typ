= Optional application observations

`CandidateRoutedApplicationObserver` provides optional, per-call observations
of the existing Route/Apply application. It does not select rules, schedule
mathematical work, change admission, or authenticate new reduction identities.
The ordinary methods retain their signatures and perform no observation clocks,
metadata allocation, formatting, or callback locking.

Use `reduce_linear_combination_with_observation` or its routed-entry equivalent.
The `with_cache_and_observation` methods also accept the ordinary backing and
completed-node capture callbacks. All four use the existing traversal and
arithmetic. The saved-application facade forwards the same observer through
terminal substitutions. The existing basic `Copy` snapshot and version-one JSON
progress APIs retain their contracts.

== Callback ownership and failure

The observer borrows one callback for the enclosing call. Each envelope borrows
only immutable key/path metadata for the callback's duration; it contains no
coefficient or Atom payload. A consumer should copy bounded metadata into a
nonblocking sink and render on its own thread. It must not reenter the engine:
an embedding application can hold its own mutex while Native owns its state.

Returning `Detach` disables subsequent delivery, including reuse of that
observer. A returned telemetry error or an unwinding callback panic records an
observer fault and disables delivery. Neither replaces the native result or its
typed resource cause. Aborting panics cannot be intercepted. The existing
legacy snapshot callbacks still unwind after normal pin cleanup, as documented;
their public behavior has not been silently changed.

The optional cancellation flag remains cooperative cancellation of mathematical
work. Detaching a UI or pausing its display must not set it. Ambient shared
resource budgets retain their existing checks and typed errors. No observation
count is a new reduction quota.

== Metric scopes

An operational identity includes Route versus Apply and, for Apply, its owner.
`unique_nodes` counts distinct identities entered in one native request.
`completed_unique_operational_nodes` counts their first successful ready result;
`completion_attempts` also counts memo reuse and eviction recomputation. These
request-local counts remain distinct from retained cache entries and from
campaign trace counts, whose completion means only local expansion.

Ready results in Preserve mode can contain unresolved integrals. Completion is
not a family-closure, master-minimality, or persistence-durability claim.
`completed_roots` increments after the root's weighted contribution is collected.
`requested_roots` is known after exact input coalescing. Native `output_terms`
counts the actual current output map, not an expanded pending expression.
Saved terminal-output terms have their own terminal-stage field.

`pending_frames` includes Enter and Combine frames. Frontier samples separately
count distinct queued Enter identities and canonical integrals. Their frame
grant and exactness flag are explicit; an incomplete sample gives lower bounds.
It does not describe undiscovered future dependencies. Cache coefficient
censuses include guards and exclude scratch, allocator overhead, and RSS.
Factorized censuses are conservative, and observation never materializes them.

Canonical key measures are numerator rank R, positive-power sum A, signed
power difference D = A - R, positive support count, and dot excess A - support.
They are not the physical input expression's rank. Componentwise maxima need
not belong to one key. None of these scalars is universally monotone under
pinches, transports, or weighted owner orders.

The local-descent counter counts surviving child edges whose persisted order
was checked by the evaluator. It is not a count of order-comparison attempts or
a global remaining-work certificate. The authenticated 640-terminal runtime
explicitly makes no global termination claim. Any displayed ETA belongs to the
consumer, must identify its observed scope and heuristic method, and must remain
unknown without useful samples; discovered frontier work may continue growing.

== Activity and cycle diagnostics

Activity notices precede enclosing owner preparation, transport, rule evaluation,
coefficient combination, and root materialization/collection calls. Periodic
quantitative samples retain the existing interval/work gate; detailed path and
frontier sampling is bounded. No callback runs inside a CAS operation. A stale
notice means only that no newer boundary has been observed; it does not identify
internal GCD work or prove a dependency cycle.

A cycle event comes from the existing exact active-node check, before memory or
persistent cache hits. It reports the attempted identity and bounded ancestor
path/backedge before the unchanged typed cycle error. Inactive memo revisits,
Route-to-Apply steps, diamond sharing, and recomputation are legitimate. Cycle
checks remain active when observations and diagnostic count limits are disabled.

Native core success precedes saved terminal substitutions, and saved application
success can precede a caller's numerical evaluation. `observer.emit` accepts
caller-owned stage envelopes through the same callback; their explicit caller
phase and scope must not rewrite native status. The app projection
`saved_application_observation_document` extends the existing version-one JSON
shape with scoped detail and never serializes coefficients. Forwarded summaries
are labeled as the latest native request and retain their genuine native sample
age; later saved/caller notices do not refresh that timestamp.

== Stored expression scopes

The optional `stored_state` census reads fixed-size metadata maintained only for
advanced observation. `unstarted_roots` counts coalesced roots whose traversal has
not started; the current root is identified separately. `deferred_rule_maps` and
`deferred_child_integral_occurrences` count retained immediate child maps,
including the currently popped Combine map until combination finishes. Repeated
integrals in different maps or operational stages are separate occurrences.
`collected_output_integrals` is the exact current core output map length after
successful weighted additions, including cancellation during root collection.
These scopes can overlap and must not be summed into distinct remaining work.
There is no materialized globally expanded pending expression.

Every delivered native boundary, including activity notices, reads the current
O(1) census. Its sequence and native-summary age identify that observation.
Saved/caller forwarding retains the last census and its genuine native age;
it does not resample or refresh it. A new dispatch resets the census. Before
coalescing, and after an error drops the local roots/output maps, their counts
are unknown. Retained queued frames remain separately counted after an error;
the dropped active Combine map does not. Checked counter overflow omits the
affected count rather than limiting reduction or changing its result.

A caller can additionally supply `complexity.encoded_expression_bytes` and
`complexity.terms` for an already owned Atom at an explicitly labeled caller
phase. Encoded Atom bytes and stored summands are not Native coefficient byte
bounds, a globally distinct integral census, or process memory. Native never
formats, walks, expands, or materializes expressions to fill this field.
