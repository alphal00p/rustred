"""Generate and apply the closing IBPs for the equal-mass tadpole family."""

import tomllib

import rustred


def main() -> None:
    generated = rustred.generate_closing_artifact(
        family=rustred.ClosingFamily.UNIT_MASS_VACUUM_K1,
    )
    generation = tomllib.loads(generated.to_toml())
    assert generation["family_selector"] == "unit-mass-vacuum-k1"
    assert generation["validation"]["source_rows"] == 1
    assert generation["validation"]["guarded_rules"] == 1
    assert len(generation["rules"]) == 1

    inspection = rustred.inspect_closing_artifact(generated.artifact)
    assert inspection.status == "inspected"

    reduction = rustred.reduce_with_closing_artifact(
        generated.artifact,
        [3],
    )
    assert reduction.target_powers == [3]
    assert len(reduction.terms) == 1
    term = reduction.terms[0]
    assert term.master_powers == [1]
    assert term.common_mass_squared_power == -2

    print(generated.to_toml(), end="")
    print(reduction.to_toml(), end="")


if __name__ == "__main__":
    main()
