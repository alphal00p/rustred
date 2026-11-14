//! Sector-box materialisation on the three-loop K6 vacuum family with one
//! deformed mass, `D0 = k1^2 - 1 - eta` (the family of the golden eta-DE
//! test; every case runs in seconds).
//!
//! The family carries the name of the campaign family, so that its
//! fingerprint is the one recorded in the golden solution files.
//!
//! The session is the campaign's eta-DE session: staircase seeding without
//! padding, census routing of the top sector, and the eta-derivative
//! closure (the derivative of every master with a line on slot 0 is
//! demanded until no new target appears).

use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::algebra::{CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::SolverError;
use rustred::solver::sector_box::{
    BoxLaportaSession, BoxOptions, BoxPhase, CensusRouter, ColumnOrder, ExplicitSample,
    MasterCheckMode, Seeding, SlotPriority,
};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

const N: usize = 6;
const TOP: [i16; N] = [1, 1, 1, 1, 1, 1];
const PRIME: u64 = 4_611_686_018_427_387_847;

fn k6_family() -> Arc<IntegralFamily> {
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

fn options(workers: usize) -> BoxOptions {
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

fn new_session(workers: usize) -> BoxLaportaSession<N> {
    let family = k6_family();
    let router = Arc::new(CensusRouter::discover(family.clone(), &[true; N]).unwrap());
    BoxLaportaSession::<N>::new(
        family,
        &CutConstraint::none(N).unwrap(),
        router,
        options(workers),
    )
    .unwrap()
}

/// Extend with the top integral and the eta-derivative closure.
fn session(workers: usize) -> (BoxLaportaSession<N>, Vec<[i16; N]>) {
    let mut session = new_session(workers);
    let mut targets = vec![TOP];
    loop {
        let report = session.extend(&targets, &[TOP]).unwrap();
        let have: BTreeSet<[i16; N]> = targets.iter().copied().collect();
        let before = targets.len();
        for master in &report.masters {
            let mut derivative = *master;
            derivative[0] += 1;
            if master[0] > 0 && !have.contains(&derivative) {
                targets.push(derivative);
            }
        }
        if targets.len() == before {
            return (session, targets);
        }
    }
}

fn q(numerator: i64, denominator: i64) -> Rational {
    Rational::from((Integer::from(numerator), Integer::from(denominator)))
}

fn sample(prime: u64, point: &[(&str, Rational)]) -> ExplicitSample {
    ExplicitSample {
        prime,
        point: point
            .iter()
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect(),
    }
}

/// The closure reproduces the campaign run: 12 masters, 9 targets.
#[test]
fn eta_closure_has_twelve_masters_and_nine_targets() {
    let (session, targets) = session(2);
    assert_eq!(session.masters().len(), 12);
    assert_eq!(targets.len(), 9);
    assert!(session.masters().contains(&TOP));
}

/// Explicit values: masters reduce to themselves, the table does not depend
/// on the worker count, and a repeated call (cached exact system) gives the
/// same table.
#[test]
fn explicit_probe_is_deterministic_and_masters_are_unit_rows() {
    let (mut two, targets) = session(2);
    let (mut one, _) = session(1);
    let point = sample(PRIME, &[("d", q(37, 11)), ("eta", q(5, 13))]);
    let table = two.probe_explicit(&targets, &point).unwrap();
    assert_eq!(table, one.probe_explicit(&targets, &point).unwrap());
    assert_eq!(table, two.probe_explicit(&targets, &point).unwrap());
    assert_eq!(table.masters, two.masters());
    for (t, target) in targets.iter().enumerate() {
        if let Some(position) = table.masters.iter().position(|m| m == target) {
            for (m, value) in table.values[t].iter().enumerate() {
                assert_eq!(*value, u64::from(m == position));
            }
        }
    }
}

/// Bad requests are typed errors, never panics or silent defaults.
#[test]
fn explicit_probe_rejects_bad_requests() {
    let generic = [("d", q(37, 11)), ("eta", q(5, 13))];
    let mut fresh = new_session(1);
    assert!(matches!(
        fresh.probe_explicit(&[TOP], &sample(PRIME, &generic)),
        Err(SolverError::InvalidInput(_))
    ));
    let (mut session, targets) = session(1);
    let invalid =
        |result: Result<_, SolverError>| matches!(result, Err(SolverError::InvalidInput(_)));
    // Not demanded.
    assert!(invalid(session.probe_explicit(
        &[[3, 1, 1, 1, 1, 1]],
        &sample(PRIME, &generic)
    )));
    // Unknown, repeated or missing parameters.
    assert!(invalid(session.probe_explicit(
        &targets,
        &sample(PRIME, &[("d", q(37, 11)), ("mass", q(1, 2))])
    )));
    assert!(invalid(session.probe_explicit(
        &targets,
        &sample(
            PRIME,
            &[("d", q(37, 11)), ("d", q(37, 11)), ("eta", q(1, 2))]
        )
    )));
    assert!(invalid(
        session.probe_explicit(&targets, &sample(PRIME, &[("d", q(37, 11))]))
    ));
    // Not an odd prime below 2^63.
    for prime in [2, 9, 4_611_686_018_427_387_849, (1u64 << 63) + 29] {
        assert!(
            invalid(session.probe_explicit(&targets, &sample(prime, &generic))),
            "{prime}"
        );
    }
    // A fixed value without an image modulo the prime.
    let pole = Rational::from((Integer::from(1), Integer::from(PRIME)));
    assert!(matches!(
        session.probe_explicit(&targets, &sample(PRIME, &[("d", pole), ("eta", q(5, 13))])),
        Err(SolverError::UnluckyBoxSample {
            phase: BoxPhase::Conditions
        })
    ));
    // eta = 2 and eta = 3 are poles of the reduction.
    for eta in [2, 3] {
        assert!(
            matches!(
                session.probe_explicit(
                    &targets,
                    &sample(PRIME, &[("d", q(37, 11)), ("eta", q(eta, 1))])
                ),
                Err(SolverError::UnluckyBoxSample { .. })
            ),
            "eta = {eta}"
        );
    }
    // The session still works after the refusals.
    assert!(
        session
            .probe_explicit(&targets, &sample(PRIME, &generic))
            .is_ok()
    );
}
