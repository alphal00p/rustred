//! Independent validation for the current-lineage exact Ready publication seam.
//!
//! The massive one-loop tadpole below is only a compact validation family.  It
//! enters through the same topology-neutral generated-affine path as every
//! other family: generated IBPs, sector discovery, Boolean residual cover,
//! exact case inventory, solve plan, physical-row replay, exact session, and
//! recentering.  No recurrence coefficient, loop-count dispatch, or old
//! `GeneratedResidualAffineWhenBad` authority is supplied to production.
//!
//! Publication/provider assertions are kept next to this fixture and must be
//! connected directly to the new exact Ready-consuming API.  In particular,
//! this module must never make the current lineage look complete by copying
//! ordinals into the older affine matcher.

use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::generated_affine_parametric_ordering::{
    GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingLimits,
};
use crate::generated_affine_prepare_point_schedule::{
    GeneratedAffinePreparePointScheduleCertificate, GeneratedAffinePreparePointScheduleLimits,
};
use crate::generated_affine_residual_boolean_cover::{
    GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
};
use crate::generated_affine_residual_case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
    GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
};
use crate::generated_affine_residual_case_premises::{
    GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
    compile_generated_affine_residual_case_premises,
};
use crate::generated_affine_residual_case_reelimination::{
    GeneratedAffineResidualCaseReeliminationCompilation,
    GeneratedAffineResidualCaseReeliminationCompiler,
    GeneratedAffineResidualCaseReeliminationLimits,
};
use crate::generated_affine_residual_group_exact_physical_row::{
    GeneratedAffineResidualGroupExactPhysicalRow,
    GeneratedAffineResidualGroupExactPhysicalRowCompiler,
    GeneratedAffineResidualGroupExactPhysicalRowLimits,
};
use crate::generated_affine_residual_group_exact_session::{
    GeneratedAffineResidualGroupExactSession, GeneratedAffineResidualGroupExactSessionError,
    GeneratedAffineResidualGroupExactSessionEventStats,
    GeneratedAffineResidualGroupExactSessionLimits,
    GeneratedAffineResidualGroupExactSessionRecenterOutcome,
    GeneratedAffineResidualGroupExactSessionRecenterReady,
};
use crate::generated_affine_residual_group_physical_key::{
    GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKeyLimits,
};
use crate::generated_affine_residual_group_ready_publication::{
    GENERATED_AFFINE_RESIDUAL_GROUP_READY_PUBLICATION_ANALYSIS_V2_SCHEMA,
    GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler,
    GeneratedAffineResidualGroupReadyPublicationAnalysisError,
    GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
    GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome,
};
use crate::generated_affine_residual_group_solve_plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanLimits,
};
use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
use crate::{
    AffineDenominator, CoefficientContext, ConcreteIntegralKey, GeneratedFamilyRuleSystemCompiler,
    GeneratedFamilyRuleSystemConfig, GeneratedFamilyRuleSystemLimits,
    GeneratedFamilyRuleSystemProvider, GeneratedFamilyRuleSystemProviderLimits,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpGenerator,
    ParametricReductionEngine, ParametricSectorLeafDisposition, PowerShiftPolicy,
    ReductionEngineLimits, SectorMask, SectorRestrictions,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SessionSnapshot {
    state_version: usize,
    target_count: usize,
    event_stats: GeneratedAffineResidualGroupExactSessionEventStats,
}

impl SessionSnapshot {
    fn capture(session: &GeneratedAffineResidualGroupExactSession) -> Self {
        Self {
            state_version: session.state_version(),
            target_count: session.target_count(),
            event_stats: session.event_stats(),
        }
    }
}

struct NaturalOneLoopReadyFixture {
    family: IntegralFamily,
    context: ParametricCoefficientContext,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    source: Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    session: GeneratedAffineResidualGroupExactSession,
    ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
    before_recenter: SessionSnapshot,
}

fn massive_tadpole(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        // D1 = k^2 - m2, matching the scalar Vakint oracle convention.
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

/// Six-loop, unit-mass coordinate basis used only to gate the topology-neutral
/// parametric generator at `K = L(L + 1) / 2 = 21`.  Deliberately stop before
/// sector Boolean-cover construction: an all-inactive 21-coordinate sector has
/// `2^21` cases and belongs in a separately bounded scale test, not a fast
/// Ready-publication unit test.
fn six_loop_unit_mass_coordinate_basis(name: &str) -> IntegralFamily {
    const LOOPS: usize = 6;
    const ARITY: usize = LOOPS * (LOOPS + 1) / 2;

    let coefficients = CoefficientContext::new(["d"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let denominators = (0..ARITY)
        .map(|row| {
            AffineDenominator::new(
                coefficients.integer(-1),
                (0..ARITY)
                    .map(|column| {
                        if row == column {
                            one.clone()
                        } else {
                            zero.clone()
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    IntegralFamily::new(
        name,
        (0..LOOPS).map(|loop_| format!("k{}", loop_ + 1)).collect(),
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![zero; ARITY],
    )
    .unwrap()
}

fn one_loop_plan(
    name: &str,
    active: bool,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedAffineResidualGroupSolvePlan>,
) {
    let family = massive_tadpole(name);
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();

    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([active]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    assert_eq!(
        discovery.row_span().rows().len(),
        1,
        "one loop must contribute exactly L(L+E)=1 generated IBP row",
    );

    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 1;
    queue_limits.max_translation_points = 3;
    let queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    if active {
        assert!(
            queue
                .work_items()
                .iter()
                .any(|item| { item.extraction().assignment().entries() == &[(0, 1)] })
        );
    } else {
        assert!(
            queue
                .work_items()
                .iter()
                .any(|item| item.extraction().assignment().is_empty()),
            "the inactive sector must retain a naturally generated independent cylinder",
        );
    }

    let boolean = Arc::new(
        GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            GeneratedAffineResidualSourceAuthority::initial_global(queue),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap(),
    );
    let inventory = Arc::new(
        GeneratedAffineResidualCaseInventoryCompiler::compile(
            &family,
            &context,
            boolean,
            GeneratedAffineResidualCaseInventoryLimits::default(),
        )
        .unwrap(),
    );
    let group_ordinal = (0..inventory.group_count())
        .max_by_key(|&ordinal| {
            let group = inventory
                .authenticated_group_view(&context, ordinal)
                .unwrap();
            (group.free_positions().len(), group.case_ordinals().len())
        })
        .expect("the natural tadpole inventory must contain an affine group");
    let group = inventory
        .authenticated_group_view(&context, group_ordinal)
        .unwrap();
    let authority = Arc::new(
        GeneratedAffineResidualCaseAuthority::try_new(
            &family,
            &context,
            Arc::clone(&inventory),
            group.anchor_case_ordinal(),
            GeneratedAffineResidualCaseAuthorityLimits::default(),
        )
        .unwrap(),
    );
    let frame = Arc::new(
        GeneratedAffineResidualGroupPhysicalFrame::try_new(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
        )
        .unwrap(),
    );
    let plan = Arc::new(
        GeneratedAffineResidualGroupSolvePlan::try_new(
            &family,
            &context,
            inventory,
            authority,
            frame,
            GeneratedAffineResidualGroupSolvePlanLimits::default(),
        )
        .unwrap(),
    );
    if active {
        assert!(
            plan.free_positions().is_empty(),
            "the active residual target must remain the literal n=1 point",
        );
    } else {
        assert_eq!(
            plan.free_positions(),
            [0],
            "the validation source must be an independent one-dimensional cylinder",
        );
    }
    (family, context, plan)
}

fn production_rows(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
) -> Vec<Arc<GeneratedAffineResidualGroupExactPhysicalRow>> {
    let frame = plan.physical_frame();
    let mut rows = Vec::new();
    for &case_ordinal in frame.case_ordinals() {
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                family,
                context,
                Arc::clone(plan.inventory().unwrap()),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let premises = match compile_generated_affine_residual_case_premises(
            family,
            context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        )
        .unwrap()
        {
            GeneratedAffineResidualCasePremisesOutcome::Ready(value) => Arc::new(value),
            GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                continue;
            }
        };
        let ordering = Arc::new(
            GeneratedAffineParametricOrderingCertificate::try_new(
                family,
                context,
                Arc::clone(&authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap(),
        );
        let schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                family,
                context,
                Arc::clone(&ordering),
                &authority,
                1,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        let compilation = GeneratedAffineResidualCaseReeliminationCompiler::compile(
            family,
            context,
            authority,
            premises,
            ordering,
            schedule,
            GeneratedAffineResidualCaseReeliminationLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) =
            compilation
        else {
            continue;
        };
        let certificate = Arc::new(certificate);
        let mut retained_row_ordinal = 0;
        for (witness_ordinal, witness) in certificate.witnesses().iter().enumerate() {
            if !witness.outcome().is_retained() {
                continue;
            }
            rows.push(Arc::new(
                GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
                    family,
                    context,
                    Arc::clone(&certificate),
                    retained_row_ordinal,
                    witness_ordinal,
                    Arc::clone(frame),
                    GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
                )
                .unwrap(),
            ));
            retained_row_ordinal += 1;
        }
    }
    assert!(
        !rows.is_empty(),
        "the generated one-loop fixture produced no authenticated physical row"
    );
    rows
}

fn natural_one_loop_ready(name: &str, active: bool) -> NaturalOneLoopReadyFixture {
    let (family, context, plan) = one_loop_plan(name, active);
    let sources = production_rows(&family, &context, &plan);
    let target_offsets = plan
        .targets()
        .iter()
        .map(|locator| {
            (
                *locator,
                plan.physical_frame()
                    .anchor_offset(locator.inventory_position(), locator.case_ordinal())
                    .unwrap()
                    .values()
                    .to_vec(),
            )
        })
        .collect::<Vec<_>>();

    let session = GeneratedAffineResidualGroupExactSession::try_new(
        &family,
        &context,
        Arc::clone(&plan),
        211,
        GeneratedAffineResidualGroupExactSessionLimits::default(),
    )
    .unwrap();
    let before_recenter = SessionSnapshot::capture(&session);
    let mut rejected = Vec::new();
    for source in sources {
        let replayed_source = source
            .replay_for_database(&family, &context, plan.physical_frame())
            .unwrap();
        let source_terms = replayed_source
            .terms()
            .map(|(key, coefficient)| (key.shift().values().to_vec(), format!("{coefficient:?}")))
            .collect::<Vec<_>>();
        drop(replayed_source);
        let transaction = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let outcome = session
            .recenter_staged_new_pivot(&family, &context, transaction)
            .unwrap();
        assert_eq!(SessionSnapshot::capture(&session), before_recenter);
        match outcome {
            GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) => {
                assert_eq!(ready.targets_consumed(), 0);
                return NaturalOneLoopReadyFixture {
                    family,
                    context,
                    plan,
                    source,
                    session,
                    ready,
                    before_recenter,
                };
            }
            other => rejected.push((format!("{other:?}"), source_terms)),
        }
    }
    panic!(
        "no natural generated one-loop physical row reached exact Ready; \
         free_positions={:?}; target_offsets={target_offsets:?}; rejected={rejected:?}",
        plan.free_positions(),
    )
}

#[test]
fn natural_generated_one_loop_row_reaches_exact_ready_without_mutation() {
    let fixture =
        natural_one_loop_ready("exact-ready-publication-natural-inactive-one-loop", false);
    assert_eq!(fixture.plan.physical_frame().arity(), 1);
    assert_eq!(fixture.source.targets_consumed(), 0);
    assert!(!fixture.source.publishes_rule());
    assert!(!fixture.source.infers_master());
    assert_eq!(fixture.ready.targets_consumed(), 0);
    assert_eq!(fixture.ready.source_ordinal(), 0);
    assert_eq!(fixture.ready.pivot_ordinal(), 0);
    assert!(!fixture.ready.terms().is_empty());
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();

    let outcome = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        fixture.ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    assert_eq!(outcome.targets_consumed(), 0);
    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
        ready_for_conditions,
    ) = outcome
    else {
        panic!("the natural compact affine geometry must pass exact descent analysis");
    };
    assert_eq!(
        ready_for_conditions.schema(),
        GENERATED_AFFINE_RESIDUAL_GROUP_READY_PUBLICATION_ANALYSIS_V2_SCHEMA,
    );
    let geometry = ready_for_conditions.geometry();
    assert_eq!(geometry.ambient_arity(), 1);
    assert_eq!(geometry.free_count(), 1);
    assert_eq!(geometry.matrix_entries_inspected(), 1);
    assert_eq!(geometry.selector_entries_inspected(), 1);
    assert_eq!(geometry.constant_rows(), 0);
    assert_eq!(geometry.symbolic_rows(), 1);
    assert!(
        !ready_for_conditions.descent().is_empty(),
        "a nontrivial generated Ready row must retain exact descent witnesses",
    );
    let locator = *ready_for_conditions.ready().target_locator();
    for witness in ready_for_conditions.descent() {
        let rhs = fixture
            .plan
            .physical_frame()
            .key_for_exact_local(
                locator.inventory_position(),
                locator.case_ordinal(),
                ready_for_conditions.ready().terms()[witness.term_ordinal()]
                    .shift()
                    .values(),
            )
            .unwrap();
        assert!(witness.first_decisive_component().is_some());
        assert!(witness.replay(&rhs, ready_for_conditions.source_key()));
    }
    let stats = ready_for_conditions.stats();
    assert_eq!(
        stats.physical_key_component_scans(),
        stats.physical_keys_constructed() * stats.arity(),
    );
    assert!(
        stats.physical_key_retained_integer_bits()
            <= stats.physical_key_prospective_retained_integer_bits(),
    );
    assert!(
        stats.key_comparison_integer_bit_work()
            <= stats.key_prospective_comparison_integer_bit_work(),
    );
    assert!(
        ready_for_conditions.ready().terms()[ready_for_conditions.pivot_term_ordinal()]
            .shift()
            .values()
            .iter()
            .all(Integer::is_zero),
        "the retained pivot ordinal must name the unique centered zero shift",
    );
    assert_eq!(ready_for_conditions.targets_consumed(), 0);
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    ready_for_conditions
        .replay(&fixture.family, &fixture.context, &fixture.session)
        .unwrap();
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();
}

#[test]
fn active_n_one_point_is_honestly_not_a_generated_pivot_target() {
    let (family, context, plan) = one_loop_plan("exact-ready-publication-active-n-one-point", true);
    let sources = production_rows(&family, &context, &plan);
    let session = GeneratedAffineResidualGroupExactSession::try_new(
        &family,
        &context,
        Arc::clone(&plan),
        212,
        GeneratedAffineResidualGroupExactSessionLimits::default(),
    )
    .unwrap();
    let before = SessionSnapshot::capture(&session);
    for source in sources {
        let transaction = session
            .stage_replayed_row(&family, &context, &source)
            .unwrap();
        let outcome = session
            .recenter_staged_new_pivot(&family, &context, transaction)
            .unwrap();
        let GeneratedAffineResidualGroupExactSessionRecenterOutcome::NoTarget(no_target) = outcome
        else {
            panic!("the active n=1 residual must not be manufactured into exact Ready");
        };
        assert_eq!(no_target.targets_consumed(), 0);
        assert_eq!(SessionSnapshot::capture(&session), before);
    }
    session.replay(&family, &context).unwrap();
}

#[test]
fn one_below_analysis_limit_returns_the_exact_ready_for_lossless_retry() {
    let fixture = natural_one_loop_ready(
        "exact-ready-publication-natural-one-below-recoverable",
        false,
    );
    assert!(fixture.ready.terms().len() > 1);
    let mut limits = GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
    limits.max_key_comparisons = 0;
    let failure = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        fixture.ready,
        limits,
    )
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceLimit {
            resource: "Ready analysis key comparisons",
            requested: 1,
            limit: 0,
        }
    ));
    let (_, ready) = failure.into_parts();
    assert_eq!(ready.targets_consumed(), 0);
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();

    let retry = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        retry,
        GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(_)
    ));
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
}

#[test]
fn aggregate_retained_cap_is_exact_recoverable_and_nonmutating() {
    let fixture = natural_one_loop_ready(
        "exact-ready-publication-natural-aggregate-retained-cap",
        false,
    );
    let baseline = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        fixture.ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(baseline) =
        baseline
    else {
        panic!("the natural independent cylinder must pass baseline analysis");
    };
    let peak = baseline.stats().peak_prospective_retained_bytes();
    assert!(peak > 0);
    assert!(baseline.stats().retained_bytes() <= peak);
    let ready = baseline.into_ready();
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );

    let mut one_below = GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
    one_below.max_retained_bytes = peak - 1;
    let failure = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        ready,
        one_below,
    )
    .unwrap_err();
    assert_eq!(
        failure.error(),
        GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceLimit {
            resource: "Ready analysis retained bytes",
            requested: peak,
            limit: peak - 1,
        }
    );
    let (_, ready) = failure.into_parts();
    assert_eq!(ready.targets_consumed(), 0);
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();

    let mut exact = GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
    exact.max_retained_bytes = peak;
    let retry = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        ready,
        exact,
    )
    .unwrap();
    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(retry) =
        retry
    else {
        panic!("the exact aggregate retained boundary must succeed");
    };
    assert_eq!(retry.stats().peak_prospective_retained_bytes(), peak);
    assert_eq!(retry.targets_consumed(), 0);
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    retry
        .replay(&fixture.family, &fixture.context, &fixture.session)
        .unwrap();
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();
}

#[test]
fn physical_key_preflight_counters_are_exact_recoverable_and_nonmutating() {
    let fixture = natural_one_loop_ready(
        "exact-ready-publication-natural-key-preflight-resource-counters",
        false,
    );
    let baseline = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        fixture.ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(baseline) =
        baseline
    else {
        panic!("the natural independent cylinder must pass baseline analysis");
    };
    let stats = baseline.stats();
    let mut ready = baseline.into_ready();

    macro_rules! exact_and_one_below {
        ($getter:ident, $field:ident, $resource:literal) => {{
            let exact_value = stats.$getter();
            assert!(exact_value > 0, "{} must be exercised", $resource);

            let mut one_below =
                GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
            one_below.$field = exact_value - 1;
            let failure = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
                &fixture.family,
                &fixture.context,
                &fixture.session,
                ready,
                one_below,
            )
            .unwrap_err();
            assert_eq!(
                failure.error(),
                GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceLimit {
                    resource: $resource,
                    requested: exact_value,
                    limit: exact_value - 1,
                }
            );
            ready = failure.into_parts().1;
            assert_eq!(
                SessionSnapshot::capture(&fixture.session),
                fixture.before_recenter,
            );

            let mut exact = GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
            exact.$field = exact_value;
            let outcome = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
                &fixture.family,
                &fixture.context,
                &fixture.session,
                ready,
                exact,
            )
            .unwrap();
            let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
                accepted,
            ) = outcome
            else {
                panic!("the exact {} boundary must succeed", $resource);
            };
            assert_eq!(accepted.stats().$getter(), exact_value);
            ready = accepted.into_ready();
        }};
    }

    exact_and_one_below!(
        physical_key_component_scans,
        max_physical_key_component_scans,
        "Ready analysis physical-key component scans"
    );
    exact_and_one_below!(
        physical_key_construction_integer_bit_work,
        max_physical_key_construction_integer_bit_work,
        "Ready analysis physical-key construction integer-bit work"
    );
    exact_and_one_below!(
        physical_key_prospective_retained_integer_bits,
        max_physical_key_prospective_retained_integer_bits,
        "Ready analysis prospective physical-key retained integer bits"
    );
    exact_and_one_below!(
        physical_key_retained_integer_bits,
        max_physical_key_retained_integer_bits,
        "Ready analysis physical-key retained integer bits"
    );
    exact_and_one_below!(
        key_prospective_comparison_integer_bit_work,
        max_key_prospective_comparison_integer_bit_work,
        "Ready analysis prospective key-comparison integer-bit work"
    );

    drop(ready);
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
}

