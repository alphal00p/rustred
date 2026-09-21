# Shared rank-ten pressure test: stopped, not completed

This records a finite operational diagnostic, not the complete parametric R=10
solve. The input contains two concrete numerator patterns per saved owner, each
of rank ten with one raised denominator: 134 entries spanning all 67 classes.
All 67 saved programs and 8,179 nonidentity routing maps are reused; no IBP is
regenerated. Positive powers outside these entries are not tested by this run.

## Resource envelope and outcome

The release CLI frozen from `6c53b3a` ran with 50 outer workers on CPU IDs 0–49
(50 distinct physical cores on this host), with all native inner pools set to
one. Python supervised a 450 GB RSS soft threshold, 500 GB hard threshold and
480 GB child address-space ceiling. **There was no wall-time deadline.**

| Measurement | Observed result |
|---|---:|
| Whole command wall time | 928.95 s, forcibly stopped |
| Whole command CPU time | 8,218.73 s |
| GNU-time peak RSS | 268,597,984 KiB (275.04 decimal GB) |
| Python sampled aggregate peak RSS | 274,865,041,408 bytes |
| Last completed local expansions | 4,797 |
| Last queued operational nodes | 36,710,476 |
| Last distinct integral keys | 36,696,634 |
| Last deduplication hits | 40,897,564 |
| Last rule applications / transports | 1,176 / 3,332 |
| Last failed / still-active workers | 42 / 8 |
| Final Rust result / graph checkpoint | Neither produced |

The breadth of generated dependencies was already substantial. After a first
worker failure, the scheduler stopped admitting work, but eight native routing
calls kept running and allocating memory while the graph counters stayed fixed.
The original monitor did not expose the first typed failure during this drain;
that cause was not recovered before termination. Do not attribute that first
failure to a particular budget or algebraic defect without further evidence.

The operator wrote a stop-request receipt and killed only the verified owned
Rust process group after investigating a native arithmetic defect below. The
child exited by SIGKILL; the wrapper exited 137 and reaped it. This was not a
successful cooperative completion, a timeout, or a resumable graph checkpoint.
The supervisor's `hard_stopped=false` means **it** did not issue the kill; the
separate operator receipt and exit status record the actual forced stop. Saved
IBP programs and all measurement receipts remain intact.

## What profiling and inspection established

A 20-second, 49 Hz attached profile finds roughly 35% self samples in memory
copying and 17% in GMP's `hgcd2`; recovered native arithmetic stacks pass through
Symbolica's rational-coefficient polynomial `heap_pow`. Not all copying stacks
were recovered, so these samples do not justify attributing all copying to the
shared queue. The run is an instrumented diagnostic, not a completed benchmark.

Two independent source reviews confirmed a correctness defect in the pinned
Symbolica `heap_pow`: its mixed-radix encoder accumulates place weights in a
`u32` and multiplies digits in `u32` before promoting to native `Integer`.
For example, twelve used variables at power seven have radix eight, and the
highest weight is `8^11 = 2^33`, which wraps to zero in an unchecked release
build. Distinct monomials can then acquire the same encoded exponent. The
decoder also narrows its radix divisor to `u32`. This is a native-library bug,
not a reason to implement polynomial arithmetic inside RustRed.

A bounded, read-only native routing diagnostic now binds the encoding defect
to **all eight stragglers**. They use just two distinct momentum maps. Rebuilding
and verifying those maps without exponentiation shows that each relevant
numerator row contains 13 nonzero variable coefficients and a nonzero constant.
For powers seven, eight and nine, the highest place weights are respectively
`8^12 = 68,719,476,736`, `9^12 = 282,429,536,481` and `10^12 = 1,000,000,000,000`.
Every one exceeds `u32`. Thus these actual calls encounter defective encoding,
not merely a hypothetical larger family. This does not recover the first worker
failure or establish that the fix eliminates all legitimate graph growth.

The map-only diagnostic finishes in 1.05 s with 112,976 KiB peak RSS. It neither
expands a numerator nor runs a reduction; it is not a corrected-solver timing.

## Corrected native expansion replay

The [portable native patch](../../patches/symbolica/README.md) passes two public
power-versus-multiplication regressions and all 18 numerator-expansion tests.
Both actual affine rows were then replayed at powers seven, eight and nine,
in six serial release processes. Each complete polynomial, including every
exact coefficient and exponent, equals repeated **native Symbolica**
multiplication. There is no custom polynomial arithmetic in this comparison.

| Power | Actual terms (each map) | Native power, map A (s) | Native power, map B (s) | Native multiplication reference (s, A / B) | Largest whole-process RSS (KiB) |
|---:|---:|---:|---:|---:|---:|
| 7 | 77,520 | 0.204760 | 0.207406 | 0.036888 / 0.036838 | 29,748 |
| 8 | 203,490 | 0.587013 | 0.589015 | 0.112796 / 0.112986 | 86,248 |
| 9 | 497,420 | 1.593039 | 1.585310 | 0.316994 / 0.306470 | 217,520 |

