# LC2: Symbolica-patch-free reference build for the five-loop campaign (2026-09-29)

Owner direction: rerun the five-loop campaign on a RustRed that uses unmodified upstream Symbolica (Ben Ruijl's
rebased `dev`). Labels: **[M]** measured (log cited), **[E]** estimate or interpretation. Nothing here is an ETA,
a finishability claim or a closure claim. The LC1 campaign was not touched: its `inputs/` and `bin/steering.json`
were read, and nothing else in it was read or written. Raw evidence: `TMP/lc2/`, `TMP/lc2-smoke/`,
`TMP/inputs-v6/`, progress note `TMP/progress/lc2.md`.

## 0. Launch command (owner)

In Zellij session `rustred`, tab `fable_5_1`:

1. Pause LC1: focus its pane and press Ctrl-C **once**. Wait for `Durable checkpoint: ...` and the prompt (exit 4).
   LC1 stays resumable with its own frozen binary and inputs.
2. In a plain shell (no `taskset`; the launcher checks the shell's affinity against CPUs 128-227):

```sh
cd /common/dev/rustred && env TMPDIR=/common/dev/rustred/TMP nix develop --command python examples/python/production_saved_owner_campaign.py --campaign-directory campaigns/five-loop-qcd-feynman-d9d10-lc2 --start
```

Pause, monitor and resume work as for LC1 (`docs/research/fable51_lc1_launch_2026-09-28.md`, section 6), with
`lc2` in the paths. The full 600 GB RAM cap is admitted only when MemAvailable >= 650 GB at start, so pause LC1
first.

## 1. What changed (branch `fable_5_1-lc2`)

| commit | change |
|---|---|
| 6078492d | `vendor/symbolica` -> upstream `dev` `ef0db494` with **no local patch** (contains `939c4de8`, the multithreaded polynomial-context fix with public `zero_with_new_context`/`clone_with_context_of`, and `7b31114c`, the `heap_pow` overflow fix). `patches/symbolica/heap-pow-wide-radix.patch` is deleted; README and `tools/research/ops/provenance.py` no longer apply or check it. `Cargo.lock` unchanged. |
| d3732591, 1ee36253 | Thread-owned Symbolica contexts and context seals in the inspection hot path (cherry-picks of `c50d838f`, `bf6195c7` from the evaluation branch `fable_5_1-symeval`; applied cleanly). |
| 5e9f200d | Native frame preflight: the expected atom-format byte is derived from the linked Symbolica (1 since upstream `a19c760d`) instead of the constant 0. Format-5 artifacts (atom format 0, state export version 5) are still rejected, now with `native artifact uses Symbolica export format 5 ...; convert it with examples/python/convert_native_v5_to_v6.py`. Two unit tests. |
| b09a602a | `examples/python/convert_native_v5_to_v6.py` + test: rewrites only the state export version (5 -> 6) and the per-frame atom-format byte (0 -> 1); refuses other envelope versions, sections other than state/coefficients/family/program, already converted files, symbol user data or poly variables that could embed atoms, and non-Num or non-format-0 frames; writes a new directory with a per-file sha256 manifest (`conversion-manifest.json`). |

Walk semantics are unchanged: `walk-semantics-version` prints `{"walk_semantics_version":1,"checkpoint_format":"RUSTRED-WALK-CP5","checkpoint_schema":5}`.

## 2. Converted inputs [M]

Sources were read only. Log `TMP/lc2/logs/convert.log`.

| output | source | owners |
|---|---|---:|
| `TMP/inputs-v6/five-loop/inputs` | `campaigns/five-loop-qcd-feynman-d9d10-lc1/inputs` | 67 (1,280,854,595 B) |
| `TMP/inputs-v6/region-control/{fg,bmw,h,x}` | `TMP/four-loop-region-control.eazKG2/*` | 124 / 134 / 314 / 328 |
| `TMP/inputs-v6/four-all`, `four-all-p5` | `TMP/c4l-build.wBU9zC/staged-physics-r12anchors`, `TMP/c4l-s2/staged-p5-r12anchors` | 16 / 16 |
| `TMP/inputs-v6/c5f` | retired `five-loop-saved-coarse-cover/inputs` | 67 |

The converter's output is byte-identical to owners that the new Symbolica itself wrote when they were regenerated
from the textual sources: 900/900 region-control owners and 16/16 four-all owners. It is also byte-identical to the
validated evaluation prototype on the 67 C-5F owners.

## 3. Gates [M]

Launch binary: `TMP/fable51-controls/bin/rustred-lc2-fd9b9ac9`, sha256
`fd9b9ac96a50513d8119a43938313e8a729110d6a3815b0f949cec4acb1d9d60`, `[profile.campaign]` (fat LTO, one codegen unit,
no mimalloc). It was built at 5e9f200d in a clean worktree with a clean submodule. Crates, `Cargo.toml`,
`Cargo.lock` and the gitlink are identical at the branch tip. Logs are in `TMP/lc2/logs/`.

| gate | result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `rustred --lib` (release, `RUSTRED_TESTS_REQUIRE_LICENSE=1`) | 2844 passed / 0 failed / 32 ignored; includes both `native_heap_pow` regressions (pass without any local patch), the thread-owned tests and the 2 new preflight tests |
| `rustred-app --lib` | 822 / 0 / 12 |
| `rustred-app --test cli_routed_campaign` | 6 / 0 |
| Python `unittest discover -s examples/python` | 260 OK, 1 skipped (257 plus 3 converter tests) |
| format-5 input probe | the LC2 binary on the original FG owners exits 4 with the converter message |

**Strict Ordered identity.** The new binary ran on the v6 inputs; the reference was `rustred-4a17f9c7` on the
original v5 inputs. The reference runs were reused from `TMP/fable51-controls/k2-ref-fp` (Ordered output is
deterministic), which is the same reference as the LC1 gates. Command lines are the reference ones with only the
input directories substituted. `compare_walk_records.py --mode strict` found 0 differing records, both exit codes
were 0, and containment_checks were equal. Sources: `TMP/lc2/gates/gates.tsv` and `log`.

| control | records | containment_checks |
|---|---:|---:|
| FG W6 | 98,909 | 169,509,549 |
| BMW W6 | 158,951 | 653,022,941 |
| H W6 | 24,929 | 15,228,826 |
| X W6 | 47,193 | 20,507,017 |
| four-all W6 | 65,444 | 43,338,845 |
| four-all-p5 W6 | 68,483 | 48,046,064 |
| C-5F W16 | 1,273,376 | 5,307,741,824 |

**Oracle.** Each check ran `walk-verify-closure --require-closure --reinspect all`, then `assert_oracle_pass.py`,
then the paired audit. The verifier was the LC2 binary, because the older verifier cannot read format-6 owners. All
7 PASS: FG 248/248, BMW 268/268, H 628/628, X 656/656, four-all 32/32, four-all-p5 32/32, C-5F 1/1 roots
independently verified (`TMP/lc2/gates/oracle.tsv`).

**Instructions per native and IPC** (perf stat of the native, user mode, W6 unless noted). These are a single run
each (n = 1) under concurrent load; they are not a speed claim.

| control | FG | BMW | H | X | four-all | four-all-p5 | C-5F W16 |
|---|---:|---:|---:|---:|---:|---:|---:|
| instructions / native | 3.82M | 5.35M | 14.87M | 18.19M | 7.76M | 7.38M | 9.54M |
| IPC | 2.93 | 2.95 | 3.14 | 3.14 | 2.68 | 2.58 | 3.08 |

The only reference with perf data is four-all-p5 on 4a17f9c7: 7.54M instructions per native, IPC 2.57.

## 4. Prepared campaign (not started) [M]

`campaigns/five-loop-qcd-feynman-d9d10-lc2` was prepared with LC1's preparation command (launch note, section 4).
Only three things changed: `--prepare-from TMP/inputs-v6/five-loop-source`, the LC2 `--executable`, and the
campaign directory. The prepare-from source is the converted LC1 inputs (hard links), with the owner sha256 in its
input receipt updated to the format-6 owners. No existing campaign directory was modified.

- Steering `options` are equal to LC1's, and `command_arguments` are equal apart from the campaign path and the
  binary sha. The settings are W100 on CPUs 128-227, Ready, lookahead 256, frontier policy `stop`, 600 GB with a 5 %
  margin, a 50 GB host floor, the swap-growth stop and 14,400 s checkpoints.
- The selection (`d2667dc9...`), queries (`2c714860...`, 183 queries, order `preserve`), all three attachments,
  owner masks, owner bytes and owner paths are equal to LC1's. Every owner sha256 equals the converter's output for
  the corresponding LC1 owner. The sha256 of the ordered owner digest list is `c93b937d...`.
- Digests: `bin/steering.json` `f3330f58296bc4b70f0ed7b51d9ec60b0ccc3a54282775fc89ebbb4c62b78819`,
  `inputs/input-receipt.json` `f8478e40e864298f01e54fee2133c9f28d4d9eff3fbf5c5582df27fec6680df0`, frozen binary
  `bin/rustred-fd9b9ac9...`.
- Validation mode (`--json`) returned rc 0 with `launch_requested = false` and
  `hard_capped_by_available_memory = false` (`TMP/lc2/prepare/validate.json`).

**Functional smoke** (`TMP/lc2-smoke/`). This used the same preparation on a TMP copy, with W32 on CPUs 32-63 and
300 s checkpoints.

| | leg 1 (fresh `--start`) | leg 2 (`--resume --start`) |
|---|---|---|
| stop path | SIGINT to the supervisor (= Ctrl-C), 23:25:00Z | stop file `runs/<RUN>/stop-request.json`, 23:28:20Z |
| exit / state / stop reason | 4 / `paused` / `operator_signal_2` (4 s after the signal) | 4 / `paused` / `existing_operator_stop_file` (6 s) |
| checkpoints | gen 1 bootstrap, gen 2 end of preparation, **gen 3 periodic at 300 s (3.81 s)**, **gen 4 pause (0.44 s)** | **gen 5 after restore of gen 4 (0.34 s)**, **gen 6 pause (1.06 s)** |
| committed domains / natives at stop | 2,366,275 / 830,225 | 2,808,837 / 1,020,319 (continues from leg 1) |
| frontiers (max over every heartbeat) | **0** | **0** |
| RAM-guard stop / peak RSS | none / 8.2 GB | none / 8.0 GB |

Receipts: `TMP/lc2-smoke/campaign/runs/{20260928T231833.335961Z,20260928T232514.353576Z}`, logs
`TMP/lc2-smoke/{smoke,run1,run2}.log`, summaries `TMP/lc2-smoke/*.analysis.json`.

## 5. Frontier expectation [E]

LC2 changes neither the walk semantics nor any algebraic value: every Ordered control is record-for-record identical
to the reference, with the same containment_checks, and the five-loop owners carry the same algebra in a new header.
There is therefore no mechanism by which LC2 would run into a frontier more readily than LC1 or v2. v2 ran 18.8 h
on these inputs and LC1's rehearsal ran with 0 frontiers. Ready publication is schedule-dependent, so LC2's
trajectory will not repeat LC1's step by step, as between any two LC1 runs.
