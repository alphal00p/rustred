# Explicit guarded saved-rule diagnostics

`owner-guarded-apply` inspects one explicitly selected saved candidate per query
using the native guarded one-hop visitor. It does not select the first applicable
rule, generate rules, establish integer feasibility, insert domains into the
shared queue, or establish recursive coverage. This diagnostic is separate from
`owner-domain-match` and its shared traversal mode.

Exit **0** means all requested native inspections and report rendering finished.
Residual complements and RHS problems can still remain. Exit **4** includes
incomplete native work, cancellation and report limits; the saved report retains
the completed query/event prefix. Invalid invocation or I/O failures use the
existing CLI error categories. Neither exit code means that the input family is
solved.

## Input and invocation

Use an existing owner selection manifest and explicit query document:

```json
{
  "schema": "rustred.owner-guarded-rule-queries.json.v1",
  "queries": [{
    "id": "saved-candidate-diagonal",
    "owner": "11",
    "batch": 0,
    "rule": 239,
    "lower": [0, 0],
    "upper": [null, null],
    "max_numerator_rank": 10
  }]
}
```

This is a schema example, not an assertion that candidate 239 exists in a given
owner. The native loader authenticates the saved owner; the batch index and
saved rule ordinal select immutable data. Caller-supplied guards are rejected.
IDs are unique, nonempty strings of at most 128 UTF-8 bytes. Query JSON is bounded
to 1 MiB. The `max_numerator_rank` field is required: a nonnegative `u32` bounds
the actual query rank, while explicit `null` leaves it unbounded. Bounds use the
existing local coordinates: active `n_i = 1 + x_i`, inactive `n_i = -x_i`, and
rank is the sum of inactive `x_i`. `null` upper bounds mean infinity. Native
polynomial displays and image substitutions instead use physical index
coordinates. Entry rank is never silently clipped to a bundle's search rank.

```sh
python3 examples/python/apply_guarded_owner_rules.py \
  --executable /absolute/path/to/rustred \
  --manifest selection.json --owner-base /absolute/path/to/owners \
  --queries guarded-queries.json --output guarded-result.json \
  --events guarded-events.jsonl --stop-file guarded-stop
```

The Python driver only forwards paths and allowances, constrains native inner
pools to one, and replaces itself with the CLI. Caller affinity/address-space
limits and exit status survive; there is no elapsed deadline, compilation,
algebra or persisted license. Existing bound campaign drivers are unchanged.

## Native work and bounded report policy

Optional `--work-limits limits.json` accepts a strict JSON object of positive
integer overrides. Unknown/duplicate keys, null limits, and non-object sections
are rejected. Omitted values preserve native defaults. The file is at most
16 KiB. Supported keys are:

```json
{
  "max_predicates": 100000,
  "max_predicate_terms": 4000000,
  "max_events": 1000000,
  "applied": {
    "max_term_visits": 1000000,
    "max_shift_groups": 1000000,
    "max_boundary_cells": 100000,
    "max_sign_splits": 1000000,
    "max_native_operations": 4000000,
    "max_events": 1000000,
    "max_scratch_terms": 1000000,
    "max_scratch_boxes": 65536,
    "max_scratch_coordinate_cells": 2097152,
    "max_guard_univariate_degree": 16
  }
}
```

The effective policy is recorded. Outer guarded events and inner applied events
are distinct native allowances. This interface does not override native factor,
input/context, denominator, source-validity or exact algebra admission.

Application report defaults are 256 queries, 100,000 retained events, 16 MiB
payload allowance and 64 KiB per expression/debug display. Override with
`--max-queries` (ceiling 10,000), `--max-report-events` (ceiling 1,000,000),
`--max-report-bytes` (128 KiB through 256 MiB) and `--max-expression-bytes`
(1 byte through 16 MiB). Report bytes are a conservative cumulative JSON payload
charge, including a fixed 128 KiB metadata/error reserve and failed display
reservations, not process RSS. Expressions are formatted through a cancelling
bounded writer before extending strings; JSON escaping has a sixfold admission
reserve. The final compact JSON encoder is bounded too. A large expression is
not silently truncated into a purported complete predicate. On refusal the
diagnostic stops and preserves its truthful incomplete prefix.

## Reading the output

Each query retains one guarded-domain definition, referenced by its events:
saved case kind, exact clipped source box/rank, zero equalities, each whole
`NotAllZero` excluded conjunction, all source nonzero conditions, and **every
original denominator**, including denominators of zero or cancelled RHS terms.
Native expression text is display-only, not a round-trip artifact codec or an
independent applicability authority.

Incoming complements and candidate-rejection residuals remain explicit. They
are not asserted disjoint or nonempty, and are not reached missing-rule claims.
The complement is the requested box/rank minus the attached guarded domain.

A successor retains its exact source boundary cell and source domain reference,
target box/rank, and inverse physical shift as decimal strings. Interpret it as
`source_n = child_m + argument_shift`, including **all** source predicates and
the inherited source rank constraint. The target box alone is not the guarded
image. A conditional coefficient adds its nonzero condition; it does not prove
that an edge is reached. Coefficient displays are in the native restricted
source coordinates.

Successors remain provisional until the visitor returns successfully and emits
`guarded_rule_finished` with no problems. Even then only the conditional own-rule
one-hop identity is inspected: complements, feasibility, first-priority
execution and recursive dependencies remain outside this result. Native
per-original-term validity/descent checks precede equal-shift cancellation.

Native `stats.events` includes a last consumer-rejected callback, whereas
`retained_events` counts only fully rendered events. Progress contains compact
scalars, not duplicated guard/event arrays. Cancellation is cooperative at the
existing native boundaries and during display writes.
