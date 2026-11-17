//! Session tests on the three-loop K6 (tetrahedron) vacuum family with line 0
//! deformed to mass^2 `1 + eta`, parameters `(d, eta)`: the eta-derivative
//! demand of the campaign's K6 DE has 12 masters (campaign engine and RustRed
//! main's LiteRed lane agree on them).

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::sector::CutConstraint;

use super::super::super::SolverError;
use super::super::{
    BoxLimits, BoxOptions, CensusRouter, ColumnOrder, IdentityRouter, Insertion, LimitKind,
    SectorRouter, Seeding, SlotPriority,
};
use super::{BoxLaportaSession, census_targets, down_closure_representatives};

const N: usize = 6;

fn k6_eta() -> Arc<IntegralFamily> {
    const ROWS: [[i64; 6]; 6] = [
        [1, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [1, 0, -2, 0, 0, 1],
        [1, -2, 0, 1, 0, 0],
        [0, 0, 0, 1, -2, 1],
    ];
    let context = CoefficientContext::try_new(["d", "eta"]).unwrap();
    let denominators = ROWS
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
            "k6_eta",
            ["k1", "k2", "k3"].map(String::from).to_vec(),
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

/// The eta-derivative demand of the campaign K6 DE: the top integral and the
/// derivative seeds `m + e_0` of its masters with a positive slot-0 power.
const TARGETS: [[i16; N]; 9] = [
    [1, 1, 1, 1, 1, 1],
    [2, 1, 1, 1, 1, 1],
    [2, 1, 1, 1, 1, 0],
    [2, 1, 1, 1, 0, 1],
    [2, 1, 0, 1, 0, 2],
    [2, 1, 1, 1, 0, 0],
    [2, 1, 1, 0, 0, 1],
    [2, 1, 0, 1, 0, 1],
    [2, 1, 1, 0, 0, 0],
];

/// The campaign engine's 12 masters for this demand.
const CAMPAIGN_MASTERS: [[i16; N]; 12] = [
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

fn options(workers: usize, insertion: Insertion) -> BoxOptions {
    BoxOptions {
        seeding: Seeding::KiraBox { r: 8, s: 1, d: 2 },
        order: ColumnOrder::LinesRankDots,
        slot_priority: SlotPriority::TopSectorFirst(63),
        workers,
        insertion,
        ..BoxOptions::default()
    }
}

fn session(workers: usize, insertion: Insertion) -> BoxLaportaSession<N> {
    session_with(options(workers, insertion))
}

fn session_with(options: BoxOptions) -> BoxLaportaSession<N> {
    let family = k6_eta();
    let router: Arc<dyn SectorRouter<N>> =
        Arc::new(CensusRouter::discover(family.clone(), &[true; N]).unwrap());
    BoxLaportaSession::new(family, &CutConstraint::none(N).unwrap(), router, options).unwrap()
}

#[test]
fn k6_eta_has_twelve_masters_at_both_samples() {
    let mut session = session(2, Insertion::BottomUp);
    let report = session.extend(&TARGETS, &[]).unwrap();
    assert_eq!(report.masters.len(), 12, "{:?}", report.masters);
    assert_eq!(report.masters_first_sample, 12);
    assert_eq!(report.masters_second_sample, 12);
    assert_eq!(report.rounds, 1);
    assert!(report.growth.is_empty());
    assert!(report.selected_rows > 0 && report.selected_rows <= session.stats().rows);
    assert_eq!(session.masters(), &report.masters[..]);
}

#[test]
fn preferred_campaign_masters_become_the_basis() {
    let mut session = session(2, Insertion::BottomUp);
    let report = session.extend(&TARGETS, &CAMPAIGN_MASTERS).unwrap();
    let masters: BTreeSet<[i16; N]> = report.masters.iter().copied().collect();
    let expected: BTreeSet<[i16; N]> = CAMPAIGN_MASTERS.iter().copied().collect();
    assert_eq!(masters, expected);
}

#[test]
fn masters_do_not_depend_on_workers_or_insertion_schedule() {
    let one = session(1, Insertion::BottomUp)
        .extend(&TARGETS, &[])
        .unwrap();
    let three = session(3, Insertion::BottomUp)
        .extend(&TARGETS, &[])
        .unwrap();
    assert_eq!(one.masters, three.masters);
    assert_eq!(one.pivot_hash, three.pivot_hash);
    assert_eq!(one.selected_hash, three.selected_hash);
    let top_down = session(2, Insertion::TopDownLevels)
        .extend(&TARGETS, &[])
        .unwrap();
    assert_eq!(one.masters.len(), top_down.masters.len());
}

#[test]
fn extend_is_incremental() {
    let mut session = session(2, Insertion::BottomUp);
    let first = session.extend(&TARGETS[..1], &[]).unwrap();
    let rows = session.stats().rows;
    let second = session.extend(&TARGETS, &[]).unwrap();
    assert!(session.stats().rows >= rows);
    assert_eq!(second.masters.len(), 12);
    // The earlier rows are kept: the system only grows, and the top
    // integral alone needs no more masters than the whole demand.
    assert!(first.masters.len() <= second.masters.len());
}

/// Review finding (PR 6): a preferred integral that does not end up as a master
/// is reported. Four preferred integrals of the top sector, which has fewer
/// masters than that: each preferred integral is either a master or reported
/// in `preferred_not_masters`, never both, and at least one is reported.
#[test]
fn preferred_integrals_that_are_not_masters_are_reported() {
    let mut session = session(2, Insertion::BottomUp);
    let preferred = [
        [1, 1, 1, 1, 1, 1],
        [2, 1, 1, 1, 1, 1],
        [1, 2, 1, 1, 1, 1],
        [1, 1, 2, 1, 1, 1],
    ];
    let report = session.extend(&TARGETS, &preferred).unwrap();
    let masters: BTreeSet<[i16; N]> = report.masters.iter().copied().collect();
    let reported: BTreeSet<[i16; N]> = report.preferred_not_masters.iter().copied().collect();
    assert!(!reported.is_empty(), "{:?}", report.masters);
    for integral in preferred {
        assert!(
            masters.contains(&integral) != reported.contains(&integral),
            "{integral:?}: masters {:?}, reported {reported:?}",
            report.masters
        );
    }
    // Without preferred integrals nothing is reported.
    let plain = session_with(options(2, Insertion::BottomUp))
        .extend(&TARGETS, &[])
        .unwrap();
    assert!(plain.preferred_not_masters.is_empty());
}

/// Review finding (PR 6): every opt-in cap is a typed error.
#[test]
fn opt_in_caps_are_typed_errors() {
    let capped = |limits: BoxLimits| {
        session_with(BoxOptions {
            limits,
            ..options(2, Insertion::BottomUp)
        })
        .extend(&TARGETS, &[])
    };
    let kind = |result: Result<_, SolverError>| match result {
        Err(SolverError::Limit {
            kind,
            estimate,
            cap,
        }) => {
            assert!(estimate > cap);
            Some(kind)
        }
        _ => None,
    };
    assert_eq!(
        kind(capped(BoxLimits {
            max_rows: 10,
            ..BoxLimits::default()
        })),
        Some(LimitKind::Rows)
    );
    assert_eq!(
        kind(capped(BoxLimits {
            max_columns: 10,
            ..BoxLimits::default()
        })),
        Some(LimitKind::Columns)
    );
    assert_eq!(
        kind(capped(BoxLimits {
            max_bytes: 1_000,
            ..BoxLimits::default()
        })),
        Some(LimitKind::Bytes)
    );
    // The default has no cap.
    assert!(capped(BoxLimits::default()).is_ok());
}

/// Review finding (PR 6): Kira 3 truncation caps the numerator rank of
/// non-demand subsectors, so it seeds fewer integrals; with a box this large
/// the masters do not change.
#[test]
fn truncation_seeds_less_and_keeps_the_masters() {
    let wide = |truncate| {
        let mut session = session_with(BoxOptions {
            seeding: Seeding::KiraBox { r: 8, s: 2, d: 2 },
            truncate,
            ..options(2, Insertion::BottomUp)
        });
        let report = session.extend(&TARGETS, &[]).unwrap();
        (report.masters.len(), session.stats().seeds, report.truncate)
    };
    let (full_masters, full_seeds, none) = wide(None);
    let (cut_masters, cut_seeds, some) = wide(Some(5));
    assert_eq!((none, some), (None, Some(5)));
    assert_eq!(full_masters, 12);
    assert_eq!(cut_masters, 12);
    assert!(cut_seeds < full_seeds, "{cut_seeds} vs {full_seeds}");
}

/// Two-loop sunrise vacuum with three unit-mass lines: two masters with the
/// census router (the sunrise and one tadpole product), four with the
/// identity router (the three tadpole products are distinct sectors there).
#[test]
fn identity_router_session_keeps_symmetric_copies() {
    const M: usize = 3;
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let rows: [[i64; 3]; 3] = [[1, 0, 0], [0, 0, 1], [1, -2, 1]];
    let denominators = rows
        .iter()
        .map(|row| {
            AffineDenominator::new(
                context.integer(-1),
                row.map(|value| context.integer(value)).to_vec(),
            )
        })
        .collect();
    let family = Arc::new(
        IntegralFamily::new(
            "sunrise2",
            ["k1", "k2"].map(String::from).to_vec(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); M],
        )
        .unwrap(),
    );
    let run = |router: Arc<dyn SectorRouter<M>>| {
        let mut session = BoxLaportaSession::<M>::new(
            family.clone(),
            &CutConstraint::none(M).unwrap(),
            router,
            BoxOptions {
                seeding: Seeding::KiraBox { r: 6, s: 0, d: 3 },
                workers: 2,
                ..BoxOptions::default()
            },
        )
        .unwrap();
        session
            .extend(&[[1, 1, 1], [2, 1, 1], [2, 2, 2]], &[])
            .unwrap()
    };
    let census = run(Arc::new(
        CensusRouter::discover(family.clone(), &[true; M]).unwrap(),
    ));
    let identity = run(IdentityRouter::shared(&family).unwrap());
    assert_eq!(census.masters.len(), 2, "{:?}", census.masters);
    assert_eq!(identity.masters.len(), 4, "{:?}", identity.masters);
    assert!(identity.masters.contains(&[1, 1, 1]));
    for sector in [[1, 1, 0], [1, 0, 1], [0, 1, 1]] {
        assert!(
            identity
                .masters
                .iter()
                .any(|master| master.map(|p| p > 0) == sector.map(|p| p > 0)),
            "{:?}",
            identity.masters
        );
    }
}

/// `modular_reduction` expresses a demanded integral over masters, and
/// `set_progress` receives phase lines.
#[test]
fn modular_reduction_and_progress() {
    let lines = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut session = session(2, Insertion::BottomUp);
    let counter = lines.clone();
    session.set_progress(move |_| {
        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    });
    let report = session.extend(&TARGETS, &[]).unwrap();
    assert!(lines.load(std::sync::atomic::Ordering::Relaxed) > 0);
    let masters: BTreeSet<[i16; N]> = report.masters.iter().copied().collect();
    let row = session.modular_reduction(&TARGETS[1]).unwrap();
    assert!(!row.is_empty());
    assert!(
        row.iter()
            .all(|(integral, value)| masters.contains(integral) && *value != 0)
    );
    // A master reduces to itself.
    let master = report.masters[0];
    assert_eq!(
        session.modular_reduction(&master).unwrap(),
        vec![(master, 1)]
    );
}

/// `down_closure_representatives` and `census_targets` on the K6 census: the
/// representative down-closure of the top, and one corner per representative
/// sector without dots or numerators.
#[test]
fn census_demand_of_the_top_sector() {
    let family = k6_eta();
    let router = CensusRouter::discover(family.clone(), &[true; N]).unwrap();
    let representative = |bits: u64| {
        let sector: [bool; N] = std::array::from_fn(|slot| bits >> slot & 1 == 1);
        SectorRouter::<N>::representative(&router, &sector).map(|rep| {
            rep.iter()
                .enumerate()
                .filter(|(_, active)| **active)
                .map(|(slot, _)| 1u64 << slot)
                .sum::<u64>()
        })
    };
    let classes = down_closure_representatives([63], representative);
    assert!(classes.contains(&63));
    let corners = census_targets::<N>(&router, N, &[true; N], 0, 0);
    assert_eq!(corners.len(), classes.len());
    assert!(
        corners
            .iter()
            .all(|corner| corner.iter().all(|&p| p == 0 || p == 1))
    );
}
