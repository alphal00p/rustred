= Ordered weighted reduction

The opt-in ordered scheduler applies the existing authenticated programs to a
global weighted frontier. It uses exact coefficients and BTreeMap storage; it
does not generate identities, change dispatch priority, rewrite artifacts, or
import a depth-first completed-result cache. Loop count is data rather than a
scheduler specialization.

== Selection and shared semantics

```rust
use rustred::reduction::ReductionStrategy;
use rustred_app::SavedApplicationOptions;

let options = SavedApplicationOptions {
    reduction_strategy: ReductionStrategy::Ordered,
    ..SavedApplicationOptions::default()
};
// An already prepared application also has set_reduction_strategy(...).
```

The default remains `DepthFirst`. Existing weighted reduction methods retain
their output and error types. Closed artifacts, candidate bundles and saved
owner/route applications have thin adapters to one generic scheduler. Rule
selection, specialization, canonicalization, transport and sufficient nonzero
conditions use the existing checked operations.

Every original root is admitted and its original coefficient poles retained
before cancellation. At each step equal canonical operational identities combine
exactly, and zero weights disappear before their descendants are expanded.
Applied conditions remain in the proof even when later weights cancel. Different
valid schedules may require different sufficient conditions. Unknown residuals
are classified after exact final combination; completion is not inferred from
termination alone.

== Phases and canonical identities

Saved applications process support sizes downward, draining routing before the
owners at a given size. Within an owner, the saved local order strictly decreases.
Proper pinches move to smaller support. Verified routing may expand a numerator
into several weighted endpoints; all of them must enter the common canonical
frontier. Route and Apply are distinct operational identities.

Closed/candidate artifacts use descending-support phases when their authenticated
order is support-primary. Other orders use one phase with their exact saved key
order. Every emitted nonterminal edge is checked against the scheduler position;
unsafe or non-descending transitions are errors. Existing complete factorization
is an opaque checked action with request-local state; this mode does not promise
cross-root grouping inside that action's dependent reductions.

== Observation and state ownership

The observer reports a fixed phase total, completed phases, current support and
owner, and current/peak operational frontier size. Separate counters describe
global merges and exact frontier versus terminal cancellations. Input and
intra-rule coalescence are not silently counted as global frontier cancellations.
Empty phases complete normally. Phase fraction is a structural measure rather
than elapsed-time percentage or an ETA. Independent requests have separate epochs.

Separate current/peak coefficient-monomial counters sum numerator and denominator
monomials of the resident rational frontier and terminal coefficients. Incremental
updates require no arithmetic, printing or frontier scans, and expose coefficient
swell even when the integral count is unchanged. This size proxy excludes the
active popped coefficient, guards, arithmetic temporaries, keys and byte storage.
Terminal counts precede saved terminal substitutions; process RSS remains the
memory measure.

The ordered engine does not read, update or clear depth-first completed-result
caches. Explicit persistent-cache attachment or capture in ordered mode is
unsupported. Switching back preserves the depth-first state. New phase-transfer
or completed-result caching is outside this implementation.

Exact field weights must be provided as weights rather than separated into
unrelated scalar-carrier calls. Application frontends are responsible for
collecting compatible family, mass, dimension and spectator conventions before
calling the weighted reducer. Distinct coefficient fields remain distinct.
