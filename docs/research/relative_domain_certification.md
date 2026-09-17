# Retaining the actual domain during certification

Implementation and validation checkpoint, 2026-09-16. These changes address
two captured four-loop proof failures. They do not themselves establish a
four-loop closing artifact. The common mathematical contract is described in
[the convergence plan](certification_convergence.md).

## Target-relative exclusions

The new affine helper proves that `piece AND target` implies every equality
and fixed restriction of one complete exclusion. It first admits the original
equations, then substitutes exact singleton values using Symbolica and compares
the native consistent ranks of the target and target-plus-exclusion systems.
An inconsistent target is empty; an inconsistent augmented system is **not**
evidence that the exclusion contains the target. Separate exclusions are never
merged into a fictitious conjunction.

This is a sufficient rational-affine implication proof for integer points,
not a claim that rational free coordinates describe an integer lattice.
Unrepresentable geometry and native failures remain inconclusive; arithmetic
resource exhaustion remains an error. Original predicates remain authoritative.
The helper uses exact native `Integer` values, including physical `2^64` when
an active local singleton is `u64::MAX`.

For the captured FG115 piece, the target is `1+n2+n9=0` and an exclusion is
`1+n8+n9=0`. The piece fixes `n8=-1,n9=0`, so that whole exclusion holds on
the target. Its apparent non-descending `0/0` contribution has no admitted
application point; no pole cancellation is authorized. The nearby `n8=-2`
point and a box spanning both values remain negative controls.

The proof is wired into the common prepared-parent emptiness seam. Generation
lowering skips an empty piece; cold verification rejects a supplied vacuous
cell rather than accepting its coefficients or descent on that basis.

## Persistent singleton restriction for guard proofs

The guard pipeline now retains the exact supported singleton specialization of
its target-restricted guard and every complete exclusion, through coefficient
splitting, factor implications, and subsequent conjunction charts. Previously
a temporary bool-only check could discard that information before building
another symbolic chart.

All algebra uses the existing checked native-backed polynomial specialization
service. Original polynomial admission and cumulative resource accounting
remain before specialization. Values outside this service's `i64` carrier are
left symbolic, never truncated; this only enlarges the domain to be proved.
No coefficient value or original denominator is replaced by a zero-locus-only
normalization at application time.

The captured H229 case has `n1=0`, `n3<=-1`, `n4<=-2`. Keeping `n1=0` avoids
the unnecessary coupled expansion that previously exceeded the conservative
65,536-bit preflight allowance. The captured guard now passes at the unchanged
default limits, also under coordinate permutation. Widening to the exact zero
`n1=0,n3=-1,n4=-1` still fails.

## Validation status

- 249 source-port tests pass; five existing larger workloads remain ignored.
  This includes the 17 internal rank-slice/scoped-cover regressions introduced
  after the H/FG release executable was frozen.
- 44 affine-domain tests pass, including twelve new target-relative checks.
- Three new guard regressions cover the H capture, permutation/widening,
  complete-exclusion alignment, and a singleton outside the compact `i16` range.
- Independent source and mathematical audits passed both changes.
- A broad 1,437-test foundry invocation was intentionally stopped in an
  unrelated slow K6 debug campaign after 5m24s of test execution. It had no
  reported failing test, but is not counted as a completed suite. The focused
  completed results above are the test gate.

The release executable containing both domain fixes passed fresh K1/K3/K6
generation, separate certification, cold inspection and canary application.
Candidate bytes, artifact bytes and inspection/reduction output are unchanged
from the previous release. K6 still has 5,640 cells, 38 terminals and an
8,925,944-byte artifact; its separate certification took 1.167 s in this single
shared-host control (1.363 s whole process). This is not a repeated performance
comparison. The subsequently added internal rank primitives were tested in the
focused gate, not silently attributed to this earlier frozen release build.

Full saved-bundle FG and H recertification now terminate at the explicit
endpoint-storage preflight rather than the previously captured domain failures:

| Saved candidate | Wall time | User CPU | Peak RSS (KiB) | Endpoint cells requested / allowed |
| --- | ---: | ---: | ---: | ---: |
| FG | 214.75 s | 203.54 s | 5,258,740 | 11,600 / 8,192 |
| H | 280.63 s | 266.51 s | 7,849,036 | 8,720 / 8,192 |

Both exited with status 8 and produced no certified artifact. The endpoint
preflight counts `(3 * rhs_terms + 4) * arity * 2` storage cells; the FG count
corresponds to a 192-term K10 rule and H to 144 terms. These are explicit
resource-policy failures, not evidence of an invalid identity or coverage gap.
Separate 1,200-second-bounded retries used the existing caller option
`--max-domain-bound-endpoint-cells 32768`; defaults and coefficient limits were
unchanged. Both reached further checks and failed closed without artifacts:

| Candidate | Wall time | User CPU | Peak RSS (KiB) | Next failure |
| --- | ---: | ---: | ---: | --- |
| FG | 260.03 s | 244.24 s | 15,460,832 | Guard separable-factor work preflight requests 859,963,392 units, limit 64,000,000 |
| H | 280.23 s | 266.15 s | 9,236,444 | Guard zero locus not proved outside the complete affine application domain |

The FG result is another resource-policy failure; the H result is an unresolved
domain implication. Neither demonstrates a missing candidate IBP, nor proves
that every candidate is valid. These errors currently lack the offending
retained rule's location. A follow-up diagnostic adds sector, retained ordinal
and fixed coordinates to opaque lowering errors, without erasing typed resource
or affine errors. The next proof investigation must reproduce that precise
obligation, not repeatedly increase all limits. A successful future run will
still need an independent cold load under its declared resource policy.

### Isolated H282 obligation

