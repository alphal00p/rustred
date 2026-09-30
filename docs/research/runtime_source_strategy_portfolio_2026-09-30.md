# Runtime source-strategy portfolio: 4L confirmation and small 5L transfer

Sparse-row/coefficient priority improved the tested 4L **generation + saved-program
walk + cold verification** boundary in both isolated, counterbalanced pairs:
17.45% and 18.73% below the fresh default baseline. The small natural-coordinate
5L scout also reduced work, but is not a full 67-owner qualification. All measured
walks passed their exact cold-All scope. Stronger source certification did not pass.

This result concerns **newly generated programs**. Changing a discovery strategy
does not accelerate walking already saved programs. No production owner library
was regenerated or replaced, and these data do not establish an Epoch speedup.

## Frozen engine, recipes and authority boundary

Every arm used the same real optimized campaign executable:

- Source: `1b33ad2956d87f29db6ef9987542f3364f443814`.
- Binary: `TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-1b33ad29`.
- SHA256: `73253922552ef341e3e97522d9481a4e369d388e9f8612c4ac92468c3c584a14`.
- Frozen Python/tool source: `TMP/codex-parallel-campaign.oiPK29/python-delivery-62c763cd`.
- Fixed walker: Ready + Union, W16, CPUs 64–79; sparse exact generation,
  numerical depth 2, finite-case search. LC2 remained untouched on CPUs 128–227.

All recipes use version 1, `sectors: {kind: active-first}`. Only source-row
discovery priority changes; original row IDs and integral ordering stay intact:

| Recipe | Row descriptor, in lexicographic priority order |
| --- | --- |
| Default | `input-order`; generation omits `--discovery-strategy` |
| Sparse-coeff | Ascending `terms`, then `coefficient-monomials` |
| Shift-cost | Ascending `absolute-shifts`, `positive-shifts`, then `coefficient-monomials`; all coordinate weights 1 |

Exact JSON lives in `TMP/codex-runtime-pivot-portfolio.78ueFJ/strategies/` and
`five-transfer/strategies/`; shift weights have arity 10 and 15 respectively.
These are runtime inputs to one common finite plan, not per-recipe Rust builds.
This is implementation A (row/discovery scheduling), **not** the unimplemented B
persisted integral-order comparator. No minimal-master-basis claim follows.

The public-core plan diagnostic rematerialized all 508 prepared 4L sector plans:
both nondefault recipes differ from default in all 508; sparse and shift differ
in 341 and coincide in 167. Its executed b95 core tree exactly equals the frozen
engine's core tree, and family/configuration fingerprints match generation. These
are constructor plans, not traces of every later case. Exact 5L per-sector plan
equality was not measured; descriptor labels alone are not such evidence.

## P1: common 4L exploratory portfolio

Each recipe regenerated both complete roots `1111111110` and `0111111111`, selected
16 fresh owner payloads, retained all 508 routes and the byte-identical 58 Required
queries with zero Auxiliary rows. Natural coordinates, row order, bounds and roles
were unchanged. Cold verification used explicit `all-roots`, reinspection `All`,
reference levers off, and verified all 58 queries / 32 admitting roots.

Times are seconds. Generation/cold are guard wall times; native includes owned
process-group drain. The last column excludes separately failed certification,
staging and orchestration: it is a matched boundary, **not** whole-pipeline time.

| Recipe | Generation | Native + drain | Cold | Native + cold | Generation + native + cold | Native inspections |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Default | 78.356 | 7.778 | 12.163 | 19.940 | 98.296 | 24,388 |
| Sparse-coeff | 56.360 | 4.952 | 8.160 | 13.111 | 69.471 | 15,326 |
| Shift-cost | 65.332 | 4.830 | 7.174 | 12.003 | 77.335 | 16,638 |

These first runs were concurrent exploratory measurements. Sparse was selected
for generation-inclusive confirmation; shift's single-run 1.108s native + cold advantage
over sparse was insufficient to establish it as the winner.

| Recipe | Rules, both root downsets | Coefficient bytes, both downsets | Selected 16-owner payload bytes | Domains | Events |
| --- | ---: | ---: | ---: | ---: | ---: |
| Default | 44,943 | 92,468,229 | 6,562,373 | 66,551 | 1,895,228 |
| Sparse-coeff | 18,628 | 76,167,043 | 5,127,975 | 38,666 | 870,982 |
| Shift-cost | 18,202 | 75,407,306 | 5,113,526 | 40,747 | 750,906 |

All three have 831 finite-residual integral records: the **non-deduplicated
386 + 445 sum across both root downsets**, not exact terminal keys of the selected
16 owners or independent masters. Exact selected terminal-key equality is unknown.

All six stronger `certify-candidates` attempts failed: five exhausted the native
affine-literal consistency proof budget; default root511 hit candidate collection
entry ingress limits. No certified artifact or successful standalone source-replay
receipt exists. Their guard costs were 55.323 / 67.323 / 70.324s; adding these to
the matched boundary gives 153.620 / 136.794 / 147.659s before other preparation.
Failures were retained; limits were not raised and artifact inspection was skipped.
Cold-All establishes the saved-program obligation graph, not the missing stronger
original-source certificate. These remain explicitly uncertified diagnostics.

## Isolated 4L confirmation

Fresh generation was repeated for every arm in order default → sparse, then
sparse → default. No other authorized compiler or native pilot overlapped; LC2
continued. Every guard acquired global heavy plus local pilot-p1. The optional
known-failing stronger certification was explicitly omitted, not relabelled PASS.

