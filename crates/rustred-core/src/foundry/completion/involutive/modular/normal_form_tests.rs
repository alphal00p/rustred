use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
use crate::foundry::completion::CompletionGeometryLimits;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::limits::InvolutiveWorkBudget;
use super::super::normal_form::{JanetNormalForm, try_janet_normal_form};
use super::super::{
    ForwardShift, InvolutiveLimits, JanetBasisEpoch, OrdinaryChartLiftLimits, OreConsequence,
    OreOrderingAdapter, OreRow, try_lift_completed_ordinary_sources,
};
use super::normal_form::{ModularFrozenNormalFormProblem, ModularNormalFormProposal};
use super::ore::ModularOreRow;
use super::work::ModularNormalFormWork;
use super::{ModularCoefficientDag, ModularGuideError, ModularGuideLimits, ModularProbe};

const PRIME_A: u64 = 998_244_353;
const PRIME_B: u64 = 1_000_000_007;
const PRIME_C: u64 = 1_000_000_009;

fn context(scope: &str, arity: usize) -> IndexedCoefficientContext {
    let base = CoefficientContext::new(std::iter::empty::<&str>());
    IndexedCoefficientContext::try_new(&base, scope, arity).unwrap()
}

fn ordering(
    sector: impl IntoIterator<Item = bool>,
    limits: InvolutiveLimits,
) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new(sector).unwrap(),
        limits,
    )
    .unwrap()
}

fn shift(values: impl IntoIterator<Item = u64>, limits: InvolutiveLimits) -> ForwardShift {
    ForwardShift::try_new(values, limits).unwrap()
}

