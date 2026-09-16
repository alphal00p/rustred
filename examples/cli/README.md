# CLI example

[`run.sh`](run.sh) generates the complete `unit-mass-vacuum-k3` artifact into
a temporary file, authenticates and inspects those bytes, and applies them to
`I(2,2,1)`:

```bash
sh examples/cli/run.sh
```

The inspection output reports algorithm
`rustred.generated.two-loop-unit-mass-sunset.v1`, arity 3, four source rows,
five guarded rules, two masters, and four zero sectors. The reduction output
contains master keys `[0,1,1]` and `[1,1,1]`, with common-mass-squared powers
`-3` and `-2`, respectively. The temporary artifact is removed on exit.

## Completed K=6 artifact

The K=6 example uses the generic `family-close` command with
[`three_loop_k6.toml`](../input/three_loop_k6.toml). The mathematical family and
propagator order match the Vakint artifact; all 64 sectors are included.
The identifier-safe metadata name `rustred_three_loop_unit_mass_vacuum_k6_v1`
intentionally replaces the older hyphenated name, changing the fingerprint.
Regenerated Vakint assets and their loader identity must migrate together;
the mathematics and terminal keys are unchanged. There are no authored IBPs
or oracle hints in the input.
Build the ordinary release CLI once, then run the example without recompiling:

```bash
cargo build --release --locked -p rustred-app --bin rustred
sh examples/cli/run_k6_closing_artifact.sh /tmp/rustred-k6.rr 6
```

Set `RUSTRED_BIN` when the release binary lives outside `target/release`.
The script refuses to overwrite an artifact, authenticates it in a fresh CLI
process, and applies it to `[2,1,1,1,1,1]`. Inspection reports the canonical
K=6 family with 38 terminal entries; reduction reports 30 exact master terms.
Generation reports 38 solved sectors, 26 proved-zero sectors, 623 generated
rules and 5,640 installed rule cells. The equivalent generation command is:

```bash
./target/release/rustred family-close \
  --input examples/input/three_loop_k6.toml --input-format toml \
  --n-cores 6 --progress --output /tmp/rustred-k6.rr
```

Use a fresh output path when running either command; neither requires the
specialized Rust-library example binary.

The same pure-RustRed K=6 foundry lanes are directly runnable through the CLI.
Use a release build and keep their reports separate:

```bash
cargo build --release --locked -p rustred-app --bin rustred
mkdir k6-external-run k6-autonomous-run

./target/release/rustred campaign run-waves \
  --config examples/k6_external_search_hints.toml \
  --output k6-external-run/report.toml \
  --measurements-output k6-external-run/measurements.toml \
  --artifact-output k6-external-run/artifact.rribp \
  --n-cores 4

./target/release/rustred campaign run-waves \
  --config examples/k6_autonomous_campaign.toml \
  --output k6-autonomous-run/report.toml \
  --measurements-output k6-autonomous-run/measurements.toml \
  --artifact-output k6-autonomous-run/artifact.rribp \
  --n-cores 4
```

The first configuration contains only reviewed search metadata. The second has
no external hints. Both regenerate the same nine ordinary K=6 IBPs inside
RustRed and use the same exact replay/descent/coverage compiler. They are
bounded closure investigations, so `outcome = "incomplete"` is expected until
the remaining K=6 cover is actually closed. An incomplete run writes no
artifact file. Only an exactly closed campaign is encoded, independently
cold-loaded and replayed, and written to the requested artifact path.
Use a fresh directory for every release run: without `--force`, report,
measurement, and artifact destinations are all create-if-absent, so an older
immutable artifact cannot be silently replaced.
