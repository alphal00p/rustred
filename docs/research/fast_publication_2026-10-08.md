# Saved-campaign publication: optional deep verification

## Purpose and assurance

Ordinary publication should not repeat an independent campaign proof whenever
the starting rank is increased. The new default extracts a trusted saved-scope
inventory; `--deep-verification` retains the independent geometry and successor
reinspection. A modular numerical check is not substituted for an exact check.
Rather, the optional checks are explicitly *not performed* in the fast mode.

Both modes authenticate the captured checkpoint files and their exact
request/owner bindings. The fast mode checks record structure and completion
of the saved dependency graph, then enumerates the rules and terminal keys
actually classified in recorded Apply inspections. The graph's edges are
trusted in this mode. It never replaces encountered terminals by every residual
declared in an installed program. It reports `TRUSTED_SAVED_SCOPE`, not `PASS`,
and `independently_verified: false`.

The existing deep verifier proves coinductive dependency coverage: sealed cycles
can count as closed. Neither mode establishes strict descent, termination of
coefficient back-substitution, unrestricted family closure, master independence,
or numerical master values. The published starting scope remains explicit.

## What changed

- Deduplicate identical recorded Apply scopes using a digest index **and full
  structural equality**. Hash collisions never merge distinct scopes.
- Match successful Apply scopes without constructing RHS successor domains or
  replaying Route inspections. Partial and G2 records use their actual inspected
  residual, not the original larger domain. Abandoned uninspected records are
  excluded. Failed/interrupted records retain the full local prefix replay so
  later, never-visited matches cannot be invented.
- Accumulate identities in worker-local sets/maps and merge deterministically,
  retaining multiplicities and resource limits without a lock per classification.
- Hash checkpoint buffer fills, not each tiny decoded integer. Track consumed
  bytes separately from prefetched bytes; unread tails cannot pass validation.
- Read the native record projection directly, removing a serialize-to-JSON-text
  and parse-back round trip. Framing, record counts and EOF authentication remain.
- Decode compact domain images into checked stack buffers instead of allocating
  two temporary coordinate vectors per saved domain.
- Avoid building the reverse graph when every saved record is sealed. This is
  the same coinductive closure result, not a new mathematical assumption.
- In the optional exact union checker, cache integer interval sums, borrow
  owner masks, apply/undo individual constraints in place, and allocate only
  nonempty split pieces. Predicate answers, traversal order and budget outcomes
  are unchanged.
- Emit loading, structural-check and census progress. Inspection distinguishes
  inventory completion from independent verification.

The old CP6 format lacks a saved rule/terminal identity census, so publication
still reads its graph/records, prepares the owners, and performs exact matching.
This change is not an incremental checkpoint-inventory architecture and does
not promise constant-time publication as the campaign grows.

## Existing checkpoint widths

Testing found a pre-existing mismatch between authenticated on-disk arity and
the current executable's padded storage capacity. In particular, THE_ONE stores
15 slots while the new dispatch uses capacity 16. G2 residual bytes must be
decoded at their original width, then compared using checked zero padding.
The fix preserves the original-width record binding and rejects nonzero/open
omitted coordinates or owner bits on nonexistent wire axes. It does not change
the checkpoint or require regenerating rules. This covers cold publication;
the separate runtime `AnchorView` resume-width issue is outside this change.

Two more capacity-mode gaps were exposed by actual cold controls and repaired:
partial overlays now restore their physical owner/root/cases/order with the
existing checked padding machinery, retaining all source replay and digest
checks; native Route pinches no longer reopen nonexistent storage axes as
unbounded numerator directions. These are input/geometry fixes, not weakened
verifier comparisons. The old failed control receipts are preserved as failures.

The real Python lifecycle also found that a verification-only upgrade reset a
completed finite-search cursor by unconditionally calling `extend`. Exact
same-inventory/same-depth imports now retain that cursor unchanged. Genuine
changes of inventory or depth still use the original extension operation.

## Measurements and validation

THE_ONE's earlier full R≤4,D≤9 publication completed with 335/335 queried
domains verified, 938 raw terminals, 6,851 encountered rules and no violations.
Its saved graph contains 60,115,005 domains and 596,470,375 edges. Full publication
took 14,135 seconds: checkpoint loading 1,347 s, preparation 88 s, structural
checks 10,332 s, and native reinspection 2,286 s. A 15-second sampling profile
identified interval-geometry work and allocation/copying as the main serial cost.
These are measurements of that earlier run, not new-path projections.

Four alternating matched single-core pairs, using actual saved anchor regions:

| Exact union predicate workload | Before | After | Speedup |
|---|---:|---:|---:|
| 49 four-loop regions, 2,000 repetitions each | 1.03–1.05 s | 0.290–0.293 s | 3.54–3.58× |
| First 2,000 THE_ONE anchor regions, 100 repetitions each | 10.28–10.46 s | 2.413–2.417 s | 4.26–4.33× |

