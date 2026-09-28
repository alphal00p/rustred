# Shared `Arc<PolynomialContext>` limits multi-thread scaling of polynomial work

**Summary.** Threads working on read-only `MultivariatePolynomial`s that share one context (the normal case for
polynomials created from one template) need more CPU time per operation the more threads run, although all do the
same work and nobody writes the polynomial data. Every clone, every polynomial derived from `self` (`zero()`,
`monomial()`, the result of `replace()`, each output of `to_multivariate_polynomial_list`) and every drop does a
locked read-modify-write on the context's reference count (`replace()` also on the variable map's count), and
`variables()`/`nvars()` load the `variables` pointer stored right after that count. On a CPU with many L3 domains
(AMD EPYC 9754: 8 cores per L3/CCX) these lines move between CCXs on nearly every operation; at 96 threads this costs
~2.6x more (full, split work) if the allocator put that pointer in the counts' line (3 of 4 offsets) than if not. This
reproducer (public API only) shows the effect, shows that it vanishes when each thread uses its own context on the
same data, and measures small patches.

## Environment and requirements

- Linux x86_64 (pinning via `sched_setaffinity`; compiled out elsewhere, untested), glibc 2.42 malloc (`--features
  mimalloc` switches Symbolica's allocator on), rustc 1.97.1, `--release` with line tables; deps `libc`, `ahash`.
- Host: 2x AMD EPYC 9754 (Zen 4c), per socket 16 CCXs of 8 cores (one L3 each) and 4 NUMA nodes; runs on socket 1
  (CPUs 128-255, no SMT siblings online), Linux 6.18, `numa_balancing=1`; see `results/*/*/host.txt`.
- Symbolica: crates.io `=3.0.0` (default in `Cargo.toml`: `default-features = false`, `integer-gmp`, `float-mpfr`,
  `tracing_max_level_info`), whose `src/poly/polynomial.rs` is byte-identical to upstream `dev` 445b882d. Patch
  measurements use a clone of `dev` 445b882d through `[patch.crates-io]` (commented in `Cargo.toml`).
- **License:** multi-threaded Symbolica needs an active license; set `SYMBOLICA_LICENSE`. Both programs check
  `LicenseManager::is_licensed()` first and refuse to run without one.

## The problem in a few lines (condensed from `examples/minimal.rs`)

```rust
let template = Poly::new(&Z, None, vars.clone());           // Poly = MultivariatePolynomial<IntegerRing, u16>
let shared: Vec<Poly> = /* 4096 polynomials from template.zero() + append_monomial: ONE context */;
std::thread::scope(|s| for t in 0..threads { s.spawn(move || { // e.g. 96 threads on 12 CCXs
    let set = if private { &copy_onto_new_context(&shared) } else { &shared }; // private: same data, own context
    for i in 0..ops {
        let p = &set[(i * 7919 + t * 104_729) % set.len()];
        acc += p.to_multivariate_polynomial_list(&[0, 1, 2, 3], true).len(); // +1 context clone per output
        acc += p.clone().replace(4 + i % 12, &Integer::from(2)).nterms();   // clone, new context, map clone
    }                                                       // every drop: -1 on the shared counts
}); });
```

## What the measured program does (`src/main.rs`)

4096 random polynomials over Z in 16 variables (8-64 terms) built from one template share one context and map.
K pinned threads run the same fixed sequence of operations on randomly chosen polynomials. `--work full` mirrors the
application's hot path: (1) validate the stored polynomial against the expected map (`variables() ==`, `nvars()`);
(2) `to_multivariate_polynomial_list(&[0,1,2,3], true)`; (3) `clone()`, `replace()` of 3 variables by integers,
`is_zero()`, validate. `specialize` = 1+3, `split` = 1+2. CPU = thread CPU time of the timed loop (after a 10%
warm-up). Variants (asserted per thread at set-up):

| variant | polynomial data | context / variable map the thread uses |
|---|---|---|
| `shared` | the one set | the shared context and map |
| `private-ctx` | own copy (`zero_with_capacity` + `append_monomial_back`) | fresh context, **shared** map |
| `rehome` (`rehome-api`) | the one set, read only | own context; each operation first copies the polynomial onto it (`-api`: `patch/proposed.diff`) |
| `private` | own copy | fresh context and fresh map |

**Layout control.** `--ctx-offset`/`--map-offset` (default 0 and 32) place the `ArcInner` (strong, weak, data) of the
shared context / map at that offset of its 64-byte line, by allocating candidates through the public API until one
lands there; each run reports both (checked with gdb). The context's `variables` pointer is at +16 (in the counts'
line unless at offset 48); the map `Vec`'s cap/ptr/len at +16/+24/+32 (len in the counts' line for offsets 0, 16).

