# W0.3 native re-inspection harness: in-process scaling, root cause, verdict (2026-09-27)

Lane `harness`, branch `fable_5_1-v3-harness`, worktree `.claude/worktrees/fable51-csr`.
Master plan item W0.3; data and full tables: `TMP/w0/harness/RESULTS.md` and `TMP/w0/harness/tables/`.
Labels: **[M]** measured (run directory under `TMP/w0/harness/`, binary sha256), **[E]** estimate or inference.
No ETA and no closure claim.

## 0. Verdict

**One process, with a private copy of the owner programs / reducer per CCX (8 cores sharing one L3).**
The in-process loss is not NUMA placement and not the allocator. It comes from cache lines that every inspector
thread *writes*: the atomic reference counts of Symbolica's per-polynomial `Arc<PolynomialContext>` and of the
shared variable map, plus data that sits in those same lines. All owner-program polynomials share one context, and
every native creates and drops thousands of polynomials from it (section 3, instruction-level evidence).

- Today's design, one shared copy: CPU per native at K=96 is **3.75x** K=1 on a quiet socket [M, session C]. Gate 0.3
  (<= 1.3x) **FAILS**.
- The same process with 12 private copies, one per CCX (binary C, `RUSTRED_HARNESS_REPLICAS=12`): **1.14x / 1.17x**
  of the session-C K=1 base (1.10x / 1.13x of the same binary's own K=1). One shared copy in the same session gives
  2.06x / 2.14x, and 12 separate 8-thread processes, one per CCX, give 1.16x. Results are identical (differential
  proof on FG and X; per-native counts and stats on all 9,000 natives). With per-CCX replication the gate value is
  met on the loaded socket (1.17 < 1.3) [M]; formally the gate needs the quiet-socket confirmation (caveat).
- Placement control: private per-node copies all first-touched on node 4 (g3-h128) cost the same as copies
  first-touched on their own node (g3): 1.226x vs 1.221x, with identical fill counts. Where the data lives does not
  matter; who writes the lines does.
- Caveat: the whole of session E ran while other users' jobs kept socket 1 83-99% busy (62-86% foreign load on
  our K=24/K=96 CPU sets). Under the lane protocol these timings are void as A/Bs; the counter evidence is not.
  Foreign load biases the shared-copy arm *down* (fewer of our threads contend at once: 2.1x vs 3.75x quiet) and the
  private-copy arms *up* (cache pollution by time-shared foreign threads: the per-CCX process arm, which shares
  nothing, still shows 1.12-1.16x). The paired same-session comparison (g1 2.1x -> g12 1.14-1.17x = per-CCX
  processes) and the cross-CCX fill counts (1.55e4 -> 4.8e3 -> 0.7 per native) are consistent. A quiet-socket
  confirmation remains open (section 8).

**Rescaling factor** for W96 throughput projections (CPU per native relative to one thread on an idle socket-1 core):
**1.17** with per-CCX in-process replication (largest measured value); **3.75** for the current shared design
(a lower bound for the epoch inspector, which adds per-successor work); add up to **1.54x** for inspectors on
socket-0 SMT cores with a busy sibling. Details in section 6.

## 1. The question

Gate 0.3: CPU per native at 96 threads <= 1.3x the single-thread value, on the gen-7 pending sample
(9,000 stratified native-pending IDs of the v2 gen-7 checkpoint, `fixtures/gen7-pending-10k.json`), with the
walk's own `inspection::inspect` and a counting sink. The outcome decides whether the epoch engine runs ~90-136
inspector threads in one process or several inspector processes, and which factor every throughput projection
must carry.

The harness reproduces the walk's native streams exactly (differential proof, RESULTS.md section 2: FG 98,869,
BMW 147,233, H 24,680, X 46,826 and C-5F 980,945 natives identical; negative control detected) and the gen-7
restore cross-check passes (9,070 inspected natives: stats, accepted events and frontiers equal to the CP5
records, `crosscheck/crosscheck.json`) [M].

## 2. Scaling [M]

Session C (socket 1, binary A `e168c1f0...`, glibc, first touch, same 9,000 natives in cost order; foreign load on
the run CPUs 3-32%, repeats agree within 0.3%): CPU per native relative to K=1 (36.61 ms Apply):

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