fn consequence(
    source_ordinal: usize,
    terms: impl IntoIterator<Item = (ForwardShift, crate::algebra::IndexedCoefficient)>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> OreConsequence {
    let row = OreRow::try_new(ordering, terms, context, limits).unwrap();
    OreConsequence::try_from_source(source_ordinal, row, ordering, context, limits).unwrap()
}

fn epoch(
    consequences: impl IntoIterator<Item = OreConsequence>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> JanetBasisEpoch {
    JanetBasisEpoch::try_initial(
        consequences,
        ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap()
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn assert_trace_matches_exact(
    proposal: &ModularNormalFormProposal,
    exact: &JanetNormalForm,
    basis: &JanetBasisEpoch,
) {
    assert_eq!(proposal.trace().steps().len(), exact.steps().len());
    for (modular, exact) in proposal.trace().steps().iter().zip(exact.steps()) {
        assert_eq!(modular.target_shift(), exact.target_shift());
        assert_eq!(modular.divisor_ordinal(), exact.divisor_ordinal());
        assert_eq!(modular.operator_shift(), exact.operator_shift());
        assert_eq!(
            modular.divisor_leading_shift(),
            basis.elements()[exact.divisor_ordinal()].leading_shift(),
        );
    }
}

fn sampled_exact_consequence(
    exact: &OreConsequence,
    context: &IndexedCoefficientContext,
    modulus: u64,
    point: &[i64],
) -> (Box<[ForwardShift]>, Box<[u64]>) {
    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(context, limits).unwrap();
    let mut work = ModularNormalFormWork::default();
    let row = ModularOreRow::try_from_exact(exact, &mut dag, context, &mut work, limits).unwrap();
    let mut probe = ModularProbe::try_new(&dag, context, 0, modulus, point, limits).unwrap();
    row.try_require_guards(&dag, &mut probe).unwrap();
    let support = row
        .try_sampled_support(&dag, &mut probe, &mut work, limits)
        .unwrap();
    let shifts = support.try_shifts(&row).unwrap();
    let residues = support.residues().to_vec().into_boxed_slice();
    (shifts, residues)
}

#[test]
fn modular_monic_normalization_matches_the_exact_projective_row() {
    let exact_limits = InvolutiveLimits::default();
    let context = context("modular-monic-differential", 1);
    let ordering = ordering([true], exact_limits);
    let zero = shift([0], exact_limits);
    let e = shift([1], exact_limits);
    let n = context.index(0).unwrap();
    let source = consequence(
        0,
        [(e.clone(), n), (zero.clone(), context.one())],
        &ordering,
        &context,
        exact_limits,
    );
    let mut exact_work = InvolutiveWorkBudget::default();
    let exact_monic = source
        .try_monic_copy_sealed(&ordering, &context, exact_limits, &mut exact_work)
        .unwrap()
        .unwrap();

    let limits = ModularGuideLimits::default();
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let mut work = ModularNormalFormWork::default();
    let row =
        ModularOreRow::try_from_exact(&source, &mut dag, &context, &mut work, limits).unwrap();
    let mut probe = ModularProbe::try_new(&dag, &context, 0, PRIME_A, &[2], limits).unwrap();
    let (row, leader) = row
        .try_monic(&ordering, &mut dag, &mut probe, &mut work, limits)
        .unwrap();
    assert_eq!(leader.as_ref(), Some(&e));
    let one = dag.one();
    assert_eq!(row.coefficient(&e), Some(&one));
    assert_eq!(row.guards().len(), 1);
    let support = row
        .try_sampled_support(&dag, &mut probe, &mut work, limits)
        .unwrap();
    let actual_shifts = support.try_shifts(&row).unwrap();
    let (expected_shifts, expected_residues) =
        sampled_exact_consequence(&exact_monic, &context, PRIME_A, &[2]);
    assert_eq!(actual_shifts, expected_shifts);
    assert_eq!(support.residues(), expected_residues.as_ref());

    // At n=0 the exact structural leader n E vanishes. A modular lane must
    // reject it instead of silently promoting the constant tail to leader.
    let mut dag = ModularCoefficientDag::try_new(&context, limits).unwrap();
    let mut work = ModularNormalFormWork::default();
    let source_row =
        ModularOreRow::try_from_exact(&source, &mut dag, &context, &mut work, limits).unwrap();
    let mut sampled_support_probe =
        ModularProbe::try_new(&dag, &context, 1, PRIME_A, &[0], limits).unwrap();
    let sampled_support = source_row
        .try_sampled_support(&dag, &mut sampled_support_probe, &mut work, limits)
        .unwrap();
    assert_eq!(source_row.terms().len(), 2);
    assert_eq!(
        sampled_support.try_shifts(&source_row).unwrap().as_ref(),
        std::slice::from_ref(&zero),
    );
    let sampled_zero_row = source_row.try_copy(&mut work, limits).unwrap();
    let mut rejected_probe =
        ModularProbe::try_new(&dag, &context, 2, PRIME_A, &[0], limits).unwrap();
    assert_eq!(
        sampled_zero_row.try_monic(&ordering, &mut dag, &mut rejected_probe, &mut work, limits,),
        Err(ModularGuideError::SampledZeroMonicLeader),
    );
    assert!(rejected_probe.is_rejected());

    // The active Ore translate has leader (n+1) E^2, so the same base point
    // legitimately unmasks the pivot. This guards against pruning or
    // selecting support before applying the coefficient automorphism.
    let zero_exact = OreConsequence::try_zero(&ordering, &context, exact_limits).unwrap();
    let zero_row =
        ModularOreRow::try_from_exact(&zero_exact, &mut dag, &context, &mut work, limits).unwrap();
    let mut translated_probe =
        ModularProbe::try_new(&dag, &context, 3, PRIME_A, &[0], limits).unwrap();
    let one = dag.one();
    let translated = zero_row
        .try_left_axpy(
            &one,
            &e,
            &source_row,
            &ordering,
            &mut dag,
            &mut translated_probe,
            exact_limits,
            &mut work,
            limits,
        )
        .unwrap();
    let (translated, translated_leader) = translated
        .try_monic(
            &ordering,
            &mut dag,
            &mut translated_probe,
            &mut work,
            limits,
        )
        .unwrap();
    let e2 = shift([2], exact_limits);
    assert_eq!(translated_leader.as_ref(), Some(&e2));
    assert_eq!(translated.coefficient(&e2), Some(&dag.one()));
}

#[test]
fn live_row_limit_is_enforced_before_import_and_on_each_merge_push() {
    let exact_limits = InvolutiveLimits::default();
    let context = context("modular-live-row-boundary", 2);
    let ordering = ordering([true, true], exact_limits);
    let left_exact = consequence(
        0,
        [
            (shift([0, 0], exact_limits), context.one()),
            (shift([2, 0], exact_limits), context.one()),
        ],
        &ordering,
        &context,
        exact_limits,
    );
    let right_exact = consequence(
        1,
        [
            (shift([0, 1], exact_limits), context.one()),
            (shift([2, 1], exact_limits), context.one()),
        ],
        &ordering,
        &context,
        exact_limits,
    );

    let too_small = ModularGuideLimits {
        max_live_row_terms: 1,
        ..ModularGuideLimits::default()
    };
    let mut dag = ModularCoefficientDag::try_new(&context, too_small).unwrap();
    let mut work = ModularNormalFormWork::default();
    assert_eq!(
        ModularOreRow::try_from_exact(&left_exact, &mut dag, &context, &mut work, too_small,),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular live row terms",
            requested: 2,
            limit: 1,
        }),
    );

    let capped = ModularGuideLimits {
        max_live_row_terms: 3,
        ..ModularGuideLimits::default()
    };
    let mut dag = ModularCoefficientDag::try_new(&context, capped).unwrap();
    let mut work = ModularNormalFormWork::default();
    let left =
        ModularOreRow::try_from_exact(&left_exact, &mut dag, &context, &mut work, capped).unwrap();
    let right =
        ModularOreRow::try_from_exact(&right_exact, &mut dag, &context, &mut work, capped).unwrap();
    let mut probe = ModularProbe::try_new(&dag, &context, 0, PRIME_A, &[1, 1], capped).unwrap();
    let one = dag.one();
    let zero = shift([0, 0], exact_limits);
    assert_eq!(
        left.try_left_axpy(
            &one,
            &zero,
            &right,
            &ordering,
            &mut dag,
            &mut probe,
            exact_limits,
            &mut work,
            capped,
        ),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular live row terms",
            requested: 4,
            limit: 3,
        }),
    );

    let exact_cap = ModularGuideLimits {
        max_live_row_terms: 4,
        ..ModularGuideLimits::default()
    };
    let mut dag = ModularCoefficientDag::try_new(&context, exact_cap).unwrap();
    let mut work = ModularNormalFormWork::default();
    let left = ModularOreRow::try_from_exact(&left_exact, &mut dag, &context, &mut work, exact_cap)
        .unwrap();
    let right =
        ModularOreRow::try_from_exact(&right_exact, &mut dag, &context, &mut work, exact_cap)
            .unwrap();
    let mut probe = ModularProbe::try_new(&dag, &context, 1, PRIME_A, &[1, 1], exact_cap).unwrap();
    let merged = left
        .try_left_axpy(
            &dag.one(),
            &zero,
            &right,
            &ordering,
            &mut dag,
            &mut probe,
            exact_limits,
            &mut work,
            exact_cap,
        )
        .unwrap();
    assert_eq!(merged.terms().len(), 4);
}

