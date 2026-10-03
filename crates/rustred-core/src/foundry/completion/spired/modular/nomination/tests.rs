use super::*;
use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::{IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits};

fn family(name: &str, guarded: bool, rational_dimension: bool) -> IntegralFamily {
    let base =
        CoefficientContext::try_new(if guarded { vec!["d", "x"] } else { vec!["d"] }).unwrap();
    let d = base.parameter("d").unwrap();
    let dimension = if rational_dimension {
        base.try_div(&d, &base.integer(101), Default::default())
            .unwrap()
    } else {
        d
    };
    let power_shift = if guarded {
        base.try_div(
            &base.one(),
            &base.parameter("x").unwrap(),
            Default::default(),
        )
        .unwrap()
    } else {
        base.zero()
    };
    IntegralFamily::new(
        name,
        vec!["k".into()],
        vec![],
        base.clone(),
        dimension,
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        vec![],
        vec![power_shift],
    )
    .unwrap()
}
fn complete(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    prepared.complete(rows).unwrap()
}
fn request(offset: i64) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(0, IntegralShift::try_new([offset]).unwrap())
}
fn shift(value: i64) -> IndexShift {
    IndexShift::try_new([value], 1).unwrap()
}
fn support(outcome: Outcome) -> Support {
    match outcome {
        Outcome::TargetSupport(s) => s,
        other => panic!("expected nomination, got {other:?}"),
    }
}

#[test]
fn borrowed_corpus_reuses_primes_and_preserves_original_request_positions() {
    let f = family("nomination-primes", false, false);
    let generator = ParametricIbpGenerator::try_new(&f).unwrap();
    let completed = complete(&generator);
    let requests = [request(0), request(-1)];
    let forbidden = BTreeSet::from([shift(1)]);
    let target = shift(0);
    let prepared = PreparedOrdinaryNomination::try_new(
        generator.context(),
        &completed,
        &requests,
        &forbidden,
        &target,
        &[],
        Limits::default(),
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.corpus.sources(), &completed));
    let a = support(prepared.sample(101, &[7], &[3]).unwrap());
    let b = support(prepared.sample(103, &[11], &[4]).unwrap());
    assert_eq!(a.requests.as_ref(), &[request(-1)]);
    assert_eq!(a.input_ordinals.as_ref(), &[1]);
    assert_eq!(a.requests, b.requests);
    assert_eq!(a.stats.rows_evaluated, 2);
    assert_eq!(a.stats.registered_forbidden_columns, 1);
    assert_eq!(a.stats.forbidden_rank, 1);
    assert_eq!(a.stats.augmented_rank, 2);
}

#[test]
fn structural_zero_residue_is_registered_and_false_hit_fails_exact_complete_f() {
    use symbolica::{
        domains::{SelfRing, rational_polynomial::RationalPolynomialField},
        prelude::Z,
        tensors::sparse::{LuLMode, SparseRowReducer},
    };
    let f = family("nomination-rank-drop-hit", false, false);
    let generator = ParametricIbpGenerator::try_new(&f).unwrap();
    let completed = complete(&generator);
    let requests = [request(0)];
    let forbidden = BTreeSet::from([shift(1)]);
    let target = shift(0);
    let prepared = PreparedOrdinaryNomination::try_new(
        generator.context(),
        &completed,
        &requests,
        &forbidden,
        &target,
        &[],
        Limits::default(),
    )
    .unwrap();
    // At positive n=p the true coefficient -2n samples to zero, giving a false hit.
    let sampled = support(prepared.sample(101, &[7], &[101]).unwrap());
    assert_eq!(sampled.stats.registered_forbidden_columns, 1);
    assert_eq!(sampled.stats.forbidden_rank, 0);
    assert_eq!(sampled.stats.augmented_rank, 1);
    assert_eq!(sampled.input_ordinals.as_ref(), &[0]);
    // Reuse Symbolica for the unchanged exact [F,target] block. Its only pivot
    // is F, so no exact target exists in this nominated one-row subbank.
    let row = completed.source_relation(0).unwrap();
    let values = vec![
        row.terms()[&shift(1)].raw().clone(),
        row.terms()[&target].raw().clone(),
    ];
    let mut exact = SparseRowReducer::new(2, RationalPolynomialField::new(Z), LuLMode::Pattern);
    assert_eq!(exact.add_row(&values, &[0, 1]), Some(0));
    assert_eq!(exact.u().nrows(), 1);
}

