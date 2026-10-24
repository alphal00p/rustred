//! UNCERTIFIED LiteRed lane on RustFlow's native four-loop wheel (P14).
//!
//! Slow regression tests (about one minute of in-process candidate
//! generation per family on 8 threads), ignored by default:
//!
//! ```text
//! cargo test --release -p rustred --test litered_p14 -- --ignored --test-threads 1
//! ```
//!
//! The family is the one of `sector_symmetry_census.rs`: physical lines
//! `D0..D7`, auxiliary ISPs `D8 = k0.k1`, `D9 = k0.k2`, and an optional
//! deformation `D_s -> D_s - eta`. Each test runs the path RustFlow uses
//! (`DynamicLiteRedSession::generate` with the ISPs-at-ends priority and the
//! RustRed census) and checks:
//!
//! - the number of candidate terminals (the RustRed family-solve residuals)
//!   and of masters after census compression: 216 -> 42 (rim),
//!   246 -> 42 (spoke), 179 -> 14 (undeformed);
//! - that every census class carries a master, the top-sector master count
//!   and that the target `T` is a master. The counts equal the independent
//!   Kira oracle for these families (42/27/3, 42/26/4, 14/11/2);
//! - for the deformed families, that two independent reconstructions of
//!   non-trivial coefficients agree: exact values at a rational `(d, eta)`
//!   point (multi-prime CRT) and the `Q(eta)` reconstruction at the same `d`
//!   (Thiele), compared at that `eta` modulo `2^61 - 1`.
use std::sync::Arc;

