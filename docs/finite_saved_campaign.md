# Exact finite-demand saved campaigns

The opt-in `finite-demand` engine enumerates the recorded finite starting
domains in bounded windows, traces their exact dependencies using existing
owner rules, and shares completed dependencies between windows. It does not
generate new IBPs, restrict descendants to the starting bounds, or turn an
unresolved endpoint into a master.

This engine uses a **fresh checkpoint namespace**. Existing owner programs,
routes and original rank-source templates are reusable; an old CP6 traversal
ledger is not imported. Keep old campaigns and publications separately.
Successful finite traversal establishes completion of the recorded input union,
not unrestricted family closure, master independence or the availability of
numerical master values.

## Start, resume and extend

First stage the saved inputs with the existing rank-campaign preparation tool.
Retain its original query attachment: later rank extensions must use the original
templates, not widen a truncated rank-zero document. Empty low-rank intersections
remain recorded and activate when their requested geometry becomes nonempty.

The first run explicitly selects the engine, executable and operational policy:

The example `examples/input/finite_campaign/high_memory.json` selects 32 workers
and a 300 GB optional resident-Work grant. It is a high-memory incremental-run
profile, not a claim that R6 fits or closes. Use it only with an appropriate
process-tree RAM ceiling and available host memory. Aggregate resource stops
are optional: `null` means no requested cap. Buffer sizes remain explicit.
Small machines should use a smaller explicitly reviewed policy.

```bash
python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN" --engine finite-demand \
  --executable "$RUSTRED_EXECUTABLE" --finite-policy "$RUSTRED_POLICY" \
  --cpus "$RUSTRED_CPUS" --max-memory-bytes "$RUSTRED_RAM_BYTES" \
  --memory-policy process-limit
```

New finite launches default to `process-limit`. For example,
`--max-memory-bytes 600000000000` retains a 600 GB process-tree RSS ceiling and,
with the default 5% guard margin, a 570 GB cooperative-stop threshold. Host
`MemAvailable` is telemetry and does not reduce that ceiling. This is an allowed
process limit, not a host memory reservation. To request admission based on
available host memory instead, explicitly use `--memory-policy host-aware`
with `--host-memory-reserve-bytes BYTES`; that mode may lower the effective cap.
The selected memory policy is saved for later runs.

Both saved-campaign engines remain available in the current executable. The
symbolic-domain `legacy` lane (including Epoch/CP6) is not deprecated or replaced
by this finite engine. It can compress many integer inputs into one symbolic
region and may be considerably faster on some higher-rank scopes. Finite demand
instead visits concrete inputs and their exact descendant Work; it trades that
compression for a bounded starting enumeration and exact shared reuse.
Choose/retain the lane explicitly; do not feed one engine the other's checkpoint.

For an existing **legacy** campaign, the same process-limit guard is opt-in:

```bash
python -B examples/python/saved_campaign.py run \
  --campaign /path/to/legacy-campaign --resume \
  --memory-policy process-limit --max-memory-bytes 600000000000 \
  --swap-growth-stop-bytes-per-second 0
```

This resumes its frozen legacy solver, not finite demand. It retains570/600GB
RSS soft/hard guards at the default5% margin; host availability is telemetry
only. Unlike a finite campaign's saved operational settings, this legacy override
is per invocation: repeat the policy flag on later `run`/resume commands. No
host-memory reservation is promised, and OS/cgroup limits still apply.

Worker counts are operational, not part of the saved mathematical scope.
For a later extension, `--workers N --cpus CPUSET` freezes a new policy with
that worker count while retaining every other policy field. The CPU set must
contain exactly N permitted CPUs. The former64-worker library ceiling is gone;
positive counts are checked against actual CPU availability and the Symbolica
license. On multi-threaded CPUs, select distinct physical cores deliberately;
the generic affinity check does not distinguish physical cores from SMT siblings.
The window partition remains unchanged. An explicit `--executable NEW_BINARY`
selects the new build for that launch and later invocations; do not overwrite an
in-use content-addressed executable or edit the campaign's frozen `bin/` files.

