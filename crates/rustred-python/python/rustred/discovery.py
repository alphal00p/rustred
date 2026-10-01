"""Named finite discovery recipes; not integral/pivot orders or proof authority.

The returned JSON goes to ``family_candidates(discovery_strategy=...)`` or a
file passed to ``rustred family-candidates --discovery-strategy``. Evaluation
and ordinal validation are native. New choices do not compile engine code.
"""

import json
import copy
import sys


def discovery_strategy(*, rows="input-order", sectors="active-first", weights=None,
                       descending=False, rule_selection=None):
    """Select one generic row feature and independent sector job priority.

    More elaborate lexicographic recipes/materialized Rust callback plans can
    use the same documented JSON descriptor directly. This helper neither
    clips descendants nor changes the mathematical integral comparison. Pass a
    ``rule_portfolio(...)`` recipe to opt into version 2; otherwise serialization
    remains the original version-1 first-valid strategy.
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
    descriptor = {"version": 1, "sectors": sector, "rows": row}
    if rule_selection is not None:
        descriptor["version"] = 2
        descriptor["rule_selection"] = _validated_portfolio(rule_selection)
    return json.dumps(descriptor,
                      sort_keys=True, separators=(",", ":"))


_QUALITY_FEATURES = frozenset({
    "max-numerator-shift-excursion", "total-numerator-shift-excursion",
    "max-positive-shift-excursion", "exceptional-cases", "affine-exceptional-cases",
    "guard-branches", "guard-predicates", "rhs-terms", "coefficient-monomials",
    "source-rows", "search-rows", "total-positive-shift-excursion",
})


def rule_portfolio(*, alternatives, quality, max_depth, max_rows,
                   max_exact_trace_rows, max_exact_trace_terms, trigger=None):
    """Build a bounded rule-choice recipe for ``discovery_strategy``.

    ``alternatives`` contains one or two named unweighted row features (e.g.
    ``"terms"``), or explicit native source-priority dictionaries. ``quality``
    is an ordered list of feature names (ascending), or dictionaries containing
    ``feature`` and ``descending``. Stable ties keep the baseline/earlier trial.
    ``trigger`` defaults to ``{"kind": "always"}``; conditional recipes use
    ``{"kind": "any-at-least", "thresholds": [{"feature": ..., "minimum": N}]}``.

    All budgets apply only to optional trials, never the baseline. The native
    solver retains exact rule/exception eligibility and validates actual basis
    ordinals/arity. Shift-excursion scores are heuristics, not physical bounds.
    This recipe, including its trigger, is part of generation checkpoint identity.
    """
    if not isinstance(alternatives, (list, tuple)):
        raise TypeError("alternatives must be a list or tuple")
    if not isinstance(quality, (list, tuple)):
        raise TypeError("quality must be a list or tuple")
    return _validated_portfolio({
        "kind": "bounded-portfolio", "version": 1,
        "alternatives": [json.loads(discovery_strategy(rows=value))["rows"]
                         if isinstance(value, str) else value for value in alternatives],
        "limits": {"max_depth": max_depth, "max_rows": max_rows,
                   "max_exact_trace_rows": max_exact_trace_rows,
                   "max_exact_trace_terms": max_exact_trace_terms},
        "quality": [{"feature": value, "descending": False}
                    if isinstance(value, str) else value for value in quality],
        "trigger": {"kind": "always"} if trigger is None else trigger,
    })


def _keys(value, expected, label):
    if not isinstance(value, dict) or set(value) != set(expected):
        raise ValueError(f"{label} requires exactly {', '.join(sorted(expected))}")


def _uint(value, maximum, label, minimum=0):
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"{label} must be an integer in [{minimum}, {maximum}]")


def _validated_portfolio(recipe):
    _keys(recipe, {"kind", "version", "alternatives", "limits", "quality", "trigger"}, "rule portfolio")
    if recipe["kind"] != "bounded-portfolio" or type(recipe["version"]) is not int or recipe["version"] != 1:
        raise ValueError("unsupported rule portfolio kind/quality version")
    alternatives = recipe["alternatives"]
    if not isinstance(alternatives, (list, tuple)) or not 1 <= len(alternatives) <= 2:
        raise ValueError("rule portfolio requires one or two alternatives")
    # Source inventory and materialized row permutations are native-authoritative.
    # Here reject non-descriptors; the shared native parser checks their fields.
    for alternative in alternatives:
        if not isinstance(alternative, dict) or alternative.get("kind") not in {
                "input-order", "features", "materialized"}:
            raise ValueError("alternative must be a native source-priority descriptor")
    limits = recipe["limits"]
    _keys(limits, {"max_depth", "max_rows", "max_exact_trace_rows", "max_exact_trace_terms"}, "trial limits")
    _uint(limits["max_depth"], (1 << 32) - 1, "max_depth")
    for name in ("max_rows", "max_exact_trace_rows", "max_exact_trace_terms"):
        _uint(limits[name], 2 * sys.maxsize + 1, name, 1)
    quality = recipe["quality"]
    if not isinstance(quality, (list, tuple)) or not 1 <= len(quality) <= 12:
        raise ValueError("quality requires one to twelve unique keys")
    seen = set()
    for priority in quality:
        _keys(priority, {"feature", "descending"}, "quality priority")
        feature = priority["feature"]
        if not isinstance(feature, str) or feature not in _QUALITY_FEATURES or feature in seen:
            raise ValueError("unknown or repeated quality feature")
        if type(priority["descending"]) is not bool:
            raise TypeError("quality descending must be bool")
        seen.add(feature)
    trigger = recipe["trigger"]
    if isinstance(trigger, dict) and trigger.get("kind") == "always":
        _keys(trigger, {"kind"}, "always trigger")
    else:
        _keys(trigger, {"kind", "thresholds"}, "conditional trigger")
        if trigger["kind"] != "any-at-least":
            raise ValueError("unknown portfolio trigger")
        thresholds = trigger["thresholds"]
        if not isinstance(thresholds, (list, tuple)) or not 1 <= len(thresholds) <= 12:
            raise ValueError("conditional trigger requires one to twelve unique thresholds")
        seen = set()
        for threshold in thresholds:
            _keys(threshold, {"feature", "minimum"}, "trigger threshold")
            feature = threshold["feature"]
            if not isinstance(feature, str) or feature not in _QUALITY_FEATURES or feature in seen:
                raise ValueError("unknown or repeated trigger feature")
            _uint(threshold["minimum"], (1 << 64) - 1, "trigger minimum")
            seen.add(feature)
    return copy.deepcopy(recipe)
