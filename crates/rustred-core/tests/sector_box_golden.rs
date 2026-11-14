//! Golden eta-DE reproduction against a directory of campaign solution
//! files (`rustred-harness-solution.v1`), for families too large for
//! default CI. Ignored; run with
//!
//! ```text
//! SB_GOLDEN_FAMILY=family.json SB_GOLDEN_DEMAND=demand.json SB_GOLDEN_DIR=de/ \
//!   cargo test --release -p rustred --test sector_box_golden -- --ignored --nocapture
//! ```
//!
//! - `SB_GOLDEN_FAMILY`: the family as campaign JSON (`name`,
//!   `loop_momenta`, `denominators[] = {constant, coefficients}` over the
//!   loop-loop scalar products, constants `<rational>`, `<rational>-eta` or
//!   `<rational>+eta`; `power_shifts`), parameters `d` and `eta`;
//! - `SB_GOLDEN_DEMAND`: `targets`, `preferred`, `slot_top`;
//! - `SB_GOLDEN_DIR`: every `solution-*.json` in it is one value of `d`;
//! - `SB_GOLDEN_WORKERS` (default 2).
//!
//! The session is the campaign eta-DE session (census routing of
//! `slot_top`, staircase seeding without padding, no LP gate, the
//! eta-derivative closure on slot 0). Every file must be reproduced exactly
//! (masters, rules, fixed value, every reconstruction and verification
//! prime), must be readable with this session's family block, and must pass
//! a probabilistic certificate at a prime independent of its
//! reconstruction. The storage arity is `N = 10`: the 4-loop prism family
//! of the campaign (`v9a/pr`, 53 masters, 68 values of `d`).

use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::sector_box::{
    BoxLaportaSession, BoxOptions, BoxSolution, CensusRouter, ColumnOrder,
    DEFAULT_CERTIFICATE_PRIME, FfOptions, MasterCheckMode, SectorRouter, Seeding, SlotPriority,
};
use serde_json::Value;
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

const N: usize = 10;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} (see the module documentation)"))
}

fn read(path: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn rational(text: &str) -> Rational {
    let (n, d) = text.trim().split_once('/').unwrap_or((text.trim(), "1"));
    Rational::from((
        n.trim().parse::<Integer>().unwrap(),
        d.trim().parse::<Integer>().unwrap(),
    ))
}

fn constant(context: &CoefficientContext, text: &str) -> Coefficient {
    let limits = ExactAlgebraLimits::default();
    let value = |text: &str| {
        let r = rational(text);
        let n = context.integer(r.numerator().to_string().parse::<i64>().unwrap());
        let d = context.integer(r.denominator().to_string().parse::<i64>().unwrap());
        context.try_div(&n, &d, limits).unwrap()
    };
    let eta = context.parameter("eta").unwrap();
    if let Some(base) = text.strip_suffix("-eta") {
        context.try_sub(&value(base), &eta, limits).unwrap()
    } else if let Some(base) = text.strip_suffix("+eta") {
        context.try_add(&value(base), &eta, limits).unwrap()
    } else {
        value(text)
    }
}

fn family(data: &Value) -> Arc<IntegralFamily> {
    let context = CoefficientContext::try_new(["d", "eta"]).unwrap();
    let strings = |key: &str| -> Vec<String> {
        data[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    };
    let denominators = data["denominators"]
        .as_array()
        .unwrap()
        .iter()
        .map(|den| {
            let row = den["coefficients"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| constant(&context, v.as_str().unwrap()))
                .collect();
            AffineDenominator::new(constant(&context, den["constant"].as_str().unwrap()), row)
        })
        .collect::<Vec<_>>();
    let shifts = strings("power_shifts")
        .iter()
        .map(|v| constant(&context, v))
        .collect();
    Arc::new(
        IntegralFamily::new(
            data["name"].as_str().unwrap().to_string(),
            strings("loop_momenta"),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            shifts,
        )
        .unwrap(),
    )
}

fn integral(value: &Value) -> [i16; N] {
    let list = value.as_array().unwrap();
    assert_eq!(list.len(), N, "the family must have {N} denominators");
    std::array::from_fn(|slot| list[slot].as_i64().unwrap() as i16)
}

#[test]
#[ignore = "4-loop golden (68 values of d, minutes in release); needs SB_GOLDEN_*"]
fn golden_campaign_eta_de() {
    let family = family(&read(&env("SB_GOLDEN_FAMILY")));
    let demand = read(&env("SB_GOLDEN_DEMAND"));
    let workers = std::env::var("SB_GOLDEN_WORKERS").map_or(2, |v| v.parse().unwrap());
    let top = demand["slot_top"].as_u64().unwrap();
    let mask: [bool; N] = std::array::from_fn(|slot| top >> slot & 1 == 1);
    let router = Arc::new(CensusRouter::discover(family.clone(), &mask).unwrap());
    let targets: Vec<[i16; N]> = demand["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(integral)
        .collect();
    // Preferred integrals that route onto one integral (the campaign rule).
    let preferred: Vec<[i16; N]> = demand["preferred"]
        .as_array()
        .map_or_else(Vec::new, |list| list.iter().map(integral).collect())
        .into_iter()
        .filter(|p| {
            SectorRouter::<N>::route(router.as_ref(), p).is_ok_and(|route| route.len() <= 1)
        })
        .collect();
    let options = BoxOptions {
        seeding: Seeding::Staircase {
            s_pad: 0,
            d_pad: 0,
            max_growth: 0,
        },
        order: ColumnOrder::LinesRankDots,
        stability: None,
        workers,
        lp_gate: false,
        slot_priority: SlotPriority::TopSectorFirst(top),
        master_check: MasterCheckMode::Report,
        truncate: demand["truncate"].as_i64().map(|v| v as i16),
        ..Default::default()
    };
    let mut session =
        BoxLaportaSession::<N>::new(family, &CutConstraint::none(N).unwrap(), router, options)
            .unwrap();
    let mut targets = targets;
    for _round in 0..=8 {
        let report = session.extend(&targets, &preferred).unwrap();
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
            break;
        }
    }
    let harness = session.harness_family();
    let dir = env("SB_GOLDEN_DIR");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("solution-")
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no solution-*.json in {dir}");
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let golden: Value = serde_json::from_str(&text).unwrap();
        let d0 = rational(golden["fixed"]["d"].as_str().unwrap());
        let ff = FfOptions {
            max_primes: 64,
            ..Default::default()
        };
        let solution = session
            .materialize_ff(&targets, &[("d".into(), d0)], &ff)
            .unwrap();
        let (expected, _) = BoxSolution::<N>::from_harness_json(&text, &harness).unwrap();
        assert_eq!(solution.masters, expected.masters, "{path:?}");
        assert_eq!(solution.targets, expected.targets, "{path:?}");
        assert_eq!(solution.coefficients, expected.coefficients, "{path:?}");
        assert_eq!(solution.fixed, expected.fixed, "{path:?}");
        assert_eq!(
            solution.reconstruction_primes(),
            expected.reconstruction_primes(),
            "{path:?}"
        );
        assert_eq!(
            solution.verification_primes(),
            expected.verification_primes(),
            "{path:?}"
        );
        session
            .certify(&expected, Some(DEFAULT_CERTIFICATE_PRIME), 0x6332_7432)
            .unwrap();
        eprintln!(
            "{}: identical, certified at {DEFAULT_CERTIFICATE_PRIME}",
            path.display()
        );
    }
    eprintln!("{} files reproduced", files.len());
}
