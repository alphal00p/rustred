# Precise saved-rule route images and cumulative closure progress

## Scope and status

This opt-in change tightens the conservative images of **existing verified
momentum maps**. It neither generates IBPs nor changes the requested physical
input scope. The implementation is generic in topology and number of coordinates.
Its mathematical/unit gates pass. A complete small five-loop R1 comparison is
now **negative**: tighter images increase total domain work. Keep the feature
default-off; there is no recommendation to replace the running R6/D10 campaign.

The separate monitoring change makes the principal closure bar refer to all
cumulative required requests, including rank extensions. Original-root progress
remains secondary. This is conservative progress evidence, not independent
certification or an ETA.

## Why the image can be smaller

Write the verified substitution of an inactive source denominator as
`Q_i = c_i + sum_j M_ij Q'_j`. Existing native compilation already supplies
the exact nonzero incidence and coefficients. Let `r_i` be its numerator power,
`A` the sum of positive indices, `R = sum_i r_i`, and `D = A - R`.

For each target coordinate, only source rows with `M_ij != 0` can contribute
polynomial degree. The existing source-subset degree service bounds their total
by `C_j`, including coordinate bounds and the shared rank budget. Consequently:

- an originally inactive target coordinate has numerator degree at most `C_j`;
- a newly pinched positive target of minimum power `b_j` has residual numerator
  degree at most `C_j - b_j`;
- unreachable inactive coordinates stay fixed at zero.

If a resulting monomial has degree `T`, its signed index sum is
`D' = D + R - T`. Only rows with a nonzero constant `c_i` can lose degree.
Their joint source-subset bound `C_constant` therefore gives
`D' <= D_max + C_constant`, intersected with the previous safe upper bound.
When all **used** rows are homogeneous, `D` is preserved. Constants on rows
whose exponent is fixed at zero do not inflate the bound.

These are necessary bounds, not an assertion that every point inside the image
is attainable. Pinch siblings are derived from the original source projection,
never from an all-positive sibling's additionally tightened projection. The
minimum pinch cost is subtracted from A/rank where appropriate, not twice from
the signed D bound. Guards, source-validity obligations, absent routes, storage
padding, cancellation, and checked overflow behavior remain explicit.

All algebra is still the existing Symbolica-backed verified transport. The new
work is index/degree bookkeeping, not a polynomial-expansion or CAS kernel.
This is independent of the earlier optional joint-source-support mask pruning;
its negative performance evidence is not erased by this new hypothesis.

## Interfaces and checkpoint behavior

- Rust core: `CandidateDomainRouteOptions.precise_image_bounds`.
- Application: `OwnerDomainWalkRequest.route_precise_image_bounds`.
- Native/Python campaign steering: `--route-precise-image-bounds`.
- Default: false; flag-off behavior and existing checkpoint identities remain
  unchanged.

When enabled, CP5/CP6 request bindings include the recipe
`verified-incidence-affine-degree-v1`. An off checkpoint cannot simply be resumed
with the flag on. Use a separate fresh campaign for the comparison; do not mutate
a user campaign to enable it.

Cold verification retains the selected image geometry even under
`--reference-levers off`. Turning it off there would demand a different, larger
artificial region rather than verify the saved obligations. The all-node replay
checks the resulting graph; separate exact native endpoint tests check the
inclusion premise. These are distinct gates, not two independent implementations
of the same geometry proof.

## Cumulative required-query observation

An input-sized index records admitted roots that contain each required query,
using the same existing containment criterion as the final query certifier.
It is prepared on restore/admission, not by scanning the whole ledger on each
heartbeat. Closed bits are inspected after completed dependency-closure scans;
repeated heartbeats reuse the observation for that scan/admission revision.

Auxiliary roots can supply exact containing coverage but do not increase the
required denominator. A required query is not counted before admission.
Allocation failures, missing input records, or an absent closure scan yield
unknown, not a fabricated zero. Scan age and conservative lower-bound labels
remain visible. Old binaries without cumulative evidence show unknown rather
than relabeling their original-root counter as enlarged-scope closure.

