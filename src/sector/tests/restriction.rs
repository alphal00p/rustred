use super::super::{CutConstraint, Error, Mask, Pattern, PatternSlot, Restrictions};

#[test]
fn cuts_and_patterns_are_exclusions_never_zero_proofs() {
    let restrictions = Restrictions::try_new(
        CutConstraint::try_from_positions(4, [0, 2]).unwrap(),
        Pattern::try_new([
            PatternSlot::Any,
            PatternSlot::Inactive,
            PatternSlot::Any,
            PatternSlot::Active,
        ])
        .unwrap(),
    )
    .unwrap();

    for bits in 0_u8..16 {
        let sector =
            Mask::try_new((0..4).map(|position| bits & (1 << (3 - position)) != 0)).unwrap();
        let expected_admissible = sector.active_bits()[0]
            && sector.active_bits()[2]
            && !sector.active_bits()[1]
            && sector.active_bits()[3];
        let exclusion = restrictions.exclusion(&sector).unwrap();
        if expected_admissible {
            assert_eq!(exclusion, None);
        } else {
            let exclusion = exclusion.expect("inadmissible sectors carry exclusion evidence");
            assert!(
                !exclusion.missing_required_active().is_empty()
                    || !exclusion.pattern_mismatches().is_empty()
            );
        }
    }

    assert_eq!(restrictions.cuts().to_bit_string(), "1010");
    assert_eq!(restrictions.pattern().to_stable_string(), "*0*1");
    assert!(matches!(
        CutConstraint::try_from_positions(4, [1, 1]),
        Err(Error::DuplicateIndex { position: 1 })
    ));
}
