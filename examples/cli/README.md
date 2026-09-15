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

The K=6 closing artifact is generated once and then consumed by the ordinary
RustRed CLI. The example prefers existing release binaries, so repeated runs
do not recompile; set `RUSTRED_K6_GENERATOR` and `RUSTRED_BIN` when binaries
live outside `target/release`:

```bash
sh examples/cli/run_k6_closing_artifact.sh /tmp/rustred-k6.rr 6
```

The script refuses to overwrite an artifact, authenticates it in a fresh CLI
process, and applies it to `[2,1,1,1,1,1]`. Inspection reports the canonical
K=6 family with 38 terminal entries; reduction reports 30 exact master terms.
Generation is autonomous and uses no FORM-derived rules.

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
