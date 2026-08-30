use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::cell::{RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRuleLimits, derive_sector_interior_rule, derive_sector_monotone_rule_for_target,
};
use crate::identity::{IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits};
use crate::sector::symmetry::permutation::compile;
use crate::sector::symmetry::{
    CanonicalizationLimits, Canonicalizer, CoefficientMatrix, Limits as SymmetryLimits,
    MomentumMap, verify,
};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::{
    ReachabilityDisposition, ReachabilityError, ReachabilityLimits, ReachabilityPlanner,
    ReachabilityTerminal, ReachabilityTerminalKind,
};

fn terminal(kind: ReachabilityTerminalKind, ordinal: usize) -> ReachabilityTerminal {
    ReachabilityTerminal::new(kind, ordinal)
}

fn tadpole_cell(
    dimension: Option<i64>,
    power_shift: i64,
    anchor: i64,
) -> (IndexedCoefficientContext, RuleCell) {
    // Keep one base variable even in fixed integer dimension. Symbolica's
    // sparse base map is intentionally exercised here, not its zero-variable
    // polynomial corner case.
    let base = CoefficientContext::new(["d"]);
    let dimension = dimension
        .map(|value| base.integer(value))
        .unwrap_or_else(|| base.parameter("d").unwrap());
    let family = IntegralFamily::new(
        format!("reachability-tadpole-d{dimension}-s{power_shift}"),
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        dimension,
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![base.integer(power_shift)],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows).unwrap();
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let sources = SourceViewBatch::try_select(translated, &[0], RuleCellLimits::default()).unwrap();
    let rule = derive_sector_interior_rule(
        generator.context(),
        sources.relations(),
        &[anchor],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let application = rule.domain().clone();
    let context = generator.context().clone();
    let cell = RuleCell::try_tightened(
        &context,
        rule,
        sources,
        application,
        RuleCellLimits::default(),
    )
    .unwrap();
    (context, cell)
}

fn sunset_family() -> IntegralFamily {
    let base = CoefficientContext::new(["d"]);
    let zero = base.zero();
    let one = base.one();
    IntegralFamily::new(
        "reachability-sunset",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                base.integer(-1),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                base.integer(-1),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(base.integer(-1), vec![one.clone(), base.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn vacuum_map(coefficients: &CoefficientContext, entries: [i64; 4]) -> MomentumMap {
    MomentumMap::new(
        CoefficientMatrix::try_new(
            2,
            2,
            entries.into_iter().map(|entry| coefficients.integer(entry)),
        )
        .unwrap(),
        CoefficientMatrix::try_new(2, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

fn sunset_cell_and_canonicalizer() -> (IndexedCoefficientContext, RuleCell, Canonicalizer) {
    let family = sunset_family();
    let generators = [
        vacuum_map(family.coefficient_context(), [0, 1, 1, 0]),
        vacuum_map(family.coefficient_context(), [1, 0, -1, -1]),
    ]
    .into_iter()
    .map(|map| {
        compile(
            &family,
            verify(&family, &family, map, SymmetryLimits::default()).unwrap(),
        )
        .unwrap()
    })
    .collect::<Vec<_>>();
    let canonicalizer = Canonicalizer::try_new(
        OrderingPolicy::default(),
        generators,
        CanonicalizationLimits::default(),
    )
    .unwrap();

    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows).unwrap();
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0, 0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let sources =
        SourceViewBatch::try_select(translated, &[0, 1, 2, 3], Default::default()).unwrap();
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &[1, 1, 1],
        &[0, 0, 1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let application = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&[1, 1, 1]).unwrap(),
        [
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
        ],
        rule.pivot().values(),
        &rhs,
    )
    .unwrap();
    let context = generator.context().clone();
    let cell = RuleCell::try_refined(
        &context,
        rule,
        sources,
        application,
        [],
        [],
        Default::default(),
    )
    .unwrap();
    (context, cell, canonicalizer)
}

#[test]
fn concrete_tadpole_chain_is_exact_deterministic_and_terminal_aware() {
    let (context, cell) = tadpole_cell(None, 0, 1);
    let planner = ReachabilityPlanner::try_new(
        &context,
        OrderingPolicy::default(),
        None,
        [&cell],
        ReachabilityLimits::default(),
    )
    .unwrap();
    let roots = [IntegralKey::try_new([4]).unwrap()];
    let terminals = |key: &IntegralKey| {
        (key.powers() == [1]).then_some(terminal(ReachabilityTerminalKind::Master, 0))
    };

    let first = planner.discover(&roots, &terminals).unwrap();
    let second = planner.discover(&roots, &terminals).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first
            .nodes()
            .iter()
            .map(|node| node.target().powers()[0])
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert!(first.uncovered().next().is_none());
    let statistics = first.statistics();
    assert_eq!(statistics.submitted_roots(), 1);
    assert_eq!(statistics.canonical_roots(), 1);
    assert_eq!(statistics.discovered_nodes(), 4);
    assert_eq!(statistics.terminal_nodes(), 1);
    assert_eq!(statistics.rule_applications(), 3);
    assert_eq!(statistics.dependency_edges(), 3);
    assert_eq!(statistics.coefficient_specializations(), 3);
    assert_eq!(statistics.retained_lattice_coordinate_cells(), 23);
    for node in &first.nodes()[1..] {
        let ReachabilityDisposition::Rule(application) = node.disposition() else {
            panic!("positive nonmaster tadpole powers must use the recurrence")
        };
        assert_eq!(application.cell_ordinal(), 0);
        assert_eq!(application.dependencies().len(), 1);
        let dependency = &application.dependencies()[0];
        assert_eq!(dependency.raw_child(), dependency.canonical_child());
        OrderingPolicy::default()
            .prove_strict_descent(node.target().powers(), dependency.raw_child().powers())
            .unwrap();
    }
}

#[test]
fn ordered_cell_selection_stops_at_the_first_applicable_owner() {
    let (context, cell) = tadpole_cell(None, 0, 1);
    let planner = ReachabilityPlanner::try_new(
        &context,
        OrderingPolicy::default(),
        None,
        [&cell, &cell],
        Default::default(),
    )
    .unwrap();
    let roots = [IntegralKey::try_new([2]).unwrap()];
    let terminals = |key: &IntegralKey| {
        (key.powers() == [1]).then_some(terminal(ReachabilityTerminalKind::Master, 0))
    };
    let frontier = planner.discover(&roots, &terminals).unwrap();
    let application = frontier
        .nodes()
        .iter()
        .find_map(|node| match node.disposition() {
            ReachabilityDisposition::Rule(application) => Some(application),
            _ => None,
        })
        .unwrap();
    assert_eq!(application.cell_ordinal(), 0);
    assert_eq!(frontier.statistics().rule_cell_probes(), 1);
}

#[test]
fn a_vanished_exact_guard_exposes_the_key_without_applying_the_cell() {
    // The physical power shift -1 makes the tadpole pivot proportional to
    // n-1. Derivation at n=2 is valid, while the same cell's concrete n=1
    // assignment is deliberately exceptional.
    let (context, cell) = tadpole_cell(None, -1, 2);
    let planner = ReachabilityPlanner::try_new(
        &context,
        OrderingPolicy::default(),
        None,
        [&cell],
        Default::default(),
    )
    .unwrap();
    let roots = [IntegralKey::try_new([2]).unwrap()];
    let frontier = planner.discover(&roots, &|_: &IntegralKey| None).unwrap();

    assert_eq!(
        frontier
            .uncovered()
            .map(IntegralKey::powers)
            .collect::<Vec<_>>(),
        [&[2][..]]
    );
    assert!(frontier.statistics().guard_specializations() > 0);
    assert_eq!(frontier.statistics().coefficient_specializations(), 0);
}

#[test]
fn an_identically_zero_specialized_rhs_is_not_a_dependency() {
    // For d=4 the one-loop recurrence coefficient of I(n) in I(n+1)
    // vanishes at n=2. The rule still applies to target I(3), but its exact
    // concrete dependency support is empty.
    let (context, cell) = tadpole_cell(Some(4), 0, 1);
    let planner = ReachabilityPlanner::try_new(
        &context,
        OrderingPolicy::default(),
        None,
        [&cell],
        Default::default(),
    )
    .unwrap();
    let roots = [IntegralKey::try_new([3]).unwrap()];
    let frontier = planner.discover(&roots, &|_: &IntegralKey| None).unwrap();
    assert_eq!(frontier.nodes().len(), 1);
    let ReachabilityDisposition::Rule(application) = frontier.nodes()[0].disposition() else {
        panic!("the cell remains applicable when only its RHS coefficient vanishes")
    };
    assert!(application.dependencies().is_empty());
    assert_eq!(frontier.statistics().coefficient_specializations(), 1);
    assert_eq!(frontier.statistics().dependency_edges(), 0);
}

#[test]
fn roots_and_descending_children_are_canonicalized_by_the_exact_action() {
    let (context, cell, canonicalizer) = sunset_cell_and_canonicalizer();
    let planner = ReachabilityPlanner::try_new(
        &context,
        OrderingPolicy::default(),
        Some(&canonicalizer),
        [&cell],
        Default::default(),
    )
    .unwrap();
    let roots = [IntegralKey::try_new([2, 1, 1]).unwrap()];
    let root_canonical = canonicalizer.canonicalize(&roots[0]).unwrap();
    assert_eq!(root_canonical.canonical().powers(), [1, 1, 2]);
    let terminal_root = root_canonical.canonical().clone();
    let frontier = planner
        .discover(&roots, &move |key: &IntegralKey| {
            (key != &terminal_root)
                .then_some(terminal(ReachabilityTerminalKind::ExternalBoundary, 0))
        })
        .unwrap();
    assert_eq!(frontier.canonical_roots()[0].powers(), [1, 1, 2]);
    let application = frontier
        .nodes()
        .iter()
        .find_map(|node| match node.disposition() {
            ReachabilityDisposition::Rule(application) => Some(application),
            _ => None,
        })
        .unwrap();
    assert!(!application.dependencies().is_empty());
    for dependency in application.dependencies() {
        assert_eq!(
            dependency.canonical_child(),
            canonicalizer
                .canonicalize(dependency.raw_child())
                .unwrap()
                .canonical()
        );
        OrderingPolicy::default()
            .prove_strict_descent(
                frontier.canonical_roots()[0].powers(),
                dependency.raw_child().powers(),
            )
            .unwrap();
    }
}

#[test]
fn reports_are_sorted_by_persisted_complexity_not_submission_order() {
    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "reachability-empty",
        1,
    )
    .unwrap();
    let planner = ReachabilityPlanner::try_new(
        &context,
        OrderingPolicy::default(),
        None,
        [],
        Default::default(),
    )
    .unwrap();
    let roots = [
        IntegralKey::try_new([4]).unwrap(),
        IntegralKey::try_new([2]).unwrap(),
        IntegralKey::try_new([4]).unwrap(),
    ];
    let frontier = planner.discover(&roots, &|_: &IntegralKey| None).unwrap();
    assert_eq!(frontier.statistics().submitted_roots(), 3);
    assert_eq!(frontier.statistics().canonical_roots(), 2);
    assert_eq!(
        frontier
            .uncovered()
            .map(|key| key.powers()[0])
            .collect::<Vec<_>>(),
        [2, 4]
    );
}

#[test]
fn terminal_classification_precedes_rule_cell_selection() {
    let (context, cell) = tadpole_cell(None, 0, 1);
    let planner = ReachabilityPlanner::try_new(
        &context,
        OrderingPolicy::default(),
        None,
        [&cell],
        Default::default(),
    )
    .unwrap();
    let roots = [IntegralKey::try_new([2]).unwrap()];
    let frontier = planner
        .discover(&roots, &|_: &IntegralKey| {
            Some(terminal(ReachabilityTerminalKind::Master, 7))
        })
        .unwrap();

    assert_eq!(frontier.nodes().len(), 1);
    assert_eq!(
        frontier.nodes()[0].disposition(),
        &ReachabilityDisposition::Terminal(terminal(ReachabilityTerminalKind::Master, 7))
    );
    let statistics = frontier.statistics();
    assert_eq!(statistics.terminal_nodes(), 1);
    assert_eq!(statistics.rule_applications(), 0);
    assert_eq!(statistics.rule_cell_probes(), 0);
    assert_eq!(statistics.guard_specializations(), 0);
    assert_eq!(statistics.coefficient_specializations(), 0);
    assert_eq!(statistics.dependency_edges(), 0);
}

#[test]
fn structural_and_dynamic_resource_limits_are_typed_at_exact_boundaries() {
    let (context, cell) = tadpole_cell(None, 0, 1);
    let mut limits = ReachabilityLimits::default();
    limits.max_rule_cells = 0;
    assert!(matches!(
        ReachabilityPlanner::try_new(&context, OrderingPolicy::default(), None, [&cell], limits,),
        Err(ReachabilityError::ResourceLimit {
            resource: "rule cells",
            requested: 1,
            limit: 0,
        })
    ));

    let roots = [IntegralKey::try_new([2]).unwrap()];
    let terminal = |key: &IntegralKey| {
        (key.powers() == [1]).then_some(ReachabilityTerminal::new(
            ReachabilityTerminalKind::Master,
            0,
        ))
    };
    let cases: [(fn(&mut ReachabilityLimits), &str); 7] = [
        (
            |limits: &mut ReachabilityLimits| limits.max_roots = 0,
            "submitted roots",
        ),
        (
            |limits: &mut ReachabilityLimits| limits.max_discovered_nodes = 1,
            "discovered nodes",
        ),
        (
            |limits: &mut ReachabilityLimits| limits.max_pending_nodes = 0,
            "pending nodes",
        ),
        (
            |limits: &mut ReachabilityLimits| limits.max_rule_cell_probes = 0,
            "rule-cell probes",
        ),
        (
            |limits: &mut ReachabilityLimits| limits.max_guard_specializations = 0,
            "guard specializations",
        ),
        (
            |limits: &mut ReachabilityLimits| limits.max_coefficient_specializations = 0,
            "coefficient specializations",
        ),
        (
            |limits: &mut ReachabilityLimits| limits.max_dependency_edges = 0,
            "dependency edges",
        ),
    ];
    for (update, resource) in cases {
        let mut limits = ReachabilityLimits::default();
        update(&mut limits);
        let planner = ReachabilityPlanner::try_new(
            &context,
            OrderingPolicy::default(),
            None,
            [&cell],
            limits,
        )
        .unwrap();
        assert!(matches!(
            planner.discover(&roots, &terminal),
            Err(ReachabilityError::ResourceLimit {
                resource: actual,
                ..
            }) if actual == resource
        ));
    }

    let mut exact = ReachabilityLimits::default();
    exact.max_pending_nodes = 1;
    exact.max_retained_lattice_coordinate_cells = 11;
    let planner =
        ReachabilityPlanner::try_new(&context, OrderingPolicy::default(), None, [&cell], exact)
            .unwrap();
    assert_eq!(
        planner
            .discover(&roots, &terminal)
            .unwrap()
            .statistics()
            .retained_lattice_coordinate_cells(),
        11
    );

    exact.max_retained_lattice_coordinate_cells = 10;
    let planner =
        ReachabilityPlanner::try_new(&context, OrderingPolicy::default(), None, [&cell], exact)
            .unwrap();
    assert_eq!(
        planner.discover(&roots, &terminal),
        Err(ReachabilityError::ResourceLimit {
            resource: "retained lattice coordinate cells",
            requested: 11,
            limit: 10,
        })
    );
}
