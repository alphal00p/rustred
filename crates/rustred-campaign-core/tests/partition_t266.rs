//! T266 jointest9 at scale (R30, E-31): the checker's pre-extracted normal form (h11b `out/rules266_nf.json`,
//! 33,660 records = 26,012 distinct domains; read-only) and the closure's 12,501 uncovered keys (k2
//! `out/close/uncovered.txt`). (1) the FALLBACK face list from the uncovered closure keys (lower bound, 361 faces
//! in the checker's count) with every key confirmed uncovered by the exact oracle; (2) ONE timed attempt of the
//! exact partition under a wall-clock budget (default 25 min; `RUSTRED_T266_BUDGET_S`), reporting how far it got.
//! Both `#[ignore]`: `light_build.sh part test -p rustred-campaign-core --release --test partition_t266 --
//! --ignored --nocapture`.

use std::io::BufRead;
use std::path::PathBuf;
use std::time::Duration;

use rustred_campaign_core::cover::RuleRef;
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::partition::{
    Budget, DomainList, ElidedPolicy, KeyRange, LiteralTests, PartitionOptions, Progress,
    faces_from_uncovered_keys, gap_partition,
};
use rustred_campaign_core::poly::Poly;

fn src_dir() -> Option<PathBuf> {
    let p = PathBuf::from(
        std::env::var("RUSTRED_T266_SOURCES").unwrap_or("/common/ben/sym-investigation".into()),
    );
    if p.join("h11b/out/rules266_nf.json").is_file() {
        Some(p)
    } else {
        println!("SKIP: T266 sources not found");
        None
    }
}

fn load(dir: &std::path::Path) -> (DomainList, NodeFrame) {
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("h11b/out/rules266_nf.json")).unwrap(),
    )
    .unwrap();
    let e = v["E"].as_u64().unwrap() as usize;
    let n = 20;
    let frame = NodeFrame::without_header_lines_first(n, e);
    let mask: Vec<bool> = (0..n).map(|i| i < e).collect();
    let mut l = DomainList::new(n);
    for r in v["rules"].as_array().unwrap() {
        let mut d = CaseDomain::free(n, Some(mask.clone()));
        let p = |s: &serde_json::Value| {
            Poly::parse(
                &s.as_str().unwrap().replace("**", "^").replace(' ', ""),
                n,
                false,
            )
            .ok()
        };
        // a literal whose coefficients exceed i128 cannot be represented: the rule is EXCLUDED (fail closed: fewer
        // covered keys, never more)
        let eq: Option<Vec<Poly>> = r["EQ"].as_array().unwrap().iter().map(p).collect();
        let bad: Option<Vec<Vec<Poly>>> = r["BAD"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b.as_array().unwrap().iter().map(p).collect())
            .collect();
        let (Some(eq), Some(bad)) = (eq, bad) else {
            println!("excluded rule {} (coefficient beyond i128)", r["k"]);
            continue;
        };
        d.equations = eq;
        d.exceptions = bad;
        let k = r["k"].as_u64().unwrap() as usize;
        l.push(
            RuleRef {
                table: 0,
                record: k,
                label: k as i64,
            },
            d,
        );
    }
    (l, frame)
}

fn uncovered_keys(dir: &std::path::Path) -> Vec<Key> {
    let f = std::io::BufReader::new(
        std::fs::File::open(dir.join("k2/out/close/uncovered.txt")).unwrap(),
    );
    f.lines()
        .map(|l| {
            let l = l.unwrap();
            let k = l.split_whitespace().next().unwrap();
            serde_json::from_str::<Vec<i32>>(k).unwrap()
        })
        .collect()
}

#[test]
#[ignore = "needs the checker's T266 sources (read-only); release; run with --ignored"]
fn t266_fallback_faces_from_closure_gaps() {
    let Some(dir) = src_dir() else { return };
    let (l, frame) = load(&dir);
    println!("{} of 26,012 distinct domains loaded", l.items.len());
    let keys = uncovered_keys(&dir);
    assert_eq!(keys.len(), 12_501);
    let opts = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        resolve_loci: false,
        budget: Budget {
            max_nodes: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };
    let rep = gap_partition(&l, &frame, &opts, None).unwrap();
    println!("T266 normal form: {:?}", rep.nf.stats);
    let mut unc = 0;
    for k in &keys {
        if rep.uncovered(k).unwrap() {
            unc += 1;
        }
    }
    let faces = faces_from_uncovered_keys(&frame, &keys, KeyRange::Measured);
    let max_dots = faces.iter().map(|f| f.sd.len()).max().unwrap();
    println!(
        "T266 fallback: {} faces (lower bound, closure_lower_bound), up to {max_dots} dotted lines; {unc} of 12,501 keys uncovered by the exact oracle",
        faces.len()
    );
    assert_eq!(unc, 12_501);
    assert_eq!(faces.len(), 361);
}

#[test]
#[ignore = "timed attempt (<= 30 min, 2 GB scope); run with --ignored"]
fn t266_exact_partition_timed_attempt() {
    let Some(dir) = src_dir() else { return };
    let (l, frame) = load(&dir);
    let secs: u64 = std::env::var("RUSTRED_T266_BUDGET_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1500);
    let opts = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        literal_tests: LiteralTests::Exact,
        resolve_loci: false,
        budget: Budget {
            max_nodes: None,
            wall: Some(Duration::from_secs(secs)),
            max_memo: Some(120_000),
        },
        ..Default::default()
    };
    let cb = |p: &Progress| {
        println!(
            "progress: nodes {} memo {} depth {} at {} ms",
            p.nodes, p.memo_size, p.depth, p.seconds_x1000
        )
    };
    let rep = gap_partition(&l, &frame, &opts, Some(&cb)).unwrap();
    println!(
        "T266 attempt: outcome {:?}; inputs {:?}; states {:?}; nodes {} memo hits {} memo clears {}; U {} F {} (partial if budget exceeded); faces {}; {} ms",
        rep.outcome,
        rep.nf.stats,
        rep.states.iter().map(|s| s.len()).collect::<Vec<_>>(),
        rep.stats.nodes,
        rep.stats.memo_hits,
        rep.stats.memo_clears,
        rep.n_u(),
        rep.n_f(),
        rep.faces.len(),
        rep.millis
    );
}
