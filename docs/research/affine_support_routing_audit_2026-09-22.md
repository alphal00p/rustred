# Independent audit: support-aware affine routing

Date: 2026-09-22. Scope: the pending support-aware routing changes on top of
`74147f9c`, reviewed independently of their implementation. This is a source and
mathematical audit; the integrator owns release testing and campaign measurements.
No build, solver campaign, or implementation edit was performed by this audit.

Integrator follow-up: the release gate passes 2,768 core tests (32 existing
diagnostics ignored), 361 application-library tests (one index replay ignored),
82 integration tests, and all 72 Python/API/steering tests. The focused native
routing gate passes 40 tests. Logs are under
`TMP/affine-support-gate.cv24Y0/`; the fresh Python extension and frozen CLI
were selected explicitly. These gates include pre-existing unrelated scheduler
working-tree changes. The independent source verdict below is unchanged;
campaign performance is reported separately.

## Verdict

**PASS at the source/mathematical level; the release gate is now green.** No
correctness blocker was found. The change safely tightens an existing conservative
image cover; it does not prove endpoint reachability, missing-rule status, or
family closure. No campaign speedup is claimed from this inspection.

## Mathematical argument

The admitted map is a unit bijection on active denominator rows and an affine
polynomial substitution on inactive rows. Write an inactive source denominator
as `P_i = c_i + sum_j M_ij Q_j`, raised to its nonnegative numerator power `t_i`.
In every expanded monomial, the degree in target variable `Q_j` is at most
`sum_{i in S_j} t_i`, where `S_j` contains exactly the inactive rows with
`M_ij != 0`. Constants cannot increase this degree, and coefficient cancellation
can only remove monomials.

For a projected source box `l_i <= t_i <= u_i` with total numerator degree at
most `Rmax`, every endpoint therefore satisfies

```text
degree_j <= C_j
C_j = min(sum_{i in S_j} u_i,
          Rmax - sum_{i inactive and not in S_j} l_i).
```

Absent bounds are genuine infinity. An empty support gives `C_j = 0`, even for
unbounded source rank. The implementation uses this same formula, with checked
wide sums and the already projected source bounds. These are independent
necessary bounds, not a claim that all column maxima can occur together.

If the positive source exponent mapped to target `j` is `1 + x_j`, with
`x_j >= L_j`, then the surviving positive local exponent has the safe lower
bound `max(0, L_j - C_j)`. Removing that denominator requires
`degree_j >= 1 + x_j >= L_j + 1`, hence is impossible when `C_j <= L_j`.
The implementation correctly uses a strict `C_j > L_j` pinch test; there is no
off-by-one loss at a zero target exponent.

For several simultaneous pinches, the existing total weighted cost remains
necessary and is still checked. In particular, two columns may individually
allow degree two without jointly allowing four units of cancellation. Each
pinched child starts from the source-derived bounds, not the more restrictive
projection of the all-positive sibling. Its newly inactive axes reset local
lower bounds to zero. Both details are essential to conservatism and are
implemented correctly.

## Implementation checks

- `integral_transport/compile.rs` builds immutable CSR incidence using the exact
  Symbolica coefficient `is_zero` operation. It includes only inactive source
  rows, lists each row once per target, and preserves strictly increasing order.
  Affine constants are deliberately excluded. There is no new CAS kernel,
  coefficient clone per worker, or numerator expansion in this service.
- Allocation sizes are bounded by the previously admitted matrix dimensions;
  the support-offset length and entry count use checked arithmetic. The public
  artifact schema is unchanged because this cache belongs to prepared runtime
  routing state.
- `domain_overcover/support.rs` distinguishes infinity from wide finite sums,
  handles empty supports, and rejects arithmetic/invalid-domain failures rather
  than silently saturating. The surviving-lower conversion cannot truncate a
  positive result because it is bounded by the original `u64` lower bound.
- `domain_overcover/visit.rs` enables the new tightening only for the correlated
  power-bound lane. The default visitor retains its former output/accounting.
  Every rejected subset still consumes a mask allowance. Cancellation,
  source-condition obligations, strict-subsector reentry, and non-closure
  semantics remain intact.
- The new lower bound is a property of every transported term. It does not
  reimpose the original campaign's A/R/D starting limits on descendants. Their
  translated bounds continue through the existing projection pipeline.
- The existing explicit rejection of a finite affine-routed rank wider than
  the public rank representation remains fail-closed; it is not replaced by
  infinity or a clipped value.

## Test coverage inspected

The slice adds three support-arithmetic tests and seven native integration
tests. The arithmetic differential test exhausts more than 10,000 small source
boxes/support masks, including multiple relevant and irrelevant source axes.
Other checks cover infinity, empty support, wide finite sums, overflow, and the
exact pinch threshold. Native tests compare the support cache to Symbolica's
verified matrix and cover finite source powers without an explicit rank cap,
zero-degree affine rows, total weighted pinch cost, pinched-axis lower reset,
unbounded rank, cancellation, and source-condition obligations.

The existing exact affine-transport differential test still checks more than
500 concrete endpoints, including affine constants and double pinches. Existing
default-lane equivalence and charged-pruning tests remain relevant. This audit
inspected their assertions and subsequently checked the integrator's saved logs;
it did not execute the tests independently.

The logs in `TMP/affine-support-gate.cv24Y0/` report:

| Release gate | Result |
|---|---:|
| Focused routing tests | 40 passed, 0 failed; 0.11 s |
| Full core library | 2,768 passed, 32 ignored, 0 failed; 164.08 s |
| Application library | 361 passed, 1 ignored, 0 failed; 38.36 s |
| Application integration binaries | 82 passed, 0 failed |
| Python API, matcher, supervisor | 50 + 10 + 12 passed, 0 failed |

The receipt identifies the frozen CLI SHA256 as
`e870efc3a51f8ec515b7efffcee087e98620c4c5e4dc2639325af0cb6d163a0a` and
the matching freshly built Python extension. It also explicitly records
pre-existing scheduler/escrow working-tree changes: these results validate the
combined gated tree and are not a clean-commit attribution to this routing slice
alone. The matched campaign remains a separate performance/coverage experiment.

One non-blocking future test improvement is a native affine fixture with at
least two inactive rows and asymmetric column support, exercising the complete
CSR-to-routing path with the mandatory irrelevant lower-degree term. The pure
arithmetic exhaustive test already covers that term, while the current small
native affine fixture has one inactive source row.

## Performance interpretation

This change can remove impossible route masks and retain stronger surviving
positive lowers, thereby reducing later queue admission work. It does not make
the containment index asymptotically faster and does not establish that the
five-loop campaign now finishes. Measure it on the same saved owner snapshot,
queries, ordering, worker budget, and stop policy before attributing any end-to-end
benefit. Counts of conservative overcovered points remain distinct from actual
reachable integral keys.
