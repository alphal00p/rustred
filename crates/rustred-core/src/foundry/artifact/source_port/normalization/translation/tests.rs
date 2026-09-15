use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::source_port::certificate::OriginalSourceContribution;
use crate::solver::{CoordinateCase, SourceSystem};

use super::super::tests::{family, row_id, scaled_fixture};
use super::*;

fn replay(entries: impl IntoIterator<Item = (i64, Coefficient)>) -> OriginalSourceReplay<1> {
    OriginalSourceReplay {
        normalization: OriginalRowNormalization::OriginalGeneratorOrdinaryV1,
        contributions: entries
            .into_iter()
            .map(|(offset, weight)| OriginalSourceContribution {
                source_row: row_id(),
                offset: [offset],
                weight,
            })
            .collect(),
        source_conditions: Vec::new(),
    }
}

fn translate(
    corpus: &OriginalSourceCorpus,
    family: &IntegralFamily,
    replay: &OriginalSourceReplay<1>,
    fixed: &[FixedIndexRestriction],
) -> Result<TranslatedOriginalSources, SourcePortAuditError> {
    corpus.translate_normalized(
        &ParametricIbpGenerator::try_new(family).unwrap(),
        replay,
        fixed,
        Default::default(),
        Default::default(),
    )
}

#[test]
fn canonical_join_preserves_full_rows_and_wide_offsets() {
    let family = family(false);
    let system = SourceSystem::<1>::from_family(&family).unwrap();
    let corpus = OriginalSourceCorpus::try_new(&family, &system, &[row_id()]).unwrap();
    let context = corpus.context();
    let mut input = replay([
        (4096, context.integer(2).raw().clone()),
        (-3, context.one().raw().clone()),
        (4096, context.integer(3).raw().clone()),
        (0, context.integer(7).raw().clone()),
        (0, context.integer(-7).raw().clone()),
    ]);
    let result = translate(&corpus, &family, &input, &[]).unwrap();
    assert_eq!(result.sources.len(), 2);
    assert_eq!(
        result.sources.provenance()[0]
            .translated()
            .offset()
            .values(),
        [-3]
    );
    assert_eq!(
        result.sources.provenance()[1]
            .translated()
            .offset()
            .values(),
        [4096]
    );
    assert_eq!(result.contributions[0].2, context.one());
    assert_eq!(result.contributions[1].2, context.integer(5));
    for (ordinal, row_id, _) in &result.contributions {
        assert_eq!(row_id, result.sources.relations()[*ordinal].row_id());
        assert_eq!(
            row_id,
            result.sources.provenance()[*ordinal]
                .translated()
                .source_row()
        );
        assert_eq!(
            result.sources.relations()[*ordinal].terms().len(),
            corpus.completed.relations()[0].terms().len()
        );
    }
    input.contributions.reverse();
    let reversed = translate(&corpus, &family, &input, &[]).unwrap();
    assert_eq!(result.contributions, reversed.contributions);
    assert_eq!(result.sources.provenance(), reversed.sources.provenance());
    assert_eq!(result.sources.relations(), reversed.sources.relations());
}

#[test]
fn fixed_weight_specialization_does_not_specialize_original_source_rows() {
    let family = family(false);
    let (corpus, _) = scaled_fixture();
    let input = OriginalSourceReplay::retain_checked(
        vec![(row_id(), [2])],
        vec![corpus.context().one().raw().clone()],
    )
    .unwrap();
    let normalized = corpus
        .normalize(input, &CoordinateCase::generic().into())
        .unwrap();
    let result = translate(
        &corpus,
        &family,
        &normalized,
        &[FixedIndexRestriction::new(0, 3)],
    )
    .unwrap();
    assert_eq!(result.contributions[0].2, corpus.context().integer(6));
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let expected = generator
        .translate_selected_completed_source_rows(
            &corpus.completed,
            [TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([2]).unwrap(),
            )],
            Default::default(),
        )
        .unwrap();
    assert_eq!(
        result.sources.relations()[0].terms(),
        expected.sources()[0].terms()
    );
    assert!(
        result.sources.relations()[0]
            .terms()
            .values()
            .any(|c| !c.raw().numerator.is_constant())
    );
    assert_eq!(
        result.sources.provenance()[0]
            .translated()
            .offset()
            .values(),
        [2]
    );
}