## Validation evidence

Release native core gate: **57 passed, 0 failed, 0 ignored**, 0.11 s test time,
1.17 s supervised. Evidence:
`TMP/route-image-core-tests-final-20261015/`.
Tests include native exact endpoint inclusion, homogeneous and affine maps,
true pinches, all constant-bearing source rows, untouched padding, wide bounds,
guard/source conditions, and exact flag-off identity.

Two earlier 55/57 runs failed during test-fixture construction/assertions, before
endpoint checks. Those failures are retained. The final admitted determinant-one
fixture has native constants `[0,0,0,0,0,3]`; an independently linked public-API
probe additionally checked 3,024 exact endpoints across 72 source points.
No production bypass or reduced assertion was introduced to make it pass.

Application gates include eleven required-closure tests, three precision-policy
tests, and twenty independently executed lifecycle/extension/reporting tests.
Some groups overlap and must not be summed as unique test coverage. Real CP6
rank extension, alias requests, auxiliary roots, repeated resume, and final
certifier agreement are exercised. Python forwarding, old frozen policies,
dashboard, scan-age, and shared-supervisor tests also pass. Raw evidence is under
`TMP/routing-closure-app-*-tests-20261015/` and the corresponding Python receipts.

## Performance protocol and remaining work

Use one frozen optimized executable, identical owners/routes/orderings and the
same complete small R0/R1 hot-owner queries; change only the precision flag.
Time preparation, walk and cold verification separately. Record domain work,
pending pressure, memory, process CPU and required closure. Censored prefixes
are not successful comparisons. Production CPUs/campaigns are excluded.

The four-loop test-owner extraction reuses saved H/FG/BMW/X bundles and does not
generate rules. Literal per-family tests are not a substitute for the historical
combined four-family routed benchmark; missing route fixtures must be reported,
not manufactured or quietly omitted from the acceptance claim.

## Measured comparisons

The frozen CLI is `TMP/releases/20261015-precise-route/rustred`, SHA256
`c9fa26af80738a032fb80d1aef7175005330085118af34db9cb3e40ecff78b6f`.
Four workers on CPUs16--19,32GB process-tree allowance and150GB host reserve
were used for the same hot owner, all67 owners and69 prepared payloads, unchanged
rules, source ordering and required query. Shared-host contention is recorded.

The nine-point R0/D10 control closes with two domains and20 events in either
mode. It does not exercise affine routing and establishes no performance gain.
Both corrected all-node cold checks pass. The original incorrect cold invocations
used a checkpoint-only summary as `--result`; their INCOMPLETE receipts remain.
The corrected `--no-result` invocations authenticate the full CP6 ledger without
restarting the walks. Inclusive time, including the incorrect invocations, was
879.78s, within the30-minute experimental envelope.

The complete279-point R1/D10 region is nontrivial:

| Metric | Default image | Precise image |
|---|---:|---:|
| Owner preparation |137.487s|128.924s|
| Traversal |187.500s|213.867s|
| Native preparation plus traversal |324.987s|342.791s|
| Scheduled domains |476,244|505,368|
| Native inspections |377,541|410,705|
| Maximum encountered scalar rank |6|5|
| Route masks |5,990,257|4,224,877|
| G2 union-cover records |36,308|72,137|
| Dependency edges |3,558,997|4,405,670|
| Sampled peak process-tree RSS |9.011GB|8.903GB|
| Independent cold check |PASS|PASS|

Both arms drain with zero pending/frontier domains and all native records
reinspected. Precision saves route masks but increases domains6.12%, inspections
8.78% and traversal14.06%. More fragmented reusable regions and containment work
are consistent with the counters; this is not an isolated causal attribution.
One shared-host pair is sufficient to reject deployment on this evidence, not
to prove a universal slowdown. No further pure-precision tuning is justified
without a new mechanism/evidence. Full pair duration was1162.94s.

Four-loop **single-parent** D8/R1 regression controls also pass both arms:

