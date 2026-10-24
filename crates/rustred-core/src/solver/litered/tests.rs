use std::sync::Arc;

use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::reduction::ReductionLimits;
use crate::sector::symmetry::{
    SectorSymmetryCensus, SectorSymmetryCensusControl, SectorSymmetryCensusOptions,
};
use crate::sector::{Mask, OrderingPolicy, zero};
use crate::solver::{
    CandidateFamilyAdmission, CandidateReducer, CandidateReductionError, SectorConfig,
    SectorSolveOptions, SectorSolver, SourceSystem,
};

use super::*;

/// Two-loop vacuum sunset `k0^2-1, k1^2-1, (k0-k1)^2-1-eta`; with `eta`
/// absent it is the unit-mass sunset.
fn sunset(deformed: bool) -> IntegralFamily {
    let parameters: &[&str] = if deformed { &["d", "eta"] } else { &["d"] };
    let context = CoefficientContext::try_new(parameters.iter().copied()).unwrap();
    let rows: [[i64; 3]; 3] = [[1, 0, 0], [0, 0, 1], [1, -2, 1]];
    let denominators = rows
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let mut constant = context.integer(-1);
            if deformed && slot == 2 {
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
    IntegralFamily::new(
        "litered_sunset",
        vec!["k0".into(), "k1".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        vec![],
        vec![context.zero(); 3],
    )
    .unwrap()
}

fn solved(
    family: &IntegralFamily,
) -> (
    Vec<([bool; 3], crate::solver::SectorSolution<3>)>,
    Vec<zero::Certificate>,
) {
    let analyzer = zero::Analyzer::try_unrestricted(family).unwrap();
    let mut certificates = Vec::new();
    let mut nonzero = Vec::new();
    let mut zeros = Vec::new();
    for code in 0..8usize {
        let sector: [bool; 3] = std::array::from_fn(|axis| code & (1 << axis) != 0);
        match analyzer.analyze(&Mask::try_new(sector).unwrap()).unwrap() {
            zero::Decision::ProvedZero(certificate) => {
                zeros.push(sector);
                certificates.push(certificate);
            }
            zero::Decision::Inconclusive(_) => nonzero.push(sector),
            zero::Decision::Excluded(_) => panic!("unrestricted fixture"),
        }
    }
    let source = SourceSystem::<3>::from_family(family).unwrap();
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros),
        ..Default::default()
    };
    let records = nonzero
        .into_iter()
        .map(|sector| {
            let solution = SectorSolver::new(&source, sector, config.clone())
                .unwrap()
                .solve_sector(SectorSolveOptions::default())
                .unwrap();
            (sector, solution)
        })
        .collect();
    (records, certificates)
}

fn reducer(family: &IntegralFamily, admission: CandidateFamilyAdmission) -> CandidateReducer<3> {
    let (records, zeros) = solved(family);
    CandidateReducer::try_new_with_admission(
        family,
        [true; 3],
        OrderingPolicy::SpiredUncutV1,
        records,
        zeros,
        ReductionLimits::default(),
        None,
        admission,
    )
    .unwrap()
}

