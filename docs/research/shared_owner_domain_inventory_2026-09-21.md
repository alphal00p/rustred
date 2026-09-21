# Saved-owner parametric successor inventory — September 21

This diagnostic uses the 67 saved five-loop representative programs and their
verified routing selection. It does **not** generate IBPs, certify an artifact,
enumerate concrete positive powers, or establish the complete R10 solve.
The input rank is the sum of negative index magnitudes; positive powers remain
unbounded and successor ranks are not truncated to ten.

The generic Rust service streams all stored rule terms over coordinate sign
cells, retaining the original native guards, coefficient and exact source-rank
predicate. It does not subtract earlier applicable rules/terminals or resolve
coefficient-zero faces. Consequently, a reported activation, absent route or
rank increase is a **potential obligation**, not a demonstrated missing rule.
See [the interface and limitations](../owner_domain_scan.md).

## First measured prefix

Release CLI `4cc1d0e0be5107c8e6d64874ab0cdebb775ce0c09c14ec25f35cf0398b41a3cf`,
Python-steered on CPU40 with one native worker, 32 GiB address-space ceiling and
no elapsed deadline. The owner selection is the same one used in the matched
shared traversal controls. The scan starts only after the full 2,576-core and
281-application release test gates pass. Compilation is outside the boundary.

| Measurement | Result |
| --- | ---: |
| Owner/routing preparation | 121.833 s |
| Native scan and summary aggregation | 1.165 s |
| Whole command | 176.57 s |
| CPU, user + system | 173.32 s |
| Peak RSS | 5,802,088 KiB |
| Completed owners / next partial owner | 7 / 1 |
| Rules / terms visited | 2,275 / 160,306 |
| Retained potential regions | 530,814 |
| Retained summary groups | 16,384 |

The command returns **incomplete, exit4**, because the configured summary-group
allowance is reached. This is not a source-solver failure, mathematical
obstruction or complete scan of all 67 owners. The core counts one additional
callback rejected by that limit, so its region counter is 530,815.

Among retained regions, 65,037 are same-support sign pieces, 175,872 are strict
pinches and 289,905 activate a previously inactive coordinate. The last category
is particularly sensitive to coefficient/guard zeros and cannot be interpreted
as genuine upward-sector reduction. Six summary groups have a positive
same-support rank increment and an unbounded positive-coordinate **prefilter**;
native equalities may further restrict those boxes. The largest reported
one-step rank envelope is fourteen, not a recursive rank bound.

The six positive-increment groups illustrate why that distinction matters:
five only reach rank one or two, whereas one group's 25 potential pieces reach
rank eleven. Its first stored example has source powers
`[0,1,0,0,1,t,1,1,1,1,-k,1,0,1,1]`, with `t>=4`, `0<=k<=10`.
The shift produces
`[0,1,0,-1,1,t-3,1,1,1,1,-k,1,0,1,2]`.
At `k=10`, this is a rank-eleven ray with just one symbolic positive index.
That is a small candidate for existing native univariate guard matching, not
evidence that another large owner generation is needed. Three excluded
conjunctions, original denominators, coefficient cancellation and dispatch
priority still need to be checked before it is an actual source-search request.

The detailed result is 25,131,290 bytes and the event journal is 18,434,857 bytes.
The first implementation unnecessarily copied the full detailed final report
into the live monitor. The command spends about 54 seconds after native
aggregation, including unbuffered journal serialization and cleanup; the
58.29 system-CPU seconds are consistent with expensive small writes, not extra
IBP search. A compact final progress projection and a larger explicit reporting
allowance were then validated before the full inventory retry below. The full saved
result remains detailed; a progress summary does not replace it.

Reproducibility evidence: `TMP/owner-domain-census67.4JxGKT/`, including the
launch script, input/binary hashes, time output, event journal, detailed result
and independent runtime review. Files under `TMP/` are local evidence, not
distributed artifacts. No completed full-domain solve is claimed.

## Complete stored-rule inventory

