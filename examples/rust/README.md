# Rust library example

[`two_loop_single_mass_vacuum.rs`](two_loop_single_mass_vacuum.rs) calls
`derive_two_loop_unit_mass_sunset`, checks the complete artifact manifest,
encodes durable bytes, and applies the resulting parametric IBPs to
`I(2,2,1)`.

```bash
cargo run --locked -p rustred-app --example two-loop-single-mass-vacuum
```

The defining output is:

```text
algorithm = rustred.generated.two-loop-unit-mass-sunset.v1
ordinary_sources = 4
closing_rule_cells = 5
source = ordinary-ibp:0:0
source = ordinary-ibp:0:1
source = ordinary-ibp:1:0
source = ordinary-ibp:1:1
target = [2, 2, 1]
master [0, 1, 1]: ... mass_squared_power = -3
master [1, 1, 1]: ... mass_squared_power = -2
```

`durable_bytes` and the full exact coefficient strings are also printed.
