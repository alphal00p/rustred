//! Concrete validation fixtures for the generic raw-family source-set layer.
//!
//! No recurrence, pivot, master, loop-count branch, or expected reduction is
//! supplied to production.  The massive tadpole and equal-mass sunset merely
//! exercise topology-derived inventory, Symbolica-generated IBPs, and the
//! generic cylindrical composition path.

use std::mem::size_of;
use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA,
    GeneratedCylindricalFamilySourceSetCertificate, GeneratedCylindricalFamilySourceSetCompiler,
    GeneratedCylindricalFamilySourceSetError, GeneratedCylindricalFamilySourceSetLimits,
    GeneratedCylindricalPersistentEliminationCertificate, GeneratedSymbolicRowSpanConfig,
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpConfig,
    ParametricIbpGenerator, PowerShiftPolicy, SectorRestrictions,
};

const THROUGH_DEPTH: usize = 1;

fn tadpole(name: &str, massive: bool) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            if massive {
                coefficients.parse("-m2").unwrap()
            } else {
                coefficients.zero()
            },
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

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

fn context(family: &IntegralFamily) -> ParametricCoefficientContext {
    ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone()
}

fn compile(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    limits: GeneratedCylindricalFamilySourceSetLimits,
) -> Result<GeneratedCylindricalFamilySourceSetCertificate, GeneratedCylindricalFamilySourceSetError>
{
    compile_with_row_span(
        family,
        context,
        GeneratedSymbolicRowSpanConfig::default(),
        limits,
    )
}

fn compile_with_row_span(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    row_span: GeneratedSymbolicRowSpanConfig,
    limits: GeneratedCylindricalFamilySourceSetLimits,
) -> Result<GeneratedCylindricalFamilySourceSetCertificate, GeneratedCylindricalFamilySourceSetError>
{
    GeneratedCylindricalFamilySourceSetCompiler::compile(
        family,
        context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        ParametricIbpConfig::default(),
        row_span,
        THROUGH_DEPTH,
        limits,
    )
}

#[derive(Clone, Copy, Debug)]
enum AggregateCap {
    PreparePoints,
    ExpandedRows,
    RetainedRows,
    PersistentEvents,
    PersistentPivots,
}

impl AggregateCap {
    fn resource(self) -> &'static str {
        match self {
            Self::PreparePoints => "family cylindrical total prepare points",
            Self::ExpandedRows => "family cylindrical total expanded rows",
            Self::RetainedRows => "family cylindrical total retained rows",
            Self::PersistentEvents => "family cylindrical total persistent events",
            Self::PersistentPivots => "family cylindrical total persistent pivots",
        }
    }

    fn exact(self, certificate: &GeneratedCylindricalFamilySourceSetCertificate) -> usize {
        let stats = certificate.stats();
        match self {
            Self::PreparePoints => stats.total_prepare_points(),
            Self::ExpandedRows => stats.total_expanded_rows(),
            Self::RetainedRows => stats.total_retained_rows(),
            Self::PersistentEvents => stats.total_persistent_events(),
            Self::PersistentPivots => stats.total_persistent_pivots(),
        }
    }

    fn set(self, limits: &mut GeneratedCylindricalFamilySourceSetLimits, value: usize) {
        match self {
            Self::PreparePoints => limits.max_total_prepare_points = value,
            Self::ExpandedRows => limits.max_total_expanded_rows = value,
            Self::RetainedRows => limits.max_total_retained_rows = value,
            Self::PersistentEvents => limits.max_total_persistent_events = value,
            Self::PersistentPivots => limits.max_total_persistent_pivots = value,
        }
    }

    /// Make a later child stage fail independently if the family aggregate is
    /// checked only after a complete source has already been constructed.
    fn install_hostile_later_limit(self, limits: &mut GeneratedCylindricalFamilySourceSetLimits) {
        match self {
            Self::PreparePoints | Self::ExpandedRows => limits.row_system.max_source_rows = 0,
            Self::RetainedRows | Self::PersistentEvents => limits.persistent.max_events = 0,
            Self::PersistentPivots => limits.persistent.max_pivot_assumption_closures = 0,
        }
    }
}