## How to run

```sh
export SYMBOLICA_LICENSE=...                             # required
cargo build --release --examples                         # standalone package (empty [workspace])
taskset -c 0-95 target/release/examples/minimal 1 shared # then: 96 shared, 96 private
taskset -c 128 target/release/symbolica-arc-mre --threads 96 --variant shared --work full --cpus 128-223
PERF=$(command -v perf) scripts/run.sh                   # K = 1..96 x 4 variants x 3 works; perf of the timed loop
EXTRA="--ctx-offset 48" VARIANTS=shared scripts/run.sh   # the other layout; scripts/profile.sh: perf record
```

`FIRST_CPU` (default 128) must start a CCX; thread i runs on CPU FIRST_CPU+i (K=8: one CCX, K=96: twelve CCXs on
three nodes); the process starts on FIRST_CPU, so the shared data live on its node. Other CPUs: set `PERF_EVENTS`
(see `scripts/run.sh`). `run.sh` sets `glibc.malloc.arena_max` to the CPU count (glibc 2.35-2.38 derive the limit
from the pinned thread's affinity) [not tested]. A single-L3 machine shows the within-CCX part (K=8, table 3).

## Results

Socket 1, one process per run, 20,000 timed operations per thread, 3 repeats interleaved, **median** (min-max in the
`summary.md` files: within 10% for most rows, up to 23%); `perf stat` counts only the timed loop. Other users' jobs
kept the run CPUs 17-55% busy on average, depending on the part (per run in `runs.jsonl`); compare within a table.
Raw: `results/socket1-20260928T133455Z-fix/` (`sweep/`, `layout/`, `profile/`, `patch-ab/`, `map-offset/`, driver, log).

Table 1 (`sweep/`), `--work full`, default layout (c0/m32), ratio to K=1 of the same variant:

| variant | K=1 ns | K=8 | K=24 | K=48 | K=96 | IPC K=1 -> 96 | cross-CCX fills/op, K=96 |
|---|---:|---:|---:|---:|---:|---|---:|
| `shared` | 14,683 | 1.17 | 4.16 | 7.16 | **13.79** | 2.73 -> 0.20 | 25.3 |
| `private-ctx` | 14,648 | 1.00 | 1.04 | 1.13 | 1.50 | 2.73 -> 1.81 | 3.6 |
| `rehome` | 15,210 | 0.97 | 0.98 | 1.00 | 1.02 | 2.73 -> 2.76 | 0.9 |
| `private` | 14,740 | 0.98 | 0.99 | 1.00 | 1.02 | 2.70 -> 2.69 | 0.9 |

Table 2 (`sweep/`), ratio to K=1:

| work | `shared` K=8 | `shared` K=96 | `private-ctx` K=96 | `rehome` K=96 | `private` K=96 |
|---|---:|---:|---:|---:|---:|
| `specialize` (clone, `replace`, `is_zero`) | 0.99 | 3.55 | 2.39 | 1.00 | 1.02 |
| `split` (`to_multivariate_polynomial_list`) | 1.77 | 23.99 | 1.10 | 1.05 | 1.07 |

Table 3 (`layout/`), `shared` unless noted, ratio to K=1, by line offset of the context / map `ArcInner`:

| layout | full K=8 | full K=96 | split K=8 | split K=96 | specialize K=96 | `private-ctx` specialize K=96 |
|---|---:|---:|---:|---:|---:|---:|
| c0/m32: `variables` in the context counts' line (default) | 1.19 | 15.67 | 1.76 | 27.86 | 4.07 | 2.65 |
| c48/m32: `variables` on the next line | 1.01 | 5.92 | 1.06 | 10.88 | 2.69 | 2.71 |
| c48/m0: also map `len` in the map counts' line | 1.07 | 7.66 | 1.07 | 11.55 | 9.59 | 9.03 |

- Instructions per operation do not change with K (1.24e5 for `full`); cycles do. User-mode cycles per CPU second
  are 3.1 GHz in all rows except one `rehome` run (kernel time; hence medians). Checksums agree everywhere.
- `perf record`, K=96, default layout, `full` (`profile/`; skid puts samples on the instruction after the stalled
  one): 37% of cycles in `to_multivariate_polynomial_list`, 85% of those on `mov 0x20(%rax)` right after `mov
  0x10(%rax),%rax`, the per-term load of the `variables` pointer from the counts' line (`nvars()`); `replace`: 43% on
  the same pattern. At `--ctx-offset 48` the two functions take 20% and 5.1% of cycles instead of 37% and 10.6%.
  `realloc` takes 10.6%, 85% right after the `lock cmpxchg` of its per-thread arena lock [E: a serializing locked
  instruction waiting for earlier accesses to the shared lines].
- `private`/`split`: the remaining 0.8 cross-CCX fills per operation come from ahash's process-global counter: 0.0
  with `--ahash-source thread-local` (`patch-ab/`); fill samples drop from about 1,700 to 13 (`profile/`).
- Cost of `private`: 818 MiB RSS at K=96 vs 25 MiB for `shared` in this small example, 0.3-0.6 s set-up.
- Reading: shared read-only *data* are harmless (`rehome`); writing shared counts, or loading their lines, is not.

## Root cause

Lines of `src/poly/polynomial.rs`, byte-identical in crates.io 3.0.0, `dev` 445b882d and 953e26e2:
- `MultivariatePolynomial` holds `context: Arc<PolynomialContext<F>>` (`:767`), `PolynomialContext { ring: F,
  variables: Arc<Vec<PolyVariable>> }` (`:809-813`). Derived polynomials share it: derived `Clone` (`:754`),
  `zero()`/`zero_with_capacity()` via `from_context` (`:903-913`, `:965-975`), `constant()`/`one()`/`monomial()`
  (`:988`, `:999`, `:1016`); `unify_variables` merges equal contexts into one `Arc` (`:1200-1213`, e.g. from `Add`
  `:1731`). Each of these, and each drop, is a locked increment/decrement of the one strong count.
- `replace()` (`:2605-2636`) returns `from_coefficient_list(.., self.variables().clone(), ..)` (`:2632`, `:2340-2356`):
  a new context and an increment (later a decrement) of the map's count; `map_coeff` (`:2062-2087`) likewise, e.g.
  once per prime in modular GCD (`gcd.rs:5688`).
- `ring()`/`variables()` (`:862-870`) load from the context's `ArcInner`; `nvars()` (`:1083-1086`) loads the
  `variables` pointer (context +0x10), then the map `Vec`'s length. It runs per term in `exponents(i)` (`:1119-1125`,
  used by the term iterator `:7275-7291`), in `degree()` (`:2161-2173`) and in `append_monomial` (`:1450`).
