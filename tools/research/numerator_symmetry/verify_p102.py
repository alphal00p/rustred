"""Independent exact audit of the public P102 numerator-symmetry example.

This is a research regression driver, not a topology-specific solver rule. It
uses native HEPKit DOT/model ingress and Symbolica for all polynomial/matrix
operations. Inputs must be downloaded separately from the pinned source commit.
The optional fingerprint adapter changes only in-memory serialization metadata;
all loop-basis annotations, propagators, numerators and factors remain untouched.
"""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
from pathlib import Path
import re

from symbolica import E, Matrix, Replacement
from symbolica.community import hepkit as hep


UPSTREAM = "aca9b1d324044165c2981213a920d6828d679dc0"
NAMES = ("P102", "P106", "P162", "P86")
INPUT_SHA256 = {
    "P102": "d6adeddc296e3d0c101f178fd38e78d25e2606b247f4dc664c7c5aad155496b9",
    "P106": "e81bc2a869659b99c9e3c8819b52ba96e0dae58dfa549d9a2ace5516175ef156",
    "P162": "8f76c65dc0ea4bfda1783b96099c8c1334219f8ae23224a30a33b05d9057d97f",
    "P86": "8ccc7b9db9e398ddbeae2996a35ad1c7fd3536b3512789ad618641626a0c18ec",
}
MODEL_SHA256 = "d2628bb5672b18807039be6f9218c07701cfcb80320458277cda2b9f0ef47bd1"


def zero(expression):
    """Exact Symbolica polynomial normalization; no sampling/zero heuristic."""
    assert expression.expand() == 0, expression