#[test]
fn compact_geometry_resource_counters_are_exact_recoverable_and_nonmutating() {
    let fixture = natural_one_loop_ready(
        "exact-ready-publication-natural-compact-geometry-resource-counters",
        false,
    );
    let baseline = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        fixture.ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(baseline) =
        baseline
    else {
        panic!("the natural compact affine geometry must pass baseline analysis");
    };
    let stats = baseline.stats();
    let mut ready = baseline.into_ready();

    macro_rules! exact_and_one_below {
        ($getter:ident, $field:ident, $resource:literal) => {{
            let exact_value = stats.$getter();
            assert!(exact_value > 0, "{} must be exercised", $resource);

            let mut one_below =
                GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
            one_below.$field = exact_value - 1;
            let failure = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
                &fixture.family,
                &fixture.context,
                &fixture.session,
                ready,
                one_below,
            )
            .unwrap_err();
            assert_eq!(
                failure.error(),
                GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceLimit {
                    resource: $resource,
                    requested: exact_value,
                    limit: exact_value - 1,
                }
            );
            ready = failure.into_parts().1;
            assert_eq!(
                SessionSnapshot::capture(&fixture.session),
                fixture.before_recenter,
            );
            fixture
                .session
                .replay(&fixture.family, &fixture.context)
                .unwrap();

            let mut exact = GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
            exact.$field = exact_value;
            let outcome = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
                &fixture.family,
                &fixture.context,
                &fixture.session,
                ready,
                exact,
            )
            .unwrap();
            let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
                accepted,
            ) = outcome
            else {
                panic!("the exact {} boundary must succeed", $resource);
            };
            assert_eq!(accepted.stats().$getter(), exact_value);
            assert_eq!(
                SessionSnapshot::capture(&fixture.session),
                fixture.before_recenter,
            );
            accepted
                .replay(&fixture.family, &fixture.context, &fixture.session)
                .unwrap();
            ready = accepted.into_ready();
        }};
    }

    exact_and_one_below!(
        free_positions_inspected,
        max_free_positions_inspected,
        "Ready analysis free positions inspected"
    );
    exact_and_one_below!(
        matrix_entries_inspected,
        max_matrix_entries_inspected,
        "Ready analysis matrix entries inspected"
    );
    exact_and_one_below!(
        selector_entries_inspected,
        max_selector_entries_inspected,
        "Ready analysis selector entries inspected"
    );
    exact_and_one_below!(
        geometry_integer_bit_work,
        max_geometry_integer_bit_work,
        "Ready analysis geometry integer-bit work"
    );
    exact_and_one_below!(
        geometry_witness_bytes,
        max_geometry_witness_bytes,
        "Ready analysis geometry witness bytes"
    );

    drop(ready);
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
}

