# W0 lane `symbolica`: Symbolica capability and upgrade audit

Date 2026-09-27. Worktree `.claude/worktrees/fable51-symbolica`, branch `fable_5_1-v3-symbolica`
(base b15316b9; tools committed at a64aa267 and 0dcd25b0 under `tools/research/symbolica_lane/`). Labels: [M] measured (run directory and binary sha256 given),
[src] read in source at the named revision, [E] estimate. No ETA or closure claim is made.

Revisions:
- vendored: `953e26e2` = v3.0.0 + 24 dev commits (all additive: `src/poly/reconstruction*`, `build.rs`,
  benches) + RustRed patch `patches/symbolica/heap-pow-wide-radix.patch` [src: `git diff --stat v3.0.0 953e26e2 -- src lib`].
- upstream `dev` = `445b882d` (fetched 2026-09-27 12:37 UTC) = vendored + 1 commit "Move C API behind a feature flag"
  (Cargo.toml `c_api = []`, `#[cfg(feature = "c_api")] pub mod cpp;`). No newer tag than v3.0.0 exists
  (`git ls-remote --tags`).
- upstream `main` = `70375b9e` (2026-09-26) is NEWER than dev and DIVERGENT: 39 commits on top of v3.0.0,
  without the 24 dev reconstruction commits (merge-base v3.0.0). It is the actual "latest Symbolica".
- no other upstream branch is newer than v3.0.0. `dev_poly` (GCD/factor series, 2026-08-28..09-01) and
  `smaller_poly` (DoubleInteger) are verified contained in v3.0.0 [src]; `skip-license-scopes`, `sub-evaluators`,
  `function_simplify` predate v3.0.0 and concern license scopes, evaluators and function simplification (not
  verified line by line; none touches the walk path).
- `devmain` = local trial merge of `upstream/main` into `445b882d` (scratch worktree of the submodule repo,
  commit `3272a7fc`; conflicts only in `Cargo.toml` comments and `build.rs` worktree handling, resolved to main's
  side; the dev-only `[[example]] reconstruction_joint_benchmark` block kept). Not pushed anywhere.

## 1. Capability audit per plan need

"vendored" = 953e26e2; "dev" = 445b882d; "main" = 70375b9e. All [src].

