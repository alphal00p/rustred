# Dependency-closure progress for shared saved-owner campaigns

## What the counters mean

An initial domain is a set of integral indices, not one integral and not one
new IBP rule. Local inspection classifies that domain and emits obligations
for right-hand-side successors and routing. Publishing that inspection does
not mean its successors are resolved.

The requested progress bar counts **initial domains whose reachable dependency
work is closed**. A separate plain counter retains initial publication. Across
all discovered domains, the display distinguishes recursively closed domains,
still unresolved domains, local native completions and unpublished queue work.
The denominator for all discovered work may continue growing; neither count
is a forecast of remaining wall time.

For example, A depends on X and Y, while B depends on Y and Z. After X and Y
are closed, A can close while B remains pending on Z. Resolving shared Y must
benefit every dependent root, not only the first root to discover it. A
deduplicated successor or a reuse of an initial helper remains a dependency;
it is not evidence of completed coverage by itself.

The actual 134 helper-first input queries coalesce into 67 admitted initial
domains. The progress bar counts those 67 domains. It conservatively follows
the broader representative when a narrower input was merged into a helper;
it does not claim the earliest possible completion time for every narrower
original request.

## Scope, cycles and authority

The intended criterion is finite dependency-work coverage: all reachable local
inspections have finished without errors/frontiers and every emitted obligation
has been accounted for. Closed groups of mutually dependent *abstract domains*
must be considered together; a cycle with an unfinished outgoing obligation
must remain unresolved.

This is the same scoped coverage objective as the existing global successful
worklist exhaustion, made visible per root. It is not a new independent proof
of integral-level strict descent, numerical reduction termination, arbitrary
family completeness, or minimality of masters. `family_closure_claim` remains
false. An unsupported monitoring path or absent historical dependency data
must display unknown, not zero and not a substituted publication fraction.

The implementation stores deduplicated parent-to-successor edges in the native
walker. A reverse traversal from locally unfinished or failed nodes marks all
ancestors that must remain open. The complement is the recursively covered set,
including sealed cycles with no unresolved outgoing obligations. Refreshes run
no more often than every five seconds and back off after expensive scans; a
dirty snapshot is a conservative lower bound. Refresh time, edge counts and
logical storage estimates are exposed for monitoring the overhead. The storage
estimates are not process RSS and exclude allocator overhead.

Ordered and Ready publication persist this graph in CP3 and CP4 respectively.
Checkpoint recovery validates the graph against publication records, aliases
and initial-overlap anchors. Owner-batched execution currently reports the new
counts as unavailable because its bucket-local domain IDs do not form this
shared graph. This limitation does not affect the recommended shared campaign.

## Existing runs and the clean restart

Old checkpoints retain local outcomes and some delegation relationships but
not the complete parent-to-successor graph. Reconstructing a reliable per-root
history would require replaying previous inspections, potentially millions of
them. They also bind their original executable bytes. This change therefore
uses a fresh campaign/checkpoint format instead of silently migrating old
progress or presenting guessed closure counts. Old executables and checkpoints
remain usable together at their original paths; new monitor readers can show
their existing counters but report recursive closure as unavailable.

The user explicitly approved starting over and clearing the active campaign
directory. The running helper-first campaign was stopped cooperatively and
saved generation 5 before exiting: 1,687,139 native inspections completed and
2,612,542 domains pending at that stop. All three old campaigns were then moved
to the untracked recovery archive
`TMP/retired-campaigns-20260925.UtI4ay/`, leaving `campaigns/` clear. No input
or checkpoint bytes were rewritten. This is recoverable removal rather than
permanent deletion; the archive README describes restoration to original paths.
The saved rule inputs remain reusable without IBP regeneration.

## Validation record

The full `rustred-app` release unit suite passes: **687 passed, zero failed,
four ignored**. This includes the seven new coordinator integration tests,
Ordered/Ready checkpoint recovery, subdivision and native dashboard checks.
All 110 Python steering/monitor tests pass, including 24 monitor-specific tests
for old/missing history, conservative stale counts, freshness, TTY colors and
plain output. The nine optimized native tracker tests also pass, including a
reference reachability comparison across incremental deterministic graphs,
shared descendants, cycles with unfinished exits, failed leaves, corruption and
checkpoint replay. These focused checks do not replace coordinator integration
tests or the complete FG control.

