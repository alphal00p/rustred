//! Independent adversarial checks of collection-only admission boundaries.
use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn sunset(name: &str) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                .into_iter()
                .map(|row| {
                    AffineDenominator::new(
                        context.integer(-1),
                        row.into_iter().map(|n| context.integer(n)).collect(),
                    )
                })
                .collect(),
            vec![],
            vec![context.zero(); 3],
        )
        .unwrap(),
    )
}

#[test]
fn equal_u_and_total_power_do_not_erase_distinct_dot_multisets() {
    // U is identical and both total powers are five. Neither the parameter
    // monomial nor its Gamma prefactors are fixed by those two facts alone.
    let first = IntegralKey::try_new([3, 1, 1]).unwrap();
    let second = IntegralKey::try_new([2, 2, 1]).unwrap();
    let plan = VacuumFamilyAliasPlan::prepare(
        &[
            (sunset("collection-audit-dot-a"), BTreeSet::from([first])),
            (sunset("collection-audit-dot-b"), BTreeSet::from([second])),
        ],
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.statistics().eligible_parametric, 2);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals().len(), 2);
    assert_eq!(plan.raw_terminals(), plan.canonical_terminals());
}

#[test]
fn external_family_is_retained_even_when_its_active_key_looks_like_a_vacuum() {
    let context = CoefficientContext::new(["d"]);
    let vacuum = Arc::new(
        IntegralFamily::new(
            "collection-audit-vacuum",
            vec!["k".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.one()],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    );
    let external = Arc::new(
        IntegralFamily::new(
            "collection-audit-external",
            vec!["k".into()],
            vec!["p".into()],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(context.integer(-1), vec![context.one(), context.zero()]),
                // (k+p)^2 - 1 with p^2=1. This inactive row still makes
                // the authenticated owner an external-momentum family.
                AffineDenominator::new(context.zero(), vec![context.one(), context.integer(2)]),
            ],
            vec![vec![context.one()]],
            vec![context.zero(); 2],
        )
        .unwrap(),
    );
    let vacuum_key = IntegralKey::try_new([1]).unwrap();
    let external_key = IntegralKey::try_new([1, 0]).unwrap();
    let plan = VacuumFamilyAliasPlan::prepare(
        &[
            (vacuum.clone(), BTreeSet::from([vacuum_key.clone()])),
            (external.clone(), BTreeSet::from([external_key.clone()])),
        ],
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.statistics().skipped[&Skip::ExternalMomenta], 1);
    assert_eq!(plan.statistics().eligible_parametric, 1);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.raw_terminals(), plan.canonical_terminals());
    assert_eq!(
        plan.representative(&vacuum, &vacuum_key)
            .unwrap()
            .family_fingerprint(),
        vacuum.fingerprint()
    );
    assert_eq!(
        plan.representative(&external, &external_key)
            .unwrap()
            .family_fingerprint(),
        external.fingerprint()
    );
}