`--windows-per-batch N` groups up to N **existing** prefix windows into one
shared parallel trace and one atomic commit (native policy field
`max_windows_per_batch`, default1). It preserves the existing per-window
partition, all worker parallelism, shared completed-Work index, and in-flight
descendant deduplication. Several rows may share a group; repeated starting
integrals count as input occurrences but are traced only once per native group.
This setting can change on cold resume without discarding completed work.

The dashboard displays current group size and input count. Chase time and
convergence refer to the entire active group. Only a fully successful group
advances its saved cursors; interruption or failure leaves the preceding
checkpoint valid and the unfinished group must be retried. Larger groups reduce
commit frequency but may increase memory and the time before the next durable
boundary. The process RAM guard still applies. Initial admission of the grouped
starting inputs remains serial; grouping is not a guarantee of full CPU usage.
Use `--windows-per-batch 1` to retain the ungrouped finite schedule. The separate
legacy symbolic-domain/Epoch lane is unaffected.

The optional resident completed-Work index uses compact hash slots and chunked
coordinate storage. It retains exact phase/powers identity (Apply owners are
validated against positive-power support) without a separate allocation per
record. This in-memory layout is independent of the saved format; old finite
checkpoints can be resumed without conversion. Allocation and memory accounting
remain before checkpoint adoption, and the process RAM guard remains in force.

After the last window, cleanup is explicitly shown as **Solve complete ·
releasing working memory**. The completed checkpoint is already durable, but
the portable artifact is not yet published. Publication does not hydrate the
optional completed-Work membership index: it authenticates the source, scope,
cursors and raw inventory through the existing publication path. This avoids
rebuilding and discarding the solve cache just to extract terminals and rules.
It is not an independent replay or an exhaustive reread of unused Work pages.
The saved solving cache allowance is unchanged for the next rank extension.

These commands launch work; they are not preparation-only commands. The saved
engine and policy are reused by later invocations:

```bash
python -B examples/python/saved_campaign.py run --campaign "$RUSTRED_CAMPAIGN" --resume
python -B examples/python/saved_campaign.py extend --campaign "$RUSTRED_CAMPAIGN" --rank 1
python -B examples/python/saved_campaign.py extend --campaign "$RUSTRED_CAMPAIGN" --rank 2
```

Run each extension after the preceding stage finishes. Omitting
`--max-power-difference` preserves the existing D cap. For the five-loop D≤10
ladder, increasing R does **not** require replacing D by `10+R`. Coordinate,
positive-power and other topology-specific bounds in the original templates
still apply: R/D maxima are not blanket admission of every integral beneath them.

Ctrl+C requests cooperative stopping. Only completely traced, durably committed
windows advance the saved cursor; an unfinished window is recomputed on resume.
If interrupted before the first native store exists, the reported
`run --repair-preparation` command explicitly repeats preparation. It never
deletes or resets an existing checkpoint. Publication interruption preserves
the previous completed artifact and any already committed resumable phase
state; stopping before phase staging may leave no new phase state yet.

## Inspect and manually refine

```bash
python -B examples/python/saved_campaign.py inspect --campaign "$RUSTRED_CAMPAIGN"
python -B examples/python/saved_campaign.py inspect --campaign "$RUSTRED_CAMPAIGN" --format json
```

Inspection combines native committed progress with the last published artifact,
when available. It distinguishes the requested scope from an earlier publication
and shows installed executable rule records, encountered rules, raw terminals
and the retained candidate-master basis. No publication means pending, not a
fallback to the old CP6 inventory reader. Terminal counts are not independence
claims. Use `--format table` to request the native table explicitly.

Successful stages publish automatically; master refinement remains manual:

