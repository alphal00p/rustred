"""Generate and apply the closing IBPs for the equal-mass sunset family."""

import tomllib

import rustred


def main() -> None:
    generated = rustred.generate_closing_artifact(
        family=rustred.ClosingFamily.UNIT_MASS_VACUUM_K3,
    )
    generation = tomllib.loads(generated.to_toml())
    assert generation["family_selector"] == "unit-mass-vacuum-k3"
    assert generation["validation"]["source_rows"] == 4
    assert generation["validation"]["guarded_rules"] == 5
    assert len(generation["rules"]) == 5

    inspection = rustred.inspect_closing_artifact(generated.artifact)
    assert inspection.status == "inspected"

    reduction = rustred.reduce_with_closing_artifact(
        generated.artifact,
        [2, 2, 1],
    )
    assert reduction.target_powers == [2, 2, 1]
    assert len(reduction.terms) == 2
    mass_powers = {
        tuple(term.master_powers): term.common_mass_squared_power
        for term in reduction.terms
    }
    assert mass_powers == {(1, 1, 1): -2, (0, 1, 1): -3}

    print(generated.to_toml(), end="")
    print(reduction.to_toml(), end="")


if __name__ == "__main__":
    main()
