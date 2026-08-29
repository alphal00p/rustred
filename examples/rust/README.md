# Rust library example

[`two_loop_single_mass_vacuum.rs`](two_loop_single_mass_vacuum.rs) uses the
fine-grained `rustred` crate to parse and lower the family, construct the
topology-independent IBP generator, generate every prepared ordinary row, and
print the resulting equations.

From the repository root, run:

```bash
cargo run --locked -p rustred-app --example two-loop-single-mass-vacuum
```

The example is registered on the existing non-publishable application package
only so that it participates in workspace `--all-targets` checks; its code
uses the public `rustred` library API directly.
