# Interrupted refinement recovery and proved-relation import

## Goal

Provide a tested command sequence for the user's interrupted `(2,2)` modular
refinement: recover the durable checkpoint without restarting its search,
absorb the independently established numerator-symmetry refinements, publish
the combined exact maps, and start a clean `(2,0)` exact round. Normal pause,
resume, and later `(3,0)` or other shape changes must preserve proved maps.

## Safeguards

- Work in the existing isolated `codex/numerator-symmetry-refinement` worktree
  under `/shared/codex-2/rustred`. Preserve unrelated main-worktree changes.
- Do not modify, start, stop, or recover the user's production campaign during
  development. All native acceptance uses new TMP fixtures and isolated copies.
- Keep old checkpoints and source publications immutable. Recovery and import
  write separate atomic outputs, with explicit source provenance.
- Native state and integrity hashes are not an independent mathematical proof.
  The researched 871-key result is an expected output, not import authority.
- Reuse existing Symbolica algebra, exact sparse reduction, symmetry replay,
  routing, coefficient application, guard handling, and native binary formats.
  Do not introduce a CAS or rational-reconstruction implementation.
- No topology-name, five-loop, or P102 dispatch in production algorithms. Source
  selection and finite proof recipes are explicit input data.
- Do not add arbitrary work/count/time ceilings. Retain explicit caller grants,
  checked representation limits, process RAM guards, and incomplete diagnostics.

## Implementation lanes

1. Native recovery: explicit finalize/recover operation requiring the existing
   exclusive writer lock, valid durable native state, and complete inventory.
   Preserve accepted state/maps/guards, do not resume source search, finite-field
   discovery or reconstruction. Label inherited/trusted and independently
   replayed authority accurately. Refuse live writers or malformed checkpoints.
2. Proof bridge: package the compact published baseline authority, exact native
   numerator-symmetry witnesses, finite saved-equation source selections and
   authenticated program/route inputs. Replay the declared finite recipe and
   authenticate all exact maps/guards before combining them with the current
   publication. Do not import the research reducer merely because its hash fits.
3. Publication: produce a normal application-aware publication, preserving
   existing query admission, program provenance, original terminal identities,
   conditions and any independently proved current refinements. Do not silently
   replace newer maps with the older research convention or declare new masters.
4. Python/CLI workflow: explicit recovery/import commands, finalization, fresh
   shape selection, ordinary resume, monotone continuation and proofs-only shape
   changes. Expose truthful stage/progress summaries. Keep ordinary behavior
   unchanged unless the new operation is requested.
5. Independent acceptance and mathematical/code audit. Root integrates, freezes
   release builds, verifies results, documents commands, and commits/pushes.

## Validation

- Stale-running durable checkpoint recovery, active-writer refusal, missing lock,
  bad metadata/digest/native state, source byte immutability and output resume.
- Source recipe/certificate replay, exact complete saved equations and guards,
  wrong family/scope/program/normalization rejection, and corrupt-map/witness or
  dropped-guard mutation controls. No modular hit or count grants authority.
- Exact composition of every original, existing-current and imported map through
  the result; outputs remain in the declared terminal convention. Retain all
  source conditions and generic-parameter assurance without claiming uniform
  exceptional-dimension validity or minimality.
- Proper cold-loadable app publication, unchanged physics admission, no new IBPs
  during import, fresh `(2,0)` with zero old cursors/auxiliaries, interruption and
  same-round resume, then `(3,0)` and another shape retaining proved relations.
- Focused Rust/native/CLI/Python tests and independent audit; real five-loop
  proof import/export only in a separate TMP delivery, never in production.

## Delivery

Push clean, audited code and documentation with the ValentinHirschi identity.
Leave proof/application artifacts untracked. Supply exact user commands for
cleanup, import, publication, fresh `(2,0)`, pause/resume and later shapes, with
measured initialization/replay overhead and explicit limits of the claims.