```bash
python -B examples/python/saved_campaign.py refine \
  --campaign "$RUSTRED_CAMPAIGN" --seed-depth 1 --containing-sector-depth 1
```

The option is named **`--containing-sector-depth`**, not
`--external-sector-depth`. It adds IBP sources by promoting up to that many
inactive indices to +1. `--seed-depth` controls the additional finite seed
search, independently of the campaign's starting numerator rank. Neither depth
proves that the surviving basis is minimal.

If refinement was stopped before its finite search finished, retain its proved
relations before extending:

```bash
python -B examples/python/saved_campaign.py finalize-refinement --campaign "$RUSTRED_CAMPAIGN"
python -B examples/python/saved_campaign.py extend --campaign "$RUSTRED_CAMPAIGN" --rank 3
```

Completed and finalized exact terminal substitutions are retained by subsequent
publications. This does not imply a general speedup of dependency traversal.

## Memory, progress and publication boundaries

`max_resident_work_bytes` in the operational policy enables an exact in-memory
index of previously completed dependencies. Zero disables it. The index keeps
full integral coordinates, phase and owner identity; hashes alone cannot answer
membership. Restore hydrates a complete generation before the index may answer
negative lookups. New entries become available only after successful durable
commit. If the configured estimate is insufficient, execution falls back to the
existing exact paged lookup and reports the transition.

This is the closure walk's completed-Work index, not Vakint's coefficient-result
cache. It does not itself compute master coefficients or replace the existing
application caches.

This grant accounts for resident keys, estimated table capacity and specified
coexisting payloads; it is not an allocator-exact RSS limit. The process-tree
RAM guard remains authoritative. Operational grants can be explicitly adjusted
on resume without changing the mathematical scope; the window partition cannot.
Small benchmark policies must not be reused as production policies without
reviewing their file, page, directory, disk and per-window allowances.

Directory accounting uses the actual retained directory capacity, plus explicit
decode headroom. The previous accounting incorrectly added the directory's
permission cap; setting that cap to `null` could overflow during resident
initialization. The repaired path counts shared directory storage once and
includes both the old and new directory while a commit keeps them alive.
An uncapped allowance does not request an allocation of its maximum value.

The finite policy governs tracing **and** publication, refinement, inspection
and cold loading; a publication reader no longer silently imposes a smaller
4096-segment allowance. For aggregate counters and work/IO grants (runs, pages,
files, stored references, rows, source bytes, disk bytes and merge/read work),
an explicit `null` removes that cap; a nonnegative integer applies the user's
cap. This does not remove checked arithmetic, source/routing authentication,
native type/shape validation or the process-tree RAM guard.

For example, the relevant entries in `storage` may be:

```json
{
  "max_runs": null,
  "max_pages": null,
  "max_disk_bytes": null
}
```

These are excerpts, not a complete policy. The full high-memory example also
removes the corresponding raw-census/merge caps. Page chunk sizes, resident
cache sizes and frame/transient allocation budgets stay explicit numeric
settings. They control buffering and decoding, not allowed physics input.
Frames larger than the former64MiB ceiling are supported when requested;
`max_transient_bytes` must cover the largest encoded frame plus its decoder
budget (the larger of64MiB and that configured frame size). Raw decoding has
the corresponding `max_decode_transient_bytes` requirement. A small page/cache
setting is not an instruction to allocate the entire RAM allowance.
The high-memory example allows1GiB manifest/state frames with2GiB transient
decode headroom; these are maximum allowances, not upfront allocations.

Choose a changed policy explicitly on resume, without changing the window
partition:

```bash
python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN" --resume --finite-policy path/to/policy.json
python -B examples/python/saved_campaign.py inspect \
  --campaign "$RUSTRED_CAMPAIGN" --finite-policy path/to/policy.json
```

