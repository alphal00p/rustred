# Precise saved-rule route images and cumulative closure progress

## Scope and status

This opt-in change tightens the conservative images of **existing verified
momentum maps**. It neither generates IBPs nor changes the requested physical
input scope. The implementation is generic in topology and number of coordinates.
Its mathematical/unit gates pass; whole-campaign performance remains unmeasured.
It is not yet a recommendation to replace the running R6/D10 campaign.

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

Still open: matched performance pilots, deployment of a frozen CLI, the full
R6/D10 scoped closure milestone, physical-lowering admission and broad five-loop
Vakint acceptance. A passing geometry gate is not evidence that the remaining
campaign will terminate in a particular time.