Session D (20:55-21:50 UTC) added: a second K=1 base, 1.035x of the first (37.90 ms; so the base spread is 3.5%);
K=1 with mimalloc 0.963x (35.26 ms), which means mimalloc is a flat ~4% at K=1 and at K=96 (3.75x either way), not a
scaling fix; and 12 processes x 8 threads, one per CCX with memory bound to its node (`ccd12-fix`): 1.111x
(`ccd12`, the first attempt, was mis-pinned by `taskset` before `numactl --cpunodebind`: 1.047x CPU per native,
wall/CPU 3.73). From 21:00 UTC two unpinned jobs of another user (~120 cores) and gammaboard workers kept
socket 1 75-90% busy; runs after 21:01 have 42-83% foreign load on their CPU sets and wall/CPU up to 1.48, so
their CPU-per-native ratios are biased low (fewer of our threads run at once) and are not used as timing
results. Their counter data (per-native fills, instructions) remain usable.

Session E (22:22-23:02 UTC, binary C sha256 `518c3365...` = B + replicas, source commit 4dda8735; socket 1 83-99%
busy throughout, so every row carries the same heavy foreign load; `tables/session-E.md`, `fills-E.md`,
`identity-E.txt`):

| arm (K=96 on nodes 4-6 unless noted) | copies of owner programs + indexes | CPU/native vs session-C K=1 | vs C's own K=1 | cross-CCX fills / native | RSS after prepare |
|---|---|---:|---:|---:|---:|
| g1 (today) / repeat | 1 shared | 2.138 / 2.055 | 2.085 / 2.007 | 1.55e4 / 1.59e4 | 5.1 GB |
| g3: one per node | 3 | 1.221 | 1.176 | 1.33e4 (mostly near) | 15.3 GB |
| g3-h128: one per node, all first-touched on node 4 | 3 | 1.226 | 1.183 | 1.29e4 | 15.3 GB |
| g12: one per CCX / repeat | 12 | 1.168 / 1.141 | 1.127 / 1.099 | 4.8e3 / 4.8e3 | 61.1 GB |
| ccd12-C: 12 processes x 8 threads, one per CCX | 12 (1 per process) | 1.155 | 1.116 | 0.7 | 12 x 5.1 GB |
| K=24 on node 4: g1 | 1 shared | 1.503 | 1.452 | 1.48e4 | 5.1 GB |
| K=24 on node 4: g3 (one per CCX) | 3 | 1.109 | 1.068 | 2.8e3 | 15.3 GB |
| C-k1 (K=1, half the sample) | 1 | 1.036 | 1.000 | 0.7 | 5.1 GB |

Per-native counts and native stats are identical to the K=1 base for all 9,000 natives in every arm [M]. Preparing
12 copies in parallel takes 124-143 s (one copy: 87-100 s).

## 3. Root cause: shared written cache lines in Symbolica's polynomial context [M]

