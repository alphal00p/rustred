# W0.3 native re-inspection harness: in-process scaling, root cause, verdict (2026-09-27, revised 2026-09-28)

Lane `harness`, branch `fable_5_1-v3-harness`, worktree `.claude/worktrees/fable51-csr`.
Master plan item W0.3; data and full tables: `TMP/w0/harness/RESULTS.md` and `TMP/w0/harness/tables/`.
Labels: **[M]** measured (run directory under `TMP/w0/harness/`, binary sha256), **[E]** estimate or inference.
No ETA and no closure claim.

## Fix round 2026-09-28 (independent verification): what changed

An adversarial verification re-derived the lane's numbers from the receipts (all spot checks matched) and raised
19 problems about how they were read. The corrections below are applied throughout this note.

1. **The per-CCX factor 1.14-1.17x is provisional** [M values, loaded socket; E as a projection]. Its only source
   is session E, which ran at 63-86% foreign load on the run CPUs (protocol-void as a timing A/B). In the same
   load, the shared copy measured 2.06-2.14x, against 3.75x on the quiet socket (session C), and 2.94x in session D
   at similar load (same code path and instruction count as session E's g1). Loaded K=96 ratios therefore vary by
   ~40% between sessions and cannot bound the quiet per-CCX value in either direction. Quote the same-session
   contrast (shared 2.06-2.14x -> per-CCX 1.14-1.17x -> per-CCX processes 1.155x), never 1.17 against the quiet
   3.75. Until a quiet session has measured g1, g12 and ccd12-C with >= 2 repeats each, **throughput projections are
   bracketed by 1.42 (p4, the best configuration measured on a quiet socket) and 3.75 (shared copy, quiet)**; 1.17
   may appear only as a labelled provisional scenario. The earlier "[E] a quiet value is expected to be lower" is
   withdrawn (section 6).
2. **Fill tables:** `tables/fills-D.md` and `fills-E.md` subtracted a socket-0, binary-B, single-prepare reference
   once per prepare from socket-1 runs with up to 12 concurrent prepares, giving negative far-DRAM fills and IPC ~6.
   Their cycle, IPC and DRAM columns are withdrawn; only the cross-CCX column was valid. Replaced by raw tables
   (`fills-C-raw.md`, `fills-D-raw.md`, `fills-E-raw.md`, prepare included, with a prepare bound).
3. **"DRAM fills flat from K=8 to K=24" was an artefact** of spreading one prepare over 2 vs 4 passes. With a prepare
   estimate removed, demand DRAM fills rise ~5.6e3 -> ~8.3e3 per native (+49%, [E], loaded session D); on the quiet
   session C (`any` fills) +8%. Both are small next to the 1.5e4 cross-CCX transfers per native that appear.
4. **The mechanism is now budgeted quantitatively at quiet K=96 (section 3)**: extra far-DRAM fills cover at most
   7-9% of the extra cycles, cross-CCX fills at an uncontended latency at most ~6%; >= 84% remains, which the
   cross-CCX transfers can carry only as contended (queued) transfers of ~1.1e4 cycles each [E]. "Explains every
   measured fact" is a qualitative statement.
5. **Placement:** the conclusion is limited to the owner-program/reducer data under the counting sink. The g3-h128
   control is one loaded run without numa_maps, and the host runs automatic NUMA balancing
   (`kernel.numa_balancing=1` [M]), so the remote copies may have been migrated. Layer and index placement for the
   epoch inspector stays an open W3.5 measurement (section 4).
6. **Root cause wording:** the split between Symbolica's context-count line and the variable-map-count line (written
   by `Arc<Vec<PolyVariable>>` clones in Symbolica or RustRed, or by false sharing) is unresolved (section 3).
7. **Workload mix:** the factor is for the gen-7 pending mix only; four-loop controls and C-5F were not measured with
   per-CCX copies (section 6).
8. **Units, baselines, noise:** RSS is GiB in the old text (5.1 GiB = 5.46 GB per extra copy; 12 copies 61.1 GiB =
   65.7 GB); K=1 base spread 3.5% is the noise floor of every gate ratio; session-C repeats agree within <= 1.5%
   (not 0.3%) and several session-C runs exceeded the 10% foreign-load rule; mimalloc is 2-4% at K=96 (n=2) and
   unresolved at K=1; binary-B ratios use a perf-record base and are not timing results.
9. **Confirmation sessions:** the load-gated session F (it re-checked the load only before waiting for the lock) was
   cancelled and replaced by `sessions/sessionF2.sh`: sessions F2 (g1/g12/ccd12-C/C-k1, 2 repeats each), G
   (placement: g3, g3-h128, g3 with all memory bound to node 4, numa_maps sampled; K=24 g1/g3) and H (workload mix:
   c4l-x, c4l-fg, C-5F, g1 vs g12), each <= 35 min, gated at < 10% busy before and after taking the socket-1 lock and
   before every run. Write-up owner: to be assigned by the orchestrator.

## 0. Verdict

