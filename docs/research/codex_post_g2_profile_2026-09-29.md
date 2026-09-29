# Post-G2 profile: bounded directional evidence

The completed diagnostic suggests testing a small endpoint-buffer reuse change.
It does **not** establish a runtime gain, a production bottleneck, or additional
closure correctness. The existing campaign was not changed. Deployment remains
held for the user's requested consolidation of validated ideas.

## Recording and scope

The frozen G2 candidate (`d12db6cf`, binary identity `8169221a`) ran the finite
five-loop Ready control under user-CPU sampling. The run exited normally and
uncensored, but no cold verification was performed for this diagnostic; it is
not another benchmark repetition. Its launcher/profile boundary was 177.153 s.
Native preparation was 82.7767 s, traversal 77.8806 s, and native elapsed time
160.6573 s. These are different boundaries, not interchangeable speed figures.

Evidence is retained at:

- Recording and native receipts:
  `/common/dev/rustred/TMP/codex-post-ready-profile-2026-09-29/prepared/`.
- Analysis, exact command/guard receipts, scripts and original outputs:
  `/common/dev/rustred/TMP/profile-analysis.lNU1ZU/`.
- Compact numerical results: `COMPACT_SUMMARY.json`; derivation: `summarize.py`;
  fuller interpretation: `ANALYSIS.md` in that analysis directory.

Offline analysis used CPUs 32–39, one process, `python-0.lock`, local build-ID
storage, no downloads, and the existing 250/150 GiB headroom guard with a
1200 s timeout plus 60 s kill grace. Successful passes took 1.16–2.23 s each;
maximum single waited-child RSS was 245,124 KiB. No stacks were exported, no
host settings were changed, and no native run was repeated during analysis.

## Sampling quality limits

| Check | Result |
| --- | ---: |
| Frozen recording size | 422,853,736 bytes |
| Captured samples / sampled threads | 49,119 / 18 |
| Sampled processes (TGIDs) | 1 |
| Complete symbol rows / unresolved sampled IP or DSO rows | 631 / 0 |
| Reported lost samples | 0 |
| Throttle / unthrottle records | 49,119 / 49,115 |
| Distinct event/stream pairs | 273, not a thread count |
| Paired disabled intervals / censored trailing starts | 49,115 / 4 |
| Ordering / duplicate-start / unmatched-end anomalies | 0 / 0 / 0 |
| Disabled interval median / p95 / maximum | 664.182 µs / 937.382 µs / 15.318 s |
| Recorded period | 10,101,010 ns for every sample |
| DWARF unwind quality | Not assessed |