#[test]
fn synthetic_nonconstant_trace_matches_exact_replay_across_independent_probes() {
    let exact_limits = InvolutiveLimits::default();
    let context = context("modular-normal-form-synthetic", 1);
    let ordering = ordering([true], exact_limits);
    let zero = shift([0], exact_limits);
    let e = shift([1], exact_limits);
    let e2 = shift([2], exact_limits);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();

    // Initial Janet normalization turns n E + 1 into the exact monic row
    // E + 1/n and freezes the matching nonzero localization witness.
    let basis = epoch(
        [consequence(
            0,
            [(e.clone(), n), (zero.clone(), context.one())],
            &ordering,
            &context,
            exact_limits,
        )],
        &ordering,
        &context,
        exact_limits,
    );
    let subject = consequence(
        1,
        [
            (e2, n_plus_one.clone()),
            (e, n_plus_one),
            (zero, context.one()),
        ],
        &ordering,
        &context,
        exact_limits,
    );
    let mut problem = ModularFrozenNormalFormProblem::try_new(
        &subject,
        basis.division(),
        None,
        &ordering,
        &context,
        exact_limits,
        ModularGuideLimits::default(),
    )
    .unwrap();
    let foreign_problem = ModularFrozenNormalFormProblem::try_new(
        &subject,
        basis.division(),
        None,
        &ordering,
        &context,
        exact_limits,
        ModularGuideLimits::default(),
    )
    .unwrap();
    let exact = try_janet_normal_form(subject, &basis, &ordering, &context, exact_limits).unwrap();
    assert!(exact.is_zero());

    let mut previous = None;
    for (ordinal, modulus, point) in [(0, PRIME_A, [2]), (1, PRIME_B, [5]), (2, PRIME_C, [11])] {
        let proposal = problem.try_probe(ordinal, modulus, &point).unwrap();
        assert!(problem.owns(&proposal));
        assert!(!foreign_problem.owns(&proposal));
        assert_trace_matches_exact(&proposal, &exact, &basis);
        assert_eq!(
            proposal.trace().sampled_start_leader(),
            Some(&shift([2], exact_limits))
        );
        assert_eq!(proposal.trace().steps().len(), 2);
        assert!(proposal.trace().sampled_remainder_leader().is_none());
        assert!(proposal.trace().sampled_remainder_support().is_empty());
        assert_eq!(
            proposal
                .nonzero_evidence()
                .sampled_start_support_residues()
                .len(),
            proposal.trace().sampled_start_support().len(),
        );
        assert_eq!(
            proposal.nonzero_evidence().step_target_residues().len(),
            proposal.trace().steps().len(),
        );
        assert!(
            proposal
                .nonzero_evidence()
                .step_target_residues()
                .iter()
                .all(|&residue| residue != 0),
        );
        assert_eq!(proposal.census().normal_form_steps(), 2);
        assert!(proposal.census().divisor_index_query_operations() > 0);
        if let Some(previous) = &previous {
            assert_eq!(proposal.trace(), previous);
        }
        previous = Some(proposal.trace().clone());
    }

    // A repeated probe sees the same field-independent root: temporary DAG
    // nodes and translations from the previous lane were rolled back.
    let first = problem.try_probe(7, PRIME_A, &[13]).unwrap();
    let second = problem.try_probe(8, PRIME_A, &[13]).unwrap();
    assert_eq!(first.trace(), second.trace());
    assert_eq!(first.census(), second.census());
}

