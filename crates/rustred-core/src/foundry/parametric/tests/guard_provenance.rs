use crate::identity::ParametricIbpGenerator;
use crate::sector::OrderingPolicy;

use super::super::prepare::prepare_problem;
use super::super::{
    ParametricGuardOrigin, ParametricRuleError, ParametricRuleLimits, derive_sector_interior_rule,
};
use super::support::{guarded_tadpole_family, sole_ordinary_relation};

#[test]
fn generated_source_conditions_keep_full_parametric_provenance() {
    let (base, family) = guarded_tadpole_family();
    let x = base.parameter("x").unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let relations = batch.complete(rows).unwrap().into_relations();
    let provenance_cells = relations[0]
        .nonzero_conditions()
        .iter()
        .flat_map(|condition| condition.sources())
        .map(|source| match source {
            crate::identity::IdentityConditionSource::RelationInputTermDenominator {
                shift,
                ..
            }
            | crate::identity::IdentityConditionSource::RelationCollectedTermDenominator {
                shift,
                ..
            } => shift.len(),
            crate::identity::IdentityConditionSource::RelationTranslation { offset, .. }
            | crate::identity::IdentityConditionSource::IndexTranslation { offset } => offset.len(),
            _ => 0,
        })
        .sum::<usize>();
    assert_eq!(provenance_cells, 3);
    let exact_limits = ParametricRuleLimits {
        max_guard_provenance_index_cells: provenance_cells,
        ..ParametricRuleLimits::default()
    };
    let rule = derive_sector_interior_rule(
        generator.context(),
        &relations,
        &[1],
        OrderingPolicy::default(),
        exact_limits,
    )
    .unwrap();

    let lifted_x = generator
        .context()
        .lift_base_polynomial(&x.numerator)
        .unwrap();
    assert!(rule.nonzero_guards().iter().any(|guard| {
        guard.polynomial() == &lifted_x
            && guard
                .origins()
                .iter()
                .any(|origin| matches!(origin, ParametricGuardOrigin::SourceCondition { .. }))
    }));
    let one_below = ParametricRuleLimits {
        max_guard_provenance_index_cells: provenance_cells - 1,
        ..exact_limits
    };
    assert_eq!(
        derive_sector_interior_rule(
            generator.context(),
            &relations,
            &[1],
            OrderingPolicy::default(),
            one_below,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "parametric guard provenance index cells",
            requested: provenance_cells,
            limit: provenance_cells - 1,
        })
    );
}

#[test]
fn duplicate_valued_source_denominator_origins_use_the_canonical_column_arc() {
    let (base, family) = guarded_tadpole_family();
    // Separate generators deliberately allocate pointer-distinct shift caches
    // while retaining identical family and indexed-context semantics.
    let first_generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let second_generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = first_generator.context().clone();
    let relations = vec![
        sole_ordinary_relation(&first_generator),
        sole_ordinary_relation(&second_generator),
    ];

    let first_shift = relations[0]
        .terms()
        .keys()
        .find(|shift| shift.values() == [1])
        .unwrap();
    let second_shift = relations[1]
        .terms()
        .keys()
        .find(|shift| shift.values() == [1])
        .unwrap();
    assert!(!std::ptr::eq(
        first_shift.values().as_ptr(),
        second_shift.values().as_ptr(),
    ));

    let x = base.parameter("x").unwrap();
    let expected_denominator = context.lift_base_polynomial(&x.numerator).unwrap();
    let first_coefficient = &relations[0].terms()[first_shift];
    let first_coefficient = context.bind_sealed(first_coefficient).unwrap();
    assert_eq!(
        context
            .denominator_condition_from_bound(first_coefficient)
            .unwrap(),
        expected_denominator,
    );

    let problem = prepare_problem(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let canonical = &problem
        .columns
        .iter()
        .find(|column| column.shift.values() == [1])
        .unwrap()
        .shift;
    let mut retained_origin_shifts = Vec::new();
    for (source_ordinal, source) in problem.sources.iter().enumerate() {
        let (guard_polynomial, origin_shift) = source
            .guards
            .iter()
            .find_map(|guard| match &guard.origin {
                ParametricGuardOrigin::SourceCoefficientDenominator {
                    source_ordinal: origin_source,
                    shift,
                    ..
                } if *origin_source == source_ordinal && shift.values() == [1] => {
                    Some((&guard.polynomial, shift))
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(guard_polynomial, &expected_denominator);
        assert!(std::ptr::eq(
            origin_shift.values().as_ptr(),
            canonical.values().as_ptr(),
        ));
        retained_origin_shifts.push(origin_shift);
    }
    assert_eq!(retained_origin_shifts.len(), 2);
    assert!(std::ptr::eq(
        retained_origin_shifts[0].values().as_ptr(),
        retained_origin_shifts[1].values().as_ptr(),
    ));
}
