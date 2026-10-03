use super::super::lower_cuts::LowerCuts;
use super::*;

fn old_rule_shifted(disposition: OwnerDomainMatchDisposition) -> OwnerDomainMatchDisposition {
    match disposition {
        OwnerDomainMatchDisposition::SelectedRule { batch, rule } => {
            OwnerDomainMatchDisposition::SelectedRule {
                batch,
                rule: rule + 1,
            }
        }
        other => other,
    }
}

#[test]
fn native_lower_cut_predicates_encode_active_inactive_and_multi_axis_or() {
    let base = rustred::algebra::CoefficientContext::try_new(["d"]).unwrap();
    let c = IndexedCoefficientContext::try_new(&base, "lower-cut-predicates", 2).unwrap();
    for lower in [0u64, 1, 2, 3] {
        let cuts = LowerCuts::new(
            &[lower, lower],
            &[None, None],
            &[None; 2],
            &[true, false],
            6,
        )
        .unwrap();
        let polynomials = cuts
            .polynomials(&c, &[true, false], Default::default())
            .unwrap();
        assert_eq!(polynomials.len(), 2 * lower as usize);
        for active in 1..=5 {
            for inactive in -4..=0 {
                let excluded = polynomials.iter().any(|p| {
                    c.specialize_fixed_polynomial(
                        p,
                        &[(0, active), (1, inactive)],
                        Default::default(),
                    )
                    .unwrap()
                    .is_zero()
                });
                assert_eq!(
                    excluded,
                    active - 1 < lower as i64 || -inactive < lower as i64
                );
            }
        }
        // Old lo0/1 native expression and append ordering are unchanged.
        if lower == 1 {
            for (axis, value) in [(0, 1), (1, 0)] {
                let old = c
                    .numerator_condition_with_limits(
                        &c.sub(&c.index(axis).unwrap(), &c.integer(value)).unwrap(),
                        Default::default(),
                    )
                    .unwrap();
                assert_eq!(polynomials[axis], old);
            }
        }
    }
}

#[test]
fn lower_cut_preflight_refuses_counts_overflow_finite_upper_and_charges_old_exclusions() {
    assert!(LowerCuts::new(&[2, 3], &[None, None], &[None; 2], &[true, false], 4).is_err());
    assert!(LowerCuts::new(&[u64::MAX], &[None], &[None], &[true], usize::MAX).is_err());
    assert!(LowerCuts::new(&[u64::MAX], &[None], &[None], &[false], usize::MAX).is_err());
    assert!(LowerCuts::new(&[1], &[Some(10)], &[None], &[true], 10).is_err());
    let fixed = LowerCuts::new(&[9], &[Some(9)], &[Some(10)], &[true], 0).unwrap();
    fixed.check_existing(&[], 0).unwrap();
    let (_, family) = base();
    let (c, _) = request(&family);
    let polynomial = c
        .numerator_condition_with_limits(&c.index(0).unwrap(), Default::default())
        .unwrap()
        .raw()
        .clone();
    let cuts = LowerCuts::new(&[2], &[None], &[None], &[true], 3).unwrap();
    cuts.check_existing(&[vec![polynomial.clone()]], 3).unwrap();
    assert!(
        cuts.check_existing(&[vec![polynomial.clone(), polynomial]], 3)
            .is_err()
    );
    assert!(cuts.check_existing(&[vec![], vec![]], 3).is_err());
}

#[test]
fn checked_lower_two_and_three_preserve_source_guards_and_old_fallback() {
    let (bytes, family) = base();
    let old = load(&bytes);
    for lower in [2u64, 3] {
        for policy in [
            RuleDispatchPolicy::Partition,
            RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
        ] {
            let (_, mut proposal) = request(&family);
            proposal.lower[0] = lower;
            let export = encode_checked_priority_owner_with_policy::<1>(
                &bytes,
                proposal,
                Default::default(),
                Default::default(),
                policy,
            )
            .unwrap();
            assert_eq!(
                export.proof().requested_bounds(),
                (&[lower][..], &[None][..])
            );
            assert!(
                export
                    .proof()
                    .cells()
                    .any(|cell| !cell.rule().nonzero_guards().is_empty())
            );
            let new = load(export.bytes());
            for power in 1..=lower {
                assert_eq!(
                    disposition(&new, power),
                    old_rule_shifted(disposition(&old, power))
                );
            }
            for power in [lower + 1, lower + 2, 30] {
                assert_eq!(
                    disposition(&new, power),
                    OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
                );
            }
            let old_records = codec::read(&bytes, Default::default()).unwrap();
            let new_records = codec::read(export.bytes(), Default::default()).unwrap();
            assert_eq!(
                &new_records.sectors[0].rules[1..],
                old_records.sectors[0].rules.as_slice()
            );
            assert_eq!(
                new_records.sectors[0].finite_residuals,
                old_records.sectors[0].finite_residuals
            );
        }
    }
}

#[test]
fn whole_piece_lower_cut_does_not_split_a_crossing_baseline_piece() {
    let (bytes, family) = base();
    let (_, mut proposal) = request(&family);
    proposal.lower[0] = 2;
    let export = encode_checked_priority_owner_with_policy::<1>(
        &bytes,
        proposal,
        Default::default(),
        Default::default(),
        RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
    )
    .unwrap();
    let matches = |programs: &CandidateOwnerPrograms<1>, lower| {
        let mut pieces = Vec::new();
        programs
            .visit_owner_domain_matches(
                [true],
                &[lower],
                &[Some(5)],
                Some(0),
                Default::default(),
                &AtomicBool::new(false),
                |piece| {
                    pieces.push((
                        piece.lower().to_vec(),
                        piece.upper().to_vec(),
                        piece.disposition(),
                    ));
                    ControlFlow::Continue(())
                },
            )
            .unwrap();
        pieces
    };
    let expected = matches(&load(&bytes), 1)
        .into_iter()
        .map(|(lo, hi, d)| (lo, hi, old_rule_shifted(d)))
        .collect::<Vec<_>>();
    let actual = matches(&load(export.bytes()), 1);
    assert_eq!(actual, expected);
    assert!(
        actual
            .iter()
            .all(|(_, _, d)| *d != OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 })
    );
    assert_eq!(
        matches(&load(export.bytes()), 2),
        vec![(
            vec![2],
            vec![Some(5)],
            OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
        )]
    );
}

#[test]
fn lower_cut_does_not_discharge_unrepresented_guards_or_widen_a_fixed_face() {
    let (bytes, family) = base();
    let (c, mut proposal) = request(&family);
    proposal.lower[0] = 2;
    // n-2 is excluded by this lower cut, but the unchanged runtime guard
    // admission does not use boundary predicates as a new implication oracle.
    proposal.retained_conditions.push(
        c.numerator_condition_with_limits(
            &c.sub(&c.index(0).unwrap(), &c.integer(2)).unwrap(),
            Default::default(),
        )
        .unwrap(),
    );
    let error = encode_checked_priority_owner::<1>(
        &bytes,
        proposal,
        Default::default(),
        Default::default(),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("proof guard is not retained"),
        "{error}"
    );
    let (_, mut proposal) = request(&family);
    proposal.lower[0] = 2;
    proposal.fixed = vec![FixedIndexRestriction::new(0, 3)];
    assert!(
        encode_checked_priority_owner::<1>(
            &bytes,
            proposal,
            Default::default(),
            Default::default()
        )
        .is_err()
    );
}