#[test]
fn one_loop_latent_sampled_zero_survives_active_and_inactive_ore_translation() {
    let exact_limits = InvolutiveLimits::default();
    for (scope, active) in [
        ("modular-one-loop-latent-active", true),
        ("modular-one-loop-latent-inactive", false),
    ] {
        let context = context(scope, 1);
        let ordering = ordering([active], exact_limits);
        let zero = shift([0], exact_limits);
        let e = shift([1], exact_limits);
        let e2 = shift([2], exact_limits);
        let n = context.index(0).unwrap();
        let basis = epoch(
            [consequence(
                0,
                [(e.clone(), context.one()), (zero, n)],
                &ordering,
                &context,
                exact_limits,
            )],
            &ordering,
            &context,
            exact_limits,
        );
        let subject = consequence(1, [(e2, context.one())], &ordering, &context, exact_limits);
        let mut problem = ModularFrozenNormalFormProblem::try_new(
            &subject,
            basis.division(),
            None,
            &ordering,
            &context,
            exact_limits,
            ModularGuideLimits::default(),
        )
        .unwrap();
        let exact =
            try_janet_normal_form(subject, &basis, &ordering, &context, exact_limits).unwrap();

        // The exact remainder is n(n+1) or n(n-1), hence structurally
        // nonzero. At n=0 its image vanishes, which remains inconclusive.
        assert!(!exact.is_zero());
        assert_eq!(exact.steps().len(), 2);
        let proposal = problem.try_probe(0, PRIME_A, &[0]).unwrap();
        assert_trace_matches_exact(&proposal, &exact, &basis);
        assert_eq!(proposal.trace().steps().len(), 2);
        assert!(proposal.trace().sampled_remainder_support().is_empty());
        assert!(
            proposal
                .nonzero_evidence()
                .sampled_remainder_residues()
                .is_empty(),
        );
        assert!(proposal.census().sampled_zero_observations() >= 1);

        // If the initially sampled-zero n tail had been pruned before the
        // Ore shift, the first cancellation would falsely end the trace.
        assert_eq!(proposal.trace().steps()[0].operator_shift(), &e);
        assert_eq!(
            proposal.trace().steps()[1].operator_shift(),
            &shift([0], exact_limits)
        );
    }
}

