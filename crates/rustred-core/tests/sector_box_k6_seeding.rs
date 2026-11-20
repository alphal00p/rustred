//! Sector-box seeding on the three-loop K6 vacuum family with the mass of
//! line 0 deformed (`D0 = k1^2 - 1 - eta`), the smallest family of the
//! campaign's eta differential equations.
//!
//! Staircase seeding with the derivative closure (demand the master with
//! its line-0 power raised by one, until closed) gives the campaign's 12
//! masters; the strict master check passes with the per-class counts and
//! fails with a wrong one; `settle_on_expected` stops growth as soon as every
//! class has its expected count; demand seeding grows to the same masters;
//! the +1 stability check confirms them. Seconds in CI.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustred::algebra::{CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::SolverError;
use rustred::solver::sector_box::{
    BoxLaportaSession, BoxOptions, CensusRouter, ColumnOrder, MasterCheckMode, ModularReport,
    SectorRouter, Seeding, SlotPriority,
};

const N: usize = 6;
const TOP: [i16; N] = [1, 1, 1, 1, 1, 1];

/// Coefficients over `(k1k1, k1k2, k1k3, k2k2, k2k3, k3k3)`.
const SCALAR_PRODUCTS: [[i64; 6]; 6] = [
    [1, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 0],
    [0, 0, 0, 0, 0, 1],
    [1, 0, -2, 0, 0, 1],
    [1, -2, 0, 1, 0, 0],
    [0, 0, 0, 1, -2, 1],
];

/// The campaign's masters (column order, hardest first).
const MASTERS: [[i16; N]; 12] = [
    [1, 1, 1, 1, 1, 0],
    [1, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 1, 1],
    [1, 1, 0, 1, 0, 2],
    [1, 1, 1, 1, 0, 0],
    [1, 1, 1, 0, 0, 1],
    [1, 1, 0, 1, 0, 1],
    [0, 1, 1, 1, 1, 0],
    [0, 1, 1, 1, 0, 1],
    [1, 1, 1, 0, 0, 0],
    [0, 1, 1, 1, 0, 0],
    [1, 1, 1, 1, 1, 1],
];