use rustred::algebra::{CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use rustred::solver::litered::{
    DEFAULT_PRIME, DynamicLiteRedSession, LiteRedOptions, PrimeField, isps_at_ends_priority,
};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

/// RustFlow `native_family` coefficients over
/// (k0k0, k0k1, k0k2, k0k3, k1k1, k1k2, k1k3, k2k2, k2k3, k3k3).
const SCALAR_PRODUCTS: [[i64; 10]; 10] = [
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
    [1, -2, 0, -2, 1, 0, 2, 0, 0, 1],
    [0, 0, 0, 0, 1, 2, 2, 1, 2, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, -2, -2, -2, 1, 2, 2, 1, 2, 1],
    [0, 0, 0, 0, 1, 2, 0, 1, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
];
const CONSTANTS: [i64; 10] = [-1, -1, -1, -1, -1, -1, -1, -1, 0, 0];
const PHYSICAL: usize = 8;
const TOP_SECTOR: u16 = 255;

fn p14(deformed: Option<usize>) -> Arc<IntegralFamily> {
    let parameters: &[&str] = if deformed.is_some() {
        &["d", "eta"]
    } else {
        &["d"]
    };
    let context = CoefficientContext::try_new(parameters.iter().copied()).unwrap();
    let denominators = SCALAR_PRODUCTS
        .iter()
        .zip(CONSTANTS)
        .enumerate()
        .map(|(slot, (row, constant))| {
            let mut constant = context.integer(constant);
            if deformed == Some(slot) {
                constant = context
                    .try_sub(
                        &constant,
                        &context.parameter("eta").unwrap(),
                        ExactAlgebraLimits::default(),
                    )
                    .unwrap();
            }
            AffineDenominator::new(constant, row.map(|value| context.integer(value)).to_vec())
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            "p14_native",
            (0..4).map(|loop_| format!("k{loop_}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); 10],
        )
        .unwrap(),
    )
}

fn key(powers: [i64; 10]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn rational(numerator: i64, denominator: i64) -> Rational {
    Rational::from((Integer::from(numerator), Integer::from(denominator)))
}

fn sector_id(key: &IntegralKey) -> u16 {
    key.powers()
        .iter()
        .enumerate()
        .filter(|(_, power)| **power > 0)
        .map(|(slot, _)| 1 << slot)
        .sum()
}

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
}

struct Expected {
    terminals: usize,
    masters: usize,
    classes: usize,
    top_sector_masters: usize,
}

fn check(deformed: Option<usize>, expected: Expected) {
    let family = p14(deformed);
    let root: Vec<bool> = (0..10).map(|slot| slot < PHYSICAL).collect();
    let priority = isps_at_ends_priority(&root).unwrap();
    assert_eq!(priority, vec![8, 0, 1, 2, 3, 4, 5, 6, 7, 9]);
    let options = LiteRedOptions {
        workers: workers(),
        ..LiteRedOptions::default()
    };
    let (mut lane, report) = DynamicLiteRedSession::generate(
        Arc::clone(&family),
        &root,
        workers(),
        true,
        options,
        Some(&priority),
    )
    .unwrap();
    assert_eq!(report.census_classes, Some(expected.classes));
    assert_eq!(lane.terminals(), expected.terminals);

    let top = key([1, 1, 1, 1, 1, 1, 1, 1, 0, 0]);
    let dotted = key([2, 1, 1, 1, 1, 1, 1, 1, 0, 0]);
    let numerator = key([1, 1, 1, 1, 1, 1, 1, 1, -1, -1]);
    let ids = lane
        .add_targets(&[top.clone(), dotted.clone(), numerator.clone()])
        .unwrap();
    assert_eq!(lane.statistics(&ids).uncovered, 0);
    let compression = lane.compress(&[]).unwrap();
    assert_eq!(compression.terminals, expected.terminals);
    assert_eq!(compression.masters.len(), expected.masters);
    assert_eq!(lane.masters(), compression.masters);
    assert_eq!(compression.classes.len(), expected.classes);
    assert!(
        compression.classes.iter().all(|class| class.masters > 0),
        "every census class carries a master"
    );
    assert_eq!(
        compression.classes.iter().map(|c| c.masters).sum::<usize>(),
        expected.masters
    );
    let top_masters = compression
        .masters
        .iter()
        .filter(|master| sector_id(master) == TOP_SECTOR)
        .count();
    assert_eq!(top_masters, expected.top_sector_masters);
    assert!(compression.masters.contains(&top), "T is a master");

    if deformed.is_none() {
        return;
    }
    // Two independent reconstructions of the same non-trivial coefficients.
    let d = rational(5399, 1350);
    let eta = rational(5, 13);
    let exact = lane
        .reduce_exact_at_point(&[d.clone(), eta.clone()], &ids)
        .unwrap();
    let univariate = lane.reduce_univariate(&[Some(d), None], &ids).unwrap();
    assert_eq!(univariate.free_parameter, 1);
    // 2^61 - 1 is not among the reconstruction primes (which start below 2^62).
    let field = PrimeField::new(DEFAULT_PRIME).unwrap();
    let x = field.from_rational(&eta).unwrap();
    let mut nonzero = 0;
    for (row, (at_point, in_eta)) in exact
        .coefficients
        .iter()
        .zip(&univariate.coefficients)
        .enumerate()
    {
        assert_eq!(at_point.len(), expected.masters);
        assert_eq!(in_eta.len(), expected.masters);
        for (value, function) in at_point.iter().zip(in_eta) {
            let image = field.from_rational(value);
            assert!(image.is_some());
            assert_eq!(
                image,
                function.evaluate(x, &field),
                "target {row}: exact point and Q(eta) reconstructions differ"
            );
            nonzero += usize::from(!value.is_zero());
        }
    }
    // T reduces to itself; the dotted and numerator targets are non-trivial.
    assert!(nonzero > 4, "only {nonzero} nonzero coefficients");
}

#[test]
#[ignore = "slow: in-process P14 candidate generation (run with --ignored, release)"]
fn rim_p14_compresses_216_terminals_to_42_masters() {
    check(
        Some(0),
        Expected {
            terminals: 216,
            masters: 42,
            classes: 27,
            top_sector_masters: 3,
        },
    );
}

#[test]
#[ignore = "slow: in-process P14 candidate generation (run with --ignored, release)"]
fn spoke_p14_compresses_246_terminals_to_42_masters() {
    check(
        Some(1),
        Expected {
            terminals: 246,
            masters: 42,
            classes: 26,
            top_sector_masters: 4,
        },
    );
}

#[test]
#[ignore = "slow: in-process P14 candidate generation (run with --ignored, release)"]
fn undeformed_p14_compresses_179_terminals_to_14_masters() {
    check(
        None,
        Expected {
            terminals: 179,
            masters: 14,
            classes: 11,
            top_sector_masters: 2,
        },
    );
}
