//! Massless propagator-type ("p-integral") families: one external momentum
//! `q`, Euclidean `q.q = 1`, massless lines (see `docs/p_integrals.md`).
//!
//! - unit-scale classification of the family kinematics;
//! - the sector-symmetry census with marked external vertices: only
//!   `q`-preserving maps `k = A k' + B q'`, `q = +-q'` are proposed, and the
//!   classes compress the raw candidate terminals onto exactly the
//!   Lee-Pomeransky master count of each class representative;
//! - verified affine route maps with `q -> -q` and shifts `k -> q - k`;
//! - in-process generation and exact reduction (LiteRed lane) of the one-,
//!   two- and three-loop families, checked at several rational `d` against
//!   exact G-function values of recursively one-loop integrals and against
//!   the triangle-rule closed form of the two-loop kite.
//!
//! The four-loop runs are slow and ignored by default:
//!
//! ```text
//! cargo test --release -p rustred --test p_integrals -- --ignored --test-threads 1
//! ```
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustred::algebra::CoefficientContext;
use rustred::family::{IntegralKey, UnitScaleKinematics};
use rustred::propagator::PresentationId;
use rustred::propagator::class::{ClassLimits, classify};
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot, SlotLine};
use rustred::reduction::terminal_normalization::TerminalSectorSymmetryEquations;
use rustred::sector::masters::{MasterCount, MasterCountOptions, MasterCounter};
use rustred::sector::symmetry::integral_transport::compile;
use rustred::sector::symmetry::{
    CoefficientMatrix, DenominatorAction, Jacobian, Limits, MomentumMap, SectorSymmetryCensus,
    SectorSymmetryCensusControl, SectorSymmetryCensusOptions, verify,
};
use rustred::sector::{Mask, zero::Analyzer};
use rustred::solver::litered::{DynamicLiteRedSession, LiteRedLaneReport, LiteRedOptions};
use symbolica::domains::rational::Rational;

use support::p_integrals::{
    BE, BUBBLE, CROSSED_4L, GValue, KITE, LA, LADDER_4L, NO, PFamily, collect,
    compile as compile_source, g, rational,
};

/// Rational dimensions away from integers and half-integers.
fn dimensions() -> [Rational; 3] {
    [rational(37, 10), rational(43, 10), rational(59, 10)]
}

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
}

fn census(family: &PFamily) -> SectorSymmetryCensus {
    let native = family.family();
    let analyzer = Analyzer::try_unrestricted(&native).unwrap();
    let top = Mask::try_new(family.root.iter().copied()).unwrap();
    SectorSymmetryCensus::discover_subsectors(
        native,
        &top,
        &analyzer,
        SectorSymmetryCensusOptions::default(),
        &SectorSymmetryCensusControl::default(),
    )
    .unwrap()
}

fn mask(family: &PFamily, lines: &[usize]) -> Mask {
    Mask::try_new((0..family.lines.len()).map(|slot| lines.contains(&(slot + 1)))).unwrap()
}

/// Generate the downset of the family root in process, compress the
/// candidate terminals with the census and return the lane, its masters and
/// the report.
fn lane(family: &PFamily) -> (DynamicLiteRedSession, Vec<IntegralKey>, LiteRedLaneReport) {
    let options = LiteRedOptions {
        workers: workers(),
        ..LiteRedOptions::default()
    };
    let (mut lane, report) = DynamicLiteRedSession::generate(
        family.family(),
        family.root,
        workers(),
        true,
        options,
        None,
    )
    .unwrap();
    lane.compress(&[]).unwrap();
    let masters = lane.masters();
    (lane, masters, report)
}

