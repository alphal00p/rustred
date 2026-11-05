#!/usr/bin/env python3
"""Export vakint's canonical p-integral families to crates/rustred-core/data/p/p-families.v1.json.

Usage: scripts/p_families_from_vakint.py <gammaloop checkout> [--rev REV] [--out FILE]

Reads `crates/vakint/src/p_integral.rs` of the gammaloop revision REV through `git show` (the checkout is never
touched) and writes the schema `rustred.p-families.v1`, consumed by `rustred::propagator::families`. One entry per
family of vakint's `P_FAMILIES`, in vakint's order: lines, then ISPs, slots never renumbered. `momentum` rows are over
`(k_1, .., k_L, q)`, with vakint's `p(1)` as `q`; a line's momentum flows from `endpoints[0]` to `endpoints[1]`; an
ISP row is a momentum whose square is the slot. `gram` is Minkowski `q.q = -1`, i.e. Euclidean `Q^2 = 1`.

Bump `P_FAMILIES_VERSION` (`crates/rustred-core/src/propagator/mod.rs`) whenever the output changes.
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

DEFAULT_REV = "21f6624e2"
SOURCE = "crates/vakint/src/p_integral.rs"
SCHEMA = "rustred.p-families.v1"
GRAM = [[-1]]
FAMILY_COUNT = 16

ENTRY = re.compile(
    r'PFamilyData \{\s*name: "(?P<name>\w+)",\s*loops: (?P<loops>\d+),.*?'
    r'lines: "topo\((?P<lines>.*?)\)",\s*isps: &\[(?P<isps>.*?)\],\s*\}',
    re.S,
)
PROP = re.compile(r"^prop\((\d+),edge\((\d+),(\d+)\),(.*),0,pow\((\d+)\)\)$")
TERM = re.compile(r"([+-]?)(k|p)\((\d+)\)")


def die(message):
    sys.exit(f"p_families_from_vakint: {message}")


def momentum(text, loops):
    """`k(1)-k(2)+p(1)` as a row over (k_1, .., k_L, q)."""
    row = [0] * (loops + 1)
    if TERM.sub("", text):
        die(f"cannot parse the momentum {text!r}")
    for sign, kind, index in TERM.findall(text):
        index = int(index)
        if kind == "p" and index != 1:
            die(f"unexpected external momentum in {text!r}")
        if kind == "k" and not 1 <= index <= loops:
            die(f"unexpected loop momentum in {text!r}")
        row[loops if kind == "p" else index - 1] += -1 if sign == "-" else 1
    return row


def family(match):
    name, loops = match["name"], int(match["loops"])
    lines = []
    for position, prop in enumerate(match["lines"].split("*"), start=1):
        parsed = PROP.match(prop)
        if not parsed or int(parsed[1]) != position or int(parsed[5]) != position:
            die(f"{name}: cannot parse {prop!r}")
        lines.append({"endpoints": [int(parsed[2]), int(parsed[3])], "momentum": momentum(parsed[4], loops)})
    isps = [momentum(isp, loops) for isp in re.findall(r'"([^"]+)"', match["isps"])]
    if len(lines) != 3 * loops - 1 or len(lines) + len(isps) != loops * (loops + 3) // 2:
        die(f"{name}: {len(lines)} lines and {len(isps)} ISPs at {loops} loops")
    vertices = max(vertex for line in lines for vertex in line["endpoints"])
    entry = {
        "name": name,
        "loops": loops,
        "vertices": vertices,
        "lines": lines,
        "isps": isps,
        "external": {"in": 1, "out": 2},
        "gram": GRAM,
    }
    check_conservation(entry)
    return entry


def check_conservation(entry):
    """Momentum is conserved at every vertex, with q entering at `in` and leaving at `out`."""
    q = [0] * entry["loops"] + [1]
    width = len(q)
    balance = {vertex: [0] * width for vertex in range(1, entry["vertices"] + 1)}
    for line in entry["lines"]:
        tail, head = line["endpoints"]
        for column in range(width):
            balance[tail][column] -= line["momentum"][column]
            balance[head][column] += line["momentum"][column]
    for column in range(width):
        balance[entry["external"]["in"]][column] += q[column]
        balance[entry["external"]["out"]][column] -= q[column]
    for vertex, row in balance.items():
        if any(row):
            die(f"{entry['name']}: momentum is not conserved at vertex {vertex}: {row}")


def render(value, indent=0):
    """Readable, deterministic JSON: objects one key per line, a line or an ISP row on one line."""
    pad = " " * indent
    if isinstance(value, dict) and "momentum" not in value and "in" not in value:
        items = [f'{pad}  {json.dumps(key)}: {render(item, indent + 2)}' for key, item in value.items()]
        return "{\n" + ",\n".join(items) + f"\n{pad}}}"
    if isinstance(value, list) and value and isinstance(value[0], dict):
        items = [f"{pad}  {render(item, indent + 2)}" for item in value]
        return "[\n" + ",\n".join(items) + f"\n{pad}]"
    return json.dumps(value, separators=(", ", ": "))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("gammaloop", type=Path)
    parser.add_argument("--rev", default=DEFAULT_REV)
    parser.add_argument("--out", type=Path, default=Path("crates/rustred-core/data/p/p-families.v1.json"))
    arguments = parser.parse_args()
    shown = subprocess.run(
        ["git", "-C", str(arguments.gammaloop), "show", f"{arguments.rev}:{SOURCE}"],
        capture_output=True,
        text=True,
        check=False,
    )
    if shown.returncode:
        die(shown.stderr.strip())
    table = shown.stdout.split("pub(crate) const P_FAMILIES", 1)[-1].split("\n];", 1)[0]
    families = [family(match) for match in ENTRY.finditer(table)]
    if len(families) != FAMILY_COUNT:
        die(f"found {len(families)} families, expected {FAMILY_COUNT}")
    document = {
        "schema": SCHEMA,
        "provenance": f"exported by scripts/p_families_from_vakint.py from gammaloop {arguments.rev}:{SOURCE}",
        "families": families,
    }
    text = render(document) + "\n"
    if json.loads(text) != document:
        die("the rendered JSON does not parse back to the document")
    arguments.out.write_text(text)


if __name__ == "__main__":
    main()