#[test]
fn weight_poles_survive_join_cancellation_and_fail_before_fixed_pole_cancellation() {
    let family = family(false);
    let system = SourceSystem::<1>::from_family(&family).unwrap();
    let corpus = OriginalSourceCorpus::try_new(&family, &system, &[row_id()]).unwrap();
    let context = corpus.context();
    let denominator = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let positive = context.div(&context.one(), &denominator).unwrap();
    let negative = context.sub(&context.zero(), &positive).unwrap();
    let mut input = replay([
        (0, positive.raw().clone()),
        (0, negative.raw().clone()),
        (1, context.one().raw().clone()),
    ]);
    input
        .source_conditions
        .push(denominator.raw().numerator.clone());
    let result = translate(&corpus, &family, &input, &[]).unwrap();
    assert_eq!(result.sources.len(), 1);
    assert_eq!(
        result.sources.provenance()[0]
            .translated()
            .offset()
            .values(),
        [1]
    );
    assert_eq!(result.weight_conditions.len(), 1);
    assert_eq!(
        result.weight_conditions[0].raw(),
        &denominator.raw().numerator
    );
    assert_eq!(
        input.source_conditions,
        vec![denominator.raw().numerator.clone()]
    );
    assert_eq!(input.contributions.len(), 3);
    assert!(
        translate(
            &corpus,
            &family,
            &input,
            &[FixedIndexRestriction::new(0, 1)]
        )
        .is_err()
    );
    let regular = translate(
        &corpus,
        &family,
        &input,
        &[FixedIndexRestriction::new(0, 2)],
    )
    .unwrap();
    assert_eq!(regular.sources.len(), 1);
    assert!(regular.weight_conditions.is_empty());
    let guard_budget = RuleCellLimits {
        max_guards: 0,
        ..Default::default()
    };
    assert!(
        corpus
            .translate_normalized(
                &ParametricIbpGenerator::try_new(&family).unwrap(),
                &input,
                &[],
                Default::default(),
                guard_budget
            )
            .is_err()
    );
}

