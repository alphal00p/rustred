# Combined G2′ / explicit-role rescue on LC2

Status: integrated on `fable_5_1`; all14 representative correctness runs pass.
**Not yet a production-switch recommendation.**
Plan: `CODEX_PROGRESS_PLAN.md`. Measurement protocol:
`codex_lc2_integration_protocol_2026-09-29.md`. Continuous record:
`CODEX_PROGRESS.md`.

## What changes

G2′ borrows an earlier inspection only over its inspected scope, represents
the exact union and residual, and records its dependencies. An undecidable
residual plan falls back to ordinary inspection. Fresh planning excludes
quarantined anchors. Accepted decisions survive replay with their original
dependencies; subsequent frontier taint cannot silently turn them into valid
coverage. G2′ remains opt-in (`--g2-residual-anchors union`); default `off`
preserves the legacy path.

Rescue uses a complete immutable partition of exact query IDs, not names or
substrings. All 116 frozen physical/convenience queries remain required; the
67 original helpers are auxiliary. The 183 original geometries and order are
unchanged. Missing declarations mean every query is required, and ambiguous
scope cannot authorize rescue. Amendments are append-only and cannot remove a
required query. Required coverage needs exact containment in recursively
closed input-root domains, independently of unrelated failed helpers.

These changes reuse the saved owner rules and existing Symbolica operations.
They do not introduce an algebra kernel, regenerate the rules, minimize
terminals, or change pending-growth-per-completion.

## Frozen identities

| Component | Identity |
|---|---|
| Tested native source | `d12db6cfa23c09f7d9c2946416ea49763ece48f0` |
| Tested Python steering | `cd52c90d5280125f42f6c348a14f89e862206798` |
| Main integration / metric-reader follow-up | `9ef5464d` / `77059cdf` |
| Symbolica | `ef0db494533c87adb40356c996241680dc5a7bff`, patch-free, atom format 6 |
| Optimized candidate SHA-256 | `8169221a8977ae261e777ddca5ac9e82fcb339377362d988472930595b1ea341` |
| LC2 reference SHA-256 | `fd9b9ac96a50513d8119a43938313e8a729110d6a3815b0f949cec4acb1d9d60` |

Candidate path:
`TMP/codex-g2-rescue.tzdFuj/candidate-bin/rustred-d12-8169221a`.
It is a read-only frozen copy. Both comparison executables use the campaign
profile: release optimization, fat LTO and one codegen unit. Compilation is
excluded from runtime measurements; the candidate build itself took
3,446.419 seconds including its guarded launcher.
The merged committed native/dependency tree is identical to the tested native
source. The metric-reader follow-up changes only Python research tooling.

## Completed validation

| Check | Observed result |
|---|---|
| Combined native G2 focus | 20 passed, 8.30 s |
| Full app suite | 861 passed, 12 ignored, 141.13 s |
| Separate width-50 subset | 13 passed, no skips, 0.13 s; exercises previously omitted W50 subcases |
| External CLI tests | 6 passed, no ignored tests, 0.16 s |
| Full Python suite | 284 tests, one expected optional enumeration skip, 22.425 s |
| Separate optional full input enumeration | Passed, 84.763 s |
| Offline input checker | 7,424 membership probes, zero disagreements |
| Root post-merge Python / tooling reruns | 284 tests, one expected optional skip /24 passed;22.601s /3.860s |

The width-50 tests establish correctness coverage, not scaling or high CPU
utilization. They overlap the full suite and must not be summed into a new
unique-test count. Compilation and guarded process time are recorded
separately in `TMP/codex-g2-rescue.tzdFuj/`.

The actual Python/native FG composition test used unchanged roleless inputs
(all 248 queries required), 124 owners, Ordered/W6 on CPUs64–69:

- Fresh Union paused cleanly after real work, with a durable checkpoint.
- Ordinary resume retained Union and completed. All owned processes drained.
- Final output contained 98,867 closed domains, 98,843 native inspections,
  305 G2 records and 314 anchor links.
- Cold native verification reinspected every native record and verified all
  248 roots, with zero frontiers/uncovered obligations. Guarded time11.156 s.
- Independent Python audit passed in6.139 s and accounted for19 unfinished
  attempts carried from the earlier session.

The pause occurred before the first accepted G2 loan. Thus this process test
does **not** prove durable accepted-plan replay; native seam tests cover that
separate invariant. The first Python audit invocation incorrectly supplied a
request object where an argv array was expected. Its failure is retained; the
unchanged auditor passed using its existing automatic request handling.

Evidence: `TMP/codex-g2-pilot-prep.n7Kd5q/python-union-smoke/`.

## Representative controls and measurement caveats

Seven Ordered controls are required: FG, BMW, H, X, combined four-loop,
combined four-loop partial-initial control, and finite five-loop. Each Off
result must match the valid LC2 record/counter reference exactly. Both modes
also require full cold reinspection and a paired Python audit.

All seven controls now pass both modes with full reinspection.
Counts below are Off / Union; all listed input queries close. These single
Ordered correctness runs are
not substitutes for the Ready deployment pairs.