def linear(row, variables):
    return sum((weight * variable for weight, variable in zip(row, variables)), E("0"))


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--allow-model-fingerprint-adapter", action="store_true")
    args = parser.parse_args()

    model_text = (args.inputs / "model.json").read_text()
    assert hashlib.sha256(model_text.encode()).hexdigest() == MODEL_SHA256
    model_json = json.loads(model_text)
    particle = model_json["particles"]
    assert len(particle) == 1
    assert {key: particle[0][key] for key in ("name", "pdg_code", "spin", "color", "mass", "width")} == {
        "name": "phi", "pdg_code": 9000001, "spin": 1, "color": 1,
        "mass": "mUV", "width": "ZERO",
    }
    assert model_json["propagators"] == [{
        "name": "phi_prop", "particle": "phi", "numerator": "𝑖",
        "denominator": "-mUV^2+P(idx(1,1))^2",
    }]
    rules = model_json["vertex_rules"]
    for index, rule in enumerate(rules):
        assert rule["name"] == f"V_SCALAR{index + 2}"
        assert rule["particles"] == ["phi"] * (index + 2)

    model = hep.Model.from_json(model_text)
    identity_graph = hep.FeynmanDiagram.from_dot(
        model, 'digraph {v->v[id=0, particle="phi", lmb_id=0];}'
    )
    current_fingerprint = bytes(json.loads(identity_graph.to_json())["model"]).hex()
    diagrams, json_graphs, hashes, metadata_adapters = {}, {}, {}, {}
    for name in NAMES:
        raw = (args.inputs / f"{name}.dot").read_text()
        hashes[name] = hashlib.sha256(raw.encode()).hexdigest()
        assert hashes[name] == INPUT_SHA256[name]
        fingerprints = re.findall(r'model_fingerprint="([0-9a-f]{64})"', raw)
        assert len(fingerprints) == 1
        old_fingerprint = fingerprints[0]
        if old_fingerprint != current_fingerprint:
            if not args.allow_model_fingerprint_adapter:
                raise ValueError(
                    f"{name}: stored model fingerprint {old_fingerprint} differs from "
                    f"the native model's {current_fingerprint}; pass the explicit "
                    "research-only metadata-adapter flag after inspecting model.json"
                )
            raw = raw.replace(
                f'model_fingerprint="{old_fingerprint}"',
                f'model_fingerprint="{current_fingerprint}"',
            )
            metadata_adapters[name] = {"stored": old_fingerprint, "loaded": current_fingerprint}
        diagram = hep.FeynmanDiagram.from_dot(model, raw)
        diagram.validate()
        graph = json.loads(diagram.to_json())
        assert diagram.loop_count == 5
        assert graph["symmetry_factor"] == 1
        for key in ("overall_factor", "numerator_prefactor", "projector"):
            assert E(graph[key]) == 1
        for edge in diagram.internal_edges:
            assert edge.particle_name == "phi" and edge.particle_pdg == 9000001
        for vertex in graph["vertices"]:
            assert rules[vertex["interaction"]]["name"] in {
                "V_SCALAR3", "V_SCALAR5", "V_SCALAR6", "V_SCALAR9"
            }
        diagrams[name], json_graphs[name] = diagram, graph

    for name in ("P106", "P162", "P86"):
        assert E(diagrams[name].numerator) == 1

    p102, p106 = json_graphs["P102"], json_graphs["P106"]
    assert p102["edges"] == p106["edges"]
    assert p102["loop_momentum_basis"] == p106["loop_momentum_basis"]
    assert p102["loop_momentum_basis"]["loop_edges"] == [1, 2, 4, 5, 6]
    assert p102["loop_momentum_basis"]["edge_signatures"]["1"]["loops"] == [1, 0, 0, 0, 0]

    family = diagrams["P102"].integral_family()
    loops = family.loop_momenta
    a, b, h, c, q = loops
    kin = hep.Kinematics(E("gammalooprs::dim"), momenta=loops)
    sp = kin.scalar_product
    mass = model.particle("phi").mass
    m2 = mass**2
    p = q - a - b + c
    physical = [p, a, b, h + q, h, c, q]
    for denominator, momentum in zip(family.denominators[:7], physical):
        zero(denominator - sp(momentum, momentum) + m2)
    raw_numerator = E(diagrams["P102"].numerator).replace_multiple([
        Replacement(
            E(f"FeynKit::LoopMomentum({i},spenso::mink(gammalooprs::dim))"),
            E(f"gammalooprs::K({i},spenso::mink(gammalooprs::dim))"),
        ) for i in range(5)
    ])
    numerator = sp(a + b - c, a + b - c) - m2
    zero(raw_numerator - numerator)
    zero(numerator - (sp(p, p) - m2 + sp(q, q) - 2 * sp(q, p)))

    # r=(p,a,b,-c), sum r=q. These integer candidate rows are semantics;
    # Symbolica computes every determinant and verifies every quadratic image.
    r_rows = [(-1, -1, 0, 1, 1), (1, 0, 0, 0, 0),
              (0, 1, 0, 0, 0), (0, 0, 0, -1, 0)]
    r = [linear(row, loops) for row in r_rows]
    zero(sum(r, E("0")) - q)
    orbit = E("0")
    certificates = []
    for permutation in itertools.permutations(range(4)):
        matrix = [r_rows[permutation[1]], r_rows[permutation[2]],
                  (0, 0, 1, 0, 0), tuple(-x for x in r_rows[permutation[3]]),
                  (0, 0, 0, 0, 1)]
        determinant = Matrix.from_nested(matrix).det()
        assert determinant == 1 or determinant == -1
        mapped = [linear(row, loops) for row in matrix]
        aa, bb, hh, cc, qq = mapped
        pp = qq - aa - bb + cc
        zero(pp - r[permutation[0]])
        images = [pp, aa, bb, hh + qq, hh, cc, qq]
        source_order = [0, 1, 2, 5]
        edge_permutation = [source_order[permutation[0]], source_order[permutation[1]],
                            source_order[permutation[2]], 3, 4,
                            source_order[permutation[3]], 6]
        for transformed, destination in zip(images, edge_permutation):
            zero(sp(transformed, transformed) - sp(physical[destination], physical[destination]))
        orbit += sp(aa + bb - cc, aa + bb - cc) - m2
        certificates.append({"matrix": matrix, "edge_permutation": edge_permutation,
                             "determinant": str(determinant)})
    average = orbit / 24
    expected = sp(q, q) / 2 + sum((sp(x, x) - m2 for x in r), E("0")) / 4
    zero(average - expected)
    # The original numerator and its average are NOT equal pointwise.
    assert (numerator - average).expand() != 0

    # Exact native signed momentum witnesses for the two contractions.
    pinch_maps = {
        "P162": {"removed": 0, "edges": [(1, 0), (2, 3), (3, 1), (4, 2), (5, 5), (6, 4)],
                 "matrix": [(1, 0, 0, 0, 0), (0, 0, 1, 0, 0), (0, 1, 0, 0, 0),
                            (0, 0, 0, 0, 1), (0, 0, 0, -1, 0)]},
        "P86": {"removed": 6, "edges": [(0, 0), (1, 1), (2, 2), (3, 5), (4, 3), (5, 4)],
                "matrix": [(1, 0, 0, 0, 0), (0, 1, 0, 0, 0), (0, 0, 1, 0, 0),
                           (0, 0, 0, 1, 0), (0, 0, -1, 0, -1)]},
    }
    for name, witness in pinch_maps.items():
        target = diagrams[name].integral_family()
        target_loops = target.loop_momenta
        target_kin = hep.Kinematics(E("gammalooprs::dim"), momenta=target_loops)
        source_loops = [linear(row, target_loops) for row in witness["matrix"]]
        aa, bb, hh, cc, qq = source_loops
        source_edges = [qq-aa-bb+cc, aa, bb, hh+qq, hh, cc, qq]
        for source_edge, target_edge in witness["edges"]:
            vector = source_edges[source_edge]
            zero(target_kin.scalar_product(vector, vector) - m2 - target.denominators[target_edge])
        assert Matrix.from_nested(witness["matrix"]).det() == -1

    # A dotted or unequal-mass r-line breaks the full S4 weighted stabilizer.
    swap_p_a = next(x for x in certificates if x["edge_permutation"] == [1, 0, 2, 3, 4, 5, 6])
    perm = swap_p_a["edge_permutation"]
    for weights in ([2, 1, 1, 1, 1, 1, 1], [1, 2, 1, 1, 1, 1, 1]):
        assert any(weights[i] != weights[j] for i, j in enumerate(perm))
    masses = [m2, E("other_mass_squared"), m2, m2, m2, m2, m2]
    assert any((masses[i] - masses[j]).expand() != 0 for i, j in enumerate(perm))
    # All terms have degree 5*d-12 after restoring m; scalar P106 has 5*d-14.
    d = E("gammalooprs::dim")
    zero((5*d - 2*7 + 2) - (5*d - 2*6))
    zero((5*d - 2*7 + 2) - (2 + 5*d - 2*7))

    print(json.dumps({
        "source_commit": UPSTREAM, "input_sha256": hashes,
        "model_sha256": hashlib.sha256(model_text.encode()).hexdigest(),
        "metadata_only_model_adapters": metadata_adapters,
        "stored_lmb_edges": [1, 2, 4, 5, 6],
        "s4_witnesses": certificates, "pinch_witnesses": pinch_maps,
        "exact_result": "P102 = P162 + P86/2 + mUV^2*P106/2",
        "unit_mass_result": "P102 = P162 + P86/2 + P106/2",
        "unequal_power_and_mass_controls": "reject full S4",
        "claim": "exact integrated relation; not a pointwise numerator identity or basis-minimality claim",
    }, indent=2))


if __name__ == "__main__":
    main()