The existing release `family-close --progress` route locates H's failure at
retained rule 53/80 (one-based display) in sector mask 282. Restricting the
caller-supplied root to that sector, without changing the input family or any
solver policy, reproduces the same failed guard in about one second. This is
a diagnostic subfamily, not a replacement for the required complete campaign.
The full H attempt took 289.61 s including regeneration; FG took 261.31 s and
located its factor-work failure at sector 115, retained rule 83/177.

The H guard, after its actual singleton `n0=-1` is substituted, is

```text
P = (2*n9-n7)*(n7+n9+1) + d*(n7-4*n9),   n7 <= -1, n9 <= -1.
```

For this polynomial to vanish identically over the coefficient field `Q(d)`,
both coefficients must vanish. Since `n7+n9+1 <= -1`, the constant coefficient
requires `n7=2*n9`, and the `d` coefficient requires `n7=4*n9`. Together they
force `n7=n9=0`, outside the actual domain. This is not a proof that `P` has no
root at any specially chosen numerical dimension; the existing coefficient-
field guard contract is the one being checked.

The conjunction service already uses native affine elimination to derive fixed
coordinates. The new implementation compares those joint fixed consequences
against the actual domain box before returning inconclusive. Physical index
values are converted to the existing active/inactive local coordinates; both
signs and finite endpoints are checked. Widening the box to admit `(0,0)` is a
failing negative control. No topology-specific identity or new elimination
algorithm is introduced. Independent source and mathematical audits passed.

### Isolated FG115 coefficient scheduling

The second capture is a guard polynomial of degree eight in `n2`, `n8`, and
`d`. Its target includes `-1-n8+2*n3=0`, and one complete exclusion is the
conjunction of that equality with `1-n8+n2=0`. The coefficient of `d^6` in
the original guard is exactly

```text
648 * (1-n8+n2)^2.
```

On the target outside that whole exclusion, this coefficient cannot vanish.
It alone therefore proves that the entire guard is a nonzero element of
`Q(d)`. The earlier execution attempted expensive factor work on a larger
coefficient first and exhausted its allowance before reaching this short proof.

The new scheduling admits the complete original polynomial, coefficient system,
target, and exclusions first, then visits coefficient equations in increasing
native term count with a deterministic original-order tie break. Existing
Symbolica factorization and exact implication services do the algebra. All
coefficient equations remain available for simultaneous-conjunction reasoning,
and all visits share the existing work budgets. No failed proof is suppressed,
no limit is raised, and an exclusion's unrelated conjuncts cannot be discarded.
The full captured polynomial, actual zero points, absent/incomplete exclusions,
and swapped dimension-degree positions are included in the focused regressions.
Independent source and mathematical audits passed. The combined source-port
gate passes 268 tests, with zero failures and five existing ignored tests;
the same frozen debug test binary passes 44 affine-domain and 15 completion
tests. Both captured obligations and their negative controls pass. Two initial
test-setup failures (private box-constructor access and a negative fixture
canonicalizing to a coordinate-only case) were corrected without weakening
the proof assertions; their original logs are retained. The fresh optimized
build and complete saved-candidate recertification are pending.

Focused evidence: `/tmp/rustred-guard-scheduling.cfXaNt/`.
Fresh release evidence: `/tmp/rustred-joint-guard-release.VS9hEJ/`.

Evidence directory: `/tmp/rustred-relative-domain-release.pxuNef/`.
The release binary SHA-256 is
`6a3aebaa5239c6ff60251be36512d59c44b82284c60c059f5264744f7a25ba08`.

Neither change is topology dispatch, a new CAS implementation, an increased
proof budget, or a relaxation of source replay, guards, descent or coverage.
The proposed rank-scoped certification service will reuse these same exact
domain semantics; see [the rank-bound recommendation](rank_bounded_certification.md).

### Fresh FG recertification resource-bound evidence (2026-09-17)

The resumed release recertification used the unchanged saved FG candidate
bundle and the normal exact replay/publication path. With an endpoint-cell
limit of `32768`, it reached this precise diagnostic:

```text
sector [true,true,false,false,true,true,true,false,false,false]
retained rule 157/177 (zero-based)
fixed [1,1,-1,0,1,1,1,0,-3,0]
combined domain bound endpoint cells: requested 33080, limit 32768
```

That run took 468.00 s wall, used 15,428,848 KiB peak RSS, and exited with
status 8 without writing an artifact. This is a resource-policy rejection,
not an identity, descent, or coverage proof.

Raising only the endpoint-cell limit to `65536` did not establish closure. A
second release run took 310.94 s wall, used 25,225,696 KiB peak RSS, and then
failed closed at the next exact aggregate-cover preflight:

```text
artifact combined cover boxes requires 288047 units, limit is 65536
```

It also exited with status 8 and produced no artifact. These measurements show
why repeatedly increasing a global cap is not a certification strategy:
endpoint and aggregate-cover structures have distinct cumulative costs, and
the latter can grow substantially after the former succeeds. No FG closure
claim should be inferred from either run.

The safe scoped-certification direction remains an explicit
successor-closed proof domain. A future rank-bounded mode must persist the
admitted entry domain and an exact finite union of coordinate/affine slices,
then verify: (1) every admitted entry is in that domain or a proved
zero/terminal; (2) every applicable retained rule is replayed and descending;
(3) every nonzero RHS image is contained in the same domain or an independently
checked terminal; and (4) the same domain and checks are reproduced on cold
load and runtime entry. Existing rank-slice and successor-scope helpers are
useful building blocks, but until these obligations are wired into the durable
schema they must not be exposed as a bounded closure certificate. Increasing
budgets, finite sampling, and finite-field agreement remain diagnostics only.