#[test]
fn invalid_normalization_rows_context_fixed_faces_and_empty_support_fail_closed() {
    let family = family(false);
    let system = SourceSystem::<1>::from_family(&family).unwrap();
    let corpus = OriginalSourceCorpus::try_new(&family, &system, &[row_id()]).unwrap();
    let context = corpus.context();
    let mut input = replay([(0, context.one().raw().clone())]);
    input.normalization = OriginalRowNormalization::NativeDenominatorClearedOrdinaryV1;
    assert!(translate(&corpus, &family, &input, &[]).is_err());
    input.normalization = OriginalRowNormalization::OriginalGeneratorOrdinaryV1;
    input.contributions[0].source_row = RowId::Derived {
        label: "unknown".into(),
    };
    assert!(translate(&corpus, &family, &input, &[]).is_err());
    input.contributions[0].source_row = row_id();
    input.contributions[0].weight = CoefficientContext::new(["foreign"]).one();
    assert!(translate(&corpus, &family, &input, &[]).is_err());
    input.contributions[0].weight = context.one().raw().clone();
    assert!(
        translate(
            &corpus,
            &family,
            &input,
            &[FixedIndexRestriction::new(1, 2)]
        )
        .is_err()
    );
    assert!(
        translate(
            &corpus,
            &family,
            &input,
            &[
                FixedIndexRestriction::new(0, 2),
                FixedIndexRestriction::new(0, 2)
            ]
        )
        .is_err()
    );
    let other = super::super::tests::family(true);
    assert!(translate(&corpus, &other, &input, &[]).is_err());
    let empty = replay([
        (0, context.one().raw().clone()),
        (0, context.integer(-1).raw().clone()),
    ]);
    assert!(translate(&corpus, &family, &empty, &[]).is_err());
    assert!(translate(&corpus, &family, &replay([]), &[]).is_err());
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    assert!(
        corpus
            .translate_normalized(
                &generator,
                &input,
                &[],
                TranslatedSourceLimits {
                    max_requested_source_translations: 0,
                    ..Default::default()
                },
                Default::default()
            )
            .is_err()
    );
    assert!(
        corpus
            .translate_normalized(
                &generator,
                &input,
                &[],
                Default::default(),
                RuleCellLimits {
                    max_source_views: 0,
                    ..Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn fixed_zero_weight_keeps_its_parameter_pole_and_leaves_surviving_source_unmodified() {
    let family = family(false);
    let system = SourceSystem::<1>::from_family(&family).unwrap();
    let corpus = OriginalSourceCorpus::try_new(&family, &system, &[row_id()]).unwrap();
    let context = corpus.context();
    let parameter = context
        .parse_expression_with_limits("s", Default::default())
        .unwrap();
    let numerator = context
        .sub(&context.index(0).unwrap(), &context.integer(2))
        .unwrap();
    let weight = context.div(&numerator, &parameter).unwrap();
    let input = replay([(0, weight.raw().clone()), (1, context.one().raw().clone())]);
    let result = translate(
        &corpus,
        &family,
        &input,
        &[FixedIndexRestriction::new(0, 2)],
    )
    .unwrap();
    assert_eq!(result.sources.len(), 1);
    assert_eq!(
        result.sources.provenance()[0]
            .translated()
            .offset()
            .values(),
        [1]
    );
    assert_eq!(result.weight_conditions.len(), 1);
    assert_eq!(
        result.weight_conditions[0].raw(),
        &parameter.raw().numerator
    );
    assert!(
        result.sources.relations()[0]
            .terms()
            .values()
            .any(|c| !c.raw().numerator.is_constant())
    );
    assert_eq!(input.contributions.len(), 2);
}

#[test]
fn completed_source_scope_is_checked_not_only_equal_arity_and_variable_map() {
    let family = family(false);
    let system = SourceSystem::<1>::from_family(&family).unwrap();
    let mut corpus = OriginalSourceCorpus::try_new(&family, &system, &[row_id()]).unwrap();
    let input = replay([(0, corpus.context().one().raw().clone())]);
    corpus
        .completed
        .replace_family_fingerprint_for_test("unrelated-original-family");
    assert!(translate(&corpus, &family, &input, &[]).is_err());
}

#[test]
fn row_id_join_uses_generator_chronology_not_adapter_order() {
    let context = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "translated-original-sunset",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
            .into_iter()
            .map(|row| {
                AffineDenominator::new(
                    context.integer(-1),
                    row.into_iter().map(|v| context.integer(v)).collect(),
                )
            })
            .collect(),
        Vec::new(),
        vec![context.zero(); 3],
    )
    .unwrap();
    let system = SourceSystem::<3>::from_family(&family).unwrap();
    let ids: Vec<_> = (0..2)
        .flat_map(|differentiated_loop| {
            (0..2).map(move |contraction_momentum| RowId::OrdinaryIbp {
                differentiated_loop,
                contraction_momentum,
            })
        })
        .collect();
    let corpus = OriginalSourceCorpus::try_new(&family, &system, &ids).unwrap();
    let context = corpus.context();
    let input = OriginalSourceReplay {
        normalization: OriginalRowNormalization::OriginalGeneratorOrdinaryV1,
        contributions: ids
            .iter()
            .enumerate()
            .rev()
            .map(|(axis, id)| OriginalSourceContribution {
                source_row: id.clone(),
                offset: [0; 3],
                weight: context.integer(axis as i64 + 1).raw().clone(),
            })
            .collect(),
        source_conditions: Vec::new(),
    };
    let result = corpus
        .translate_normalized(
            &ParametricIbpGenerator::try_new(&family).unwrap(),
            &input,
            &[],
            Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(result.sources.len(), 4);
    for (ordinal, actual_id, weight) in result.contributions {
        let relation = &corpus.completed.relations()[ordinal];
        assert_eq!(&actual_id, relation.row_id());
        let adapter_ordinal = ids.iter().position(|id| id == &actual_id).unwrap();
        assert_eq!(weight, context.integer(adapter_ordinal as i64 + 1));
        assert_eq!(
            result.sources.provenance()[ordinal]
                .translated()
                .source_ordinal(),
            ordinal
        );
        assert_eq!(
            result.sources.relations()[ordinal].terms(),
            relation.terms()
        );
    }
}