Map A has source support `010011000011111`; map B has
`011000000001111`. These descriptions are external diagnostic inputs, not
engine dispatch keys. Timed power excludes polynomial input construction and
the multiplication reference. Whole-process measurements include both native
calculations and equality checking, reaching at most 1.93 seconds. Each process
uses one CPU and an 8 GiB address-space ceiling. A 120-second per-process
diagnostic guard was not reached; it is **not** a time limit on the joint campaign.
Compilation and concurrent release validation are outside these inner timings;
these single observations on a shared host are not a controlled speedup ratio.

The existing native multiplication path is faster on these six inputs, which
is a useful later performance lead. No production power-dispatch optimization
has been made beyond correcting the encoding. The uncorrected large powers
were deliberately not replayed. The correction fixes an actual defect in the
stalled inputs, but a full corrected dependency traversal, its remaining
resource limits, and R=10 closure remain unmeasured here.

## Corrected 50-worker retry: explicit per-call resource failure

The corrected release executable from `59d0ab2` reused the exact same 134
entries, saved owners, routing witnesses, CPU affinity and resource envelope.
Both matched small controls passed first. The new run has **no wall-time
deadline** and exits normally with an incomplete result (status 4), without an
operator/resource kill. It is not a successful solve.

| Measurement | Corrected retry |
|---|---:|
| Whole command wall / CPU | 240.20 / 580.30 s |
| Shared traversal until all workers drained | 119.231 s |
| GNU-time peak RSS | 23,931,876 KiB (24.51 decimal GB) |
| Sampled aggregate peak RSS | 24,298,639,360 bytes |
| Completed local expansions / queued nodes | 3,750 / 27,162,551 |
| Distinct integral keys / dedup hits | 27,147,906 / 23,862,056 |
| Rule applications / transports | 985 / 2,544 |
| Missing rules / owners observed | 0 / 0 |
| Complete Rust diagnostic / resumable graph | Yes / no |

The first live error appears at 118.777 s of traversal while 49 other workers
are draining. All workers have returned by 119.231 s; the final 50 failed-node
count includes peers observing the shared failure, not 50 independent causes.
The originating route is
`[2,1,0,1,0,-4,1,-5,1,1,0,2,0,0,1]` (numerator rank nine).
Its **projected polynomial support** is 4,241,160 against a per-call default of
4,000,000. This is the product of separate factor support estimates, not an
observed native polynomial of that size. Increasing an aggregate campaign
budget does not alter that separate per-call allowance.

The corrected run no longer exhibits the eight runaway powers. It does expose
genuine scheduling overhead: a 20-second/49-Hz profile has 31,596 samples and
zero lost samples; about 20.0% self samples lie in mutex contention, 15.6% in
the shared work-tree search, and 5.3% in integer-vector comparisons. The profile
also records throttle/unthrottle events: zero lost samples does not imply
unthrottled or unbiased sampling, and these hotspots are descriptive. Sampled
utilization during this phase is only about 3–5 cores despite 50 workers.
This is an instrumented, resource-censored diagnostic—not a completed timing
comparison or evidence that the 27-million-key queue finishes within 15 hours.

The next changes therefore target bounded batch publication and a smaller
shared membership index. Separately, a tighter generic support envelope can
account for the common variable set and total degree: nine powers in fifteen
variables have at most `binomial(24,9) = 1,307,504` monomials of degree at most
nine. Pairwise multiplication work remains separately bounded; an output-size
bound is not a claim about arithmetic cost. Explicit per-call CLI/Python budget
steering also remains useful. No new IBP search is justified by this resource
failure; directed source feedback must use actual missing-rule domains.

Receipts: `TMP/shared-r10-corrected-pressure.ynEtcB/` and
`TMP/shared-owner-campaign.nobl7q4b/`. The corrected native powers, matched
controls and this failed large traversal have distinct timing boundaries.

## Next measured iteration

### Batched retry: cooperatively stopped for optimization

The `6d01349` binary passes the matched controls and proceeds beyond the
previous projected-support failure on the same 134 rank-ten entries and
67-owner selection. The 50-worker run has no elapsed deadline and hits neither
its RSS threshold nor a native expansion allowance. The operator instead
requests a cooperative optimization stop after observing persistently poor
useful throughput and more than 53 million queued dependencies. It returns a
complete diagnostic with `Cancelled`, not a completed reduction.

| Measurement | Batched retry |
|---|---:|
| Whole command wall / CPU | 468.76 / 1,115.50 s |
| Shared traversal through native-call drain | 332.873 s |
| GNU-time peak RSS | 31,504,664 KiB (32.26 decimal GB) |
| Sampled aggregate peak RSS | 32,209,813,504 bytes |
| Completed local expansions / queued nodes | 67,182 / 53,696,854 |
| Distinct integral keys / dedup hits | 53,725,660 / 423,873,438 |
| Rule applications / transports | 10,636 / 21,539 |
| Declared terminals / zeros visited | 3 / 34,649 |
| Missing rules / owners observed | 0 / 0 |
| Complete Rust diagnostic / resumable graph | Yes / no |

