#!/usr/bin/env python3
"""Seven-loop inputs for the shared, loop-count-independent study launcher.

Preparation alone runs no IBP generation. Native generation requires --start
and an executable compiled with the input's 28-coordinate dispatch.
"""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from six_loop_study.generate import main


if __name__ == "__main__":
    raise SystemExit(main(entrypoint=Path(__file__).resolve()))