#[test]
fn two_loop_k3_shaped_mixed_sector_trace_matches_exact_remainder_images() {
    let exact_limits = InvolutiveLimits::default();
    let context = context("modular-normal-form-k3", 3);
    let ordering = ordering([true, false, true], exact_limits);
    let zero = shift([0, 0, 0], exact_limits);
    let axes = [
        shift([1, 0, 0], exact_limits),
        shift([0, 1, 0], exact_limits),
        shift([0, 0, 1], exact_limits),
    ];
    let squares = [
        shift([2, 0, 0], exact_limits),
        shift([0, 2, 0], exact_limits),
        shift([0, 0, 2], exact_limits),
    ];
    let basis = epoch(
        axes.iter().enumerate().map(|(ordinal, leader)| {
            consequence(
                ordinal,
                [
                    (leader.clone(), context.one()),
                    (zero.clone(), context.index(ordinal).unwrap()),
                ],
                &ordering,
                &context,
                exact_limits,
            )
        }),
        &ordering,
        &context,
        exact_limits,
    );
    let subject = consequence(
        3,
        squares.iter().cloned().map(|power| (power, context.one())),
        &ordering,
        &context,
        exact_limits,
    );
    let mut problem = ModularFrozenNormalFormProblem::try_new(
        &subject,
        basis.division(),
        None,
        &ordering,
        &context,
        exact_limits,
        ModularGuideLimits::default(),
    )
    .unwrap();
    let exact = try_janet_normal_form(subject, &basis, &ordering, &context, exact_limits).unwrap();

    let mut traces = Vec::new();
    for (ordinal, modulus, point) in [
        (0, PRIME_A, [2, 3, 5]),
        (1, PRIME_B, [7, 11, 13]),
        (2, PRIME_C, [17, 19, 23]),
    ] {
        let proposal = problem.try_probe(ordinal, modulus, &point).unwrap();
        assert_trace_matches_exact(&proposal, &exact, &basis);
        let (expected_support, expected_residues) =
            sampled_exact_consequence(exact.remainder(), &context, modulus, &point);
        assert_eq!(
            proposal.trace().sampled_remainder_support(),
            expected_support.as_ref(),
        );
        assert_eq!(
            proposal.nonzero_evidence().sampled_remainder_residues(),
            expected_residues.as_ref(),
        );
        assert!(proposal.census().normal_form_steps() > 0);
        assert!(proposal.census().shift_coordinate_operations() > 0);
        traces.push(proposal.trace().clone());
    }
    assert!(traces.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn generated_two_loop_sunset_source_has_the_same_modular_and_exact_trace() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let exact_limits = InvolutiveLimits::default();
    let ordering = OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new([true, false, true]).unwrap(),
        &completed,
        exact_limits,
    )
    .unwrap();
    let lift_limits = OrdinaryChartLiftLimits {
        involutive: exact_limits,
        ..OrdinaryChartLiftLimits::default()
    };
    let lifted_basis = try_lift_completed_ordinary_sources(
        &completed,
        &ordering,
        generator.context(),
        lift_limits,
    )
    .unwrap();
    let basis_consequences = lifted_basis
        .try_into_consequences(&completed, &ordering, generator.context(), exact_limits)
        .unwrap();
    let basis = epoch(
        basis_consequences.into_vec(),
        &ordering,
        generator.context(),
        exact_limits,
    );

    // Regenerate an independently allocated source from the same sealed
    // ordinary-source owner, then translate it in the mixed sector. This is
    // an actual generated K=3 source row, not a hand-shaped surrogate.
    let lifted_subject = try_lift_completed_ordinary_sources(
        &completed,
        &ordering,
        generator.context(),
        lift_limits,
    )
    .unwrap();
    let divisor = lifted_subject.sources()[0].consequence();
    let operator = shift([0, 3, 0], exact_limits);
    let subject = OreConsequence::try_zero(&ordering, generator.context(), exact_limits)
        .unwrap()
        .try_left_axpy(
            &generator.context().one(),
            &operator,
            divisor,
            &ordering,
            generator.context(),
            exact_limits,
        )
        .unwrap();
    let mut problem = ModularFrozenNormalFormProblem::try_new(
        &subject,
        basis.division(),
        None,
        &ordering,
        generator.context(),
        exact_limits,
        ModularGuideLimits::default(),
    )
    .unwrap();
    let exact = try_janet_normal_form(
        subject,
        &basis,
        &ordering,
        generator.context(),
        exact_limits,
    )
    .unwrap();

    let base_count = generator.context().base().parameter_names().len();
    let mut accepted = 0usize;
    for (ordinal, modulus, seed) in [(0, PRIME_A, 31_i64), (1, PRIME_B, 47_i64)] {
        let mut point = vec![seed; base_count];
        point.extend([seed - 19, seed - 17, seed - 13]);
        let proposal = problem.try_probe(ordinal, modulus, &point).unwrap();
        assert_trace_matches_exact(&proposal, &exact, &basis);
        let (expected_support, expected_residues) =
            sampled_exact_consequence(exact.remainder(), generator.context(), modulus, &point);
        assert_eq!(
            proposal.trace().sampled_remainder_support(),
            expected_support.as_ref(),
        );
        assert_eq!(
            proposal.nonzero_evidence().sampled_remainder_residues(),
            expected_residues.as_ref(),
        );
        accepted += 1;
    }
    assert_eq!(accepted, 2);
}

