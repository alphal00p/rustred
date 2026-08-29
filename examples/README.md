# RustRed examples

These examples derive the complete ordinary parametric IBP source set for the
two-loop, single-mass-scale vacuum (equal-mass sunset) family

```text
D1 = k1^2 - m2
D2 = k2^2 - m2
D3 = (k1 + k2)^2 - m2.
```

There are two loop momenta and no external momenta, so the generic ordinary
source count is `L * (L + E) = 2 * 2 = 4`. Rust and Python assert the four
stable row IDs directly, while the CLI fixture is pinned by its integration
test. Every surface uses the equation convention

```text
sum(coefficient * I(n + shift)) = 0.
```

Choose the interface you want to exercise:

- [`rust/`](rust/) calls the fine-grained `rustred` Rust library directly;
- [`cli/`](cli/) invokes the `rustred derive` command; and
- [`python/`](python/) uses the public `import rustred` Python API.

Run the documented commands inside the repository's pinned `nix develop`
environment (or another environment providing the same Rust/Python tools).

The examples generate the full universal parametric source identities over
`K(n)`. They do not claim that those four identities are already a closed
sector-reduction table. The common mass parameter `m2` can be specialized to
`1` after generation when only the single-scale vacuum family is needed.
