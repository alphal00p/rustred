# Shared domain routing and bounded guard-resource refinement

## Scope

This slice reuses the saved five-loop programs. It neither regenerates IBPs nor
claims complete R10 closure. The required scope remains all 67 external graph
classes, unbounded positive denominator powers and unclipped intermediate
numerator ranks. Independent certification and terminal minimization remain
deferred; these operations discover and reuse application work.

Two changes address the preceding measured stops:

- Selected native GCD/factorization preflight refusals can use the existing
  bounded inactive-coordinate refinement. The identical predicate, rank and
  dispatch priority are retried. Work already attempted remains charged.
  Insufficient optional refinement returns the original typed error. Actual
  backend failures, output limits, cancellation and global work limits are not
  silently converted into geometric uncertainty. Local-match diagnostics now
  retain the active predicate and coordinate box.
- A separate admitted-route visitor streams rank-preserving support-domain
  covers without expanding numerator polynomials. Full target roots enter
  Apply, strict subsupports reenter Route, and the queue keeps these phases
  distinct. Domain/rank reuse saves scheduling, not completed-work evidence.
  Missing maps and unchecked source conditions remain explicit obligations.

The implementation is generic in topology and const-generic arity; all census
labels, families, programs and momentum maps remain external inputs. Native
Symbolica algebra is unchanged. No CAS/reconstruction kernel was added.

See [the public interface and limits](../shared_owner_domain_matching.md) for
the Rust service and CLI/Python options. The route operation is explicitly a
conservative cover, not an exact numerator expansion. Its extra points cannot
by themselves establish reached missing rules or new masters. Local-match
predicate provenance is not yet forwarded through the RHS-inspection wrapper.

## Rank accounting

An admitted map produces endpoints `B-e`: active denominator powers supply
nonnegative `B`, and numerator monomials obey `|e|<=D`. Thus endpoint numerator
rank is at most `D`, and positive support is a subset of the mapped root.
Removing `k` active denominators needs at least `k` numerator degree; the
visitor avoids enumerating impossible support losses when `k>R`.

The current implementation deliberately retains incoming rank `R` on every
cover. Independent analysis identified a safe subsequent tightening for this
exact-support API: `rank(B-e)=|e|-sum(min(e_i,B_i))<=R-k`. This requires counting
loss from actual positive support, not an arbitrary larger transport root.
It has not been applied to the baseline measured here.

Strict local descent alone does not bound intermediate rank uniformly over
unbounded dots. For example, `(p,-r)->(p-1,-r-1)` can descend at every concrete
step while allowing arbitrarily large intermediate rank as initial `p` grows.
The worklist therefore reports maximum admitted finite rank and the number of
unbounded-rank domains. These are scheduled overcover scopes, not claims that
every enclosed point is reached. Any growing-rank chain needs its guards,
coefficient support and ordering examined; it must not be clipped to R10.

## Validation and measurements

The core release gate passes **2,623 tests**, with zero failures and 32 existing
ignored diagnostics. Focused selections pass 7 resource-refinement tests,
9 route-cover tests and 48 owner-domain tests. An independent runtime review
checked the final binary and result logs. Full core runtime is 157.80 seconds;
this is test-suite time, **not** five-loop solving time.

The application gate passes **314 tests** (233 unit and 81 integration), with
zero failures; its focused domain selection passes 26 tests. Python steering
passes **19 tests**. The final production executable was frozen after the full
test stage: an earlier copy differed following a Cargo rebuild, and both early
launches were cooperatively stopped during preparation, before any domain
traversal. Those cancelled launches are not performance controls.

The controls below use the final frozen release executable, existing saved
programs, explicit work budgets, fixed affinity, inner pools of one and no
elapsed timeout. Compilation is outside their measured command boundaries.

| Trial | Preparation | Domain work | Whole command | CPU | Peak RSS | Result |
|---|---:|---:|---:|---:|---:|---|
| Full67 R10 local matching | 104.41 s | 120.24 s | 229.44 s | 227.74 s | 6.63 GB | Incomplete: 59/67 locally resolved |
| Six R10/R11 inputs, shared routed work | 106.39 s | 7.42 s | 120.72 s | 119.21 s | 5.95 GB | Incomplete: containment budget |

