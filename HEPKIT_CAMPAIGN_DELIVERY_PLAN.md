# HEPKit campaign delivery after five-loop research wind-down

## Goal

Finish the requested RustRed release and independent five-loop campaign, then
deliver a symbolica-community HEPKit PR with native streamed campaign events,
lazy artifact exploration and a polished four-loop marimo notebook. Request
BenRuijl as reviewer. This is an extension of the October4 user directive, not
a claim that the original five-loop performance/closure objective is solved.

## Sequence and ownership

1. **RustRed release first.** Finish PR2 integration and independent review,
   optimized build, complete four-family control/cold verification and bounded
   five-loop canary. Preserve unrelated work and live production. Stage the
   separate banana485 variant with all116 required inputs and67 helpers.
   Commit/push main and freeze exact launch commands and executable identity.
2. **Native API implementation.** Inventory the existing Rust/PyO3 campaign,
   progress, cancellation and binary-artifact interfaces. Reuse them and the
   shared Symbolica engine; do not start another solver or parse CLI text as
   the notebook's native event API. Keep generic family configuration as input.
   Provide bounded streamed event consumption, safe worker lifetime/GIL release,
   cancellation and completion/error handling. Preserve current API defaults.
3. **Artifact exploration.** Add cheap metadata and paged rule/terminal access,
   lazy expression rendering, useful rich representations and exact detail
   inspection. Avoid eager materialization of all rules into Python strings.
4. **Community integration and notebook.** Work in a separate workspace-local
   clone/branch. Pin the pushed RustRed revision, share the host Symbolica
   kernel, and follow existing HEPKit conventions. In particular, use the
   standard DOT-based graph input and existing Graph/FeynmanDiagram, routing,
   kinematics and integral-family primitives as the notebook's front door.
   Pass the resulting native family to RustRed without rematching graphs or
   requiring a second manually maintained TOML definition. Preserve denominator
   order and the distinction between physical propagators and auxiliary ISPs.
   Standalone RustRed source-input APIs remain available independently.
   Model the notebook on the
   gallery's clear explanatory cells, direct scientific calls and collapsed
   setup. Put Python helpers longer than roughly50 lines into appropriate
   reusable support and expandable startup cells. Display live progress and
   browse actual generated IBPs/terminals, not simulated events or precomputed
   results disguised as a solve.
5. **Validation and PR.** Independently audit implementation/API behavior,
   run the actual four-loop workload serially, and visually inspect its live
   dashboard and lazy explorer. Report measured timing boundaries; approximately
   five minutes is an expectation, not an asserted result or timeout. Open
   the community PR, request BenRuijl, and report the actual outcomes.

Root remains orchestrator/integrator/final verifier. Rotate native implementation,
notebook integration and independent review across the available agents after
their current release tasks. Serialize overlapping files and resource-heavy
jobs. Record decisions, evidence, failures and source revisions in
`CODEX_PROGRESS.md`; do not reopen parked optimization research during delivery.

## Acceptance evidence

October5 lazy-view requirement: large artifacts must not become large notebook
outputs. Fetch bounded structural pages; decode only the selected coefficient;
render small previews with explicit expansion/export controls. A collapsed
accordion is not lazy if its contents were already constructed. Bound RHS,
guard, coefficient-choice and terminal tables as well as raw JSON. Test with
the actual generated artifacts and an oversized display case; distinguish
bounded browser output from native coefficient-decoding memory costs.

October5 additional requirement: update the community's Vakint dependency/API
as necessary for its FORM-less four-loop RustRed backend, with high-level rich
HEPKit objects rather than a second low-level input dialect. Add a nontrivial
four-loop numerator evaluation, tested with FORM unavailable and an established
numerical reference. Verify the fresh-to-shipped artifact lineage, including
normalization and master-catalogue preparation, before describing equivalence.
Keep the community kernel shared, preserve default/legacy backend behavior,
and use the appropriate pushed GammaLoop revision. Coordinate dependency edits
centrally with root; do not disturb other tasks' checkouts or production runs.

October5 addition: compare actual four-loop generated residual records and
distinct terminal integrals with FMFT's numerical input/master basis, using
verified source definitions. Explain the nonminimal-terminal versus master
distinction in the notebook. Prefer a demonstrable post-generation reduction
of terminals through existing RustRed APIs when feasible; preserve the current
run and its outputs, avoid a new generation workload or a separate CAS, and
do not equate coordinate vectors from different families without a routing map.

The October4 AMFlow+DiffExp follow-up also belongs to this delivery: remove the
incidental 12-slot runtime-dispatch restriction, test an actual higher-arity
family, and document the API/build capabilities separately from mathematical
coverage. Coordinate with the AMFlow task because its native adapter contains a
second independent 1–12 dispatcher. Preserve the solver's generic algorithms;
do not describe a larger compiled dispatch table as an unlimited runtime API.
Send the downstream task the pushed revision and any bridge-signature migration.

- Pushed RustRed main contains reviewed PR2 and the owned optimization changes;
  foreign local edits/reference material/license/campaign outputs are excluded.
- Frozen second-campaign inputs preserve the complete scope, repairs and
  correlated caps. Commands use disjoint resources and fresh state. Existing
  production/checkpoints remain untouched; only the user launches production.
- Native event API tests cover execution, ordered/typed events, completion,
  cancellation/error, bounded buffering, dropped consumers and safe GIL use.
- Artifact tests cover metadata, pagination/index bounds, exact selected rule
  details and terminals, cold loading, and lazy rendering behavior.
- Notebook passes static checks and an actual optimized four-loop execution;
  performance reports separate preparation/generation/traversal/verification.
  Visual review checks alignment, legibility, progress semantics and responsive
  rule/terminal navigation. No false complete-family or minimal-master claim.
- Community dependency points to a pushed RustRed revision; the tested host
  shares one Symbolica state and existing HEPKit behavior is covered. DOT
  graph-to-family-to-generation tests use standard HEPKit primitives and
  preserve routing/denominator/auxiliary-coordinate semantics.
- Community PR exists, with BenRuijl requested (or any platform restriction
  explicitly reported), clear instructions and evidence. Never sign a CLA or
  other legal agreement on the user's behalf if GitHub requests one.
- Final response includes both repository delivery status, honest results,
  exact new-campaign launch/resume instructions and notebook/PR links. Mark the
  amended delivery goal complete and stop only after all required work is done.

## Initial inspection

The fresh clone is `TMP/symbolica-community-rustred-gallery-20261004`, initially
at `b224e50`. Its existing native HEPKit module already embeds
`rustred-feynkit`; the community Symbolica kernel is patched to the upstream
`community` branch. The gallery provides `four_loop_numerator.py`, using
`app.setup(hide_code=True)` and short visible calculation cells. These are
starting references, not evidence that the requested campaign API exists yet.
