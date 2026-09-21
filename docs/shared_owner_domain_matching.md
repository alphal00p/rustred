# Local parametric owner-domain matching

`owner-domain-match` classifies explicit coordinate boxes against the ordered
rules and terminals in saved owner programs. It cold-loads the selected immutable
owners once and uses the existing native guard machinery. This is a local
applicability operation, not an RHS expansion, source solve or closure campaign.

```sh
python examples/python/match_shared_owner_domains.py \
  --executable /path/to/gated/rustred-cli \
  --manifest selection.json --queries queries.json \
  --output result.json --events events.jsonl --stop-file stop
```

The equivalent Rust command is `rustred-cli owner-domain-match` with the same
arguments except `--executable`. The generic selection format is the same as
`routed-campaign` and `owner-domain-scan`. For literal local queries, a selection
may contain only the required saved owners and an empty
`initial_frontier_routes` array; native loading still validates family, context,
ordering and saved policy. A full routing selection remains usable, but adds
preparation work and does not make this operation traverse routes.

## Query input

```json
{
  "schema": "rustred.owner-domain-queries.json.v1",
  "queries": [
    {
      "id": "child-prefilter-r11",
      "owner": "10",
      "lower": [0, 11],
      "upper": [null, 11],
      "max_numerator_rank": 11
    }
  ]
}
```

This illustrative two-coordinate query is the ray `n0>=1, n1=-11` for a family
with a saved owner `10`. All arrays and masks must have the loaded family's
arity. Local coordinates are `n=1+x` on active axes and `n=-x` on inactive axes;
lower bounds are unsigned integers and a null upper bound means infinity.
Every query explicitly supplies its own rank cap: a nonnegative 32-bit integer,
or null for unbounded numerator rank. Rank is the sum of **inactive** local
coordinates, not positive dots. Descendant queries are not clipped to the
saved program's entry rank. IDs are unique and contain 1–128 UTF-8 bytes; input
is bounded to 1 MiB.

Boxes and a rank cap cannot encode arbitrary inherited native equalities,
nonzero guards or reachability. A query extracted from a conservative successor
scan remains a **prefilter** unless those other obligations are established
separately. In particular, an exact local gap does not establish that an earlier
source rule reaches it or justify treating it as a reached missing-rule frontier.
Searching an explicitly over-covering input can still be valid, but may do work
that the original source domain never needs.

## Result and exit status

The full atomic JSON result distinguishes `selected_rule`, `terminal`,
`exact_zero_sector`, `exact_gap`, `unresolved`, and `invalid_source_condition`
pieces. An undecidable earlier guard remains unresolved, not a fall-through gap.

- Exit 0 means `classification_complete=true`: all requested local domains have
  an exact classification. This **can include exact gaps or invalid conditions**.
- `all_queries_locally_applicable=true` additionally excludes gaps and invalid
  source conditions. Even then, RHS specialization/cancellation, descent and
  recursive successor coverage have not been established.
- Cancellation, an unresolved piece, resource exhaustion or another incomplete
  classification produces a nonzero exit (normally 4 for an incomplete result).
  The saved result retains the available prefix and error; no partial success
  is silently promoted to a complete classification.

The result always keeps `family_closure_claim=false`, `ibp_generation=false`
and `rhs_successors_expanded=false`. A completed empty domain may be vacuously
locally applicable; this is not evidence that the queried region is inhabited.

## Operational controls

The result and event destinations must be fresh paths; neither is overwritten.
The full result is atomically installed. Events contain compact scalar progress,
not retained piece arrays. A terminal-only dashboard shows classification counts;
`--no-progress` disables it. Without `--events`, JSON heartbeat events go to
stdout; the result still requires `--output`.

The stop file requests cancellation through the existing shared atomic flag;
native calls are not forcibly preempted. No elapsed deadline or extra worker
pool is introduced. Python replaces itself with the Rust process, inherits the
license without printing or persisting it, and sets native inner thread pools
to one. Direct CLI callers must also respect the native inner-pool contract.

`--max-queries` defaults to 256 (ceiling 10,000), and `--max-total-pieces` defaults
to 100,000 (ceiling 1,000,000 retained output pieces). Per-query counters can be
set explicitly with `--max-rules-per-query`, `--max-terminal-checks-per-query`,
`--max-predicates-per-query`, `--max-pieces-per-query`, `--max-cells-per-query`,
`--max-split-operations-per-query`, and `--max-coordinate-cells-per-query`.
Unspecified values retain native defaults. These are operational allowances,
not mathematical rank restrictions or hard RSS guarantees; native per-predicate
algebra limits remain unchanged. Raising a work cap never changes an unresolved
answer into a claim of applicability without actually completing that work.