#[test]
fn six_loop_arity_21_parametric_generator_is_topology_neutral_and_replay_stable() {
    const ARITY: usize = 21;
    const ORDINARY_ROWS: usize = 36;

    let family =
        six_loop_unit_mass_coordinate_basis("exact-ready-publication-six-loop-generator-only-gate");
    assert_eq!(family.loop_count(), 6);
    assert_eq!(family.external_count(), 0);
    assert_eq!(family.coordinates().len(), ARITY);
    assert_eq!(family.denominator_count(), ARITY);

    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    assert_eq!(generator.context().index_count(), ARITY);
    let generated = generator.generate_ordinary_ibp().unwrap();
    assert_eq!(generated.len(), ORDINARY_ROWS);
    for (ordinal, row) in generated.iter().enumerate() {
        assert_eq!(row.arity(), ARITY);
        assert_eq!(row.context_fingerprint(), generator.context().fingerprint());
        assert_eq!(row.family_fingerprint(), family.fingerprint());
        assert_eq!(
            row.row_id(),
            &crate::ParametricRowId::OrdinaryIbp {
                contraction_momentum: ordinal / family.loop_count(),
                differentiated_loop: ordinal % family.loop_count(),
            },
        );
    }

    // A second independent generation through the public generic entry point
    // must replay byte-for-byte in canonical relation manifests.
    let replayed = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate_ordinary_ibp()
        .unwrap();
    assert_eq!(replayed, generated);
    assert!(
        generated
            .iter()
            .zip(&replayed)
            .all(|(left, right)| left.stable_manifest() == right.stable_manifest())
    );
}

