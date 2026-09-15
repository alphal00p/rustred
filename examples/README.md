# RustRed examples

The Rust, CLI, and Python examples generate the complete closing
parametric-IBP artifact for the two-loop single-scale vacuum sunset family.
RustRed derives four ordinary sources and five guarded rule cells, closes every
sector with exact `S3` routing, zero terminals, and one-loop factorization,
then reduces the sample integral `I(2,2,1)` to masters `I(1,1,1)` and
`I(0,1,1)`. The Python directory also contains the smaller complete one-loop
tadpole campaign used as a fast public-API smoke test.

[`input/four_loop_h.toml`](input/four_loop_h.toml) is a generic explicit
family-input example for a four-loop vacuum parent. It is deliberately kept
as user data rather than a RustRed built-in: the ten rows (nine physical
denominators plus an ISP at power zero) are parsed by the same strict TOML
frontend used for any supplied family. Use it with `rustred derive` as shown
in [the CLI documentation](../docs/CLI.md#four-loop-vacuum-input-external-family-data).
For a topology-neutral solver diagnostic, use the same input directly with
`rustred family-solve`; this path accepts arbitrary Project TOML and never
dispatches on the family name:

```console
rustred family-solve --input examples/input/four_loop_h.toml \
  --input-format toml --sectors 0000000001 --n-cores 4
```

- [`rust/`](rust/) uses the public `rustred` library directly.
- [`cli/`](cli/) uses `rustred campaign generate`, `inspect`, and `reduce`.
- [`python/`](python/) uses the public `import rustred` package.

The completed autonomous K=6 artifact is also available through both consumer
surfaces. [`cli/run_k6_closing_artifact.sh`](cli/run_k6_closing_artifact.sh)
generates it once (or reuses an existing release generator), then cold-loads
and reduces it with the CLI. [`python/k6_closing_artifact.py`](python/k6_closing_artifact.py)
performs the same authenticated load and exact reduction through
`import rustred`; neither consumer recompiles or regenerates an existing
artifact.

The artifact sets the common squared mass to one. Each reduction coefficient
also reports the exact power of `mass_squared` that restores a general common
mass by dimensional homogeneity.

No parameter declaration exists or is needed in these preset closing-artifact
workflows. In the separate generic `derive` input formats, RustRed infers
family scalars such as `d` and `m2`; their optional parameter allowlist is an
advanced validation aid, not required family data.

Run the examples from the repository root in the pinned development
environment with `SYMBOLICA_LICENSE` set.

The two root-level K=6 campaign documents exercise separate three-loop foundry
investigation lanes. They are useful bounded search diagnostics, but are not
needed by the completed artifact consumer examples above:

- [`k6_external_search_hints.toml`](k6_external_search_hints.toml) supplies
  55 reviewed integral-anchor rectangles, a coordinate order, and a modular
  probe portfolio. These are proposal metadata only; RustRed regenerates all
  IBPs and proves every admitted rule itself.
- [`k6_autonomous_campaign.toml`](k6_autonomous_campaign.toml) is the visibly
  separate control. It has no hints object at all, so RustRed derives its
  search program internally.

Both documents are bounded experiments. A successful process exit can still
report `outcome = "incomplete"`; only the exact compiler-closed outcome may
install an in-memory K=6 artifact. On that outcome RustRed deterministically
encodes the artifact, cold-loads and exactly replays it at the untrusted
boundary, and exposes the canonical bytes to the CLI and Python API. An
incomplete campaign never emits artifact bytes.
