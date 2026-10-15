# Exact-union finite-trace diagnostic protocol

Status: the corrected complete-union diagnostic and independent source/runtime
audits passed. It used all 116 original rows, proved 35 empty, exhaustively
enumerated 81 nonempty regions, and admitted 2557 distinct union starts. Complete
terminal, zero and composite-rule-identity sets agree between all 81 separate
traces and one shared trace. The measured result supports cross-query trace
sharing on this exact cohort; no production execution extension is implemented.

One initial run stopped before tracing on an incorrect bookkeeping
assertion. The native enumeration succeeded for all 116 rows: 35 empty, 81 nonempty,
2557 starts, maximum 196. Native geometry equality produced 105 groups (24 empty),
while the saved walker had 92 containment-admitted root IDs (11 empty). The first
run, build and frozen executable remain under the isolated worktree's
`TMP/runtime`, `TMP/build`, and `TMP/frozen`; there is no trace/parity/speedup
result from that run. Independent review approved the narrow bookkeeping
correction before the corrected typecheck, build and single corrected pilot.

The isolated source is `TMP/finite-union-diagnostic-20261015`, based on
`f28666d58e534f1b55eb0af36713b17e47b278d8`. Only the ignored child test and its
test-module registration change executable source. There is no production API,
walker, checkpoint, publication, rule-generation or persistent-cache change.
The child module is compiled only in Linux CLI libtests; its process CPU clock
uses the existing optional CLI `libc` dependency.
The corrected test instantiates only storage 16, after checking physical arity 15
and `campaign_storage_arity(15)==16`. This is the same selected instantiation for
the same frozen input; production generic dispatch is unchanged.

## Completed measurements and provenance

| Quantity | 81 separate traces | One shared trace |
| --- | ---: | ---: |
| Native-call wall seconds | 51.705816205 | 13.077614667 |
| Native-call process CPU seconds | 51.213991323 | 12.966570792 |
| Whole-side wall seconds, including evidence absorption | 52.055519261 | 13.166850157 |
| Operational-state visits | 1,215,215 | 300,673 |
| Physical-key visits | 1,211,505 | 297,712 |
| Successful rule applications | 536,591 | 118,655 |
| Transport calls | 383,579 | 87,682 |
| Transport operation proxy | 53,320,124 | 18,623,068 |
| Transport endpoint proxy | 11,576,012 | 4,671,105 |

Every one of the 82 traces completed without a frontier, missing owner/rule,
queued/active/failed node or incomplete census. Both full set inventories contain
312 raw candidate terminal keys, 91,063 zero keys and 1,964 composite rule
identities. Each side's census multiplicities sum to its own successful native
application count; equality of multiplicities is not required.

Preparation took 122.347143145 s (214.456575918 process CPU seconds) and was paid
once, outside both side timings. Enumeration took 0.037754549 s. Native test time
was 192.326286336 s; guarded child time 194.479451118 s; complete driver time,
including binding checks, 201.595678124 s. Sampled peak process-tree RSS was
7,730,749,440 bytes. Exit was 0 with no resource stop, and all owned processes
drained. The independent reviewer compared all full sets and all 2557 flattened
input keys directly and rehashed 3435 source/vendor files and 77 immutable runtime
files, including the 69 ordered owner/overlay payloads.

The corrected typecheck took 59.227820355 s and the optimized app-libtest build
1468.380683170 s. The first successful build took 1636.348955630 s; its initial
runtime failed the incorrect layout-count assertion after 136.390283247 s guarded
time, before any trace. That failure is setup evidence, not negative algebra or
performance evidence. The earlier typecheck macro-call syntax error and the
pre-Cargo absent-vendor-lockfile binding error are also retained.

All evidence paths in this section are relative to the isolated worktree above.
The final frozen executable is `TMP/frozen-2/rustred-app-tests`, SHA256
`d8ae80b7dbecdad90929faa1e8b798de777d1501cd76cf98f496dcb4debb9bad`.
It is optimized at level 3, with `native,cli,capacity-dispatch`, and uses the
unchanged Symbolica/Graphica/Numerica vendor revision
`ef0db494533c87adb40356c996241680dc5a7bff` (3.0.0).

| Evidence | SHA256 |
| --- | --- |
| Tested `finite_replay/union_diagnostic.rs` | `783949cf0cb52d8fa3ca5985c6ee5a0cf77acca5104349c88aa5ef8d1a8f9c02` |
| Tested `finite_replay/tests.rs` | `e8783ceb6b37b1f687b91f75cf109e4fddf5988025ba7a8d8393d46f894c4c55` |
| `TMP/frozen-2/binding.json` | `266d9310db5b6e58f723867ad48e0a399474a25897c9d4b8e29d7cc8d7b650ae` |
| `TMP/build-2/binding-before.json` | `7c84f63b0763ee04caf9f6b2bb75e3e3a63fb8da71325c56a6404adf3d888ba4` |
| `TMP/runtime-2/native/receipt.json` | `5e6c384126b6e43ad92803d09854be12e338ff6790dbb4c0f8e5340265b40fed` |
| `TMP/runtime-2/native/enumeration.json` | `d035920f22f871adde52f57afc61a90f3fd6b6154deeabe7da5327c18c93e6c0` |
| `TMP/runtime-2/native/separate.json` | `3334e5b0224d6a1c86cb6b0360d93cc7840c6899a116dcf49d06cdf3b0c49847` |
| `TMP/runtime-2/native/shared.json` | `671a5a99a50646c90d7be35bf4c869015e6b6dc5fe83641a775c3f3724ebdd96` |
| `TMP/runtime-2/summary.json` | `3d150a9819b550915acabd9594f42aa58be3aa7176ba00d15af6bd915ccf34ff` |
| `TMP/runtime-2/independent-audit.json` | `d092c4d1acb104425f4f43a28010649f53cb8130d126458a5e9f9814e890108a` |

