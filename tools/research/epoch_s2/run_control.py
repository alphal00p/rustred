#!/usr/bin/env python
"""epoch-s2 lane: TMP/fable51-controls/run_control.py with outputs under TMP/epoch-s2/runs.
Usage: run_control.py --binary BIN --family fg --label NAME --cpus 0-5 [--policy epoch] [--workers N] [--extra ...]
"""
import importlib.util
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "run_control", "/common/dev/rustred/TMP/fable51-controls/run_control.py")
rc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rc)
rc.OUT_ROOT = Path("/common/dev/rustred/TMP/epoch-s2/runs")
rc.main()