The chosen run policy is frozen and reused by later calls. An inspection
override is read-only and does not change it. Rust callers have explicit
`*_with_policy` inspection/loading/application entry points; standalone CLI
inspection/refinement accepts `--finite-policy`. Reader allowances come from
the caller, never from an untrusted artifact's self-declared grants. Very
large user grants can increase memory or IO cost; the rank-one measurements
do not establish capacity or completion time for rank six.

The principal progress bar counts required query rows closed in the **committed
scope**. Current-window scheduled/expanded nodes are displayed separately and
do not count as completed starting requests. Empty recorded rows can be closed
without native reductions. Neither counter alone provides a reliable completion
time; the input ETA below is only a rough extrapolation using elapsed time.

The table also shows committed input occurrences over the cached starting-input
total. Overlapping rank requests count separately; this is not a unique-integral
count. The total is summed once from admission's cached counts, while completed
windows and last-window new Work/reuse hits come from checkpoint metadata.
Heartbeats retain these summaries without rerunning native counts or scanning
the legacy closure ledger. CPU usage, process RSS, the requested cap and stop
threshold, and host available/total RAM remain visible; RAM uses decimal GB.

`Chase elapsed` is native elapsed time for the current finite window, separate
from overall process elapsed time. `Chase gap` is discovered minus expanded
Work, including active work. `Convergence · 5 min` and `Convergence · 30 min`
compare this gap to its recorded value that long ago:
`(gap_now - gap_then) / (gap_now + gap_then)`, or zero for a zero denominator.
Negative is green (shrinking); positive up to0.5 is yellow; above0.5 is red;
zero is neutral. These are backlog trends, not completion fractions or ETAs.
History warms up after each window/restart, is bounded to30minutes plus a sample,
and does not fabricate fresh observations from stale native snapshots. All
consumers receive the same `finite_chase` structured heartbeat fields.

`ETA · run average` is a deliberately rough remaining **solve** duration based
on the displayed committed input-occurrence count, not recent progress:
`elapsed * (total - committed) / (committed - initial_committed)`. Elapsed time
covers the whole native monitored invocation, including preparation/restore.
The initial checkpoint supplies the retained prefix; earlier ranks and earlier
invocations are not incorrectly counted as new throughput. `Inputs/s · run avg`
shows the corresponding average. Before new committed work exists the estimate
warms up; during a long uncommitted chase it can increase. Publication is not
included. Different inputs have very different costs, so this is neither a
reliable prediction nor an upper bound. Structured consumers receive the same
`finite_input_eta` value and its explicit estimate-only status.

The event journal is buffered in64KiB while still flushing every complete
record. Write/flush failures retain the existing cancellation behavior; this
does not defer checkpoints or change their durability contract.

Before the native dashboard starts, the launcher reports startup phases and
elapsed time on stderr (input checks, executable freezing and scope validation).
This is activity reporting, not a percentage-complete estimate. Interactive
runs then hand the terminal to the bordered, coloured native solve dashboard, followed by the
separate **Artifact publication** dashboard. Non-interactive runs keep bounded
periodic log messages. `inspect --format json` and dry-run output are unchanged.

The terminal supervisor connects the native cursor-query input/output as well
as its dashboard output to the same terminal; structured solve events and
results remain in their explicit files. In this interactive mode the guard's
`stdout`/`stderr` capture files are empty. Process-group isolation, Ctrl+C,
checkpoint requests and RAM supervision are unchanged. Dashboard errors are
printed below the final table, never across its last line. The initial October16
launcher-only repair needed no native rebuild; the subsequent configurable-limit
and table-renderer update requires the newly built executable.

Publication reuses the existing normalization/refinement and portable
application format, with native finite-source provenance. It does not package
the large Work ledger into an ordinary application artifact. The finite engine
reports trusted generated provenance; the old symbolic graph's optional deep
replay mode is not available for this different checkpoint engine. Requesting
that unsupported mode is an explicit error, not a silent downgrade.

The finite backend and its emitted artifact must still pass the relevant
application tests before claiming full Vakint coverage of a physical numerator.
