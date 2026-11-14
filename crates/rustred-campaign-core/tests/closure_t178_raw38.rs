//! T178 order B closure over the 38 RAW piece tables with the core's witness check ON (corefix2, coordinator
//! addendum 3): the box gives 285,618 covered / 409 uncovered / 39 Blocked(Elided) (tests/data/t178b_raw38_blocked39.
//! json); the closure from that box treats those 39 as open keys naming their blocking records. NEW reference numbers
//! (recorded here). The same open keys from the case exports with the checked attestation of each raw table (the elided
//! records are not attested and block as WitnessNotCarried, whose bodies Tier K follows since lane closurefix); with the
//! witness check OFF the legacy 330,287 keys / 753 open.
//! `#[ignore]`: needs RUSTRED_CAMPAIGN_FIXTURES and the raw tables (RUSTRED_T178B_RAW, cover_raw_tables.rs);
//! `light_build.sh <lane> test -p rustred-campaign-core --release --test closure_t178_raw38 -- --ignored --nocapture`.

mod closure_common;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use closure_common::{box_set, headers, k3_common, t178_bodies};
use rustred_campaign_core::closure::{
    ClosureConfig, ClosureSummary, HandDowns, NoFacts, OpenStatus, Tier, closure,
};
use rustred_campaign_core::cover::{Attestation, Dispatcher, Table};
use rustred_campaign_core::domain::{DomainOptions, TargetSource};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::record::{ReadOptions, read_table_file};

const EXPORT_TARGETS: TargetSource = TargetSource::CanonicalByConstruction {
    attestation: "T178B case exports of engine table_dump.v1 tables",
};

fn raw_table_path(piece: &str) -> PathBuf {
    let root = std::env::var_os("RUSTRED_T178B_RAW")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/t178/regenB",
            )
        });
    let rel = match piece.split_once('_') {
        None if piece == "R0" => "R0/try1".to_string(),
        Some((dir, sub)) => format!("{dir}/{sub}"),
        None => panic!("unknown piece {piece}"),
    };
    root.join(rel).join("node/F.table.jsonl")
}

