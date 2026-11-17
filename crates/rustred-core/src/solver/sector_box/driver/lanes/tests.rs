//! Lane tests on the three-loop K6 vacuum family with one deformed mass
//! (`D0 = k1^2 - 1 - eta`), the family of the golden eta-DE test.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::domains::rational::Rational;
use symbolica::prelude::{Integer, Ring};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::reconstruct::rational_image;
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::sector::CutConstraint;

use super::super::super::super::{Integral, SolverError};
use super::super::super::probe::LaneFailure;
use super::super::super::rows::{Zp, ZpRowFactory};
use super::super::super::system::ExactRowBuilder;
use super::super::super::{
    BoxOptions, CensusRouter, ColumnOrder, MasterCheckMode, Seeding, SlotPriority,
};
use super::super::BoxLaportaSession;
use super::ExplicitSample;

pub(crate) const N: usize = 6;
pub(crate) const TOP: [i16; N] = [1, 1, 1, 1, 1, 1];

/// K6 over `(k1k1, k1k2, k1k3, k2k2, k2k3, k3k3)`: three unit masses on
/// `k1`, `k2`, `k3` (the first one shifted by `eta`) and three on
/// `k1 - k3`, `k1 - k2`, `k2 - k3`.
pub(crate) fn k6_family() -> Arc<IntegralFamily> {
    let context = CoefficientContext::try_new(["d", "eta"]).unwrap();
    let rows: [[i64; 6]; 6] = [
        [1, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [1, 0, -2, 0, 0, 1],
        [1, -2, 0, 1, 0, 0],
        [0, 0, 0, 1, -2, 1],
    ];
    let denominators = rows
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let mut constant = context.integer(-1);
            if slot == 0 {
                constant = context
                    .try_sub(
                        &constant,
                        &context.parameter("eta").unwrap(),
                        ExactAlgebraLimits::default(),
                    )
                    .unwrap();
            }
            AffineDenominator::new(constant, row.map(|v| context.integer(v)).to_vec())
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            "w6_k6_eta_slot0",
            vec!["k1".into(), "k2".into(), "k3".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); N],
        )
        .unwrap(),
    )
}

/// The options of the campaign eta-DE runs: staircase seeding without
/// padding, census routing of the top sector, no LP gate, no stability
/// check, top-sector slot priority.
pub(crate) fn k6_options(workers: usize) -> BoxOptions {
    BoxOptions {
        seeding: Seeding::Staircase {
            s_pad: 0,
            d_pad: 0,
            max_growth: 0,
        },
        order: ColumnOrder::LinesRankDots,
        stability: None,
        workers,
        lp_gate: false,
        slot_priority: SlotPriority::TopSectorFirst(63),
        master_check: MasterCheckMode::Report,
        ..Default::default()
    }
}

/// The session of the eta-DE: demand the top integral, then add the eta
/// derivative (power of slot 0 raised by one) of every master with a
/// line on slot 0 until no new target appears. Returns the targets.
pub(crate) fn k6_session(workers: usize) -> (BoxLaportaSession<N>, Vec<[i16; N]>) {
    let family = k6_family();
    let mask: [bool; N] = [true; N];
    let router = Arc::new(CensusRouter::discover(family.clone(), &mask).unwrap());
    let mut session = BoxLaportaSession::<N>::new(
        family,
        &CutConstraint::none(N).unwrap(),
        router,
        k6_options(workers),
    )
    .unwrap();
    let mut targets = vec![TOP];
    loop {
        let report = session.extend(&targets, &[TOP]).unwrap();
        let have: BTreeSet<[i16; N]> = targets.iter().copied().collect();
        let mut added = false;
        for master in &report.masters {
            if master[0] > 0 {
                let mut derivative = *master;
                derivative[0] += 1;
                if !have.contains(&derivative) {
                    targets.push(derivative);
                    added = true;
                }
            }
        }
        if !added {
            return (session, targets);
        }
    }
}

pub(crate) fn q(numerator: i64, denominator: i64) -> Rational {
    Rational::from((Integer::from(numerator), Integer::from(denominator)))
}

const PRIME: u64 = 4_611_686_018_427_387_847;

fn evaluate(coefficient: &Coefficient, field: &Zp64, point: &[Zp]) -> Option<Zp> {
    let map = |value: &Integer| value.to_finite_field(field);
    let numerator = coefficient.numerator.evaluate_with_coeff_map(
        map,
        &point[..coefficient.numerator.nvars()],
        field,
    );
    let denominator = coefficient.denominator.evaluate_with_coeff_map(
        map,
        &point[..coefficient.denominator.nvars()],
        field,
    );
    Some(field.mul(&numerator, &field.try_inv(&denominator)?))
}