/// Reduce `targets` exactly at several `d` and check every target with a
/// closed form (a sum of G-products): `value(target) = sum_i c_i
/// value(master_i)`. Returns the number of checked identities.
fn check_reductions(
    family: &PFamily,
    lane: &mut DynamicLiteRedSession,
    masters: &[IntegralKey],
    targets: &[IntegralKey],
    closed_form: impl Fn(&IntegralKey, &Rational) -> Option<Vec<GValue>>,
) -> usize {
    let ids = lane.add_targets(targets).unwrap();
    assert_eq!(lane.statistics(&ids).uncovered, 0);
    let minus_one = rational(-1, 1);
    let mut checked = 0;
    for d in dimensions() {
        let reduction = lane
            .reduce_exact_at_point(std::slice::from_ref(&d), &ids)
            .unwrap();
        for (target, row) in targets.iter().zip(&reduction.coefficients) {
            let Some(expected) = closed_form(target, &d)
                .or_else(|| family.value(target, &d).map(|value| vec![value]))
            else {
                continue;
            };
            let Some(terms) = masters
                .iter()
                .zip(row)
                .filter(|(_, c)| !c.is_zero())
                .map(|(master, c)| family.value(master, &d).map(|v| v.scale(c)))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let difference = terms
                .into_iter()
                .chain(expected.iter().map(|value| value.scale(&minus_one)));
            assert!(
                collect(difference).is_empty(),
                "{}: {:?} at d = {d} differs from its closed form",
                family.name,
                target.powers()
            );
            checked += 1;
        }
    }
    checked
}

/// The census classes carry exactly the Lee-Pomeransky master count of
/// their representative sector.
fn check_master_counts(family: &PFamily, lane: &mut DynamicLiteRedSession) -> usize {
    let report = lane.compress(&[]).unwrap();
    let native = family.family();
    let mut counter = MasterCounter::try_new(&native, MasterCountOptions::default()).unwrap();
    let mut expected = 0;
    for class in &report.classes {
        let MasterCount::Counted(count) = counter.count(&class.representative).unwrap() else {
            panic!("{}: no master-count verdict", family.name);
        };
        assert_eq!(
            class.masters, count,
            "{}: class {:?}",
            family.name, class.representative
        );
        expected += count;
    }
    assert_eq!(report.masters.len(), expected);
    expected
}

#[test]
fn unit_scale_kinematics_classify_propagator_and_vacuum_families() {
    for (external_square, expected) in [(1, Some(1)), (-1, Some(-1)), (2, None)] {
        assert_eq!(
            compile_source(&KITE.source(external_square))
                .unit_scale_kinematics()
                .unwrap(),
            expected.map(|external_square| UnitScaleKinematics::UnitScalePropagator {
                external_square
            })
        );
    }
    let sunset = |mass: &str| {
        compile_source(&format!(
            "I(loops(k1,k2),externals(),dimension(d),prop(D1,k1^2{mass},1),\
             prop(D2,k2^2{mass},1),prop(D3,(k1+k2)^2{mass},1))"
        ))
    };
    assert_eq!(
        sunset("-1").unit_scale_kinematics().unwrap(),
        Some(UnitScaleKinematics::UnitMassVacuum)
    );
    assert_eq!(sunset("").unit_scale_kinematics().unwrap(), None);
}