At the decision snapshot (301.623 seconds of traversal), only 62,370 local
expansions have finished while 53,573,903 nodes remain queued. Recent measured
throughput is hundreds of expansions per second and sampled utilization is
only a few cores. These observations motivate an optimization checkpoint, not
a mathematically proved remaining runtime or an exhausted 15-hour deadline.
No IBPs are regenerated or lost; the original saved programs remain reusable.
The final 49 failed-node reports reflect cancellation of in-flight work, not
49 independent mathematical failures. No process is force-killed.

A single 20-second/49-Hz attached user-cycle profile now highlights native
`Integer` comparison (17.51% self), the native integer-key tree used by powering
(9.86%), `heap_pow` (8.46%) and radix decoding (5.74%). Shared membership-table
rehashing accounts for another 5.74%. These are on-CPU samples, not wall-time
fractions: a separate live observation finds 49 threads sleeping in futex
waits. The profile alone cannot attribute those waits or prove contention has
disappeared. It records 13,896 samples, throttle activity, and zero lost samples;
event encoding and sampled phase differ from the earlier profile.

The immediate next experiment changes only the external owner-selection policy:
reuse cheap, compatible, already-saved literal owners for frequently observed
routing supports. This can bypass numerator transport altogether. It must be
reported as a different rule/route selection, not identical-workload scaling.
Native powering and support-only transport reuse remain separately identified
optimization candidates, not implemented speedups.

Receipts: `TMP/shared-r10-batched-pressure.tVw9tr/` and
`TMP/shared-owner-campaign.el9oduxq/`, including the explicit operator stop
reason, complete result and sampled resources. This run still does not cover
the complete parametric R10 domain.

### Continuing priorities

Before another code change, a separate input-policy experiment installed eight
additional compatible saved literal owners (75 programs covering the same 67
graph classes). Six cheap additions account for 44.56% of sampled active-route
occupancy in the preceding run; two earlier problematic supports are included
too. This is an occupancy-biased selection, not a claim about call frequency or
CPU cost. Input grows by 98,312,571 bytes to 1,379,167,166 bytes, below the
unchanged 2 GiB encoded-input allowance. All original 134 targets remain.

Native loading and routing admission succeed. The run is again cooperatively
stopped for optimization, without a deadline, forced kill, or resource-limit
error. It takes 312.28 s whole-command wall / 689.75 s CPU, including 192.772 s
traversal through drain. GNU-time peak is 28,566,656 KiB (29.25 decimal GB);
sampled aggregate peak is 29,042,987,008 bytes. At termination there are 38,617
completed local expansions, 38,219,901 queued nodes, 234,012,066 deduplication
hits, 8,216 rule applications, 13,757 transports and no observed missing rule
or owner. The three observed declared terminals are not a complete terminal
basis. The altered graph is smaller at these observation boundaries, but the
run still has low throughput/utilization; neither observation is a completed
timing or an equal-workload speedup. The 68/69-owner alternatives were prepared
but not run.

Evidence: `TMP/routed-hot-direct-owner75.xXUTLg/`,
`TMP/shared-r10-direct75-pressure.S7XQ7r/`, and
`TMP/shared-owner-campaign.cuf8ej3c/`. The next narrow implementation target is
trace-only consumption of native coalesced support without constructing exact
coefficient wrappers that the dependency trace immediately discards. Public
coefficient-returning reduction remains unchanged. Any later support cache must
bind the exact prepared route and full negative-power pattern; equal rank alone
is insufficient.

1. Keep the completed native correction and full core/frontend gates as the
   baseline. Do not regenerate the saved owner programs.
2. Audit/test the observed scheduler and structural-envelope improvements,
   retaining exact native arithmetic, cancellation and separate resource caps.
3. Repeat matched small controls, then the joint rank-ten pressure workload.
   Keep true completed timings distinct from interrupted/resource outcomes.
4. Use actual remaining frontiers to drive shared parametric owner searches;
   a finite target sweep still does not replace the complete R=10 campaign.

Evidence is retained under `TMP/shared-r10-census-pressure.8zoXXQ/` and
`TMP/shared-owner-campaign.6g82688v/`, including frozen inputs/binary identity,
events, sampled resources, profile, process status and operator stop receipt.
The exact two-map diagnostic and its input/library identities are in
`TMP/pressure-route-factor-shapes.VYM18Y/`.
The corrected six-case expansion replay, independent review and frozen native
library identity are in `TMP/actual-route-native-power.QaO8i9/`.
The completed one-/six-worker rank-one controls are reported separately in
[the driver documentation](../shared_owner_campaign_driver.md).