fn point(field: &Zp64, d: &Rational, eta: &Rational) -> Vec<Zp> {
    vec![
        rational_image(d, field).unwrap(),
        rational_image(eta, field).unwrap(),
    ]
}

/// The template rows of the modular pass at an explicit point, compiled
/// independently of the exact instantiation.
fn template_factory(session: &BoxLaportaSession<N>, point: &[Zp]) -> ZpRowFactory<N> {
    ZpRowFactory::at_parameters(
        &session.sources,
        session.arity,
        session.order.clone(),
        session.router.clone(),
        PRIME,
        point,
        // The session's factories use no extra conditions either.
        &[],
    )
    .unwrap()
}

/// Every selected row re-instantiated over Q(d, eta) specialises, at a
/// point modulo a prime, to the row the modular pass's template compiler
/// produces there (same integrals, same values).
#[test]
fn exact_rows_specialise_to_the_template_rows() {
    let (session, _) = k6_session(1);
    let field = Zp64::new(PRIME);
    let point = point(&field, &q(37, 11), &q(5, 13));
    let template = template_factory(&session, &point);
    let mut builder = ExactRowBuilder::new(
        &session.sources,
        &session.factory,
        &*session.router,
        &session.order,
        session.family.coefficient_context(),
    )
    .unwrap();
    let selection = session.selection.as_ref().unwrap();
    assert!(selection.rows.len() > 10);
    let mut automorphism_rows = 0;
    for (seed, source) in &selection.rows {
        automorphism_rows += usize::from(session.factory.is_automorphism_source(*source));
        let exact: Vec<(Integral<N>, Zp)> = builder
            .row(seed, *source)
            .unwrap()
            .into_iter()
            .map(|(integral, coefficient)| {
                (integral, evaluate(&coefficient, &field, &point).unwrap())
            })
            .filter(|(_, value)| !field.is_zero(value))
            .collect();
        let zp: Vec<(Integral<N>, Zp)> = template
            .row(seed, *source)
            .unwrap()
            .into_iter()
            .map(|term| (term.integral, term.coefficient))
            .collect();
        assert_eq!(exact, zp, "row ({seed:?}, {source})");
    }
    assert!(
        automorphism_rows > 0,
        "the census router adds automorphism rows"
    );
    // A provenance that names an automorphism row the seed does not have is
    // an error, not an empty row.
    let bad = u16::try_from(session.factory.source_count() + 1000).unwrap();
    assert!(matches!(
        builder.row(&TOP, bad),
        Err(SolverError::InvalidInput(_))
    ));
}

/// Whether `vector` lies in the span of `rows` (forward reduction against
/// an echelon form).
fn in_span(field: &Zp64, ncols: usize, rows: &[Vec<(u32, Zp)>], vector: &[(u32, Zp)]) -> bool {
    let mut reducer = SparseRowReducer::new(ncols as u32, field.clone(), LuLMode::None);
    for row in rows {
        let (ids, values): (Vec<u32>, Vec<Zp>) = row.iter().copied().unzip();
        reducer.add_row(&values, &ids);
    }
    let rank = reducer.pivots().iter().flatten().count();
    let (ids, values): (Vec<u32>, Vec<Zp>) = vector.iter().copied().unzip();
    reducer.add_row(&values, &ids);
    reducer.pivots().iter().flatten().count() == rank
}

/// The explicit-sample values satisfy the template rows at that point:
/// `target - sum_m value_m * master_m` lies in the span of the selected
/// rows produced by the modular pass's compiler, and a perturbed value does
/// not.
#[test]
fn explicit_probe_values_lie_in_the_span_of_the_template_rows() {
    let (mut session, targets) = k6_session(1);
    let (d, eta) = (q(37, 11), q(5, 13));
    let table = session
        .probe_explicit(
            &targets,
            &ExplicitSample {
                prime: PRIME,
                point: vec![("d".into(), d.clone()), ("eta".into(), eta.clone())],
            },
        )
        .unwrap();
    let field = Zp64::new(PRIME);
    let point = point(&field, &d, &eta);
    let template = template_factory(&session, &point);
    let (request, masters) = session
        .lane_request(&targets, &[("d".into(), d), ("eta".into(), eta)])
        .unwrap();
    assert_eq!(masters, table.masters);
    let system = &request.system;
    let column = |integral: &Integral<N>| system.column_index(integral).unwrap() as u32;
    let selection = session.selection.as_ref().unwrap();
    let rows: Vec<Vec<(u32, Zp)>> = selection
        .rows
        .iter()
        .map(|(seed, source)| {
            template
                .row(seed, *source)
                .unwrap()
                .iter()
                .map(|term| (column(&term.integral), term.coefficient))
                .collect()
        })
        .collect();
    let mut nonzero = 0;
    let master_column = |m: usize| column(&Integral::numeric(table.masters[m]).unwrap());
    for (t, target) in targets.iter().enumerate() {
        // route(target) - sum_m value_m * master_m, merged by column.
        let mut vector: BTreeMap<u32, Zp> = BTreeMap::new();
        for (image, value) in template.route_image(target).unwrap() {
            let slot = vector
                .entry(column(&Integral::numeric(image).unwrap()))
                .or_insert(field.zero());
            *slot = field.add(&*slot, &value);
        }
        for (m, value) in table.values[t].iter().enumerate() {
            if *value != 0 {
                nonzero += 1;
                let slot = vector.entry(master_column(m)).or_insert(field.zero());
                *slot = field.sub(&*slot, &field.to_element(*value));
            }
        }
        let entries = |vector: &BTreeMap<u32, Zp>| -> Vec<(u32, Zp)> {
            vector
                .iter()
                .filter(|(_, value)| !field.is_zero(value))
                .map(|(column, value)| (*column, *value))
                .collect()
        };
        let ncols = system.columns.len();
        assert!(
            in_span(&field, ncols, &rows, &entries(&vector)),
            "target {target:?}"
        );
        // A master is independent of the rows: perturbing its coefficient
        // leaves the span.
        let slot = vector.entry(master_column(0)).or_insert(field.zero());
        *slot = field.add(&*slot, &field.one());
        assert!(!in_span(&field, ncols, &rows, &entries(&vector)));
    }
    assert!(nonzero > targets.len(), "the reductions are nontrivial");
}

