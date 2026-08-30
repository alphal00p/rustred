# Python example

[`two_loop_single_mass_vacuum.py`](two_loop_single_mass_vacuum.py) uses the
public `import rustred` API to generate, inspect, and apply the complete
two-loop closing artifact.

```bash
uv venv .venv
. .venv/bin/activate
maturin develop --features extension-module
python examples/python/two_loop_single_mass_vacuum.py
```

The script asserts four generated source rows, five guarded rule cells, and
the two expected master/mass-power pairs before printing the generation and
reduction TOML documents.