These are individual shared-host observations, not a speedup ratio or IBP
generation timings. Both select the complete saved 67-owner library with 8,246
route records. The second seeds only six previously recorded boxes, not the
complete census. It also uses larger explicit work budgets than earlier
three-owner diagnostics, so comparing their elapsed times would not be a
matched-workload benchmark. The native processes are terminal and reaped.

### Full-census local result

All 67 queries were visited; 59 finished with complete local applicability.
The retained prefix contains **213,523 selected-rule pieces, 893 terminal
pieces and 163 unresolved pieces**, with no exact gaps or invalid source
conditions. Terminal pieces are not a count of distinct masters. Seven earlier
owners retain 156 unresolved pieces; the last owner has seven more and a typed
native preflight refusal. Nothing here establishes recursive RHS closure.

Of the 163 unresolved pieces, 147 concern excluded conjunctions and 16 concern
case equalities. Every piece has two to four unbounded positive coordinates;
141 have no free inactive coordinate. The other 22 retain inactive directions,
but the counters and splitter policy indicate these are absent from the active
predicate support. No query exhausted the 65,536-face refinement allowance;
the largest incomplete-query count is 2,753. Raising that allowance alone is
therefore not the next remedy.

The last refusal is `OriginalDenominator { batch: 0, rule: 534, term: 1 }` in
external owner `011101110111000`: estimated factor work **66,650,112 > 64,000,000**.
Only original positive axes 1 and 2 are free, with powers at least 6 and 3;
every inactive coordinate is already zero. This is a prospective native
algebra refusal, not a measured failed factorization or a missing IBP, and
further numerator-rank refinement cannot split that box. The exact query and
unchanged one-owner selection were saved for a small subsequent replay.

The preceding census attempt on the same saved programs stopped in its first
owner with zero completed queries. Passing that obstacle and reaching all 67 demonstrates
useful refinement progress, not complete family coverage.

### Shared routed result

All six original input boxes completed local RHS inspection. The walk completed
**870 of 9,595 admitted domains**, with one failed current domain and **8,724
queued**. It inspected **161,842 successors** (87 conditional), reused **157,372
requests**, and completed ten route visits emitting ten full-root Apply covers
and 5,110 strict-subsector Route covers. No local gap, RHS problem, missing map
or source-validity frontier was observed in this prefix.

The stop is the application's **100,000,000 containment-comparison allowance**,
not native algebra, a timeout or RSS pressure. The admission ledger closes:
6 initial requests + 161,842 RHS requests + 5,120 route requests = 9,595 new +
157,372 reused + 1 refused. The largest admitted finite rank is R12, while
processed domains have ranks 9–11; this does not prove that an R12 point is
actually reached or that rank growth is unbounded.

The next immediate optimization is the containment index, followed by a
same-input/policy rerun. Separately, investigate sufficient selection of a
later already-valid rule on positive-coupled guard domains: dispatch priority
is policy, while source validity and each chosen identity remain mandatory.
An undecidable skipped guard must never become an exact gap if later rules do
not cover it. The proved strict-pinch `R-k` bound above is another independent
image-tightening opportunity. None licenses clipping intermediate rank or
promoting an unresolved domain to a master.

## Reproduction evidence

- Core gate: `TMP/resource-route-core.G381iL/`.
- Application gate: `TMP/shared-domain-routing.mWNK1g/`; its earlier frozen
  binary is superseded and its preparation-only trial is excluded.
- Final CLI and full67 runner/results:
  `TMP/shared-domain-routing-controls.RWWK6m/`.
- Routed runner, supervisor, counters and independent review:
  `TMP/six-query-routed-owner-walk.yOCjF1/`.
- Final CLI SHA-256:
  `e52b0590a1aad7fe4ff09ba30e6be8b7472d6898bd1fe729e06f730677c8b2b0`.

The full67 process used CPU40 and a 64 GiB address-space limit. The routed
process used CPU41, the same address-space limit and an owned-process RSS
monitor with 48/60 GiB soft/hard thresholds. Neither hit a memory boundary.
Both use the public Python driver and preserve unchanged saved programs. TMP
evidence and vendored/reference material are not part of the Git milestone.