/// The bunch kernel gives the same values for every bunch size (including a
/// padded last bunch) and agrees with the reference elimination at every
/// point; a point where a fixed value has no image is refused at compile
/// time.
#[test]
fn bunch_kernel_is_bunch_size_invariant_and_matches_the_reference() {
    let (mut session, targets) = k6_session(1);
    let (request, _) = session
        .lane_request(&targets, &[("d".into(), q(37, 11))])
        .unwrap();
    assert_eq!(request.free(), vec![1]);
    let compiled = request.compile(PRIME).unwrap();
    let field = compiled.field().clone();
    let xs: Vec<Zp> = (0..7u64)
        .map(|k| field.to_element(1_000_003 + 7919 * k))
        .collect();
    let one = compiled.probe_points::<1, N>(&request.system, &xs, 1);
    let four = compiled.probe_points::<4, N>(&request.system, &xs, 1);
    let sixteen = compiled.probe_points::<16, N>(&request.system, &xs, 1);
    assert_eq!(one.len(), xs.len());
    assert_eq!(one, four);
    assert_eq!(one, sixteen);
    let d = rational_image(&q(37, 11), &field).unwrap();
    for (x, values) in xs.iter().zip(&one) {
        let values = values.as_ref().unwrap();
        let reference = super::super::super::probe::reference_probe(&request, &field, &[d, *x]);
        assert_eq!(values, &reference.unwrap());
    }
    // At a small prime (accidental zeros are likely), whenever the kernel
    // accepts a point the reference accepts it with the same values.
    let small = 65_537;
    let small_compiled = request.compile(small).unwrap();
    let small_field = small_compiled.field().clone();
    let small_d = rational_image(&q(37, 11), &small_field).unwrap();
    let small_xs: Vec<Zp> = (1..=64u64)
        .map(|k| small_field.to_element(k * 1013))
        .collect();
    let mut accepted = 0;
    for (x, values) in
        small_xs
            .iter()
            .zip(small_compiled.probe_points::<8, N>(&request.system, &small_xs, 1))
    {
        if let Ok(values) = values {
            accepted += 1;
            let reference =
                super::super::super::probe::reference_probe(&request, &small_field, &[small_d, *x]);
            assert_eq!(reference.as_ref(), Ok(&values), "x = {x:?}");
        }
    }
    assert!(
        accepted > 32,
        "{accepted} of 64 small-prime points accepted"
    );
    // eta = 2 is a pole of the K6 reduction (the golden coefficients carry
    // the factor 1 - 5/6 eta + 1/6 eta^2 = (eta - 2)(eta - 3)/6 in their
    // denominators): the lane fails instead of returning values.
    let pole = field.to_element(2);
    let failed = compiled.probe_points::<4, N>(&request.system, &[xs[0], pole], 1);
    assert!(failed[0].is_ok());
    assert!(matches!(
        failed[1],
        Err(LaneFailure::Pivot | LaneFailure::Structure | LaneFailure::Master)
    ));
    assert!(super::super::super::probe::reference_probe(&request, &field, &[d, pole]).is_err());
    // d = 1/p has no image modulo p.
    let prime = Integer::from(PRIME);
    let (unlucky, _) = session
        .lane_request(
            &targets,
            &[("d".into(), Rational::from((Integer::one(), prime)))],
        )
        .unwrap();
    assert!(unlucky.compile(PRIME).is_none());
}
