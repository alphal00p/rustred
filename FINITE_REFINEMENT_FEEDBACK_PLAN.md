# Finite refinement feedback

Status: implemented, independently audited and measured on October8,2026.
All scoped steps below are complete. The frozen five-loop inventory improves
607→601, while combined four-loop remains20; feedback costs additional one-off
time and memory. Public lifecycle and interrupted/resumed byte-identity checks
pass. Results: `docs/research/finite_refinement_feedback_2026-10-08.md`.

## Objective and scope

Implement and measure exact feedback between the existing finite per-family
IBP searches and cross-family terminal collection, only in explicit `refine`.
Do not change the parametric campaign walker, add production work, or infer
broader closure/minimality. Publication and extension may reuse already proved
identities but must not start this discovery stage implicitly.

Simply reinserting terminal-only identities into an already eliminated system
does not add independent information. The concrete opportunity is to combine
the retained finite rowspaces, including unresolved auxiliary columns, after
valid cross-family scalar vacuum identifications. This can expose cancellations
that independent family elimination could not see. Unsupported columns remain
present and family-qualified; no auxiliary may be silently dropped/promoted.

## Implementation

1. Audit the pinned Symbolica sparse elimination and existing RustRed alias,
   exact relation, source/proof, native codec and application interfaces. Use
   Symbolica arithmetic and elimination, not a competing CAS kernel.
2. Add a cohesive family-qualified finite feedback stage over retained rows
   and proved identities. Preserve guards, native family identity, generic
   dimension, mass homogeneity and inherited authority. No new source seeds
   or topology/loop-count dispatch are part of this change.
3. Save exact reusable feedback maps/proofs in the existing native artifact
   system. Cold validation, interruption/resume, repeated refinement and scope
   extension must preserve the correct state and never exaggerate coverage.
4. Integrate an explicit on/off control for matched comparisons through the
   Rust/application/CLI/Python surfaces; keep Python thin. Report the finite
   feedback stage and useful input/output/work counters separately.
5. Independently audit mathematics, code, persistence and interpretation.

## Validation and impact assessment

- Focused cases with genuine auxiliary cancellation, family isolation,
  unsupported columns, guards, degeneracy, stable output and exact replay.
- Public workflow regression: refine, cold application, inspect, repeat,
  interrupt/resume, scope extension, and no implicit feedback on publication.
- Release matched on/off runs on immutable copies of each available four-loop
  parent, their combined inventory, and the frozen five-loop inventory. Add
  lower-loop/small workflow controls where readily available.
- Record terminal counts, preparation/full-process wall/CPU time, peak RSS,
  rows/columns/fill and artifact size. Compare all original reduction maps and
  use the existing four-loop exact FMFT census where available. Five-loop
  comparisons are algebraic, not numerical evaluations or minimality proofs.
- Keep negative results: deliver correctness and report honestly if the new
  stage finds no additional relation or costs more than it saves. Do not claim
  a campaign-closure speedup from a reduced terminal count.

## Coordination and delivery

Root coordinates tests/profiling and final verification; separate agents own
core implementation, app integration, and independent adversarial audit.
Keep progress in `CODEX_PROGRESS.md`, scratch evidence under local `TMP/`, and
use existing build locks/CPU budgets away from production CPUs64–95.
Never signal or mutate live campaigns. Preserve unrelated HEPKit/notebook work.
Document, commit and push only audited task-owned changes using
ValentinHirschi <valentin.hirschi@gmail.com>.