#[test]
fn census_colours_the_external_vertices_of_the_kite() {
    let census = census(&KITE);
    // Top, bubble product, the four triangles and the two sunsets.
    assert_eq!(census.members().len(), 8);
    assert_eq!(census.classes().len(), 4);
    let top = census.class_of(&mask(&KITE, &[1, 2, 3, 4, 5])).unwrap();
    // k1 <-> k2 and k -> q - k: lines (12)(34), (13)(24) and their product.
    assert_eq!(top.line_group_order(), Some(4));
    let triangles = census.class_of(&mask(&KITE, &[2, 3, 4, 5])).unwrap();
    assert_eq!(triangles.members().len(), 4);
    let sunset = census.member(&mask(&KITE, &[1, 4, 5])).unwrap();
    assert_eq!(sunset.representative(), &mask(&KITE, &[2, 3, 5]));
    // Two lines of {2,3,4,5} carry q against one of the representative
    // {1,2,3,5}: only an affine route k = A k' + B q relates them.
    let triangle = census.member(&mask(&KITE, &[2, 3, 4, 5])).unwrap();
    assert_eq!(triangle.representative(), &mask(&KITE, &[1, 2, 3, 5]));
    let momentum = triangle.verified_map().momentum();
    assert!(
        momentum
            .loop_external()
            .entries()
            .iter()
            .any(|c| !c.is_zero())
    );
    assert_eq!(momentum.external_linear().rows(), 1);
    // The owner classes of the p library partition the same sectors.
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let lines = KITE.lines.iter().map(|row| {
        Slot::Line(SlotLine {
            momentum: row.to_vec(),
            eps_units: 0,
        })
    });
    let presentation = Presentation::from_slots(
        PresentationId(0),
        PresentationKind::EpsNode,
        KITE.name,
        2,
        lines.collect(),
        &context,
        None,
    )
    .unwrap();
    let mut owner_classes = BTreeMap::new();
    for class in census.classes() {
        for member in class.members() {
            let raw = member.active_bits().iter().enumerate();
            let raw = raw.fold(0, |raw, (slot, &active)| raw | u64::from(active) << slot);
            let key = classify(&presentation, raw, &ClassLimits::default())
                .unwrap()
                .unwrap()
                .key;
            let representative = owner_classes
                .entry(key)
                .or_insert_with(|| class.representative().clone());
            assert_eq!(representative, class.representative());
        }
    }
    assert_eq!(owner_classes.len(), 4);
}

#[test]
fn bubble_automorphisms_fix_q_up_to_sign() {
    let census = census(&BUBBLE);
    assert_eq!(census.classes().len(), 1);
    assert_eq!(census.classes()[0].line_group_order(), Some(2));
}

#[test]
fn affine_route_maps_with_q_reversal_verify_and_transport() {
    let family = KITE.family();
    let context = family.coefficient_context();
    let int = |values: &[i64], rows, columns| {
        CoefficientMatrix::try_new(rows, columns, values.iter().map(|v| context.integer(*v)))
            .unwrap()
    };
    // (A, B, C) of k = A k' + B q', q = C q', and the expected line images.
    type Case = (&'static [i64], &'static [i64], i64, [usize; 5]);
    let cases: [Case; 3] = [
        // q -> -q with k -> -k: every line onto itself.
        (&[-1, 0, 0, -1], &[0, 0], -1, [0, 1, 2, 3, 4]),
        // k -> q - k: lines (13)(24).
        (&[-1, 0, 0, -1], &[1, 1], 1, [2, 3, 0, 1, 4]),
        // q -> -q composed with k1 <-> k2 and a shift: k1 = -k2' , k2 = -k1'.
        (&[0, -1, -1, 0], &[0, 0], -1, [1, 0, 3, 2, 4]),
    ];
    for (a, b, c, images) in cases {
        let map = MomentumMap::new(int(a, 2, 2), int(b, 2, 1), int(&[c], 1, 1));
        let verified = verify(&family, &family, map, Limits::default()).unwrap();
        assert!(matches!(verified.jacobian(), Jacobian::Unit { .. }));
        for (slot, image) in images.into_iter().enumerate() {
            assert!(matches!(
                &verified.row_actions()[slot],
                DenominatorAction::Monomial { target, scale } if *target == image && *scale == context.one()
            ));
        }
        let top = mask(&KITE, &[1, 2, 3, 4, 5]);
        let route = compile(
            &family,
            Arc::clone(&family),
            Arc::new(verified),
            top.clone(),
            top,
            Default::default(),
        )
        .unwrap();
        // A numerator on the middle line and a dot are carried exactly.
        let key = KITE.key(&[2, 1, 1, 1, -1]);
        let transported = route.transport(&key, Default::default()).unwrap();
        let mut powers = [0; 5];
        for (slot, image) in images.into_iter().enumerate() {
            powers[image] = key.powers()[slot];
        }
        assert_eq!(transported.terms().len(), 1);
        assert_eq!(transported.terms()[0].key().powers(), powers);
    }
    // k1 -> k1 + q is a valid change of variables but no symmetry: the
    // replay keeps k1^2 -> (k1+q)^2 as an affine row, not a line image.
    let shift = MomentumMap::new(
        int(&[1, 0, 0, 1], 2, 2),
        int(&[1, 0], 2, 1),
        int(&[1], 1, 1),
    );
    let verified = verify(&family, &family, shift, Limits::default()).unwrap();
    assert!(matches!(
        verified.row_actions()[0],
        DenominatorAction::Affine
    ));
}

