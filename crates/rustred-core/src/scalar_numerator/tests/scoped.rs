use super::*;
use crate::sector::Mask;

#[test]
fn scalar_scope_checks_base_and_all_expanded_keys_before_reducer_application() {
    let artifact = derive_one_loop_unit_mass_tadpole()
        .unwrap()
        .with_total_excess_scope_for_test(1, BTreeMap::from([(Mask::try_new([true]).unwrap(), 5)]))
        .unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();
    let before = reducer.statistics();
    let head = dot_head();
    let momentum = symbol("scoped_k").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![momentum.clone()],
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        service.lower(&Atom::num(0), &IntegralKey::try_new([3]).unwrap()),
        Err(ScalarNumeratorError::OutsideCertifiedTotalExcessDomain { maximum: 1 })
    ));
    assert!(matches!(
        service.lower(&Atom::num(0), &IntegralKey::try_new([1, 1]).unwrap()),
        Err(ScalarNumeratorError::WrongIntegralKeyArity {
            expected: 1,
            actual: 2
        })
    ));

    let square = dot(head, momentum.clone(), momentum.clone());
    let base = IntegralKey::try_new([2]).unwrap();
    let invalid = square.clone().pow(Atom::num(4)); // Includes starting key [-2], E=2.
    let outcome = (|| -> Result<(), ScalarNumeratorError> {
        let lowering = service.lower(&invalid, &base)?;
        for term in lowering.terms() {
            reducer.reduce_unit_mass(term.integral()).unwrap();
        }
        Ok(())
    })();
    assert!(matches!(
        outcome,
        Err(ScalarNumeratorError::OutsideCertifiedTotalExcessDomain { maximum: 1 })
    ));
    assert_eq!(reducer.statistics(), before);

    let valid = square.pow(Atom::num(2)); // [0], [1], [2] all have E <= 1.
    let plain = derive_one_loop_unit_mass_tadpole().unwrap();
    let plain_service =
        ScalarNumeratorService::try_new(&plain, head, vec![momentum], Default::default()).unwrap();
    assert_eq!(
        service.lower(&valid, &base).unwrap().terms(),
        plain_service.lower(&valid, &base).unwrap().terms()
    );
    assert_eq!(reducer.statistics(), before);
}