fn k6() -> Arc<IntegralFamily> {
    let context = CoefficientContext::try_new(["d", "eta"]).unwrap();
    let denominators = SCALAR_PRODUCTS
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
            AffineDenominator::new(constant, row.map(|value| context.integer(value)).to_vec())
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            "k6_eta_slot0",
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

fn options(seeding: Seeding) -> BoxOptions {
    BoxOptions {
        seeding,
        order: ColumnOrder::LinesRankDots,
        slot_priority: SlotPriority::TopSectorFirst(63),
        stability: None,
        workers: 2,
        lp_gate: false,
        ..Default::default()
    }
}

fn session(options: BoxOptions) -> Result<BoxLaportaSession<N>, SolverError> {
    let family = k6();
    let router: Arc<dyn SectorRouter<N>> =
        Arc::new(CensusRouter::discover(family.clone(), &[true; N]).unwrap());
    BoxLaportaSession::<N>::new(family, &CutConstraint::none(N).unwrap(), router, options)
}

/// `extend` until the demand is closed under `I[a] -> I[a + e_0]` for
/// masters with `a_0 > 0`; returns every round's report.
fn closed(
    session: &mut BoxLaportaSession<N>,
) -> Result<(Vec<ModularReport<N>>, Vec<[i16; N]>), SolverError> {
    let mut targets = vec![TOP];
    let mut reports = Vec::new();
    loop {
        let report = session.extend(&targets, &[TOP])?;
        let known: BTreeSet<[i16; N]> = targets.iter().copied().collect();
        let added: Vec<[i16; N]> = report
            .masters
            .iter()
            .filter(|master| master[0] > 0)
            .map(|master| {
                let mut seed = *master;
                seed[0] += 1;
                seed
            })
            .filter(|seed| !known.contains(seed))
            .collect();
        reports.push(report);
        if added.is_empty() {
            return Ok((reports, targets));
        }
        assert!(reports.len() < 8, "closure does not converge");
        targets.extend(added);
    }
}

fn staircase(max_growth: u8) -> Seeding {
    Seeding::Staircase {
        s_pad: 0,
        d_pad: 0,
        max_growth,
    }
}

fn per_class(report: &ModularReport<N>) -> BTreeMap<u64, usize> {
    report
        .master_check
        .per_class
        .iter()
        .filter(|(_, masters, _)| *masters > 0)
        .map(|&(class, masters, _)| (u64::from(class), masters))
        .collect()
}

/// `grow_all_levels` (opt-in, with rebuild_rounds): all flagged levels grow in
/// one round; the closed master set is the default one, in no more rounds.
#[test]
fn rebuild_rounds_grow_all_levels_with_the_same_masters() {
    let mut incremental = session(options(staircase(2))).unwrap();
    let (a, _) = closed(&mut incremental).unwrap();
    let mut rebuilt = session(BoxOptions {
        rebuild_rounds: true,
        grow_all_levels: true,
        ..options(staircase(2))
    })
    .unwrap();
    let (b, _) = closed(&mut rebuilt).unwrap();
    assert_eq!(a.last().unwrap().masters, b.last().unwrap().masters);
    let rounds =
        |reports: &[ModularReport<N>]| reports.iter().map(|report| report.rounds).sum::<usize>();
    assert!(
        rounds(&b) <= rounds(&a),
        "rebuild {} rounds vs incremental {}",
        rounds(&b),
        rounds(&a)
    );
}

/// `growth_no_inherit` (with rebuild_rounds): growth stays in the flagged
/// sectors' own boxes and cascades down only where masters sit at a sector's
/// own dot limit; the closed master set is the default one.
#[test]
fn growth_without_inheritance_closes_on_the_same_masters() {
    let mut default = session(options(staircase(2))).unwrap();
    let (a, _) = closed(&mut default).unwrap();
    let mut own = session(BoxOptions {
        rebuild_rounds: true,
        growth_no_inherit: true,
        ..options(staircase(2))
    })
    .unwrap();
    let (b, _) = closed(&mut own).unwrap();
    assert_eq!(a.last().unwrap().masters, b.last().unwrap().masters);
}

#[test]
fn staircase_closure_gives_the_campaign_masters() {
    let mut session = session(options(staircase(0))).unwrap();
    let (reports, targets) = closed(&mut session).unwrap();
    let last = reports.last().unwrap();
    assert_eq!(last.masters, MASTERS.to_vec());
    // TOP and its derivative seeds: three rounds add 1, 6 and 1 seeds.
    assert_eq!(targets.len(), 9);
    assert_eq!(reports.len(), 4);
    // Without growth every round has exactly one seeding round.
    assert!(
        reports
            .iter()
            .all(|report| report.rounds == 1 && report.growth.is_empty())
    );
    // No preferred set beyond TOP: the other masters are "extra", and the
    // report-only check does not fail.
    assert_eq!(last.master_check.extra.len(), 11);
    assert!(last.master_check.missing.is_empty() && last.master_check.excess.is_empty());
    assert_eq!(per_class(last).values().sum::<usize>(), 12);
}

#[test]
fn strict_master_check_passes_with_the_counts_and_fails_with_a_wrong_one() {
    let reference = {
        let mut session = session(options(staircase(0))).unwrap();
        per_class(closed(&mut session).unwrap().0.last().unwrap())
    };
    let strict = |counts: BTreeMap<u64, usize>| BoxOptions {
        master_check: MasterCheckMode::Strict,
        expected_counts: Some(Arc::new(counts)),
        ..options(staircase(0))
    };
    // Strict fails on extra masters outside a nonempty preferred set; with
    // the full preferred list and the right counts it passes.
    let mut session = session(strict(reference.clone())).unwrap();
    let targets = closed_targets();
    let report = session.extend(&targets, &MASTERS).unwrap();
    assert!(report.master_check.passed(), "{:?}", report.master_check);
    // Preferred masters move to the easy end of the column order: the same
    // set, possibly in another order.
    let set = |masters: &[[i16; N]]| masters.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(set(&report.masters), set(&MASTERS));
    // One class with a wrong expected count: a typed error, not a panic.
    let mut wrong = reference.clone();
    *wrong.get_mut(&63).unwrap() += 1;
    let mut session = self::session(strict(wrong)).unwrap();
    match session.extend(&targets, &MASTERS) {
        Err(SolverError::MasterCheck(message)) => assert!(message.contains("below expected")),
        other => panic!("expected a master-check error, got {other:?}"),
    }
}

/// The closed demand of the campaign: TOP and the line-0 derivative seeds of
/// the masters with a line-0 line.
fn closed_targets() -> Vec<[i16; N]> {
    let mut targets = vec![TOP];
    targets.extend(MASTERS.iter().filter(|m| m[0] > 0).map(|m| {
        let mut seed = *m;
        seed[0] += 1;
        seed
    }));
    targets
}

/// With growth allowed, `settle_on_expected` stops after the first seeding
/// round once every class has its expected count. Without it the top
/// integral, a master on the dot boundary of its own box, grows its class
/// (same master, more rows).
#[test]
fn settle_on_expected_stops_growth() {
    let run = |settle: bool| {
        let mut session = session(BoxOptions {
            expected_counts: Some(Arc::new([(63, 1)].into_iter().collect())),
            settle_on_expected: settle,
            ..options(staircase(2))
        })
        .unwrap();
        let report = session.extend(&[TOP], &[TOP]).unwrap();
        (report, session.stats().rows)
    };
    let (settled, settled_rows) = run(true);
    let (grown, grown_rows) = run(false);
    assert_eq!(settled.rounds, 1, "{:?}", settled.growth);
    assert!(settled.master_check.passed(), "{:?}", settled.master_check);
    assert!(grown.rounds > 1, "{:?}", grown.growth);
    assert_eq!(settled.masters, vec![TOP]);
    assert_eq!(grown.masters, vec![TOP]);
    assert!(settled_rows < grown_rows);
}

/// Demand seeding with growth (no margin to start with) reaches the same
/// 12 masters as staircase seeding.
#[test]
fn demand_seeding_grows_to_the_campaign_masters() {
    let mut session = session(options(Seeding::Demand {
        extra_r: 0,
        extra_s: 0,
        max_growth: 3,
    }))
    .unwrap();
    let report = session.extend(&closed_targets(), &[TOP]).unwrap();
    let set = |masters: &[[i16; N]]| masters.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(set(&report.masters), set(&MASTERS));
}

/// The +1 stability check enlarges every seeded region and finds the same
/// masters.
#[test]
fn stability_check_confirms_the_masters() {
    let mut session = session(BoxOptions {
        stability: Some((0, 0, 1)),
        ..options(staircase(0))
    })
    .unwrap();
    let report = session.extend(&closed_targets(), &[TOP]).unwrap();
    assert_eq!(report.stable, Some(true));
    assert_eq!(report.masters_before_stability, report.masters);
    assert_eq!(report.masters, MASTERS.to_vec());
}

/// Invalid option combinations are refused when the session is built.
#[test]
fn invalid_seeding_options_are_input_errors() {
    use rustred::solver::sector_box::{Insertion, TubeOptions};
    let tubes = Some(TubeOptions {
        min_rank: 1,
        halo: 0,
        subsector_halo: None,
    });
    for options in [
        // tubes need staircase seeding
        BoxOptions {
            tubes,
            ..options(Seeding::KiraBox { r: 8, s: 0, d: 2 })
        },
        // staircase seeding inserts bottom-up
        BoxOptions {
            insertion: Insertion::TopDownLevels,
            ..options(staircase(0))
        },
        // a tube needs a positive minimal rank
        BoxOptions {
            tubes: Some(TubeOptions {
                min_rank: 0,
                halo: 0,
                subsector_halo: None,
            }),
            ..options(staircase(0))
        },
        // settling needs expected counts ...
        BoxOptions {
            settle_on_expected: true,
            ..options(staircase(1))
        },
        // ... and staircase seeding
        BoxOptions {
            settle_on_expected: true,
            expected_counts: Some(Arc::new([(63, 1)].into_iter().collect())),
            ..options(Seeding::Demand {
                extra_r: 1,
                extra_s: 1,
                max_growth: 1,
            })
        },
    ] {
        assert!(matches!(
            session(options),
            Err(SolverError::InvalidInput(_))
        ));
    }
}

/// The two-loop kite (massless, one external `q` with `q^2 = 1`): the
/// auditor's family for the stability check.
const KITE: &str = "I(
  name(TOY2kite),
  loops(k1,k2),
  externals(q),
  dimension(d),
  prop(D1,(k1)^2,1),
  prop(D2,(-k1-k2)^2,1),
  prop(D3,(k2)^2,1),
  prop(D4,(k1+k2-q)^2,1),
  prop(D5,(-k2+q)^2,1),
  gram(q,q,1)
)";

