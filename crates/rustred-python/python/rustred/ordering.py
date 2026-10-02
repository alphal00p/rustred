"""Declarative mathematical integral orders, compiled and validated in Rust.

Unlike discovery strategies, this changes which integrals are simpler and is
persisted in generated rule bundles. The returned JSON can also be written to
a file for ``rustred family-candidates --integral-order``. No Python callback
executes in elimination or rule application.
"""

import json


def integral_order(arity, *, support_weights=None, support_priority=None,
                   pre_support_degree_rows=None, degree_rows=None, coordinate_priority=None,
                   coordinate_groups="active-first",
                   active_direction="descending", inactive_direction="descending"):
    """Build a version-1 uncut order descriptor without rebuilding RustRed.

    Optional ``pre_support_degree_rows`` compare weighted absolute physical
    powers before support count. For example, one all-ones row compares
    sum(abs(n_i)) globally, including across pinches. The default empty prefix
    retains the support-first order. ``degree_rows`` is a sequence of mappings
    with ``active`` and ``inactive`` unsigned coefficient vectors. Their
    weighted excess degrees are compared lexicographically after support.
    Every coordinate of each sign must occur positively in some prefix or
    suffix row; native admission enforces
    this and all resource/overflow bounds. Final coordinate ties lie within
    finite degree fibres, so either tie direction is lawful.

    Defaults reproduce the uncut SpIRed mathematical comparison: total
    excess, numerator excess, then reversed active/inactive coordinate ties.
    The full descriptor still has its own persisted identity. A custom first
    row need not bound unweighted total excess, so its use in the optional
    total-excess certificate can be rejected even when reduction is valid.
    """
    if type(arity) is not int or arity <= 0:
        raise ValueError("arity must be a positive integer")

    def vector(value, default, name, permutation=False):
        result = list(default if value is None else value)
        if len(result) != arity or any(type(x) is not int for x in result):
            raise ValueError(f"{name} must contain one integer per coordinate")
        if permutation:
            if sorted(result) != list(range(arity)):
                raise ValueError(f"{name} must be a coordinate permutation")
        elif any(x < 0 or x > (1 << 64) - 1 for x in result):
            raise ValueError(f"{name} must contain unsigned 64-bit integers")
        return result

    if coordinate_groups not in {"active-first", "inactive-first", "interleaved"}:
        raise ValueError("unknown coordinate grouping")
    if active_direction not in {"ascending", "descending"} or inactive_direction not in {
            "ascending", "descending"}:
        raise ValueError("unknown coordinate direction")
    if degree_rows is None:
        degree_rows = [{"active": [1] * arity, "inactive": [1] * arity},
                       {"active": [0] * arity, "inactive": [1] * arity}]
    def degree_vectors(values):
        rows = []
        for row in values:
            if not isinstance(row, dict) or set(row) != {"active", "inactive"}:
                raise ValueError("each degree row requires only active and inactive vectors")
            rows.append({sign: vector(row[sign], None, f"degree {sign}")
                         for sign in ("active", "inactive")})
        return rows

    rows = degree_vectors(degree_rows)
    prefix = degree_vectors([] if pre_support_degree_rows is None else pre_support_degree_rows)
    if not rows and not prefix:
        raise ValueError("at least one degree row is required")
    result = {"version": 1,
        "support_weights": vector(support_weights, [0] * arity, "support_weights"),
        "support_priority": vector(support_priority, range(arity), "support_priority", True),
        "degree_rows": rows,
        "coordinate_priority": vector(coordinate_priority, range(arity), "coordinate_priority", True),
        "coordinate_groups": coordinate_groups,
        "active_direction": active_direction, "inactive_direction": inactive_direction}
    # Omit the empty prefix so existing data-only controls are unchanged.
    if prefix:
        result["pre_support_degree_rows"] = prefix
    return json.dumps(result, sort_keys=True, separators=(",", ":"))
