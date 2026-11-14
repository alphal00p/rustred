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

use rustred::algebra::reconstruct::{RationalFunction, rational_image};
use rustred::algebra::{CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::SolverError;
use rustred::solver::sector_box::{
    BoxLaportaSession, BoxOptions, BoxPhase, BoxSolution, CensusRouter, ColumnOrder,
    ExplicitSample, FaultInjection, FfOptions, MasterCheckMode, Seeding, SlotPriority,
};
use serde_json::Value;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
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

fn value_at(function: &RationalFunction, field: &Zp64, x: &Rational) -> Option<u64> {
    let x = rational_image(x, field)?;
    function
        .evaluate(field, &x)
        .map(|value| field.from_element(&value))
}

fn golden(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/sector_box_k6/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn integral(value: &Value) -> [i16; N] {
    let list = value.as_array().unwrap();
    assert_eq!(list.len(), N);
    std::array::from_fn(|slot| list[slot].as_i64().unwrap() as i16)
}

/// The options of the campaign run that wrote the golden files.
fn campaign_ff() -> FfOptions {
    FfOptions {
        max_primes: 64,
        ..Default::default()
    }
}

/// Compare a solution with a golden `rustred-harness-solution.v1` file of
/// the campaign engine: masters in order, every rule term by term
/// (coefficients as printed), the fixed value and every reconstruction and
/// verification prime.
fn assert_matches_golden(solution: &BoxSolution<N>, golden: &Value) {
    let masters: Vec<[i16; N]> = golden["masters"]
        .as_array()
        .unwrap()
        .iter()
        .map(integral)
        .collect();
    assert_eq!(solution.masters, masters);
    let rules = golden["rules"].as_array().unwrap();
    assert_eq!(solution.targets.len(), rules.len());
    for (t, rule) in rules.iter().enumerate() {
        assert_eq!(solution.targets[t], integral(&rule["target"]));
        let expected: Vec<([i16; N], String)> = rule["terms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|term| {
                (
                    integral(&term["integral"]),
                    term["coefficient"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let found: Vec<([i16; N], String)> = solution
            .terms(t)
            .map(|(master, function)| (*master, function.format(&solution.free)))
            .collect();
        assert_eq!(found, expected, "target {:?}", solution.targets[t]);
    }
    let fixed: Vec<(String, String)> = solution
        .fixed
        .iter()
        .map(|(name, value)| (name.clone(), value.to_string()))
        .collect();
    let golden_fixed: Vec<(String, String)> = golden["fixed"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| (name.clone(), value.as_str().unwrap().to_string()))
        .collect();
    assert_eq!(fixed, golden_fixed);
    let primes: Vec<u64> = golden["stats"]["primes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["prime"].as_u64().unwrap())
        .collect();
    let verifications: Vec<u64> = golden["stats"]["verifications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["prime"].as_u64().unwrap())
        .collect();
    assert_eq!(solution.reconstruction_primes(), primes);
    assert_eq!(solution.verification_primes(), verifications);
    for prime in &verifications {
        assert!(
            !primes.contains(prime),
            "verification prime {prime} was a reconstruction prime"
        );
    }
    assert_eq!(
        solution
            .report
            .reconstruction
            .verifications
            .last()
            .unwrap()
            .failed_functions,
        0
    );
}

/// GOLDEN: the 3-loop K6 eta-DE at d0 = 37/11 and d0 = 319999/80000
/// reproduces the campaign engine's solution files exactly (12 masters, 9
/// targets, every coefficient in eta, every prime).
#[test]
fn golden_k6_eta_de_at_two_d0() {
    let (mut session, targets) = session(2);
    for (d0, file) in [
        (q(37, 11), "solution-37_11.json"),
        (q(319999, 80000), "solution-319999_80000.json"),
    ] {
        let solution = session
            .materialize_ff(&targets, &[("d".into(), d0)], &campaign_ff())
            .unwrap();
        assert_eq!(solution.free, "eta");
        assert!(solution.provenance.unseeded_master_sectors.is_empty());
        assert_matches_golden(&solution, &golden(file));
    }
}

/// The lane in eta at fixed d and in d at fixed eta agree with each other
/// and with the explicit probe at the common point; masters reduce to
/// themselves.
#[test]
fn finite_field_lane_agrees_across_parameter_splits_and_explicit_samples() {
    let (mut session, targets) = session(2);
    let (d0, eta0) = (q(37, 11), q(1, 7));
    let options = FfOptions {
        bunch: 4,
        ..Default::default()
    };
    let in_eta = session
        .materialize_ff(&targets, &[("d".into(), d0.clone())], &options)
        .unwrap();
    let in_d = session
        .materialize_ff(&targets, &[("eta".into(), eta0.clone())], &options)
        .unwrap();
    assert_eq!((in_eta.free.as_str(), in_d.free.as_str()), ("eta", "d"));
    assert!(in_d.report.reconstruction.max_numerator_degree > 0);
    assert!(in_eta.report.reconstruction.max_denominator_degree > 0);
    let table = session
        .probe_explicit(
            &targets,
            &sample(PRIME, &[("d", d0.clone()), ("eta", eta0.clone())]),
        )
        .unwrap();
    let field = Zp64::new(PRIME);
    for (t, target) in targets.iter().enumerate() {
        for m in 0..in_d.masters.len() {
            assert_eq!(
                value_at(&in_d.coefficients[t][m], &field, &d0),
                Some(table.values[t][m])
            );
            assert_eq!(
                value_at(&in_eta.coefficients[t][m], &field, &eta0),
                Some(table.values[t][m])
            );
        }
        if let Some(position) = in_d.masters.iter().position(|master| master == target) {
            for (m, function) in in_d.coefficients[t].iter().enumerate() {
                let expected = if m == position {
                    RationalFunction::constant(q(1, 1))
                } else {
                    RationalFunction::zero()
                };
                assert_eq!(function, &expected);
            }
        }
    }
}

/// Results do not depend on the worker count: identical coefficients,
/// reconstruction and verification primes and provenance. (The number of
/// points probed per prime may differ: each round evaluates one bunch per
/// worker.)
#[test]
fn finite_field_lane_is_worker_count_invariant() {
    let fixed = [("d".to_string(), q(37, 11))];
    let (mut one, targets) = session(1);
    let (mut three, _) = session(3);
    let a = one
        .materialize_ff(&targets, &fixed, &FfOptions::default())
        .unwrap();
    let b = three
        .materialize_ff(&targets, &fixed, &FfOptions::default())
        .unwrap();
    assert_eq!(a.coefficients, b.coefficients);
    assert_eq!(a.reconstruction_primes(), b.reconstruction_primes());
    assert_eq!(a.verification_primes(), b.verification_primes());
    assert_eq!(a.provenance, b.provenance);
}

/// Forced poles (eta = 2, 3 and the rational poles of the clean result),
/// discarded lanes and a corrupted first prime are resampled or left out:
/// the result equals the clean one, and the explicit probe refuses every
/// pole.
#[test]
fn finite_field_lane_resamples_injected_faults() {
    let (mut session, targets) = session(2);
    let fixed = [("d".to_string(), q(37, 11))];
    let clean = session
        .materialize_ff(&targets, &fixed, &FfOptions::default())
        .unwrap();
    let mut poles = Vec::new();
    for function in clean.coefficients.iter().flatten() {
        for k in -40..=40 {
            let x = q(k, 2);
            if function.evaluate_rational(&x).is_none() && !poles.contains(&x) {
                poles.push(x);
            }
        }
    }
    assert!(
        poles.contains(&q(2, 1)) && poles.contains(&q(3, 1)),
        "{poles:?}"
    );
    let corrupt = clean
        .coefficients
        .iter()
        .flatten()
        .position(|function| !function.is_zero() && function.degrees() != (0, 0))
        .unwrap();
    let faulty = session
        .materialize_ff(
            &targets,
            &fixed,
            &FfOptions {
                fault: Some(FaultInjection {
                    special_points: poles.clone(),
                    corrupt_prime: Some((0, corrupt)),
                    discard_lanes: 3,
                }),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(faulty.coefficients, clean.coefficients);
    let unlucky = &faulty.report.unlucky;
    assert_eq!(unlucky.get("injected"), Some(&3));
    let detected: usize = unlucky
        .iter()
        .filter(|(reason, _)| *reason != "injected")
        .map(|(_, n)| n)
        .sum();
    // The forced points are probed first; each of them is a pole of the
    // reduction, so a probe there must fail rather than return values.
    assert!(
        detected >= 1,
        "a forced pole is an unlucky lane: {unlucky:?}"
    );
    for pole in &poles {
        assert!(
            matches!(
                session.probe_explicit(
                    &targets,
                    &sample(PRIME, &[("d", q(37, 11)), ("eta", pole.clone())])
                ),
                Err(SolverError::UnluckyBoxSample { .. })
            ),
            "eta = {pole}"
        );
    }
    // The corrupted prime costs extra primes and is left out of the result.
    assert!(faulty.reconstruction_primes().len() > clean.reconstruction_primes().len());
    // A corruption outside the request is refused, not ignored.
    let bad = FfOptions {
        fault: Some(FaultInjection {
            corrupt_prime: Some((0, clean.report.entries)),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(matches!(
        session.materialize_ff(&targets, &fixed, &bad),
        Err(SolverError::InvalidInput(_))
    ));
}

/// A run with a probe log, resumed in a fresh session, replays the probes
/// and gives the identical result; a fault-injected run does not reuse the
/// clean log.
#[test]
fn finite_field_lane_resumes_from_its_probe_log() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("sector-box-k6-ff-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fixed = [("d".to_string(), q(37, 11))];
    let options = FfOptions {
        checkpoint_dir: Some(dir.clone()),
        ..Default::default()
    };
    let (mut first, targets) = session(2);
    let cold = first.materialize_ff(&targets, &fixed, &options).unwrap();
    assert_eq!(cold.report.resumed_probes, 0);
    let (mut second, _) = session(2);
    let warm = second.materialize_ff(&targets, &fixed, &options).unwrap();
    assert!(warm.report.resumed_probes > 0);
    assert_eq!(warm.coefficients, cold.coefficients);
    assert_eq!(warm.reconstruction_primes(), cold.reconstruction_primes());
    let faulty = FfOptions {
        fault: Some(FaultInjection {
            discard_lanes: 1,
            ..Default::default()
        }),
        ..options.clone()
    };
    let other = second.materialize_ff(&targets, &fixed, &faulty).unwrap();
    assert_eq!(other.report.resumed_probes, 0);
    assert_eq!(other.coefficients, cold.coefficients);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Bad finite-field requests are typed errors.
#[test]
fn finite_field_lane_rejects_bad_requests() {
    let (mut session, targets) = session(1);
    let invalid = |result: Result<BoxSolution<N>, SolverError>| {
        matches!(result, Err(SolverError::InvalidInput(_)))
    };
    let d = ("d".to_string(), q(37, 11));
    let eta = ("eta".to_string(), q(1, 7));
    let default = FfOptions::default();
    // Not exactly one free parameter.
    assert!(invalid(session.materialize_ff(&targets, &[], &default)));
    assert!(invalid(session.materialize_ff(
        &targets,
        &[d.clone(), eta],
        &default
    )));
    // Not demanded.
    assert!(invalid(session.materialize_ff(
        &[[1, 1, 1, 1, 1, 3]],
        std::slice::from_ref(&d),
        &default
    )));
    // Bunch size, no verification.
    for options in [
        FfOptions {
            bunch: 3,
            ..Default::default()
        },
        FfOptions {
            verify_points: 0,
            ..Default::default()
        },
        FfOptions {
            max_verifications: 0,
            ..Default::default()
        },
    ] {
        assert!(
            invalid(session.materialize_ff(&targets, std::slice::from_ref(&d), &options)),
            "{options:?}"
        );
    }
    // A prime budget that cannot be met is an error, not a partial result.
    let starved = FfOptions {
        max_primes: 1,
        ..Default::default()
    };
    assert!(
        session
            .materialize_ff(&targets, &[("d".into(), q(319999, 80000))], &starved)
            .is_err()
    );
}