These are warm **predicate** timings, not whole-publication speedups or a random
five-loop sample. Eight optimized tests compare the old/new recursion including
remaining region budgets, exhaust small integer grids, and check cached-state
mutation/undo. Implementation and independent audit were separate.

Read-only evidence, binaries, exact commands and receipts are under
`TMP/fast-publication-20261008/`; they are not committed. The compatible frozen
baseline is `9e6c0e9f4c6aa9b51062c213a168fac5ae4c1d2a50454eeab2e06f4c337fb327`.
Its complete four-loop control takes 6.359 s (28 raw →20 normalized keys);
the five-loop scalar R=0,D≤9 control takes 82.730 s (196→196 keys), of which
72.784 s is owner preparation. Host contention is not controlled; comparisons
must report full timing boundaries and should not extrapolate the geometry
speedup to preparation-dominated cases.

The final release control uses the same executable for fast/deep modes
(`a28f294bf7cc88a9a0804a07cfc5681f4e146f424f968fc7144b464b8d908797`),
four workers on physical CPUs 32–35, and nested pools limited to one. Compilation
is excluded. On the saved four-loop control, fast publication takes **3.667 s**
versus **15.174 s** with deep replay. Both enumerate exactly 429 rule identities,
28 raw terminal identities, 8,804 rule classification events and 85 terminal
events. The normalized output is 20 keys; finite native payload (9,053 bytes)
and collection payload (5,964 bytes) are byte-identical between modes. These
are single completed controls, not a statistical performance guarantee.

The corrected five-loop scalar fast control also completes: 141.805 s,
196 raw/normalized keys. Owner preparation takes 127.240 s; inventory matching
4.444 s, and saved-graph checks 0.007 s. This is slower than the older physical-
arity baseline overall: the new capacity-dispatch build and changed preparation
cost must not be disguised as an across-the-board speedup. The useful comparison
for the optional verification switch is fast versus deep in the same build.
That same-build deep scalar control completes in 162.047 s, with preparation
131.356 s and reinspection 21.425 s. Its 1,180 rule identities, 196 terminal
identities, 17,592 rule events, 564 terminal events, finite native payload
(27,155 bytes) and collection payload (24,324 bytes) match fast mode exactly.

| Saved publication control, same new build | Fast wall / CPU | Deep wall / CPU | Fast / deep peak RSS |
|---|---:|---:|---:|
| Combined four-loop control | 3.667 / 9.09 s | 15.174 / 44.79 s | 144 / 186 MiB |
| Five-loop scalar R=0,D≤9 | 141.805 / 266.31 s | 162.047 / 339.66 s | 7.21 / 7.28 GiB |

CPU time is user plus system. Preparation dominates the small scalar case;
the four-loop input has 26,025 saved domains, versus 33,539 in the scalar case.
These are publication controls, not IBP-generation or master-refinement times.

The actual Python workflow passes fast publication, refinement, deep assurance
upgrade, inspection, repeated-refinement no-op, fast-after-deep no-downgrade,
and genuine rank-2/D≤5 extension. Assurance upgrades preserve the exact native
and collection bytes and completed finite-work cursor. All source checkpoints
remain unchanged. Evidence: `workflow-fixed-validation.json` and
`four-loop-fast-deep-comparison-fixed-fixed.json` in the evidence directory.

Focused native tests pass for 49 master/public-workflow cases, six cold-overlay
cases and three physical-capacity Route controls. Python passes 134 campaign/
dashboard tests and 15 saved-campaign command tests. The broad capacity-mode verifier filter has
67 passing tests, one ignored, and eight known failures in optional **CP5
result-file binding**: physical-width and padded JSON produce different raw
digests. The strict binding checks were not weakened. Normal publication does
not supply this optional result file, and the real deep publication control
passes. This separate representation issue remains deferred, not reported as
a passing full suite.

Formatting is checked for every changed Rust source. Workspace-wide formatting
also encounters pre-existing unrelated differences and a missing research
module (`tools/research/rule_optimizer/routed_cancellation/record.rs`); unrelated
code is left unchanged rather than claiming that broader check passes.

## Usage

```bash
# Default: publish the completed recorded scope without deep reinspection.
python -B examples/python/saved_campaign.py publish --campaign "$CAMPAIGN"

# Optional stronger audit, including upgrading an existing publication.
python -B examples/python/saved_campaign.py publish --campaign "$CAMPAIGN" \
  --deep-verification

# Explicitly return the remembered preference to fast publication.
python -B examples/python/saved_campaign.py extend --campaign "$CAMPAIGN" \
  --rank 5 --no-deep-verification

python -B examples/python/saved_campaign.py inspect --campaign "$CAMPAIGN"
```

The rank in the example is a user choice, not an automatically enlarged scope.
Existing publications remain inspectable throughout new work. Verification-mode
changes use distinct publication phases; native resume cannot silently change
its assurance mode. Deep upgrades preserve previous finite master substitutions
and collection state. Explicit refinement inherits source assurance and does
not trigger another graph replay.
