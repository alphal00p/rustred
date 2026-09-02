# Python examples

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