#[test]
fn sampled_miss_does_not_mean_symbolic_miss_and_positive_source_replays() {
    use crate::foundry::artifact::{
        OriginalSourceCombinationRequest, OriginalSourceContribution,
        check_original_source_combination,
    };
    use crate::sector::{Mask, OrderingPolicy};
    let f = family("nomination-rank-drop-miss", false, false);
    let generator = ParametricIbpGenerator::try_new(&f).unwrap();
    let completed = complete(&generator);
    let requests = [request(-1)];
    let forbidden = BTreeSet::from([shift(1)]);
    let target = shift(0);
    let prepared = PreparedOrdinaryNomination::try_new(
        generator.context(),
        &completed,
        &requests,
        &forbidden,
        &target,
        &[],
        Limits::default(),
    )
    .unwrap();
    assert!(matches!(
        prepared.sample(101, &[7], &[102]).unwrap(),
        Outcome::SampledMiss(_)
    ));
    assert_eq!(
        support(prepared.sample(101, &[7], &[3]).unwrap())
            .input_ordinals
            .as_ref(),
        &[0]
    );
    let translated = generator
        .translate_selected_completed_source_rows(
            &completed,
            requests,
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let source = &translated.sources()[0];
    let c = generator.context();
    let weight = c.div(&c.one(), &source.terms()[&target]).unwrap();
    let rhs = source
        .terms()
        .iter()
        .filter(|(s, _)| **s != target)
        .map(|(s, v)| {
            (
                s.clone(),
                c.neg_with_limits(&c.mul(v, &weight).unwrap(), Default::default())
                    .unwrap(),
            )
        })
        .collect();
    let mask = Mask::try_new([true]).unwrap();
    let proof = check_original_source_combination(
        &f,
        OriginalSourceCombinationRequest {
            root_sector: mask.clone(),
            sector: mask,
            ordering: OrderingPolicy::SpiredUncutV1,
            lower: vec![1],
            upper: vec![None],
            fixed: vec![],
            contributions: vec![OriginalSourceContribution {
                source_row: source.provenance().source_row().clone(),
                offset: source.provenance().offset().clone(),
                weight,
            }],
            rhs,
            retained_conditions: vec![],
        },
        Default::default(),
    )
    .unwrap();
    assert!(proof.cells().count() > 0);
}

#[test]
fn singular_conditions_and_bad_prime_denominators_are_unlucky_not_misses() {
    for (guarded, rational_dimension) in [(true, false), (false, true)] {
        let f = family("nomination-singular", guarded, rational_dimension);
        let generator = ParametricIbpGenerator::try_new(&f).unwrap();
        let completed = complete(&generator);
        let requests = [request(0)];
        let target = shift(0);
        let forbidden = BTreeSet::new();
        let prepared = PreparedOrdinaryNomination::try_new(
            generator.context(),
            &completed,
            &requests,
            &forbidden,
            &target,
            &[],
            Limits::default(),
        )
        .unwrap();
        let base = if guarded { vec![7, 0] } else { vec![7] };
        let Outcome::UnluckySample {
            input_ordinal,
            cause,
            ..
        } = prepared.sample(101, &base, &[3]).unwrap()
        else {
            panic!("singular source sample was not classified as unlucky")
        };
        assert_eq!(input_ordinal, 0);
        if guarded {
            assert!(matches!(cause, EvaluationError::ConditionZero { .. }));
        } else {
            assert!(matches!(cause, EvaluationError::TermDenominatorZero { .. }));
        }
    }
}

#[test]
fn malformed_bank_context_fixed_values_and_resource_caps_refuse() {
    let f = family("nomination-refusals", false, false);
    let generator = ParametricIbpGenerator::try_new(&f).unwrap();
    let c = generator.context();
    let completed = complete(&generator);
    let requests = [request(0)];
    let target = shift(0);
    let forbidden = BTreeSet::from([shift(1)]);
    macro_rules! prepare {
        ($bank:expr, $fset:expr, $target:expr, $fixed:expr, $limits:expr) => {
            PreparedOrdinaryNomination::try_new(
                c, &completed, $bank, $fset, $target, $fixed, $limits,
            )
        };
    }
    assert!(
        prepare!(
            &[request(0), request(0)],
            &forbidden,
            &target,
            &[],
            Limits::default()
        )
        .is_err()
    );
    assert!(
        prepare!(
            &[request(i64::MAX)],
            &forbidden,
            &target,
            &[],
            Limits::default()
        )
        .is_err()
    );
    assert!(
        prepare!(
            &[TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([0, 0]).unwrap()
            )],
            &forbidden,
            &target,
            &[],
            Limits::default()
        )
        .is_err()
    );
    assert!(
        prepare!(
            &[TranslatedSourceRequest::new(
                9,
                IntegralShift::try_new([0]).unwrap()
            )],
            &forbidden,
            &target,
            &[],
            Limits::default()
        )
        .is_err()
    );
    assert!(
        prepare!(
            &requests,
            &BTreeSet::from([target.clone()]),
            &target,
            &[],
            Limits::default()
        )
        .is_err()
    );
    assert!(
        prepare!(
            &requests,
            &forbidden,
            &IndexShift::try_new([0, 0], 2).unwrap(),
            &[],
            Limits::default()
        )
        .is_err()
    );
    assert!(
        prepare!(
            &requests,
            &forbidden,
            &target,
            &[],
            Limits {
                max_plan_coordinate_cells: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    let mut limited = Limits::default();
    limited.corpus.max_terms_per_source = 1;
    assert!(prepare!(&requests, &forbidden, &target, &[], limited).is_err());
    let foreign = IndexedCoefficientContext::try_new(c.base(), "nomination-foreign", 1).unwrap();
    assert!(
        PreparedOrdinaryNomination::try_new(
            &foreign,
            &completed,
            &requests,
            &forbidden,
            &target,
            &[],
            Limits::default()
        )
        .is_err()
    );
    let fixed = [FixedIndexRestriction::new(0, 3)];
    let prepared = prepare!(&requests, &forbidden, &target, &fixed, Limits::default()).unwrap();
    assert!(prepared.sample(101, &[7], &[104]).is_err()); // Congruence is NOT fixed equality.
    for prime in [0, 2, 9] {
        assert!(prepared.sample(prime, &[7], &[3]).is_err());
    }
    assert!(prepared.sample(101, &[101], &[3]).is_err());
    let mut limited = Limits::default();
    limited.corpus.max_point_coordinates = 1;
    let prepared = prepare!(&requests, &forbidden, &target, &[], limited).unwrap();
    assert!(
        prepared
            .sample(101, &[7], &[3])
            .unwrap_err()
            .to_string()
            .contains("point coordinate")
    );
    let mut limited = Limits::default();
    limited.kernel.max_structural_terms_per_row = 0;
    let prepared = prepare!(&requests, &forbidden, &target, &[], limited).unwrap();
    assert!(
        prepared
            .sample(101, &[7], &[3])
            .unwrap_err()
            .to_string()
            .contains("F-term")
    );
    let mut limited = Limits::default();
    limited.kernel.max_trace_nodes = 0;
    let prepared = prepare!(&requests, &forbidden, &target, &[], limited).unwrap();
    assert!(matches!(
        prepared.sample(101, &[7], &[3]),
        Err(Error::Kernel(KernelError::ResourceLimit { .. }))
    ));
}
