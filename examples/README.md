# RustRed examples

These three examples generate the complete closing parametric-IBP artifact for
the two-loop single-scale vacuum sunset family. RustRed derives four ordinary
sources and five guarded rule cells, closes every sector with exact `S3`
routing, zero terminals, and one-loop factorization, then reduces the sample
integral `I(2,2,1)` to masters `I(1,1,1)` and `I(0,1,1)`.

- [`rust/`](rust/) uses the public `rustred` library directly.
- [`cli/`](cli/) uses `rustred campaign generate`, `inspect`, and `reduce`.
- [`python/`](python/) uses the public `import rustred` package.

The artifact sets the common squared mass to one. Each reduction coefficient
also reports the exact power of `mass_squared` that restores a general common
mass by dimensional homogeneity.

No parameter declaration exists or is needed in these preset closing-artifact
workflows. In the separate generic `derive` input formats, RustRed infers
family scalars such as `d` and `m2`; their optional parameter allowlist is an
advanced validation aid, not required family data.

Run the examples from the repository root in the pinned development
environment with `SYMBOLICA_LICENSE` set.
