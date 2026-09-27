#!/usr/bin/env python
"""Replace placeholder tokens in a markdown note with file contents.

  splice_results.py NOTE TOKEN=FILE [TOKEN=FILE ...] [--check]

Each TOKEN must occur exactly once in NOTE (as a whole line, or inside a
line for short inline values); it is replaced by FILE's content with
trailing newlines stripped. --check only reports the tokens still present
(any all-caps word of >= 2 underscore-joined parts on its own line).
"""
import argparse
import re
import sys
from pathlib import Path


def main():
    p = argparse.ArgumentParser()
    p.add_argument("note")
    p.add_argument("pairs", nargs="*")
    p.add_argument("--check", action="store_true")
    args = p.parse_args()
    note = Path(args.note)
    text = note.read_text()
    for pair in args.pairs:
        token, _, path = pair.partition("=")
        count = text.count(token)
        if count != 1:
            sys.exit(f"{token}: found {count} times in {note}")
        text = text.replace(token, Path(path).read_text().rstrip("\n"))
    if args.pairs:
        note.write_text(text)
    left = re.findall(r"^([A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+)\s*$", text, flags=re.M)
    print("placeholders left:", left or "none")
    if args.check and left:
        sys.exit(1)


if __name__ == "__main__":
    main()
