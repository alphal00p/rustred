# Ready-ticket publication: implementation and validation

## Motivation and scope

The live five-loop campaign remains on its original frozen Ordered executable.
A short [read-only profile](five_loop_slow_inspection_parallelism_2026-09-24.md)
found 159 completed inspections waiting behind one expensive source while only
about 1.6 of the 50 reserved cores were busy. A geometric split of an actual
saved source produced at most a single-run 1.128x native-only speedup; it did
not establish a production-walker gain. The next experiment removes the
scheduler-imposed global publication barrier, including within one owner.

This changes task scheduling and persistence, not IBP formulas, applicability,
guards, allowed starting integrals, or descendant retention. It introduces no
algebra kernel and no topology- or loop-count-specific implementation.

## Shared coordinator, independent streams

The opt-in `Ready` policy reuses the existing global queue, immutable native
programs, inspector pool, preparation helpers and exclusive admission path.
It is not another independent solver instance per sector.

1. The ledger reserves at most H actual native-parent responsibilities. Reserved
   or started duties cannot be transferred away. Forward delegation of an
   unreserved duty still retains explicit responsibility for its representative.
2. The coordinator dispatches eligible reserved work into the bounded native
   pool. H is an outstanding-work credit limit, not a fixed ID-distance window
   after the oldest unfinished source.
3. Round-robin polling accepts one ready chunk at a time, preserving callback
   order within each source. Existing speculative lookup is revalidated by the
   same exclusive admission transaction before effects become authoritative.
4. Each physical ticket owns its own diagnostics and accepted replay prefix.
   Switching the active source moves that context rather than cloning events
   or algebra. Ready initially rejects physical subdivision.
5. An accepted native `Finished` releases a credit. A later completed source can
   therefore make room for more native work while an earlier source remains
   active. The smallest unpublished ID is only a watermark, not the completed
   count; completed holes and delegated publications are explicit.
6. Failure or cancellation never turns an unfinished representative into a
   successful result. All accepted diagnostics remain attributable to their
   source; a genuine late worker fault takes precedence over resumable pause.

This permits some extra overlapping work because reservation and admission
orders can change which covers are available first. Acceptance is measured
wall-time benefit with correct accounting and acceptable CPU/RAM, not zero
redundant computation. It does not guarantee fifty-core utilization: uneven
native costs and exclusive destination admission remain possible bottlenecks.

## Checkpoint and interface contract

Ready uses an explicit CP2 checkpoint marker and schema. A snapshot includes
the shared queue and responsibility ledger, publication holes, all accepted
stream prefixes, and their admitted effects. Resume normalizes unfinished
started duties for redispatch; it verifies and suppresses each previously
accepted prefix before admitting new callbacks. Missing or contradictory
context/publication metadata fails closed. Finished records plus active and
parked prefixes must reconcile to the accepted-event count.

The request binding includes publication policy and executable identity.
An Ordered checkpoint must **not** be attached to Ready or a newly built
executable. The live campaign and its frozen restart command remain untouched;
testing a different policy requires a fresh campaign. Rule `.rrbin` artifacts
are unchanged and are reused without generating IBPs again.

The Rust policy, CLI and Python steering expose the same opt-in:

```text
--follow-successors --publication-policy ready --transfer-unreserved-lookahead 256
```

Unlimited containment comparisons remain a prerequisite of responsibility
transfer. Checkpoint/resume is supported; physical subdivision is rejected.
The production launcher accepts `--publication-policy ready` only as a frozen
initial choice. Omitted policy retains Ordered for a new campaign and retains
the exact saved policy on resume. An explicit resume mismatch is rejected.

Ready's final flat-ID walk receipt is `rustred.owner-domain-walk.json.v5`.
OwnerBatched's composite-ID v4 and Ordered's existing schemas are unchanged.
The CLI preserves actual Ready final, paused and preparation-error schemas;
a requested policy is not evidence that traversal started. IDs, cover
fragmentation and native work counts can vary with scheduling. These are not
promised byte-identical across worker counts.

