"""Generate the parametric IBPs for the two-loop equal-mass vacuum family."""

import tomllib

import rustred


FAMILY = r"""
I(
  name(equal_mass_sunset),
  loops(k1,k2),
  externals(),
  parameters(d,m2),
  dimension(d),
  prop(D1,k1^2-m2,1),
  prop(D2,k2^2-m2,1),
  prop(D3,(k1+k2)^2-m2,1)
)
"""

EXPECTED_ROWS = [
    "ordinary-ibp:0:0",
    "ordinary-ibp:0:1",
    "ordinary-ibp:1:0",
    "ordinary-ibp:1:1",
]


def main() -> None:
    result = rustred.derive(
        FAMILY,
        input_format=rustred.InputFormat.SYMBOLICA,
        relations=rustred.RelationSelection.ORDINARY,
        n_cores=1,
    )
    document = tomllib.loads(result.to_toml())

    assert document["schema"] == "rustred.derive-output.toml.v1"
    counts = document["relation_counts"]
    assert counts["generated_ordinary"] == 4
    assert counts["generated_li"] == 0
    assert counts["emitted_total"] == 4
    assert [row["stable_id"] for row in document["relations"]] == EXPECTED_ROWS

    print(result.to_toml(), end="")


if __name__ == "__main__":
    main()
