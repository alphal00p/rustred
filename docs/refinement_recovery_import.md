# Recover an interrupted refinement, import proved relations, change search shape

This is an explicit workflow. None of these operations runs automatically when
inspecting or extending a campaign. Stop the old native writer first and keep
its checkpoint. Do not delete, edit, or replace its files manually.

## What the operations mean

- **Finalize** publishes the exact maps already durable in a paused/failed round.
- **Recover interrupted** is only for a durable checkpoint still labelled
  `running` after its process was killed. It requires the existing exclusive
  writer lock and full native validation. A live writer is refused. Recovery
  neither resumes discovery nor invents proofs for in-flight candidates.
- **Import proved relations** cold-replays a finite, authenticated recipe:
  weighted-denominator-preserving momentum symmetries and selected complete
  saved-rule consequences. It preserves newer current maps and conditions,
  combines exact equations and publishes the result. A research output can be
  an expected coefficient assertion, never equation authority.
- **Fresh search** starts a new source inventory from the latest published maps.
  Old auxiliary matrices, modular images, pending candidates and source cursors
  are dropped. The old round and original source publication remain intact.

These operations retain the original admitted starting-domain union and terminal
identities. They do not establish master independence, enlarge rank/D admission,
compute numerical values or generate new parametric IBP rules. Current compact
fresh/import support is single-family; unsupported cases fail explicitly.

## Native and short interfaces

The native commands are `walk-finalize-refinement --recover-interrupted`,
`walk-import-refinement --proof-recipe`, and
`walk-master-reduce --fresh-search`. The short Python interface supplies
`finalize-refinement --recover-interrupted`,
`import-refinement --proof-recipe`, and `refine --fresh-search`.

For custom RAM/affinity settings use the production adapter below. Extra resource
flags on legacy short `refine`/`finalize-refinement` actions are not forwarded.

## Tested local five-loop sequence

Use the feature Python adapter together with its matching frozen executable.
Main's older adapter does not understand the new actions. These definitions
perform no campaign work until `rr` is called:

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN="$PWD/campaigns/five-loop-minimal-r0-d10-incremental"
export RUSTRED_BIN="$PWD/TMP/releases/20261022-refinement-bridge/rustred"
export RUSTRED_DRIVER="$PWD/TMP/numerator-symmetry-refinement-20261021/examples/python/production_saved_owner_campaign.py"
export RUSTRED_PROOF="$PWD/TMP/releases/20261022-refinement-bridge/proof-recipe.json"

rr() {
  nix develop --command python -B "$RUSTRED_DRIVER" \
    --campaign-directory "$RUSTRED_CAMPAIGN" --resume \
    --master-reduction-executable "$RUSTRED_BIN" \
    --max-memory-bytes 600000000000 --memory-policy process-limit \
    --swap-growth-stop-bytes-per-second 0 --max-zero-progress-ram-stops 0 \
    --start "$@"
}
```

The release directory also contains a sourceable `workflow.sh` with these same
definitions. CPU and RAM grants are operational settings, not mathematical
source limits. `process-limit` retains the process RAM guard without enabling a
separate host-reserve/swap stop. Host availability remains visible.
The explicit zero-progress RAM-stop setting disables the launcher's historical
retry-count gate; it does not disable the native process RAM guard.

### A. Retain the killed `(2,2)` round

For the inspected stale-running checkpoint, with no native writer alive:

```bash
rr --finalize-refinement --recover-interrupted-refinement
```

For a normal clean interruption whose native status is `paused` or `failed`,
omit recovery and use `rr --finalize-refinement` instead. Repeating explicit
recovery for a genuinely running process is not a way to bypass its lock.

Recovery writes a separate derivative publication and advances the campaign's
publication pointer only after successful validation. The old checkpoint itself
is unchanged. Only durably accepted maps survive; uncommitted candidates do not.
The present recovery path intentionally refuses an already exhausted source
search with unfinished collection. Resume that collection normally rather than
dropping its proof layers. The inspected killed `(2,2)` source is not this case.

### B. Absorb the additional exact relations

```bash
rr --import-refinement --master-proof-recipe "$RUSTRED_PROOF"
```

The tested recipe binds the old published baseline, native symmetry witnesses,
and four explicit saved-row selections. It does not replace a newer basis with
an older one. Successful import **already publishes** its maps; no second
finalization is needed before C. To resume an interrupted import, repeat this
same command. It checkpoints completed selected rows and archives its inputs.
After preparation, an authenticated archived recipe may be used if the original
recipe or source has moved. Do not alter a recipe in place to change a round.

### C. Start exact `(2,0)` without the old search history

```bash
rr --refine-masters --master-fresh-search \
  --master-refinement-strategy exact \
  --master-seed-depth 2 --master-containing-sector-depth 0 \
  --master-numerator-symmetry-assistance --no-master-saved-rule-assistance \
  --master-max-relation-columns 0 \
  --master-refinement-workers 128 --master-refinement-cpus 64-191