fn key(powers: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn targets() -> Vec<IntegralKey> {
    [
        [1, 1, 1],
        [2, 1, 1],
        [1, 2, 1],
        [1, 1, 2],
        [2, 2, 1],
        [3, 1, 2],
        [1, 1, 3],
        [2, 1, -1],
        [-1, 2, 2],
        [2, 0, 3],
    ]
    .map(key)
    .to_vec()
}

#[test]
fn parametric_admission_only_bypasses_the_unit_mass_predicate() {
    let family = sunset(true);
    let (records, zeros) = solved(&family);
    let strict = CandidateReducer::try_new_with_admission(
        &family,
        [true; 3],
        OrderingPolicy::SpiredUncutV1,
        records,
        zeros,
        ReductionLimits::default(),
        None,
        CandidateFamilyAdmission::UnitMassVacuum,
    );
    assert!(matches!(
        strict,
        Err(CandidateReductionError::UnsupportedFamily(_))
    ));
    let mut parametric = reducer(&family, CandidateFamilyAdmission::ParametricVacuum);
    let decomposition = parametric.reduce_unit_mass(&key([2, 1, 1])).unwrap();
    assert!(!decomposition.is_zero());
    // The unit-mass family still passes the strict default policy.
    let _ = reducer(&sunset(false), CandidateFamilyAdmission::UnitMassVacuum);
}

#[test]
fn zp_dag_matches_the_exact_candidate_applier_and_sweeps_agree() {
    let family = sunset(true);
    let candidate = Arc::new(reducer(&family, CandidateFamilyAdmission::ParametricVacuum));
    let mut session =
        LiteRedSession::new(Arc::clone(&candidate), None, LiteRedOptions::default()).unwrap();
    let targets = targets();
    let ids = session.add_targets(&targets).unwrap();
    let statistics = session.statistics(&ids);
    assert_eq!(statistics.uncovered, 0);
    assert!(statistics.rule_nodes > 0);
    let forward = session.forward().unwrap();
    let adjoint = session.adjoint(&ids).unwrap();
    let sample = session.structure_sample().clone();
    let mut exact = candidate.fork_empty();
    let terminal_ids: std::collections::BTreeMap<&IntegralKey, usize> = session
        .terminals()
        .iter()
        .enumerate()
        .map(|(id, key)| (key, id))
        .collect();
    for ((target, &id), adjoint_row) in targets.iter().zip(&ids).zip(&adjoint) {
        let mut dense = vec![0u64; session.terminals().len()];
        for &(terminal, value) in forward[id as usize].as_ref().unwrap() {
            dense[terminal as usize] = value;
        }
        assert_eq!(&dense, adjoint_row, "forward/adjoint at {target:?}");
        let mut expected = vec![0u64; dense.len()];
        for (terminal, coefficient) in exact.reduce_unit_mass(target).unwrap().terms() {
            expected[terminal_ids[terminal]] = sample.coefficient(coefficient).unwrap().unwrap();
        }
        assert_eq!(dense, expected, "Zp DAG versus exact applier at {target:?}");
        // Exact selection agrees with the Zp-dispatched rule.
        if let NodeKind::Rule { ordinal, .. } = session.structure().kind(id) {
            let (_, selected) = candidate.program_view().exact_selection(target).unwrap();
            assert_eq!(selected, ordinal);
        }
    }
    assert!(session.sample_statistics().compiled_rules > 0);
}

#[test]
fn exact_point_reconstruction_reproduces_exact_coefficients() {
    let family = sunset(true);
    let candidate = Arc::new(reducer(&family, CandidateFamilyAdmission::ParametricVacuum));
    let options = LiteRedOptions {
        workers: 2,
        ..Default::default()
    };
    let mut session = LiteRedSession::new(Arc::clone(&candidate), None, options).unwrap();
    let targets = targets();
    let ids = session.add_targets(&targets).unwrap();
    // Without census rows every terminal is its own master.
    let report = session.compress(&[]).unwrap();
    assert_eq!(report.masters.len(), session.terminals().len());
    let point = [
        Rational::from((Integer::from(37), Integer::from(11))),
        Rational::from((Integer::from(-5), Integer::from(13))),
    ];
    let reduction = session.reduce_exact_at_point(&point, &ids).unwrap();
    assert!(reduction.primes_used >= 2);
    let mut exact = candidate.fork_empty();
    let masters = report.masters.clone();
    for prime in reconstruction_primes(400).into_iter().rev().take(2) {
        let field = PrimeField::new(prime).unwrap();
        let sample = ZpSample::from_rationals(field.clone(), &point).unwrap();
        for (target, row) in targets.iter().zip(&reduction.coefficients) {
            let terms = exact.reduce_unit_mass(target).unwrap();
            for (master, value) in masters.iter().zip(row) {
                let expected = terms
                    .terms()
                    .get(master)
                    .map_or(0, |c| sample.coefficient(c).unwrap().unwrap());
                assert_eq!(
                    field.from_rational(value),
                    Some(expected),
                    "{target:?} {master:?}"
                );
            }
        }
    }
}

#[test]
fn census_compression_merges_symmetric_terminals_with_either_row_policy() {
    let family = Arc::new(sunset(true));
    let candidate = Arc::new(reducer(&family, CandidateFamilyAdmission::ParametricVacuum));
    let analyzer = zero::Analyzer::try_unrestricted(&family).unwrap();
    let census = Arc::new(
        SectorSymmetryCensus::discover_subsectors(
            Arc::clone(&family),
            &Mask::try_new([true; 3]).unwrap(),
            &analyzer,
            SectorSymmetryCensusOptions::default(),
            &SectorSymmetryCensusControl::default(),
        )
        .unwrap(),
    );
    let mut masters = Vec::new();
    for rows in [SymmetryRows::GroupOrbit, SymmetryRows::Generators] {
        let options = LiteRedOptions {
            rows,
            ..Default::default()
        };
        let mut session =
            LiteRedSession::new(Arc::clone(&candidate), Some(Arc::clone(&census)), options)
                .unwrap();
        let ids = session.add_targets(&targets()).unwrap();
        let report = session.compress(&[]).unwrap();
        assert!(report.rows.rows > 0);
        assert!(report.masters.len() < report.terminals, "{report:?}");
        // Pivots are stable at an independent prime and point.
        let field = PrimeField::new(reconstruction_primes(3)[2]).unwrap();
        let sample = ZpSample::new(field, vec![123_456_789, 987_654_321]);
        session.reduce_at(&sample, &ids).unwrap();
        masters.push(report.masters);
    }
    assert_eq!(masters[0], masters[1]);
    // Preferred columns are forced to the easy end and become masters.
    let mut session = LiteRedSession::new(
        Arc::clone(&candidate),
        Some(Arc::clone(&census)),
        LiteRedOptions::default(),
    )
    .unwrap();
    session.add_targets(&targets()).unwrap();
    let preferred = key([2, 1, 1]);
    let report = session.compress(std::slice::from_ref(&preferred)).unwrap();
    assert_eq!(report.masters.len(), masters[0].len());
    assert!(report.masters.contains(&preferred));
}

#[test]
fn prime_utilities_are_consistent() {
    assert!(is_prime_u64(DEFAULT_PRIME));
    let primes = reconstruction_primes(4);
    assert!(
        primes
            .iter()
            .all(|&p| is_prime_u64(p) && p != DEFAULT_PRIME)
    );
    assert!(primes.windows(2).all(|w| w[0] > w[1]));
    let field = PrimeField::new(primes[0]).unwrap();
    for a in [1u64, 2, 12345, primes[0] - 1] {
        assert_eq!(field.mul(a, field.inv(a).unwrap()), 1);
    }
    assert_eq!(field.from_i64(-1), primes[0] - 1);
}

#[test]
fn univariate_reconstruction_in_eta_matches_exact_coefficients() {
    let family = sunset(true);
    let candidate = Arc::new(reducer(&family, CandidateFamilyAdmission::ParametricVacuum));
    let options = LiteRedOptions {
        workers: 4,
        ..Default::default()
    };
    let mut session = LiteRedSession::new(Arc::clone(&candidate), None, options).unwrap();
    let targets = targets();
    let ids = session.add_targets(&targets).unwrap();
    let report = session.compress(&[]).unwrap();
    let d = Rational::from((Integer::from(5399), Integer::from(1350)));
    let reduction = session
        .reduce_univariate(&[Some(d.clone()), None], &ids)
        .unwrap();
    assert_eq!(reduction.free_parameter, 1);
    let mut exact = candidate.fork_empty();
    let prime = reconstruction_primes(500)[499];
    let field = PrimeField::new(prime).unwrap();
    // eta = 3 is the (1+1)^2 = 1+eta threshold of this sunset; avoid poles.
    for x in [12_345u64, 1_000_003, 77_777_777_777] {
        let sample = ZpSample::new(field.clone(), vec![field.from_rational(&d).unwrap(), x]);
        for (target, row) in targets.iter().zip(&reduction.coefficients) {
            let terms = exact.reduce_unit_mass(target).unwrap();
            for (master, function) in report.masters.iter().zip(row) {
                let expected = terms
                    .terms()
                    .get(master)
                    .map_or(0, |c| sample.coefficient(c).unwrap().unwrap());
                assert_eq!(function.evaluate(x, &field), Some(expected), "{target:?}");
            }
        }
    }
}

#[test]
fn in_process_generation_and_dynamic_lane_agree_with_the_fixture() {
    let family = Arc::new(sunset(true));
    let (generated, statistics) = generate_candidate_program::<3>(
        &family,
        [true; 3],
        2,
        CandidateFamilyAdmission::ParametricVacuum,
        None,
    )
    .unwrap();
    // An explicit coordinate priority keeps keys and terminal support valid.
    let (prioritized, _) = generate_candidate_program::<3>(
        &family,
        [true; 3],
        2,
        CandidateFamilyAdmission::ParametricVacuum,
        Some(&[2, 0, 1]),
    )
    .unwrap();
    assert!(!prioritized.terminals().is_empty());
    assert_eq!(isps_at_ends_priority(&[true; 3]), None);
    assert_eq!(
        isps_at_ends_priority(&[true, true, false, true, false]),
        Some(vec![2, 0, 1, 3, 4])
    );
    let fixture = reducer(&family, CandidateFamilyAdmission::ParametricVacuum);
    assert_eq!(generated.terminals(), fixture.terminals());
    assert_eq!(statistics.terminals, fixture.terminals().len());
    let (mut lane, report) = DynamicLiteRedSession::generate(
        Arc::clone(&family),
        &[true; 3],
        2,
        true,
        LiteRedOptions::default(),
        None,
    )
    .unwrap();
    assert_eq!(report.census_classes.is_some(), true);
    let ids = lane.add_targets(&targets()).unwrap();
    let compression = lane.compress(&[]).unwrap();
    assert!(compression.masters.len() < compression.terminals);
    let point = [
        Rational::from((Integer::from(7), Integer::from(3))),
        Rational::from((Integer::from(-2), Integer::from(5))),
    ];
    let exact = lane.reduce_exact_at_point(&point, &ids).unwrap();
    assert_eq!(exact.coefficients.len(), ids.len());
}
