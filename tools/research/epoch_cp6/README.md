# CP6 checkpoint-only matched controls

This opt-in research adapter does **not** modify the historical Ready runner,
guards, accepted receipts, native runtime, or default steering. It separates
collection from acceptance. Native exit `4` and the unchanged runner's exit `1`
remain recorded; successful collection prints `COLLECTED_UNACCEPTED`.

`collect.py` checks the exact launch at the original `ArmGuard.launch` seam and
then calls that same method on that same guard with unchanged arguments. The
original guard owns start/session, stop handling, lock lifetime, sampler shutdown
and descendant drain. No second process supervisor or native subprocess exists
here. This adapter is for fresh, checkpoint-enabled Epoch, G2 off, explicit
`all-miss` or `snapshot`, no perf. A rejected launch keeps the original runner's
failure receipts. A receipt-write failure after drain cannot become acceptance.

## Plan and operation

Prepare one immutable JSON plan per arm; no live campaign is authorized by this
directory. Paths are absolute. The plan fields are:

| Fields | Meaning |
| --- | --- |
| `contract` | Exactly `rustred.epoch-cp6-control.v1`. |
| `mode`, `b` | `all-miss` or `snapshot`; positive lockstep B at most4096. |
| `run`, `checkpoint` | Fresh output directory and its `checkpoint` child, as produced by the historical runner's rewrite. |
| `queries`, `queries_sha256`, `queries_blake3` | Frozen actual query file with explicit required/auxiliary roles; SHA256 and BLAKE3 of its exact bytes. Obtain BLAKE3 using an existing trusted tool/library, not a new hash implementation. The adapter derives role counts from the file. |
| `binary`, `binary_sha256` | Frozen, already validated executable; no build/upgrade in this adapter. |
| `native_argv`, `native_cwd` | Complete expected **post-rewrite** argv and cwd. Include explicit mode, Epoch, checkpoint, queries, output, events, stop-file and all original mathematical/resource options. |
| `runner_argv` | Arguments to the unchanged `tools/research/w1_g2prod/run_arm.py`, without script name. Use `--command` with a frozen template containing explicit mode, `--policy epoch`, `--g2 off`, exact output/CPU/worker/lock/limit options. |
| `runner_sha256`, `historical_helper`, `historical_helper_sha256` | Pins for the original runner and its imported `TMP/fable51-controls/run_control.py`. It currently resolves the main `/common/dev/rustred` root; do not assume the adapter's worktree changes that cwd. |
| `launcher_argv` | Exact original `nice -n … nix develop /common/dev/rustred --command` plus `native_argv`; no command insertion. |
| `cpus`, `locks`, `minimum_start_bytes`, `minimum_run_bytes`, `time_limit`, `grace` | Registered original guard policy; nonempty affinity excludes protected CPUs128–227. `locks` is ordered, heavy lock first. Limits are seconds. |
| `resource_environment`, `lockstep_environment` | Expected nonsecret resource env entries at launch; optional lockstep override must match exactly, including absence. Never put credentials in the plan. |
| `python`, `verification_guard`, `verification_guard_sha256`, `verification_cwd` | Pinned Python path and existing `guard_build.py` identity/cwd used for untimed verification. |
| `verify_threads`, `verification_timeout` | Positive verifier thread count and bounded timeout in seconds. |
| `verification_resources` | `cold-verifier` and `python-audit`, each with exact `cpus` array and ordered `locks` array. The existing verification guard uses the registered start/run headroom. |

The template must omit `--g2-residual-anchors` (the original runner requires this).
Freeze `native_argv` after applying that runner's existing rewrite; it adds output,
events and stop-file paths and relocates the checkpoint. This adapter refuses any
different actual launch before calling the original launch method. Keep the
checkpoint directory exclusive through acceptance: no resume/adoption/writer.

After separate root allocation, the sequence is:

1. `python -B collect.py --plan PLAN.json --collect-cp6-checkpoint-only`.
2. `python -B gate.py freeze --plan PLAN.json`. This refuses pre-existing cold
   outputs/guard directories, writes `cp6-before-verification.json`, and prints
   exact verification command arrays. It does not run them.
3. Run those arrays, unchanged, through the **existing** registered
   `guard_build.py`, with evidence directories `RUN/cold-verifier` and
   `RUN/python-audit`, registered cwd/CPU/locks, and exclusive outputs. Native cold
   must use **`--no-result --require-closure --reinspect all --reference-levers off`**.
   Omitting `--result` is not enough: the CLI otherwise auto-selects sibling result.
   Expected guarded exits are0 for raw cold and1 for Python's honest summary
   `INCOMPLETE`. Any guard reason or other exit refuses acceptance.
4. `python -B gate.py accept --plan PLAN.json`. It emits `cp6-accepted.json` only
   after all predicates pass; failed/censored receipts remain evidence, not passes.

The gate reuses `assert_oracle_pass.gate`, then additionally requires full native
All reinspection; exact query bytes/count/required+auxiliary roles; complete
admission; request/owner bindings; matching generation; zero errors/frontiers/
pending/unadmitted work; every domain oracle-closed; unchanged summary and command.
Python summary `INCOMPLETE` is merely the expected interface response, never
independent paired record proof. The accepted authority is `raw_cp6_cold_all`.
The actual116 required+67 auxiliary payload is required for that scope; synthetic
183-row mechanics or deduplicated root counts do not establish it.

Before and after cold verification, all checkpoint entry names and regular-file
metadata must match, including payload size/inode/mtime/ctime. Small latest/
previous/session/lock files are additionally SHA256-bound. Reads may change atime,
which is deliberately excluded. Summary manifest digest must match the envelope;
the ordinary native cold reader authenticates canonical manifest and payload
digests. This adapter does not rehash large checkpoint payloads or implement
BLAKE3/certification. This is an owned local receipt workflow, not protection
against a malicious process forging reports or racing the exclusive directory.

## Measurement and remaining gates

Timing stays the original launcher-through-owned-group-drain boundary, excluding
lock admission and recorder shutdown. Binary identity checks precede the original
launch clock. Cold verification/Python costs are separate. Never insert perf into
a timed arm: a later explicitly allocated profile is a separate diagnostic run.

Before real controls: exact-tip native runtime/outer/mode tests, a tiny real public
smoke (`--no-result` raw All PASS and summary `--result` INCOMPLETE), guarded
collection/drain, and cold no-write checks must pass. None was run to develop this
tool. Mock tests validate only predicates, file bindings and delegation.

Then preregister AllMiss/Snapshot ABBA on one binary, identical inputs, B, workers,
affinity, allowances and checkpoint interval; record host noise. Require exact
normalized raw mathematical state, records/edges/roles, not just accepted closure
or equal counts. Mode/request-binding, transport/session/timing and documented
lookup accounting exclusions must be explicit; no mathematical exclusions. A
skipped lookup counter is not measured benefit; a mismatch, failed gate or absent
repeatable whole-campaign improvement falsifies default enablement.

Both CP6 modes perform the same staged drain/save/output workload. A later Epoch
versus legacy Ready comparison must register their different finalization/result
work: legacy does work that CP6 deliberately omits and partly moves into untimed
cold verification. CP6 drain timing alone cannot establish deployment-equivalent
speed or completion reporting. Raw scoped closure PASS is not family closure,
production readiness, a restart instruction or completion of the full architecture.