fn exact_aggregate_limits(
    certificate: &GeneratedCylindricalFamilySourceSetCertificate,
) -> GeneratedCylindricalFamilySourceSetLimits {
    let stats = certificate.stats();
    let mut limits = certificate.limits();
    limits.max_total_prepare_points = stats.total_prepare_points();
    limits.max_total_expanded_rows = stats.total_expanded_rows();
    limits.max_total_retained_rows = stats.total_retained_rows();
    limits.max_total_persistent_events = stats.total_persistent_events();
    limits.max_total_persistent_pivots = stats.total_persistent_pivots();
    limits.max_binding_bytes = stats.binding_bytes();
    limits.max_replay_comparison_units = stats.replay_comparison_units();
    limits
}

fn one_loop_baseline() -> (
    IntegralFamily,
    ParametricCoefficientContext,
    GeneratedCylindricalFamilySourceSetCertificate,
) {
    let family = tadpole("family-source-set-aggregate-cap-tadpole", true);
    let context = context(&family);
    let certificate = compile(
        &family,
        &context,
        GeneratedCylindricalFamilySourceSetLimits::default(),
    )
    .unwrap();
    (family, context, certificate)
}

#[track_caller]
fn assert_one_below_aggregate_cap(cap: AggregateCap) {
    let (family, context, baseline) = one_loop_baseline();
    let exact = cap.exact(&baseline);
    assert!(exact > 0, "{cap:?} fixture census must be nonzero");

    let mut limits = exact_aggregate_limits(&baseline);
    cap.set(&mut limits, exact - 1);
    cap.install_hostile_later_limit(&mut limits);
    let error = compile(&family, &context, limits).unwrap_err();
    match error {
        GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource,
            requested,
            limit,
        } => {
            assert_eq!(resource, cap.resource());
            assert_eq!(requested, exact);
            assert_eq!(limit, exact - 1);
        }
        other => panic!("expected the {cap:?} family resource limit, got {other:?}"),
    }
}

#[test]
fn one_loop_source_set_is_generic_replayable_and_scope_bounded() {
    let family = tadpole("family-source-set-massive-tadpole", true);
    let context = context(&family);
    let certificate = compile(
        &family,
        &context,
        GeneratedCylindricalFamilySourceSetLimits::default(),
    )
    .unwrap();
    assert_eq!(
        certificate
            .solve_order()
            .iter()
            .map(|sector| sector.to_bit_string())
            .collect::<Vec<_>>(),
        ["1"]
    );
    assert_eq!(certificate.persistent_sources().len(), 1);
    assert_eq!(certificate.stats().shared_row_spans(), 1);
    assert_eq!(certificate.stats().generated_symbolic_rows(), 1);
    assert_eq!(certificate.stats().source_scope_entries(), 1);
    assert_eq!(
        certificate.stats().source_index_bytes(),
        size_of::<rustred::SectorMask>()
            + size_of::<Arc<GeneratedCylindricalPersistentEliminationCertificate>>()
            + size_of::<bool>()
    );
    let root = certificate.persistent_sources()[0]
        .row_system()
        .start()
        .sector_root_start()
        .unwrap();
    assert!(Arc::ptr_eq(
        root.inventory_arc(),
        certificate.inventory_arc()
    ));
    assert!(Arc::ptr_eq(
        root.row_span_arc(),
        certificate.row_span_arc().unwrap()
    ));
    certificate.replay(&family, &context).unwrap();

    let mut exact = GeneratedCylindricalFamilySourceSetLimits::default();
    exact.max_source_scope_entries = certificate.stats().source_scope_entries();
    exact.max_source_index_bytes = certificate.stats().source_index_bytes();
    let exactly_bounded = compile(&family, &context, exact).unwrap();
    assert_eq!(
        exactly_bounded.stats().source_scope_entries(),
        exact.max_source_scope_entries
    );
    assert_eq!(
        exactly_bounded.stats().source_index_bytes(),
        exact.max_source_index_bytes
    );

    let mut scope_one_below = exact;
    scope_one_below.max_source_scope_entries -= 1;
    assert!(matches!(
        compile(&family, &context, scope_one_below),
        Err(GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource: "family cylindrical source-scope entries",
            requested: 1,
            limit: 0,
        })
    ));
    let mut index_one_below = exact;
    index_one_below.max_source_index_bytes -= 1;
    assert!(matches!(
        compile(&family, &context, index_one_below),
        Err(GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource: "family cylindrical source-index bytes",
            requested,
            limit,
        }) if requested == certificate.stats().source_index_bytes()
            && limit + 1 == requested
    ));

    let foreign_family = tadpole("family-source-set-foreign-tadpole", true);
    assert!(matches!(
        certificate.replay(&foreign_family, &context),
        Err(GeneratedCylindricalFamilySourceSetError::WrongFamily)
    ));
    let foreign_context = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "family-source-set-foreign-context",
        family.denominator_count(),
    )
    .unwrap();
    assert!(matches!(
        certificate.replay(&family, &foreign_context),
        Err(GeneratedCylindricalFamilySourceSetError::WrongContext)
    ));
}