**Recommended design (provisional on the quiet-socket confirmation): one process, with a private copy of the owner
programs / reducer per CCX (8 cores sharing one L3).** The in-process loss is not the allocator and, within the
<= 9% bound of the section-3 budget, not the placement of the owner programs. It comes from cache lines that every inspector thread *writes*: atomic reference
counts of the `Arc`s that all polynomials of an owner program share (Symbolica's `Arc<PolynomialContext>` and/or
the shared `Arc<Vec<PolyVariable>>` variable maps), plus data that sits in those same lines. Every native creates
and drops thousands of polynomials from them (section 3, instruction-level evidence).

- Today's design, one shared copy: CPU per native at K=96 is **3.75x** K=1 [M, session C: foreign load 8.6% and
  31.7% on the run CPUs for the two repeats, which agree within 0.03%]. Gate 0.3 (<= 1.3x) **FAILS**.
- The same process with 12 private copies, one per CCX (binary C, `RUSTRED_HARNESS_REPLICAS=12`), measured
  **1.17x / 1.14x** of the session-C K=1 base (1.13x / 1.10x of the same binary's own K=1; 1.15x / 1.12x of the mean
  of the two K=1 runs), in session E at 75-82% foreign load. In the same session one shared copy gave 2.14x / 2.06x
  and 12 separate 8-thread processes, one per CCX, 1.155x. Results are identical (differential proof on FG and X;
  per-native counts and stats on all 9,000 natives). **[M values, loaded; E, provisional as a factor]**: the gate
  value is met only on the loaded socket; gate 0.3 is not passed until the quiet confirmation (fix-round item 1).
- Within one node (K=24 on node 4, no remote memory at all) per-CCX copies take 1.50x to 1.11x in the same loaded
  session, cross-CCX fills 1.48e4 -> 2.76e3 per native, DRAM fills unchanged; quiet g-k24 is 1.46x with a 0.1% remote
  share. This is the primary evidence that sharing, not placement, drives the loss.
- Placement of the owner-program copies: the one control (g3-h128: three per-node copies all first-touched on node 4)
  cost the same as copies first-touched on their own node (g3), 1.226x vs 1.221x, identical cross-CCX fill counts,
  raw far-DRAM fills 560 vs 533 per native (g1: 5.1-6.8e3). This shows that the private copies remove the remote
  traffic; it does not show that the remote copies stayed remote (single loaded run, automatic NUMA balancing on,
  no numa_maps). The statement is limited to owner-program data under the counting sink (section 4).
- Caveat: the whole of session E ran at 63-86% foreign load (wall/CPU 1.16-1.71). Load affects the arms in ways that
  do not cancel: it lowered the contended shared-copy arm (2.06-2.14x vs 3.75x quiet, and vs 2.94x in session D at
  similar load), and it lowered the clock (cycles per user-CPU-second 2.74-3.08 G in sessions D/E vs 3.10-3.11 G in
  session C), which inflates every CPU-time ratio including the fully private process arm. The residual shared lines
  of the per-CCX arm (the process-global ahash counter and others, 4.8e3 cross-CCX fills per native) are exactly the
  kind of cost that load suppresses. The quiet per-CCX value may therefore be lower or higher than 1.14-1.17x.

**Rescaling factor** for W96 throughput projections (CPU per native relative to one thread on socket 1, gen-7
pending mix): **bracket 1.42-3.75 until the quiet confirmation**; per-CCX in-process copies **1.14-1.17
[E, loaded, provisional]**; shared copy **3.75** (a lower bound for the epoch inspector, which adds per-successor
work); add up to **1.54x** for inspectors on socket-0 SMT cores with a busy sibling. Details in section 6.

## 1. The question

Gate 0.3: CPU per native at 96 threads <= 1.3x the single-thread value, on the gen-7 pending sample
(9,000 stratified native-pending IDs of the v2 gen-7 checkpoint, `fixtures/gen7-pending-10k.json`), with the
walk's own `inspection::inspect` and a counting sink. The outcome decides whether the epoch engine runs ~90-136
inspector threads in one process or several inspector processes, and which factor every throughput projection
must carry.

The harness reproduces the walk's native streams exactly (differential proof, RESULTS.md section 2: FG 98,869,
BMW 147,233, H 24,680, X 46,826 and C-5F 980,945 natives identical; negative control detected) and the gen-7
restore cross-check passes (9,070 inspected natives: stats, accepted events and frontiers equal to the CP5
records, `crosscheck/crosscheck.json`) [M]. The harness does no admission, queueing, publication or index lookup
(receipt scope string); it measures the native algebra only.

## 2. Scaling [M]

Session C (socket 1, binary A `e168c1f0...`, glibc, first touch, same 9,000 natives in cost order): CPU per native
relative to K=1 (36.61 ms Apply). Foreign load on the run CPU sets 3-32%: g-k1 2.8%, g-k96 31.7% (repeat 8.6%),
s-ft 26.9%, p4 28.7% (repeat 8.2%), so the plan's 10% rule was not met for several runs; "quiet" in this note means
foreign load on the run CPU set, not on the socket (during g-k1 the other 7 cores of its CCX were 57% busy and
socket 1 48% busy). Repeats agree within <= 1.5% (s-ft 1.0%, g-k96-mi 1.5%, g-k96 0.03%, p4 0.07%).

