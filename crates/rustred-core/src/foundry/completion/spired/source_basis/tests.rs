use symbolica::domains::SelfRing;

use crate::algebra::CoefficientPolynomial;
use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{SpiredCase, SpiredCoordinateFace, SpiredExecutionCase};
use super::{
    SpiredSourceBasisError, SpiredSourceBasisLimits, SpiredSourceBasisSpecializationPolicy,
    try_precondition_spired_ordinary_sources,
};

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn broad_case(
    artifact: &ClosedArtifact,
    completed: &CompletedIbpSourceRows,
) -> SpiredExecutionCase {
    let arity = artifact.family().denominators().len();
    let sector = Mask::try_new(vec![true; arity]).unwrap();
    let target = IntegralShift::try_new(vec![0_i64; arity]).unwrap();
    let structural_shifts = completed
        .relations()
        .iter()
        .flat_map(|source| source.terms().keys().map(|shift| shift.values()))
        .collect::<Vec<_>>();
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, target.values(), &structural_shifts)
            .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        arity,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        owners,
    )
    .unwrap()
}

fn assert_basis_shape(
    artifact: &ClosedArtifact,
    completed: &CompletedIbpSourceRows,
    translated_offsets: &[&[i64]],
) {
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let case = broad_case(artifact, completed);
    let basis = try_precondition_spired_ordinary_sources(
        generator.context(),
        completed,
        &case,
        SpiredSourceBasisLimits::default(),
    )
    .unwrap();

    assert_eq!(basis.ordering(), OrderingPolicy::default());
    assert_eq!(
        basis.sector(),
        case.stratum().domain().sector(),
        "the proof object must retain its sector-ordering scope"
    );
    assert!(!basis.rows().is_empty());
    assert!(basis.rows().len() <= completed.source_row_count());
    assert_eq!(
        basis.dropped_dependent_rows() + basis.rows().len(),
        completed.source_row_count()
    );
    assert_eq!(
        basis.specialization_policy(),
        SpiredSourceBasisSpecializationPolicy::RawOrdinaryFallbackRequired
    );
    assert!(basis.requires_raw_ordinary_fallback());

    let mut previous_pivot = None;
    for row in basis.rows() {
        assert!(previous_pivot.is_none_or(|previous| previous < row.pivot_column()));
        previous_pivot = Some(row.pivot_column());
        assert_eq!(
            row.terms()[0].shift().values(),
            basis.columns()[row.pivot_column()].values()
        );
        assert!(!row.pivot_coefficient().is_zero());
        assert!(row.terms().iter().all(|term| !term.coefficient().is_zero()));
        assert!(!row.provenance().is_empty());
        let native_coefficients = row
            .terms()
            .iter()
            .map(|term| term.coefficient().raw().clone())
            .chain(
                row.provenance()
                    .iter()
                    .map(|term| term.coefficient().raw().clone()),
            )
            .collect();
        assert!(
            CoefficientPolynomial::gcd_multiple(native_coefficients).is_one(),
            "the Symbolica multi-GCD must certify a primitive physical/provenance row"
        );
    }
    for source_ordinal in 0..completed.source_row_count() {
        assert!(
            basis
                .raw_reconstruction(source_ordinal)
                .is_some_and(|reconstruction| !reconstruction.is_empty())
        );
    }
    for &offset in translated_offsets {
        basis
            .try_verify_translated_provenance(
                generator.context(),
                completed,
                offset,
                SpiredSourceBasisLimits::default(),
            )
            .unwrap();
    }
}

#[test]
fn k1_basis_has_the_same_exact_translated_span_and_raw_fallback() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 1);
    assert_basis_shape(&artifact, &completed, &[&[0], &[3], &[-2]]);
}

#[test]
fn k3_basis_has_the_same_exact_translated_span_and_raw_fallback() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 4);
    assert_basis_shape(
        &artifact,
        &completed,
        &[&[0, 0, 0], &[1, -1, 2], &[-2, 1, 0]],
    );
}

#[test]
fn source_basis_limits_fail_before_unbounded_native_work() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = broad_case(&artifact, &completed);

    assert!(matches!(
        try_precondition_spired_ordinary_sources(
            generator.context(),
            &completed,
            &case,
            SpiredSourceBasisLimits::default().with_max_physical_columns(0),
        ),
        Err(SpiredSourceBasisError::ResourceLimit {
            resource: "source-preconditioner physical columns",
            limit: 0,
            ..
        })
    ));
    assert!(matches!(
        try_precondition_spired_ordinary_sources(
            generator.context(),
            &completed,
            &case,
            SpiredSourceBasisLimits::default().with_max_native_dense_entry_bound(0),
        ),
        Err(SpiredSourceBasisError::ResourceLimit {
            resource: "source-preconditioner native dense-entry bound",
            limit: 0,
            ..
        })
    ));
    assert!(matches!(
        try_precondition_spired_ordinary_sources(
            generator.context(),
            &completed,
            &case,
            SpiredSourceBasisLimits::default().with_max_replay_exact_operations(0),
        ),
        Err(SpiredSourceBasisError::ResourceLimit { limit: 0, .. })
    ));

    assert!(matches!(
        try_precondition_spired_ordinary_sources(
            generator.context(),
            &completed,
            &case,
            SpiredSourceBasisLimits::default().with_max_polynomial_operations(0),
        ),
        Err(SpiredSourceBasisError::ResourceLimit {
            resource: "source-preconditioner polynomial operations",
            limit: 0,
            ..
        })
    ));
}
