//! Sector-box seeding on RustFlow's native four-loop wheel (P14): demand
//! and staircase seeding with growth, Kira 3 truncation, the stability
//! check, numerator tubes and the master check.
//!
//! The family is the exact native presentation of `sector_symmetry_census.rs`:
//! physical lines `D0..D7`, ISPs `D8 = k0.k1` and `D9 = k0.k2`, and an optional
//! deformation `D_s -> D_s - eta` with symbolic `eta`. Every parameter is
//! sampled modulo `2^61 - 1`, so the modular structure is the generic one.
//! Four loops: every test is ignored in default CI (minutes in debug, seconds
//! in release); the three-loop K6 tests of `sector_box_k6_seeding.rs` cover
//! the same options in CI. Run with
//! `cargo test --release -p rustred --test sector_box_seeding -- --ignored`.

use std::sync::Arc;

use rustred::algebra::{CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::sector_box::{
    BoxLaportaSession, BoxOptions, CensusRouter, ColumnOrder, IdentityRouter, SectorRouter,
    Seeding, TubeOptions, census_targets,
};

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
const N: usize = 10;
const TOP: [i16; N] = [1, 1, 1, 1, 1, 1, 1, 1, 0, 0];

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

fn sector_mask(id: u32) -> [bool; N] {
    std::array::from_fn(|slot| id >> slot & 1 == 1)
}

fn session(
    family: &Arc<IntegralFamily>,
    options: BoxOptions,
) -> (BoxLaportaSession<N>, Arc<IdentityRouter>) {
    let router = IdentityRouter::shared(family).unwrap();
    let session = BoxLaportaSession::<N>::new(
        family.clone(),
        &CutConstraint::none(N).unwrap(),
        router.clone(),
        options,
    )
    .unwrap();
    (session, router)
}

/// CI-sized options: no LP gate (the census test enables it).
fn small_options(seeding: Seeding) -> BoxOptions {
    BoxOptions {
        seeding,
        stability: Some((1, 0, 0)),
        workers: 2,
        lp_gate: false,
        ..Default::default()
    }
}

fn sector_id(values: &[i16; N]) -> u32 {
    values
        .iter()
        .enumerate()
        .filter(|(_, value)| **value > 0)
        .map(|(slot, _)| 1u32 << slot)
        .sum()
}

fn census_router(family: &Arc<IntegralFamily>) -> Arc<dyn SectorRouter<N>> {
    Arc::new(CensusRouter::discover(family.clone(), &sector_mask(255)).unwrap())
}

fn census_session(
    family: &Arc<IntegralFamily>,
    router: &Arc<dyn SectorRouter<N>>,
    options: BoxOptions,
) -> BoxLaportaSession<N> {
    BoxLaportaSession::<N>::new(
        family.clone(),
        &CutConstraint::none(N).unwrap(),
        router.clone(),
        options,
    )
    .unwrap()
}

/// A box without margin puts the target on its boundary: the sector grows
/// once, after which the master set is final.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn boundary_master_triggers_one_growth_step() {
    let family = p14(None);
    let (mut session, _) = session(
        &family,
        BoxOptions {
            stability: None,
            ..small_options(Seeding::Demand {
                extra_r: 0,
                extra_s: 0,
                max_growth: 2,
            })
        },
    );
    // The corner of the six-line sector 207 (slots 0,1,2,3,6,7).
    let corner = [1, 1, 1, 1, 0, 0, 1, 1, 0, 0];
    let report = session.extend(&[corner], &[]).unwrap();
    assert_eq!(report.masters, vec![corner]);
    assert_eq!(report.rounds, 2);
    assert_eq!(report.growth, vec![vec![207]]);
    let sector = report
        .sectors
        .iter()
        .find(|sector| sector.id == 207)
        .unwrap();
    assert_eq!(sector.seed_box.map(|seed_box| seed_box.r), Some(7));
    assert_eq!(sector.growth, 1);
}

/// Census of the down-closure of one six-line sector: per-sector masters
/// equal the Lee-Pomeransky counts wherever there is a verdict.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn six_line_down_closure_census_matches_lee_pomeransky() {
    let family = p14(None);
    // Sector 207 = slots {0,1,2,3,6,7}; Kira: one unique master there.
    let top = sector_mask(207);
    let (mut session, router) = session(
        &family,
        BoxOptions {
            lp_gate: true,
            ..small_options(Seeding::Demand {
                extra_r: 1,
                extra_s: 0,
                max_growth: 2,
            })
        },
    );
    let targets = census_targets::<N>(&*router, N, &top, 1, 0);
    let report = session.extend(&targets, &[]).unwrap();
    let mut mismatches = Vec::new();
    for sector in &report.sectors {
        if let Some(lp) = sector.lp
            && sector.masters != lp
        {
            mismatches.push((sector.id, sector.masters, lp));
        }
    }
    assert!(mismatches.is_empty(), "sector, masters, LP: {mismatches:?}");
    assert!(report.monitor_violations.is_empty());
    assert_eq!(
        report.masters_first_sample, report.masters_second_sample,
        "the two samples disagree"
    );
}