#[test]
fn foreign_family_binding_is_rejected_without_consuming_ready_or_session_state() {
    let fixture =
        natural_one_loop_ready("exact-ready-publication-authenticated-binding-owner", false);
    let foreign_family = massive_tadpole("exact-ready-publication-authenticated-binding-foreign");
    let foreign_context = ParametricIbpGenerator::try_new(&foreign_family)
        .unwrap()
        .context()
        .clone();
    let failure = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &foreign_family,
        &foreign_context,
        &fixture.session,
        fixture.ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        GeneratedAffineResidualGroupReadyPublicationAnalysisError::Session(_)
    ));
    let (_, ready) = failure.into_parts();
    assert_eq!(ready.targets_consumed(), 0);
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );

    let retry = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        retry,
        GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(_)
    ));
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();
}

#[test]
fn ready_v2_replay_rejects_value_equal_foreign_target_state_allocation() {
    let fixture = natural_one_loop_ready(
        "exact-ready-publication-v2-replay-foreign-allocation",
        false,
    );
    let owner_outcome = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &fixture.session,
        fixture.ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
        owner_certificate,
    ) = owner_outcome
    else {
        panic!("the owner Ready must pass V2 analysis");
    };

    // Deliberately reproduce every value-level input, including the numeric
    // session number, while allocating a fresh exact database/target state.
    let foreign_session = GeneratedAffineResidualGroupExactSession::try_new(
        &fixture.family,
        &fixture.context,
        Arc::clone(&fixture.plan),
        211,
        GeneratedAffineResidualGroupExactSessionLimits::default(),
    )
    .unwrap();
    let foreign_before = SessionSnapshot::capture(&foreign_session);
    let foreign_transaction = foreign_session
        .stage_replayed_row(&fixture.family, &fixture.context, &fixture.source)
        .unwrap();
    let GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(foreign_ready) =
        foreign_session
            .recenter_staged_new_pivot(&fixture.family, &fixture.context, foreign_transaction)
            .unwrap()
    else {
        panic!("the value-equal foreign session must independently reach Ready");
    };
    let owner_ready = owner_certificate.ready();
    assert_eq!(foreign_ready.stats(), owner_ready.stats());
    assert_eq!(foreign_ready.source_ordinal(), owner_ready.source_ordinal(),);
    assert_eq!(foreign_ready.pivot_ordinal(), owner_ready.pivot_ordinal(),);
    assert_eq!(foreign_ready.target_locator(), owner_ready.target_locator());
    assert_eq!(
        foreign_ready.target_premises(),
        owner_ready.target_premises()
    );
    assert_eq!(
        foreign_ready.coefficient_translation(),
        owner_ready.coefficient_translation(),
    );
    assert_eq!(foreign_ready.terms(), owner_ready.terms());
    assert_eq!(foreign_ready.row_guards(), owner_ready.row_guards());
    assert_eq!(
        foreign_ready.targets_consumed(),
        owner_ready.targets_consumed()
    );

    let owner_geometry = fixture
        .session
        .authenticated_ready_geometry(&fixture.family, &fixture.context, owner_ready)
        .unwrap();
    let foreign_geometry = foreign_session
        .authenticated_ready_geometry(&fixture.family, &fixture.context, &foreign_ready)
        .unwrap();
    assert!(Arc::ptr_eq(
        owner_geometry.frame(),
        foreign_geometry.frame()
    ));
    assert_eq!(owner_geometry.locator(), foreign_geometry.locator());
    assert_eq!(
        owner_geometry.ambient_arity(),
        foreign_geometry.ambient_arity()
    );
    assert_eq!(
        owner_geometry.free_positions(),
        foreign_geometry.free_positions()
    );
    assert_eq!(
        owner_geometry.compact_affine_matrix(),
        foreign_geometry.compact_affine_matrix(),
    );
    assert_eq!(
        owner_geometry.target_anchor(),
        foreign_geometry.target_anchor()
    );
    drop(owner_geometry);
    drop(foreign_geometry);
    assert_eq!(SessionSnapshot::capture(&foreign_session), foreign_before);

    let foreign_outcome = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &fixture.family,
        &fixture.context,
        &foreign_session,
        foreign_ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
        foreign_certificate,
    ) = foreign_outcome
    else {
        panic!("the value-equal foreign Ready must pass V2 analysis");
    };
    assert_eq!(foreign_certificate.schema(), owner_certificate.schema());
    assert_eq!(foreign_certificate.geometry(), owner_certificate.geometry());
    assert_eq!(
        foreign_certificate.pivot_term_ordinal(),
        owner_certificate.pivot_term_ordinal(),
    );
    assert_eq!(
        foreign_certificate.source_key(),
        owner_certificate.source_key()
    );
    assert_eq!(
        foreign_certificate.source_key().retained_integer_bits(),
        owner_certificate.source_key().retained_integer_bits(),
    );
    assert_eq!(
        foreign_certificate.source_key().retained_bytes(),
        owner_certificate.source_key().retained_bytes(),
    );
    assert_eq!(foreign_certificate.descent(), owner_certificate.descent());
    assert_eq!(foreign_certificate.hazards(), owner_certificate.hazards());
    assert_eq!(foreign_certificate.limits(), owner_certificate.limits());
    assert_eq!(foreign_certificate.stats(), owner_certificate.stats());

    assert_eq!(
        owner_certificate.replay(&fixture.family, &fixture.context, &foreign_session),
        Err(
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::Session(
                GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation,
            ),
        ),
    );
    assert_eq!(
        foreign_certificate.replay(&fixture.family, &fixture.context, &fixture.session),
        Err(
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::Session(
                GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation,
            ),
        ),
    );
    assert_eq!(SessionSnapshot::capture(&foreign_session), foreign_before);
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    foreign_session
        .replay(&fixture.family, &fixture.context)
        .unwrap();
    fixture
        .session
        .replay(&fixture.family, &fixture.context)
        .unwrap();

    owner_certificate
        .replay(&fixture.family, &fixture.context, &fixture.session)
        .unwrap();
    foreign_certificate
        .replay(&fixture.family, &fixture.context, &foreign_session)
        .unwrap();
    assert_eq!(
        SessionSnapshot::capture(&fixture.session),
        fixture.before_recenter,
    );
    assert_eq!(SessionSnapshot::capture(&foreign_session), foreign_before);
}

