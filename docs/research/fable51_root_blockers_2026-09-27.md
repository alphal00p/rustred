# Why roots 25, 35, 42, 43 were closed in the interim campaign but not in v2 (2026-09-27)

Question (user): the interim campaign (`five-loop-qcd-feynman-d9d10`, binary 32fdec, older
inputs with 60 of 67 helpers at unbounded positive power A, W50) showed 12 of 67 initial roots
closed from ~4 h on, while v2 (`five-loop-qcd-feynman-d9d10-v2`, binary 102adcc3, inputs with 54
helpers bounded at A_max, W100) stayed at 8 for 16 h despite ~60% more completions.

Method: per-root closed flags read from both campaigns' checkpoints (interim CP4 generation 6,
v2 CP5 generation 6); initial-domain ids mapped by replaying initial admission of each
`inputs/queries.json` (67 initial domains in both, same owner sequence); then an ignored
diagnostic test (branch `fable_5_1-root-diagnostics`, 886387aa) restored a block clone of v2
generation 6 through the full production restore path (bindings, digests, validation) and walked
the dependency graph from every root. Receipt: `TMP/root-blockers-gen6/receipt.json` (three runs,
identical per-root numbers). Workflow `wf_75768a9e-6e9`: one analyst, two read-only verifiers.

## Measured
- The 8 v2-closed roots {0, 2, 12, 14, 15, 23, 24, 26} are a subset of the interim's 12; the extra
  four are ids 25, 35, 42, 43 (owners 001001000101111, 001001011101001, 101101100101000,
  011100111100001). Three of the four have the SAME helper box in both input sets (unbounded A,
  rank 12/13); only 43 differs (A unbounded vs A <= 22).
- v2 generation 6 (16.2 h): 68.87M nodes, 1.128G edges, 25.7M unsealed nodes, 0 frontiers.
  Forward reachability reproduces the persisted closed set exactly.
- The four roots have small reachable regions (10.7K-47K nodes). Every node blocking them is a
  pending queue entry (never inspected, no frontier): 4,227 blockers for 25/43 (ids 48.4M-62.8M,
  3.8M-14.4M unreserved obligations ahead in id-ordered Ready reservation), the same 20 blockers for
  35/42 (ids 61.73M, 13.6M obligations ahead). Their regions escape both inputs' helpers through
  rank growth (ranks 13-17 vs helper ranks 11-13) and Route phase, not through A.
- For root 42 each dependency level was admitted at the tail of the global queue one pass after its
  parent (largest id per level 0.21M, 1.80M, 3.26M, 5.38M, 8.68M, 14.36M, 23.71M, 38.72M, 61.73M);
  for 25/43 many levels sit within ~1M ids, so "depth" is not "passes" there.
- None of the nodes reachable from the four roots is contained in the interim's initial helper box
  of its owner: the narrow hypothesis "the interim's unbounded helpers absorbed their descendants"
  is refuted for the roots' own regions.
- Globally, 8.84M non-initial v2 nodes (12.8%) are contained in an interim initial helper but not in
  the v2 one; removing them and everything reachable only through them would drop 43.5% of the
  v2 graph and 54% of the unreserved queue (an upper bound: the interim's own inspections of its
  larger boxes are not modelled).

## Not established (verifier findings)
- Why the interim reached these chain ends sooner. Plausible contributors: a shorter queue (the
  upper bound above), different worker count (50 vs 100) and binary (32fdec vs 102adcc3), and
  absorption into interim NON-initial representatives, which this analysis did not test. The
  interim's queue was not measured.

## Consequence for the next push
- Root closure under id-ordered (breadth-first) Ready reservation completes a small, deep root
  region only when the global queue has advanced past its deepest level: closure progress (and the
  size of the pending set) are governed by scheduling order. A closure-aware scheduling policy
  (priority for nodes in nearly-closed regions, depth-first within a root's region, or containers
  before contained) is a lever for incremental certification and pending-set memory, with a
  possible effect on total work through admission order; it belongs in the v3 plan with a <= 1 h
  pilot comparing closure progress and total admitted domains.
- Helper shapes change total work substantially (the 43.5% upper bound), but unbounded helpers on
  owners that route into guard-sensitive owners produce frontiers; the input decision needs the
  owner route graph, not this per-root test.