**Counters.** Demand-fill sources per native (Zen 4 `ls_dmnd_fills_from_sys.*`, user mode, binary B, session D;
`tables/fills-D.md`; near/far cache = a line supplied by another CCX's L3 in the same / another node):

| run | cross-CCX fills (near + far cache) | DRAM fills (near + far, raw) | cycles (prepare subtracted) |
|---|---:|---:|---:|
| K=8, one CCX | 0.1 | 1.05e4 | 8.3e7 |
| 12 x 8 processes, one per CCX (binary A) | 0.8 | 2.3e4 (incl. 12 prepares) | - |
| K=24, one node | 1.50e4 (all near) | 1.08e4 | 1.25e8 |
| K=96, three nodes | 1.85e4 (0.78e4 near, 1.08e4 far) | 3.2e4 (half remote) | 2.53e8 |

(Prepare, the owner import, contributes < 0.01 cross-CCX fills per native; its cycles and DRAM fills are
subtracted only for single-prepare runs, using a prepare-only run on the lane CPUs, `prepare-only/B-ev7`.)

From K=8 to K=24 the DRAM fills do not move (+3%) while 1.5e4 cross-CCX fills per native appear and cost 4.2e7
cycles: about 2,800 cycles per transfer. That is the cost of a contended line, not of bandwidth or of remote
memory (the node is the same). At K=96, 1.85e4 cross-CCX fills per native and 3x the DRAM fills go with 1.7e8
extra cycles. Inside one CCX (K=8, or 12 x 8 processes) there are no cross-CCX fills and no slowdown.

**Where the fills happen.** Sampling the cross-CCX fills themselves (`perf record -e cpu/event=0x43,umask=0x14/u`,
frame-pointer call chains, K=96, `sessions/D/B-k96-xccx`, 308,460 samples = 1.71e4 per native, matching the
counter) and annotating the hottest functions:

- `rustred::algebra::coefficient::validation::validate_polynomial_on_map`: 81.6% of its cross-CCX fills land
  right after `mov 0x30(%rsi)` (the polynomial's `context: Arc<PolynomialContext>`), `mov 0x10(%rax)` (the
  `variables` Arc stored at +0x10 of the context's ArcInner, i.e. in the line of its strong/weak counts) and
  `mov 0x20(%rax)` / `0x18(%rcx)` (length and pointer of the `Vec<PolyVariable>` at +0x10..0x28 of the variables
  ArcInner, again the line of its counts) (`tables/annot-xccx-ef68f7.txt`).
- `MultivariatePolynomial::to_multivariate_polynomial_list` (Symbolica): 82.8% after the same chain
  `context -> variables -> len`, i.e. `nvars()` (`tables/annot-xccx-tompl.txt`).
- The rest is spread over every function that creates, clones or drops polynomials (`drop_glue` 6.4%,
  `replace`, `execute_fixed_polynomial`, `specialize_fixed_indices_authenticated`, ...) and glibc `free`
  (17.8%: header reads and the fastbin `lock cmpxchg` in `_int_free_chunk`).
- Controls: harness bookkeeping outside `inspect` 0.19% of the fills, Symbolica global `State` / symbol table
  0.00%, GMP 0.06%, lock/futex symbols 0.00% (futex census: no blocking lock, RESULTS.md).

**Mechanism** [M source, E attribution of cycles]. In the vendored Symbolica 953e26e2 every
`MultivariatePolynomial` holds `context: Arc<PolynomialContext<F>>` (`vendor/symbolica/src/poly/polynomial.rs:810`)
with `PolynomialContext { ring, variables: Arc<Vec<PolyVariable>> }` (`:852-856`). `zero()`,
`zero_with_capacity()`, `constant()` and the derived `Clone` clone the context Arc (`:1009`, `:1016`, `:1023`,
`:797`; `from_context` `:948`),
every drop decrements it, `set_variables` does `Arc::make_mut` on it (`:923`) and `unify_variables` re-points
contexts (`:1243`). `nvars()`, `variables()` and `ring()` read through it (`:905-913`, `:1127`). All polynomials
of an owner program share one context and one variable map, and every native builds and drops thousands of
temporaries from them, so every inspector thread performs locked read-modify-writes on the same few cache lines
and reads data that sits in those lines. Inside one L3 this is cheap; across CCXs each access is a contended
cache-to-cache transfer (2,800 cycles in one node, more across nodes). RustRed adds its own shared maps of the
same kind (`IndexedCoefficientContext { variables, index_variables: Arc<Vec<PolyVariable>> }`,
`crates/rustred-core/src/algebra/indexed/context/model.rs:19-20`; `CoefficientContext.variables`,
`crates/rustred-core/src/algebra/coefficient/context.rs:25`) whose Arcs are compared and cloned per call.
Upstream Symbolica dev 445b882d has the same design (same struct, `polynomial.rs:767` there); it differs from the
vendored tree only by a C-API feature flag.

This one mechanism explains every measured fact: 1.02x inside one CCX; 1.46x inside one node with no remote
access at all (near-cache transfers); the same 1.42x for p4 (each process shares within its node); 3.75x across
three nodes; no effect of interleaving (placement is not the cause; interleaving only makes the read-shared
program data remote); ~1.05-1.11x for per-CCX processes; mimalloc only ~4%; constant instructions per native.

**What it is not.** Not DRAM bandwidth (DRAM fills per native flat from K=8 to K=24); not NUMA placement of the
read-shared data (interleave has no effect; the placement control of section 4); not the allocator (mimalloc
removes all arena sharing and gains 4%; allocator symbols carry 15% of the K=1 -> K=96 cycle growth and grow
3.1x vs 4.0x for the rest, `tables/compare-B-k1-k24-k96-self.md`); not a lock (no futex waits, no lock symbols);
not the harness (0.19%); not Symbolica's global State (0%).

**glibc free.** glibc `free` holds 17.8% of the cross-CCX fill samples, but there is no arena sharing: K=8 runs,
pinned and unpinned, each have 10 arenas (main, test thread, one per worker; `malloc_info` at exit through an
LD_PRELOAD shim, `arena-check/{pinned,unpinned}.mallinfo.xml`) [M]. 21% of `cfree`'s samples sit at `__libc_free+7`
and most of the rest are chunk-header reads. The callers are drops of polynomials and coefficient systems
(`drop_glue<MultivariatePolynomial>` 16%, `guards::resolve` 38%, `applied::algebra::polynomial` 23% as first
non-allocator frame), which perform the locked decrement of the shared context just before calling `free`. So
these samples are most likely skid from that decrement, i.e. the same mechanism [E].

## 4. In-process replication (the fix prototype) [M]

The prototype is test-only and minimal (commit 4dda8735, `walking/reinspection.rs`). `RUSTRED_HARNESS_REPLICAS=G`
runs the harness's owner import (`prepare::prepare_with_fingerprints`, the walk's own) G times, each on a thread
pinned to the first CPU of its worker group, and builds the initial indexes G times. Worker k uses copy k*G/K.
`RUSTRED_HARNESS_REPLICA_HOME=cpu` prepares every copy on one CPU (placement control). The production analogue is
one `RoutedCandidateReducer` (plus the initial orthant/overlap indexes) per CCX group of inspector threads.

- Correctness [M]: digest-sink differential proof against the real walk's tap with private copies, FG with
  8 copies at K=8 98,869/98,869 identical and X with 4 copies 46,826/46,826 identical (`diff-C/`). Per-native
  counts and stats are identical to the K=1 base in every session-E arm (`tables/identity-E.txt`, PASS).
- Effect [M, foreign-loaded socket]: section 2, session E. Within one node (K=24), per-CCX copies take the
  1.45-1.50x down to 1.07-1.11x, which is the within-node component the critique asked about. It is the same
  mechanism as across nodes, carried by near-cache transfers: 1.48e4 -> 2.8e3 cross-CCX fills per native, while
  DRAM fills do not change.
- What remains with per-CCX copies [M, `sessions/E/C-k96-g12-xccx`, 3.45e3 sampled cross-CCX fills per native]:
  35% are ahash `RandomState::new()` (`DefaultRandomSource::gen_hasher_seed`: `counter.fetch_add` on a process-global
  `AtomicUsize`, ahash 0.8.12 `src/random_state.rs:161-164`, called from `RandomState::new` `:234-238`). 96.8% of
  those come from Symbolica `MultivariatePolynomial::to_multivariate_polynomial_list` (two `HashMap::new()` per
  call, `vendor/symbolica/src/poly/polynomial.rs:3518-3530`). The rest is spread thinly (sorting, `free`, drops). A
  replica cannot remove a process-global counter; separate processes do (0.7 fills per native). But in the same
  session the per-CCX in-process arm costs the same as the per-CCX process arm (1.14-1.17x vs 1.16x), so the
  residue is cheap: spread over many lines, lightly contended.

## 5. Options, in cost order

In cost order (engineering cost first; measured factor at K=96; memory for ~96 inspector threads):

1. **In-process per-CCX replication of the owner programs / reducer and the initial indexes** (recommended).
   Build one reducer per CCX group at startup, each by a thread pinned into that group, and hand each inspector
   its group's reducer. Measured 1.10-1.17x, identical results. Cost: +5.1 GB RSS per extra copy (61 GB for 12
   copies; ~87 GB [E] for the 17 CCX of a W136 inspector split) and a parallel prepare of 124-143 s. No change to
   job/result plumbing, and no semantics change: "pinning" is performance-only per the v3 bump contract.
