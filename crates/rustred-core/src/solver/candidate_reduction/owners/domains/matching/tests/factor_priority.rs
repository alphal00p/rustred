use super::*;

#[test]
fn factor_guard_box_original_denominator_proof_keeps_priority_and_concrete_parity() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let positive = c
        .sub(
            &c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.integer(2),
        )
        .unwrap();
    let denominator = c
        .mul(&c.sub(&c.index(0).unwrap(), &c.one()).unwrap(), &positive)
        .unwrap();
    let mut first = rule(0);
    // Even a zero RHS retains its original denominator obligation.
    first.rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: poly(&c, denominator),
    });
    batch(&mut p).rules = vec![first, rule(1)];
    let pieces = collect(&p, &[3, 1, 0], &[None, None, Some(0)], Some(0));
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].lower(), &[3, 1, 0]);
    assert_eq!(pieces[0].upper(), &[None, None, Some(0)]);
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    for x in 3..8 {
        for y in 1..6 {
            assert_eq!(
                at(&pieces, [x, y, 0]),
                concrete(
                    &p,
                    IntegralKey::try_new([x as i64 + 1, y as i64 + 1, 0]).unwrap()
                )
            );
        }
    }
    // The univariate factor genuinely vanishes at n0=1. Fixed native
    // specialization must reject rule0 and retain the later stored rule.
    let zero_face = collect(&p, &[0, 1, 0], &[Some(0), None, Some(0)], Some(0));
    assert_eq!(zero_face.len(), 1);
    assert_eq!(
        zero_face[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 }
    );
    for y in 1..6 {
        assert_eq!(
            at(&zero_face, [0, y, 0]),
            concrete(&p, IntegralKey::try_new([1, y as i64 + 1, 0]).unwrap())
        );
    }
}