| configuration | CPUs | CPU/native vs K=1 | IPC |
|---|---|---:|---:|
| K=8, one CCX (8 cores, one L3) | 128-135 | 1.018 | 3.28 |
| K=24, one node (3 CCX) | 128-151 | 1.463 | 2.29 |
| K=48, two nodes | 128-175 | 2.235 | 1.49 |
| K=96, three nodes | 128-223 | 3.754 / 3.753 (repeat) | 0.89 |
| K=96, mimalloc (LD_PRELOAD, malloc override) | 128-223 | 3.614 / 3.669 | 0.83 |
| K=96 over 4 nodes x 24, first touch / interleave | 4 x 24 | 3.861 / 3.824 ; 3.827 / 3.838 | 0.87 |
| 4 node-bound processes x 24 threads (p4) | 4 x 24 | 1.417 / 1.416 | 2.46 |

Only Apply natives degrade (Route 0.99x). Instructions per native are constant (2.843e8). The four-loop controls
degrade more at K=96 (FG 5.74x, BMW 4.62x, H 5.14x, X 7.34x), because their natives are shorter and create
polynomials at a higher rate.

**K=1 base and noise floor.** A second K=1 run (session D, 3.5% foreign load on CPU 128, CCX neighbours 63% busy)
measured 1.035x of the first (37.90 ms). This 3.5% spread is the noise floor of every gate ratio in this note; against
the mean of the two K=1 runs, g-k96 is 3.69x, p4 1.39x, g12 1.15x / 1.12x (no verdict changes). K=1 with mimalloc
(g-k1-mi, 0.963x of g-k1, 0.930x of g-k1-r2) ran at 21:39Z with its CCX neighbours 93% and socket 1 86% busy: the K=1
mimalloc effect is unresolved within the base noise. At K=96 mimalloc gives 0.963x / 0.978x (session C, n=2), i.e.
2-4%, and does not change the scaling. Session D also measured 12 processes x 8 threads, one per CCX with memory bound
to its node (`ccd12-fix`): 1.111x at 83% foreign load (`ccd12`, the first attempt, was mis-pinned by `taskset`
before `numactl --cpunodebind`). From 21:00 UTC two unpinned jobs of another user (~120 cores) and gammaboard
workers kept socket 1 75-99% busy; runs after 21:01 have 42-83% foreign load and are not timing results. The
binary-B ratios of session D (B-k8/k24/k96-stat 0.999 / 1.389 / 2.678) use B-k1-rec as base, a perf-record run
(1.096x g-k1, where instructions predict ~1.025x), so they are deflated by ~7% and are not timing results either;
against g-k1 they are 1.095 / 1.522 / 2.935.

Session E (22:22-23:02 UTC, binary C sha256 `518c3365...` = B + replicas, source commit 4dda8735; 63-86% foreign
load on every run CPU set, wall/CPU 1.16-1.71; `tables/session-E.md`, `session-E-Cbase.md`, `fills-E-raw.md`,
`identity-E.txt`). The first ratio column uses binary A's K=1 (one run); the second binary C's own K=1 (half the
sample, one run, 1.036x of binary A's); both carry the 3.5% base spread. RSS in GiB (GB).

| arm (K=96 on nodes 4-6 unless noted) | copies of owner programs + indexes | CPU/native vs session-C K=1 | vs C's own K=1 | cycles per instruction vs C-k1 [E] | cross-CCX fills / native | raw far-DRAM fills / native | RSS after prepare |
|---|---|---:|---:|---:|---:|---:|---:|
| g1 (today) / repeat | 1 shared | 2.138 / 2.055 | 2.085 / 2.007 | 2.16 / 2.02 | 1.55e4 / 1.59e4 | 5.07e3 / 6.82e3 | 5.20 GiB (5.58 GB) |
| g3: one per node | 3 | 1.221 | 1.176 | 1.13 | 1.33e4 (mostly near) | 533 | 15.35 GiB (16.48 GB) |
| g3-h128: one per node, all first-touched on node 4 | 3 | 1.226 | 1.183 | 1.10 | 1.29e4 | 560 | 15.35 GiB |
| g12: one per CCX / repeat | 12 | 1.168 / 1.141 | 1.127 / 1.099 | 1.06 / 1.08 | 4.8e3 / 4.8e3 | 1.77e3 / 1.72e3 | 61.15 GiB (65.66 GB) |
| ccd12-C: 12 processes x 8 threads, one per CCX | 12 (1 per process) | 1.155 | 1.116 | 1.04 | 0.7 | 37 | 12 x 5.20 GiB |
| K=24 on node 4: g1 | 1 shared | 1.503 | 1.452 | 1.41 | 1.48e4 | 7 | 5.20 GiB |
| K=24 on node 4: g3 (one per CCX) | 3 | 1.109 | 1.068 | 1.03 | 2.8e3 | 10 | 15.35 GiB |
| C-k1 (K=1, half the sample) | 1 | 1.036 | 1.000 | 1.00 | 0.7 | 22 | 5.20 GiB |

"cycles per instruction vs C-k1" is an estimate (`tables/fills-E-raw.md`): native cycles = cycles x section user CPU /
total user CPU (assumes the same clock in prepare and natives; with 12 copies the prepare is ~50% of the user CPU, so
the g12/ccd12-C values are the least reliable), divided by the natives' instructions (prepare instructions
subtracted; C-k1 covers half the sample, whose natives are 2.7% heavier). The private arms ran at 2.74-2.89 G cycles
per user-CPU-second against 2.99 G for C-k1 and 3.00-3.08 G for g1, so part of their CPU-time excess under load is a
lower clock, not more cycles. It is not a quiet-socket value either. Raw far-DRAM fills include the prepares (<= ~2.4e3 per
native per prepare [E, 8.8e7 DRAM fills per prepare in the socket-0 references]); for g3 / g3-h128 the raw value is
already an upper bound. Per-native counts and native stats are identical to the K=1 base for all 9,000 natives in
every arm [M]. Preparing 12 copies in parallel takes 124-143 s (one copy: 87-100 s).