/// Staircase seeding unites per-target boxes: fewer seeds than the hull,
/// the same masters as the hull box.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn staircase_unites_target_boxes_and_keeps_the_masters() {
    let family = p14(None);
    // Sector 207 (slots 0,1,2,3,6,7): a rank-2 integral without dots and a
    // two-dot integral without numerators.
    let numerator = [1, 1, 1, 1, 0, 0, 1, 1, -1, -1];
    let dotted = [2, 1, 2, 1, 0, 0, 1, 1, 0, 0];
    let run = |seeding| {
        let (mut session, _) = session(
            &family,
            BoxOptions {
                stability: None,
                lp_gate: true,
                order: ColumnOrder::LinesRankDots,
                ..small_options(seeding)
            },
        );
        session.extend(&[numerator, dotted], &[]).unwrap()
    };
    let staircase = run(Seeding::Staircase {
        s_pad: 1,
        d_pad: 1,
        max_growth: 2,
    });
    let hull = run(Seeding::KiraBox { r: 9, s: 3, d: 3 });
    assert_eq!(staircase.masters, hull.masters);
    let top = staircase
        .sectors
        .iter()
        .find(|sector| sector.id == 207)
        .unwrap();
    assert_eq!(top.seed_region.len(), 2, "{:?}", top.seed_region);
    assert!(top.seeds < top.hull_seeds);
    assert!(staircase.seeds < staircase.hull_seeds);
    assert!(staircase.seeds < hull.seeds);
}

/// Kira 3 `truncate_sp`: subsectors without demand of their own inherit at
/// most `max(1, t - l + 1)` numerators; the masters do not change.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn truncation_caps_inherited_numerators_without_changing_masters() {
    let family = p14(Some(0));
    let targets = [TOP, [2, 1, 1, 1, 1, 1, 1, 1, 0, 0]];
    let run = |truncate| {
        let (mut session, _) = session(
            &family,
            BoxOptions {
                truncate,
                stability: None,
                ..small_options(Seeding::KiraBox { r: 9, s: 2, d: 1 })
            },
        );
        session.extend(&targets, &[]).unwrap()
    };
    let full = run(None);
    // l = t_top - s_target = 8 and one less.
    for l in [8, 7] {
        let truncated = run(Some(l));
        assert_eq!(truncated.masters, full.masters, "l = {l}");
        assert!(truncated.seeds < full.seeds, "l = {l}");
        for sector in &truncated.sectors {
            let lines = sector.lines as i16;
            let cap = if sector.id == 255 {
                2
            } else {
                1.max(lines - l + 1).min(2)
            };
            assert_eq!(
                sector.seed_box.map(|b| b.s),
                Some(cap),
                "sector {}",
                sector.id
            );
        }
        assert_eq!(truncated.truncate, Some(l));
    }
}

/// Under truncation the stability check reruns the plan with enlarged
/// explicit boxes: subsectors keep the Eq. 9 cap, the pre-flight predicts
/// the stability seeds exactly, and the masters are stable.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn truncated_stability_keeps_the_cap() {
    let family = p14(Some(0));
    let targets = [TOP, [2, 1, 1, 1, 1, 1, 1, 1, 0, 0]];
    let (mut session, _) = session(
        &family,
        BoxOptions {
            truncate: Some(8),
            stability: Some((0, 1, 0)),
            ..small_options(Seeding::KiraBox { r: 9, s: 2, d: 1 })
        },
    );
    let report = session.extend(&targets, &[]).unwrap();
    assert_eq!(report.rounds, 1);
    assert_eq!(report.stable, Some(true));
    // The stability round adds the seeds of the enlarged explicit box.
    let (mut unchecked, _) = crate::session(
        &family,
        BoxOptions {
            truncate: Some(8),
            stability: None,
            ..small_options(Seeding::KiraBox { r: 9, s: 2, d: 1 })
        },
    );
    unchecked.extend(&targets, &[]).unwrap();
    assert!(session.stats().seeds > unchecked.stats().seeds);
    for sector in &report.sectors {
        // The explicit top box grows to s = 3; inherited boxes stay at 1.
        let cap = if sector.id == 255 { 3 } else { 1 };
        assert_eq!(
            sector.seed_box.map(|b| b.s),
            Some(cap),
            "sector {}",
            sector.id
        );
    }
}

