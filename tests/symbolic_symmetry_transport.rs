use std::collections::BTreeMap;

use rustred::{
    AffineDenominator, Coefficient, CoefficientContext, ConcreteIntegralKey, GuardOrigin,
    IndexSpace, IntegralFamily, InternalSymmetrySearchLimits, ParametricCoefficientError,
    ParametricIbpGenerator, ParametricRelation, ParametricRowId,
    SYMBOLIC_SYMMETRY_ROW_TRANSPORT_V1_SCHEMA, SectorRestrictions,
    SymbolicSymmetryRowTransportCompiler, SymbolicSymmetryRowTransportError,
    SymbolicSymmetryRowTransportLimits, discover_bounded_vacuum_internal_symmetries,
};

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

#[test]
fn complete_row_transport_permutes_coefficients_and_shifts_together() {
    let family = equal_mass_sunset("symbolic-symmetry-row-transport");
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    let swap = report
        .symmetries()
        .iter()
        .find(|symmetry| symmetry.denominator_permutation() == [1, 0, 2])
        .unwrap();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let context = generated.context();
    let source = generated.ibp_li().next().unwrap();
    let limits = SymbolicSymmetryRowTransportLimits::default();

    let certificate =
        SymbolicSymmetryRowTransportCompiler::compile(&family, context, source, swap, limits)
            .unwrap();
    assert_eq!(
        certificate.schema(),
        SYMBOLIC_SYMMETRY_ROW_TRANSPORT_V1_SCHEMA
    );
    assert_eq!(certificate.stats().source_terms(), source.terms().len());
    assert_eq!(certificate.stats().output_terms(), source.terms().len());
    assert_eq!(
        certificate.symmetry_permutation(),
        swap.denominator_permutation()
    );
    assert_eq!(
        certificate.symmetry_map_guard_polynomials().len(),
        swap.affine_map().replay_guards().len()
    );

    let permutation = swap.denominator_permutation();
    for (source_shift, source_coefficient) in source.terms() {
        let mut target_shift = vec![0_i64; source.arity()];
        for (source_index, &target_index) in permutation.iter().enumerate() {
            target_shift[target_index] = source_shift.values()[source_index];
        }
        let expected = context
            .permute_indices(source_coefficient, permutation, limits.arithmetic)
            .unwrap();
        let actual = certificate
            .transported_relation()
            .terms()
            .iter()
            .find(|(shift, _)| shift.values() == target_shift)
            .map(|(_, coefficient)| coefficient)
            .unwrap();
        assert_eq!(actual, &expected);
    }
    certificate.replay(&family, context).unwrap();

    let mut output_limited = limits;
    output_limited.max_output_terms = source.terms().len() - 1;
    assert!(matches!(
        SymbolicSymmetryRowTransportCompiler::compile(
            &family,
            context,
            source,
            swap,
            output_limited,
        ),
        Err(SymbolicSymmetryRowTransportError::ResourceLimit {
            resource: "symbolic symmetry-transport output terms",
            ..
        })
    ));

    let mut manifest_limited = limits;
    manifest_limited.max_manifest_bytes = 0;
    assert!(matches!(
        SymbolicSymmetryRowTransportCompiler::compile(
            &family,
            context,
            source,
            swap,
            manifest_limited,
        ),
        Err(SymbolicSymmetryRowTransportError::ResourceLimit {
            resource: "symbolic symmetry-transport manifest bytes",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn specialization_commutes_with_verified_whole_identity_transport() {
    let family = equal_mass_sunset("symbolic-symmetry-specialization-covariance");
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    let cycle = report
        .symmetries()
        .iter()
        .find(|symmetry| symmetry.denominator_permutation() == [1, 2, 0])
        .unwrap();
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let context = generated.context();
    let source = generated.ibp_li().nth(2).unwrap();
    let limits = SymbolicSymmetryRowTransportLimits::default();
    let certificate =
        SymbolicSymmetryRowTransportCompiler::compile(&family, context, source, cycle, limits)
            .unwrap();

    let target_assignment = [5, 2, 4];
    let source_assignment = cycle
        .denominator_permutation()
        .iter()
        .map(|&target| target_assignment[target])
        .collect::<Vec<_>>();
    let raw_source = source
        .specialize(context, &source_assignment, limits.arithmetic)
        .unwrap();
    let raw_target = certificate
        .transported_relation()
        .specialize(context, &target_assignment, limits.arithmetic)
        .unwrap();

    let mut transported_source = BTreeMap::<ConcreteIntegralKey, Coefficient>::new();
    for (source_key, coefficient) in raw_source.terms() {
        let target_key = cycle.transport_source_key(source_key).unwrap();
        let previous = transported_source.insert(target_key, coefficient.clone());
        assert!(
            previous.is_none(),
            "a permutation must be injective on keys"
        );
    }
    assert_eq!(&transported_source, raw_target.terms());
    certificate.replay(&family, context).unwrap();
}

#[test]
fn coefficient_transport_rejects_non_bijections() {
    let family = equal_mass_sunset("symbolic-symmetry-invalid-permutation");
    let generated = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generated.context();
    let coefficient = context.index(0).unwrap();
    assert_eq!(
        context.permute_indices(
            &coefficient,
            &[0, 0, 2],
            SymbolicSymmetryRowTransportLimits::default().arithmetic,
        ),
        Err(ParametricCoefficientError::InvalidIndexPermutation)
    );
}

#[test]
fn transported_guards_retain_source_and_permutation_provenance() {
    let family = equal_mass_sunset("symbolic-symmetry-guard-provenance");
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    let swap = report
        .symmetries()
        .iter()
        .find(|symmetry| symmetry.denominator_permutation() == [1, 0, 2])
        .unwrap();
    let generated = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generated.context();
    let limits = SymbolicSymmetryRowTransportLimits::default();

    let divisor = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let guarded = context
        .checked_div_guarded(&context.one(), &divisor)
        .unwrap();
    let expected_guard = context
        .permute_nonzero_condition_indices(
            &guarded.nonzero[0],
            swap.denominator_permutation(),
            limits.arithmetic,
        )
        .unwrap();
    let source_id = ParametricRowId::Derived {
        label: "symbolic-symmetry-guard-source".into(),
    };
    let mut source = ParametricRelation::new(family.fingerprint(), source_id, context);
    source
        .add_guarded_term(context, IndexSpace::try_new(3).unwrap().zero(), guarded)
        .unwrap();

    let mut guard_limited = limits;
    guard_limited.max_output_guards = 0;
    assert!(matches!(
        SymbolicSymmetryRowTransportCompiler::compile(
            &family,
            context,
            &source,
            swap,
            guard_limited,
        ),
        Err(SymbolicSymmetryRowTransportError::ResourceLimit {
            resource: "symbolic symmetry-transport output guards",
            ..
        })
    ));

    let certificate =
        SymbolicSymmetryRowTransportCompiler::compile(&family, context, &source, swap, limits)
            .unwrap();
    let retained = certificate
        .transported_relation()
        .guarded_nonzero_conditions()
        .iter()
        .find(|condition| condition.polynomial() == expected_guard.polynomial())
        .expect("the pre-cancellation source denominator must survive transport");
    assert!(retained.origins().iter().any(|origin| matches!(
        origin,
        GuardOrigin::IndexPermutation { source_to_target }
            if source_to_target.as_ref() == swap.denominator_permutation()
    )));
    assert!(retained.origins().iter().any(|origin| matches!(
        origin,
        GuardOrigin::RelationIndexPermutation { source_to_target, .. }
            if source_to_target.as_ref() == swap.denominator_permutation()
    )));
    certificate.replay(&family, context).unwrap();
}
