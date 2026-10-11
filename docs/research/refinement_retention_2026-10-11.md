# Retaining stopped finite refinement without arbitrary matrix-count ceilings

## Scope and diagnosis

The requested change is operational: retain exact finite refinements across
later scope extensions and remove the preset million-column failure. It is not
a new source-selection or master-minimization algorithm. All elimination and
coefficient arithmetic remain in Symbolica.

Read-only evidence from THE_ONE's failed refinement (scope `692ef98e…`):

| Measurement | Value |
|---|---:|
| Last observed source rows | 481,131 / 600,250 |
| Last observed columns | 999,953 |
| Last observed independent rows | 457,348 |
| Last observed sparse nonzeros | 8,796,666 |
| Refinement peak RSS | 3,151,450,112 bytes |
| Configured hard RAM limit | 600,000,000,000 bytes |
| Durable source rows | 273,492 |
| Durable columns | 575,189 |
| Durable exact eliminations | 31 (939 → 908) |

The actual error was `terminal relations columns: 1000001 exceeds 1000000`.
Growth over the observed range was roughly linear, with no observed memory
runaway. This does **not** prove the remaining search will be inexpensive or
find further relations. Later telemetry recorded 34 eliminations; only the 31
in the durable checkpoint are recoverable. The much larger inventory-memory
counter in the inherited report describes earlier publication, not this search.

## Implementation

- Application-level aggregate finite matrix counters no longer have preset
  count ceilings. `refine --max-relation-columns N` optionally imposes one;
  `0` removes it. A compatible resume can change this operational budget or
  executable without restarting its mathematical source cursor. Launcher RAM
  admission/stop policies, checked Symbolica index widths, per-operation algebra
  bounds and native byte-size checks remain.
- Collection resource recipe v3 keeps aggregate retained feedback and map/guard
  composition RAM-governed. Old recipes decode under their original limits
  before an inherited collection is rebound. Diagonal source-generation budgets
  are unchanged. Actual-checkpoint export exposed the separate 10,000-condition
  composition cap; the same policy removes it without dropping any condition.
- Collection binary transport admits up to `max_program_bytes / sizeof(u32)`
  repeated atom references, retaining its existing 1 GiB program budget. This
  avoids an unrelated 8M-reference ceiling when a large common condition set
  accompanies many outputs. The wire schema and coefficient encoding are
  unchanged; no separate CAS or reconstruction code is introduced.
- `finalize-refinement` publishes a distinct immutable package from a stopped
  refinement. It drains only pending reindexing of already-proved rows and
  rebinds already-proved collection maps. It does not search new ordinary IBPs,
  invoke equation providers or run new finite-feedback/diagonal discovery.
- Source rows, guards, authority and unfinished cursors survive. The source
  checkpoint stays unchanged. Export provenance explicitly records incomplete
  finite search/refinement. Both campaign publication pointers select the result
  for subsequent extensions. Repeating the same explicit refinement resumes its
  original cursor; changing its mathematical recipe starts from the retained
  publication instead.
- Extension refuses to silently abandon an unfinished compatible current-scope
  refinement. It asks for explicit finalization first. This is a metadata check,
  not an automatic expensive refinement step.

The high-level finalization command intentionally requires a stopped refinement
for the **current** input scope. An older failed refinement whose active phase
has already been superseded requires explicit native export and authenticated
republication against the new completed scope; it is not silently installed as
the latest artifact. In particular, this change does not mutate the already
running THE_ONE extension or its frozen launcher.

## Cost and validation boundary

Export retains the native sparse matrix, not merely a compact list of final
substitutions. It cold-loads and packages that state; an extension which changes
the finite basis may need to replay stored rows. The saving is avoiding new
relation discovery, not a claim of free publication or faster campaign solves.

Validation evidence is recorded in `TMP/refinement-retention-20261011/`.
Native tests cover cap increase/removal without lost rows, partial-export
portability, guard/provider preservation, tamper rejection, pending-rebuild
interruption/resume, source locking, and extension reuse. Python tests cover
supervision, both publication pointers, original-cursor resume, recipe changes,
explicit zero budget, and extension safeguards. The actual failed checkpoint
is copied into that private evidence directory for export/reload checks;
production is never an output target. No full 600,250-row search rerun is part
of this bounded implementation task.

## Final release results

All 89 focused native tests passed: 21 core terminal-relation tests, 48
application master-reduction tests, 16 CLI/master-table tests, and 4 artifact
inspection tests. All 129 Python wrapper/phase/extension/dashboard tests passed.
The independent implementation and interpretation audit found no remaining
blocker. Release compilation took 3m29s; compilation is excluded below.

| Private actual-checkpoint operation | Result |
|---|---:|
| Export wall time | 73.992 s |
| Export user / system CPU | 62.870 / 10.441 s |
| Export peak RSS | 12,495,396,864 bytes |
| Fresh-process cold native reload | 68.856 s |
| Raw / remaining / eliminated terminal keys | 939 / 908 / 31 |
| Preserved source rows / columns | 273,492 / 575,189 |
| Preserved nonzeros | 5,055,843 |
| Newly searched source rows | 0 |

The source checkpoint and exported native state match byte-for-byte. All private
source-file hashes remain unchanged after cold reload. Two independent exports
also have identical collection payloads. Both `finite_search_complete` and
`refinement_complete` remain false: a usable retained refinement is not a claim
that the unfinished search has completed. Peak export memory is higher than
the original sparse search because export materializes the portable collection;
this task does not claim zero-cost retention or faster symbolic solving.

The validated frozen executable is
`TMP/refinement-retention-20261011/rustred-refinement-retention`, SHA-256
`fe3f1d9167e931d9179240a61202f0c6340252b93aae9a484133d0cfe1a46050`.
The same executable is installed at `target/release/rustred`.

The private export was run with:

```bash
taskset -c 48-55 TMP/refinement-retention-20261011/rustred-refinement-retention \
  walk-finalize-refinement \
  --refinement TMP/refinement-retention-20261011/failed-source \
  --directory TMP/refinement-retention-20261011/actual-partial-export-validated \
  --threads 8 \
  --events TMP/refinement-retention-20261011/actual-partial-export-validated-events.jsonl
```

The cold-load command used the same source/output and `--resume`, without a
new events file. Inherited license settings were left unchanged; nested
Rayon/OpenMP/BLAS thread counts were capped at one. Measurement and payload
comparison receipts are `actual-partial-export-validated-receipt.json` and
`actual-partial-export-repeat-comparison.json` in the evidence directory.
Native test logs are `core-tests.log`, `app-tests-final.log`,
`cli-tests-final.log`, and `inspect-tests-final.log`; Python logs are
`python-wrapper-tests.log`, `python-phase-tests-final.log`,
`python-extension-tests.log`, and `python-dashboard-tests-final.log`.

The public command for a stopped refinement of the current scope is:

```bash
python -B examples/python/saved_campaign.py finalize-refinement \
  --campaign "$RUSTRED_CAMPAIGN" --executable "$PWD/target/release/rustred"
```

Subsequent `extend` uses that retained publication. Normal `refine` remains
explicit; `--max-relation-columns 0` clears a previously stored optional cap.
The original THE_ONE campaign was not modified, stopped or resumed. In
particular, this private test does not install its older source into the
already-running higher-rank extension.