#[test]
fn exclusion_guards_and_resource_stops_are_probe_local_and_typed() {
    let exact_limits = InvolutiveLimits::default();
    let context = context("modular-normal-form-boundaries", 1);
    let ordering = ordering([true], exact_limits);
    let e = shift([1], exact_limits);
    let e2 = shift([2], exact_limits);
    let n = context.index(0).unwrap();
    let guarded = consequence(
        0,
        [(e.clone(), context.one())],
        &ordering,
        &context,
        exact_limits,
    )
    .try_require_nonzero_guard(
        context
            .numerator_condition_with_limits(&n, exact_limits.indexed_algebra.exact_algebra)
            .unwrap(),
        &context,
        exact_limits,
    )
    .unwrap()
    .0;
    let basis = epoch([guarded], &ordering, &context, exact_limits);
    let subject = consequence(
        1,
        [(e2.clone(), context.one())],
        &ordering,
        &context,
        exact_limits,
    );
    let mut problem = ModularFrozenNormalFormProblem::try_new(
        &subject,
        basis.division(),
        None,
        &ordering,
        &context,
        exact_limits,
        ModularGuideLimits::default(),
    )
    .unwrap();
    assert_eq!(
        problem.try_probe(0, PRIME_A, &[0]),
        Err(ModularGuideError::SampledZeroLocalizationGuard),
    );
    let recovered = problem.try_probe(1, PRIME_A, &[1]).unwrap();
    assert_eq!(recovered.trace().steps().len(), 1);

    let mut excluded = ModularFrozenNormalFormProblem::try_new(
        &subject,
        basis.division(),
        Some(0),
        &ordering,
        &context,
        exact_limits,
        ModularGuideLimits::default(),
    )
    .unwrap();
    let excluded = excluded.try_probe(2, PRIME_A, &[1]).unwrap();
    assert_eq!(excluded.trace().excluded_divisor(), Some(0));
    assert!(excluded.trace().steps().is_empty());
    assert_eq!(excluded.trace().sampled_remainder_support(), &[e2]);

    assert!(matches!(
        ModularFrozenNormalFormProblem::try_new(
            &subject,
            basis.division(),
            Some(1),
            &ordering,
            &context,
            exact_limits,
            ModularGuideLimits::default(),
        ),
        Err(ModularGuideError::InvalidExcludedDivisor {
            ordinal: 1,
            basis_rows: 1,
        })
    ));

    for limits in [
        ModularGuideLimits {
            max_normal_form_steps: 0,
            ..ModularGuideLimits::default()
        },
        ModularGuideLimits {
            max_normal_form_divisor_visits: 0,
            ..ModularGuideLimits::default()
        },
        ModularGuideLimits {
            max_trace_bytes: 0,
            ..ModularGuideLimits::default()
        },
        ModularGuideLimits {
            max_divisor_index_query_operations: 0,
            ..ModularGuideLimits::default()
        },
    ] {
        let mut capped = ModularFrozenNormalFormProblem::try_new(
            &subject,
            basis.division(),
            None,
            &ordering,
            &context,
            exact_limits,
            limits,
        )
        .unwrap();
        assert!(matches!(
            capped.try_probe(3, PRIME_A, &[1]),
            Err(ModularGuideError::ResourceLimit { .. })
        ));
    }

    assert!(matches!(
        ModularFrozenNormalFormProblem::try_new(
            &subject,
            basis.division(),
            None,
            &ordering,
            &context,
            exact_limits,
            ModularGuideLimits {
                max_problem_basis_rows: 0,
                ..ModularGuideLimits::default()
            },
        ),
        Err(ModularGuideError::ResourceLimit {
            resource: "modular normal-form problem basis rows",
            requested: 1,
            limit: 0,
        })
    ));
}