/// An under-seeded system (a Kira box too small for the dotted targets) is
/// found unstable by the +1 check. `Report` returns it with
/// `stable == Some(false)`; `Strict` refuses it with a `MasterCheck` error.
#[test]
fn strict_refuses_an_unstable_system() {
    const K: usize = 5;
    let kite = Arc::new(rustred::node::driver::compile_family(KITE).unwrap());
    let targets: [[i16; K]; 2] = [[2, 1, 1, 1, 1], [1, 2, 1, 1, 2]];
    let run = |master_check: MasterCheckMode| {
        let router: Arc<dyn SectorRouter<K>> =
            Arc::new(CensusRouter::discover(kite.clone(), &[true; K]).unwrap());
        let mut session = BoxLaportaSession::<K>::new(
            kite.clone(),
            &CutConstraint::none(K).unwrap(),
            router,
            BoxOptions {
                seeding: Seeding::KiraBox { r: 6, s: 0, d: 1 },
                stability: Some((1, 1, 1)),
                master_check,
                workers: 1,
                lp_gate: false,
                ..Default::default()
            },
        )
        .unwrap();
        session.extend(&targets, &[])
    };
    let report = run(MasterCheckMode::Report).unwrap();
    assert_eq!(report.stable, Some(false));
    assert_ne!(report.masters, report.masters_before_stability);
    let refused = run(MasterCheckMode::Strict);
    assert!(
        matches!(&refused, Err(SolverError::MasterCheck(message)) if message.contains("stability")),
        "{refused:?}"
    );
}