The source manifests bind the pre-results protocol text; this documentation was
subsequently updated with the audited results. Both tested Rust source hashes
above are unchanged. Evidence and binaries remain ignored and are not part of
the source delivery commit.

## Frozen inputs and build

Use `/shared/codex-2/rustred/TMP/finite-v3-full-r0-resource-correction-20261015/command.json`.
The test reads its adjacent `plan.json` and `cold.json`. It checks the command
against `plan.walk`, the native request/query digest against the successful cold
receipt, and every ordered imported payload digest against that receipt's
69-element native vector. The command's output/checkpoint paths are only parsed;
the test never calls the walker or opens those outputs.

Before building, freeze the worktree base/diff, all source hashes, lockfile,
Cargo/toolchain/dependency bindings, command/plan/cold receipt hashes, selection
and query hashes. Hash the 69 immutable payloads independently before and after
the run in addition to the test's native imported-byte binding. Record the
existing native source vector and immutable source paths; do not decode binary
checkpoint sections with a new reader.

Build an optimized app libtest with `--locked --offline -p rustred-app
--no-default-features --features native,cli,capacity-dispatch --lib --no-run
--message-format=json -j4`, in the isolated worktree. Use the existing local
toolchain and matching vendor override for all three Symbolica packages; record
the resolved Symbolica/Graphica/Numerica versions and commits from Cargo metadata.
Use the parent's existing build lease/resource supervisor. Do not mutate shared
vendor sources, the production checkout, or dependency lockfiles to make a
build pass. Any necessary dependency-binding correction needs source review.

Freeze the resulting executable under a new ignored release directory. Verify
its `--list` output contains exactly the named diagnostic, and record its hash.
Compilation has a separate bounded receipt and is excluded from runtime timings.
The corrected build has an authorized60-minute envelope, with3420 seconds before
cooperative stop,120 seconds of cleanup and60 seconds for final checks. Runtime
keeps its separate30-minute envelope below.

The exact corrected Cargo command, run in the isolated worktree under the
recorded build guard, was:

```bash
bash /shared/codex-2/rustred/TMP/intermediate-cache-regeneration-20261014/toolchain.sh \
  env CARGO_TARGET_DIR=/shared/codex-2/rustred/target CARGO_BUILD_JOBS=4 \
  cargo test --release --locked --offline \
  --config /shared/codex-2/rustred/TMP/finite-union-diagnostic-20261015/TMP/cargo-config.toml \
  -p rustred-app --no-default-features --features native,cli,capacity-dispatch \
  --message-format=json -j4 --lib --no-run
```

## Exact runtime invocation

The executable argv after freezing is:

```text
<frozen-app-libtest>
application::routed_campaign::walking::finite_replay::tests::union_diagnostic::complete_r0_union_matches_all_separate_native_traces
--exact --ignored --test-threads=1 --nocapture
```

For the one corrected attempt, use `TMP/build.py build --attempt 2 --start`
after the reviewed typecheck, then `TMP/run.py --attempt 2 --start`. Those
create `TMP/build-2`, `TMP/frozen-2`, and `TMP/runtime-2` without modifying the
initial failure's directories or its frozen binary.

Required environment:

```text
RUSTRED_RUN_UNION_DIAGNOSTIC=1
RUSTRED_TESTS_REQUIRE_LICENSE=1
RUSTRED_UNION_DIAGNOSTIC_COMMAND=/shared/codex-2/rustred/TMP/finite-v3-full-r0-resource-correction-20261015/command.json
RUSTRED_UNION_DIAGNOSTIC_OUTPUT=<new-absent-directory-under-isolated-worktree-TMP>
RAYON_NUM_THREADS=1
SYMBOLICA_HIDE_BANNER=1
```

The exact corrected native child argv was the following; its recorded output
directory already exists and must not be reused for another execution:

```bash
env RUSTRED_RUN_UNION_DIAGNOSTIC=1 RUSTRED_TESTS_REQUIRE_LICENSE=1 \
  RUSTRED_UNION_DIAGNOSTIC_COMMAND=/shared/codex-2/rustred/TMP/finite-v3-full-r0-resource-correction-20261015/command.json \
  RUSTRED_UNION_DIAGNOSTIC_OUTPUT=/shared/codex-2/rustred/TMP/finite-union-diagnostic-20261015/TMP/runtime-2/native \
  /shared/codex-2/rustred/TMP/finite-union-diagnostic-20261015/TMP/frozen-2/rustred-app-tests \
  application::routed_campaign::walking::finite_replay::tests::union_diagnostic::complete_r0_union_matches_all_separate_native_traces \
  --exact --ignored --test-threads=1 --nocapture
```