/// Numerator tubes (ENGINE-PLAN M7): a rank-3 target of sector 207 (ISPs
/// 4, 5, 8, 9) seeded with the tube `u * y`, `u | N(target)`, `deg y <= 1`,
/// instead of the full `s <= 3` box gives the same masters with fewer
/// seeds; subsectors inherit the target's box through the Eq. 9 cap with
/// `l = t_top - s_target`; the pre-flight counts the tube and stability
/// seeds exactly, and the replanned stability check (halo and own boxes
/// + 1) reproduces the masters.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn tubes_replace_the_rank_box_and_keep_the_masters() {
    let family = p14(None);
    let router = census_router(&family);
    let targets = [
        [1, 1, 1, 1, -1, 0, 1, 1, -1, -1],
        [1, 1, 1, 1, 0, 0, 1, 1, 0, 0],
    ];
    let options = |tubes, stability| BoxOptions {
        tubes,
        stability,
        truncate: Some(6 - 3),
        ..small_options(Seeding::Staircase {
            s_pad: 0,
            d_pad: 1,
            max_growth: 0,
        })
    };
    let mut boxed_session = census_session(&family, &router, options(None, None));
    let boxed = boxed_session.extend(&targets, &[]).unwrap();
    let tubes = TubeOptions {
        min_rank: 2,
        halo: 1,
        subsector_halo: None,
    };
    // Without the stability check: fewer seeds than the full rank box.
    let mut plain_session = census_session(&family, &router, options(Some(tubes), None));
    let plain = plain_session.extend(&targets, &[]).unwrap();
    assert_eq!(plain.masters, boxed.masters);
    let tube_seeds: usize = plain.sectors.iter().map(|sector| sector.tube_seeds).sum();
    assert!(
        tube_seeds > 0 && plain.seeds < boxed.seeds,
        "{} vs {}",
        plain.seeds,
        boxed.seeds
    );
    let mut session = census_session(&family, &router, options(Some(tubes), Some((0, 1, 0))));
    let tubed = session.extend(&targets, &[]).unwrap();
    assert_eq!(tubed.masters_before_stability, boxed.masters);
    assert_eq!(tubed.stable, Some(true));
    assert_eq!(tubed.rounds, 1);
    let top = tubed
        .sectors
        .iter()
        .find(|sector| sector.id == 207)
        .unwrap();
    assert!(top.tube_seeds > 0, "{top:?}");
    // The tube target's own box stays at s = min_rank - 1 (+1 stability).
    assert_eq!(top.seed_box.map(|b| b.s), Some(2), "{top:?}");
}

/// Sub-tubes (`TubeOptions::subsector_halo`): the subsectors of the rank-3
/// target's sector 207 get the target's tube over their own ISPs (pinched
/// lines included), routed onto their representatives. The masters equal
/// the boxed run's, the pre-flight still counts the seeds `extend`
/// generates, the stability check passes, and tube seeds now also appear
/// below the target's sector.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn subsector_tubes_seed_subsectors_and_keep_the_masters() {
    let family = p14(None);
    let router = census_router(&family);
    let targets = [
        [1, 1, 1, 1, -1, 0, 1, 1, -1, -1],
        [1, 1, 1, 1, 0, 0, 1, 1, 0, 0],
    ];
    let options = |tubes, stability| BoxOptions {
        tubes,
        stability,
        truncate: Some(6 - 3),
        ..small_options(Seeding::Staircase {
            s_pad: 0,
            d_pad: 1,
            max_growth: 0,
        })
    };
    let mut boxed_session = census_session(&family, &router, options(None, None));
    let boxed = boxed_session.extend(&targets, &[]).unwrap();
    let plain = TubeOptions {
        min_rank: 2,
        halo: 1,
        subsector_halo: None,
    };
    let sub = TubeOptions {
        subsector_halo: Some(1),
        ..plain
    };
    let tube_seeds = |options| {
        let mut session = census_session(&family, &router, options);
        let report = session.extend(&targets, &[]).unwrap();
        assert_eq!(report.masters, boxed.masters);
        report
            .sectors
            .iter()
            .map(|sector| sector.tube_seeds)
            .sum::<usize>()
    };
    let (plain_tubes, sub_tubes) = (
        tube_seeds(options(Some(plain), None)),
        tube_seeds(options(Some(sub), None)),
    );
    assert!(
        sub_tubes > plain_tubes,
        "sub-tubes add tube seeds: {sub_tubes} vs {plain_tubes}"
    );
    let mut session = census_session(&family, &router, options(Some(sub), Some((0, 1, 0))));
    let tubed = session.extend(&targets, &[]).unwrap();
    assert_eq!(tubed.masters_before_stability, boxed.masters);
    assert_eq!(tubed.stable, Some(true));
    assert!(
        tubed
            .sectors
            .iter()
            .any(|sector| sector.id != 207 && sector.tube_seeds > 0),
        "no subsector got tube seeds"
    );
}