/// A session that `Strict` refused stays refused (re-audit of #29): a
/// second `extend`, which seeds nothing new and so does not re-run the
/// stability check, is refused with the earlier verdict, as is an `extend`
/// with more targets. In `Report` mode a second `extend` keeps reporting
/// the earlier verdict (`stable == Some(false)`), not `None`.
#[test]
fn a_strict_refusal_fails_the_session() {
    const K: usize = 5;
    let kite = Arc::new(rustred::node::driver::compile_family(KITE).unwrap());
    let targets: [[i16; K]; 2] = [[2, 1, 1, 1, 1], [1, 2, 1, 1, 2]];
    let session = |master_check: MasterCheckMode| {
        let router: Arc<dyn SectorRouter<K>> =
            Arc::new(CensusRouter::discover(kite.clone(), &[true; K]).unwrap());
        BoxLaportaSession::<K>::new(
            kite.clone(),
            &CutConstraint::none(K).unwrap(),
            router,
            BoxOptions {
                seeding: Seeding::KiraBox { r: 6, s: 0, d: 1 },
                stability: Some((1, 1, 1)),
                master_check,
                workers: 1,
                lp_gate: false,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let mut strict = session(MasterCheckMode::Strict);
    assert!(matches!(
        strict.extend(&targets, &[]),
        Err(SolverError::MasterCheck(_))
    ));
    for again in [&targets[..], &targets[..1]] {
        let refused = strict.extend(again, &[]);
        assert!(
            matches!(&refused, Err(SolverError::MasterCheck(message))
                if message.contains("refused under Strict earlier") && message.contains("stability")),
            "{refused:?}"
        );
    }
    let mut report = session(MasterCheckMode::Report);
    assert_eq!(report.extend(&targets, &[]).unwrap().stable, Some(false));
    assert_eq!(report.extend(&targets, &[]).unwrap().stable, Some(false));
}

fn own_box_options() -> BoxOptions {
    BoxOptions {
        rebuild_rounds: true,
        growth_no_inherit: true,
        ..options(staircase(2))
    }
}

/// A growth series resumed from its first round's state (masters + growth
/// map, from the round hook) without eliminating that round again ends on
/// the same masters and pivots as the uninterrupted series.
#[test]
fn a_resumed_growth_series_equals_the_uninterrupted_one() {
    type State = (usize, Vec<[i16; N]>, BTreeMap<u64, (u8, u8)>);
    let states: Arc<std::sync::Mutex<Vec<State>>> = Arc::default();
    let mut whole = session(own_box_options()).unwrap();
    let sink = states.clone();
    whole.set_round_hook(move |round, masters, growth| {
        sink.lock()
            .unwrap()
            .push((round, masters.to_vec(), growth.clone()));
    });
    let a = whole.extend(&[TOP], &[TOP]).unwrap();
    assert!(
        a.rounds >= 2,
        "no growth round to resume from ({} rounds)",
        a.rounds
    );
    let (round, masters, growth) = states.lock().unwrap()[0].clone();
    assert_eq!(round, 1);
    let mut resumed = session(own_box_options()).unwrap();
    resumed.set_growth(growth);
    resumed.resume_from(masters).unwrap();
    let b = resumed.extend(&[TOP], &[TOP]).unwrap();
    assert_eq!(a.masters, b.masters);
    assert_eq!(a.pivot_hash, b.pivot_hash);
    assert_eq!(whole.stats().rows, resumed.stats().rows);
}

/// `growth_budget_seeds`: a budget the next batch cannot meet stops the
/// session with a rows limit; a budget it always meets changes nothing; and
/// the budget needs growth_no_inherit with rebuild_rounds.
#[test]
fn the_growth_budget_stops_or_changes_nothing() {
    let mut free = session(own_box_options()).unwrap();
    let (a, _) = closed(&mut free).unwrap();
    let mut roomy = session(BoxOptions {
        growth_budget_seeds: Some(usize::MAX),
        ..own_box_options()
    })
    .unwrap();
    let (b, _) = closed(&mut roomy).unwrap();
    assert_eq!(a.last().unwrap().masters, b.last().unwrap().masters);
    let mut tight = session(BoxOptions {
        growth_budget_seeds: Some(1),
        ..own_box_options()
    })
    .unwrap();
    assert!(matches!(closed(&mut tight), Err(SolverError::Limit { .. })));
    assert!(matches!(
        session(BoxOptions {
            growth_budget_seeds: Some(1),
            ..options(staircase(2))
        }),
        Err(SolverError::InvalidInput(_))
    ));
}
