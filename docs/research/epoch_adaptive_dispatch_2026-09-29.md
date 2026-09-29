# Bounded adaptive Epoch dispatch

Implementation note, 2026-09-29. Integrated native adaptive tests and independent
source review have passed; matched performance qualification remains pending.
It is not part of the compatible Stage A stable executable.

The public policy contract is `--epoch-dispatch fifo|adaptive`. FIFO is the
unchanged default. Adaptive requires checkpoint-enabled rolling Epoch and is
bound on resume. It changes which Pending obligation is inspected next, not
the query inventory, algebraic pivot order, closure requirements, or admission.

The dispatcher retains at most 64 candidate IDs from its monotone Pending
cursor. It chooses the smallest estimated cost from 32 fixed buckets:
Apply/Route phase, four owner-cardinality bands, and four rank bands. Estimates
are integer EWMAs of observed native elapsed seconds, actual newly admitted
domains, and known reuse. The score is
`cost_us * (growth + 1) / (reuse + 1)`, compared by `u128` cross products.
Equal scores choose the lowest immutable ID. Non-finite/negative durations are
ignored; finite durations saturate at one hour and growth/reuse at one million
for scheduling only. Native receipts and obligations are not truncated.

The rolling controller supplies verified, successfully published entries in
dispatch-sequence order. Since P3 reports new domains for the whole cut, their
actual total is distributed by quotient/remainder across those entries as an
explicit bucket estimate; it is not claimed as exact per-parent provenance.

Every eighth Pending selection chooses the oldest candidate regardless of
score. Thus a retained candidate is selected within at most 512 Pending
selections. Existing retry and deferred priorities run before this policy and
are unchanged. All candidates remain Pending until selected and reserved;
none is dropped because it is estimated expensive. Hints for obligations
resolved by another valid publication can be discarded after checking the
authoritative ledger.

The optional authenticated scalar field `adaptive_dispatch` contains the
candidate buffer, all 32 observation buckets, and fairness position. Bounded
deserialization rejects a 65th candidate before allocating from an untrusted
length hint. Restore validates ordering, bounds, integer ranges, and every
Pending ID behind the cursor against the saved candidates. Missing state is
rejected for Adaptive; FIFO omits the field and retains the historical Dispatch
binary section layout. Restored unfinished jobs retain replay precedence and
are not replaced by newly chosen candidates. Fresh-only configuration cannot
reset a resumed observation history.

Executed native tests cover individual cost/growth/reuse priority effects,
deterministic ties, oldest-slot fairness, saturated arithmetic, malformed
state, candidate conservation, retry/deferred precedence, binary section
round-trip, authenticated CP6 replay/resave and real interruption/resume. All12
adaptive-named tests pass in the `f3f707af` application run; its separate
cold-reader failure and final targeted correction are accounted for in the
delivery ledger, not hidden by this narrower result. The final executable keeps
this adaptive implementation unchanged. A matched comparison must report
work volume and closure as well as elapsed time; this heuristic has no claimed
speedup yet.