2. **In-process per-node replication.** 1.18-1.22x on the loaded socket, 15 GB for 3 nodes. Within-node sharing
   remains (1.33e4 near-cache fills per native, the same as K=24 with one copy), so on a quiet socket expect about
   the p4 value, 1.42x [E, from p4 1.417 and K=24 1.463, both quiet], which fails the 1.3x gate.
3. **Node-bound inspector processes** (`--epoch-resolve merge`). 1.417x on a quiet socket [M, p4], the same
   within-node loss as option 2, plus byte-serialized jobs/results and cross-process merge. Per-CCX processes
   (1.11-1.16x [M, loaded]) are no better than option 1 and need 12+ processes. Not recommended for the loss; keep
   S2 job/result types byte-serializable anyway, as the critique suggests.
4. **Shared-memory snapshot segments.** Do not address the cause: the contended lines are reference counts written
   during algebra on in-memory Symbolica objects, not read-shared snapshot data, and Symbolica polynomials (with
   their Arcs) cannot live in a shared read-only segment. Not recommended.
- Placement only (interleave, first touch per node without private copies): no effect [M: s-il 3.83x vs s-ft
  3.86x; g3-h128 = g3]. The v3 design's "shared read-mostly data uses `numactl --interleave` over nodes 0-5"
  (engine design 3.6) should be replaced by option 1.
- Allocator (mimalloc override): a flat ~4% at every K [M]; orthogonal, keep N3 for that 4%.
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

CPU per native relative to one thread on an idle socket-1 core (Apply-dominated gen-7 pending mix; Route natives
do not degrade):