#[test]
fn exact_analysis_preserves_a_rhs_shift_beyond_machine_integer_width() {
    let (family, context, plan) =
        one_loop_plan("exact-ready-publication-arbitrary-precision-rhs", false);
    let frame = plan.physical_frame();
    let locator = plan.targets()[0];
    let target_values = frame
        .anchor_offset(locator.inventory_position(), locator.case_ordinal())
        .unwrap()
        .values()
        .to_vec();
    let huge = (Integer::one() << 4096_u32) + Integer::from(17);
    let mut plus_values = target_values.clone();
    plus_values[0] = &plus_values[0] + &huge;
    let mut minus_values = target_values.clone();
    minus_values[0] = &minus_values[0] - &huge;

    let target_key = frame
        .test_key_for_borrowed_physical_values(&target_values)
        .unwrap();
    let plus_key = frame
        .test_key_for_borrowed_physical_values(&plus_values)
        .unwrap();
    let minus_key = frame
        .test_key_for_borrowed_physical_values(&minus_values)
        .unwrap();
    assert!(
        plus_key < target_key,
        "the V1 inactive plus-q neighbor must be descending",
    );
    assert!(minus_key > target_key);
    let rhs_key = plus_key;

    let session = GeneratedAffineResidualGroupExactSession::try_new(
        &family,
        &context,
        Arc::clone(&plan),
        213,
        GeneratedAffineResidualGroupExactSessionLimits::default(),
    )
    .unwrap();
    let before = SessionSnapshot::capture(&session);
    let transaction = session
        .stage_authenticated_terms_for_test(
            &context,
            vec![(rhs_key, context.one()), (target_key, context.one())],
            Vec::new(),
        )
        .unwrap();
    let GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) = session
        .recenter_staged_new_pivot(&family, &context, transaction)
        .unwrap()
    else {
        panic!("the authenticated target pivot must reach exact Ready");
    };
    assert!(
        ready
            .terms()
            .iter()
            .flat_map(|term| term.shift().values())
            .any(|value| value == &huge || value == &(-&huge))
    );
    assert_eq!(SessionSnapshot::capture(&session), before);

    let outcome = GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
        &family,
        &context,
        &session,
        ready,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
    )
    .unwrap();
    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
        ready_for_conditions,
    ) = outcome
    else {
        panic!("the exact descending high-bit row must pass analysis");
    };
    assert!(
        ready_for_conditions
            .stats()
            .key_comparison_integer_bit_work()
            > 4096
    );
    assert_eq!(ready_for_conditions.stats().hazard_integer_operations(), 2);
    assert_eq!(
        ready_for_conditions.stats().hazard_integer_bit_work(),
        16_391,
        "the exact work census must include both one-bit constant operands",
    );
    let hazard = ready_for_conditions
        .hazards()
        .iter()
        .find(|hazard| hazard.count() == &huge)
        .expect("the inactive plus-q shift must retain one exact hazard range");
    assert_eq!(hazard.first() + hazard.count(), Integer::one());
    assert_eq!(hazard.last(), &Integer::zero());
    assert_eq!(SessionSnapshot::capture(&session), before);
    session.replay(&family, &context).unwrap();
}

