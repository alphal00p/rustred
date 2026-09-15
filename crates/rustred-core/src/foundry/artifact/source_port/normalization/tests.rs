use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

use super::*;

pub(super) fn row_id() -> RowId {
    RowId::OrdinaryIbp {
        contraction_momentum: 0,
        differentiated_loop: 0,
    }
}

pub(super) fn family(symbolic_slope: bool) -> IntegralFamily {
    family_order(symbolic_slope, false)
}

fn family_order(symbolic_slope: bool, reversed: bool) -> IntegralFamily {
    let context = if reversed {
        CoefficientContext::new(["s", "d"])
    } else {
        CoefficientContext::new(["d", "s"])
    };
    IntegralFamily::new(
        "source-port-normalization-tadpole",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![if symbolic_slope {
                context.parameter("s").unwrap()
            } else {
                context.one()
            }],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

pub(super) fn scaled_fixture() -> (OriginalSourceCorpus, SourceSystem<1>) {
    let family = family(false);
    let original = SourceSystem::<1>::from_family(&family).unwrap();
    let template = &original.rows()[0][0].coefficient;
    let index = template
        .variable(&template.variables()[original.index_variables()[0]])
        .unwrap();
    let multiplier = &index + &template.one();
    let scaled_rows = original
        .rows()
        .iter()
        .map(|row| {
            row.iter()
                .map(|term| crate::solver::Term {
                    integral: term.integral,
                    coefficient: &term.coefficient * &multiplier,
                })
                .collect()
        })
        .collect();
    let scaled = SourceSystem::new(scaled_rows, *original.index_variables()).unwrap();
    let corpus = OriginalSourceCorpus::try_new(&family, &scaled, &[row_id()]).unwrap();
    (corpus, scaled)
}

fn request(corpus: &OriginalSourceCorpus, offset: i64) -> OriginalSourceReplay<1> {
    OriginalSourceReplay::retain_checked(
        vec![(row_id(), [offset])],
        vec![corpus.context.one().raw().clone()],
    )
    .unwrap()
}

#[test]
fn generator_scale_is_proved_against_every_original_coefficient() {
    let (corpus, scaled) = scaled_fixture();
    let original = &corpus.rows[&row_id()];
    let relation = &corpus.completed.relations()[original.ordinal];
    let expected = corpus
        .context
        .add(&corpus.context.index(0).unwrap(), &corpus.context.one())
        .unwrap();
    assert_eq!(original.scale.raw(), &expected.raw().numerator);
    let mut wrong = scaled.rows()[0].clone();
    wrong[0].coefficient = wrong[0].coefficient.clone().mul_coeff(2.into());
    assert!(checked_scale(&corpus.context, relation, &wrong).is_err());
    wrong = scaled.rows()[0].clone();
    wrong[0].integral = crate::solver::Integral::symbolic([7]).unwrap();
    assert!(checked_scale(&corpus.context, relation, &wrong).is_err());
    wrong = scaled.rows()[0].clone();
    wrong[0].coefficient = CoefficientContext::new(["foreign"]).one().numerator;
    assert!(checked_scale(&corpus.context, relation, &wrong).is_err());
    assert!(checked_scale::<1>(&corpus.context, relation, &[]).is_err());
}

#[test]
fn source_scale_translation_precedes_fixed_target_specialization() {
    let (corpus, _) = scaled_fixture();
    let replay = corpus
        .normalize(
            request(&corpus, 2),
            &CoordinateCase::new([Some(3)]).unwrap(),
        )
        .unwrap();
    assert_eq!(
        replay.normalization,
        OriginalRowNormalization::OriginalGeneratorOrdinaryV1
    );
    // Scale n+1 at original source n=3+2 is6, not the unshifted value4.
    assert_eq!(
        replay.contributions[0].weight,
        corpus.context.integer(6).raw().clone()
    );
    assert_eq!(replay.contributions[0].offset, [2]);
    assert!(
        corpus
            .normalize(replay, &CoordinateCase::new([Some(3)]).unwrap())
            .is_err()
    );
}

#[test]
fn retained_translation_is_i64_not_the_compact_source_power_encoding() {
    let (corpus, _) = scaled_fixture();
    let replay = corpus
        .normalize(request(&corpus, 4096), &CoordinateCase::generic())
        .unwrap();
    let expected = corpus
        .context
        .add(
            &corpus.context.index(0).unwrap(),
            &corpus.context.integer(4097),
        )
        .unwrap();
    assert_eq!(replay.contributions[0].weight, expected.raw().clone());
    let fixed = corpus
        .normalize(
            request(&corpus, i64::MAX),
            &CoordinateCase::new([Some(1)]).unwrap(),
        )
        .unwrap();
    let expected = corpus.context.integer(i64::MAX).raw() + corpus.context.integer(2).raw();
    assert_eq!(fixed.contributions[0].weight, expected);
}

#[test]
fn unknown_row_ids_and_foreign_weight_maps_are_rejected() {
    let (corpus, _) = scaled_fixture();
    let mut unknown = request(&corpus, 0);
    unknown.contributions[0].source_row = RowId::OrdinaryIbp {
        contraction_momentum: 1,
        differentiated_loop: 0,
    };
    assert!(
        corpus
            .normalize(unknown, &CoordinateCase::generic())
            .is_err()
    );
    let mut foreign = request(&corpus, 0);
    foreign.contributions[0].weight = CoefficientContext::new(["foreign_weight"]).one();
    assert!(
        corpus
            .normalize(foreign, &CoordinateCase::generic())
            .is_err()
    );
    let family = family(false);
    let source = SourceSystem::<1>::from_family(&family).unwrap();
    assert!(OriginalSourceCorpus::try_new(&family, &source, &[]).is_err());
    assert!(
        OriginalSourceCorpus::try_new(
            &family,
            &source,
            &[RowId::Derived {
                label: "invented".into()
            }]
        )
        .is_err()
    );
    let wrong_indices = SourceSystem::new(source.rows().to_vec(), [0]).unwrap();
    assert!(OriginalSourceCorpus::try_new(&family, &wrong_indices, &[row_id()]).is_err());
}

#[test]
fn exchanged_same_size_ordinary_rows_do_not_forge_row_id_binding() {
    let context = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "normalization-row-id-sunset",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
            .into_iter()
            .map(|row| {
                AffineDenominator::new(
                    context.integer(-1),
                    row.into_iter()
                        .map(|value| context.integer(value))
                        .collect(),
                )
            })
            .collect(),
        Vec::new(),
        vec![context.zero(); 3],
    )
    .unwrap();
    let source = SourceSystem::<3>::from_family(&family).unwrap();
    let mut ids = (0..2)
        .flat_map(|differentiated_loop| {
            (0..2).map(move |contraction_momentum| RowId::OrdinaryIbp {
                contraction_momentum,
                differentiated_loop,
            })
        })
        .collect::<Vec<_>>();
    OriginalSourceCorpus::try_new(&family, &source, &ids).unwrap();
    assert_eq!(source.rows()[1].len(), source.rows()[2].len());
    ids.swap(1, 2);
    assert!(OriginalSourceCorpus::try_new(&family, &source, &ids).is_err());
}

#[test]
fn original_parameter_conditions_survive_normalization_without_integer_branching() {
    let family = family(true);
    let source = SourceSystem::<1>::from_family(&family).unwrap();
    let corpus = OriginalSourceCorpus::try_new(&family, &source, &[row_id()]).unwrap();
    let replay = corpus
        .normalize(request(&corpus, 0), &CoordinateCase::generic())
        .unwrap();
    let expected = corpus.completed.relations()[corpus.rows[&row_id()].ordinal]
        .nonzero_conditions()
        .iter()
        .map(|condition| condition.polynomial().raw())
        .filter(|polynomial| !polynomial.is_constant())
        .collect::<Vec<_>>();
    assert!(
        !expected.is_empty(),
        "fixture must expose a pre-cancellation parameter condition"
    );
    assert_eq!(replay.source_conditions.len(), expected.len());
    for condition in expected {
        assert!(replay.source_conditions.contains(condition));
    }
}

#[test]
fn parameter_registration_order_does_not_change_original_coordinate_binding() {
    let family = family_order(true, true);
    let source = SourceSystem::<1>::from_family(&family).unwrap();
    let corpus = OriginalSourceCorpus::try_new(&family, &source, &[row_id()]).unwrap();
    assert_eq!(corpus.context.base().parameter_names(), ["s", "d"]);
    assert_eq!(*source.index_variables(), [2]);
    let replay = corpus
        .normalize(
            request(&corpus, 4),
            &CoordinateCase::new([Some(1)]).unwrap(),
        )
        .unwrap();
    assert_eq!(
        replay.normalization,
        OriginalRowNormalization::OriginalGeneratorOrdinaryV1
    );
    assert!(!replay.contributions.is_empty());
}

#[test]
fn translated_source_condition_zero_is_rejected_before_cancellation() {
    let (corpus, _) = scaled_fixture();
    let index = corpus.context.index(0).unwrap();
    let condition = corpus
        .context
        .sub(&index, &corpus.context.integer(3))
        .unwrap();
    let condition = corpus
        .context
        .admit_native_polynomial_result_with_limits(
            condition.raw().numerator.clone(),
            Default::default(),
        )
        .unwrap();
    assert!(
        corpus
            .condition_for_target(&condition, &[2], &[(0, 1)])
            .is_err()
    );
    // At source argument1+1 the same condition is-1, not zero.
    let restricted = corpus
        .condition_for_target(&condition, &[1], &[(0, 1)])
        .unwrap();
    assert_eq!(
        restricted.raw(),
        &corpus.context.integer(-1).raw().numerator
    );
}
