//! Independent test of the original target boundary after auxiliary aliasing.
use super::*;
use crate::family::AffineDenominator;

#[test]
fn free_requested_class_does_not_promote_its_auxiliary_alias_representative() {
    let context = CoefficientContext::new(["d"]);
    let family = Arc::new(
        IntegralFamily::new(
            "diagonal-audit-auxiliary-representative",
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
    );
    let requested = IntegralKey::try_new([2, 1, 1]).unwrap();
    let auxiliary = IntegralKey::try_new([1, 1, 2]).unwrap();
    let plan = VacuumDiagonalCollectionPlan::prepare(
        &[(family.clone(), BTreeSet::from([requested.clone()]))],
        Default::default(),
    )
    .unwrap();

    // The unrequested corner and its other dotted children enter the source
    // space. Full-U canonicalization prefers the generated [1,1,2], but that
    // may not replace the public name of the sole requested free class.
    let canonical = plan.aliases().representative(&family, &requested).unwrap();
    assert_eq!(canonical.integral(), &auxiliary);
    assert!(!plan.raw_terminals().contains(canonical));
    assert!(plan.statistics().auxiliary_columns > 0);
    assert!(plan.equations().is_empty());
    assert_eq!(plan.remaining_terminals(), plan.raw_terminals());
    let reduction = plan.apply(&family, &requested).unwrap();
    assert_eq!(reduction.terms().len(), 1);
    let (output, coefficient) = reduction.terms().first_key_value().unwrap();
    assert_eq!(output.integral(), &requested);
    assert_eq!(output.family_fingerprint(), family.fingerprint());
    assert_eq!(coefficient, &context.one());
    assert_eq!(
        plan.common_mass_squared_power(&family, &requested, output)
            .unwrap(),
        0
    );
    assert!(matches!(
        plan.apply(&family, &auxiliary),
        Err(VacuumCollectionError::UndeclaredTerminal)
    ));
}