#[test]
fn active_global_coverage_reduces_dots_and_leaves_only_the_n_one_residual() {
    let family = massive_tadpole("exact-ready-publication-active-global-split");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    assert_eq!(discovery.row_span().rows().len(), 1);

    let n_one = discovery
        .coverage()
        .classification_for_indices(&context, &[1])
        .unwrap()
        .expect("n=1 belongs to the active orthant");
    assert!(matches!(
        n_one.disposition(),
        ParametricSectorLeafDisposition::Uncovered
            | ParametricSectorLeafDisposition::Unsupported { .. }
    ));
    for power in [2, 3, 4] {
        assert!(matches!(
            discovery
                .coverage()
                .classification_for_indices(&context, &[power])
                .unwrap()
                .expect("positive power belongs to the active orthant")
                .disposition(),
            ParametricSectorLeafDisposition::DescendingRule { .. }
        ));
    }

    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 1;
    queue_limits.max_translation_points = 3;
    let queue =
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap();
    assert_eq!(queue.work_items().len(), 1);
    let residual = &queue.work_items()[0];
    assert_eq!(residual.source_case(), n_one.case());
    assert_eq!(residual.extraction().assignment().entries(), &[(0, 1)]);
    queue.replay(&family, &context).unwrap();
}

#[test]
fn combined_generated_global_provider_reduces_tadpole_powers_two_through_four() {
    let family = massive_tadpole("exact-ready-publication-combined-global-provider");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits.discovery.adaptive.max_search_depth = 0;
    family_limits.live_leaf_queue.translation_radius = 0;
    family_limits.live_leaf_queue.max_translation_points = 1;
    let certificate = GeneratedFamilyRuleSystemCompiler::compile(
        &family,
        &context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        family_limits,
    )
    .unwrap();
    let master = ConcreteIntegralKey::try_new([1]).unwrap();
    let provider = GeneratedFamilyRuleSystemProvider::try_with_selected(
        &family,
        &context,
        certificate,
        [master.clone()],
        GeneratedFamilyRuleSystemProviderLimits::default(),
    )
    .unwrap();
    provider.replay().unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    for (power, expected) in [
        (2, "(d-2)/(2*m2)"),
        (3, "(d-4)*(d-2)/(8*m2^2)"),
        (4, "(d-6)*(d-4)*(d-2)/(48*m2^3)"),
    ] {
        let result = engine
            .reduce(&ConcreteIntegralKey::try_new([power]).unwrap())
            .unwrap();
        result.require_complete().unwrap();
        assert_eq!(result.terms().len(), 1);
        assert_eq!(
            result.terms().get(&master),
            Some(&family.coefficient_context().parse(expected).unwrap()),
            "generated global reduction differs from the frozen Vakint oracle at power {power}",
        );
    }
}

#[test]
fn current_lineage_test_must_not_launder_ready_into_the_old_matcher() {
    let test_source = include_str!("generated_affine_residual_group_ready_publication_tests.rs");
    let production_source = include_str!("generated_affine_residual_group_ready_publication.rs");
    let old_compiler = ["GeneratedResidualAffineWhenBad", "Compiler::compile"].concat();
    let narrowing_conversion = ["try_to_", "index_shift"].concat();
    let i64_conversion = ["to_", "i64"].concat();
    for (label, source) in [
        ("independent validation", test_source),
        ("current-lineage production", production_source),
    ] {
        assert!(
            !source.contains(&old_compiler),
            "{label} must not adapt exact Ready into the old matcher",
        );
        assert!(
            !source.contains(&narrowing_conversion) && !source.contains(&i64_conversion),
            "{label} must keep exact offsets as Symbolica Integers",
        );
    }
    assert!(!production_source.contains("loop_count()"));
    assert!(!production_source.contains("tadpole"));
}