The Python dashboard can now display completed native inspections waiting for
publication separately from active/blocked workers and measured busy cores.
This optional field is omitted when reading older supervisor status. No live
process has been changed to enable it.

## Validation status

Independent source review passes for the ledger, shared coordinator,
checkpoint accounting, CLI/Python steering and monitor field. It found and
prompted fixes for ambiguous missing-versus-false alias bits, missing CP2 stream
metadata, receipt schema collisions and insufficiently decisive late-fault
test assertions.

The fast Python gate passes 15 monitor/launcher, 17 domain-steering and 21
supervisor tests. The release application, CLI and integration gate passes
699 tests with one existing ignored test and no failures. This includes real
native serial on-disk resumes, same-owner refill beyond the reservation bound,
multiple accepted prefixes in shared-coordinator tests with synthetic workers,
changed-prefix rejection, late native failure after
cancellation, and ledger/codec validation. An independent raw audit reconciles
the results in `TMP/ready-publication-gate.ermstd/app/`.

### Completed four-loop controls

All eight fresh processes in one predeclared alternating Ordered/Ready rotation
complete. They use identical saved rules, query objects and ordering within
each pair: all 900 owners, 1,800 original-plus-auxiliary queries, W6 on CPUs50–55,
H256, hourly checkpoints and unlimited cumulative work. There is only a RAM
guard; no IBPs are regenerated. The live five-loop process continues on its
separate CPUs0–49. The tested executable SHA256 is
`c6ac466479dda82e0f64dd9b059ee99d1d0e51f169658092eab0eba003251d43`.

Each cell below is **Ordered / Ready**. Times are seconds; RSS is decimal GB.

| Family | Native traversal | Whole-command wall | Whole-command CPU | Peak sampled RSS |
|---|---:|---:|---:|---:|
| FG | 16.996 / 18.798 | 22.10 / 24.08 | 56.18 / 54.22 | 1.168 / 1.149 |
| BMW | 38.826 / 43.863 | 44.09 / 50.10 | 123.25 / 117.45 | 1.828 / 1.786 |
| H | 15.007 / 15.169 | 20.09 / 20.08 | 49.65 / 47.97 | 0.666 / 0.666 |
| X | 38.011 / 36.688 | 46.10 / 44.10 | 113.92 / 114.18 | 1.253 / 1.228 |
| Sum; RSS maximum | 108.840 / 114.518 | 132.38 / 138.36 | 343.00 / 333.82 | 1.828 / 1.786 |

Native traversal excludes preparation but includes its final durable checkpoint.
Whole-command measurements enclose the small resource supervisor, whose
two-second polling quantizes exit detection. These are shared-host measurements,
not filesystem-cache-cold runs or confidence intervals.

Ready uses 318,843 native inspections versus Ordered's 317,608, and schedules
330,304 obligations versus 329,982. All descendants are retained. Independent
record-by-record audits pass for explicit IDs, forward alias chains, initial
anchor dependencies, input coverage, counters, checkpoints and complete
worker/ledger drain. Every run has zero frontiers, unsupported transitions,
native problems and pending obligations. Optional-original refusals remain
0/3/4/17 per family in both policies. This is local exhaustion of the stated
entry workloads, not unrestricted family closure.

Ready is **5.22% slower** in summed traversal with 2.68% less whole-command CPU.
X's modest improvement does not establish a general gain. Keep Ordered as the
default; these controls do not measure the long-headed five-loop workload.
Raw evidence and the independent audit are in
`TMP/ready-four-loop-control.GA8zxH/`.

### Initial four-loop recovery attempts: inconclusive

The separate BMW recovery attempt completed normally before observing two
simultaneously accepted source prefixes. No interrupt or resume occurred.
Its sampled maximum was one prefix, despite publication holes reaching 296;
shorter unobserved overlaps remain possible. This is **inconclusive**, not a
successful repeated-resume test. Its original receipts remain intact.

