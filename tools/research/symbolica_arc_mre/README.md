# Shared `Arc<PolynomialContext>` limits multi-thread scaling of polynomial work

**Summary.** When many threads work on read-only `MultivariatePolynomial`s that share one context (the normal
situation for polynomials created from a common template), CPU time per operation grows with the number of
threads although every thread does the same work on data nobody writes. The cause is the context's atomic
reference count: every clone, every polynomial created from `self` (`zero()`, `constant()`, `monomial()`, ...)
and every drop performs a locked read-modify-write on the one cache line holding the count, and `nvars()` /
`variables()` read pointers in that same line (and in the line holding the variable map's count). On a CPU with
many L3 domains (AMD EPYC 9754: 8 cores per L3/CCX) these lines move between CCXs on nearly every operation.
This reproducer (Symbolica public API only) shows the effect and that it disappears when each thread uses its own
context, even while all threads keep reading the same polynomial data. Proposed changes are at the end.

## Environment

- Host: 2x AMD EPYC 9754 (Zen 4c), 128 cores per socket, 16 CCXs of 8 cores per socket (one L3 each), 4 NUMA
  nodes per socket; runs on socket 1 (CPUs 128-255, no SMT siblings online). Linux 6.18, glibc malloc.
  Full host description: `results/<run>/host.txt`.
- Symbolica: our vendored checkout at `953e26e2` (`symbolica-v3.0.0-24-g953e26e2`), which carries an unrelated
  uncommitted change to `heap_pow` (u32 exponent packing) that this program does not exercise; confirmation runs
  against pristine upstream `dev` `445b882d` (same `src/poly/polynomial.rs`; the two commits differ only in a
  C-API feature flag). Features: `default-features = false`,
  `integer-gmp`, `float-mpfr` (no mimalloc; `--features mimalloc` switches Symbolica's allocator on).
- rustc 1.97.1, `--release`, `debug = "line-tables-only"`.
- **License:** multi-threaded Symbolica needs an active license; set `SYMBOLICA_LICENSE`. The program checks
  `LicenseManager::is_licensed()` first and exits with an error if no license is active.

## What the program does (`src/main.rs`)

Builds 4096 random sparse polynomials over Z in 16 variables (4 "base" + 12 "index" variables, 8-64 terms,
148,523 terms in total), all created from one template and therefore sharing one context and one variable map
(checked at start-up through `variables()` addresses). K threads, each pinned to one CPU (`sched_setaffinity`),
then run the same fixed sequence of operations on randomly chosen polynomials of the set. One operation
(`--work full`) mirrors the hot path of the application where the problem was found:

1. validate the stored polynomial against the expected variable map (`variables() ==`, `nvars()`, layout);
2. `to_multivariate_polynomial_list(&[0,1,2,3], true)` on the stored polynomial;
3. `clone()`, then `replace(var, &Integer)` for 3 of the index variables (descending), `is_zero()`, validate;
4. drop everything.

`--work specialize` runs 1+3, `--work split` runs 1+2. CPU time is thread CPU time
(`CLOCK_THREAD_CPUTIME_ID`) of the timed loop, after a warm-up of 10% of the operations. Variants:

| variant | polynomial data | context / variable map used by the thread |
|---|---|---|
| `shared` | the one set | the one shared context and map |
| `private` | own copy per thread (rebuilt via `zero_with_capacity` + `append_monomial_back`) | fresh context, fresh map |
| `private-ctx` | own copy per thread | fresh context, **shared** map |
| `rehome` | the one set (read only) | each operation first copies the stored polynomial onto the thread's own fresh context (public API, same rebuild as `private`), then works on that copy |
| `rehome-api` | as `rehome`, using the API of `patch/proposed.diff` | (needs the patch and `--features proposed-api`) |

All variants produce identical checksums (verified per K in every run).

## How to run

```sh
export SYMBOLICA_LICENSE=...                      # required
cargo build --release                             # standalone package (empty [workspace])
./target/release/symbolica-arc-mre --threads 96 --variant shared --ops 40000 --cpus 128-223
scripts/run.sh                                    # sweep K = 1 8 24 48 96 x variants x works x 2 repeats
PERF=$(command -v perf) scripts/run.sh           # same, with perf stat per run (user-mode events)
PERF=$(command -v perf) VARIANT=shared WORK=full K=96 scripts/profile.sh   # perf record + annotate
```

`FIRST_CPU` (default 128) must be the first CPU of a CCX; the sweep pins thread i to CPU FIRST_CPU+i, so K=8 is
one CCX, K=24 three CCXs of one node, K=96 twelve CCXs on three nodes. To use upstream instead of the local
checkout, replace the three `path` entries in `Cargo.toml` by
`symbolica = { git = "https://github.com/symbolica-dev/symbolica", branch = "dev", default-features = false, features = [...] }`
and `[patch.crates-io] numerica = { git = ..., branch = "dev" }`, `graphica = { git = ..., branch = "dev" }`
(or drop the `[patch]` table and use a crates.io release, e.g. `symbolica = "=3.0.0"`).

## Results

Socket 1 of the host above, one process per run, 40,000 operations per thread, 2 repeats (they agree within 5%),
`perf stat` user-mode counters. Raw data: `results/socket1-20260928T103516Z/` (`*/runs.jsonl` one line per run,
`*/summary.md`, `*/host.txt`, `topology.txt`, per-run stdout/stderr/perf CSV in `per-run-files.tar.gz`).
CPU time per operation, ratio to K=1 of the same variant (K=1 in ns):

`--work full`, vendored 953e26e2 (`sweep/`):

| variant | K=1 ns | K=8 | K=24 | K=48 | K=96 | K=96 ns | IPC K=1 -> 96 | cross-CCX fills/op, K=96 |
|---|---:|---:|---:|---:|---:|---:|---|---:|
| `shared` | 14,310 | 1.21 | 4.21 | 8.22 | **16.29** | 233,074 | 2.77 -> 0.17 | 26.8 |
| `private-ctx` | 14,403 | 1.00 | 1.04 | 1.16 | 1.68 | 24,177 | 2.75 -> 1.71 | 3.6 |
| `rehome` | 14,846 | 0.99 | 1.00 | 1.02 | 1.08 | 15,962 | 2.79 -> 2.65 | 0.6 |
| `private` | 14,495 | 0.99 | 1.01 | 1.01 | 1.08 | 15,692 | 2.74 -> 2.60 | 0.7 |

K=96 / K=1 by kind of work:

| work | `shared` | `private-ctx` | `rehome` | `private` |
|---|---:|---:|---:|---:|
| `specialize` (clone, `replace`, `is_zero`) | 4.28 | 2.88 | 0.99 | 1.01 |
| `split` (`to_multivariate_polynomial_list`) | 29.97 | 1.21 | 1.19 | 1.22 |

- Instructions per operation do not change with K (1.36e5 for `full`); cycles do. Checksums are identical for all
  variants at every K.
- Pristine upstream `dev` 445b882d (`upstream-445b882d/`, `full`): `shared` 16.91x (14,238 -> 240,691 ns),
  `rehome` 1.08x, `private` 1.10x.
- `perf record` at K=96 (`profile/`): in `shared`/`full`, 38% of all cycles are in `to_multivariate_polynomial_list`,
  and 82% of those sit at one load chain, `mov 0x10(%rax)` (context -> `variables` pointer, in the line of the
  context's counts) then `mov 0x20(%rax)` (the `Vec` length, i.e. `nvars()`, in the line of the variable map's
  counts), executed per term (`*.annotate.*.txt`); 41% of `replace`'s cycles sit at the same load chain. With
  private copies, the cross-CCX fills left in `split` come from `ahash::RandomState::new` (`gen_hasher_seed` +
  `from_keys`: 37% of the far-cache fill samples).
- Cost of `private`: one copy per thread (818 MiB RSS at K=96 vs 25 MiB for `shared` in this small example).
- The host is shared: the run CPUs were on average 21% busy with other users' work in the second before/after each
  run (48% during `patched-proposed/`; per run in `runs.jsonl`); all threads stayed on their CPUs.

Reading: shared read-only *data* are harmless (`rehome` reads the very same polynomials as `shared` and scales like
`private`). What does not scale is writing shared counts: the context's (`shared` vs `private-ctx`), the variable
map's (`private-ctx` vs `private`, visible in `specialize` because `replace` clones the map), and ahash's
process-global counter (`split` with private copies, 1.22x).

## Root cause

`src/poly/polynomial.rs` is byte-identical in the vendored 953e26e2 and upstream `dev` 445b882d; lines apply to both.

- `MultivariatePolynomial` holds `context: Arc<PolynomialContext<F>>` (`:767`), with `PolynomialContext { ring: F,
  variables: Arc<Vec<PolyVariable>> }` (`:809-813`). Derived polynomials share it: derived `Clone` (`:754`),
  `zero()`/`zero_with_capacity()` via `from_context` (`:903-913`, `:965-975`), `constant()`/`one()`/`monomial()`
  (`:988`, `:999`, `:1016`); `unify_variables` even merges equal contexts into one `Arc` (`:1200-1213`). Each of
  these, and each drop, is a locked increment/decrement of the one context's strong count.
- `replace()` (`:2605-2636`) returns `from_coefficient_list(.., self.variables().clone(), ..)` (`:2632`,
  `:2340-2356`): a new context plus an increment (later a decrement) of the variable map's count, which all
  polynomials with that map share. `replace_last()` uses `zero_with_capacity()` (`:2647`).
- `ring()`/`variables()` (`:862-870`) read fields in the 64-byte line that starts with the context's counts
  (`ArcInner` = strong, weak, data); `nvars()` (`:1083-1086`) also reads the `Vec` length at offset 0x20 of the map's
  `ArcInner`, in the line of the map's counts. `nvars()` runs per term in `exponents(i)` (`:1119-1125`, used by the
  term iterator `:7275-7291`) and in `degree()` (`:2161-2173`).
- `to_multivariate_polynomial_list()` (`:3475-3525`) creates each output with `monomial(self, ..)` (`:3505`,
  `:3515`: one increment per output, one decrement per output drop) and its map with `HashMap::new()` (`:3481`,
  `:3487`; `ahash::HashMap`, `:9`), i.e. `ahash::RandomState::new()`, which does a `fetch_add` on one process-global
  counter (ahash 0.8.12 `src/random_state.rs:161-164`, called from `:234-238`).

Why read-only sharing turns into cache-line ping-pong: a locked add needs the line exclusively in the writer's core.
The L3 is per CCX (8 cores), so an increment by a thread on another CCX moves the line across the fabric (to a CCX of
the same node or of another node), and the next `nvars()`/`variables()` load on every other CCX misses on the line
that was just taken away. With K threads touching these lines every few thousand instructions, the line is almost
always in transit and threads queue for it: instructions stay constant, cycles grow, IPC falls (2.77 -> 0.17 here).
Within one CCX the line stays in the shared L3, hence the small effect at K=8.

## Where this came from, and what we do meanwhile

RustRed (IBP reduction) runs, per job, Symbolica polynomial work (validation against the variable map, fixed-index
specialization with `replace`, normalization, `to_multivariate_polynomial_list`) on one shared, read-only set of
polynomials. On this host, one process with 96 threads needed 3.75x the CPU time per job of one thread (1.02x at 8,
1.46x at 24, 2.24x at 48), IPC 3.43 -> 0.89, instructions per job constant; 81.6% of the cross-CCX fills in its
validation routine and 82.8% in `to_multivariate_polynomial_list` sat right after the loads described above.
The workaround we adopted is one private copy of the polynomial set per CCX, built by a thread of that CCX: 1.14-1.17x
at 96 threads (measured under heavy foreign load, provisional), but 5.09 GiB per copy (56-61 GiB for 11-12 copies),
about two minutes of set-up, code tied to the cache topology, and copies must never meet in arithmetic
(`unify_variables` would merge their contexts again). The ratios in this example are larger than RustRed's,
presumably because its operations take ~15 us instead of ~36 ms, i.e. far fewer instructions between two touches
of the shared lines [E].

## Proposed changes (ranked)

1. **Do not write shared counts on hot paths** (largest effect).
   a. Functions that create several polynomials from `self` should put them on one per-call context (or use
   `&PolynomialContext` for temporaries and clone the `Arc` only for results that escape). `patch/proposed.diff` does
   this in `to_multivariate_polynomial_list` (which then also reads `nvars()` once, not per term): `shared` `split`
   29.97x -> 1.66x and `shared` `full` 16.29x -> 2.74x at K=96 [M, `patched-proposed/`; unpatched values from
   `sweep/`]; K=1 unchanged within 1% (14,449 vs 14,310 ns for `full`). Other candidates: `replace` (cloned map, new
   context per call), users of `zero*()`, and GCD/factorization internals, which create many temporaries [E].
   b. The permanent fix is a context handle whose clone and drop do not perform an atomic read-modify-write on a line
   shared by all threads: e.g. interned contexts (a global registry of (ring, variable map); the polynomial stores a
   `&'static` reference or an index; contexts are never freed, bounded by the number of distinct maps), or a
   per-thread canonical context (a thread-local map from a shared context to an equal, thread-owned one, used by
   `Clone`, `zero()`, `monomial()`, ...). Expected [E]: `shared` then behaves like `rehome` (1.08x at K=96 on the
   same data), with no user action.
2. **An API for per-thread contexts** (small, no behavior change): `zero_with_new_context()` and
   `clone_with_context_of(&template)` (in `patch/proposed.diff`). A thread then clones read-only shared polynomials
   onto its own context: `rehome-api` is 0.99-1.01x at K=96 for `full`, `specialize` and `split` [M], costs +2.4% at
   K=1 vs `shared` (the public-API rebuild `rehome` +4.4%), and needs no copy of the whole set. Worth documenting
   together with the fact that `unify_variables` merges equal contexts.
3. **Keep read-mostly fields off the count lines**: `patch/experiment-context-padding.diff` only pads
   `PolynomialContext` (`#[repr(C)]`, 112 bytes before the fields) and caches `nvars` in it. Measured [M,
   `experiment-padded/`]: `shared` `full` 16.29x -> 5.97x, `specialize` 4.28x -> 2.96x, `split` 29.97x -> 10.49x.
   Cheap (~128 bytes per context, no visible cost at K=1) and complementary to 1; what remains is the
   read-modify-write traffic itself.
4. **No process-global state per call**: use a fixed-key (or thread-local-seeded) hasher for internal maps with
   computed keys; the patch uses `ahash::RandomState::with_seeds` in `to_multivariate_polynomial_list`: `private`
   `split` 1.22x -> 1.05x and `private` `full` 1.08x -> 1.01x at K=96 [M]. Fixed keys matter only for maps keyed by
   untrusted input.

The patches are against upstream `dev` 445b882d and were only built and measured in scratch copies; they are
illustrations, not reviewed changes; with each applied, Symbolica's `poly::` unit tests pass (316, plus one new test
for the proposed API; `patch-tests/`). **Measured vs estimated:** all numbers in Results and those marked
[M] are measurements on this host (2 repeats, shared machine); the effects of 1b and of 1a on other functions are
estimates [E]; absolute ratios depend on the size of the operations.

## Files

- `src/main.rs` the reproducer; `Cargo.toml`/`Cargo.lock` (standalone, offline-buildable here).
- `scripts/run.sh` sweep (CPU lists by CCX, optional `perf stat`, optional `flock` for shared hosts, foreign-load
  record per run); `scripts/summarize.sh` tables from `runs.jsonl`; `scripts/profile.sh` `perf record` + annotate.
- `patch/proposed.diff` (changes 1a, 2, 4) and `patch/experiment-context-padding.diff` (change 3), both against
  upstream `dev` 445b882d.
- `results/socket1-20260928T103516Z/`: `sweep/` (vendored build), `upstream-445b882d/`, `patched-proposed/` (final
  diff; `patched-proposed-v1/`: an earlier revision, same numbers within 3%), `experiment-padded/`, `profile/`
  (reports, annotations; no `perf.data`), `patch-tests/`, `session-driver*.sh` (the exact sessions), `topology.txt`.