**Session-to-session variation at load [M].** Session D's B-k96-stat and session E's C-k96-g1 run the same code path
(binary C with REPLICAS=1 prepares on the main thread, as B does) with the same instructions (1.152e13 vs 1.153e13)
at similar foreign load (65.6% vs 67.4%), yet cycles are 9.51e12 vs 7.03e12, local-CCX fills 2.24e9 vs 0.96e9 and
CPU per native 2.935x vs 2.138x. Loaded K=96 ratios are not reproducible across sessions.

## 3. Root cause: shared written cache lines of the polynomial context and variable maps

**Counters [M, raw per native, `tables/fills-D-raw.md`, binary B, session D, loaded].**

| run | cross-CCX fills (near + far cache) | DRAM fills near + far, raw (prepare estimate) | foreign load |
|---|---:|---:|---:|
| K=8, one CCX | 0.15 | 1.05e4 (4.9e3) | 42% |
| 12 x 8 processes, one per CCX (binary A) | 1.0 | 2.29e4 (1.5e4) | 83% |
| K=24, one node | 1.50e4 (all near) | 1.08e4 (2.4e3) | 49% |
| K=96, three nodes | 1.85e4 (0.78e4 near, 1.08e4 far) | 3.2e4 (2.4e3) | 66% |

Prepare-only references (`prepare-only/A-ev7`, `B-ev7`: one prepare on socket-0 CPUs 64/65) give ~8.8e7 DRAM fills
and ~1.2e3 cross-CCX fills per prepare, so the cross-CCX column is prepare-free while the DRAM column carries the
prepare share given in parentheses [E]. From K=8 to K=24, demand DRAM fills per native rise from ~5.6e3
to ~8.3e3 with the prepare estimate removed (+49% [E]; the old "+3%, flat" compared raw values with the prepare spread
over 2 vs 4 passes), while 1.5e4 cross-CCX fills per native appear. The fill counts mix clean read-shared transfers
and contended transfers of written lines; they locate transfers, they do not measure cost.

**Cycle budget at quiet K=96 [`tables/budget-C.md`, `tools/research/harness/cycle_budget.py`].** Session C, g-k96
and its repeat vs the two K=1 runs, per native. Native cycles are cycles x section-user-CPU share (prepare <= 1% at
K=96; 24% at K=1) [E]. Session C recorded `ls_any_fills` (demand + prefetch) far-cache and DRAM fills; its near-cache
count was not recorded and is taken from session D's B-k96-stat (loaded) [E]. Latencies are bounds, not measured on
this host: 775 cycles = 250 ns at 3.1 GHz for an uncontended remote-node DRAM fill or cross-CCX transfer [E].

| item (per native) | count | cycles | share of the extra |
|---|---:|---:|---:|
| native cycles K=1 / K=96 | - | 8.92e7 / 3.310e8 (3.71x) | - |
| extra cycles | - | 2.42e8 | 100% |
| extra far-DRAM fills x 775 (x 600) cycles, upper bound | 2.95e4 | <= 2.28e7 (1.77e7) | <= 9.4% (7.3%) |
| cross-CCX fills: far cache [M] + near cache [E] x 775 cycles, uncontended | 1.17e4 + 0.78e4 | 1.51e7 | 6.2% |
| residual | - | >= 2.04e8 | >= 84% |

The residual is not explained by DRAM traffic (all DRAM fills together change by ~+5e3 per native with the prepare
estimate removed) nor by remote-DRAM latency (charging the full latency bound to every extra far fill covers <= 9%).
Carried by the cross-CCX transfers it needs ~1.1e4 cycles (~3.6 us) per transfer, ~15x an uncontended transfer, which
requires transfers to queue on a few contended lines [E]. (At quiet K=24 the uncontended bound covers 29% and the
residual 71%.) The same data read as a queue: ~70 of 96 threads stalled,
1.44e7 cross-CCX transfers per second, 4.9 us per transfer; a single line serviced every 100-200 ns carries 5-10
million transfers per second, so the measured rate corresponds to ~1.4-2.9 saturated lines [E]. The quiet K=24 run
gives ~2.7e3 cycles per transfer (~0.9 us, 3.4x uncontended; 24 threads, one node) and K=96 ~1.1e4 (4.2x more with
4x the threads), as a queue on a hot line would [E]. This is a consistency argument, not a measurement of per-line
cost.

