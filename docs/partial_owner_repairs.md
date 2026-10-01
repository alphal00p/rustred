# Source-derived partial owner repairs

A saved owner may have valid rules but miss a later descendant. RustRed can
search that actual case against the owner's original IBP sources, retain a
partial rule result, and add it without regenerating or changing existing
owners. This is not a declaration that the whole owner or campaign closes.

## Authority and scope

`BoundOwnerSearch::solve_domains_with_observer` returns a genuine partial-domain
result. Use the descendant's actual scope, which need not equal the original
entry-rank limit. For example, a rank-ten entry can produce a rank-eleven
descendant; clipping the latter to rank ten would hide a missing obligation.

Before installation, `replay_overlay_rules` checks the exact identity and guards
against regenerated original sources. `append_residual_free_domain_overlays`
refuses every unresolved finite residual. It does not declare new masters, and
it preserves the existing rule batches, terminal table and owner snapshot.
Rules are appended: already applicable old rules or terminals keep priority.

The partial result may be saved with `encode_generated_domain_overlay`. The
binary uses the common RRbin envelope and Symbolica state/coefficient tables,
with the distinct `DomainRules` kind. It records the base-owner content digest,
family, owner/root/order, nominated cases, prospective search policy, rules and
an explicitly empty residual list. It is not accepted as a complete-sector
candidate payload. Generation counters are not persisted.

`load_generated_domain_overlay` binds that payload to the supplied immutable
owner and redoes exact source/guard replay plus strict-descent checking before
returning a current-snapshot overlay. This cold validation is not repeated
during application. Native Symbolica frames require a trusted matching build;
structural limits do not make the underlying importer a hostile-input sandbox.

## Campaign selection

The existing selection JSON accepts an optional ordered list:

```json
"domain_rule_overlays": [
  {
    "path": "overlays/repair.rrbin",
    "bytes": 12345,
    "owner_mask": "101"
  }
]
```

The mask and byte count above are illustrative. Use the actual selected owner
and actual file size. Paths are relative to `--owner-base`. All base and partial
payloads receive combined resource admission before native import. Each repair
is cold-validated, then installed before routes and the walk are prepared.
Failure or cancellation does not publish a partly prepared reducer.

The staging Python utilities copy these opaque payloads into a portable
`overlays/` directory and preserve their order. They do no algebra. Query
objects and required/auxiliary roles are unchanged. The checkpoint binds the
selection and ordered base-plus-repair payload digests. Adding or changing a
repair therefore requires a **fresh walk**; do not edit an old campaign or
bypass its checkpoint identity. Existing owner generation can still be reused.

## Targeted research probe

The Rust example `probe_owner_case` supports generic selected owners and
coordinate cases. `--powers` takes integer powers, with `*` for free indices.
The current research probe requires explicit `--search-rank none` for free
indices (the underlying Rust API also supports finite rank scopes). A normal
source search with `--replay --inspect-installed --output-overlay FILE` writes
only nonempty, replayed, residual-free rules and refuses to overwrite a file.
Output must still pass an independent cold load before delivery.

`--load-overlay FILE` skips generation and uses the checked cold loader.
For downstream diagnostics, add `--follow-repaired-targets targets.csv` and
`--follow-max-nodes LIMIT`, using the full selection with its routing witnesses.
The file contains concrete physical index vectors, one comma-separated vector
per line. Exact singleton root admission replaces only the saved entry-rank
gate; descendants are never rank-clipped. The existing native routed engine
follows the results, reporting uncovered targets and resource-limited partial
runs explicitly.

There are three distinct results to keep separate:

1. Source replay and descent validate each added identity.
2. A complete local lookup establishes applicability on the nominated domain.
3. Discharging all reachable obligations establishes closure of the requested
   scope. A finite diagnostic trace is not a proof about an infinite ray.

The October1 five-loop repair and measured limits are documented in
[the frontier investigation](research/five_loop_a1_frontier_2026-10-01.md).