| Need (plan §3.9 etc.) | vendored | dev | main | Symbolica API (exact names) | Notes |
|---|---|---|---|---|---|
| N1: prime field arithmetic | yes | yes | yes | `domains::finite_field::{Zp64 = FiniteField<u64>, FiniteField<Mersenne64>}` (p = 2^61-1, `Mersenne64::SHIFT = 61`), `FiniteFieldCore`, `Field`/`Ring` ops (`mul`, `add`, `div`, `inv`, `pow`, `is_zero`) | numerica `finite_field.rs` unchanged v3.0.0 -> main |
| N1: integer -> residue | yes | yes | yes | `ToFiniteField<u64>`/`<Mersenne64>` for `Integer` (`c.to_finite_field(&field)`) | |
| N1: evaluate a multivariate integer polynomial at a point mod p | yes | yes | yes | `MultivariatePolynomial::evaluate_with_coeff_map(\|c\| c.to_finite_field(&f), &point, &f)`; or `map_coeff(...)` + `replace_all(&point)` | **RustRed already uses exactly this** in `crates/rustred-core/src/foundry/completion/frame/modular/sample.rs:412-441` (polynomial and coefficient numerator/denominator) |
| N1: evaluate a rational function mod p | yes | yes | yes | `RationalPolynomial::evaluate_with_coeff_map::<U: Field>`, `RationalPolynomial::to_finite_field(&field)` | `to_finite_field` re-normalizes (GCD); evaluation does not |
| N1: primality / primes | yes | yes | yes | `Integer::is_prime(k)` (RustRed `validate_prime` relies on deterministic MR below 2^64), `numerica::domains::integer::SMALL_PRIMES` (100), `finite_field::SMOOTH_PRIMES` (323) | no "random 63-bit prime" helper; `Mersenne64` avoids the need |
| N1: rational reconstruction / CRT | yes | yes | yes | `Rational::rational_reconstruction`, `Rational::maximal_quotient_reconstruction`, `Integer::chinese_remainder`, `MultivariatePolynomial::chinese_remainder` | not needed for a zero certificate |
| N1: black-box rational-function reconstruction (Cuyt-Lee / balanced Zippel over Zp64 and over Q) | yes | yes | **no** | `poly::reconstruction::{reconstruct_rational_function, reconstruct_rational_function_over_q, ReconstructionOptions, ReconstructionMethod}` | dev-only; RustRed `solver/discovery/semi_numerical*` depends on it, so a main-only upgrade breaks the build |
| N1: "fast zero test" | yes | yes | yes | structural `is_zero()` on canonical sparse polynomials; no probabilistic zero-test API exists | a nonzero residue at one point is a certificate of non-identical-vanishing; the decision rule is RustRed logic over the evaluation above, not CAS code |
| N2: allocation-free applied geometry | n/a | n/a | n/a | none | lattice boxes, sign cells and boundaries are RustRed domain logic; Symbolica has no lattice-geometry API |
| N2/N4: polynomial specialization (index values -> integers, keep base variables) | partial | partial | partial | `replace(n, &v)` (one variable, re-sorts), `replace_last(n, &v)` (row merge + power cache; valid when the variables after `n` are absent), `replace_except`, `replace_all` (all variables -> scalar), `evaluate_with_coeff_map`; normalization via `FromNumeratorAndDenominator::from_num_den(num, den, &Z, do_gcd)` | no multi-variable partial substitution that also projects onto a smaller variable map exists in any revision; RustRed's loop in `algebra/indexed/specialization.rs` (`execute_specialize_polynomial_raw`) is data movement around Symbolica `Integer` arithmetic and `append_monomial`, not an algorithm. A chain of `replace_last` from the last index variable would allocate one polynomial per index |
| N2/N4: allocation behaviour | yes | yes | yes | buffer reuse: `zero_with_capacity`, `clear()` (keeps capacity), `reserve`, `append_monomial`; thread-local scratch inside Symbolica (`DENSE_MUL_BUFFER`, `TOTAL_DEGREE_RANK_TABLE`) | no polynomial arena or pool API; each `MultivariatePolynomial` owns 2 Vecs plus an `Arc` variable list |
| N3: allocator | yes | yes | changed | feature `faster_alloc` = `mimalloc` as Rust `#[global_allocator]` (`src/lib.rs:194-197`), OFF in RustRed (`Cargo.toml` default-features = false) | does not override C `malloc` (GMP via `gmp-mpfr-sys`); `libmimalloc-sys` 0.1.49 feature `override` and `mimalloc` 0.1.52 are in the offline cargo cache. main switches the backend to `rustfs-mimalloc` 0.5.4 (not in the offline cache) and adds `faster_alloc_dynamic_tls` |
| GCD performance | yes | = | = | `MultivariatePolynomial::gcd` (via `PolynomialGCD`), `from_num_den(.., true)` normalization | the Aug 28-Sep 1 2026 GCD/factor optimisation series (branch `dev_poly`: Hu-Monagan planning, modular GCD reconstruction reuse, dense Montgomery, Kronecker products) is already in v3.0.0 and therefore vendored; `src/poly/gcd.rs`, `factor.rs`, `domains/rational_polynomial.rs`, `factorized_rational_polynomial.rs`, numerica `integer.rs`/`finite_field.rs`/`rational.rs` are unchanged v3.0.0 -> main and -> dev. **No GCD speed-up is available upstream** |
| Guard factorization | yes | = | = | `Factorize::factor()`, `square_free_factorization`, `gcd`, `to_multivariate_polynomial_list(&positions, true)`, `replace` | used in `algebra/indexed/base_coefficients.rs` (lines 244, 457, 550, 594); unchanged upstream |
| Coefficient zero tests (exact) | yes | = | = | `is_zero`, `is_constant`, the guard lane above | the walk reads only the zero/nonzero decision; an unnormalized numerator decides `Zero::Yes` exactly (N = 0 iff N/g = 0) but may weaken the `MissesDomain` (Uniform) decision, so `from_num_den(.., false)` is a semantics question for N1/N4, not a CAS gap |
| Global state / locks that could serialize 96 inspectors | see right | = | = | `LicenseManager::check()` and `is_licensed()` are a relaxed atomic load once licensed (vendored and main); global `RwLock<State>` WRITE is taken only by symbol creation, parsing, `State::get_or_insert_variable_list` (linear scan over all registered lists; reached when a `RationalPolynomial` becomes an atom coefficient) and `get_or_insert_finite_field` (finite-field atom coefficients), and state import/export; `poly/gcd.rs`, `factor.rs`, `polynomial.rs` use thread-locals only; the only other global caches are univariate real-root isolation (`poly/univariate/roots.rs`, `RwLock<HashMap>`) and algebraic root normalization | the walk's native path (`routed_campaign/walking`, `candidate_reduction/owners`) creates no atoms or symbols and calls no root isolation [src grep]; `is_licensed()` is called once per worker thread at spawn (`walking/parallel.rs:781`). Not measured at K = 96 (that is W0.3) |
| Other hand-written algebra in `rustred-core` replaceable by Symbolica | - | - | - | - | spot checks: determinants (`family/symanzik/operations.rs`), projective GCDs (`involutive/projective/polynomial.rs`), modular sampling, guard GCD/factor all already delegate to Symbolica; the remaining native-path hand code is the specialization loop (above) and `exact_linear_integer_root` (root of a linear factor) |

