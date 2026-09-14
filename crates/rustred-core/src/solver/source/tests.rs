use std::collections::BTreeMap;

use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

use super::*;

fn two_external_family(scaled: bool, zero_gram: bool) -> IntegralFamily {
    let base = CoefficientContext::new(["d", "s00", "s11", "c1", "c2", "a", "b", "nu1", "nu2"]);
    let parameter = |name| base.parameter(name).unwrap();
    IntegralFamily::new(
        "spired-li-source-fixture",
        vec!["k".into()],
        vec!["p0".into(), "p1".into()],
        base.clone(),
        parameter("d"),
        vec![
            AffineDenominator::new(base.zero(), vec![base.one(), base.zero(), base.zero()]),
            AffineDenominator::new(
                parameter("c1"),
                vec![
                    base.zero(),
                    if scaled { parameter("a") } else { base.one() },
                    base.zero(),
                ],
            ),
            AffineDenominator::new(
                parameter("c2"),
                vec![
                    base.zero(),
                    base.zero(),
                    if scaled { parameter("b") } else { base.one() },
                ],
            ),
        ],
        vec![
            vec![
                if zero_gram {
                    base.zero()
                } else {
                    parameter("s00")
                },
                base.zero(),
            ],
            vec![
                base.zero(),
                if zero_gram {
                    base.zero()
                } else {
                    parameter("s11")
                },
            ],
        ],
        vec![base.zero(), parameter("nu1"), parameter("nu2")],
    )
    .unwrap()
}

#[test]
fn two_external_li_matches_direct_external_derivative_with_native_denominator_clearing() {
    for scaled in [false, true] {
        let system = SourceSystem::<3>::from_family(&two_external_family(scaled, false)).unwrap();
        assert_eq!(system.rows.len(), 4);
        let prototype = &system.rows.iter().flatten().next().unwrap().coefficient;
        let variable = |position| {
            prototype
                .variable(&prototype.variables()[position])
                .unwrap()
        };
        let s00 = variable(1);
        let s11 = variable(2);
        let c1 = variable(3);
        let c2 = variable(4);
        let n1 = &variable(system.indices[1]) + &variable(7);
        let n2 = &variable(system.indices[2]) + &variable(8);
        let aa = if scaled {
            &variable(5) * &variable(5)
        } else {
            prototype.one()
        };
        let bb = if scaled {
            &variable(6) * &variable(6)
        } else {
            prototype.one()
        };
        // D0=k², D1=a k.p0+c1, D2=b k.p1+c2, p0.p1=0.
        // SpIRed contracts the external Lorentz generator with (p1,p0).
        // Its four coefficients are a/b * n1*s00*(c2-D2) and
        // b/a * n2*s11*(D1-c1); clearing their LCM a*b gives these rows.
        // Noninteger power offsets are included once, as in shiftNonIntPows.
        let expected = BTreeMap::from([
            (
                Integral::symbolic([0, 1, 0]).unwrap(),
                &(&(&aa * &c2) * &s00) * &n1,
            ),
            (
                Integral::symbolic([0, 1, -1]).unwrap(),
                -(&(&aa * &s00) * &n1),
            ),
            (
                Integral::symbolic([0, 0, 1]).unwrap(),
                -(&(&(&bb * &c1) * &s11) * &n2),
            ),
            (Integral::symbolic([0, -1, 1]).unwrap(), &(&bb * &s11) * &n2),
        ]);
        let actual: BTreeMap<_, _> = system.rows[3]
            .iter()
            .map(|term| (term.integral, term.coefficient.clone()))
            .collect();
        assert_eq!(actual, expected, "scaled={scaled}");
        assert_eq!(!system.conditions.is_empty(), scaled);
    }
}

#[test]
fn ordinary_and_li_source_order_matches_cpp_not_generic_generator_order() {
    let family = two_external_family(false, false);
    let system = SourceSystem::<3>::from_family(&family).unwrap();
    let ordinary_only = SourceSystem::<3>::from_family_with_lorentz(&family, false).unwrap();
    assert_eq!(ordinary_only.rows.len(), 3);
    assert_eq!(ordinary_only.rows, system.rows[..3]);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let generated = (0..batch.len())
        .map(|index| batch.generate(index))
        .collect();
    let completed = batch.complete(generated).unwrap();
    for (ported, generic) in [2, 1, 0].into_iter().enumerate() {
        assert_eq!(
            system.rows[ported],
            lower_relation::<3>(&completed.relations()[generic], &mut Vec::new()).unwrap()
        );
    }

    // Four external vectors distinguish reversed triangular traversal from
    // simply reversing the generic lexicographic pair vector.
    let mut identities = Vec::new();
    for contraction in 0..6 {
        for differentiated_loop in 0..2 {
            identities.push(RowId::OrdinaryIbp {
                contraction_momentum: contraction,
                differentiated_loop,
            });
        }
    }
    for first_external in 0..4 {
        for second_external in first_external + 1..4 {
            identities.push(RowId::LorentzInvariance {
                first_external,
                second_external,
            });
        }
    }
    identities.sort_unstable_by_key(|row| reference_source_order(row, 2, 4));
    let ordinary_expected: Vec<_> = (0..2)
        .flat_map(|differentiated_loop| {
            [5, 4, 3, 2, 0, 1].map(|contraction_momentum| RowId::OrdinaryIbp {
                contraction_momentum,
                differentiated_loop,
            })
        })
        .collect();
    assert_eq!(identities[..12], ordinary_expected);
    let pairs_expected = [(2, 3), (1, 3), (0, 3), (1, 2), (0, 2), (0, 1)].map(
        |(first_external, second_external)| RowId::LorentzInvariance {
            first_external,
            second_external,
        },
    );
    assert_eq!(identities[12..], pairs_expected);
}

#[test]
fn identically_zero_li_rows_keep_reference_source_ordinals() {
    let family = two_external_family(false, true);
    let system = SourceSystem::<3>::from_family(&family).unwrap();
    assert_eq!(system.rows.len(), 4);
    assert!(system.rows[3].is_empty());
    assert!(system.rows[..3].iter().any(|row| !row.is_empty()));
}

#[test]
fn vacuum_source_counts_are_unchanged_by_default_li_inclusion() {
    fn check<const N: usize>(family: IntegralFamily, count: usize) {
        let all = SourceSystem::<N>::from_family(&family).unwrap();
        let ordinary = SourceSystem::<N>::from_family_with_lorentz(&family, false).unwrap();
        assert_eq!(all.rows.len(), count);
        assert_eq!(all.rows, ordinary.rows);
    }
    check::<1>(crate::solver::tests::tadpole(), 1);
    check::<3>(crate::solver::tests::sunset(), 4);
    check::<6>(crate::solver::tests::vac3(), 9);
}