**Where the transfers happen [M].** Sampling the cross-CCX fills themselves (`perf record -e
cpu/event=0x43,umask=0x14/u`, frame-pointer call chains, K=96, `sessions/D/B-k96-xccx`, 308,460 samples = 1.71e4 per
native, matching the counter) and annotating the hottest functions:

- `rustred::algebra::coefficient::validation::validate_polynomial_on_map`: 68.9% of its cross-CCX samples sit right
  after `mov 0x30(%rsi)` (the polynomial's `context: Arc<PolynomialContext>`), `mov 0x10(%rax)` (the `variables` Arc
  pointer at +0x10 of the context's ArcInner, the line of the context's strong/weak counts) and `mov 0x20(%rax)`
  (length of the `Vec<PolyVariable>` at +0x10..0x28 of the variables ArcInner, the line of the variable map's counts);
  another 12.7% sit after `mov 0x18(%rcx)` / `mov 0x18(%rax)`, where `%rcx` is a second `Vec<PolyVariable>` ArcInner
  reached through the map argument (`mov (%rdx),%rcx`, RustRed side) (`tables/annot-xccx-ef68f7.txt`).
- `MultivariatePolynomial::to_multivariate_polynomial_list` (Symbolica): 82.8% after the chain
  `context -> variables -> len`, i.e. `nvars()` (`tables/annot-xccx-tompl.txt`).
- The rest is spread over every function that creates, clones or drops polynomials (`drop_glue` 6.4%,
  `replace`, `execute_fixed_polynomial`, `specialize_fixed_indices_authenticated`, ...) and glibc `free`
  (17.8%: header reads and the fastbin `lock cmpxchg` in `_int_free_chunk`).
- The cycle growth from K=1 to K=96 is spread the same way (`tables/compare-B-k1-k24-k96-self.md`: top symbols
  `replace` 9.4%, `validate_polynomial_on_map` 9.4%, `execute_fixed_polynomial` 7.2%, `to_multivariate_polynomial_list`
  3.9%): consistent with a cost paid wherever polynomials are touched, not decisive on its own.
- Controls: harness bookkeeping outside `inspect` 0.19% of the fills, Symbolica global `State` / symbol table
  0.00%, GMP 0.06%, lock/futex symbols 0.00% (futex census: no blocking lock, RESULTS.md).

**Mechanism** [M source; E attribution of cycles]. In the vendored Symbolica 953e26e2 every `MultivariatePolynomial`
holds `context: Arc<PolynomialContext<F>>` (`vendor/symbolica/src/poly/polynomial.rs:810`) with `PolynomialContext {
ring, variables: Arc<Vec<PolyVariable>> }` (`:852-856`). `zero()`, `zero_with_capacity()`, `constant()` and the
derived `Clone` clone the context Arc (`:1009`, `:1016`, `:1023`, `:797`; `from_context` `:948`), every drop
decrements it, `set_variables` does `Arc::make_mut` on it (`:923`) and `unify_variables` re-points contexts
(`:1243`). The variable-map Arc is written when a new context is built from a cloned map (e.g. `new(...,
variables.clone())` `:706`, `get_vars()` `:1217`) and by RustRed's own clones of `Arc<Vec<PolyVariable>>`
(`IndexedCoefficientContext { variables, index_variables }`, `crates/rustred-core/src/algebra/indexed/context/model.rs:19-20`;
`CoefficientContext.variables`, `crates/rustred-core/src/algebra/coefficient/context.rs:25`). `nvars()`, `variables()`
and `ring()` read through both (`:905-913`, `:1127`). All polynomials of an owner program share one context and one
variable map, so every inspector thread performs locked read-modify-writes on the same few lines and reads data in
them. **Which line carries how much is unresolved**: the context-count line (Symbolica polynomial clone/drop) and/or
the variable-map-count line (Symbolica or RustRed `Arc<Vec<PolyVariable>>` clones, or false sharing with adjacent
allocations). The samples cannot separate them (skid, no IBS/c2c). An A/B with the RustRed hot-path clones of the
variable-map Arcs removed would isolate the RustRed share without touching Symbolica. Upstream Symbolica dev 445b882d
has the same design (same struct, `polynomial.rs:767` there).

Qualitatively this mechanism accounts for the measured pattern: 1.02x inside one CCX; 1.46x inside one node with no
remote memory (near-cache transfers); the same 1.42x for p4 (each process shares within its node); 3.75x across three
nodes; no effect of interleaving (placement is not the cause; interleaving only makes the read-shared program data
remote); ~1.01-1.16x for per-CCX processes; mimalloc 2-4%; constant instructions per native. The quantitative part is
the budget above.

**What it is not.** Not DRAM bandwidth; not remote-DRAM latency of the owner-program data (budget above; interleave
has no effect); not the allocator (mimalloc removes all arena sharing and gains 2-4%; allocator symbols carry 15% of the
K=1 -> K=96 cycle growth and grow 3.1x vs 4.0x for the rest, `tables/compare-B-k1-k24-k96-self.md`); not a lock (no
futex waits, no lock symbols); not the harness (0.19%); not Symbolica's global State (0%).

**glibc free.** glibc `free` holds 17.8% of the cross-CCX fill samples, but there is no arena sharing: K=8 runs,
pinned and unpinned, each have 10 arenas (main, test thread, one per worker; `malloc_info` at exit through an
LD_PRELOAD shim, `arena-check/{pinned,unpinned}.mallinfo.xml`) [M]. 21% of `cfree`'s samples sit at `__libc_free+7`
and most of the rest are chunk-header reads. The callers are drops of polynomials and coefficient systems
(`drop_glue<MultivariatePolynomial>` 16%, `guards::resolve` 38%, `applied::algebra::polynomial` 23% as first
non-allocator frame), which perform the locked decrement of the shared context just before calling `free`. So
these samples are most likely skid from that decrement, i.e. the same mechanism [E].

## 4. In-process replication (the fix prototype) and placement

The prototype is test-only and minimal (commit 4dda8735, `walking/reinspection.rs`). `RUSTRED_HARNESS_REPLICAS=G`
runs the harness's owner import (`prepare::prepare_with_fingerprints`, the walk's own) G times, each on a thread
pinned to the first CPU of its worker group, and builds the initial indexes G times. Worker k uses copy k*G/K.
`RUSTRED_HARNESS_REPLICA_HOME=cpu` prepares every copy on one CPU (placement control). The production analogue is
one `RoutedCandidateReducer` (plus the initial orthant/overlap indexes) per CCX group of inspector threads.