#[test]
fn bubble_reduces_onto_one_master_exactly() {
    let (mut lane, masters, _) = lane(&BUBBLE);
    assert_eq!(masters, [BUBBLE.key(&[1, 1])]);
    let targets = [[2, 1], [2, 2], [3, 1], [1, 3], [3, -1], [1, -1], [0, 1]]
        .map(|powers| BUBBLE.key(&powers));
    let checked = check_reductions(&BUBBLE, &mut lane, &masters, &targets, |_, _| None);
    assert_eq!(checked, targets.len() * dimensions().len());
}

#[test]
fn kite_reduces_onto_two_masters_and_its_triangle_rule_closed_form() {
    let (mut lane, masters, report) = lane(&KITE);
    assert_eq!(report.census_classes, Some(4));
    assert_eq!(report.generation.terminals, 3);
    assert_eq!(check_master_counts(&KITE, &mut lane), 2);
    assert_eq!(masters.len(), 2);
    let top = KITE.key(&[1, 1, 1, 1, 1]);
    // K = 2/(d-4) G(1,1) [G(2,1) - G(2,3-d/2)].
    let kite = |key: &IntegralKey, d: &Rational| {
        (key == &top).then(|| {
            let prefactor = &rational(2, 1) / &(d - &rational(4, 1));
            let g11 = g((1, 0), (1, 0), d).scale(&prefactor);
            vec![
                g11.mul(&g((2, 0), (1, 0), d)),
                g11.mul(&g((2, 0), (3, -1), d)).scale(&rational(-1, 1)),
            ]
        })
    };
    let targets = [
        [1, 1, 1, 1, 1],
        [2, 1, 1, 1, 0],
        [1, 1, 2, 2, 0],
        [1, 0, 0, 1, 2],
        [0, 2, 1, 0, 1],
        [1, 0, 0, 1, 1],
        [3, 0, 0, 1, 1],
    ]
    .map(|powers| KITE.key(&powers));
    let checked = check_reductions(&KITE, &mut lane, &masters, &targets, kite);
    assert_eq!(checked, targets.len() * dimensions().len());
}