The reporting-only correction passes all **283 application/integration tests**;
the unchanged core passes **2,576 tests**, with 32 existing ignored diagnostics.
The Python steering/monitoring tests pass **12/12**. Independent implementation
and runtime reviews bind the release executable
`6c8974227226fedb1e95c64bd000b56dbf1b097739905ea650d8729f124e64d2`.

The new Python-driven run uses the same owner selection, rank ten, CPU40,
one native worker and 32 GiB address-space ceiling. The explicit reporting
allowances are 500,000 summary groups, 20 million regions per owner and
100 million total regions. Other work limits remain unchanged. There is no
elapsed deadline, compilation, new IBP generation or concurrent RustRed solve.

| Measurement | Result |
| --- | ---: |
| Owners scanned completely | 67 / 67 |
| Rules / RHS terms visited | 17,975 / 1,667,335 |
| Potential successor regions / summary groups | 4,758,436 / 129,706 |
| Sign-split operations | 3,091,101 |
| Owner/routing preparation | 106.639 s |
| Native scan and summary aggregation | 11.009 s |
| Application elapsed | 118.245 s |
| Whole command wall / CPU | 123.13 / 119.17 s |
| Peak RSS | 6,295,564 KiB (about 6.45 GB) |
| Detailed result / event journal | 199,058,529 / 63,661 bytes |

The command exits **zero**, with `scan_complete=true` and
`family_closure_claim=false`. Every stored owner rule was scanned within these
work limits; this is not proof of first-applicable cover, nonzero/feasible
successor edges, or recursive rank-ten closure. The report retains all its
guard/priority-overapproximation caveats. The native scan does not decide
whether additional IBPs are required.

The inventory separates potential above-entry-rank obligations:

| Potential transition with one-step envelope above R10 | Groups | Regions |
| --- | ---: | ---: |
| Same support | 3 | 217 |
| Strict pinch | 1,555 | 42,544 |
| Activation of an inactive coordinate | 21,041 | 371,662 |

All three same-support groups have increment +1 and envelope R11, and already
have an installed target owner. The strict-pinch pieces all have an installed
owner, verified route or known-zero label. None of this establishes that the
installed program covers the actual successor domain. The three same-support
groups are therefore **not a count of the remaining blockers**, nor the whole
set of obligations. Activation pieces especially may disappear after native
coefficient/guard specialization and ordered applicability are resolved. The
maximum envelope across all transitions is fourteen; this is still only a
one-step bound, not a recursive rank ceiling.

Across all transitions/ranks, 992,331 potential regions have none of the three
available labels (installed owner, verified route, known zero). This is not a
count of nonzero integrals or missing owners. Such unfiltered sign pieces must
not trigger thousands of speculative source jobs.

The larger report no longer floods live monitoring: completion events contain
only bounded counts, status and error details. The complete detailed output is
still saved. The old and new workloads have different report allowances and
scope reached, and preparation varies, so their whole-command ratio is not an
isolated timing comparison of the reporting patch.

Evidence: `TMP/owner-domain-full67.eVQtQh/`; final application gate:
`TMP/owner-domain-compact-app.mGCec7/`. Both are release runs with recorded
input/executable identity and independent reviews; local evidence is not an IBP
artifact to distribute with Vakint.
The independently compared first seven complete owner records match the old
prefix exactly, including every summary key, count and example. All 122 event
records are compact (largest 996 bytes), and the final record retains every
scope/completion caveat. Aggregate region/group sums and input/binary bindings
also pass independent checks.

## Next use of the inventory

Use actual successor requirements to prioritize the existing source-feedback
service. First resolve ordered applicability and coefficient-zero conditions
with the existing Symbolica-backed case/guard primitives. Only genuine,
representable gaps should nominate additional source searches. A relaxed
rank-envelope hole or uncoalesced term does not justify regenerating an owner.
Keep unresolved geometry explicit and retain unbounded positive tails.

This is campaign work discovery, not the deferred independent certification
project. Rule minimization and five-loop numerical master evaluation remain
after complete bounded coverage, as required by the staged goal.