- Correctness [M]: digest-sink differential proof against the real walk's tap with private copies, FG with
  8 copies at K=8 98,869/98,869 identical and X with 4 copies 46,826/46,826 identical (`diff-C/`). Per-native
  counts and stats are identical to the K=1 base in every session-E arm (`tables/identity-E.txt`, PASS). Scope: two
  Apply-only four-loop families plus counts on the gen-7 sample; this is adequate for the test-only probe, not a
  proof for the production per-CCX reducer (W1.2 must run the full differential: FG/BMW/H/X via run_control.py and
  C-5F with digest sinks, plus the five-loop controls).
- Effect [M, loaded socket]: section 2, session E. Within one node (K=24), per-CCX copies take 1.45-1.50x down to
  1.07-1.11x: the within-node component the critique asked about, carried by near-cache transfers
  (1.48e4 -> 2.8e3 cross-CCX fills per native, DRAM fills unchanged).
- Placement [M, one loaded run; interpretation E]. Raw far-DRAM fills per native (prepare included, so upper
  bounds): g3 533, g3-h128 560, against g1 5.07e3 / 6.82e3 (>= ~2.6e3 with a generous prepare estimate removed).
  Private copies cut far-DRAM traffic by >= 5x even when two thirds of the readers' copies were first-touched on
  another node. Two readings remain open: the hot part of each copy is served from the readers' L3, or the kernel
  moved the remote pages. This host runs automatic NUMA balancing (`kernel.numa_balancing=1`; `numa_pages_migrated`
  2.09e10 since boot, ~2.3e5 pages in a 37-s test window [M, 2026-09-28]), and no numa_maps or vmstat deltas were
  recorded in session E. The placement statement is therefore limited to owner-program/reducer copies under the
  counting sink; it says nothing about the epoch inspector's snapshot layers and indexes, which are latency-bound
  lookup data that cannot be copied 12 times (critique 2.1: 89.4% of the M1 heap sat on node 4). Session G (queued)
  repeats g3 / g3-h128 with numa_maps sampling and adds g3 with all memory bound to node 4 (pages cannot migrate).
- What remains with per-CCX copies [M, `sessions/E/C-k96-g12-xccx`, 3.45e3 sampled cross-CCX fills per native]:
  35% are ahash `RandomState::new()` (`gen_hasher_seed` 17.7% + `from_keys` 17.5%: `counter.fetch_add` on a
  process-global `AtomicUsize`, ahash 0.8.12 `src/random_state.rs:161-164`, called from `RandomState::new`
  `:234-238`). 96.8% of those come from Symbolica `MultivariatePolynomial::to_multivariate_polynomial_list` (two
  `HashMap::new()` per call, `vendor/symbolica/src/poly/polynomial.rs:3518-3530`). The rest is spread thinly
  (sorting, `free`, drops). A replica cannot remove a process-global counter; separate processes do (0.7 fills per
  native). Under the session-E load the in-process per-CCX arm cost the same as the per-CCX process arm (1.14-1.17x
  vs 1.155x); that load also suppresses contention on a line all 96 threads share, so "cheap" is shown only under
  load [E]. If each residual transfer cost what a transfer costs in the quiet shared K=96 run (~1.1e4 cycles), g12
  would be ~1.6x; at the quiet K=24 cost (~2.7e3), ~1.15x [E bracket]; hence fix-round item 1.

## 5. Options, in cost order

In cost order (engineering cost first; factor at K=96; memory for ~96 inspector threads, GiB = 2^30 B, GB = 1e9 B):