The second correctness-only fixture preserves X's exact 656 query objects and
promotes three existing high-output anchors together with each one's preceding
finite entry query, leaving the remaining relative order and all mathematical
settings unchanged. Keeping each finite query before its broad anchor preserves
distinct initial obligations without weakening the input assertions. Independent
preflight passed, but this fixture also completed normally before the required
trigger: 35 progress samples contained at most one accepted source prefix, with
up to 89 published holes. It completed 46,827 native inspections and 355 delegated
obligations with zero frontiers, errors, unsupported transitions or pending work.
The guard measured 42.024 seconds and 1.245 GB peak RSS. There was no interrupt,
resume or retry. Evidence is retained in `TMP/ready-x-resume-control.nVaRU6/`.
This is a second inconclusive recovery fixture, not a failed reduction or a
successful multi-prefix resume, and is excluded from the timing comparison.

At this point neither repeated interruption/resume nor private
corrupted-checkpoint rejection had been exercised. The later five-loop
partial-recovery test below supplies genuine multiple-prefix restore evidence;
it does not supply full-completion equivalence or negative-corruption rejection.

### Ordered old/new binary diagnostic

The audit also compared the new binary's Ordered controls with the earlier
frozen executable on the same inputs. Aggregate native work counters match
exactly, yet summed traversal increased from 89.443 to 108.840 seconds and
whole-command CPU from 283.95 to 343.00 seconds. These noncontemporaneous runs
cannot isolate code, build settings or host conditions. Both binaries embed the
same Rust compiler version, but complete matching build provenance is unavailable.
Do not attribute this solely to the Ready implementation or dismiss it as noise.

A fresh four-run, Ordered-only diagnostic uses FG old/new followed by BMW
new/old on the same six cores and unchanged inputs. All four completed and
passed independent full-record audit. FG traversal is 12.952/15.866 seconds;
BMW is 32.283/38.533 seconds. Summed traversal increases 20.26%, while
whole-command CPU increases 20.47% (146.64 to 176.65 seconds). Native work and
coverage match exactly. The extra 0.291 seconds of checkpoint writes do not
explain the 9.164-second traversal gap. Raw receipts are in
`TMP/ordered-binary-compare.eUDPn6/`.

