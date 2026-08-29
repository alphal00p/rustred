# CLI example

[`two_loop_single_mass_vacuum.symbolica`](two_loop_single_mass_vacuum.symbolica)
declares the complete equal-mass sunset family. The runner requests ordinary
parametric IBPs explicitly and writes the canonical
`rustred.derive-output.toml.v1` document to standard output.

From the repository root, run:

```bash
sh examples/cli/run.sh
```

Equivalently, invoke the command directly:

```bash
cargo run --locked -p rustred-app --bin rustred -- derive \
  --input examples/cli/two_loop_single_mass_vacuum.symbolica \
  --input-format symbolica \
  --relations ordinary \
  --n-cores 1 \
  --output -
```
