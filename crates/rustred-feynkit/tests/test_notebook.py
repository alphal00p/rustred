"""Execute the complete FeynCalc reproduction against the installed hep bridge."""

import json
from pathlib import Path

import pytest

NOTEBOOK = (
    Path(__file__).resolve().parents[3]
    / "examples/notebooks/feyncalc_phi4_two_loop.ipynb"
)


def test_feyncalc_phi4_two_loop_notebook():
    pytest.importorskip("IPython", reason="the notebook requires IPython display")
    notebook = json.loads(NOTEBOOK.read_text())
    namespace = {"__name__": "__main__"}
    # Use the same installed extension as the other integration tests. No stored
    # outputs are trusted or modified, and the notebook's exact physics checks
    # run after genuine Laporta and parametric reductions.
    for index, cell in enumerate(notebook["cells"]):
        if cell["cell_type"] == "code":
            source = "".join(cell["source"])
            # Execute only this trusted, version-controlled repository notebook.
            exec(compile(source, f"{NOTEBOOK}:cell_{index}", "exec"), namespace)  # noqa: S102

    assert namespace["laporta"].stats["rows"] > 0
    assert namespace["parametric"].rules
    assert {"Zphi", "Zm"} <= namespace.keys()
