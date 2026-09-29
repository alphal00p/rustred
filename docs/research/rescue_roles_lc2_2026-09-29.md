# LC2 rescue integration: immutable required-query scope

The rescued walk now distinguishes scope from query naming. An original
`rustred.owner-domain-queries.json.v2` document may carry:

```json
"query_roles": {
  "required": ["requested-integral-domain", "convenience-domain"],
  "auxiliary": ["coverage-helper"]
}
```

This must be a complete, disjoint exact-ID partition. Duplicate, missing and
unknown IDs are rejected, as are duplicate role declaration keys. Without a
declaration every query is required; rescue refuses to run without an explicit
declaration. A required query containing `anchor` in its name stays required;
an auxiliary query need not follow any naming convention.

The original query JSON already participates in the immutable checkpoint
request binding. Changing roles therefore invalidates a resume, just like
changing a query's geometry. This is intentionally **not** a migration of old
untyped LC2 checkpoints. Those campaigns remain untouched; rescue with the new
scope declaration starts fresh. No general compatibility importer is provided.

The tracked five-loop D9/D10 input retains exactly the same 183 query objects:
116 required queries (82 connected physical, 18 factorized physical and 16
non-entry convenience queries) plus 67 auxiliary helpers. Only the role
declaration and its receipt's byte count/hash changed. The offline input
planner derives roles from its root/helper objects, not from name patterns;
the independent checker reconstructs that partition from the receipt.

## Amendment and closure invariants

- Every amendment remains append-only, byte-digest chained from the original
  request, and replayed in order. New rows are auxiliary by protocol.
- `supersede` may name only earlier auxiliary queries. It cannot remove or
  relabel any original required query.
- Frontier ancestry is quarantined for fresh lookups. Accepted historical
  prefixes keep their recorded worker-view epoch and dependencies for replay.
- Closure is checked per required query, by exact containment in a recursively
  closed input root. Required queries that share a superseded helper root do
  not disappear. A replacement covering only one of two such queries leaves
  the other explicitly incomplete.
- Open auxiliary work does not obstruct a scoped certificate when every
  required query has independently verified closed coverage. This is not an
  unrestricted family-closure claim.

The former `--helper-pattern` authority is removed. Rescue offers
`--helper-id-prefix` solely to name newly appended auxiliary queries; the
offline verifier and Python audit have no name-based scope switch. The
published rescue summary separately counts all input queries and required
queries certified/uncovered. Production steering uses schema v5; the
old name-based rescue v4 is deliberately not migrated.

## Integration boundary and validation

The rescue branch was merged onto the LC2 baseline without rewriting its
history. The previously local Symbolica heap-power patch was preserved in the
worktree submodule stash; the submodule now uses plain pinned LC2 upstream.
No computer algebra primitive was introduced or changed.

G2 integration must filter fresh lending through the existing quarantine and
worker-view epoch hooks while preserving already accepted pin replay. The
combined G2/rescue activation-order gates remain an integrator responsibility;
this scope patch alone is not evidence those gates passed.

Focused tests cover malformed declarations, misleading names, absent scope,
required-query supersede refusal, request-scope tampering, shared-root partial
replacement, actual amended checkpoint resumes, Python supervision/auditing,
staging and planner/checker consistency. The separate native release test was
cancelled while still waiting for the shared build-memory lock, before any
compiler/test child existed (`TMP/codex-integration/rescue-roles-release-1`).
Native execution is **pending** the integrator's combined G2/rescue release
build, to avoid a duplicate expensive compilation. No production
campaign was started, stopped, resumed or edited during this integration.

The focused frontend matrix ran 161 tests: 160 passed, one optional full
skeleton-enumeration test gated by `RUSTRED_SLOW_TESTS=1` was skipped; that
test does not require a native executable.
It includes `test_owner_query_roles`, `test_frontier_rescue`,
`test_audit_owner_domain_walk`, `test_stage_saved_owner_campaign`,
`test_plan_renormalization_entry_queries`, `test_ram_guard_frontier_policy`,
`test_shared_owner_campaign` and `test_campaign_monitor`. The independent
entry checker passed 7,424 membership probes with zero disagreements and
verified the same 271,990,954,170 requested lattice points. These are input
and frontend checks, not evidence of five-loop closure.