| Pair / actual order | Default generation + native + cold | Sparse generation + native + cold | Sparse reduction | Default native + cold | Sparse native + cold |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1: default → sparse | 103.290 | 85.268 | 17.45% | 21.941 | 13.930 |
| 2: sparse → default | 103.875 | 84.421 | 18.73% | 21.525 | 13.080 |

All four cold checks passed all 58 queries / 32 roots, with respectively
24,197 / 15,344 / 15,310 / 24,432 native inspections in execution order. Checkpoint
file inventories, content digests and exact string-encoded mtimes were unchanged.
Whole arm clocks were 107.041 / 88.195 / 88.423 / 107.508s, below each 1800s cap.
All 16 confirmation process groups drained and both locks were independently free.
This confirms the scoped recipe improvement; it does not certify a replacement
source library, a 5L production speedup, or a parallel-engine performance threshold.

## P3: natural-coordinate small 5L transfer

All three recipes used the exact earlier P2 **natural** family and query, with
identity coordinate permutation, one Required query containing 784 bounded entry
points (`A <= 10`, `R <= 1`, `D >= 9` plus the original coordinate bounds), and
14 newly generated literal owners below root `101101100101000`. Descendants were
not clipped. This is not the original 67-owner / 183-query workload. Stronger
artifact certification was not retried; no certificate claim is made.

| Recipe | Generation | Native + drain | Cold | Generation + native + cold | Native inspections | Rules | Coefficient bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Default | 21.160 | 1.575 | 2.176 | 24.911 | 2,841 | 4,283 | 5,639,668 |
| Sparse-coeff | 15.156 | 1.360 | 1.154 | 17.670 | 1,943 | 698 | 2,091,216 |
| Shift-cost | 15.160 | 1.450 | 1.157 | 17.767 | 1,933 | 697 | 2,083,983 |

Every arm passed explicit all-roots / All, independently verifying the one exact
query and root, with zero violations and unchanged checkpoint bytes/mtimes. Each
has 14 finite-residual records; exact terminal-key equality/minimality is unknown.
Sparse and shift are effectively indistinguishable in these short scout timings.

S1's cold phase overlapped default generation/walk/cold and sparse generation.
Sparse walk/cold and all shift work occurred after S1 drained. Therefore the
three timings are **not an isolated matched qualification**. Exact phase start,
receipt-end times and groups are retained in each summary. All nine P3 groups
drained; whole clocks were 140.797 / 94.699 / 81.328s, including orchestration.

## Resource receipts and audit

CPU below is actual waited-child user + system seconds; RSS is maximum single
waited child in KiB, not simultaneous aggregate tree memory. Columns are
generation / native / cold. The short walks had no 10s contention samples.

| Scope / recipe | CPU seconds (G / N / C) | Peak RSS KiB (G / N / C) |
| --- | --- | --- |
| P1 default | 794.706 / 30.868 / 56.856 | 750,832 / 229,684 / 106,980 |
| P1 sparse | 608.019 / 15.461 / 21.218 | 662,768 / 168,388 / 70,632 |
| P1 shift | 604.649 / 13.822 / 17.252 | 607,772 / 152,884 / 70,576 |
| Confirmation 1 default | 768.471 / 30.280 / 65.480 | 789,140 / 245,060 / 104,688 |
| Confirmation 1 sparse | 595.121 / 14.546 / 22.160 | 659,748 / 156,400 / 73,160 |
| Confirmation 2 sparse | 599.250 / 14.527 / 18.696 | 656,672 / 153,000 / 70,616 |
| Confirmation 2 default | 775.760 / 28.965 / 68.181 | 809,044 / 237,584 / 113,436 |
| P3 default | 146.304 / 2.552 / 2.099 | 373,620 / 139,864 / 147,164 |
| P3 sparse | 71.797 / 2.024 / 1.405 | 291,052 / 73,248 / 73,460 |
| P3 shift | 72.246 / 1.964 / 1.380 | 295,872 / 72,340 / 73,500 |

Evidence root: `TMP/codex-runtime-pivot-portfolio.78ueFJ/`:

- `binary-binding.json`, `bound-matrix.json`, `resource-amendment.json` and
  `plan-dump-functional/RESULT.md`: identity, exact scope and realized 4L plans.
- `runs/{default,sparse-coeff,shift-cost}/summary.json`: P1 raw receipt pointers.
- `confirmation/{pair1,pair2}/runs/{default,sparse-coeff}/summary.json`: isolated
  generation/walk/cold records and exact guard arguments; the isolated resource
  amendment documents the changed global-lock ownership.
- `five-transfer/runs/{default,sparse-coeff,shift-cost}/summary.json`: P3 scope,
  metadata, timing, resources and overlap; `scout-resource-amendment.json` records
  S1-owned global heavy rather than an assumed root-held reservation.
- Each input receipt records fresh shard provenance; each `ready/cold.json` is
  the actual verifier result. Default P1's initial nanosecond mtimes were rounded
  by JSON orchestration, so its exact no-write evidence is inventory/size/SHA256;
  all subsequent mtimes were stored as exact strings. No input/checkpoint was rewritten.

Independent agent `parallel_gate_critique` audited P1, P3 and all four isolated
confirmation arms: exact scope/roles/routes, descriptor and owner bindings, actual
commands/locks, raw cold-All results and timing interpretation PASS. The repeated
generation-inclusive improvement is not a 1.5× claim.
