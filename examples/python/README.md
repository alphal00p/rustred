# Python examples

## Saved-owner production campaign

The current prepared five-loop input snapshot lives in
`campaigns/five-loop-saved-coarse-cover/inputs`. It contains the saved owners,
the unchanged 67-root base query document and 67 ordinary auxiliary owner
regions for reuse. Python does no algebra or topology dispatch. The previous
`campaigns/five-loop-saved` run is paused and retains its own checkpoint; its
inputs must not be substituted when resuming that checkpoint.
The full campaign is an **explicit launch**, not an example smoke test. The
current snapshot is already running in Zellij session `rustred`, tab
`five_loop_vacuum`; do not start a second copy. For a separately staged new
campaign (replace `NEW_CAMPAIGN` with its directory):

```sh
mkdir -p TMP
export TMPDIR="$PWD/TMP" TMP="$PWD/TMP" TEMP="$PWD/TMP"
export CARGO_HOME="$PWD/TMP/cargo-home" CARGO_TARGET_DIR="$PWD/target"
nix develop --command cargo build --release --locked -p rustred-app --bin rustred -j 16
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/NEW_CAMPAIGN \
  --executable target/release/rustred --start
```

The launcher freezes exact executable bytes and the original steering policy on
first use. Omit `--start` to freeze/prepare and print the command without
launching. The production preset has no solve timeout or cumulative work stop;
it retains bounded worker buffers, input/scratch/algebra admission, at most
50 CPUs and a default 500 GB decimal requested RAM ceiling. Hourly native checkpoints are the
resume authority. The Python RAM guard requests checkpoint-and-stop at 95%
of the effective hard ceiling, earlier under host/cgroup pressure. Configure
`--max-memory-bytes` (any positive byte count, for example 700000000000) or
`--ram-guard-margin-percent` on first preparation or as per-invocation resume overrides.
No additional address-space cap is imposed. Sampled RSS cannot strictly prevent
between-sample overshoot; hard emergencies preserve the last completed save.

Ctrl-C requests a graceful saved pause and prints the exact restart command.
Wait for completion of that save. The production resume command automatically
reuses the frozen workers, CPU set and optional subdivision policy:

```sh
nix develop --command python examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/five-loop-saved-coarse-cover --resume --start
nix develop --command python examples/python/campaign_monitor.py \
  campaigns/five-loop-saved-coarse-cover/runs/RUN_NAME
```

For a later resume requesting 700 GB, add `--max-memory-bytes 700000000000`.
This does not modify a running process, the original frozen `steering.json`,
executable or native solver policy. Wait for the current process's saved pause
before resuming. Repeat the RAM override on each production resume, or use the
supervisor's emitted exact restart command; omitted values use the original
frozen RAM defaults. Host/cgroup protection can still admit less RAM, and the
95% checkpoint threshold uses that effective ceiling (665 GB only when the
full 700 GB is admitted). Plans and receipts distinguish these policies.

`active-run.json` identifies the latest run. Each run has atomic `status.json`,
PID/start/boot metadata, bounded-tail event monitoring and separate native
stdout/stderr. The colored TTY header/bar and plain redirected summaries show
measured CPU activity separately from worker reservations, finite initial-entry
publication separately from descendant work, and checkpoint writing/completion.
The entry bar is not a closure percentage; the closure ETA stays unknown.
Use `--once` or `--json` with the monitor for a read-only snapshot; `NO_COLOR`
disables color. Stale heartbeats/process identities are reported explicitly.

For another supplied snapshot, `stage_saved_owner_campaign.py --help` describes
the generic staging helper. Its default is byte-preserving. Optional
`--anchor-max-numerator-rank R` appends one ordinary owner-orthant query per
selected manifest owner, in manifest order, with no positive-power or D bound.
Add `--anchor-max-positive-power A` to bound anchor positive power; optionally
restrict that bound to `--anchor-positive-power-owners MASK1,MASK2` (otherwise it
applies to every anchor). These parameters are input data, with no loop-specific
defaults. All original query objects and their order remain unchanged, and
`queries-original.json` retains the exact supplied bytes. The receipt and
production plan record the additional obligations. Anchors are not certified
coverage, and escaping descendants remain ordinary required work. Native
admission remains authoritative, including for original diagnostic queries.
Choose a fresh staging/campaign destination: input planning never changes an
existing campaign or its checkpoint/resume policy.