| inspector configuration | factor | basis |
|---|---:|---|
| one shared copy (current code), K=96 over 3 nodes | 3.75 | [M] session C, quiet socket; lower bound for the epoch inspector (critique THR-7), which adds per-successor image/digest work and lookups |
| one shared copy, K=24 in one node | 1.46 | [M] session C |
| per-node copies (or node-bound processes) | 1.42 | [M] p4 quiet; per-node in-process 1.18-1.22 on the loaded socket |
| **per-CCX copies in one process** | **1.17** | [M] largest of g12 / g12-r2 vs the session-C K=1 base, loaded socket; the per-CCX process arm gives the same |
| per-CCX processes | 1.16 | [M] loaded socket |
| extra factor for an SMT core whose sibling is busy (socket 0) | up to 1.54 | [M] one K=1 run on CPU 80, sibling 38.7% busy |
| mimalloc override | x 0.96 | [M] K=1 and K=96 |

W96 on socket 1 with per-CCX copies therefore delivers about 96 / 1.17 = 82 single-thread-equivalents of native
work [E]. With the current shared copy it delivers 96 / 3.75 = 26. The four-loop controls degrade more with a shared
copy (4.6-7.3x at K=96, [M]); per-CCX copies were not measured on them. Route natives (about 71% of five-loop natives by count, but
cheap: 0.04 ms each at K=1) show ~1.0 in every configuration, so the factor applies to Apply CPU. The 1.17 comes from a loaded socket; the launch will also share
socket 1 (owner decision), so it is the relevant condition. A quiet-socket value is expected to be lower, not
higher [E]: the per-CCX arms share nothing expensive, and the foreign load inflated even the fully private process
arm to 1.12-1.16x, against 1.02x for K=8 in one CCX on a quieter socket.

## 7. Other results of the lane

- K=1 base replicated: 36.61 / 37.90 ms (1.035x); mimalloc 35.26 ms [M]. The socket-0 K=1 run (CPU 80) is
  1.543x socket 1: same instructions, 2.4x L2 misses and 2.5x DRAM fills; its SMT sibling (CPU 336) was 38.7%
  busy and socket 0 has SMT while socket 1 does not [M counters, E cause: L1/L2 sharing with the sibling]. Any
  inspector placed on an SMT core with a busy sibling pays up to that factor on top of the scaling factor.
- Binary B (frame pointers, line tables): sha256 `14c4e1c31cc9aa8d5d25e8bcb076428c327ec1a18c5461bd85653dd87f6daf55`;
  its gen-7 restore reproduces binary A's fixture exactly (sample, prefix, owner digests, request, strata).
- C (GMP/MPFR) share of heap calls: 2.27% of calls, 1.68% of bytes; a Rust `#[global_allocator]` misses them,
  mimalloc `override` covers them (RESULTS.md section 7).
- Native cancellation: p99 54 µs, max 1.2 ms from flag to return; heavy heads return within 0.63 ms (section 8).
- Heavy heads (top 90 by cost, K=8, frame pointers): allocator 17% of native samples, Symbolica polynomial code
  50% inclusive (GCD 22%), specialization 48% inclusive (section 9).
- Successor streams of gen7-pending-9k and the four C-4L families are written and verified (section 10).

## 8. Open issues

- Quiet-socket confirmation of the replica arms (g1, g12, ccd12-C on the same natives with < 10% foreign load)
  was not possible tonight: from 21:00 UTC another user's two unpinned gammaloop jobs (~120 cores) plus
  gammaboard workers kept socket 1 83-99% busy. Session E's timings are protocol-void as A/Bs; the counters and the
  identity results are valid. Needs cpuset exclusivity (W0.10) or a quiet window; plan `sessions/planE.txt`, first
  three lines, ~9 min under the lock.
- The five-loop epoch inspector adds per-successor work that the counting sink does not do. Its shared structures
  (snapshot layers, hints) must follow the design rule in section 5, or 3.75x stays a lower bound for it too.
- Four-loop controls with per-CCX copies were not measured (with a shared copy they degrade 4.6-7.3x).
- Production wiring of option 1 (one reducer per CCX group; the harness prototype is test-only) belongs to the epoch
  engine (S2/S3); the memory budget needs a replication term (~5.1 GB per copy).
- Symbolica upstream request (context refcount traffic, per-call ahash seeding): owner decision; no CAS code was
  written here.
- Instruction attribution uses cycle and fill sampling with skid (no IBS/c2c: `perf_event_paranoid=2`, IBS needs
  system-wide mode). The load chains are unambiguous (both candidate loads read Arc-header lines), but the split
  between the context line and the variable-map line is not resolved.
- Session D ccd12 (the first per-CCX process arm) was mis-pinned (fixed in run_harness.sh: numactl before
  taskset); ccd12-fix and ccd12-C replace it.