fn run(disp: &Dispatcher, tag: &str) -> (ClosureSummary, BTreeMap<(String, i64), usize>) {
    let bodies = t178_bodies(disp);
    let cmp = NodeComparator::from_header(&headers()["t178B_table"]).unwrap();
    let bx = box_set(&k3_common::t178_box());
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 4,
        hand_downs: HandDowns::Counts,
        ..Default::default()
    };
    let c = closure(disp, &bodies, &cmp, &bx, &cfg, &NoFacts).unwrap();
    let s = c.summary();
    let mut blocked: BTreeMap<(String, i64), usize> = BTreeMap::new();
    for o in c
        .open_keys()
        .iter()
        .filter(|o| o.status == OpenStatus::Blocked && o.in_start)
    {
        let r = o.rule.as_ref().expect("a blocked key names its record");
        *blocked.entry((r.table.clone(), r.label)).or_default() += 1;
    }
    println!(
        "T178B {tag}: rounds {} keys {} (box {} + {}) edges {} open {} = {} + {} by status {:?} violations {} \
         used {} blocked box keys by record {:?} ({:.1} s)",
        s.rounds,
        s.keys_total,
        s.keys_in_start,
        s.keys_outside,
        s.edges,
        s.open_total,
        s.open_in_start,
        s.open_outside,
        s.open_by_status,
        s.descent_violations,
        s.used_rules,
        blocked,
        c.seconds
    );
    (s, blocked)
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES and the 38 raw T178B tables; run with --ignored in release"]
fn t178b_raw38_closure_witness_on() {
    let Some(fx) = std::env::var_os("RUSTRED_CAMPAIGN_FIXTURES").map(PathBuf::from) else {
        println!("SKIP: RUSTRED_CAMPAIGN_FIXTURES not set");
        return;
    };
    let cfg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fx.join("T178B/cfg_t178_regenB.json")).unwrap(),
    )
    .unwrap();
    let names: Vec<String> = cfg["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            t.as_str()
                .unwrap()
                .trim_start_matches("cases/t178B/")
                .trim_end_matches(".jsonl")
                .to_string()
        })
        .collect();
    assert_eq!(names.len(), 38);
    let export_opts = DomainOptions {
        targets: EXPORT_TARGETS,
        ..Default::default()
    };
    let (mut raw_tables, mut attested, mut legacy) = (vec![], vec![], vec![]);
    for name in &names {
        let cases = read_table_file(
            fx.join(format!("T178B/cases/{name}.jsonl")),
            ReadOptions::default(),
        )
        .unwrap();
        let rp = raw_table_path(name);
        if !rp.is_file() {
            println!("SKIP: {} missing", rp.display());
            return;
        }
        let raw = read_table_file(&rp, ReadOptions::default()).unwrap();
        let mut rt = Table::from_table_file(
            name.clone(),
            rp.display().to_string(),
            &raw,
            DomainOptions::default(),
        )
        .unwrap();
        let i_of: HashMap<i64, i64> = cases
            .rules
            .iter()
            .map(|c| (c.index.unwrap(), c.i.unwrap()))
            .collect();
        for (k, r) in raw.rules.iter().enumerate() {
            rt.labels[k] = i_of[&r.index.unwrap()];
        }
        let a = Attestation::from_raw_table(
            &raw,
            None,
            format!("built from {} by the test", rp.display()),
        )
        .unwrap();
        attested.push(
            Table::from_table_file_attested(
                name.clone(),
                name.clone(),
                &cases,
                export_opts,
                &a,
                Some(&raw),
            )
            .unwrap(),
        );
        legacy.push(
            Table::from_table_file(
                name.clone(),
                name.clone(),
                &cases,
                DomainOptions {
                    witness: false,
                    ..export_opts
                },
            )
            .unwrap(),
        );
        raw_tables.push(rt);
    }
    let prio: Vec<usize> = (0..38).collect();
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/t178b_raw38_blocked39.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let want: BTreeMap<(String, i64), usize> = fixture["blocking_records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                (
                    r["piece"].as_str().unwrap().to_string(),
                    r["case_i"].as_i64().unwrap(),
                ),
                r["box_keys"].as_u64().unwrap() as usize,
            )
        })
        .collect();
    // (1) raw tables, witness check on
    let (s1, b1) = run(
        &Dispatcher::new(raw_tables, prio.clone()).unwrap(),
        "raw38 witness on",
    );
    assert_eq!(s1.keys_in_start, 286_066);
    assert_eq!(
        b1, want,
        "the 39 blocked box keys name their blocking records"
    );
    assert_eq!(s1.descent_violations, 0);
    // NEW reference (2026-11-14, loopfix): 330,260 keys = 286,066 + 44,194; 4,378,106 edges; open 799 = 448 + 351
    // (753 uncovered + 46 blocked, 39 of them in the box); 1,435 used rules; 27 rounds
    assert_eq!(
        (
            s1.rounds,
            s1.keys_total,
            s1.keys_outside,
            s1.edges,
            s1.open_total,
            s1.open_in_start,
            s1.open_outside,
            s1.used_rules
        ),
        (27, 330_260, 44_194, 4_378_106, 799, 448, 351, 1_435)
    );
    assert_eq!(s1.open_by_status.get("blocked"), Some(&46));
    assert_eq!(s1.open_by_status.get("uncovered"), Some(&753));
    // (2) exports + checked attestation: the same open keys and blocking records. Since lane closurefix (re-audit F1b)
    // the 46 keys block there as WitnessNotCarried (the elided records are not attested), so Tier K FOLLOWS their
    // bodies (a Tier C run may lift them) and reaches the legacy key set (330,287 keys, 4,387,847 edges, 1,439 used);
    // in the raw tables they block as Elided (never lifted, never followed).
    let (s2, b2) = run(
        &Dispatcher::new(attested, prio.clone()).unwrap(),
        "exports attested (checked)",
    );
    assert_eq!(
        (
            s2.open_total,
            s2.open_in_start,
            s2.open_by_status.get("blocked")
        ),
        (s1.open_total, s1.open_in_start, Some(&46))
    );
    assert_eq!(
        (s2.keys_total, s2.edges, s2.used_rules, s2.open_followed),
        (330_287, 4_387_847, 1_439, 46)
    );
    assert_eq!((s1.open_followed, s1.requires_regularity_fact), (0, 0));
    assert_eq!(b2, b1);
    // (3) witness check off: the legacy numbers
    let (s3, _) = run(
        &Dispatcher::new(legacy, prio).unwrap(),
        "exports witness off (legacy)",
    );
    assert_eq!(
        (
            s3.keys_total,
            s3.open_total,
            s3.open_in_start,
            s3.open_outside
        ),
        (330_287, 753, 409, 344)
    );
}