Retained Cargo fingerprints bind exactly to both executable SHA256 hashes.
Their compiler, features, rustflags and configuration fields match, but their
profile hashes differ. The newer build used `CARGO_PROFILE_RELEASE_LTO=off`.
This is not equivalent to Cargo's default `lto=false`: the default retains
within-crate thin LTO, while `off` disables it, as described in the
[Cargo profile reference](https://doc.rust-lang.org/cargo/reference/profiles.html#lto).
The old numeric profile hash does not alone reconstruct its complete original
build environment. A same-source build with explicit `LTO=false` has now
completed successfully: 742.97 seconds under the resource guard, 11.877 GB peak
RSS, with unchanged source and old frozen binaries. Its executable SHA256 is
`8f725507122332a05f68b308d08cd2c5bf9db2932f403cde8ebcba8373cdf106`.
The resulting Cargo profile fingerprint **matches the older binary exactly**;
this removes the demonstrated profile mismatch from the next paired timing
comparison. It does not by itself establish that performance is restored.
Receipts are in `TMP/ordered-profile-build.8HxlqF/`.
Both Ordered binaries retain checkpoint schema 1; schema 2 is specific
to Ready. No live five-loop process or configuration is changed.

The normalized four-run comparison has also completed and passed independent
full-record audit, on the same inputs and CPU allocation. It starts only after
the live generation-6 checkpoint finished; all eight before/after snapshots
confirm the same saved generation with no active write.

| Family | Old / normalized-new traversal (s) | Change |
|---|---:|---:|
| FG | 14.268 / 14.853 | +4.10% |
| BMW | 35.326 / 35.456 | +0.37% |
| Sum | 49.593 / 50.308 | +1.44% |

Whole-command CPU is 161.88/165.79 seconds (+2.42%), and whole-command wall is
60.53/62.38 seconds (+3.06%). Preparation is 2.823/2.734 seconds; included
checkpoint writes are 3.075/3.344 seconds. All native integer work counters,
required inputs and complete native/alias/anchor discharge match exactly, with
zero frontiers, errors, unsupported transitions or pending work.

The large 20% executable difference is **not reproduced** with matching profiles.
The residual slower observations are retained; one shared-host rotation proves
neither zero regression nor equal-or-better performance. The correct outcome is
to publish the audited opt-in as experimental, not to claim a Ready speedup or
change the production default. At this measurement checkpoint real W6
multi-prefix recovery had not yet been exercised; see the later partial test
below. See `TMP/ordered-normalized-compare.9uPavj/RESULTS.md` for all receipts and
timing boundaries.

Ready remains experimental, with no five-loop completion or speedup claim and
no change to the live campaign.

### Five-loop partial recovery: two fresh-process restores pass

The independently audited experiment in
`TMP/ready-five-loop-prefix-v3.k1nLpy/` uses the normalized frozen executable,
six disjoint cores, the same 67 saved owner programs and three explicitly
supplied queries for one owner. It is not the full 134-input production campaign.
All descendants are retained, with RAM-only protection and no elapsed/work cap.
The experiment deliberately requests cooperative checkpoint stops; it does not
run this workload to exhaustion.

| Exact unfinished source | First paused save | After first restore | After second restore |
|---|---:|---:|---:|
| Source0 accepted-event cursor | 203,019 | 370,146 | 423,269 |
| Source2 accepted-event cursor | 215,283 | 377,660 | 435,284 |

Both sources remain unfinished at each save. A genuinely finished source1
stays published ahead of them, while the contiguous publication watermark
remains0. Each restore actually consumes a saved checkpoint containing multiple
positive partial-source cursors; increasing a global event counter alone was
not accepted as replay evidence. Independent raw checkpoint scans reconcile
every accepted event and confirm preservation of all previously published
records, ledger entries and history.

All three native processes stop cleanly with paused exit4. The final checkpoint
is207,199,050 bytes, with118,708 published records (78,513 native completions),
4,886,229 accepted events and239,665 pending obligations. Peak sampled RSS is
6.69 GB. No resource stop, non-cancellation fault, recorded frontier or unsupported
transition occurred. This establishes **partial multi-source recovery**, not
final completion/equivalence, negative-corruption rejection or a speedup.

Two earlier harness failures are retained, not counted as recovery tests:
the first misread nested bootstrap metadata before any inspection; the second
sent SIGINT directly to the native CLI and therefore terminated it. The CLI's
supported cancellation interface is its `--stop-file`; the production Python
supervisor handles Ctrl-C by writing that file. The successful test uses this
documented cooperative boundary. It is not a direct-native-SIGINT test, and
neither correction changed the Rust solver or the live production process.

The remaining full-drain comparison is still open. Keep Ordered as the default
and do not promote Ready solely from this partial recovery success.

### Missing parked-prefix rejection passes

The subsequent independent negative test in `TMP/ready-negative-prefix.faPZyj/`
uses a separate copy of the genuine generation5 checkpoint. It removes only
parked source2's accepted prefix (377,660 events), preserving the completed
records, ledger and original total of2,781,287 accepted events. The private
container's byte count and BLAKE3 are updated, so a checksum failure cannot
masquerade as semantic validation. The source checkpoint remains unchanged.

The frozen CLI rejects this private input with exactly
`ready checkpoint accepted-prefix accounting mismatch`, before importing owners
or doing native inspection/publication. Independent review checks the raw error,
progress, unchanged originals and valid private container hash. The guard records
4.004 seconds, exit4 and no resource stop/error/forced termination. A precreated
cooperative stop file was only a fail-safe against unexpectedly starting a walk;
cancellation alone was not an accepted test outcome.

This closes the missing-parked-context rejection test, not arbitrary corruption
coverage or full-completion equivalence. No solver, production checkpoint or live
configuration was changed.