1. **In-process per-CCX replication of the owner programs / reducer and the initial indexes** (recommended,
   provisional). Build one reducer per CCX group at startup, each by a thread pinned into that group, and hand each
   inspector its group's reducer. Measured 1.10-1.17x on a loaded socket [provisional], identical results. Cost:
   +5.09 GiB (5.46 GB) RSS per extra copy after prepare (12 copies 61.1 GiB = 65.7 GB, VmHWM 66.4-67.2 GB; ~93 GB [E]
   for the 17 CCX of a W136 inspector split) and a parallel prepare of 124-143 s. RSS was measured only after
   prepare in a 36,000-native run: growth of per-copy reducer caches over hours is not measured (first W1.2 pilot).
   No change to job/result plumbing, and no semantics change: "pinning" is performance-only per the v3 bump contract.
2. **In-process per-node replication.** 1.18-1.22x on the loaded socket, 15.4 GiB (16.5 GB) for 3 nodes.
   Within-node sharing remains (1.33e4 near-cache fills per native, the same as K=24 with one copy), so on a quiet
   socket expect about the p4 value, 1.42x [E, from p4 1.417 and K=24 1.463, both quiet], which fails the 1.3x gate.
3. **Node-bound inspector processes** (`--epoch-resolve merge`). 1.417x on a quiet socket [M, p4], the same
   within-node loss as option 2, plus byte-serialized jobs/results and cross-process merge. Per-CCX processes
   (1.11-1.16x [M, loaded]) are no better than option 1 under load and need 12+ processes; they remove the
   process-global residue (ahash) that option 1 keeps, which only the quiet session can price. Keep S2 job/result
   types byte-serializable anyway, as the critique suggests.
4. **Shared-memory snapshot segments.** Do not address the cause: the contended lines are reference counts written
   during algebra on in-memory Symbolica objects, not read-shared snapshot data, and Symbolica polynomials (with
   their Arcs) cannot live in a shared read-only segment. Not recommended.
- Placement only, for the owner programs (interleave or first touch without private copies): no effect [M: s-il
  3.83x vs s-ft 3.86x, quiet]. For owner-program data the v3 design's "shared read-mostly data uses `numactl
  --interleave` over nodes 0-5" (engine design 3.6) should give way to option 1. **Layer and index placement (first
  touch vs interleave vs per node) is NOT settled by this lane**: it stays an open W3.5 measurement that must include
  per-successor lookups and record numa_maps and /proc/vmstat numa_* deltas (automatic NUMA balancing is on).
- Allocator (mimalloc override): 2-4% at K=96 (n=2); unresolved at K=1 within the 3.5% base noise; orthogonal.
- Upstream Symbolica (not ours to write; report to the owner): polynomial temporaries clone and drop the shared
  `Arc<PolynomialContext>` (`polynomial.rs:797/810/852-856/948/1009/1016/1023/923/1243`) and read `nvars()` through
  it, and `to_multivariate_polynomial_list` seeds a fresh ahash `RandomState` per call. A context passed by reference,
  or interned without refcount traffic, plus a fixed-seed hasher would let a single shared copy scale. Upstream dev
  445b882d has the same design (`polynomial.rs:767`).

Design rule for the epoch inspector (S3-S5): no per-operation atomic read-modify-write on shared data in the
inspector hot path. An `Arc` snapshot clone per *job* is fine; `Arc` clones or refcounted handles per lookup,
per successor or per polynomial on shared structures reproduce this loss. Shared counters and statistics must be
per-thread or per-CCX and summed at merge.

## 6. Rescaling factor for W96 throughput projections

CPU per native relative to one thread on socket 1 (gen-7 pending mix: Apply-dominated; Route natives do not
degrade). Every value carries the 3.5% K=1 base spread.

| inspector configuration | factor | basis |
|---|---:|---|
| **projection bracket until the quiet confirmation** | **1.42 - 3.75** | 1.42 = best quiet-measured configuration (p4); 3.75 = shared copy, quiet |
| one shared copy (current code), K=96 over 3 nodes | 3.75 | [M] session C (8.6-31.7% foreign on the run CPUs); lower bound for the epoch inspector (critique THR-7), which adds per-successor image/digest work and lookups |
| one shared copy, K=24 in one node | 1.46 | [M] session C |
| per-node copies (or node-bound processes) | 1.42 | [M] p4, session C; per-node in-process 1.18-1.22 on the loaded socket |
| per-CCX copies in one process | 1.14-1.17 | [E, loaded, provisional] session E g12 / g12-r2 vs the session-C K=1 base (1.10-1.13 vs binary C's own K=1); gen-7 pending mix only |
| per-CCX processes | 1.11-1.16 | [E, loaded, provisional] ccd12-fix (D), ccd12-C (E) |
| extra factor for an SMT core whose sibling is busy (socket 0) | up to 1.54 | [M] one K=1 run on CPU 80, sibling 38.7% busy |
| mimalloc override | x 0.96-0.98 at K=96 | [M] n=2; K=1 unresolved |

W96 on socket 1 delivers 96 / 3.75 = 26 single-thread-equivalents of native work with the shared copy and 96 / 1.42
= 68 with per-node copies [E]; with per-CCX copies 96 / 1.17 = 82 is a provisional scenario, not a projection basis.
The direction of the quiet per-CCX value is unknown: load lowered the contended arm (2.06-2.14x vs 3.75x) and the
clock (which inflates the private arms), and it also suppresses the residual contention of the per-CCX arm (the
process-global ahash counter). The launch will share socket 1 (owner decision), but the shared-copy results of
sessions D and E (2.94x vs 2.14x at similar load) show that a loaded value is not reproducible enough to stand in for
launch conditions either.

**Workload mix.** The factor is for the gen-7 pending mix. The four-loop controls degrade more with a shared copy
(4.6-7.3x at K=96 [M]); their natives take 0.31-1.78 ms and call the per-call ahash seeding
(`to_multivariate_polynomial_list`, two `HashMap::new` per call) more often per unit of work, and the five-loop
controls invert the gen-7 cost mix (handoff 0.1 item 7). Per-CCX copies were measured on none of them (diff-C checked
FG and X identity only, without timing). Before any launch-(B) or P-IMP projection uses a per-CCX factor, g1 vs g12
at K=96 must be measured on the c4l-x and c4l-fg streams and on C-5F (session H, queued).

## 7. Other results of the lane

- K=1 base replicated: 36.61 / 37.90 ms (1.035x); mimalloc 35.26 ms under heavy neighbour load [M]. The socket-0 K=1
  run (CPU 80) is 1.543x socket 1: same instructions, 2.4x L2 misses and 2.5x DRAM fills; its SMT sibling (CPU 336)
  was 38.7% busy and socket 0 has SMT while socket 1 does not [M counters, E cause: L1/L2 sharing with the sibling].
  Any inspector placed on an SMT core with a busy sibling pays up to that factor on top of the scaling factor.
- Clock under load [M]: cycles per user-CPU-second 3.10-3.11 G in session C (every K) vs 2.74-3.08 G in sessions D/E.
- Binary B (frame pointers, line tables): sha256 `14c4e1c31cc9aa8d5d25e8bcb076428c327ec1a18c5461bd85653dd87f6daf55`;
  its gen-7 restore reproduces binary A's fixture exactly (sample, prefix, owner digests, request, strata).
- C (GMP/MPFR) share of heap calls: 2.27% of calls, 1.68% of bytes; a Rust `#[global_allocator]` misses them,
  mimalloc `override` covers them (RESULTS.md section 7).
