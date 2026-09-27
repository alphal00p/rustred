#!/usr/bin/env python
"""W0.2 oracle lane: run_control.py with outputs under TMP/w0/oracle/runs.

Usage: run_controls.py --binary BIN --family fg --label NAME --cpus 46-51
       [--policy ordered|ready] [--workers N] [--queries PATH]
--queries swaps the command's query document (frontier fixture) and resizes
--max-queries/--max-query-bytes to it.
"""
import importlib.util
import json
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "run_control", "/common/dev/rustred/TMP/fable51-controls/run_control.py")
rc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rc)
rc.OUT_ROOT = Path("/common/dev/rustred/TMP/w0/oracle/runs")

argv = sys.argv[1:]
queries = None
if "--queries" in argv:
    index = argv.index("--queries")
    queries = Path(argv[index + 1])
    del argv[index:index + 2]
    original_rewrite = rc.rewrite

    def rewrite(command, *args, **kwargs):
        command = original_rewrite(command, *args, **kwargs)
        document = json.loads(queries.read_text())
        command[command.index("--queries") + 1] = str(queries)
        command[command.index("--max-queries") + 1] = str(len(document["queries"]))
        command[command.index("--max-query-bytes") + 1] = str(queries.stat().st_size)
        return command

    rc.rewrite = rewrite
sys.argv = [sys.argv[0]] + argv
rc.main()
