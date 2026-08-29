# Python example

[`two_loop_single_mass_vacuum.py`](two_loop_single_mass_vacuum.py) embeds the
family declaration, calls the public `import rustred` API, verifies that all
four ordinary rows were generated, and prints the canonical TOML result.

Build the development extension once and run the example from the repository
root:

```bash
uv venv .venv
. .venv/bin/activate
maturin develop --features extension-module
python examples/python/two_loop_single_mass_vacuum.py
```
