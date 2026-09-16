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
Separate 1,200-second-bounded retries use the existing caller option
`--max-domain-bound-endpoint-cells 32768`; defaults and coefficient limits remain
unchanged. A completed retry will need an independent cold load under the same
declared resource policy before any publication claim.

Evidence directory: `/tmp/rustred-relative-domain-release.pxuNef/`.
The release binary SHA-256 is
`6a3aebaa5239c6ff60251be36512d59c44b82284c60c059f5264744f7a25ba08`.

Neither change is topology dispatch, a new CAS implementation, an increased
proof budget, or a relaxation of source replay, guards, descent or coverage.
The proposed rank-scoped certification service will reuse these same exact
domain semantics; see [the rank-bound recommendation](rank_bounded_certification.md).
