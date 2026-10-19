# Finite campaign post-chase monitoring

The finite-demand engine has two different kinds of progress. Exhausting a
window group's descendant chase is not yet a durable commit. The native
`finished` flag and zero chase gap describe that chase, while the required-row
bar advances only after the corresponding atomic checkpoint is adopted.

Large deltas can spend substantial time after the chase on key export/sorting,
index validation/staging, checkpoint pages and resident-index installation.
The October 19 dashboard exposes these operations independently.

## Reading the dashboard

- The top bar remains the number of required rows closed in committed scope.
- A **POST-CHASE** bar reports exact done/total records, pairs or pages for the
  current stage when that total is known. It does not enlarge input coverage.
- Sorting reports the number of comparisons performed. Its total is unknown,
  so the dashboard does not invent a percentage or extrapolated sort ETA.
- **Phase elapsed** measures this one operation, not the preceding chase.
- **Chase report age** is the age of the last chase snapshot. A finished chase
  is marked `done` rather than appearing to be a stale, still-running chase.
- **Phase report age** measures the age of the latest actual post-chase counter.
- **Monitor heartbeat** indicates that the independent display/resource
  sampler is alive. It is deliberately separate from native progress: a live
  monitor alone is not evidence that the solver is advancing.
- CPU, process RAM and host availability continue to be sampled during all
  these stages. Long allocations and metadata writes can have no useful
  intermediate work count; their phase elapsed/resource telemetry remains live.

The pipeline reports membership accounting, key packing, key comparisons,
sorted-order validation, resident delta checks, reservation, staging,
checkpoint-key validation, work/terminal/zero-page writing, metadata writing, atomic publication,
resident-index installation and batch cleanup. Not every phase lasts long
enough to appear in the one-second sampled display.

## Why the run-average ETA sometimes has no number

The rate uses **newly committed input occurrences since this process started**
divided by the entire invocation's elapsed time, including preparation and
post-chase bookkeeping. Work retained from a previous process is not counted
as work just completed by this invocation.

A resumed process working on its first large batch can therefore have no
measured committed-input throughput for hours. The display now says
`awaiting first new commit` instead of the ambiguous `warming up`. After that
commit the ordinary whole-run estimate becomes available automatically. It
remains a rough input-count extrapolation, excludes publication and does not
predict how difficult the remaining groups will be.

## Architecture and compatibility

`GeneratedWorkProgress` is a bounded process-local producer/consumer handle
in the Rust core. Native code updates scalar counters at a throttled cadence;
the existing Rust monitor polls and emits `finite_work_progress` in its
structured heartbeat. The terminal renderer is only a consumer. No Python
callback, custom computer algebra, traversal scan or additional worker pool is
involved.

At the post-CURRENT adoption boundary, only fixed-size process-local telemetry
is updated—no external observer, serialization or fallible IO is introduced.
The handle is excluded from source identity, checkpoint schemas and artifact
payloads. Existing finite and legacy campaign lanes remain available, and
existing finite checkpoints can be continued with the new binary. No IBP rule,
scope restriction, cancellation authority or user RAM allowance is changed.

For a subsequent extension, use `saved_campaign.py extend --rank 6` with the
new executable explicitly selected. Wait until the previous requested scope
has finished solving **and publication**; the command extends that campaign
and preserves its current D ceiling rather than starting a new campaign.

## October 19 optimized delivery

The frozen local executable is
`TMP/releases/20261019-finite-post-chase-progress/rustred`;
`target/release/rustred` is refreshed too. The running campaign retains its old
frozen executable. After its rank-five solve and publication have completed:

```bash
cd /shared/codex-2/rustred
nix develop --command python -B examples/python/saved_campaign.py extend \
  --campaign /shared/codex-2/rustred/campaigns/five-loop-finite-r0-d10-resident-fixed-20261016 \
  --rank 6 \
  --executable /shared/codex-2/rustred/TMP/releases/20261019-finite-post-chase-progress/rustred \
  --workers 150 --cpus 0-63,96-181 \
  --windows-per-batch 64 --worker-pool persistent --window-preparation parallel \
  --max-memory-bytes 600000000000 --memory-policy process-limit
```

This reuses the existing campaign and preserves its D≤10 ceiling. It does not
regenerate IBPs. No production extension was started by the assistant.

Binary SHA256:
`9bf9de92618c78511e64643ae744820219d72e9321793c6795d4553148115cd3`.
Focused checks: 273 finite app tests, four public core controls, 49 Python
steering/startup checks and three real-native terminal tests pass. Existing
manual/measurement-only ignored fixtures were not represented as completed.
