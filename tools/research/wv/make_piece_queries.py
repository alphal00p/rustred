#!/usr/bin/env python
"""W0.11 D1(b) falsifier inputs: helper-piece query variants of a four-loop
control (no engine change, input only).

Every physics query is kept verbatim and in order. Each helper (id matching
the oracles' helper pattern "anchor" or "upstream") is replaced, in place, by
the listed pieces in the listed order (smallest first, so the legacy min-ID
container rule prefers the smallest piece). Piece ids keep the substring
"anchor" so both oracles classify them as helpers.

Variants (piece = (A bound or None, R bound or None), None = unbounded):
  base        the base helpers unchanged ("today's bounded helpers"), helper-first
  asis        the base file unchanged, original order (four-all)
  unbounded   [(None, None)]                       -- the frontier fixture shape
  shells      [(None,12), (None,14), (None,16), (None,None)]   rank shells, top unbounded
  aslabs      [(19,None), (25,None), (None,None)]              A-slabs, top unbounded
  r12aslabs   [(None,12), (19,None), (25,None), (33,None)]     rank-12 orthant + unbounded-rank
                                                               A-slabs, no unbounded top
  rank14 / rank16 / rank20   [(None,r)]            helpers at a larger rank (escape check)
  rank14k/16k/20k, shellsk   as above, keeping each helper's own A bound (BMW's 37 A<=19 helpers)

Legacy-engine facts that shape the variants [src, rustred-4a17f9c7 source 66ede259]:
  - walking/initial_orthants.rs: every initial *full orthant* (lower 0, upper
    unbounded, no A/D bound; the rank may be finite) of a (phase, owner) is
    reduced to ONE fast-path target, the one with the LARGEST rank bound
    (None = unbounded wins); routing.rs / inspection.rs send every successor
    whose actual rank fits that bound to it before ordinary containment.
  - queue/index.rs find_controlled: ordinary containment returns the
    minimum-ID live container.
So rank shells and a full-orthant top collapse onto the top piece for all
successors; only pieces with an A bound are resolved by min-ID containment.

Usage: make_piece_queries.py BASE_QUERIES.json VARIANT OUT.json
"""
import json
import sys

VARIANTS = {
    "base": None,  # the base helpers unchanged, helper-first order
    "unbounded": [(None, None)],
    "shells": [(None, 12), (None, 14), (None, 16), (None, None)],
    "aslabs": [(19, None), (25, None), (None, None)],
    "r12aslabs": [(None, 12), (19, None), (25, None), (33, None)],
    "rank12": [(None, 12)],
    "rank14": [(None, 14)],
    "rank16": [(None, 16)],
    "rank20": [(None, 20)],
    # BMW: keep each helper's own A bound (37 upstream helpers carry A <= 19)
    "rank14k": [("keep", 14)],
    "rank16k": [("keep", 16)],
    "rank20k": [("keep", 20)],
    "shellsk": [("keep", 12), ("keep", 14), ("keep", 16), (None, None)],
}


def is_helper(q):
    return "anchor" in q["id"] or q["id"].startswith("upstream")


def piece(helper, a, r, k):
    n = len(helper["lower"])
    tag = f"a{a if a is not None else 'inf'}-r{r if r is not None else 'inf'}"
    return {
        "id": f"piece{k}-{tag}-anchor-{helper['owner']}",
        "owner": helper["owner"],
        "lower": [0] * n,
        "upper": [None] * n,
        "max_numerator_rank": r,
        "power_bounds": {"max_positive_power": a, "min_power_difference": None,
                         "max_power_difference": None},
    }


def main():
    base, variant, out = sys.argv[1:4]
    doc = json.load(open(base))
    if variant == "asis":  # the base file unchanged (order kept)
        text = json.dumps(doc, indent=1)
        open(out, "w").write(text)
        print(json.dumps({"variant": variant, "queries": len(doc["queries"]), "bytes": len(text.encode())}))
        return
    pieces = VARIANTS[variant]
    # Helper-first per owner (as the planner and the helper-bounds controls
    # order them): each owner's pieces, then that owner's physics queries, in
    # the order owners first appear. The rank12orthant base files list all
    # physics queries first and append the helpers; that order is not kept.
    order, helpers_of, physics_of = [], {}, {}
    for q in doc["queries"]:
        o = q["owner"]
        if o not in helpers_of:
            order.append(o)
            helpers_of[o], physics_of[o] = [], []
        (helpers_of if is_helper(q) else physics_of)[o].append(q)
    queries = []
    helpers = 0
    for o in order:
        # Pieces replace the owner's helpers once (four-all has two helpers per
        # owner: the planner helper and an appended rank-12 orthant).
        hs = helpers_of[o] if pieces is None else helpers_of[o][-1:]
        helpers += len(helpers_of[o]) - len(hs)
        for q in hs:
            helpers += 1
            assert q["lower"] == [0] * len(q["lower"]), q["id"]
            assert q["upper"] == [None] * len(q["upper"]), q["id"]
            own_a = q["power_bounds"]["max_positive_power"]
            if pieces is None:
                queries.append(q)
            else:
                queries.extend(piece(q, own_a if a == "keep" else a, r, k) for k, (a, r) in enumerate(pieces))
        queries.extend(physics_of[o])
    doc = dict(doc)
    doc["queries"] = queries
    text = json.dumps(doc, indent=1)
    open(out, "w").write(text)
    print(json.dumps({"variant": variant, "helpers": helpers, "queries": len(queries),
                      "bytes": len(text.encode())}))


if __name__ == "__main__":
    main()
