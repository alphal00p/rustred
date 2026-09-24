# Bounded input admission for broader regional sharing

## Purpose and unchanged contract

The completed four-owner A12 experiment made shared Apply/Route regions worth
testing more broadly: traversal decreased from 22.388 to 11.720 seconds, with
the original entry set and every descendant retained. These are single-pair
measurements, not a full-family prediction. See the
[cover experiments](finite_cover_pilot_2026-09-24.md).

The same input construction over the complete saved manifest needs 57,621
queries: 339 Apply anchors, 57,215 Route anchors, and 67 original entry regions.
The roughly 16.14 MB compact JSON cannot pass the previous 10,000-query/1 MiB
admission. This is an input barrier, not evidence of missing IBP relations.
The full finite objective remains A≤24, R≤15, A−R≥9 over all 67 owners; anchors
are additional obligations, not a replacement or a descendant cutoff.

## Generic implementation

`OwnerDomainMatchRequest.max_query_bytes` and CLI `--max-query-bytes` allow an
explicit serialized-input byte budget. `max_queries` / `--max-queries` now accept
any positive representable count rather than imposing a second 10,000 ceiling.
Defaults remain **256 queries and 1 MiB**. Independent native work, domain,
event, and process-memory limits remain unchanged. Matching and recursive walking
use the same validation. Guarded-display and finite-entry admission retain their
previous fixed limits; they are separate interfaces.

The CLI bounds the file read; the library checks actual UTF-8 byte length before
JSON parsing. All query records are validated before native owner loading. The
typed query vector reserves the admitted actual row count, never the requested
maximum. Input text, the JSON tree, and typed records coexist during parsing:
the byte allowance is **not** an RSS bound. Existing Python CLI steering forwards
both allowances; this does not add a new native PyO3 domain-walking interface.
Progress and final results retain `requested_max_queries` and
`requested_max_query_bytes`.

The optional initial-overlap index now filters Route entries before its own
count/storage admission. Route entries cannot use that index, but remain in the
work queue and responsibility ledger. Every initial Apply descriptor remains
in the index's exclusion-membership set, including entries whose native summary
cannot provide an anchor. Original IDs and relative ordering are unchanged.
This leaves 406 relevant entries for the planned full input without increasing
the existing 4,096-entry/2 MiB limits.

Counting and construction both observe cancellation. Capacity, allocation, or
cancellation failures discard a partial index and fall back to inspecting the
original domains. Build diagnostics distinguish incomplete counting, no eligible
Apply entries, no usable anchors, and each fallback reason. They report logical
payload charge, not allocation size or RSS, and confer no coverage authority.
Ordered execution uses global initial IDs; OwnerBatched uses per-bucket indices
and local IDs. Reports survive the driver's consumption of the actual indices.
Buckets discovered later are explicitly `not_built`, not silently successful.

## Validation and next decision

Focused regressions cover exact byte/count boundaries, input beyond both old
limits, malformed final records before owner loading, large requested caps with
small actual input, cancellation, noncontiguous anchor IDs, all-Route inputs,
unusable Apply membership, actual logical payload sizing, and deterministic
allocation-failure fallback. Runtime integration tests cover both publication
policies and progress/final metadata. Independent implementation and mathematical
source reviews pass. The release application/CLI unit gate passes **562 tests,
zero failures, one existing ignored**; Python owner-query/steering checks pass
**38 tests**. The unchanged core retains its previous 2,809-test release result;
it was not rerun for this application-only slice. The first compile caught a
missed shared-parser adapter in finite-entry admission; passing an explicit
1 MiB preserves that interface's prior policy, and the rerun passes. Gate logs
are under `TMP/query-admission-release.KXf686/`.

The release CLI build passes (frozen executable SHA256
`254d7e1eefaacf0f20d76ce641836df62d47ff7daef7d45c5be0037530da9e8b`).
One unchanged-input A11 canary completes with 37,436 scheduled responsibilities,
27,806 native inspections and 695,918 events, with no frontiers, errors, or pending
work. After checking and removing only the new diagnostics, every prior
non-timing result field and attempted native-work counter agrees with the frozen
baseline. All four support counters remain unchanged: 402,205 same-support,
110,521 strict-subset, zero unsupported and zero conditional-unsupported edges.
The original structural digest remains
`bbc72262370c2b28d59307bfbb031a9be6271d4eccb6cf509749f512a240e27a`.

The canary reports an active index with four retained Apply descriptors and
four anchors, using 608 logical bytes per eligible entry at this arity. This
would charge 246,848 logical bytes for the planned 406 entries, not an RSS
estimate or evidence that the larger run has been admitted. No timed speedup
claim is made for this input-admission change. Raw receipt and comparison:
`TMP/query-admission-canary.7Omrqd/run/`.
Independent raw review also passes: all 37,436 domain records, responsibility
completion, worker/escrow drains, semantic counters and unchanged input identities
agree. Its local report is `TMP/query-admission-canary.7Omrqd/INDEPENDENT_AUDIT.md`.

This change does not accelerate algebra, regenerate saved rules, change rule
ordering, or establish the complete envelope. After validation, the next native
experiment is a bounded all-owner same-required-scope comparison, charging all
cold loading, anchor inspection, admission, output, and escaping descendants.
Only measured amortization justifies a subsequent full-envelope attempt. Extra
anchor gaps must be distinguished from failures on actual required entries.

The bounded all-owner input was prepared and independently audited:
67 required A≤12/R≤3/D≥9 regions contain 2,123,560 labelled tuples. Adding the
unchanged anchors gives 57,621 input regions and 999,539,725 distinct labelled
starting tuples, including all 1,770,085 required tuples outside P13. Compact
files are 18,353 and 16,139,743 bytes. Both controls request 57,621
queries and 32 MiB, 50 total compute workers and 450/500 GB soft/hard memory
supervision, with no elapsed deadline. Equal diagnostic stop gates are 375,000
scheduled domains, 250,000 returned inspections, or 15,000,000 committed events;
unchanged hard work caps remain in place. Stopped controls cannot establish a
completed-work speed comparison or condemn the underlying algorithm. Local input
and independent count/geometry/hash audit: `TMP/all67-regional-a12.wXo3pZ/`.

Both controls have now run and stopped cooperatively at the scheduled-work gate.
The anchored input is genuinely admitted: all 57,621 records were examined, and
the index retained all 406 Apply descriptors/usable anchors with a 246,848-byte
logical charge. Both prefixes have zero observed frontiers, but retain pending
descendants and cannot support a completion-speed comparison. This confirms the
input/index mechanism, not amortization across all owners. See the
[measurements and independent interpretation](all_owner_regional_sharing_2026-09-24.md).
