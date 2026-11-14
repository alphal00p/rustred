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

use rustred::algebra::reconstruct::{RationalFunction, rational_image, reconstruction_primes};
use rustred::algebra::{CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::SolverError;
use rustred::solver::sector_box::{
    BoxLaportaSession, BoxOptions, BoxPhase, BoxSolution, CensusRouter, CertificateMode,
    ColumnOrder, DEFAULT_CERTIFICATE_PRIME, ExplicitSample, FaultInjection, FfOptions,
    HARNESS_SOLUTION_SCHEMA, Lane, MasterCheckMode, ParameterAssignment, RouterChoice, Seeding,
    SlotPriority, solve_box_laporta,
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

/// The exact lane equals the finite-field lane coefficient by coefficient
/// (both parameter splits); the exact and the probabilistic certificate
/// accept the reduction and reject a perturbed one.
#[test]
fn exact_lane_equals_finite_field_lane_and_certificates_hold() {
    let (mut session, targets) = session(2);
    let context = session.family().coefficient_context().clone();
    for fixed in [("d", q(37, 11)), ("eta", q(3, 7))] {
        let fixed = vec![(fixed.0.to_string(), fixed.1)];
        let ff = session
            .materialize_ff(&targets, &fixed, &FfOptions::default())
            .unwrap();
        let exact = session.materialize_exact(&targets, &fixed).unwrap();
        assert_eq!(exact.free, vec![ff.free.clone()]);
        assert!(ff.mismatches(&exact, &context).unwrap().is_empty());
        let certificate = session.certify(&ff, None, 0).unwrap();
        assert_eq!(
            (certificate.mode, certificate.rules),
            (CertificateMode::Exact, targets.len())
        );
        let certificate = session
            .certify(&ff, Some(DEFAULT_CERTIFICATE_PRIME), 17)
            .unwrap();
        assert_eq!(certificate.mode, CertificateMode::Probabilistic);
        assert_eq!(certificate.prime, Some(DEFAULT_CERTIFICATE_PRIME));
        let mut wrong = ff.clone();
        let (t, m) = wrong
            .coefficients
            .iter()
            .enumerate()
            .flat_map(|(t, row)| row.iter().enumerate().map(move |(m, f)| (t, m, f)))
            .find(|(_, _, function)| !function.is_zero() && function.degrees() != (0, 0))
            .map(|(t, m, _)| (t, m))
            .unwrap();
        wrong.coefficients[t][m].numerator[0] = &wrong.coefficients[t][m].numerator[0] + &q(1, 1);
        assert_eq!(wrong.mismatches(&exact, &context).unwrap(), vec![(t, m)]);
        assert!(matches!(
            session.certify(&wrong, None, 0),
            Err(SolverError::Certification(_))
        ));
        assert!(matches!(
            session.certify(&wrong, Some(DEFAULT_CERTIFICATE_PRIME), 17),
            Err(SolverError::Certification(_))
        ));
    }
    // Every parameter free: the exact lane still works.
    let symbolic = session.materialize_exact(&targets, &[]).unwrap();
    assert_eq!(symbolic.free, vec!["d".to_string(), "eta".to_string()]);
}

/// A certificate at a prime the reconstruction lifted over or was verified
/// at is refused: it would not be independent. The campaign's default
/// certificate prime 2^62 - 171 is the 7th reconstruction prime.
#[test]
fn certificates_refuse_primes_the_reconstruction_used() {
    assert_eq!(reconstruction_primes(7)[6], (1u64 << 62) - 171);
    const { assert!(DEFAULT_CERTIFICATE_PRIME > 1 << 62) };
    let (mut session, targets) = session(1);
    let solution = session
        .materialize_ff(&targets, &[("d".into(), q(37, 11))], &FfOptions::default())
        .unwrap();
    let used: Vec<u64> = solution
        .reconstruction_primes()
        .into_iter()
        .chain(solution.verification_primes())
        .collect();
    assert!(used.len() >= 3);
    for prime in used {
        assert!(
            matches!(
                session.certify(&solution, Some(prime), 1),
                Err(SolverError::InvalidInput(_))
            ),
            "{prime}"
        );
    }
    for prime in [1, 2, 15, (1u64 << 63) + 29] {
        assert!(
            matches!(
                session.certify(&solution, Some(prime), 1),
                Err(SolverError::InvalidInput(_))
            ),
            "{prime}"
        );
    }
    // Any other prime is independent.
    assert!(
        session
            .certify(&solution, Some((1u64 << 62) - 171), 1)
            .is_ok()
    );
    // A solution that does not belong to the session is refused.
    let mut foreign = solution.clone();
    foreign.masters.swap(0, 1);
    assert!(matches!(
        session.certify(&foreign, Some(DEFAULT_CERTIFICATE_PRIME), 1),
        Err(SolverError::InvalidInput(_))
    ));
    let mut short = solution.clone();
    short.coefficients[0].pop();
    assert!(matches!(
        session.certify(&short, None, 1),
        Err(SolverError::InvalidInput(_))
    ));
    // The public methods of a solution check its shape instead of
    // truncating or panicking.
    let context = session.family().coefficient_context().clone();
    assert!(matches!(
        short.to_dynamic_solution(&context),
        Err(SolverError::InvalidInput(_))
    ));
    let exact = session
        .materialize_exact(&targets, &[("d".into(), q(37, 11))])
        .unwrap();
    assert!(matches!(
        short.mismatches(&exact, &context),
        Err(SolverError::InvalidInput(_))
    ));
    let mut unfixed = solution.clone();
    unfixed.fixed.clear();
    assert!(matches!(
        unfixed.check_shape(),
        Err(SolverError::InvalidInput(_))
    ));
    let mut wide = solution.clone();
    wide.arity = N + 1;
    assert!(matches!(
        wide.check_shape(),
        Err(SolverError::InvalidInput(_))
    ));
    assert!(solution.check_shape().is_ok());
}

/// The golden reproductions at both values of d pass the exact
/// certificate and a probabilistic certificate at a prime independent of
/// their reconstruction.
#[test]
fn golden_reproductions_are_certified_at_an_independent_prime() {
    let (mut session, targets) = session(2);
    for d0 in [q(37, 11), q(319999, 80000)] {
        let solution = session
            .materialize_ff(&targets, &[("d".into(), d0)], &campaign_ff())
            .unwrap();
        assert!(
            !solution
                .reconstruction_primes()
                .contains(&DEFAULT_CERTIFICATE_PRIME)
        );
        let certificate = session
            .certify(&solution, Some(DEFAULT_CERTIFICATE_PRIME), 0x6332_7432)
            .unwrap();
        assert_eq!(certificate.rules, solution.targets.len());
        session.certify(&solution, None, 0).unwrap();
    }
}

fn written(session: &mut BoxLaportaSession<N>, targets: &[[i16; N]]) -> (BoxSolution<N>, String) {
    let solution = session
        .materialize_ff(targets, &[("d".into(), q(37, 11))], &FfOptions::default())
        .unwrap();
    let certificate = session
        .certify(&solution, Some(DEFAULT_CERTIFICATE_PRIME), 3)
        .unwrap();
    let text = solution
        .to_harness_json(
            &session.harness_family(),
            &serde_json::json!({"job": "k6"}),
            &[certificate],
        )
        .unwrap();
    (solution, text)
}

/// `rustred-harness-solution.v1` round trip: writing and reading gives the
/// identical solution (coefficients, primes, stats, provenance) and record,
/// and rewriting gives the identical text.
#[test]
fn harness_solution_round_trips() {
    let (mut session, targets) = session(2);
    let (solution, text) = written(&mut session, &targets);
    let family = session.harness_family();
    let (read, record) = BoxSolution::<N>::from_harness_json(&text, &family).unwrap();
    // Timings are written to the microsecond; everything else is exact.
    let timings = |s: &BoxSolution<N>| {
        let r = &s.report;
        let q = &r.reconstruction;
        [
            q.probe_seconds,
            q.reconstruction_seconds,
            q.verification_seconds,
            r.build_seconds,
            r.seconds,
        ]
    };
    for (a, b) in timings(&read).iter().zip(timings(&solution)) {
        assert!((a - b).abs() <= 1e-6, "{a} vs {b}");
    }
    let mut untimed = solution.clone();
    untimed.report.reconstruction.probe_seconds = read.report.reconstruction.probe_seconds;
    untimed.report.reconstruction.reconstruction_seconds =
        read.report.reconstruction.reconstruction_seconds;
    untimed.report.reconstruction.verification_seconds =
        read.report.reconstruction.verification_seconds;
    untimed.report.build_seconds = read.report.build_seconds;
    untimed.report.seconds = read.report.seconds;
    assert_eq!(read, untimed);
    assert_eq!(record.family, family);
    assert_eq!(record.extra, serde_json::json!({"job": "k6"}));
    assert_eq!(record.certificates.len(), 1);
    assert_eq!(
        record.certificates[0].prime,
        Some(DEFAULT_CERTIFICATE_PRIME)
    );
    assert!(record.content_checked);
    assert_eq!(
        read.to_harness_json(&family, &record.extra, &record.certificates)
            .unwrap(),
        text
    );
    // The writer refuses an inconsistent solution instead of truncating it.
    let mut short = solution.clone();
    short.coefficients[0].pop();
    assert!(matches!(
        short.to_harness_json(&family, &Value::Null, &[]),
        Err(SolverError::InvalidInput(_))
    ));
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema"], HARNESS_SOLUTION_SCHEMA);
    // A file read back is certified like the original.
    session.certify(&read, None, 0).unwrap();
}

/// The reader refuses corrupted or foreign files instead of guessing.
#[test]
fn harness_reader_rejects_bad_files() {
    let (mut session, targets) = session(1);
    let (solution, text) = written(&mut session, &targets);
    let family = session.harness_family();
    let value: Value = serde_json::from_str(&text).unwrap();
    // Each check is exercised on its own: the content digest is removed
    // first (an older v1 file), except for the digest cases at the end.
    let mut undigested = value.clone();
    undigested.as_object_mut().unwrap().remove("content_blake3");
    let refused_from = |base: &Value, edit: &dyn Fn(&mut Value), what: &str| {
        let mut bad = base.clone();
        edit(&mut bad);
        let result = BoxSolution::<N>::from_harness_json(&bad.to_string(), &family);
        assert!(
            matches!(result, Err(SolverError::InvalidInput(_))),
            "{what}: {result:?}"
        );
    };
    let refused = |edit: &dyn Fn(&mut Value), what: &str| refused_from(&undigested, edit, what);
    let (t, _) = (0..targets.len())
        .map(|t| (t, solution.terms(t).count()))
        .max_by_key(|(_, n)| *n)
        .unwrap();
    refused(
        &|v| v["schema"] = "rustred-harness-solution.v2".into(),
        "schema",
    );
    refused(
        &|v| v["family"]["fingerprint"] = "another family".into(),
        "family",
    );
    refused(
        &|v| v["family"]["router_fingerprint"] = "00".into(),
        "router",
    );
    refused(&|v| v["provenance"]["lane"] = "Exact".into(), "lane");
    refused(&|v| v["fixed"] = serde_json::json!({}), "unfixed parameter");
    refused(
        &|v| v["fixed"]["d"] = "74/22".into(),
        "non-canonical fixed value",
    );
    refused(
        &|v| v["rules"][t]["terms"][0]["coefficient"] = "1/2*eta+".into(),
        "garbage coefficient",
    );
    refused(
        &|v| v["rules"][t]["terms"][0]["coefficient"] = "(1-1/2*eta)/(1-1/2*eta)".into(),
        "not in lowest terms",
    );
    refused(
        &|v| v["rules"][t]["terms"][0]["coefficient"] = "0".into(),
        "zero term",
    );
    refused(
        &|v| v["rules"][t]["terms"][0]["integral"] = serde_json::json!([3, 1, 1, 1, 1, 1]),
        "unknown master",
    );
    refused(
        &|v| {
            let first = v["rules"][t]["terms"][0].clone();
            v["rules"][t]["terms"].as_array_mut().unwrap().push(first);
        },
        "master twice",
    );
    refused(
        &|v| v["masters"][0] = serde_json::json!([1, 1, 1, 1, 1]),
        "short integral",
    );
    refused(
        &|v| {
            let prime = v["stats"]["primes"][0]["prime"].clone();
            v["stats"]["verifications"][0]["prime"] = prime;
        },
        "verification prime reused",
    );
    refused(
        &|v| {
            let prime = v["stats"]["primes"][0]["prime"].clone();
            v["certificates"][0]["prime"] = prime;
        },
        "certificate prime reused",
    );
    refused(
        &|v| v["stats"]["verifications"] = serde_json::json!([]),
        "no verification",
    );
    refused(&|v| v["stats"]["entries"] = 1.into(), "stats inconsistent");
    assert!(BoxSolution::<N>::from_harness_json("{not json", &family).is_err());
    refused_from(
        &value,
        &|v| v["content_blake3"] = "00".into(),
        "wrong digest",
    );
    // A canonical but wrong coefficient is caught by the content digest;
    // without the digest (an older v1 file) it is read, and the certificate
    // rejects it.
    let other = if value["rules"][t]["terms"][0]["coefficient"] == "1" {
        "2"
    } else {
        "1"
    };
    refused_from(
        &value,
        &|v| v["rules"][t]["terms"][0]["coefficient"] = other.into(),
        "tampered coefficient",
    );
    let mut tampered = value.clone();
    tampered["rules"][t]["terms"][0]["coefficient"] = other.into();
    tampered.as_object_mut().unwrap().remove("content_blake3");
    let (read, record) =
        BoxSolution::<N>::from_harness_json(&tampered.to_string(), &family).unwrap();
    assert!(!record.content_checked);
    assert!(matches!(
        session.certify(&read, Some(DEFAULT_CERTIFICATE_PRIME), 5),
        Err(SolverError::Certification(_))
    ));
    // An unchanged file is accepted, with and without its digest.
    assert!(BoxSolution::<N>::from_harness_json(&value.to_string(), &family).is_ok());
    assert!(BoxSolution::<N>::from_harness_json(&undigested.to_string(), &family).is_ok());
}

/// VALUE SOURCE: the campaign engine's golden files are read (same family
/// and router fingerprints), and every one passes the exact certificate and
/// a probabilistic certificate at a prime independent of its
/// reconstruction, against this engine's session.
#[test]
fn golden_files_are_certified_at_an_independent_prime() {
    let (mut session, _) = session(2);
    let family = session.harness_family();
    for file in ["solution-37_11.json", "solution-319999_80000.json"] {
        let text = std::fs::read_to_string(format!(
            "{}/tests/fixtures/sector_box_k6/{file}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let (solution, record) = BoxSolution::<N>::from_harness_json(&text, &family).unwrap();
        assert_eq!(record.family.name, "w6_k6_eta_slot0");
        assert!(!record.content_checked, "campaign files carry no digest");
        assert!(
            !solution
                .reconstruction_primes()
                .contains(&DEFAULT_CERTIFICATE_PRIME)
        );
        let certificate = session
            .certify(&solution, Some(DEFAULT_CERTIFICATE_PRIME), 0x6332_7432)
            .unwrap();
        assert_eq!(certificate.rules, solution.targets.len());
        session.certify(&solution, None, 0).unwrap();
    }
}

/// The census router fingerprint (format v1, `Debug` renderings) is the one
/// in every solution file of the campaign engine. If this fails, a
/// rendering changed: existing files would be refused by the reader.
#[test]
fn router_fingerprint_matches_the_campaign_files() {
    let (session, _) = session(1);
    assert_eq!(
        session.harness_family().router_fingerprint.as_deref(),
        Some("5768f971f58aa0b01f0508ca51cabff8fb1234f1048d192072059ed9e388f73b")
    );
}

/// The facade equals a session's `materialize`, in both lanes.
#[test]
fn facade_matches_the_session() {
    let (_, targets) = session(1);
    let lists: Vec<Vec<i16>> = targets.iter().map(|t| t.to_vec()).collect();
    let family = k6_family();
    let context = family.coefficient_context().clone();
    let assignment = ParameterAssignment {
        values: vec![(
            "d".into(),
            context
                .try_div(
                    &context.integer(37),
                    &context.integer(11),
                    ExactAlgebraLimits::default(),
                )
                .unwrap(),
        )],
    };
    for lane in [Lane::Exact, Lane::FiniteField(FfOptions::default())] {
        let via_facade = solve_box_laporta::<N>(
            family.clone(),
            &CutConstraint::none(N).unwrap(),
            &lists,
            &[TOP.to_vec()],
            RouterChoice::Census,
            options(1),
            &assignment,
            &lane,
        )
        .unwrap();
        let mut fresh = new_session(1);
        fresh.extend(&targets, &[TOP]).unwrap();
        let via_session = fresh.materialize(&targets, &assignment, &lane).unwrap();
        assert_eq!(format!("{via_facade:?}"), format!("{via_session:?}"));
        assert_eq!(via_session.residuals.len(), 12);
    }
    let symbolic = ParameterAssignment {
        values: vec![("d".into(), context.parameter("eta").unwrap())],
    };
    let mut fresh = new_session(1);
    fresh.extend(&targets, &[TOP]).unwrap();
    assert!(matches!(
        fresh.materialize(&targets, &symbolic, &Lane::Exact),
        Err(SolverError::InvalidInput(_))
    ));
    assert!(
        solve_box_laporta::<7>(
            family,
            &CutConstraint::none(7).unwrap(),
            &lists,
            &[],
            RouterChoice::Census,
            options(1),
            &assignment,
            &Lane::Exact
        )
        .is_err()
    );
}
