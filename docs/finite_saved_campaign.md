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

The example `examples/input/finite_campaign/high_memory.json` selects32 workers
and a300GB optional resident-Work grant. It is a high-memory incremental-run
profile, not a claim that R6 fits or closes. Use it only with an appropriate
process-tree RAM ceiling and available host memory. Its generous operational
limits remain inside the current portable-reader bounds; those bounds are
described below. Small machines should use a smaller explicitly reviewed policy.

```bash
python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN" --engine finite-demand \
  --executable "$RUSTRED_EXECUTABLE" --finite-policy "$RUSTRED_POLICY" \
  --cpus "$RUSTRED_CPUS" --max-memory-bytes "$RUSTRED_RAM_BYTES" \
  --host-memory-reserve-bytes "$RUSTRED_HOST_RESERVE_BYTES"
```

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

The current format also has fixed metadata/decoder and portable-publication
ceilings. A higher-rank run can reach one before its RAM ceiling. Such a refusal
does not discard the prior committed checkpoint, but resolving a fixed-format
limit can require a software change rather than a larger RAM setting. The
completed rank-one measurements do not establish capacity or completion time
for rank six.

The principal progress bar counts required query rows closed in the **committed
scope**. Current-window scheduled/expanded nodes are displayed separately and
do not count as completed starting requests. Empty recorded rows can be closed
without native reductions. No ETA follows solely from either counter.

Before the native dashboard starts, the launcher reports startup phases and
elapsed time on stderr (input checks, executable freezing and scope validation).
This is activity reporting, not a percentage-complete estimate. Interactive
runs then hand the terminal to the native solve dashboard, followed by the
separate **Artifact publication** dashboard. Non-interactive runs keep bounded
periodic log messages. `inspect --format json` and dry-run output are unchanged.

The terminal supervisor connects the native cursor-query input/output as well
as its dashboard output to the same terminal; structured solve events and
results remain in their explicit files. In this interactive mode the guard's
`stdout`/`stderr` capture files are empty. Process-group isolation, Ctrl+C,
checkpoint requests and RAM supervision are unchanged. The October 16 launcher
fix works with the existing frozen finite-demand executable: no solver rebuild
or campaign restart is required to use it on the next invocation.

Publication reuses the existing normalization/refinement and portable
application format, with native finite-source provenance. It does not package
the large Work ledger into an ordinary application artifact. The finite engine
reports trusted generated provenance; the old symbolic graph's optional deep
replay mode is not available for this different checkpoint engine. Requesting
that unsupported mode is an explicit error, not a silent downgrade.

The finite backend and its emitted artifact must still pass the relevant
application tests before claiming full Vakint coverage of a physical numerator.