The one-recording `throttle_headers.py` scanner validates file framing, every
record boundary, final cursor, and all 187,046 record counts against perf's own
statistics. It decodes only the fixed throttle timestamp/event-ID/stream-ID
payload, independently reviewed against the
[matching Linux UAPI](https://github.com/gregkh/linux/blob/v6.18.45/include/uapi/linux/perf_event.h#L874).
Elapsed event-disabled intervals may include inactive streams: they must not be
summed as lost CPU or wall time, or used to correct the percentages.

Current read-only settings were Linux 6.18.45, `CONFIG_HZ=1000`, maximum sample
rate 1000, and perf CPU-time maximum 25%. If unchanged since recording, these
imply a one-sample-per-tick limit; the kernel can throttle the first overflow
and stop/restart the software-clock timer. This explains a plausible mechanism
for the near-1:1 counts, not historical settings or unbiased sampling.
See [limit calculation](https://github.com/gregkh/linux/blob/v6.18.45/kernel/events/core.c#L469),
[throttling](https://github.com/gregkh/linux/blob/v6.18.45/kernel/events/core.c#L9588),
and [software-clock timer](https://github.com/gregkh/linux/blob/v6.18.45/kernel/events/core.c#L11139).

The 496.151510190 s period sum is exactly sample count times nominal period,
**not an independent measurement of effective CPU coverage**. Zero unresolved
sampled symbols does not prove successful call-stack unwinding. These are
captured user-CPU **self** costs, not inclusive cost, kernel CPU, blocked latency,
or the wall critical path. The initial `self/` report remains invalid: unsupported
`tid` sorting printed an error/help despite exit 0. Corrected complete reports
use documented `tgid,pid` keys and pass explicit nonempty/count checks.

## Conservative phase interiors

Existing native events/checkpoint timestamps and perf's monotonic/wall-clock
reference locate preparation completion approximately at monotonic
2157180.227–2157181.227, with first worker samples at 2157181.143–2157181.959.
The final checkpoint maps to 2157258.227–2157259.227; final worker samples are
around 2157258.5. Integer wall timestamps make this a coarse alignment.

The descriptive windows are preparation **2157118–2157168** and traversal
**2157190–2157250**, safely inside those boundaries. Each report's sample count
exactly matches a separate filter of no-callgraph sample timestamps. These
windows neither represent complete phases nor remove the throttling caveat.

| Captured sample share | Whole recording | Preparation interior | Traversal interior |
| --- | ---: | ---: | ---: |
| Samples | 49,119 | 4,504 | 32,326 |
| Inspectors | 60.76% | 0% | 72.22% |
| Admission helpers | 13.01% | 0% | 15.76% |
| Main thread | 26.17% | 100% | about 12.02%, including rare other activity |
| Exact power-domain projection | 7.07% | not a leading cost | 8.41% |
| Polynomial-on-map validation | 4.24% | 11.06% | 3.07% |

Traversal additionally shows named allocator routines at 16.53%, memmove
5.20%, SipHash writes 4.04%, and admission-preparation plumbing 2.54%. These
are disjoint instruction locations, not allocations attributed to their
callers. They establish neither quadratic scaling nor a wall-time percentage.
Preparation-heavy main-thread algebra is not directly representative of a
mature campaign whose initialization is already amortized; production's
coordinator measurements remain a separate basis for the epoch work.

## Narrow hypothesis and falsifier

In the tested source, `owners/domains/applied/geometry.rs:224` creates two
endpoint vectors of exactly generic length N. After exact correlated-power
projection, `owners/domains/applied/engine.rs:742` replaces both using `to_vec()`.
Copying the projected arrays into those already-sized buffers can remove two
replacement allocations and two old-buffer frees per applicable image. It
does not remove projection arithmetic or the necessary endpoint copies.

This is a source-valid allocation hypothesis, not attribution of the 16.53%
allocator share to this path. Keep bounds, checked arithmetic, rank, failures,
budgets, cancellation and callback order unchanged; no new CAS primitive or
cache. Isolated implementation was authorized separately; this report contains
no implementation-test or performance PASS.

The smallest gate is existing exact shifted-image/native transport coverage
for finite/infinite endpoints, sign changes, correlated A/R/D, and overflow or
failure cases; compare complete successor fields/counters and directly check
buffer pointer/capacity stability. Any semantic difference, no targeted
allocation reduction, or no repeatable unprofiled benefit falsifies the case
for retaining it. No numeric gain or W100 production forecast follows here.

## Reproduction without another native run

Exact argv and guard results are saved under `symbols-all/`, `thread-dso-hist/`,
`sample-times/`, `preparation-interior/`, `traversal-interior/`, and
`throttle-headers/`. Use their frozen `request.json` rather than reinterpreting
the invalid initial report. The perf executable is:

```text
/nix/store/wizn21b9virxqcnm4n89b09pgqkaxfn3-perf-linux-7.2.5/bin/perf
```

The complete symbol report's inner command is:

```sh
perf --no-pager report --stdio --no-children --call-graph none \
  --sort dso,symbol --field-separator ';' --percent-limit 0 \
  --show-nr-samples --show-total-period --input "$profile_data"
```

Here `profile_data` denotes the frozen `prepared/perf.data` above; `perf` must
be the exact executable just listed. Phase reports add `--time START,END` and
use `--sort comm,dso,symbol`; timestamps use `perf script --hide-call-graph --ns
--fields pid,tid,time,period`. These commands were run through the bounded
guard, not as unrestricted new profiling jobs, with
`PERF_BUILDID_DIR=.../prepared/buildid`, `DEBUGINFOD_URLS=''`, and workspace
`TMPDIR`. Profile expansion stops here; any further run needs its own decision.
