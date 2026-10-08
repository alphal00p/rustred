# Saved-campaign terminal collection integration

## Goal

Make the existing public command use the new exact collection methods where
applicable, with no separate Rust driver:

```bash
python -B examples/python/saved_campaign.py refine --campaign "$RUSTRED_CAMPAIGN"
```

Status: implemented and acceptance-tested. Native/public-workflow controls,
independent audit, interruption/resume and real scope-extension checks pass;
see `docs/research/saved_campaign_terminal_collection_2026-10-08.md` and
`CODEX_PROGRESS.md`. This directive superseded stopping at the previous
library delivery, without reopening unrelated campaign optimizations.

## Implementation sequence

1. Inspect the existing refinement, publication, native codec, application,
   inspection and scope-extension paths. Delegate independent implementation
   slices only where their state/format ownership does not overlap.
2. Compose the existing finite refinement with exact cross-family full-U
   aliases and diagonal ordinary IBP collection. Work on actual available
   inventories; preserve unsupported and numerator-bearing keys unchanged.
   Keep mathematical algorithms in Rust/Symbolica and Python as steering.
3. Preserve complete guards, family-qualified identities, original sources,
   exact replay and common-mass homogeneity when composing maps. Never infer
   minimality or family closure from fewer output labels.
4. Persist the result through topology-independent native binary conventions.
   Cold-loaded published artifacts must apply the refined maps, not merely
   display smaller counts. Extend the evolving schema if necessary rather
   than adding RustRed compatibility machinery.
5. Integrate interruption/checkpoint/resume, repeated refinement and scope
   extension. A resumed or extended run must retain valid prior identities
   without incorrectly treating a newly requested terminal as already covered.
6. Expose accurate raw, normalized and remaining counts in inspection and
   appropriate refinement-stage progress. The ordinary `refine` command must
   actually select the applicable new methods; rebuilding alone is not enough.

## Acceptance and delivery

- Independent mathematical/code audit by an agent other than the implementer.
- Exact source replay, retained auxiliary terms and predecessor conditions,
  mass restoration and master-only output on every original terminal map.
- Public-workflow controls reproducing the measured four-loop 65 -> 20 and
  frozen five-loop 608 -> 607 results when the corresponding inventories are
  present. Do not promise these counts for other ranks, scopes or inputs.
- Cold artifact application, real interruption/resume, repeated refinement,
  scope extension and unchanged unrelated paths.
- Release measurements separating input loading, preparation, publication,
  cold loading and application. Avoid repeating proofs in the hot path.
- Test Rust, CLI and Python integration; document the exact supported commands
  and limitations; commit and push coherent tested milestones on main using
  ValentinHirschi <valentin.hirschi@gmail.com>.

Do not alter or signal live user campaigns. Test immutable copies in local
untracked TMP, preserve unrelated HEPKit/notebook work, and coordinate builds
and pilot resources. Use Symbolica for algebra; audit its available API before
adding any new algebraic operation. Do not chase exact minimality or develop
new terminal-search strategies as part of this integration goal.