#[test]
fn one_loop_family_aggregate_caps_and_binding_accept_exact_boundaries() {
    let (family, context, baseline) = one_loop_baseline();
    let baseline_stats = baseline.stats();
    assert!(baseline_stats.total_prepare_points() > 0);
    assert!(baseline_stats.total_expanded_rows() > 0);
    assert!(baseline_stats.total_retained_rows() > 0);
    assert!(baseline_stats.total_persistent_events() > 0);
    assert!(baseline_stats.total_persistent_pivots() > 0);
    assert!(baseline_stats.binding_bytes() > 0);

    let exact = exact_aggregate_limits(&baseline);
    let exactly_bounded = compile(&family, &context, exact).unwrap();
    assert_eq!(exactly_bounded.limits(), exact);
    assert_eq!(exactly_bounded.stats(), baseline_stats);
    exactly_bounded.replay(&family, &context).unwrap();
}

#[test]
fn one_loop_total_prepare_points_one_below_precedes_row_system() {
    assert_one_below_aggregate_cap(AggregateCap::PreparePoints);
}

#[test]
fn one_loop_total_expanded_rows_one_below_precedes_row_system() {
    assert_one_below_aggregate_cap(AggregateCap::ExpandedRows);
}

#[test]
fn one_loop_total_retained_rows_one_below_precedes_persistent_build() {
    assert_one_below_aggregate_cap(AggregateCap::RetainedRows);
}

#[test]
fn one_loop_total_persistent_events_one_below_precedes_persistent_build() {
    assert_one_below_aggregate_cap(AggregateCap::PersistentEvents);
}

#[test]
fn one_loop_total_persistent_pivots_one_below_precedes_closure_build() {
    assert_one_below_aggregate_cap(AggregateCap::PersistentPivots);
}

#[test]
fn one_loop_binding_one_below_precedes_hostile_row_span_limits() {
    let (family, context, baseline) = one_loop_baseline();
    let exact_binding = baseline.stats().binding_bytes();
    assert!(exact_binding > 0);

    let mut limits = exact_aggregate_limits(&baseline);
    limits.max_binding_bytes = exact_binding - 1;
    let mut hostile_row_span = GeneratedSymbolicRowSpanConfig::default();
    hostile_row_span.limits.max_canonical_rows = 0;
    hostile_row_span.limits.max_augmented_rows = 0;
    let error = compile_with_row_span(&family, &context, hostile_row_span, limits).unwrap_err();
    assert!(matches!(
        error,
        GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource: "family cylindrical binding bytes",
            requested,
            limit,
        } if requested == exact_binding && limit + 1 == requested
    ));
}

#[test]
fn one_loop_replay_census_one_below_precedes_hostile_root_limits() {
    let (family, context, baseline) = one_loop_baseline();
    let exact_replay_units = baseline.stats().replay_comparison_units();
    assert!(exact_replay_units > 0);

    let mut limits = exact_aggregate_limits(&baseline);
    limits.max_replay_comparison_units = exact_replay_units - 1;
    // Root construction would fail independently if the family shallow gate
    // did not win immediately after row-span construction.
    limits.sector_root.max_prepare_points = 0;
    let error = compile(&family, &context, limits).unwrap_err();
    assert!(matches!(
        error,
        GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource: "family cylindrical logical replay comparison units",
            requested,
            limit,
        } if requested == exact_replay_units && limit + 1 == requested
    ));
}