The Nix flake also exports
`campaign`, `campaign-production`, `campaign-monitor` and `campaign-stage` apps.
See [the driver documentation](../../docs/shared_owner_campaign_driver.md) for
resource accounting, genuine native resume and low-level command examples.

## Other examples

[`one_loop_single_mass_vacuum.py`](one_loop_single_mass_vacuum.py) uses the
public `import rustred` API to generate, cold-inspect, and apply the complete
one-loop tadpole artifact:

```bash
maturin develop --release --features extension-module
python examples/python/one_loop_single_mass_vacuum.py
```

It asserts the single generated source and guarded rule, then reduces `I(3)`
to the tadpole master `I(1)` with the expected common-mass power.

[`two_loop_single_mass_vacuum.py`](two_loop_single_mass_vacuum.py) uses the
public `import rustred` API to generate, inspect, and apply the complete
two-loop closing artifact.

```bash
uv venv .venv
. .venv/bin/activate
maturin develop --release --features extension-module
python examples/python/two_loop_single_mass_vacuum.py
```

The script asserts four generated source rows, five guarded rule cells, and
the two expected master/mass-power pairs before printing the generation and
reduction TOML documents.

[`three_loop_k6_foundry_campaign.py`](three_loop_k6_foundry_campaign.py) runs
the separate K=6 foundry investigation through the release Python API. Its
default is the reviewed external-search-hint document; `--mode autonomous`
selects the strictly no-hint control:

```bash
uv venv .venv
. .venv/bin/activate
maturin develop --release --locked
mkdir k6-external-run k6-autonomous-run
python examples/python/three_loop_k6_foundry_campaign.py \
  --mode external-hints --n-cores 4 \
  --output k6-external-run/report.toml \
  --measurements-output k6-external-run/measurements.toml \
  --artifact-output k6-external-run/artifact.rribp
python examples/python/three_loop_k6_foundry_campaign.py \
  --mode autonomous --n-cores 4 \
  --output k6-autonomous-run/report.toml \
  --measurements-output k6-autonomous-run/measurements.toml \
  --artifact-output k6-autonomous-run/artifact.rribp
```

The external document carries 55 raw anchor/axis rectangles. Authenticated K4
routing reduces them to their semantic representatives during the run; no
recurrence row, right-hand side, coefficient, support, owner, terminal, or
master value is present in the input. These are deliberately sizable bounded
release experiments, not quick smoke tests, and an `incomplete` report is a
valid non-closing result. The optional artifact file is created only after
exact closure, deterministic encoding, and an independent cold-load replay
succeed.
All three destinations are create-if-absent and require an existing parent
directory, so each release run should use a fresh directory. Omitting
`--output` sends only the semantic report to stdout; measurements are emitted
only when `--measurements-output` is supplied and are never concatenated with
the report.

[`k6_closing_artifact.py`](k6_closing_artifact.py) generates the completed
autonomous K=6 artifact through `rustred.family_close`, then cold-inspects the
written bytes and applies the generic reducer. The mathematical family is
supplied in [`three_loop_k6.toml`](../input/three_loop_k6.toml), with the same
mathematical family and propagator order used by Vakint. Its identifier-safe
metadata name `rustred_three_loop_unit_mass_vacuum_k6_v1` intentionally changes
the old hyphenated-name fingerprint: regenerated Vakint assets and their
loader identity must migrate together. Terminal keys are unchanged. All 64
sectors are requested; the input contains no authored IBPs or oracle hints.

After building/installing the release Python extension, generation needs no
specialized Rust example binary or recompilation:

```bash
python examples/python/k6_closing_artifact.py k6.rr --generate --workers 6
python examples/python/k6_closing_artifact.py k6.rr
```

The first command refuses to overwrite any existing artifact and requires its
parent directory to exist. Generation reports 38 solved sectors, 26 proved-zero
sectors, 623 generated rules and 5,640 installed rule cells. Both commands check
the 6-coordinate family, 38 terminals, and the 30-term exact reduction of
`[2,1,1,1,1,1]`. The second command consumes supplied bytes only: no generation,
compilation, or FORM dependency. Run it as a separate process to demonstrate
fresh-process cold loading after generation. `--workers` controls generation
only and defaults to one.