Conclusion of the audit [src]: every algebraic primitive N1 needs (prime fields incl. a Mersenne field,
coefficient reduction, point evaluation of polynomials and rational functions mod p, primality) is present in
the vendored copy and unchanged in upstream dev and main, and RustRed already uses the same calls in its
foundry modular sampler. Nothing in N1, N2, N4, guard factorization or zero tests requires new CAS code or a
Symbolica upgrade. N3's C-malloc override is a build-level choice (mimalloc `override` feature), not provided
by Symbolica's `faster_alloc`.

## 2. Upstream main: commits relevant to RustRed (70375b9e vs v3.0.0) [src]

- `7b31114c` Prevent overflow in heap_pow: same algorithm as RustRed's patch (exact `Integer` Horner encoding,
  `quot_rem` decoding, checked radix, final zero assertion) plus two regression tests (22-variable square,
  wide nonuniform exponents; RustRed's patch carries four). With main, the RustRed patch no longer applies
  (neither forward nor reverse) and becomes unnecessary.
- `a19c760d` Support exabyte long expressions + `06906976` u64 export: atom byte layout changed,
  `EXPORT_FORMAT_VERSION` 5 -> 6 and `SUPPORTED_IMPORT_VERSIONS = [6]` only. RustRed persists generated owner
  programs, catalogs and candidate bundles through `State::export_partial`/`State::import`
  (`crates/rustred-core/src/persistence/atoms.rs:197,300`). The four-loop owner program
  `TMP/four-loop-region-control.eazKG2/fg/owners/0000-0010011100.rrbin` carries the Symbolica magic
  `0x37871367` followed by format version `05 00` at byte 0x20 [M: xxd]. See section 4 for the measured effect.
- `4b1f44fd` mimalloc backend -> `rustfs-mimalloc` 0.5.4 (only with `faster_alloc`).
- `821b0245` deadlock fix in transcendental symbol initialization (start-up only; not on the walk path).
- `5d823859`, `c5be58d6` license: legacy keys accepted with a warning, expiry warnings; hot path unchanged.
- `4953a8a5`, `4b580455`, `b91f60b6`, `6cdca414`, `ec88f9e6`, `7f02f616`: atom-level speed-ups and
  normalization changes (polynomial -> atom ordering now `cmp_factors`, fewer expansions). They can change
  printed expressions and generated artifacts, not the walk's polynomial arithmetic.
- Everything else: floats, transcendental functions, evaluators/JIT, Python API, printing.

## 3. Trial upgrade to upstream dev 445b882d (+ heap-pow patch, still required) [M]

Binary: `TMP/w0/symbolica/bin/rustred-dev445-9f2f4c4d`
(sha256 `9f2f4c4dfae3fa90b42ce3f496900c4500f89b8a859f16d6b4c48bda7b16cffd`, 129,865,208 B, built in 483 s,
`cargo build --release --locked --offline -p rustred-app --bin rustred`, no source change in RustRed).
Reference: `TMP/fable51-controls/bin/rustred-4a17f9c7`
(sha256 `4a17f9c7c0447713370a1aed2e54c9a04251106a9183fd1b8a86dc5d1abb395e`, 130,174,160 B; same RustRed
sources 66ede259 = b15316b9 for `crates/`, vendored 953e26e2 + patch). The 309 KB size difference is consistent with
the C API module no longer being compiled [E].

API breaks: none (clean build with `--locked --offline`; Cargo.lock unchanged).

Suites (dev445, `--release --locked --offline`, license set; logs in
`.claude/worktrees/fable51-symbolica/TMP/symbolica-lane/build-dev445-test-*.log`, copies in `TMP/w0/symbolica/runs/`):
- `rustred-app --lib`: 777 passed, 0 failed, 6 ignored (baseline 777/0/6 at 66ede259).
- `rustred-app --test cli_routed_campaign`: 6 passed.
- `rustred --lib` (core, 32 test threads): first run 2838 passed, 1 failed, 32 ignored; the failure
  `persistence::catalog::tests::arbitrary_exact_expressions_roundtrip_with_deduplicated_values` compares two
  `encode_native` byte strings and the second one contained extra `native_compare_*` symbols registered
  concurrently by another test in the same process (global Symbolica symbol table). It passes alone (3/3),
  within `persistence::` at 32 threads (77/77), and two full reruns of the same test binary gave 2839/0/32.
  A test-isolation race, not attributable to the dev delta (which only gates `api::cpp`).

Controls (Ordered, strict `compare_walk_records.py`; runner `tools/research/symbolica_lane/ab_controls.py`,
interleaved A,B,B,A on the same CPUs, socket 1 under `socket1.lock`, 12:46-13:14 UTC; run tree
`TMP/w0/symbolica/runs/`):

| Control | Records | Natives (Apply+Route) | strict ref-r1 vs ref-r2 | vs dev-r1 | vs dev-r2 |
|---|---:|---:|---|---|---|
| C-4L FG (W6, CPUs 128-133) | 98,909 | 98,869 | PASS, 0 differing | PASS, 0 | PASS, 0 |
| C-4L BMW (W6, 134-139) | 158,951 | 147,233 | PASS | PASS | PASS |
| C-4L H (W6, 140-145) | 24,929 | 24,680 | PASS | PASS | PASS |
| C-4L X (W6, 146-151) | 47,193 | 46,826 | PASS | PASS | PASS |
| C-5F five-loop finite (W50, 128-177) | 1,273,376 | 967,621 (271,475 + 696,146) | PASS | PASS | PASS |

Performance (per repeat, ref first; dev relative to the ref mean). "inspector ms/native" = sum of
per-record native `seconds` / native-inspection records; "process CPU" = user+system of the process tree
(wait4 rusage). Summaries: `runs/c4l/native-seconds.json`, `runs/c5f/native-seconds.json`
(`tools/research/symbolica_lane/native_seconds.py`, `summarize_ab.py`).

| family | label | natives | inspector ms/native | process CPU ms/native | traversal s | whole command s | process CPU s | peak RSS GB |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| bmw | ref | 147119 | 0.466 / 0.4526 | 0.7096 / 0.6972 | 31.01 / 32.14 | 36.03 / 37.03 | 104.4 / 102.6 | 0.4619 / 0.4621 |
| bmw | dev | 147119 | 0.4792 / 0.449 (+1.0%) | 0.7214 / 0.6918 (+0.5%) | 31.95 / 33.1 (+3.0%) | 36.53 / 37.53 (+1.4%) | 106.1 / 101.8 (+0.5%) | 0.4594 / 0.4662 (+0.2%) |
| fg | ref | 98709 | 0.3142 / 0.3139 | 0.4367 / 0.437 | 11.95 / 11.96 | 15.52 / 15.54 | 43.11 / 43.14 | 0.2641 / 0.265 |
| fg | dev | 98709 | 0.314 / 0.3154 (+0.2%) | 0.4393 / 0.4361 (+0.2%) | 11.95 / 11.94 (-0.1%) | 15.51 / 15.52 (-0.1%) | 43.37 / 43.05 (+0.2%) | 0.2635 / 0.271 (+1.0%) |
| h | ref | 24458 | 1.416 / 1.499 | 1.671 / 1.755 | 12.23 / 12.89 | 16.02 / 16.54 | 40.88 / 42.93 | 0.5187 / 0.5151 |
| h | dev | 24458 | 1.47 / 1.547 (+3.5%) | 1.728 / 1.803 (+3.0%) | 12.67 / 13.29 (+3.3%) | 16.52 / 17.03 (+3.0%) | 42.26 / 44.1 (+3.0%) | 0.508 / 0.512 (-1.3%) |
| x | ref | 45691 | 1.951 / 1.938 | 2.106 / 2.089 | 33.56 / 33.36 | 39.54 / 39.04 | 96.22 / 95.45 | 0.936 / 0.9138 |
| x | dev | 45691 | 1.968 / 1.939 (+0.5%) | 2.118 / 2.087 (+0.2%) | 33.85 / 33.38 (+0.5%) | 39.53 / 39.04 (-0.0%) | 96.75 / 95.36 (+0.2%) | 0.9359 / 0.9384 (+1.3%) |
| five-finite | ref | 967487 | 0.7975 / 0.7881 | 1.993 / 1.987 | 262.3 / 275.7 | 367.3 / 380.3 | 1928 / 1922 | 6.559 / 6.562 |
| five-finite | dev | 967487 | 0.7889 / 0.7856 (-0.7%) | 1.97 / 1.993 (-0.4%) | 266.4 / 266 (-1.0%) | 372.8 / 376.8 (+0.3%) | 1906 / 1928 (-0.4%) | 6.547 / 6.541 (-0.3%) |

(`natives` in this table counts records of kind `native_inspection`; the strict tool's per-phase native counts above also include partial/initial kinds. Peak RSS is the run_control /proc poll.)

Foreign load on the run's CPU set (busy share from /proc/stat minus the run's own CPU share):
C-4L 1.1-5.5% except FG ref-r1 11.5% and H ref-r1 21.3%; C-5F 12.6-31.1% in all four runs. By plan §7 the
C-5F timings and the two C-4L runs above 10% are void as timing evidence; the identity results stand.
Using only the valid runs, every inspector ms/native difference is within the ref repeat spread
(C-4L |Δ| ≤ 1.0% for FG/BMW/X; H dev 1.47/1.55 vs valid ref-r2 1.50).

Reading [M]: identical results on all five controls; inspector CPU per native unchanged within noise
(C-5F -0.7%, C-4L +0.2..+3.5% with the H figure inside its own repeat spread). This is expected: the dev delta
does not touch any code RustRed executes.

## 4. Trial of the latest upstream (devmain = dev 445b882d + main 70375b9e)

Build: `cargo build --release --locked --offline -p rustred-app --bin rustred` with
`--config patch.crates-io.{symbolica,numerica,graphica}.path=<scratch>/sym-merge[/lib/...]` and a separate target
directory (vendor/symbolica untouched); RustRed heap-pow patch NOT applied (upstream `7b31114c` replaces it).
Result [M]: builds cleanly in 662 s with the unchanged `Cargo.lock` (`--locked --offline`), i.e. **no compile-level API
break for RustRed** against the latest upstream code (the persistence break below is a data-format break). Binary `TMP/w0/symbolica/bin/rustred-devmain-9ad50abc`
(sha256 `9ad50abc4498eac380703edbf616121162a7920c6d57a871770331c34821b86c`, 130,060,736 B).

Controls [M]: **cannot run on the existing inputs.** FG (W6, CPUs 288-293) exits 4 after 1.0 s and the five-loop
finite control exits 4 after 7.0 s, both with
`rustred: input: native Symbolica binary I/O: Unsupported export format version 5. Please export the expression
using strings in an older Symbolica and read the string into the newer Symbolica.`
(`TMP/w0/symbolica/runs-devmain/probe/{fg,five-finite}/devmain-r1/stderr`). Every owner program checked carries
format 5 (`67 13 87 37 05 00` at byte 0x20): four-loop FG/BMW/H/X region-control owners, the five-loop finite
control owners, and the v2 campaign's `inputs/owners/*.rrbin` (67 files, read only). The four-loop owners are
repacked (`TMP/four-loop-region-control.eazKG2/repack`) from `candidate-native-migrate` candidate bundles, so the
whole chain of native artifacts is format 5.

Core suite on devmain: `rustred --lib` (32 threads, `TMP/w0/symbolica/runs-devmain/core-suite.log`,
failure list `core-suite-failures.txt`) [M]: 2763 passed, **76 failed**, 32 ignored. All 76 failures are in native
persistence paths: `foundry::artifact::persistence*` 46, `foundry::artifact::source_port` 12, `persistence::{atoms,
family,compare,catalog}` 13, `foundry::artifact::{two_loop_tests,scope,install}` 4, and one
`reduction::terminal_normalization` test that writes a native witness. The log has 68 `invalid native atom frame`
errors, including freshly written in-process artifacts (e.g. the two-loop sunset durable artifact). Cause [src]:
v3.0.0 `Atom` export writes the byte `0` followed by a u64 length, while main writes `ATOM_EXPORT_FORMAT = 1`
(`src/atom/representation.rs:30,616,2121`) and a new atom layout. RustRed's
`crates/rustred-core/src/persistence/native.rs::preflight_native_frame` (line 46) hard-codes `frame[0] == 0` and a
9-byte header. RustRed's two `native_heap_pow` regression tests pass with upstream's heap_pow fix and no local
patch.

Consequence: adopting a main-based Symbolica needs three things:
- a RustRed change to the native frame preflight and to its framing tests (persistence code, not CAS);
- regenerating every native RustRed artifact, or converting it through an exact string round-trip with a
  format-5 reader (the route Symbolica's own error message prescribes; no CAS code): candidate bundles, catalogs,
  the four-loop control owners and the five-loop inputs;
- re-baselining all controls.

For the walk it buys nothing measurable: the polynomial, GCD, factorization, rational-function and
finite-field sources are identical to the vendored copy apart from `heap_pow` [src].

## 5. Recommendation for W1

**Do not upgrade for W1. Keep the vendored `953e26e2` + `heap-pow-wide-radix.patch`.**

- **dev `445b882d`: harmless, and it brings nothing.** Evidence [M]: 5 of 5 controls strict-identical (0 differing
  records); suites green (777/0/6, 6/6, core 2839/0/32 after one isolation-race flake); no API break; inspector CPU
  per native unchanged within noise. The only change gates the C API, which RustRed does not use. If another lane
  bumps it anyway, only the binary sha changes.
- **main `70375b9e` (and any future dev that absorbs it): defer until after the launch.** RustRed compiles against
  it unchanged, but export format 6 refuses every existing format-5 artifact, and RustRed's own native frame
  preflight rejects main's new atom framing (76 core-suite failures, all in persistence) [M]. Adopting it means regenerating
  or converting all native inputs and re-baselining every control, for no walk-side gain [src]. When it is
  adopted, `patches/symbolica/heap-pow-wide-radix.patch` can be dropped (upstream `7b31114c`). Note that
  `poly::reconstruction`, which `solver/discovery/semi_numerical*` needs, exists only on dev. Upgrading to main
  alone therefore breaks the build; only a dev that contains main is usable.
- **N1 (modular zero certificates): build it on the vendored API, as the foundry already does.** The calls are
  `Zp64` or `FiniteField<Mersenne64>`, `Integer::to_finite_field`,
  `MultivariatePolynomial::evaluate_with_coeff_map` and `RationalPolynomial::evaluate_with_coeff_map`
  (pattern in `foundry/completion/frame/modular/sample.rs:412-441`), and `Integer::is_prime`. No CAS code is
  needed. Only the decision rule (nonzero residue => not identically zero; zero residue => exact path) and the
  memo are RustRed logic.
- **N2 is geometry.** Symbolica has no counterpart, so it does not fall under the no-own-CAS rule.
- **N4 / specialization: stay on the Symbolica primitives already in use** (`Integer` arithmetic,
  `append_monomial`, `from_num_den`). No revision offers a multi-variable partial substitution with projection.
  Allocation can be cut with `clear()`/`reserve()` scratch reuse, which exists in vendored.
- **Whether to skip `from_num_den(.., true)` (the heuristic-GCD share) is an N1/N4 semantics question.** It is not
  a CAS gap: skipping keeps `Zero::Yes` exact but can turn `Uniform` into `Conditional`.
- **N3:** Symbolica's `faster_alloc` installs mimalloc as the Rust `#[global_allocator]` inside the symbolica
  crate, so RustRed could not add its own allocator on top. It does not cover GMP's C `malloc`. The offline-cached
  `mimalloc` 0.1.52 exposes `override`. That choice belongs to W0.8/W1.2. On main the backend becomes
  `rustfs-mimalloc` 0.5.4, which is not in the offline cache.
- **Global state:** no Symbolica lock on the native walk path [src]. W0.3 should still record futex wait at K = 96.
  Any new inspector-side code that turns a `RationalPolynomial` into an atom would take the global `State` write
  lock (`get_or_insert_variable_list`, linear scan); keep such code off inspectors.

## 5b. Open issues

- Test-isolation race in `persistence::catalog::tests::arbitrary_exact_expressions_roundtrip_with_deduplicated_values`
  (1 of 3 full core runs failed under 32 threads with dev445 [M]; the mechanism does not involve the dev delta [src]). `encode_native` exports
  every symbol registered since an offset, so concurrent tests leak symbols into the second encoding. Harden with
  the W1.4 flaky-test work (serialize the symbol-registering tests or compare decoded values).
- The C-5F timing A/B had 13-31% foreign load on CPUs 128-177 although `socket1.lock` was held: other runs with
  fewer than 24 threads may use socket 1. Future socket-1 timing A/Bs need cpuset exclusivity (plan W0.10) or a
  foreign-load gate.
- Not measured here: futex/lock wait at K = 96 (W0.3), allocator share (W0.3/W0.8).
- If a main-based Symbolica is adopted later: RustRed persistence frame preflight change, plus artifact
  regeneration or string-based conversion (section 4).

## 6. Reproduction

All scripts are in `tools/research/symbolica_lane/` on branch `fable_5_1-v3-symbolica`.

```sh
# fetch (in the worktree's submodule; the checkout is not changed by fetching)
git -C vendor/symbolica fetch --tags https://github.com/symbolica-dev/symbolica dev
git -C vendor/symbolica fetch https://github.com/symbolica-dev/symbolica '+refs/heads/*:refs/remotes/upstream/*'
# dev trial: checkout 445b882d; git carries the heap-pow working-tree patch (polynomial.rs untouched by the commit)
git -C vendor/symbolica checkout 445b882d
tools/research/symbolica_lane/build_locked.sh dev445-bin build --release --locked --offline -p rustred-app --bin rustred
# devmain trial tree and build (vendor/symbolica untouched)
nix develop --command bash tools/research/symbolica_lane/devmain_merge.sh <scratch>/sym-merge
tools/research/symbolica_lane/build_devmain.sh devmain-bin build --release --locked --offline -p rustred-app --bin rustred
# A/B controls on socket 1, then strict comparisons and native-time summaries
flock -w 14400 TMP/locks/socket1.lock nice -n 5 taskset -c 288-319 bash tools/research/symbolica_lane/socket1_ab_session.sh
tools/research/symbolica_lane/compare_all.sh TMP/w0/symbolica/runs/c4l dev fg bmw h x
nix develop --command python tools/research/symbolica_lane/native_seconds.py <runs>/*/*/result.json --json <out>
nix develop --command python tools/research/symbolica_lane/summarize_ab.py <out> --markdown
```

The worktree's submodule was restored to `953e26e2` + patch after the trial (matches the branch HEAD pointer).
