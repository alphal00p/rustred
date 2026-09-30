"""Named finite discovery recipes; not integral/pivot orders or proof authority.

The returned JSON goes to ``family_candidates(discovery_strategy=...)`` or a
file passed to ``rustred family-candidates --discovery-strategy``. Evaluation
and ordinal validation are native. New choices do not compile engine code.
"""

import json


def discovery_strategy(*, rows="input-order", sectors="active-first", weights=None,
                       descending=False):
    """Select one generic row feature and independent sector job priority.

    More elaborate lexicographic recipes/materialized Rust callback plans can
    use the same documented JSON descriptor directly. This helper neither
    clips descendants nor changes the mathematical integral comparison.
    """
    row_features = {"terms", "coefficient-monomials", "absolute-shifts",
                    "positive-shifts", "negative-shifts"}
    if rows != "input-order" and rows not in row_features:
        raise ValueError("unknown discovery row feature")
    if sectors not in {"active-first", "input-order", "weighted-support"}:
        raise ValueError("unknown discovery sector priority")
    if type(descending) is not bool:
        raise TypeError("descending must be bool")
    weighted_row = rows.endswith("-shifts")
    if weighted_row or sectors == "weighted-support":
        if (not isinstance(weights, (list, tuple)) or not weights
                or any(type(w) is not int or not 0 <= w <= 1_000_000 for w in weights)
                or not any(weights)):
            raise ValueError("weights must be nonzero bounded unsigned integers")
        weights = list(weights)
    elif weights is not None:
        raise ValueError("weights require a weighted row or sector feature")
    row = {"kind": rows}
    if rows != "input-order":
        feature = {"kind": rows}
        if weighted_row:
            feature["weights"] = weights
        row = {"kind": "features", "priorities": [
            {"feature": feature, "descending": descending}]}
    sector = {"kind": sectors}
    if sectors == "weighted-support":
        sector.update(weights=weights, descending=descending)
    return json.dumps({"version": 1, "sectors": sector, "rows": row},
                      sort_keys=True, separators=(",", ":"))