| Parent input | Domains, identical off/on | Traversal off/on |
|---|---:|---:|
| H |27,706|5.296s /5.117s|
| FG |11,025|2.211s /2.696s|
| BMW |20,532|4.347s /4.490s|
| X |87,194|18.458s /19.299s|

These are small literal-family regressions, not the complete historical shared-
basis four-loop physical campaign. No speedup follows from their timing noise.
Separately, a much broader all-literal-FG control withA<=19,R<=12,D>=7 reached
its600s experimental stop and saved a resumable CP6 checkpoint with476,948
pending domains. It is explicitly censored, not closed; the smaller controls
are not substituted for that result. The recorded26-template four-loop physical
profile is a different scope and basis and still needs its own full control.

Evidence: `TMP/route-image-pilot-hot-r{0,1}-pair1-20261015/`,
`TMP/route-image-parent-{h,fg,bmw,x}-r1d8-20261015/`, and
`TMP/route-image-pilot-fg-pair1-20261015/`. Independent audits are stored beside
these results and in `TMP/route-image-pilots-20261015/`.

## Exact finite traversal: proxy-limit refusals, then a completed diagnostic

The existing default-off whole-initial finite replay enumerates the same279
R1/D10 inputs exactly. Its4M-endpoint pilot processed384,102 operational nodes
in17.483s but hit its aggregate endpoint allowance, with no missing owner or
rule. It then fell back to the symbolic walker, reproducing the baseline record
and edge digests exactly. An8M retest also refused the aggregate allowance,
after23.026s and536,481 completed states, then reproduced the symbolic fallback.
Both failed attempts and their full costs are retained. Neither was finite closure.

Because both refusals occurred well below actual RAM/time allowances, a distinct
preregistered diagnostic removed cumulative proxy ceilings while retaining the
same per-operation Symbolica/expansion limits, exact bounded seed enumeration,
32GB process-tree guard,150GB host reserve,600s cooperative stop and120s forced
cleanup backstop. It **completed**, and an independent cold replay reproduced
every non-timing work counter exactly. No fallback occurred.

| Same complete279-point R1/D10 region | Symbolic baseline | Finite diagnostic |
|---|---:|---:|
| Native traversal |187.500s|28.976s|
| Guarded walk |330.577s|178.463s|
| Guarded walk plus cold replay |555.987s|366.941s|
| Peak walk process-tree RSS |9.011GB|7.693GB|

The finite trace itself took28.774s live/34.137s cold:641,239 operational states,
639,959 physical keys,267,694 rule applications,242,642 transports,10,543,465
projected endpoints,280 declared terminal keys and129,343 zero keys; no remaining
queue, failure, missing rule/owner or frontier. One persisted recipe is not one
unit of computational work. Four CPUs were reserved for each process, but this
existing inline finite kernel used one compute worker. The outer rank1 metric
describes only its source recipe; the current adapter omits internal maximum
rank/dot statistics and must not be used to infer descendant bounds.

This is a genuine finite-region result and an encouraging single candidate-only
observation, not a repeated matched-pair deployment claim. The observed traversal
ratio is6.47x; initialization makes the guarded walk-plus-cold saving about34%.
No extrapolation to the full339-million-point R6 input union is justified.
Evidence: `TMP/route-image-pilot-hot-r1-finite-resource-20261015/`, plus the
preserved `...finite-v2-20261015/` and `...finite-v2-8m-20261015/` failures.

This successful finite diagnostic is not yet a production rank-
ladder feature: finite-enabled requests refuse resume/amendments, and publication
inventory rejects accepted finite summary records. The recipe stores no visited
terminal/rule inventory. A future integration must retain or replay that exact
inventory and preserve interruption/extension behavior; it cannot infer masters
from the current aggregate counters. The staged implementation is specified in
[the finite-demand integration plan](finite_demand_integration_plan_2026-10-15.md).

Still open: full R6/D10 scoped closure, usable production finite-demand traversal,
physical-lowering admission and broad five-loop Vakint acceptance. A passing
geometry gate is not evidence of eventual campaign completion or an ETA.