```

`0` for the relation-column grant means unlimited, not zero columns. The raw
numerator-symmetry provider is generic and comparatively cheap on this inventory.
Full saved-rule assistance is deliberately disabled: eagerly projecting every
large numerator equation showed severe exact coefficient swell. This choice
does not undo the four selected saved consequences already imported in B.

The complete ordinary `(2,0)` search has not been timed to completion by this
delivery. Preparation/cold proof loading can take minutes; a quiet interval is
not a reason to launch a second writer.

### D. Pause, resume, and change shape

Press **Ctrl+C once** and wait for the checkpoint and shell prompt. Resume the
same round without `--master-fresh-search`:

```bash
rr --refine-masters
```

For a different round after a clean interruption, retain the newly proved maps
first, then discard search history deliberately:

```bash
rr --finalize-refinement
rr --refine-masters --master-fresh-search \
  --master-refinement-strategy exact \
  --master-seed-depth 3 --master-containing-sector-depth 0 \
  --master-numerator-symmetry-assistance --no-master-saved-rule-assistance \
  --master-max-relation-columns 0 \
  --master-refinement-workers 128 --master-refinement-cpus 64-191
```

If the preceding round completed normally, skip finalization. If it was killed
and still says `running`, use explicit recovery instead of ordinary finalize.
To **extend and reuse the old rowspace** rather than reset it, omit fresh and
raise the requested depths monotonically. Specify both depths to avoid carrying
an unintended containing-sector depth from an earlier round.

Inspection always uses the latest successfully published artifact, which may
precede an unfinished search:

```bash
nix develop --command python -B \
  TMP/numerator-symmetry-refinement-20261021/examples/python/saved_campaign.py \
  inspect --campaign "$RUSTRED_CAMPAIGN" --executable "$RUSTRED_BIN"
```

## Validation and limits of the claim

The 22 October frozen optimized executable has SHA256
`4a8ffc2a5c0cac878895f36a58866e7776fd09a17fffd38a256a45d0e8f52f0d`.
The implementation is on `codex/numerator-symmetry-refinement`, milestone
`12ee99bb`. Focused acceptance: 181 native tests passed, four manual ignores;
214 Python tests passed, one existing affinity skip; 12 independent fresh-search
and six independent recovery CLI checks passed. Tests include genuine accepted
modular-relation recovery, source immutability, active locks, codec/guard/source
mutations, interrupted import, source relocation and preservation of stronger
current maps.

On a separate filesystem clone of the real five-loop checkpoint, recovery
completed in **10m26.67s** and import plus final cold artifact validation in
**16m09.71s**. Each peaked at approximately **20.1 GiB RSS**. The result retains
all 939 original keys and reduces the finite basis **904 -> 871**, with zero new
ordinary IBP rows and zero auxiliary output columns. This is 33 additional exact
dependencies, not 33 newly generated parametric IBPs or a minimal-basis proof.
The long real recovery used the preceding optimized preview with the identical
recovery code; a final importer-only path-protection fix was building in
parallel. Import, fresh-round tests and six separate recovery CLI gates used
the final executable identified above. Compilation is excluded from all timings.

A real fresh `(2,0)` initialization with cooperative stopping passed in 156.61s,
at 1.75 GiB peak RSS, writing a 42.85 MB checkpoint. Independent exact comparison
confirmed that all 939 maps and all 108,916 final conditions survived, with zero
old auxiliary/source/modular history. Its newly enumerated 7,646,675 ordinary
source-work items were not consumed. This is a cold initialization/pause test,
not a completed `(2,0)` refinement or a prediction of its eventual runtime.

An independent public application loader also cold-validated the complete
collection, compact authority and imported archive. All 939 application maps
match the native result and expected research coefficients exactly; all final
conditions and selected-row/equation conditions are present. P102's three-term
formula and absence of unresolved outputs passed. Whole verifier runtime was
143.36s, peak 1.74 GiB. This is an exact all-key check, not a numerical zero test.

The P102 identity is `P162 + P86/2 + m² P106/2` in the independently checked
native graph/routing convention. Production algorithms do not recognize these
names: they operate on exact family fingerprints, signed indices, weighted
denominator maps, whole numerators and conditions. Unequal masses or powers
reject invalid permutation arguments. The implementation uses existing
Symbolica algebra and RustRed transport/routing/elimination services.

Real tests write only under `TMP/recovery-import-20261022/`; the production
campaign was neither recovered, imported, resumed nor republished by this work.
Artifacts/proof data remain external and untracked. See `CODEX_PROGRESS.md` and
`RECOVERY_IMPORT_PLAN.md` for the final audit and application-export receipts.

## Separate Vakint-ready package

The independently exported 871-key application is already available at:

```text
/shared/codex-2/rustred/TMP/deliveries/five-loop-r6-d10-871-20261022/application
```

Copy the complete directory, not just `application.json`. It contains the
programs, routes, admitted scope and exact terminal substitutions, but no large
campaign traversal ledger or refinement search matrix. The old 904-key package
has not been changed. It retains the same 812 recorded query domains (758
nonempty, 54 empty); rank 6 and D=10 summaries are not blanket rectangular
admission. Caches are bound to package identity and must not be reused across
the old/new terminal convention without native validation.

Export took 237.62s, including all 939 source-map checks. Fresh native loading
took 45.6465s; a real five-IBP canary took 0.22024s, with its exact cached repeat
taking 0.15964s, one hit and no new rule applications. Every output belongs to
the declared 871-key basis. The existing actual Vakint loader accepted this
package without rebuild in 77.9403s; it prepared the 67-topology catalogue only
on load (0.15749s), leaving owner formula programs deferred. These are single
functional delivery measurements, not a matched performance comparison or full
five-loop numerical acceptance. The delivery README records the hosting archive
and hashes. All generated package files remain untracked and unpushed.