/// Every 0/1 point of the root, the top with one dot on each line, and the
/// top with one and two ISP numerators.
fn three_loop_targets(family: &PFamily) -> Vec<IntegralKey> {
    let physical = family.root.iter().filter(|&&root| root).count();
    let mut targets: Vec<IntegralKey> = (1..1usize << physical)
        .map(|bits| {
            family.key(
                &(0..family.lines.len())
                    .map(|slot| i64::from(slot < physical && bits & 1 << slot != 0))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let top: Vec<i64> = family.root.iter().map(|&root| i64::from(root)).collect();
    for slot in 0..physical {
        let mut dotted = top.clone();
        dotted[slot] = 2;
        targets.push(family.key(&dotted));
    }
    for numerator in [-1, -2] {
        let mut powers = top.clone();
        powers[family.lines.len() - 1] = numerator;
        targets.push(family.key(&powers));
    }
    targets
}

/// Raw terminals, census classes, masters, top-sector masters and checked
/// closed forms (per `d`) of one three-loop family.
struct ThreeLoop {
    terminals: usize,
    classes: usize,
    masters: usize,
    top_masters: usize,
    closed_forms: usize,
}

fn check_three_loop(family: &PFamily, expected: ThreeLoop) {
    let ThreeLoop {
        terminals,
        classes,
        masters,
        top_masters,
        closed_forms,
    } = expected;
    let (mut lane, keys, report) = lane(family);
    assert_eq!(report.generation.terminals, terminals);
    assert_eq!(report.census_classes, Some(classes));
    assert_eq!(check_master_counts(family, &mut lane), masters);
    let top: Vec<i64> = family.root.iter().map(|&root| i64::from(root)).collect();
    assert_eq!(
        keys.iter()
            .filter(|key| key
                .powers()
                .iter()
                .zip(&top)
                .all(|(p, t)| (*p > 0) == (*t > 0)))
            .count(),
        top_masters
    );
    let targets = three_loop_targets(family);
    let checked = check_reductions(family, &mut lane, &keys, &targets, |_, _| None);
    assert_eq!(checked, closed_forms * dimensions().len());
}

#[test]
fn three_loop_ladder_compresses_onto_its_masters() {
    check_three_loop(
        &LA,
        ThreeLoop {
            terminals: 10,
            classes: 16,
            masters: 5,
            top_masters: 0,
            closed_forms: 241,
        },
    );
}

#[test]
fn three_loop_benz_compresses_onto_its_masters() {
    check_three_loop(
        &BE,
        ThreeLoop {
            terminals: 10,
            classes: 18,
            masters: 4,
            top_masters: 0,
            closed_forms: 231,
        },
    );
}

#[test]
fn three_loop_nonplanar_compresses_onto_its_masters() {
    check_three_loop(
        &NO,
        ThreeLoop {
            terminals: 16,
            classes: 14,
            masters: 5,
            top_masters: 1,
            closed_forms: 212,
        },
    );
}

/// The pint-study probe measured 271 nonzero sectors, 52,340 rules and 36
/// raw terminals (no census); the marked census compresses them onto 12
/// masters in 62 classes. About 150 s on 8 threads, 1.7 GB.
#[test]
#[ignore = "slow: in-process four-loop ladder generation (run with --ignored, release)"]
fn four_loop_ladder_generates_and_compresses() {
    let options = LiteRedOptions {
        workers: workers(),
        ..LiteRedOptions::default()
    };
    let (mut lane, report) = DynamicLiteRedSession::generate(
        LADDER_4L.family(),
        LADDER_4L.root,
        workers(),
        true,
        options,
        None,
    )
    .unwrap();
    assert_eq!(report.generation.sectors, 271);
    assert_eq!(report.generation.rules, 52_340);
    assert_eq!(report.generation.terminals, 36);
    assert_eq!(report.census_classes, Some(62));
    assert_eq!(check_master_counts(&LADDER_4L, &mut lane), 12);
}

/// The crossed-rung family's census alone (its generation fails on three
/// exceptional sectors, see the `family-candidates` failure report).
#[test]
fn four_loop_crossed_rung_census_has_no_skips() {
    let census = census(&CROSSED_4L);
    assert_eq!(census.members().len(), 366);
    assert_eq!(census.classes().len(), 78);
    assert!(census.skips().is_empty());
}

/// The terminal sector-symmetry lane relates the two labelled kite
/// sunsets, `I(1,0,0,1,1) = I(0,1,1,0,1)`, through a marked route.
#[test]
fn terminal_sector_symmetry_equations_relate_the_kite_sunsets() {
    let sunsets = [KITE.key(&[1, 0, 0, 1, 1]), KITE.key(&[0, 1, 1, 0, 1])];
    let keys = BTreeSet::from(sunsets.clone());
    let equations =
        TerminalSectorSymmetryEquations::prepare(KITE.family(), &keys, Default::default()).unwrap();
    let related = keys
        .iter()
        .flat_map(|key| equations.equations(key))
        .any(|equation| {
            equation.terms.len() == 2
                && sunsets
                    .iter()
                    .all(|sunset| equation.terms.contains_key(sunset))
        });
    assert!(related);
}