Forward the existing strict native Symbolica license configuration without
printing or serializing its secret. Supply an existing parent directory for the
fresh output. The test uses exclusive directory/file creation and refuses reuse.

Run through the existing `tools/research/runtime_order/generation_guard.py::run`
with `cpus="16-19"`, `workers=4`, `max_memory_bytes=32000000000`,
`host_memory_reserve_bytes=150000000000`, `ram_guard_margin_percent=5`,
`swap_growth_stop_bytes_per_second=0`, and `grace_seconds=120`. Lease CPUs16–19
before launch; never use production CPUs64–127. Start the outer stop timer at
1620 seconds and signal the supervisor, which signals/drains its owned process
group. Allow120 seconds for cleanup and reserve60 seconds for final hashes and
receipt serialization. The complete invocation, including loading, enumeration,
serialization and cleanup, must finish within1800 seconds. The test also checks its28-minute
deadline during preparation, between cases and through the existing native trace
observer; SIGTERM/SIGINT set the same cancellation flag. Native calls remain
cooperatively interruptible only at their existing boundaries.

The existing guard fixes all inner thread pools, including Rayon, to1. The test
records both the Rayon environment value and actual pool size. Four CPUs are an
outer preparation/control allowance. Each trace uses the
existing **one-worker inline kernel**, with the same prepared immutable reducer.
The fixed order is81 separate traces followed by one shared trace. It is a
single-order diagnostic, not a repeated order-balanced performance claim.

## Admission and limits

All116 original queries are parsed natively with IDs, roles and exact geometry.
Native summaries prove35 rows empty. There must be81 nonempty regions. Every
nonempty region is independently and exhaustively enumerated using the existing
helper, retaining its1024-seed,64-positive-layer and1MiB buffer allowance.
Expected summed starts are2557 and the largest region196. Distinct union starts
are measured by native `IntegralKey` set deduplication, never assumed.

Report native geometry-equality counts separately, without enforcing an observed
105/24 count. The saved92 root IDs include11 empty root records, but initial
admission uses `epoch::admit_initial_with` / `store.lookup` containment, not
equality. An empty D9 query can reuse a preceding nonempty D10 domain in the same
phase/owner bucket. The bound cold receipt has24 two-query groups:13 have a finite
nonempty root and11 have an empty root. These saved IDs do not determine this
diagnostic's enumeration or admission. All35 original empty rows still require
their independent native emptiness proof.

One `FiniteRootAdmission` retains the81 exact regions. No enclosing hull is
constructed. Every start passes its own and union policy; no descendant is
clipped to an input cap. The whole batch explicitly admits2557 input targets,
with a separate2MiB accounted input-buffer allowance.

Both arms have the same aggregate policy, twice the recorded complete-case sums:

| Resource | Diagnostic cap |
| --- | ---: |
| Operational nodes | 2,430,430 |
| Rule applications | 1,073,182 |
| Transport calls | 767,158 |
| Transport operation proxy | 106,640,248 |
| Transport endpoint proxy | 23,152,024 |
| Coalescing additions | 23,152,024 |

The coalescing allowance uses the endpoint sum with the same twofold headroom;
observed completed-case coalescing was zero. These are diagnostic aggregate
allowances, not new per-operation algebra limits. The existing matching reduction
policy and transport expansion policy are unchanged. Rule census storage is
bounded by1,073,182 identities and128MiB payload; retained terminal/zero key
payload by512MiB. Accounting bytes exclude allocator metadata and are not RSS.

## Completion and decision

Every trace must finish with all nodes complete, no active/queued/failed work,
no reserved coalescing additions, no missing owners/rules, no frontier, exact
input accounting and a complete census. Each census's multiplicity sum must
equal its successful native application count. Cancellation, a resource refusal,
or an incomplete trace fails the diagnostic; no successful prefix counts as
parity and no limit sweep follows automatically.

Compare the complete shared terminal and zero sets against the unions from all
separate traces. Compare exact owner/batch/baseline/selected rule-identity sets;
multiplicities may differ and are retained separately. Both sides must have the
same family and payload binding.

Artifacts: `enumeration.json` contains all row proofs, exact per-row keys and
union targets; `separate.json` and `shared.json` contain full exact sets, census
multiplicities, per-call wall/CPU timings and native counters; `receipt.json`
contains bindings, policy, separate preparation/enumeration/side timings and
parity verdict. Record external wall/CPU/RSS/host-memory telemetry and owned-group
termination. Recheck executable/source/input hashes after completion.

Only a successful complete pair can establish observed overlap/time savings for
this input. Native transport operation/endpoint counters are proxies, not CPU
work. Even a positive result does not establish coefficient back-substitution,
unrestricted closure or authority for a persistent memo/publication redesign.