| Control | Required input queries | Distinct roots | Scheduled domains: Off / Union | Native inspections: Off / Union |
|---|---:|---:|---:|---:|
| FG | 248 | 248 | 98,909 / 98,867 | 98,869 / 98,843 |
| BMW | 268 | 268 | 158,951 / 129,616 | 147,233 / 119,727 |
| H | 628 | 628 | 24,929 / 24,769 | 24,680 / 24,513 |
| X | 656 | 656 | 47,193 / 46,060 | 46,826 / 46,017 |
| Combined four-loop | 58 | 32 | 65,444 / 68,184 | 30,159 / 32,085 |
| Combined, partial-initial variant | 58 | 32 | 68,483 / 71,284 | 31,717 / 33,663 |
| Finite five-loop | 1 | 1 | 1,273,376 / 969,467 | 967,621 / 752,033 |

In particular, G2 increases domain/native work on the combined controls.
This negative result is retained: exact reuse is not universally cheaper,
and opting into it needs workload-specific evidence. The finite-five-loop
control has one initial region containing1,324 integer tuples; it does not
represent completion of the full frozen production request.

The finite Ordered pair took310.284s Off and226.745s Union, including process
launch and output. Union's separate cold verifier took152.224s; the paired
Python guard took446.320s and checked all80,181 exact union records and215,989
anchor links. Verification costs are not solver times. All14 native runs are
uncensored, all63 per-stage guards exited0 without a stop reason, and every
owned process group drained. Receipts/index are in
`TMP/codex-g2-pilot-prep.n7Kd5q/CONTROL_RESULTS.json` and `CONTROL_DRAIN.json`.

An independent review found that the historical benchmark extractor could
read nested `successors`/`events` rather than root totals. The original FG Off
receipt therefore contains1,242/1,498 instead of2,182,549/2,488,138. The native
result, identity comparison, cold verification and wall timing are unaffected.
Preserved original receipts are supplemented with root-aware
`corrected-metrics.json` sidecars; incorrect fields must not enter comparisons.
The tracked bounded extractor is corrected in `3832bf58`, independently
reviewed and covered by24 focused tests. It uses complete root/path-qualified
fields from bounded buffers, leaving unavailable fields unknown. This
tooling-only fix is now merged as `77059cdf`; the original control tool and
its receipts remain unchanged. The Ready arms use the same frozen corrected
runner on both sides.

Both Ready finite-five-loop pairs have completed all acceptance gates:

| Metric | Pair 1: LC2 Off | Pair 1: G2 Union | Pair 2: LC2 Off | Pair 2: G2 Union |
|---|---:|---:|---:|---:|
| Whole command wall time | 249.626 s | 180.746 s | 252.306 s | 180.010 s |
| Scheduled domains | 1,275,122 | 964,909 | 1,276,229 | 963,440 |
| Native inspections | 979,714 | 758,183 | 981,826 | 758,280 |
| Waited CPU time | 1,082.484 s | 564.716 s | 1,093.842 s | 558.191 s |
| Sampled peak aggregate RSS | 6.361 GB | 6.239 GB | 6.382 GB | 6.236 GB |

That is27.6%/28.7% less observed wall time and24.3%/24.5% less scheduled
work for this control. The second pair ran in reverse order. All four
required-query counts are1/1, with every native record reinspected and
every scheduled record paired by the Python audit. Candidate cold verification
and Python pairing are separate from the solver timing; its Python check alone
took494.608s in repetition1 and487.373s in repetition2. Evidence:
`deployment-continuation/accepted-4.json` under the
pilot directory. These are **not** timings for full production closure.

For pair1, selected-core foreign load averaged1.146/1.076 cores, while
SMT-sibling occupancy averaged3.317/1.283 cores. Pair2 sibling occupancy was
3.096/1.402 cores (Off/Union). Most of that sibling occupancy was user-space,
but the counters do not attribute it to particular processes. Both imbalances
favored the candidate, so the wall-time ratios are not uncontaminated causal
speedup estimates. Work-count reduction repeats independently of interpreting
those timing differences. The finite control meets the numerical work gate,
but the hot controls are still running at07:10 UTC. No production deployment
decision is made yet.

The decision requires two interleaved matched
Ready pairs with identical inputs, CPU placement and worker budgets, recorded
contention and independently verified completion. Every pilot and cold check
has its own <=30-minute ceiling. Censored results are not completion timings.

## Compatibility and deployment boundary

The native G2 activation seam is tested, but this does not establish that the
production Python driver can change a frozen campaign's mode or query roles.
The new explicit-role declaration changes the request binding. Combined
G2/rescue deployment should therefore use a fresh campaign unless a cheap,
specific continuation has separately passed its tests. No importer is planned
solely to preserve LC2 progress.

LC2 is untouched and remains owner-controlled. Only after the performance or
actual-blocker gate passes will a release include exact build/staging/launch
instructions and a rollback path. An optimized control is not evidence that
the whole frozen five-loop request has closed or will finish by a given time.