#[test]
fn empty_unresolved_queue_skips_symbolic_row_span_entirely() {
    let family = tadpole("family-source-set-massless-tadpole", false);
    let context = context(&family);
    let mut hostile_row_span = GeneratedSymbolicRowSpanConfig::default();
    hostile_row_span.limits.max_canonical_rows = 0;
    hostile_row_span.limits.max_augmented_rows = 0;
    let certificate = compile_with_row_span(
        &family,
        &context,
        hostile_row_span,
        GeneratedCylindricalFamilySourceSetLimits::default(),
    )
    .unwrap();
    assert!(certificate.solve_order().is_empty());
    assert!(certificate.persistent_sources().is_empty());
    assert!(certificate.row_span_arc().is_none());
    assert_eq!(certificate.stats().shared_row_spans(), 0);
    assert_eq!(certificate.stats().generated_symbolic_rows(), 0);
    assert!(certificate.stats().replay_comparison_units() >= 24);
    certificate.replay(&family, &context).unwrap();

    let exact_replay_units = certificate.stats().replay_comparison_units();
    let mut exact = certificate.limits();
    exact.max_replay_comparison_units = exact_replay_units;
    compile_with_row_span(&family, &context, hostile_row_span, exact).unwrap();

    let mut one_below = exact;
    one_below.max_replay_comparison_units = exact_replay_units - 1;
    assert!(matches!(
        compile_with_row_span(&family, &context, hostile_row_span, one_below),
        Err(GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource: "family cylindrical logical replay comparison units",
            requested,
            limit,
        }) if requested == exact_replay_units && limit + 1 == requested
    ));
}

#[test]
fn sunset_preserves_raw_solve_order_exact_shared_arcs_and_v3_sources() {
    let family = equal_mass_sunset("family-source-set-equal-mass-sunset");
    let context = context(&family);
    let certificate = compile(
        &family,
        &context,
        GeneratedCylindricalFamilySourceSetLimits::default(),
    )
    .unwrap();
    assert_eq!(
        certificate
            .solve_order()
            .iter()
            .map(|sector| sector.to_bit_string())
            .collect::<Vec<_>>(),
        ["011", "101", "110", "111"]
    );
    assert_eq!(certificate.persistent_sources().len(), 4);
    assert_eq!(certificate.source_budgets().len(), 4);
    assert_eq!(certificate.row_span_arc().unwrap().rows().len(), 4);
    let limits = certificate.limits();
    let mut prepare_remaining = limits.max_total_prepare_points;
    let mut expanded_remaining = limits.max_total_expanded_rows;
    let mut retained_remaining = limits.max_total_retained_rows;
    let mut event_remaining = limits.max_total_persistent_events;
    let mut pivot_remaining = limits.max_total_persistent_pivots;
    for ((sector, budget), source) in certificate
        .solve_order()
        .iter()
        .zip(certificate.source_budgets())
        .zip(certificate.persistent_sources())
    {
        assert_eq!(budget.prepare_points_remaining(), prepare_remaining);
        assert_eq!(budget.expanded_rows_remaining(), expanded_remaining);
        assert_eq!(budget.retained_rows_remaining(), retained_remaining);
        assert_eq!(budget.persistent_events_remaining(), event_remaining);
        assert_eq!(budget.persistent_pivots_remaining(), pivot_remaining);
        assert_eq!(
            source.schema(),
            GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA
        );
        let root = source.row_system().start().sector_root_start().unwrap();
        assert_eq!(root.sector(), sector);
        assert!(root.assignment().is_empty());
        assert!(Arc::ptr_eq(
            root.inventory_arc(),
            certificate.inventory_arc()
        ));
        assert!(Arc::ptr_eq(
            root.row_span_arc(),
            certificate.row_span_arc().unwrap()
        ));
        prepare_remaining -= root.stats().prepare_points();
        expanded_remaining -= source.row_system().stats().expanded_rows();
        retained_remaining -= source.row_system().stats().retained_rows();
        event_remaining -= source.events().len();
        pivot_remaining -= source.stats().pivot_rows();
    }
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn sunset_source_count_is_preflighted_before_row_span_generation() {
    let family = equal_mass_sunset("family-source-set-sunset-count-preflight");
    let context = context(&family);
    let mut limits = GeneratedCylindricalFamilySourceSetLimits::default();
    limits.max_sources = 3;
    // If generation were entered first this independent cap would fail with a
    // row-span error.  The family source-count error must win deterministically.
    let mut row_span = GeneratedSymbolicRowSpanConfig::default();
    row_span.limits.max_canonical_rows = 0;
    let error = GeneratedCylindricalFamilySourceSetCompiler::compile(
        &family,
        &context,
        SectorRestrictions::unrestricted(3).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        ParametricIbpConfig::default(),
        row_span,
        THROUGH_DEPTH,
        limits,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource: "family cylindrical persistent sources",
            requested: 4,
            limit: 3,
        }
    ));
}