- `to_multivariate_polynomial_list()` (`:3475-3525`) creates each output with `monomial(self, ..)` (`:3505`, `:3515`:
  one increment per output, one decrement per output drop) and its map with `HashMap::new()` (`:3481`, `:3487`,
  `ahash::HashMap`), i.e. `ahash::RandomState::new()`: a `fetch_add` on one process-global counter (ahash 0.8.12
  `src/random_state.rs:161-164`, called from `:234-238`).

Why read-only sharing becomes cache-line ping-pong: a locked add needs the line exclusively in the writer's core and
invalidates it in all other CCXs; their next load of the count, or of any field in that line, fetches it across the
fabric. With K threads doing this every few thousand instructions the line is almost always in transit: instructions
stay constant, cycles grow, IPC falls. Within one CCX the transfers stay in its L3 but are visible (`split`: 1.77x).

## Where this came from, and what we do meanwhile

RustRed (IBP reduction) runs this kind of work on one shared read-only set: 3.75x the CPU per ~36 ms job at 96
threads (IPC 3.43 -> 0.89; 81.6% / 82.8% of the cross-CCX fills in its validation / in `to_multivariate_polynomial_list`
right after the loads above). Its workaround, one private copy per CCX: 1.14-1.17x (provisional), but 5.09 GiB per
copy (56-61 GiB in all), minutes of set-up, code tied to the topology, copies must never meet (`unify_variables`).

## Proposed changes (ranked)

`patch-ab/`: pristine `dev` 445b882d vs the same tree plus `proposed.diff` (1a, 2, 4), `ablation-1a-2-without-4.diff`
(1a, 2; 2 is inert unless called) or `experiment-context-padding.diff` (3), interleaved, 3 repeats, medians. K=96 /
K=1, default layout unless noted; K=1 CPU of `full` is within 1.2% of pristine for every build:

| Symbolica | full | full c48 | specialize | split | `private` split | `rehome`(`-api`) full / specialize / split |
|---|---:|---:|---:|---:|---:|---|
| pristine | 14.15 | 5.24 | 3.75 | 24.92 | 1.06 (ahash-tl 1.03) | 1.03 / 1.00 / 1.04 (public API) |
| 1a + 2 | 2.39 | | | 1.61 (ahash-tl 1.50) | 1.08 (ahash-tl 1.03) | |
| `proposed.diff` | 2.31 | 1.78 | 3.64 | 1.48 | 1.05 | 1.01 / 1.00 / 0.97 (`rehome-api`) |
| padding | 4.88 | 5.05 | 2.53 | 9.78 | | |

1. **Do not write shared counts on hot paths** (largest effect).
   a. Functions creating several polynomials from `self` should put them on one per-call context, or borrow
   `&PolynomialContext` for temporaries and clone the `Arc` only for results that escape. `proposed.diff` does this
   in `to_multivariate_polynomial_list` only (one context per call; `nvars()` read once): split 24.9x -> 1.61x, full
   14.2x -> 2.39x with 1a alone [M]. Left [E]: the per-call context still clones the shared map (2 map-count writes
   per call, not 2 context-count writes per output), outputs' `append_monomial` reads `nvars()` through it, `replace()`
   is untouched (`specialize` 3.64x). Next: `replace`/`from_coefficient_list` (reuse `self`'s context), `map_coeff`,
   `zero*()` users, GCD/factorization internals with many temporaries [E].
   b. *Design direction, not implemented; open questions.* Contexts whose clone/drop writes no line shared by all
   threads (interned or per-thread canonical). Interning only the context leaves the map count shared (`replace`,
   `from_coefficient_list`, `map_coeff` use `variables().clone()`; callers clone the returned `&Arc`): expect
   `private-ctx`-like scaling (1.5x full, 2.4x specialize) unless maps are interned/per-thread too and those functions
   reuse `self`'s context [E]. A registry lookup per context creation is shared state too (`new`/`map_coeff` are hot);
   never-freed contexts grow with every ring (a finite field per prime); per-thread contexts need a thread-local
   lookup in `Clone`/`zero`/`monomial` and key-lifetime/ABA care; `unify_variables` would re-share merged contexts.
2. **An API for per-thread contexts** (additive; `proposed.diff`, with a test): `zero_with_new_context()` and
   `clone_with_context_of(&t)`: `rehome-api` 0.97-1.01x at K=96 (its `split` includes 4), +2.0% at K=1 vs `shared`
   (public-API `rehome` +3.7%), no copy of the set. Document with the fact that `unify_variables` merges contexts.
3. **Keep read-mostly fields off the count lines**: `experiment-context-padding.diff` pads `PolynomialContext`
   (`#[repr(C)]`, 112 bytes before the fields, ~128 bytes per context) and caches `nvars` in it: the context layout no
   longer matters (full 4.88x at c0, 5.05x at c48). With the map length in the map counts' line (m0, `map-offset/`)
   it helps too (specialize 11.0x -> 4.8x), but the map's `ArcInner` needs the same care. The RMW traffic remains.
4. **No process-global state per call**: a fixed-key or thread-local-seeded hasher for internal maps with computed
   keys (`proposed.diff`: `RandomState::with_seeds`; fixed keys matter only for untrusted keys). Applications can do
   it today with `ahash::random_state::set_random_source` before the first map (`--ahash-source thread-local`):
   `private` split 0.8 -> 0.0 cross-CCX fills per operation, 1.06x -> 1.03x (earlier session, cross-session: 1.22x
   -> 1.05x); `shared` split with 1a: 1.61x -> 1.50x (app-side) / 1.48x (patch).

## Measured vs estimated

Results and [M] are measurements on this shared host, [E] estimates; ratios depend on operation size, layout and
foreign load. The patches (against `dev` 445b882d; hashes in `patch-ab/*/host.txt`; the ablation diff is for
measurement only) are illustrations, measured only in scratch copies. Symbolica's `poly::` tests pass with
`proposed.diff` (317, 1 new) and with the padding (316) in an earlier session (`results/socket1-20260928T103516Z/`:
vendored 953e26e2 with an unrelated `heap_pow` change in `polynomial.rs`, layout not controlled, see `PROVENANCE.txt`).
