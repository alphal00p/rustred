use crate::algebra::{
    CoefficientContext, ExactAlgebraError, IndexedAlgebraError, IndexedCoefficientContext,
};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::FULL_RANK_ORBITS;
use crate::foundry::completion::involutive::{
    ForwardShift, InvolutiveError, InvolutiveLimits, JanetBasisEpoch, OrdinaryChartLiftError,
    OrdinaryChartLiftLimits, OreConsequence, OreOrderingAdapter, OreRow,
    try_lift_completed_ordinary_sources, try_preprocess_initial_basis,
};
use crate::foundry::completion::source_discovery::{
    RequestedDomainSupportBatchShape, RequestedDomainSupportError, RequestedDomainSupportLimits,
    RequestedDomainSupportProposal, RequestedSupportProposalOrigin,
    RequestedSupportProposalProvenanceInput, try_preflight_requested_domain_support_batch,
    try_union_requested_domain_support,
};
use crate::foundry::completion::{
    CompletionGeometryError, CompletionGeometryLimits, LatticeCardinality,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{CoordinatePriority, Mask, OrderingPolicy};

use super::super::requested::K6_REQUESTED_DOMAIN_SCOPE_KEY;
use super::run::try_convert_basis_leaders_for_test;
use super::*;

fn context(scope: &str, arity: usize) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(std::iter::empty::<&str>()),
        scope,
        arity,
    )
    .unwrap()
}

fn shift(values: &[u64], limits: InvolutiveLimits) -> ForwardShift {
    ForwardShift::try_new(values.iter().copied(), limits).unwrap()
}