- Native cancellation: p99 54 µs, max 1.2 ms from flag to return; heavy heads return within 0.63 ms (section 8).
- Heavy heads (top 90 by cost, K=8, frame pointers): allocator 17% of native samples, Symbolica polynomial code
  50% inclusive (GCD 22%), specialization 48% inclusive (section 9).
- Successor streams of gen7-pending-9k and the four C-4L families are written and verified (section 10).

## 8. Open issues

- Quiet-socket confirmation (fix-round item 9): `TMP/w0/harness/sessions/sessionF2.sh` (pid in `sessions/F2.pid`,
  log `sessions/F2.out`) waits (no lock held) up to 8 h from 2026-09-28 07:05Z for CPUs 128-223 < 10% busy, then runs
  sessions F2, G, H (<= 35 min each under `socket1.lock`), re-gating after the lock and before every run. Outputs
  `sessions/{F2,G,H}/`, `tables/session-{F2,G,H}*.md`, `fills-{F2,G,H}-raw.md`, `identity-{F2,G}.txt`. Cancel:
  `kill $(cat TMP/w0/harness/sessions/F2.pid)`. Write-up owner: to be assigned by the orchestrator. At launch
  nodes 4-6 were 60-70% busy (other users), so the window may pass without data. cpuset exclusivity is not available
  (owner decision: socket 1 stays shared).
- The five-loop epoch inspector adds per-successor work that the counting sink does not do. Its shared structures
  (snapshot layers, hints) must follow the design rule in section 5, or 3.75x stays a lower bound for it too; their
  placement is a W3.5 measurement.
- Workload mix: per-CCX copies on c4l-x, c4l-fg and C-5F (session H) before a per-CCX factor enters any projection.
- Production wiring of option 1 (one reducer per CCX group; the harness prototype is test-only) belongs to W1.2 /
  the epoch engine (S2/S3); it needs the full differential (FG/BMW/H/X via run_control.py, C-5F with digest sinks,
  five-loop controls), a replication term in the memory budget (5.46 GB per copy) and a replica-RSS growth check in
  the first W1.2 pilot.
- Root-cause split between the context-count line and the variable-map-count line: A/B with RustRed's hot-path
  clones of `Arc<Vec<PolyVariable>>` removed (no Symbolica change).
- Symbolica upstream request (context refcount traffic, per-call ahash seeding): owner decision; no CAS code was
  written here.
- Instruction attribution uses cycle and fill sampling with skid (no IBS/c2c: `perf_event_paranoid=2`, IBS needs
  system-wide mode).
- The branch `fable_5_1-v3-harness` has no upstream; pushing it is part of the orchestrator's merge-order step
  (critique Track A(1)).
- Session D ccd12 (the first per-CCX process arm) was mis-pinned (fixed in run_harness.sh: numactl before
  taskset); ccd12-fix and ccd12-C replace it.