The optimized-library preflight completed the full FG control with 124/124
initial domains and 98,643/98,643 discovered domains recursively covered,
539,441 retained edges and zero pending work/frontiers. A cooperative native
stop-file request (the same mechanism used by Python's Ctrl-C handler) saved
after 18,208 native inspections with 6,047 domains pending and 106,891 edges.
A fresh process resumed CP3 and reached the same 98,627 native inspections,
98,643 domains, 539,441 edges and graph revision as the uninterrupted run.
This preflight used the freshly compiled Cargo release library with a temporary
CLI link while the full test binary compiled; final packaged-binary timing is
reported separately below, not inferred from this contended preflight.

The independent audit compared every normalized domain record, query mapping
and responsibility ledger against the old executable, and independently decoded
the checkpoint graph. All 109 required alias/partial-anchor links were present.
Its reverse-reachability calculation confirmed the final counts; the checkpoint
had an older conservative snapshot of 110/124 closed roots and 72,003 closed
domains, demonstrating meaningful partial progress before final refresh.
Cold resume reproduced the exact edge sequence, node state, graph revision,
all final integer counters and all normalized domain records of the uninterrupted
new run. Against the old run, the only top-level integer difference was 38 fewer
containment checks out of approximately 169.5 million; no geometry, rule work,
diagnostics or closure obligation changed. Speculative admission diagnostics
are not a byte-identity or mathematical-result claim.

## Matched final release control

After compilation finished, the final packaged CLI was compared in an
old/new/new/old sequence: `baseline-2`, `tracked`, `tracked-2`, `baseline-3`
under `TMP/dependency-monitor.EdVwnw/`. Each fresh process used the same
helper-first FG input, six workers on CPUs 64–69, Ordered/H256 scheduling and
unchanged saved rules. Every run completed all 98,643 domains, 98,627 native
inspections and 7,975,678 native operations, with zero queues/frontiers.
There was no new IBP generation, compilation inside the timing boundary, or
supervisor deadline. Filesystem caches were not flushed.

| Measurement | Previous monitor | Dependency monitor | Change |
|---|---:|---:|---:|
| Median native traversal | 11.761 s | 12.053 s | +2.48% |
| Median full command wall time | 16.479 s | 17.688 s | +7.34% |
| Median waited child CPU time | 43.082 s | 43.990 s | +2.11% |
| Maximum single-process peak RSS | 1.148 GB | 1.170 GB | +1.92% |
| Final checkpoint size | 165.43 MB | 197.45 MB | +32.02 MB |
| Final report size | 208.25 MB | 211.51 MB | +3.26 MB |

Medians use two completed repetitions per binary; peak RSS is the maximum of
the two, not the sum of processes. Native traversal includes admission, walking,
checkpoint/report preparation and queue cleanup; full command time also
includes native-supervisor setup, owner import, output and artifact packaging.
Post-run independent audits are excluded. This is a small local overhead
control, not a statistically established bound or five-loop timing prediction.
The older initial baseline and compile-contended preflight are not included in
the table.

The new runs retained 539,441 unique dependency edges and approximately 27.20 MB
of logical graph capacity. Their four graph refreshes consumed 4.36–5.37 ms in
total; refresh scratch was approximately 0.89 MB. The full overhead includes
edge bookkeeping and the larger checkpoints/output, not just these scans.
FG exercises Apply, reuse, aliases and partial anchors but has no Route jobs;
Route-path integration is covered by native tests, not by this timing claim.

Executable identities:

- Previous: `030e09ef661cc69b5acc0501376b8174a518eb95cb0bf0f656835e84613f6c60`.
- New Cargo release: `32fdec098a57dd0c51aef71c01c260fb5cf7d0954b0992db08bb0f955aed358a`.

Scoped formatting and staged diff checks pass. Repository-wide formatting still
reports pre-existing import-order differences in unrelated files; those files
and the user's unrelated work were not changed for this follow-up.

## Prepared handoff

`campaigns/five-loop-dependency-closure` is prepared, **not launched**. It retains
the same 67 saved owners and 134 explicit helper-first queries, freezes the new
release executable above, and requests ten workers on CPUs 0–9 with a 750 GB
RAM ceiling, 5% save-and-stop margin and hourly checkpoints. It keeps the shared
Ordered/H256 policy; pruning, physical subdivision and independent-owner
execution remain off. There is no cumulative work or wall-time cap.

No recompilation or rule regeneration is required to start this local snapshot:

```sh
cd /common/dev/rustred
export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-dependency-closure --start
```

After a clean pause, add `--resume`. The save-and-stop RSS threshold is
712.5 GB before any stricter host-headroom admission. Ten workers are the user's
chosen resource budget, not a new claim of optimal utilization. The broader
goal remains paused until further instructions.
