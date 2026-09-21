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

## Next measured iteration

1. Finish the complete release gate for the independently audited native
   correction and shared-rule installation changes. The isolated native
   regressions and six affected-map expansion replays already pass.
2. Validate live first-failure reporting, a separate progress wakeup channel,
   and the cheaper process-tree monitor. These improvements do not eliminate
   mathematically distinct dependency work.
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