/// The master check: a complete preferred set gives no extra masters, an
/// unneeded preferred integral and wrong expected counts are reported.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn master_check_reports_extra_unneeded_missing_and_excess() {
    let family = p14(None);
    let router = census_router(&family);
    let targets = census_targets::<N>(&*router, N, &sector_mask(255), 1, 0);
    let options = || BoxOptions {
        stability: None,
        ..small_options(Seeding::KiraBox { r: 9, s: 0, d: 2 })
    };
    let mut first = census_session(&family, &router, options());
    let baseline = first.extend(&targets, &[]).unwrap();
    assert_eq!(baseline.masters.len(), 14);
    assert!(baseline.master_check.extra.is_empty(), "no preferred set");
    // Preferred: the 14 masters plus a reducible one-dot integral of the
    // representative 207 (inside the box; harder than its corner master).
    let reducible = [2, 1, 1, 1, 0, 0, 1, 1, 0, 0];
    assert!(baseline.masters.contains(&[1, 1, 1, 1, 0, 0, 1, 1, 0, 0]));
    let mut counts = std::collections::BTreeMap::new();
    for master in &baseline.masters {
        *counts.entry(u64::from(sector_id(master))).or_insert(0usize) += 1;
    }
    let (&wrong_class, &right) = counts.iter().next().unwrap();
    counts.insert(wrong_class, right + 1);
    let mut preferred = baseline.masters.clone();
    preferred.push(reducible);
    let mut second = census_session(
        &family,
        &router,
        BoxOptions {
            expected_counts: Some(Arc::new(counts)),
            ..options()
        },
    );
    let report = second.extend(&targets, &preferred).unwrap();
    assert_eq!(report.masters.len(), 14);
    let check = &report.master_check;
    assert!(check.extra.is_empty(), "{:?}", check.extra);
    assert_eq!(check.unneeded, vec![reducible]);
    assert_eq!(check.missing, vec![(wrong_class as u32, right + 1, right)]);
    assert!(check.excess.is_empty());
    assert!(!check.passed());
    // Strict mode turns the mismatch into an error.
    let mut strict = census_session(
        &family,
        &router,
        BoxOptions {
            master_check: rustred::solver::sector_box::MasterCheckMode::Strict,
            expected_counts: Some(Arc::new([(wrong_class, right + 1)].into_iter().collect())),
            ..options()
        },
    );
    assert!(matches!(
        strict.extend(&targets, &[]),
        Err(rustred::solver::SolverError::MasterCheck(_))
    ));
}

/// A preferred master given in a non-representative sector is routed onto
/// its single image in the representative sector.
#[test]
#[ignore = "four loops (P14), minutes in debug: cargo test --release -p rustred --test sector_box_seeding -- --ignored"]
fn preferred_masters_are_routed_to_representatives() {
    let family = p14(None);
    let router = census_router(&family);
    // Sector 47 is routed to 31 (both five-line sunsets of the wheel).
    assert_eq!(
        router.representative(&sector_mask(47)).map(|rep| {
            rep.iter()
                .enumerate()
                .filter(|(_, a)| **a)
                .map(|(s, _)| 1u32 << s)
                .sum::<u32>()
        }),
        Some(31)
    );
    let dotted_47 = [2, 1, 1, 1, 0, 1, 0, 0, 0, 0];
    let mut session = census_session(
        &family,
        &router,
        BoxOptions {
            stability: None,
            ..small_options(Seeding::KiraBox { r: 7, s: 0, d: 2 })
        },
    );
    let report = session
        .extend(&[[1, 1, 1, 1, 1, 0, 0, 0, 0, 0]], &[dotted_47])
        .unwrap();
    let preferred: Vec<_> = report
        .masters
        .iter()
        .filter(|master| sector_id(master) == 31 && master.iter().any(|&v| v > 1))
        .collect();
    assert_eq!(preferred.len(), 1, "{:?}", report.masters);
    assert!(report.master_check.unneeded.is_empty());
}
