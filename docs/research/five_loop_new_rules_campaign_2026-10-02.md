# Independent five-loop rank-two shortcut campaign

Prepared October 2, 2026, at the user's request. The user launches this campaign
in Zellij session `rustred`, tab `codex_astra_new_rules`; preparation does not
start a solver or modify the existing campaign.

## Launch

```bash
cd /common/dev/rustred
nix develop /common/dev/rustred --command python -B \
  /common/dev/rustred/campaigns/five-loop-a1-new-rules-20261002/steering/production_saved_owner_campaign.py \
  --campaign-directory /common/dev/rustred/campaigns/five-loop-a1-new-rules-20261002 \
  --start
```

No Rust compilation or regeneration of the existing owners is needed. The
campaign contains its own frozen native executable and supervisor/dashboard.
Use the Git-aware absolute root path above, without the `path:` prefix. The
explicit `path:` backend copies ignored build caches and campaign data and is
unsuitable for this large workspace. Do not select an untracked nested flake.
After a clean stop, resume this new campaign with the same
command and `--resume` added. Do not import the other campaign's checkpoint:
the changed rule pool intentionally starts with a fresh dependency graph.

- 32 distinct physical CPUs96–127, separate from the existing campaign's64–95.
- Requested RAM ceiling600GB, cooperative save-and-stop at570GB, host reserve
  150GB. Admission rechecks available host/cgroup memory at launch and can lower
  the effective ceiling. Preparation admitted the full600GB.
- Automatic checkpoints every hour; Ctrl+C requests a saved stop. Wait for the
  checkpoint completion message. There is no runtime deadline.
- Existing Epoch rolling/snapshot/FIFO/oldest-prefix configuration, cut16,
  window76,31inspectors, lookahead256, G2 union,1024-job/256MiB result escrow.
  Unknown frontiers still stop and save; they are not silently ignored.

## What changes, and what does not

The input preserves all67 owner families,8,246 routes,116 required physical or
convenience queries,67 auxiliary helpers and the existing frontier-repair
overlay. Query bytes are identical. Only owner18910 (mask011110111001001) is
replaced: it gains the checked37-term rank-two shortcut ahead of its72 original
rules. All six existing terminals and every fallback rule remain unchanged.

The shortcut fixes the two bubble powers to one and the mixed numerator power
to minus two; seven other positive indices remain arbitrary. It is not a
fixed-point identity. The request and exact source combination are included
with the prepared inputs. `Partition` priority is intentional: the narrower
chart can subdivide an old application domain. The post-baseline whole-piece
policy was not used because the screened narrow chart did not activate there.

The existing A1-generated rule pool and validated repair are retained. Negative
or unproved alternatives (broad46, rank-one reflection, global-degree ordering,
larger symmetry averages) are not bundled merely to add more experiments.

## Validation

The existing Symbolica-backed producer regenerated147 translated ordinary IBPs
from the actual five-loop family, retaining all2,694 source-term entries. The
complete38-term homogeneous identity replayed exactly over the seven-free-index
chart. Native export verified strict descent on all36 sign cells and preserved
guards, old coefficients, rules and terminals. Owner bytes grow567,839→571,825.

The native matcher now selects the new rule at
`[0,1,2,2,1,0,1,1,1,-2,0,1,0,0,1]`, an actual required A11/R2/D9 input.
Its exact37-term RHS equals the replayed identity; the old selection produced64
nonzero terms. Changing the second index from1 to2 leaves the exact original
fallback RHS and selection intact (apart from its shifted rule ordinal).

The small optimized export adapter took135.028s to compile against the cached
release libraries,2.022s native execution and4.380s for the guarded export
wrapper. These are preparation timings, **not full-campaign timings**. No
engine rebuild was needed. Existing release tests had passed386 core reduction
tests and119 app bundle tests at the preceding checkpoint. Independent agent
`reflection_rule_probe` audited source replay, descent authority, fallback,
scope and isolation; root executed and checked the native preparation.

Frozen native executable SHA256:
`8ef80b52da13866dbe09d817acd004ec3fc9ca2130d85411ac758398086cf0a7`.

The existing live campaign uses the earlier `4e76707b` executable. This new
trial uses the tested development checkpoint; a side-by-side production timing
comparison is therefore not a strictly same-binary, single-rule intervention.

## Expectations and evidence

Two earlier five-loop local descendant-union pairs showed23.65–26.18% fewer
domains and29.68–37.02% less traversal time. They bypassed parent matching and
did not install this rule throughout the full graph. Whole walk-plus-cold-check
cost was mixed. The analogous integrated four-loop insertion increased work.
Consequently this is a justified **full-scope experiment**, not a claim of a
25% total speedup or guaranteed eventual closure. The full campaign tests both
the benefit of simpler descendants and the cost of additional partitioning.

Local preparation evidence (not committed campaign payloads):

- `TMP/prelaunch-20261002-new-rules/export/{adapter.rs,request.json,run.py}`
- `TMP/prelaunch-20261002-new-rules/export/probe-r1/RESULT.json`
- `TMP/postlaunch-20261002/new-rules-campaign-delivery/prepare.py`
- `campaigns/five-loop-a1-new-rules-20261002/prepared-launch-plan.json`
- `campaigns/five-loop-a1-new-rules-20261002/inputs/EXPORT.json`

Owner and overlay payloads are copied, with relative input paths; none point
back to the live campaign. Provenance contains historical absolute paths.
The existing frozen steering records this installation's paths, so relocation
to another directory requires updating steering, not mathematical payloads.
