#!/usr/bin/env python3
"""Lane allnodes: census of rule-level `domain_bounds` in one table, STREAMING (64 MiB chunks; memory independent of the
line length; rule lines of several GB exist). Inspect-only.
Output: {table, rules, rules_with_domain_bounds, with_nontrivial_target, examples}. A target is TRIVIAL iff it is
I(n0,n1,...,n{N-1}) (slot i = parameter n_i); anything else (offset, fixed value, permutation) is a non-trivial pattern.
Usage: census_bounds.py <table>
"""
import json
import re
import sys

t = sys.argv[1]
TOK = re.compile(rb'\n|"domain_bounds"|"target": ?"I\(([^"]*)\)"|"type": ?"rule"')
TAIL = 1 << 16
rules = n = nt = 0
ex = []
is_rule = has_db = False
target = None


def end_line():
    global n, nt
    if is_rule and has_db:
        n += 1
        if target is not None:
            tg = target.decode().split(",")
            if any(x.strip() != f"n{i}" for i, x in enumerate(tg)):
                nt += 1
                if len(ex) < 3:
                    ex.append("I(" + ",".join(tg) + ")")


with open(t, "rb") as f:
    buf = b""
    eof = False
    while not eof:
        chunk = f.read(64 << 20)
        eof = not chunk
        buf += chunk
        limit = len(buf) if eof else max(0, len(buf) - TAIL)
        pos = 0
        for m in TOK.finditer(buf):
            if m.start() >= limit:
                break
            g = m.group(0)
            if g == b"\n":
                end_line()
                is_rule = has_db = False
                target = None
            elif g == b'"domain_bounds"':
                has_db = True
            elif g.startswith(b'"type"'):
                is_rule = True
                rules += 1
            else:
                if target is None:
                    target = m.group(1)
            pos = m.end()
        buf = buf[max(pos, limit):] if not eof else b""
    end_line()
print(json.dumps({"table": t, "rules": rules, "rules_with_domain_bounds": n, "with_nontrivial_target": nt, "examples": ex}))