fn row(
    source_ordinal: usize,
    supports: &[&[u64]],
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> OreConsequence {
    let row = OreRow::try_new(
        ordering,
        supports
            .iter()
            .map(|support| (shift(support, limits), context.one())),
        context,
        limits,
    )
    .unwrap();
    OreConsequence::try_from_source(source_ordinal, row, ordering, context, limits).unwrap()
}

fn epoch(
    supports: &[&[&[u64]]],
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> JanetBasisEpoch {
    JanetBasisEpoch::try_initial(
        supports
            .iter()
            .enumerate()
            .map(|(ordinal, supports)| row(ordinal, supports, ordering, context, limits)),
        ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap()
}

fn integral_shift(values: &[i64]) -> IntegralShift {
    IntegralShift::try_new(values.iter().copied()).unwrap()
}

#[test]
fn final_leader_conversion_uses_exact_physical_support_and_all_axes() {
    let involutive = InvolutiveLimits::default();
    let context = context("involutive-seed-physical-support", 3);
    let sector = Mask::try_new([true, false, true]).unwrap();
    let ordering =
        OreOrderingAdapter::try_new(OrderingPolicy::default(), sector, involutive).unwrap();
    let basis = epoch(
        &[&[&[0, 0, 0], &[2, 1, 0]]],
        &ordering,
        &context,
        involutive,
    );

    let proposals = try_convert_basis_leaders_for_test(
        "synthetic-scope",
        &basis,
        &ordering,
        &[7; blake3::OUT_LEN],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let [proposal] = proposals.as_slice() else {
        panic!("one basis row must create one proposal")
    };
    assert_eq!(proposal.domain().point(), &[2, 1, 0]);
    assert_eq!(proposal.domain().symbolic_axes(), &[0, 1, 2]);
    assert_eq!(
        proposal.domain().sector().active_bits(),
        &[true, false, true]
    );
    assert_eq!(
        proposal
            .parent_support()
            .iter()
            .map(IntegralShift::values)
            .collect::<Vec<_>>(),
        vec![&[0_i64, 0, 0][..], &[2_i64, -1, 0]]
    );
    assert_eq!(
        proposal.provenance()[0].origin(),
        RequestedSupportProposalOrigin::InvolutiveBasisLeader
    );
}

#[test]
fn active_basis_leader_must_fit_the_exact_requested_sector_chart_carrier() {
    let maximum = i64::MAX as u64;
    let involutive = InvolutiveLimits {
        max_shift_coordinate: maximum,
        max_total_shift_degree: usize::try_from(maximum).unwrap(),
        ..InvolutiveLimits::default()
    };
    let context = context("involutive-seed-active-carrier-fringe", 1);
    let ordering = OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new([true]).unwrap(),
        involutive,
    )
    .unwrap();
    let accepted = epoch(&[&[&[maximum - 1]]], &ordering, &context, involutive);
    let accepted_proposals = try_convert_basis_leaders_for_test(
        "carrier-fringe",
        &accepted,
        &ordering,
        &[2; blake3::OUT_LEN],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    assert_eq!(accepted_proposals[0].domain().point(), &[maximum - 1]);

    let rejected = epoch(&[&[&[maximum]]], &ordering, &context, involutive);
    assert_eq!(
        try_convert_basis_leaders_for_test(
            "carrier-fringe",
            &rejected,
            &ordering,
            &[2; blake3::OUT_LEN],
            RequestedDomainSupportLimits::default(),
        ),
        Err(InvolutiveSeedError::Involutive(InvolutiveError::Geometry(
            CompletionGeometryError::CoordinateNotRepresentable {
                position: 0,
                coordinate: maximum,
                active: true,
            }
        )))
    );
}

#[test]
fn same_basis_leader_domain_support_is_canonically_unioned() {
    let sector = Mask::try_new([true, false]).unwrap();
    let make = |support: IntegralShift, obligation: &str| {
        RequestedDomainSupportProposal::try_new(
            "same-leader",
            &sector,
            &[3, 2],
            &[0, 1],
            &[support],
            RequestedSupportProposalProvenanceInput::new(
                1,
                1,
                4,
                OrderingPolicy::default().stable_id().as_str(),
                obligation,
                RequestedSupportProposalOrigin::InvolutiveBasisLeader,
            ),
            RequestedDomainSupportLimits::default(),
        )
        .unwrap()
    };
    let union = try_union_requested_domain_support(
        vec![
            make(integral_shift(&[2, -1]), "basis-row-right"),
            make(integral_shift(&[0, 0]), "basis-row-left"),
        ],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let [proposal] = union.proposals() else {
        panic!("same semantic leader domains must merge")
    };
    assert_eq!(
        proposal
            .parent_support()
            .iter()
            .map(IntegralShift::values)
            .collect::<Vec<_>>(),
        vec![&[0_i64, 0][..], &[2_i64, -1]]
    );
    assert_eq!(proposal.provenance().len(), 2);
}

#[test]
fn basis_input_and_union_input_orders_cannot_change_support_output() {
    let involutive = InvolutiveLimits::default();
    let context = context("involutive-seed-input-order", 2);
    let sector = Mask::try_new([true, false]).unwrap();
    let ordering =
        OreOrderingAdapter::try_new(OrderingPolicy::default(), sector, involutive).unwrap();
    let forward = epoch(
        &[&[&[2, 0]], &[&[0, 2]], &[&[1, 1]]],
        &ordering,
        &context,
        involutive,
    );
    let reversed = JanetBasisEpoch::try_initial(
        [
            row(2, &[&[1, 1]], &ordering, &context, involutive),
            row(1, &[&[0, 2]], &ordering, &context, involutive),
            row(0, &[&[2, 0]], &ordering, &context, involutive),
        ],
        &ordering,
        &context,
        involutive,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let convert = |basis: &JanetBasisEpoch| {
        try_convert_basis_leaders_for_test(
            "order-independent",
            basis,
            &ordering,
            &[9; blake3::OUT_LEN],
            RequestedDomainSupportLimits::default(),
        )
        .unwrap()
    };
    let expected = try_union_requested_domain_support(
        convert(&forward),
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let mut reordered = convert(&reversed);
    reordered.reverse();
    let actual =
        try_union_requested_domain_support(reordered, RequestedDomainSupportLimits::default())
            .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn report_exhaustively_contains_only_support_diagnostics_and_scalar_census() {
    let sector = Mask::try_new([true]).unwrap();
    let proposal = RequestedDomainSupportProposal::try_new(
        "payload",
        &sector,
        &[1],
        &[0],
        &[integral_shift(&[1])],
        RequestedSupportProposalProvenanceInput::new(
            1,
            1,
            0,
            OrderingPolicy::default().stable_id().as_str(),
            "basis-row",
            RequestedSupportProposalOrigin::InvolutiveBasisLeader,
        ),
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let report = InvolutiveSeedReport {
        status: InvolutiveSeedStatus::JanetQueueExhaustedProposalOnly,
        complement: InvolutiveSeedComplementDiagnostics {
            cardinality: LatticeCardinality::Finite(1),
            pure_power_exponents: vec![Some(1)].into_boxed_slice(),
        },
        census: InvolutiveSeedCensus {
            lifted_source_rows: 1,
            initial_retained_rows: 1,
            initial_equal_head_eliminations: 0,
            initial_zero_remainders: 0,
            initial_nonzero_remainders: 0,
            initial_cascading_collisions: 0,
            initial_max_collision_chain: 0,
            initial_max_head_class: 1,
            basis_rows: 1,
            basis_revision: 0,
            prolongation_attempts: 0,
            zero_remainders: 0,
            nonzero_remainders: 0,
            truncated_blind_priority_epochs: 0,
            autoreduction_passes: 1,
            autoreduction_normal_form_steps: 0,
            autoreduction_dropped_rows: 0,
            autoreduction_shared_rows: 1,
            autoreduction_materialized_rows: 0,
            proposed_support_domains: 1,
            unique_support_domains: 1,
            raw_support_entries: 1,
            unique_support_entries: 1,
        },
        localization: InvolutiveSeedLocalizationCensus {
            guards: 0,
            terms: 0,
            exponent_cells: 0,
            retained_bytes: 0,
        },
        work: InvolutiveSeedWorkCensus {
            divisor_index_build_operations: 0,
            divisor_index_query_operations: 0,
            normal_form_steps: 0,
            normal_form_divisor_visits: 0,
            normal_form_trace_bytes: 0,
            autoreduction_passes: 1,
            autoreduction_shared_rows: 1,
            autoreduction_materialized_rows: 0,
            completion_iterations: 0,
            exact_coefficient_operations: 0,
        },
        support: try_union_requested_domain_support(
            vec![proposal],
            RequestedDomainSupportLimits::default(),
        )
        .unwrap(),
    };

    let InvolutiveSeedReport {
        status,
        complement,
        census,
        localization,
        work,
        support,
    } = report;
    let InvolutiveSeedComplementDiagnostics {
        cardinality,
        pure_power_exponents,
    } = complement;
    let InvolutiveSeedCensus {
        lifted_source_rows,
        initial_retained_rows,
        initial_equal_head_eliminations,
        initial_zero_remainders,
        initial_nonzero_remainders,
        initial_cascading_collisions,
        initial_max_collision_chain,
        initial_max_head_class,
        basis_rows,
        basis_revision,
        prolongation_attempts,
        zero_remainders,
        nonzero_remainders,
        truncated_blind_priority_epochs,
        autoreduction_passes,
        autoreduction_normal_form_steps,
        autoreduction_dropped_rows,
        autoreduction_shared_rows,
        autoreduction_materialized_rows,
        proposed_support_domains,
        unique_support_domains,
        raw_support_entries,
        unique_support_entries,
    } = census;
    let InvolutiveSeedLocalizationCensus {
        guards,
        terms,
        exponent_cells,
        retained_bytes,
    } = localization;
    let InvolutiveSeedWorkCensus {
        divisor_index_build_operations,
        divisor_index_query_operations,
        normal_form_steps,
        normal_form_divisor_visits,
        normal_form_trace_bytes,
        autoreduction_passes: cumulative_autoreduction_passes,
        autoreduction_shared_rows: cumulative_autoreduction_shared_rows,
        autoreduction_materialized_rows: cumulative_autoreduction_materialized_rows,
        completion_iterations,
        exact_coefficient_operations,
    } = work;
    assert_eq!(
        status,
        InvolutiveSeedStatus::JanetQueueExhaustedProposalOnly
    );
    assert_eq!(cardinality, LatticeCardinality::Finite(1));
    assert_eq!(pure_power_exponents.as_ref(), &[Some(1)]);
    assert_eq!(support.proposals().len(), 1);
    assert_eq!(
        (guards, terms, exponent_cells, retained_bytes),
        (0, 0, 0, 0)
    );
    assert_eq!(
        (
            divisor_index_build_operations,
            divisor_index_query_operations,
            normal_form_steps,
            normal_form_divisor_visits,
            normal_form_trace_bytes,
            cumulative_autoreduction_passes,
            cumulative_autoreduction_shared_rows,
            cumulative_autoreduction_materialized_rows,
            completion_iterations,
            exact_coefficient_operations,
        ),
        (0, 0, 0, 0, 0, 1, 1, 0, 0, 0)
    );
    assert_eq!(
        (
            lifted_source_rows,
            initial_retained_rows,
            initial_equal_head_eliminations,
            initial_zero_remainders,
            initial_nonzero_remainders,
            initial_cascading_collisions,
            initial_max_collision_chain,
            initial_max_head_class,
            basis_rows,
            basis_revision,
        ),
        (1, 1, 0, 0, 0, 0, 0, 1, 1, 0)
    );
    assert_eq!(
        (
            prolongation_attempts,
            zero_remainders,
            nonzero_remainders,
            truncated_blind_priority_epochs,
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(
        (
            autoreduction_passes,
            autoreduction_normal_form_steps,
            autoreduction_dropped_rows,
            autoreduction_shared_rows,
            autoreduction_materialized_rows,
            proposed_support_domains,
            unique_support_domains,
            raw_support_entries,
            unique_support_entries,
        ),
        (1, 0, 0, 1, 0, 1, 1, 1, 1)
    );
}

fn synthetic_tadpole(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.integer(-1),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
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

#[test]
fn completed_source_pipeline_returns_only_a_proposal_fixed_point() {
    let family = synthetic_tadpole("involutive-seed-complete-pipeline");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = InvolutiveSeedLimits::default();
    let program = InvolutiveSeedProgram::try_new(
        "complete-pipeline",
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        &completed,
        limits.involutive(),
    )
    .unwrap();
    let report = program
        .try_run(&completed, generator.context(), limits)
        .unwrap();

    assert_eq!(
        report.status(),
        InvolutiveSeedStatus::JanetQueueExhaustedProposalOnly
    );
    assert_eq!(report.census().lifted_source_rows(), 1);
    assert_eq!(report.census().basis_rows(), 1);
    assert_eq!(report.census().prolongation_attempts(), 0);
    assert_eq!(report.census().proposed_support_domains(), 1);
    assert_eq!(report.census().unique_support_domains(), 1);
    assert!(report.complement().is_finite());
    assert!(report.complement().has_complete_pure_power_coverage());
    assert_eq!(
        report.complement().cardinality(),
        LatticeCardinality::Finite(1)
    );
    let [proposal] = report.support().proposals() else {
        panic!("the one-row basis must create one support domain")
    };
    assert_eq!(proposal.domain().point(), &[1]);
    assert_eq!(proposal.domain().symbolic_axes(), &[0]);
    assert_eq!(
        proposal
            .parent_support()
            .iter()
            .map(IntegralShift::values)
            .collect::<Vec<_>>(),
        vec![&[0_i64][..], &[1_i64]]
    );
}

#[test]
fn source_owner_mismatch_is_rejected_before_completion() {
    let family = synthetic_tadpole("involutive-seed-owner");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let owned = complete_ordinary(&generator);
    let foreign = complete_ordinary(&generator);
    let limits = InvolutiveSeedLimits::default();
    let program = InvolutiveSeedProgram::try_new(
        "owner-bound",
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        &owned,
        limits.involutive(),
    )
    .unwrap();
    assert_eq!(
        program.try_run(&foreign, generator.context(), limits),
        Err(InvolutiveSeedError::ChartLift(
            OrdinaryChartLiftError::ForeignSourceOwner
        ))
    );
}

#[test]
fn chart_and_support_resource_stops_are_tight_and_typed() {
    let family = synthetic_tadpole("involutive-seed-limits");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let defaults = InvolutiveSeedLimits::default();
    let program = InvolutiveSeedProgram::try_new(
        "bounded",
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        &completed,
        defaults.involutive(),
    )
    .unwrap();
    let chart_limit = InvolutiveSeedLimits {
        chart_lift: OrdinaryChartLiftLimits {
            max_source_rows: 0,
            ..defaults.chart_lift
        },
        ..defaults
    };
    assert_eq!(
        program.try_run(&completed, generator.context(), chart_limit),
        Err(InvolutiveSeedError::ChartLift(
            OrdinaryChartLiftError::Involutive(InvolutiveError::ResourceLimit {
                resource: "ordinary chart-lift source rows",
                requested: 1,
                limit: 0,
            })
        ))
    );

    let involutive = InvolutiveLimits::default();
    let context = context("involutive-seed-support-limit", 2);
    let ordering = OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new([true, false]).unwrap(),
        involutive,
    )
    .unwrap();
    let basis = epoch(&[&[&[0, 0], &[2, 1]]], &ordering, &context, involutive);
    let mut support_limits = RequestedDomainSupportLimits::default();
    support_limits.max_raw_support_entries = 1;
    assert_eq!(
        try_convert_basis_leaders_for_test(
            "bounded",
            &basis,
            &ordering,
            &[11; blake3::OUT_LEN],
            support_limits,
        ),
        Err(InvolutiveSeedError::RequestedSupport(
            RequestedDomainSupportError::ResourceLimit {
                resource: "raw requested-domain parent-support entries",
                requested: 2,
                limit: 1,
            }
        ))
    );
}

#[test]
fn basis_support_batch_preflight_rejects_exact_one_below_limits_before_conversion() {
    let involutive = InvolutiveLimits::default();
    let context = context("involutive-seed-batch-preflight", 2);
    let ordering = OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new([true, false]).unwrap(),
        involutive,
    )
    .unwrap();
    let digest = [13; blake3::OUT_LEN];
    let one = epoch(&[&[&[0, 0], &[2, 1]]], &ordering, &context, involutive);
    let one_proposals = try_convert_basis_leaders_for_test(
        "batch-preflight",
        &one,
        &ordering,
        &digest,
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let one_union =
        try_union_requested_domain_support(one_proposals, RequestedDomainSupportLimits::default())
            .unwrap();

    let mut arity_limits = RequestedDomainSupportLimits::default();
    arity_limits.max_arity = one.arity() - 1;
    assert_eq!(
        try_convert_basis_leaders_for_test(
            "batch-preflight",
            &one,
            &ordering,
            &digest,
            arity_limits,
        ),
        Err(InvolutiveSeedError::RequestedSupport(
            RequestedDomainSupportError::ResourceLimit {
                resource: "requested-domain arity",
                requested: one.arity(),
                limit: one.arity() - 1,
            }
        ))
    );

    let exact_one_work = one_union.census().canonicalization_work();
    let mut work_limits = RequestedDomainSupportLimits::default();
    work_limits.max_canonicalization_work = exact_one_work - 1;
    assert_eq!(
        try_convert_basis_leaders_for_test(
            "batch-preflight",
            &one,
            &ordering,
            &digest,
            work_limits,
        ),
        Err(InvolutiveSeedError::RequestedSupport(
            RequestedDomainSupportError::ResourceLimit {
                resource: "requested-domain support canonicalization work",
                requested: exact_one_work,
                limit: exact_one_work - 1,
            }
        ))
    );

    let exact_one_bytes = one_union.census().retained_bytes();
    let mut byte_limits = RequestedDomainSupportLimits::default();
    byte_limits.max_retained_bytes = exact_one_bytes - 1;
    assert_eq!(
        try_convert_basis_leaders_for_test(
            "batch-preflight",
            &one,
            &ordering,
            &digest,
            byte_limits,
        ),
        Err(InvolutiveSeedError::RequestedSupport(
            RequestedDomainSupportError::ResourceLimit {
                resource: "requested-domain support retained bytes",
                requested: exact_one_bytes,
                limit: exact_one_bytes - 1,
            }
        ))
    );

    let multiple = epoch(&[&[&[2, 0]], &[&[0, 2]]], &ordering, &context, involutive);
    let multiple_proposals = try_convert_basis_leaders_for_test(
        "batch-preflight",
        &multiple,
        &ordering,
        &digest,
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let token = try_preflight_requested_domain_support_batch(
        multiple_proposals.iter().map(|proposal| {
            let provenance = &proposal.provenance()[0];
            RequestedDomainSupportBatchShape::new(
                proposal.domain().stable_scope_key().len(),
                proposal.domain().sector().arity(),
                proposal.domain().symbolic_axes().len(),
                proposal.parent_support().len(),
                provenance.ordering_key().len(),
                provenance.obligation_key().len(),
            )
        }),
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    let max_atomic_bytes = multiple_proposals
        .iter()
        .map(|proposal| proposal.census().retained_bytes())
        .max()
        .unwrap();
    let max_atomic_work = multiple_proposals
        .iter()
        .map(|proposal| proposal.census().canonicalization_work())
        .max()
        .unwrap();
    let multiple_union = try_union_requested_domain_support(
        multiple_proposals,
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    assert_eq!(token.union_census(), multiple_union.census());
    assert!(multiple_union.census().retained_bytes() > max_atomic_bytes);
    assert!(multiple_union.census().canonicalization_work() > max_atomic_work);

    let exact_multiple_work = multiple_union.census().canonicalization_work();
    let mut aggregate_work_limits = RequestedDomainSupportLimits::default();
    aggregate_work_limits.max_canonicalization_work = exact_multiple_work - 1;
    assert_eq!(
        try_convert_basis_leaders_for_test(
            "batch-preflight",
            &multiple,
            &ordering,
            &digest,
            aggregate_work_limits,
        ),
        Err(InvolutiveSeedError::RequestedSupport(
            RequestedDomainSupportError::ResourceLimit {
                resource: "requested-domain support canonicalization work",
                requested: exact_multiple_work,
                limit: exact_multiple_work - 1,
            }
        ))
    );

    let exact_multiple_bytes = multiple_union.census().retained_bytes();
    let mut aggregate_byte_limits = RequestedDomainSupportLimits::default();
    aggregate_byte_limits.max_retained_bytes = exact_multiple_bytes - 1;
    assert_eq!(
        try_convert_basis_leaders_for_test(
            "batch-preflight",
            &multiple,
            &ordering,
            &digest,
            aggregate_byte_limits,
        ),
        Err(InvolutiveSeedError::RequestedSupport(
            RequestedDomainSupportError::ResourceLimit {
                resource: "requested-domain support retained bytes",
                requested: exact_multiple_bytes,
                limit: exact_multiple_bytes - 1,
            }
        ))
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct K6SeedHarnessManifestEntry {
    manifest_index: usize,
    representative: [i64; 6],
    active_line_count: usize,
}

const K6_BASELINE_DIVISOR_VISITS: usize = 262_144;
const K6_DIAGNOSTIC_TIER_ENV: &str = "RUSTRED_K6_JANET_DIAGNOSTIC_TIER";
const K6_DIAGNOSTIC_ORDERING_ENV: &str = "RUSTRED_K6_JANET_RANK_BY_SLOT";
const K6_AUTONOMOUS_WINNER_RANK_BY_SLOT: [usize; 6] = [5, 3, 4, 2, 0, 1];
// The unit-mass K6 coefficient map retains d followed by the six indices.
// Symbolica stores one dense exponent cell per mapped variable and monomial.
const K6_STUDY_COEFFICIENT_VARIABLES: usize = 1 + 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum K6JanetDiagnosticTier {
    X4,
    X16,
    X64,
    Study,
}

impl K6JanetDiagnosticTier {
    fn try_parse(value: Option<&str>) -> Result<Self, &'static str> {
        match value.unwrap_or("x4") {
            "x4" | "4" => Ok(Self::X4),
            "x16" | "16" => Ok(Self::X16),
            "x64" | "64" => Ok(Self::X64),
            "study" => Ok(Self::Study),
            _ => Err("expected x4, x16, x64, or study"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::X4 => "x4",
            Self::X16 => "x16",
            Self::X64 => "x64",
            Self::Study => "study",
        }
    }

    const fn divisor_visits(self) -> usize {
        match self {
            Self::X4 => 1_048_576,
            Self::X16 => 4_194_304,
            Self::X64 => 16_777_216,
            Self::Study => 67_108_864,
        }
    }

    fn from_environment() -> Self {
        let value = std::env::var(K6_DIAGNOSTIC_TIER_ENV).unwrap_or_else(|error| match error {
            std::env::VarError::NotPresent => "x4".to_owned(),
            std::env::VarError::NotUnicode(_) => {
                panic!("{K6_DIAGNOSTIC_TIER_ENV} is not valid UTF-8")
            }
        });
        Self::try_parse(Some(value.as_str()))
            .unwrap_or_else(|detail| panic!("invalid {K6_DIAGNOSTIC_TIER_ENV}={value:?}: {detail}"))
    }
}

fn try_parse_k6_usize(name: &'static str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<u128>()
        .map_err(|_| format!("{name}={value:?} is not an unsigned decimal integer"))?;
    usize::try_from(parsed).map_err(|_| format!("{name}={value:?} exceeds usize"))
}

fn try_parse_k6_u64(name: &'static str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u128>()
        .map_err(|_| format!("{name}={value:?} is not an unsigned decimal integer"))?;
    u64::try_from(parsed).map_err(|_| format!("{name}={value:?} exceeds u64"))
}

fn try_parse_k6_u16(name: &'static str, value: &str) -> Result<u16, String> {
    let parsed = value
        .parse::<u128>()
        .map_err(|_| format!("{name}={value:?} is not an unsigned decimal integer"))?;
    u16::try_from(parsed).map_err(|_| format!("{name}={value:?} exceeds u16"))
}

fn try_parse_k6_diagnostic_ordering(value: Option<&str>) -> Result<OrderingPolicy, String> {
    let Some(value) = value else {
        return Ok(OrderingPolicy::default());
    };
    let ranks = match value {
        "natural" => return Ok(OrderingPolicy::default()),
        "autonomous-winner" => K6_AUTONOMOUS_WINNER_RANK_BY_SLOT.to_vec(),
        _ => {
            let fields = value.split(',').collect::<Vec<_>>();
            if fields.len() != 6 {
                return Err(format!(
                    "{K6_DIAGNOSTIC_ORDERING_ENV}={value:?} must be natural, autonomous-winner, or six comma-separated ranks"
                ));
            }
            fields
                .into_iter()
                .map(|field| try_parse_k6_usize(K6_DIAGNOSTIC_ORDERING_ENV, field))
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    let priority = CoordinatePriority::try_new(6, &ranks, Default::default())
        .map_err(|error| format!("invalid {K6_DIAGNOSTIC_ORDERING_ENV}={value:?}: {error}"))?;
    OrderingPolicy::try_with_coordinate_priority(&priority)
        .map_err(|error| format!("invalid {K6_DIAGNOSTIC_ORDERING_ENV}={value:?}: {error}"))
}

fn try_k6_diagnostic_ordering_from_environment() -> Result<OrderingPolicy, String> {
    match std::env::var(K6_DIAGNOSTIC_ORDERING_ENV) {
        Ok(value) => try_parse_k6_diagnostic_ordering(Some(value.as_str())),
        Err(std::env::VarError::NotPresent) => try_parse_k6_diagnostic_ordering(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("{K6_DIAGNOSTIC_ORDERING_ENV} is not valid UTF-8"))
        }
    }
}

fn try_k6_env_usize(name: &'static str, current: usize) -> Result<usize, String> {
    match std::env::var(name) {
        Ok(value) => try_parse_k6_usize(name, value.as_str()),
        Err(std::env::VarError::NotPresent) => Ok(current),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid UTF-8")),
    }
}

fn try_k6_env_u64(name: &'static str, current: u64) -> Result<u64, String> {
    match std::env::var(name) {
        Ok(value) => try_parse_k6_u64(name, value.as_str()),
        Err(std::env::VarError::NotPresent) => Ok(current),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid UTF-8")),
    }
}

fn try_k6_env_u16(name: &'static str, current: u16) -> Result<u16, String> {
    match std::env::var(name) {
        Ok(value) => try_parse_k6_u16(name, value.as_str()),
        Err(std::env::VarError::NotPresent) => Ok(current),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid UTF-8")),
    }
}

#[test]
fn k6_release_diagnostic_tiers_are_explicit_and_bounded() {
    assert_eq!(
        K6JanetDiagnosticTier::try_parse(None),
        Ok(K6JanetDiagnosticTier::X4)
    );
    assert_eq!(
        K6JanetDiagnosticTier::try_parse(Some("16")),
        Ok(K6JanetDiagnosticTier::X16)
    );
    assert_eq!(
        K6JanetDiagnosticTier::try_parse(Some("x64")),
        Ok(K6JanetDiagnosticTier::X64)
    );
    assert_eq!(
        K6JanetDiagnosticTier::try_parse(Some("study")),
        Ok(K6JanetDiagnosticTier::Study)
    );
    assert!(K6JanetDiagnosticTier::try_parse(Some("unbounded")).is_err());

    assert_eq!(try_parse_k6_usize("TEST", "123"), Ok(123));
    assert!(try_parse_k6_usize("TEST", "-1").is_err());
    assert!(try_parse_k6_usize("TEST", &(usize::MAX as u128 + 1).to_string()).is_err());
    assert!(try_parse_k6_u64("TEST", &(u64::MAX as u128 + 1).to_string()).is_err());
    assert!(try_parse_k6_u16("TEST", &(u16::MAX as u128 + 1).to_string()).is_err());

    assert_eq!(
        try_parse_k6_diagnostic_ordering(None).unwrap(),
        OrderingPolicy::default()
    );
    assert_eq!(
        try_parse_k6_diagnostic_ordering(Some("natural")).unwrap(),
        OrderingPolicy::default()
    );
    let autonomous = try_parse_k6_diagnostic_ordering(Some("autonomous-winner")).unwrap();
    assert_eq!(
        autonomous
            .try_coordinate_priority()
            .unwrap()
            .unwrap()
            .rank_by_slot(),
        K6_AUTONOMOUS_WINNER_RANK_BY_SLOT
    );
    assert_eq!(
        try_parse_k6_diagnostic_ordering(Some("5,3,4,2,0,1")).unwrap(),
        autonomous
    );
    assert!(try_parse_k6_diagnostic_ordering(Some("0,1,2")).is_err());
    assert!(try_parse_k6_diagnostic_ordering(Some("0,1,2,3,4,4")).is_err());

    let study = k6_release_study_profile();
    let involutive = study.involutive();
    assert_eq!(involutive.max_divisor_index_retained_bytes, 536_870_912);
    assert_eq!(involutive.max_divisor_index_build_scratch_bytes, 67_108_864);
    assert_eq!(involutive.max_divisor_index_scratch_bytes, 1_048_576);
    assert_eq!(involutive.max_divisor_index_build_operations, 1_000_000_000);
    assert_eq!(involutive.max_divisor_index_query_operations, 4_000_000_000);
    assert_eq!(involutive.max_normal_form_divisor_visits, 67_108_864);
    assert_eq!(involutive.max_completion_iterations, 16_384);
    assert_eq!(involutive.max_epoch, 4_096);
    assert_eq!(involutive.max_basis_rows, 4_096);
    assert_eq!(involutive.max_normal_form_steps, 1_000_000);
    assert_eq!(involutive.max_autoreduction_passes, 4_096);
    assert_eq!(involutive.max_exact_coefficient_operations, 1_000_000_000);
    assert_eq!(involutive.max_consequence_coefficient_terms, 8_388_608);
    assert_eq!(
        involutive.max_consequence_coefficient_exponent_cells,
        58_720_256
    );
    assert_eq!(
        involutive.max_consequence_coefficient_exponent_cells,
        involutive.max_consequence_coefficient_terms * K6_STUDY_COEFFICIENT_VARIABLES
    );
    assert_eq!(
        involutive.max_consequence_coefficient_retained_bytes,
        1_073_741_824
    );
    assert_eq!(involutive.max_localization_guard_terms, 4_194_304);
    assert_eq!(involutive.max_localization_guard_exponent_cells, 29_360_128);
    assert_eq!(
        involutive.max_localization_guard_exponent_cells,
        involutive.max_localization_guard_terms * K6_STUDY_COEFFICIENT_VARIABLES
    );
    assert_eq!(
        involutive.max_localization_guard_retained_bytes,
        536_870_912
    );
    assert_eq!(involutive.max_basis_coefficient_terms, 33_554_432);
    assert_eq!(involutive.max_basis_coefficient_exponent_cells, 234_881_024);
    assert_eq!(
        involutive.max_basis_coefficient_exponent_cells,
        involutive.max_basis_coefficient_terms * K6_STUDY_COEFFICIENT_VARIABLES
    );
    assert_eq!(
        involutive.max_basis_coefficient_retained_bytes,
        2_147_483_648
    );
    assert_eq!(
        involutive
            .indexed_algebra
            .exact_algebra
            .max_polynomial_terms,
        16_777_216
    );
    assert_eq!(
        involutive
            .indexed_algebra
            .max_specialization_power_operations,
        268_435_456
    );
    assert!(involutive.max_initial_pivot_head_comparisons >= 4_000_036);
    assert!(involutive.max_initial_pivot_head_coordinate_visits >= 24_000_216);
}

// Every full-rank orbit gets both harness modes. In particular, the
// low-line sectors' corner factorizations do not discharge their complete
// dotted/numerator lattices and cannot remove them from the Janet seed matrix.
macro_rules! k6_seed_harness_matrix {
    ($(($diagnostic:ident, $strict:ident, $index:literal, $representative:expr, $lines:literal)),+ $(,)?) => {
        const K6_SEED_HARNESS_MANIFEST: &[K6SeedHarnessManifestEntry] = &[
            $(K6SeedHarnessManifestEntry {
                manifest_index: $index,
                representative: $representative,
                active_line_count: $lines,
            }),+
        ];

        $(
            /// Non-gating bounded release diagnostic; typed resource stops
            /// pass only with an explicit diagnostic label.
            #[test]
            #[ignore = "release-only diagnostic bounded K6 Janet seed"]
            fn $diagnostic() {
                run_diagnostic_k6_seed_harness($index);
            }

            /// Strict release-only Janet seed-complete gate. Passing means
            /// only queue exhaustion, finite complete pure-power geometry,
            /// and nonempty requested support. It is not compiler or artifact
            /// closure.
            #[test]
            #[ignore = "release-only strict K6 Janet seed-complete gate"]
            fn $strict() {
                run_strict_k6_janet_seed_complete_gate($index);
            }
        )+
    };
}

k6_seed_harness_matrix!(
    (
        diagnostic_release_k6_orbit_zero_bounded,
        strict_release_k6_orbit_zero_janet_seed_complete,
        0,
        [0, 0, 1, 0, 1, 1],
        3
    ),
    (
        diagnostic_release_k6_orbit_one_bounded,
        strict_release_k6_orbit_one_janet_seed_complete,
        1,
        [0, 0, 1, 1, 0, 1],
        3
    ),
    (
        diagnostic_release_k6_orbit_two_bounded,
        strict_release_k6_orbit_two_janet_seed_complete,
        2,
        [0, 0, 1, 1, 1, 1],
        4
    ),
    (
        diagnostic_release_k6_orbit_three_bounded,
        strict_release_k6_orbit_three_janet_seed_complete,
        3,
        [0, 1, 1, 1, 1, 0],
        4
    ),
    (
        diagnostic_release_k6_orbit_four_bounded,
        strict_release_k6_orbit_four_janet_seed_complete,
        4,
        [0, 1, 1, 1, 1, 1],
        5
    ),
    (
        diagnostic_release_k6_orbit_five_bounded,
        strict_release_k6_orbit_five_janet_seed_complete,
        5,
        [1, 1, 1, 1, 1, 1],
        6
    ),
);

fn assert_k6_seed_harness_manifest() {
    assert_eq!(
        K6_SEED_HARNESS_MANIFEST.len(),
        FULL_RANK_ORBITS.len(),
        "the K6 Janet seed harness matrix must cover the complete full-rank manifest"
    );
    for (manifest_index, (expected, manifest)) in K6_SEED_HARNESS_MANIFEST
        .iter()
        .zip(FULL_RANK_ORBITS.iter())
        .enumerate()
    {
        assert_eq!(
            expected.manifest_index, manifest_index,
            "K6 Janet seed harness index coverage drift"
        );
        assert_eq!(
            manifest.representative, expected.representative,
            "K6 orbit {manifest_index} representative drift"
        );
        assert_eq!(
            manifest
                .representative
                .iter()
                .filter(|&&power| power != 0)
                .count(),
            expected.active_line_count,
            "K6 orbit {manifest_index} active-line drift"
        );
    }
}

#[test]
fn k6_seed_harness_matrix_covers_every_reviewed_manifest_entry_exactly_once() {
    assert_k6_seed_harness_manifest();
}

/// Release-only lift-and-preprocess diagnostic for the initial K6 Janet heads.
///
/// This reports the exact nine lifted ordinary heads, their collision classes,
/// and the distinct heads produced by initial equal-head elimination. It does
/// not start autoreduction, prolongation, source discovery, or a campaign.
#[test]
#[ignore = "release-only K6 lifted-head diagnostic"]
fn diagnostic_release_k6_orbit_three_lifted_initial_heads_only() {
    use std::collections::BTreeMap;
    use std::time::Instant;

    assert!(
        !cfg!(debug_assertions),
        "K6 lifted-head timing requires a release build"
    );
    let started = Instant::now();
    let inputs = super::super::preset_k6::shared_k6_algebra_inputs().unwrap();
    let completed = inputs.completed();
    let limits = InvolutiveLimits::default();
    let sector = Mask::try_from_indices(&FULL_RANK_ORBITS[3].representative).unwrap();
    let ordering = OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        sector,
        completed,
        limits,
    )
    .unwrap();
    let lifted = try_lift_completed_ordinary_sources(
        completed,
        &ordering,
        inputs.generator().context(),
        OrdinaryChartLiftLimits::default(),
    )
    .unwrap();
    assert_eq!(lifted.len(), 9);

    let mut groups = BTreeMap::<Vec<u64>, Vec<usize>>::new();
    for source in lifted.sources() {
        let (leader, _) = source
            .row()
            .try_leading_term(&ordering)
            .unwrap()
            .expect("an ordinary K6 source row cannot lift to zero");
        groups
            .entry(leader.shift().values().to_vec())
            .or_default()
            .push(source.source_ordinal());
        println!(
            "k6-lifted-head orbit_index=3 source_ordinal={} row_id={} leader={:?} support_terms={}",
            source.source_ordinal(),
            source.source_row().stable_string(),
            leader.shift().values(),
            source.row().terms().len(),
        );
    }
    for (leader, source_ordinals) in &groups {
        println!(
            "k6-lifted-head-group orbit_index=3 leader={leader:?} multiplicity={} source_ordinals={source_ordinals:?}",
            source_ordinals.len(),
        );
    }
    println!(
        "k6-lifted-head-summary orbit_index=3 rows={} unique_heads={} duplicate_classes={} elapsed_seconds={:.6}",
        lifted.len(),
        groups.len(),
        groups
            .values()
            .filter(|ordinals| ordinals.len() > 1)
            .count(),
        started.elapsed().as_secs_f64(),
    );

    let preprocessing_started = Instant::now();
    let consequences = lifted
        .try_into_consequences(completed, &ordering, inputs.generator().context(), limits)
        .unwrap();
    let initial = try_preprocess_initial_basis(
        consequences.into_vec(),
        &ordering,
        inputs.generator().context(),
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let reduced_heads = initial
        .epoch()
        .elements()
        .iter()
        .map(|element| element.leading_shift().values().to_vec())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(reduced_heads.len(), initial.epoch().elements().len());
    assert_eq!(
        reduced_heads,
        std::collections::BTreeSet::from([
            vec![0, 2, 1, 1, 0, 2],
            vec![1, 1, 1, 0, 1, 1],
            vec![1, 1, 1, 1, 0, 1],
            vec![1, 1, 1, 1, 1, 1],
            vec![1, 1, 2, 1, 1, 1],
            vec![1, 2, 1, 1, 1, 1],
            vec![2, 1, 1, 1, 0, 0],
            vec![2, 1, 1, 1, 1, 0],
            vec![2, 1, 1, 2, 0, 0],
        ])
    );
    let census = initial.census();
    assert_eq!(census.input_rows(), 9);
    assert_eq!(census.retained_rows(), 9);
    assert_eq!(census.equal_head_eliminations(), 1);
    assert_eq!(census.zero_remainders(), 0);
    assert_eq!(census.nonzero_remainders(), 1);
    assert_eq!(census.cascading_collisions(), 0);
    assert_eq!(census.max_collision_chain(), 1);
    assert_eq!(census.max_head_class(), 2);
    assert_eq!(census.sort_comparisons(), 25);
    assert_eq!(census.sort_payload_visits(), 137_050);
    assert_eq!(census.pivot_head_comparisons(), 23);
    assert_eq!(census.pivot_head_coordinate_visits(), 138);
    assert_eq!(census.pivot_insertion_moves(), 26);
    println!(
        "k6-initial-head-reduction orbit_index=3 reduced_heads={reduced_heads:?} census={:?} work={:?} preprocessing_elapsed_seconds={:.6} total_elapsed_seconds={:.6}",
        initial.census(),
        initial.work_census(),
        preprocessing_started.elapsed().as_secs_f64(),
        started.elapsed().as_secs_f64(),
    );
}

fn try_run_release_k6_seed_bounded_profile(
    orbit_index: usize,
) -> Result<InvolutiveSeedReport, InvolutiveSeedError> {
    try_run_release_k6_seed_profile(
        orbit_index,
        k6_release_baseline_profile(K6_BASELINE_DIVISOR_VISITS),
        OrderingPolicy::default(),
    )
}

fn try_run_release_k6_seed_profile(
    orbit_index: usize,
    profile: InvolutiveSeedLimits,
    ordering: OrderingPolicy,
) -> Result<InvolutiveSeedReport, InvolutiveSeedError> {
    assert_k6_seed_harness_manifest();
    let expected = K6_SEED_HARNESS_MANIFEST
        .get(orbit_index)
        .expect("unreviewed K6 seed harness orbit");
    assert_eq!(expected.manifest_index, orbit_index);
    let inputs = super::super::preset_k6::shared_k6_algebra_inputs().unwrap();
    let sector = Mask::try_from_indices(&FULL_RANK_ORBITS[orbit_index].representative).unwrap();
    let involutive = profile.involutive();
    let program = InvolutiveSeedProgram::try_new(
        K6_REQUESTED_DOMAIN_SCOPE_KEY,
        sector,
        ordering,
        inputs.completed(),
        involutive,
    )
    .unwrap();
    program.try_run(inputs.completed(), inputs.generator().context(), profile)
}

fn k6_release_baseline_profile(max_normal_form_divisor_visits: usize) -> InvolutiveSeedLimits {
    let involutive = InvolutiveLimits {
        max_arity: 6,
        max_shift_coordinate: 16,
        max_total_shift_degree: 48,
        max_row_terms: 4_096,
        max_provenance_terms: 16_384,
        max_axpy_input_terms: 8_192,
        max_consequence_coefficient_terms: 262_144,
        max_consequence_coefficient_exponent_cells: 1_572_864,
        max_consequence_coefficient_retained_bytes: 67_108_864,
        max_localization_guards: 4_096,
        max_localization_guard_terms: 32_768,
        max_localization_guard_exponent_cells: 196_608,
        max_localization_guard_retained_bytes: 16_777_216,
        max_basis_rows: 128,
        max_basis_coordinate_cells: 768,
        max_basis_coefficient_terms: 1_048_576,
        max_basis_coefficient_exponent_cells: 6_291_456,
        max_basis_coefficient_retained_bytes: 268_435_456,
        max_initial_sort_comparisons: 65_536,
        max_initial_sort_payload_visits: 16_777_216,
        max_initial_pivot_head_comparisons: 65_536,
        max_initial_pivot_head_coordinate_visits: 1_048_576,
        max_initial_pivot_insertion_moves: 65_536,
        max_mask_prefix_comparisons: 65_536,
        max_mask_sort_coordinate_comparisons: 65_536,
        max_mask_retained_bytes: 1_048_576,
        max_divisor_index_retained_bytes: 8_388_608,
        max_divisor_index_build_scratch_bytes: 1_048_576,
        max_divisor_index_scratch_bytes: 1_048_576,
        max_divisor_index_build_operations: 67_108_864,
        max_divisor_index_query_operations: 1_000_000_000,
        max_prolongations: 768,
        max_prolongation_coordinate_cells: 4_608,
        max_prolongation_retained_bytes: 1_048_576,
        max_priority_candidates: 768,
        max_blind_priority_intersection_cells: 65_536,
        max_blind_priority_sort_coordinate_comparisons: 65_536,
        max_blind_priority_retained_bytes: 2_097_152,
        max_blind_boxes_scanned: 1_024,
        max_blind_boxes_retained: 256,
        max_blind_coordinate_cells: 1_536,
        max_epoch: 64,
        max_normal_form_steps: 8_192,
        max_normal_form_divisor_visits,
        max_normal_form_trace_bytes: 8_388_608,
        max_completion_iterations: 256,
        max_autoreduction_passes: 64,
        // Both counters are cumulative across every successor autoreduction in
        // the campaign, rather than per pass or per epoch.
        max_autoreduction_shared_rows: 1_000_000_000,
        max_autoreduction_materialized_rows: 1_000_000_000,
        max_exact_coefficient_operations: 1_048_576,
        indexed_algebra: Default::default(),
    };
    InvolutiveSeedLimits {
        chart_lift: OrdinaryChartLiftLimits {
            max_source_rows: 9,
            max_input_terms: 256,
            max_input_conditions: 256,
            max_input_guard_terms: 4_096,
            max_input_guard_exponent_cells: 24_576,
            max_input_guard_retained_bytes: 8_388_608,
            max_input_symbolic_terms: 8_192,
            max_input_symbolic_exponent_cells: 98_304,
            max_input_symbolic_retained_bytes: 16_777_216,
            max_input_coordinate_cells: 1_536,
            max_lifted_coordinate_cells: 1_590,
            max_coefficient_translations: 256,
            max_chart_conversion_work: 6_198,
            involutive,
        },
        geometry: CompletionGeometryLimits {
            max_arity: 6,
            max_requested_generators: 128,
            max_requested_generator_coordinate_cells: 768,
            max_minimal_generators: 128,
            max_requested_boxes: 256,
            max_requested_box_coordinate_cells: 1_536,
            max_uncovered_boxes: 1_024,
            max_uncovered_box_coordinate_cells: 6_144,
            max_split_operations: 65_536,
        },
        requested_support: RequestedDomainSupportLimits {
            max_arity: 6,
            max_raw_domains: 128,
            max_unique_domains: 128,
            max_raw_provenance_records: 128,
            max_unique_provenance_records: 128,
            max_raw_support_entries: 4_096,
            max_unique_support_entries: 4_096,
            max_raw_support_coordinate_cells: 24_576,
            max_unique_support_coordinate_cells: 24_576,
            max_canonicalization_work: 262_144,
            max_retained_bytes: 16_777_216,
        },
        max_finite_complement_points: 65_536,
    }
}

fn k6_release_study_profile() -> InvolutiveSeedLimits {
    let mut profile = k6_release_baseline_profile(67_108_864);
    let involutive = &mut profile.chart_lift.involutive;
    involutive.max_shift_coordinate = 64;
    involutive.max_total_shift_degree = 384;
    involutive.max_row_terms = 262_144;
    involutive.max_provenance_terms = 262_144;
    involutive.max_axpy_input_terms = 1_048_576;
    involutive.max_consequence_coefficient_terms = 8_388_608;
    involutive.max_consequence_coefficient_exponent_cells = 58_720_256;
    involutive.max_consequence_coefficient_retained_bytes = 1_073_741_824;
    involutive.max_localization_guards = 65_536;
    involutive.max_localization_guard_terms = 4_194_304;
    involutive.max_localization_guard_exponent_cells = 29_360_128;
    involutive.max_localization_guard_retained_bytes = 536_870_912;
    involutive.max_basis_rows = 4_096;
    involutive.max_basis_coordinate_cells = 24_576;
    involutive.max_basis_coefficient_terms = 33_554_432;
    involutive.max_basis_coefficient_exponent_cells = 234_881_024;
    involutive.max_basis_coefficient_retained_bytes = 2_147_483_648;
    // Initial pivot admission conservatively couples its lookup bound to the
    // full NF-step budget, even though this K6 ingress has only nine rows.
    involutive.max_initial_pivot_head_comparisons = 8_388_608;
    involutive.max_initial_pivot_head_coordinate_visits = 67_108_864;
    involutive.max_mask_prefix_comparisons = 16_777_216;
    involutive.max_mask_sort_coordinate_comparisons = 16_777_216;
    involutive.max_mask_retained_bytes = 67_108_864;
    involutive.max_divisor_index_retained_bytes = 536_870_912;
    involutive.max_divisor_index_build_scratch_bytes = 67_108_864;
    involutive.max_divisor_index_scratch_bytes = 1_048_576;
    involutive.max_divisor_index_build_operations = 1_000_000_000;
    involutive.max_divisor_index_query_operations = 4_000_000_000;
    involutive.max_prolongations = 24_576;
    involutive.max_prolongation_coordinate_cells = 147_456;
    involutive.max_prolongation_retained_bytes = 268_435_456;
    involutive.max_priority_candidates = 24_576;
    involutive.max_blind_priority_intersection_cells = 1_073_741_824;
    involutive.max_blind_priority_sort_coordinate_comparisons = 1_073_741_824;
    involutive.max_blind_priority_retained_bytes = 268_435_456;
    involutive.max_blind_boxes_scanned = 1_048_576;
    involutive.max_blind_boxes_retained = 65_536;
    involutive.max_blind_coordinate_cells = 393_216;
    involutive.max_epoch = 4_096;
    involutive.max_normal_form_steps = 1_000_000;
    involutive.max_normal_form_trace_bytes = 536_870_912;
    involutive.max_completion_iterations = 16_384;
    involutive.max_autoreduction_passes = 4_096;
    involutive.max_exact_coefficient_operations = 1_000_000_000;
    involutive.indexed_algebra.exact_algebra.max_exponent = u16::MAX;
    involutive
        .indexed_algebra
        .exact_algebra
        .max_polynomial_terms = 16_777_216;
    involutive.indexed_algebra.exact_algebra.max_term_operations = 268_435_456;
    involutive
        .indexed_algebra
        .max_specialization_power_operations = 268_435_456;
    involutive.indexed_algebra.max_specialization_integer_bits = 67_108_864;

    profile.geometry.max_requested_generators = 4_096;
    profile.geometry.max_requested_generator_coordinate_cells = 24_576;
    profile.geometry.max_minimal_generators = 4_096;
    profile.geometry.max_requested_boxes = 65_536;
    profile.geometry.max_requested_box_coordinate_cells = 393_216;
    profile.geometry.max_uncovered_boxes = 1_048_576;
    profile.geometry.max_uncovered_box_coordinate_cells = 6_291_456;
    profile.geometry.max_split_operations = 16_777_216;

    profile.requested_support.max_raw_domains = 4_096;
    profile.requested_support.max_unique_domains = 4_096;
    profile.requested_support.max_raw_provenance_records = 4_096;
    profile.requested_support.max_unique_provenance_records = 4_096;
    profile.requested_support.max_raw_support_entries = 1_048_576;
    profile.requested_support.max_unique_support_entries = 1_048_576;
    profile.requested_support.max_raw_support_coordinate_cells = 6_291_456;
    profile
        .requested_support
        .max_unique_support_coordinate_cells = 6_291_456;
    profile.requested_support.max_canonicalization_work = 67_108_864;
    profile.requested_support.max_retained_bytes = 536_870_912;
    profile.max_finite_complement_points = 16_777_216;
    profile
}

fn try_k6_release_diagnostic_profile(
    tier: K6JanetDiagnosticTier,
) -> Result<InvolutiveSeedLimits, String> {
    let mut profile = match tier {
        K6JanetDiagnosticTier::Study => k6_release_study_profile(),
        _ => k6_release_baseline_profile(tier.divisor_visits()),
    };

    macro_rules! usize_override {
        ($suffix:literal, $target:expr) => {{
            const NAME: &str = concat!("RUSTRED_K6_JANET_", $suffix);
            $target = try_k6_env_usize(NAME, $target)?;
        }};
    }
    macro_rules! u64_override {
        ($suffix:literal, $target:expr) => {{
            const NAME: &str = concat!("RUSTRED_K6_JANET_", $suffix);
            $target = try_k6_env_u64(NAME, $target)?;
        }};
    }
    macro_rules! u16_override {
        ($suffix:literal, $target:expr) => {{
            const NAME: &str = concat!("RUSTRED_K6_JANET_", $suffix);
            $target = try_k6_env_u16(NAME, $target)?;
        }};
    }

    let involutive = &mut profile.chart_lift.involutive;
    usize_override!(
        "MAX_DIVISOR_INDEX_RETAINED_BYTES",
        involutive.max_divisor_index_retained_bytes
    );
    usize_override!(
        "MAX_DIVISOR_INDEX_BUILD_SCRATCH_BYTES",
        involutive.max_divisor_index_build_scratch_bytes
    );
    usize_override!(
        "MAX_DIVISOR_INDEX_SCRATCH_BYTES",
        involutive.max_divisor_index_scratch_bytes
    );
    usize_override!(
        "MAX_DIVISOR_INDEX_BUILD_OPERATIONS",
        involutive.max_divisor_index_build_operations
    );
    usize_override!(
        "MAX_DIVISOR_INDEX_QUERY_OPERATIONS",
        involutive.max_divisor_index_query_operations
    );
    usize_override!(
        "MAX_DIVISOR_VISITS",
        involutive.max_normal_form_divisor_visits
    );
    usize_override!(
        "MAX_COMPLETION_ITERATIONS",
        involutive.max_completion_iterations
    );
    u64_override!("MAX_EPOCH", involutive.max_epoch);
    usize_override!("MAX_BASIS_ROWS", involutive.max_basis_rows);
    usize_override!(
        "MAX_BASIS_COORDINATE_CELLS",
        involutive.max_basis_coordinate_cells
    );
    usize_override!("MAX_PROLONGATIONS", involutive.max_prolongations);
    usize_override!(
        "MAX_PROLONGATION_COORDINATE_CELLS",
        involutive.max_prolongation_coordinate_cells
    );
    usize_override!(
        "MAX_PROLONGATION_RETAINED_BYTES",
        involutive.max_prolongation_retained_bytes
    );
    usize_override!(
        "MAX_PRIORITY_CANDIDATES",
        involutive.max_priority_candidates
    );
    usize_override!("MAX_NORMAL_FORM_STEPS", involutive.max_normal_form_steps);
    usize_override!(
        "MAX_NORMAL_FORM_TRACE_BYTES",
        involutive.max_normal_form_trace_bytes
    );
    usize_override!(
        "MAX_AUTOREDUCTION_PASSES",
        involutive.max_autoreduction_passes
    );
    usize_override!(
        "MAX_EXACT_COEFFICIENT_OPERATIONS",
        involutive.max_exact_coefficient_operations
    );
    u64_override!("MAX_SHIFT_COORDINATE", involutive.max_shift_coordinate);
    usize_override!("MAX_TOTAL_SHIFT_DEGREE", involutive.max_total_shift_degree);
    usize_override!("MAX_ROW_TERMS", involutive.max_row_terms);
    usize_override!("MAX_PROVENANCE_TERMS", involutive.max_provenance_terms);
    usize_override!("MAX_AXPY_INPUT_TERMS", involutive.max_axpy_input_terms);
    usize_override!(
        "MAX_CONSEQUENCE_COEFFICIENT_TERMS",
        involutive.max_consequence_coefficient_terms
    );
    usize_override!(
        "MAX_CONSEQUENCE_COEFFICIENT_EXPONENT_CELLS",
        involutive.max_consequence_coefficient_exponent_cells
    );
    usize_override!(
        "MAX_CONSEQUENCE_COEFFICIENT_RETAINED_BYTES",
        involutive.max_consequence_coefficient_retained_bytes
    );
    usize_override!(
        "MAX_LOCALIZATION_GUARDS",
        involutive.max_localization_guards
    );
    usize_override!(
        "MAX_LOCALIZATION_GUARD_TERMS",
        involutive.max_localization_guard_terms
    );
    usize_override!(
        "MAX_LOCALIZATION_GUARD_EXPONENT_CELLS",
        involutive.max_localization_guard_exponent_cells
    );
    usize_override!(
        "MAX_LOCALIZATION_GUARD_RETAINED_BYTES",
        involutive.max_localization_guard_retained_bytes
    );
    usize_override!(
        "MAX_BLIND_BOXES_SCANNED",
        involutive.max_blind_boxes_scanned
    );
    usize_override!(
        "MAX_BLIND_BOXES_RETAINED",
        involutive.max_blind_boxes_retained
    );
    usize_override!(
        "MAX_BLIND_COORDINATE_CELLS",
        involutive.max_blind_coordinate_cells
    );
    usize_override!(
        "MAX_BLIND_PRIORITY_INTERSECTION_CELLS",
        involutive.max_blind_priority_intersection_cells
    );
    usize_override!(
        "MAX_BLIND_PRIORITY_SORT_COORDINATE_COMPARISONS",
        involutive.max_blind_priority_sort_coordinate_comparisons
    );
    usize_override!(
        "MAX_BLIND_PRIORITY_RETAINED_BYTES",
        involutive.max_blind_priority_retained_bytes
    );
    usize_override!(
        "MAX_BASIS_COEFFICIENT_TERMS",
        involutive.max_basis_coefficient_terms
    );
    usize_override!(
        "MAX_BASIS_COEFFICIENT_EXPONENT_CELLS",
        involutive.max_basis_coefficient_exponent_cells
    );
    usize_override!(
        "MAX_BASIS_COEFFICIENT_RETAINED_BYTES",
        involutive.max_basis_coefficient_retained_bytes
    );
    usize_override!(
        "MAX_INITIAL_PIVOT_HEAD_COMPARISONS",
        involutive.max_initial_pivot_head_comparisons
    );
    usize_override!(
        "MAX_INITIAL_PIVOT_HEAD_COORDINATE_VISITS",
        involutive.max_initial_pivot_head_coordinate_visits
    );
    usize_override!(
        "MAX_INITIAL_PIVOT_INSERTION_MOVES",
        involutive.max_initial_pivot_insertion_moves
    );
    usize_override!(
        "MAX_MASK_PREFIX_COMPARISONS",
        involutive.max_mask_prefix_comparisons
    );
    usize_override!(
        "MAX_MASK_SORT_COORDINATE_COMPARISONS",
        involutive.max_mask_sort_coordinate_comparisons
    );
    usize_override!(
        "MAX_MASK_RETAINED_BYTES",
        involutive.max_mask_retained_bytes
    );
    u16_override!(
        "MAX_EXACT_EXPONENT",
        involutive.indexed_algebra.exact_algebra.max_exponent
    );
    usize_override!(
        "MAX_EXACT_POLYNOMIAL_TERMS",
        involutive
            .indexed_algebra
            .exact_algebra
            .max_polynomial_terms
    );
    usize_override!(
        "MAX_EXACT_TERM_OPERATIONS",
        involutive.indexed_algebra.exact_algebra.max_term_operations
    );
    usize_override!(
        "MAX_SPECIALIZATION_POWER_OPERATIONS",
        involutive
            .indexed_algebra
            .max_specialization_power_operations
    );
    usize_override!(
        "MAX_SPECIALIZATION_INTEGER_BITS",
        involutive.indexed_algebra.max_specialization_integer_bits
    );

    usize_override!(
        "MAX_GEOMETRY_REQUESTED_GENERATORS",
        profile.geometry.max_requested_generators
    );
    usize_override!(
        "MAX_GEOMETRY_REQUESTED_GENERATOR_COORDINATE_CELLS",
        profile.geometry.max_requested_generator_coordinate_cells
    );
    usize_override!(
        "MAX_GEOMETRY_MINIMAL_GENERATORS",
        profile.geometry.max_minimal_generators
    );
    usize_override!(
        "MAX_GEOMETRY_REQUESTED_BOXES",
        profile.geometry.max_requested_boxes
    );
    usize_override!(
        "MAX_GEOMETRY_REQUESTED_BOX_COORDINATE_CELLS",
        profile.geometry.max_requested_box_coordinate_cells
    );
    usize_override!(
        "MAX_GEOMETRY_UNCOVERED_BOXES",
        profile.geometry.max_uncovered_boxes
    );
    usize_override!(
        "MAX_GEOMETRY_UNCOVERED_BOX_COORDINATE_CELLS",
        profile.geometry.max_uncovered_box_coordinate_cells
    );
    usize_override!(
        "MAX_GEOMETRY_SPLIT_OPERATIONS",
        profile.geometry.max_split_operations
    );

    usize_override!(
        "MAX_SUPPORT_RAW_DOMAINS",
        profile.requested_support.max_raw_domains
    );
    usize_override!(
        "MAX_SUPPORT_UNIQUE_DOMAINS",
        profile.requested_support.max_unique_domains
    );
    usize_override!(
        "MAX_SUPPORT_RAW_PROVENANCE_RECORDS",
        profile.requested_support.max_raw_provenance_records
    );
    usize_override!(
        "MAX_SUPPORT_UNIQUE_PROVENANCE_RECORDS",
        profile.requested_support.max_unique_provenance_records
    );
    usize_override!(
        "MAX_SUPPORT_RAW_ENTRIES",
        profile.requested_support.max_raw_support_entries
    );
    usize_override!(
        "MAX_SUPPORT_UNIQUE_ENTRIES",
        profile.requested_support.max_unique_support_entries
    );
    usize_override!(
        "MAX_SUPPORT_RAW_COORDINATE_CELLS",
        profile.requested_support.max_raw_support_coordinate_cells
    );
    usize_override!(
        "MAX_SUPPORT_UNIQUE_COORDINATE_CELLS",
        profile
            .requested_support
            .max_unique_support_coordinate_cells
    );
    usize_override!(
        "MAX_SUPPORT_CANONICALIZATION_WORK",
        profile.requested_support.max_canonicalization_work
    );
    usize_override!(
        "MAX_SUPPORT_RETAINED_BYTES",
        profile.requested_support.max_retained_bytes
    );
    usize_override!(
        "MAX_FINITE_COMPLEMENT_POINTS",
        profile.max_finite_complement_points
    );
    Ok(profile)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StrictK6JanetSeedGateRejection {
    TypedResourceStop,
    RunFailure,
    QueueNotExhausted,
    InfiniteComplement,
    IncompletePurePowerCoverage,
    EmptyRequestedSupport,
}

fn classify_strict_k6_janet_seed_gate(
    outcome: &Result<InvolutiveSeedReport, InvolutiveSeedError>,
) -> Result<(), StrictK6JanetSeedGateRejection> {
    let report = match outcome {
        Ok(report) => report,
        Err(error) if is_expected_bounded_k6_stop(error) => {
            return Err(StrictK6JanetSeedGateRejection::TypedResourceStop);
        }
        Err(_) => return Err(StrictK6JanetSeedGateRejection::RunFailure),
    };
    if report.status() != InvolutiveSeedStatus::JanetQueueExhaustedProposalOnly {
        return Err(StrictK6JanetSeedGateRejection::QueueNotExhausted);
    }
    if !report.complement().is_finite() {
        return Err(StrictK6JanetSeedGateRejection::InfiniteComplement);
    }
    if !report.complement().has_complete_pure_power_coverage() {
        return Err(StrictK6JanetSeedGateRejection::IncompletePurePowerCoverage);
    }
    if report.support().proposals().is_empty() {
        return Err(StrictK6JanetSeedGateRejection::EmptyRequestedSupport);
    }
    Ok(())
}

fn print_k6_seed_success(mode: &str, orbit_index: usize, report: &InvolutiveSeedReport) {
    let census = report.census();
    let complement = report.complement();
    let localization = report.localization();
    let work = report.work();
    println!(
        "k6-involutive-seed mode={mode} orbit_index={orbit_index} status={:?} initial_retained_rows={} initial_equal_head_eliminations={} initial_zero_remainders={} initial_nonzero_remainders={} initial_cascading_collisions={} initial_max_collision_chain={} initial_max_head_class={} basis_rows={} basis_revision={} prolongations={} zero_remainders={} nonzero_remainders={} complement={:?} pure_power_complete={} guards={} guard_terms={} guard_exponent_cells={} guard_retained_bytes={} work_divisor_index_build_operations={} work_divisor_index_query_operations={} work_normal_form_steps={} work_divisor_visits={} work_trace_bytes={} work_autoreduction_passes={} work_autoreduction_shared_rows={} work_autoreduction_materialized_rows={} work_completion_iterations={} work_exact_coefficient_operations={} proposed_support_domains={} unique_support_domains={} raw_support_entries={} unique_support_entries={}",
        report.status(),
        census.initial_retained_rows(),
        census.initial_equal_head_eliminations(),
        census.initial_zero_remainders(),
        census.initial_nonzero_remainders(),
        census.initial_cascading_collisions(),
        census.initial_max_collision_chain(),
        census.initial_max_head_class(),
        census.basis_rows(),
        census.basis_revision(),
        census.prolongation_attempts(),
        census.zero_remainders(),
        census.nonzero_remainders(),
        complement.cardinality(),
        complement.has_complete_pure_power_coverage(),
        localization.guards(),
        localization.terms(),
        localization.exponent_cells(),
        localization.retained_bytes(),
        work.divisor_index_build_operations(),
        work.divisor_index_query_operations(),
        work.normal_form_steps(),
        work.normal_form_divisor_visits(),
        work.normal_form_trace_bytes(),
        work.autoreduction_passes(),
        work.autoreduction_shared_rows(),
        work.autoreduction_materialized_rows(),
        work.completion_iterations(),
        work.exact_coefficient_operations(),
        census.proposed_support_domains(),
        census.unique_support_domains(),
        census.raw_support_entries(),
        census.unique_support_entries(),
    );
}

fn run_diagnostic_k6_seed_harness(orbit_index: usize) {
    if cfg!(debug_assertions) {
        println!(
            "k6-involutive-seed mode=diagnostic_bounded orbit_index={orbit_index} diagnostic_skipped=debug_build resource_stop_accepted=true"
        );
        return;
    }
    let tier = K6JanetDiagnosticTier::from_environment();
    let profile = try_k6_release_diagnostic_profile(tier)
        .unwrap_or_else(|error| panic!("invalid K6 Janet diagnostic envelope: {error}"));
    let ordering = try_k6_diagnostic_ordering_from_environment()
        .unwrap_or_else(|error| panic!("invalid K6 Janet diagnostic ordering: {error}"));
    println!(
        "k6-involutive-seed-envelope mode=diagnostic_bounded orbit_index={orbit_index} tier={} ordering={} profile={profile:?}",
        tier.label(),
        ordering.stable_id(),
    );
    crate::foundry::completion::involutive::diagnostics::begin();
    let outcome = try_run_release_k6_seed_profile(orbit_index, profile, ordering);
    let checkpoint = crate::foundry::completion::involutive::diagnostics::take();
    match outcome {
        Ok(report) => print_k6_seed_success("diagnostic_bounded", orbit_index, &report),
        Err(error) if is_expected_bounded_k6_stop(&error) => {
            let checkpoint = checkpoint.expect("an active K6 diagnostic must retain a checkpoint");
            println!(
                "k6-involutive-seed mode=diagnostic_bounded orbit_index={orbit_index} tier={} divisor_visit_limit={} status=typed_stop stop={error:?} checkpoint={checkpoint:?} complement=not_reached rays=not_reached resource_stop_accepted=true",
                tier.label(),
                profile.involutive().max_normal_form_divisor_visits,
            );
        }
        Err(error) => panic!(
            "diagnostic bounded K6 seed orbit {orbit_index} reached a non-resource failure: {error}"
        ),
    }
}

fn run_strict_k6_janet_seed_complete_gate(orbit_index: usize) {
    assert!(
        !cfg!(debug_assertions),
        "strict K6 Janet seed-complete gate requires a release build"
    );
    let outcome = try_run_release_k6_seed_bounded_profile(orbit_index);
    if let Err(rejection) = classify_strict_k6_janet_seed_gate(&outcome) {
        match outcome {
            Err(error) => panic!(
                "strict K6 Janet seed-complete gate rejected orbit {orbit_index}: {rejection:?}; run_error={error:?}"
            ),
            Ok(report) => panic!(
                "strict K6 Janet seed-complete gate rejected orbit {orbit_index}: {rejection:?}; status={:?}; complement={:?}; pure_power_complete={}; support_domains={}",
                report.status(),
                report.complement().cardinality(),
                report.complement().has_complete_pure_power_coverage(),
                report.support().proposals().len(),
            ),
        }
    }
    let report = outcome.expect("an accepted strict Janet seed result is successful");
    print_k6_seed_success("strict_janet_seed_complete", orbit_index, &report);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "strict K6 Janet seed-complete gate requires a release build")]
fn strict_k6_janet_seed_gate_fails_loudly_before_algebra_in_debug_builds() {
    run_strict_k6_janet_seed_complete_gate(3);
}

#[test]
fn strict_k6_janet_seed_classifier_rejects_every_typed_resource_stop() {
    let stops = [
        InvolutiveSeedError::Involutive(InvolutiveError::ResourceLimit {
            resource: "test",
            requested: 2,
            limit: 1,
        }),
        InvolutiveSeedError::Involutive(InvolutiveError::EpochLimit {
            requested: 2,
            limit: 1,
        }),
        InvolutiveSeedError::Involutive(InvolutiveError::ShiftCoordinateLimit {
            position: 0,
            requested: 2,
            limit: 1,
        }),
        InvolutiveSeedError::Involutive(InvolutiveError::Geometry(
            CompletionGeometryError::ResourceLimit {
                resource: "test",
                requested: 2,
                limit: 1,
            },
        )),
        InvolutiveSeedError::Involutive(InvolutiveError::Algebra(
            IndexedAlgebraError::ResourceLimit {
                resource: "test",
                requested: 2,
                limit: 1,
            },
        )),
        InvolutiveSeedError::Involutive(InvolutiveError::Algebra(
            IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                resource: "test",
                requested: 2,
                limit: 1,
            }),
        )),
        InvolutiveSeedError::RequestedSupport(RequestedDomainSupportError::ResourceLimit {
            resource: "test",
            requested: 2,
            limit: 1,
        }),
    ];
    for stop in stops {
        assert_eq!(
            classify_strict_k6_janet_seed_gate(&Err(stop)),
            Err(StrictK6JanetSeedGateRejection::TypedResourceStop)
        );
    }
}

#[test]
fn strict_k6_janet_seed_classifier_requires_finite_complete_seed_geometry() {
    let family = synthetic_tadpole("strict-k6-seed-classifier");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = InvolutiveSeedLimits::default();
    let program = InvolutiveSeedProgram::try_new(
        "strict-k6-seed-classifier",
        Mask::try_new([true]).unwrap(),
        OrderingPolicy::default(),
        &completed,
        limits.involutive(),
    )
    .unwrap();
    let report = program
        .try_run(&completed, generator.context(), limits)
        .unwrap();
    let mut outcome = Ok(report);
    assert_eq!(classify_strict_k6_janet_seed_gate(&outcome), Ok(()));

    outcome.as_mut().unwrap().complement.cardinality = LatticeCardinality::Infinite;
    assert_eq!(
        classify_strict_k6_janet_seed_gate(&outcome),
        Err(StrictK6JanetSeedGateRejection::InfiniteComplement)
    );

    let report = outcome.as_mut().unwrap();
    report.complement.cardinality = LatticeCardinality::Finite(1);
    report.complement.pure_power_exponents[0] = None;
    assert_eq!(
        classify_strict_k6_janet_seed_gate(&outcome),
        Err(StrictK6JanetSeedGateRejection::IncompletePurePowerCoverage)
    );
}

fn is_expected_bounded_k6_stop(error: &InvolutiveSeedError) -> bool {
    let involutive = match error {
        InvolutiveSeedError::ChartLift(OrdinaryChartLiftError::Involutive(error))
        | InvolutiveSeedError::Involutive(error) => Some(error),
        InvolutiveSeedError::RequestedSupport(RequestedDomainSupportError::ResourceLimit {
            ..
        }) => return true,
        _ => None,
    };
    involutive.is_some_and(|error| {
        matches!(
            error,
            InvolutiveError::ResourceLimit { .. }
                | InvolutiveError::EpochLimit { .. }
                | InvolutiveError::ShiftCoordinateLimit { .. }
                | InvolutiveError::Geometry(CompletionGeometryError::ResourceLimit { .. })
                | InvolutiveError::Algebra(IndexedAlgebraError::ResourceLimit { .. })
                | InvolutiveError::Algebra(IndexedAlgebraError::ExactAlgebra(
                    ExactAlgebraError::ResourceLimit { .. }
                ))
        )
    })
}
