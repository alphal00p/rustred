//! Tier C against the coordinator's EC-2 tool as an oracle: a handful of real fact requests of T178 order B
//! (raw table R0_try1 with coefficients) sent through `rr_proof instantiate` (std build, read-only use) and the
//! answers fed back to the closure. `#[ignore]` (needs the binary and the raw table):
//! `light_build.sh loop test -p rustred-campaign-core --release --test closure_ec2 -- --ignored --nocapture`.

mod closure_common;
mod inputs;

use std::path::PathBuf;
use std::process::Command;

use closure_common::k3_common;
use rustred_campaign_core::closure::{
    ClosureConfig, FileFacts, HandDowns, KeyCodec, KeySet, NoFacts, RequestReason, RuleBodies,
    Tier, closure, write_fact_requests,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};

const RR_PROOF: &str =
    "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/build/proof-kernel-std/release/rr_proof";

#[test]
#[ignore = "runs the coordinator's rr_proof (std build) on a few requests"]
fn ec2_facts_feed_tier_c() {
    let raw = PathBuf::from(k3_common::HO).join("t178-orderB/bodies/R0_try1.F.table.jsonl");
    if !raw.is_file() || !PathBuf::from(RR_PROOF).is_file() {
        inputs::missing(format_args!(
            "SKIP: {} or {RR_PROOF} missing",
            raw.display()
        ));
        return;
    }
    let tf = read_table_file(
        &raw,
        ReadOptions {
            rhs: RhsMode::KeysOnly,
        },
    )
    .unwrap();
    let table = Table::from_table_file(
        "R0_try1",
        raw.display().to_string(),
        &tf,
        DomainOptions::default(),
    )
    .unwrap();
    let disp = Dispatcher::new(vec![table], vec![0]).unwrap();
    let mut bodies = RuleBodies::new(&disp);
    let st = bodies.load_table_records(&disp, 0, &tf).unwrap();
    println!(
        "R0_try1: {} records, {} bodies, {} patterns",
        disp.tables()[0].len(),
        st.rules,
        st.patterns
    );
    let cmp = NodeComparator::from_header(&tf.header.as_ref().unwrap().raw).unwrap();
    // a few covered box keys whose winner has a positive-numerator term
    let codec = KeyCodec::new(20, 6).unwrap();
    let probe = ClosureConfig {
        tier: Tier::K,
        threads: 2,
        max_rounds: Some(1),
        longest_chain: false,
        hand_downs: HandDowns::Off,
        ..Default::default()
    };
    let mut chosen: Vec<Vec<i32>> = vec![];
    for k in k3_common::t178_box() {
        if chosen.len() >= 4 {
            break;
        }
        let s = KeySet::from_keys(codec, [k.as_slice()]).unwrap();
        let c = closure(&disp, &bodies, &cmp, &s, &probe, &NoFacts)
            .unwrap()
            .summary();
        if c.open_in_start == 0
            && c.requires_coefficient_zero > 0
            && c.requires_coefficient_zero < 40
        {
            chosen.push(k);
        }
    }
    assert!(!chosen.is_empty());
    let start = KeySet::from_keys(codec, chosen.iter().map(|k| k.as_slice())).unwrap();
    let cfg = ClosureConfig {
        tier: Tier::C,
        table_digests: [(
            "R0_try1".to_string(),
            format!(
                "sha256:{}",
                k3_common::sha256_hex(&std::fs::read(&raw).unwrap())
            ),
        )]
        .into(),
        ..probe.clone()
    };
    let c0 = closure(&disp, &bodies, &cmp, &start, &cfg, &NoFacts).unwrap();
    let reqs = c0.fact_requests();
    // only the start keys (round 1): the requests of the truncated frontier are not issued
    let n_pos = reqs
        .iter()
        .filter(|r| r.reason == RequestReason::PositiveNumerator)
        .count();
    println!(
        "start keys {}, requests {} ({} positive numerator)",
        chosen.len(),
        reqs.len(),
        n_pos
    );
    let dir = PathBuf::from("/common/build-cache")
        .join(std::env::var("USER").unwrap_or_default())
        .join("p_integral_dev/implementer-scratch/loop/ec2");
    std::fs::create_dir_all(&dir).unwrap();
    let (rq, out) = (dir.join("requests.jsonl"), dir.join("answers.jsonl"));
    let _ = std::fs::remove_file(&out);
    write_fact_requests(&reqs, std::fs::File::create(&rq).unwrap()).unwrap();
    let o = Command::new(RR_PROOF)
        .args(["instantiate", "--requests"])
        .arg(&rq)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    println!(
        "rr_proof exit {:?}: {}",
        o.status.code(),
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .last()
            .unwrap_or("")
    );
    let facts = FileFacts::read_bound(
        std::io::BufReader::new(std::fs::File::open(&out).unwrap()),
        &reqs,
        false,
    )
    .unwrap();
    println!(
        "rejected answers: unrequested {}, term key not verified {}",
        facts.n_rejected_unrequested, facts.n_rejected_term_key
    );
    assert_eq!(facts.n_rejected(), 0, "{:?}", facts.rejections);
    println!(
        "facts {} (other lines {}, refusals {:?}, unknown {})",
        facts.n_facts,
        facts.n_other_lines,
        facts.refusals,
        facts.unknown_reasons.len()
    );
    let c1 = closure(&disp, &bodies, &cmp, &start, &cfg, &facts).unwrap();
    let (s0, s1) = (c0.summary(), c1.summary());
    println!(
        "Tier C without facts: undecided {:?}; with EC-2 facts: undecided {:?}, dropped zero {}, nonzero failures {}",
        s0.undecided, s1.undecided, s1.terms.dropped_zero, s1.positive_numerator_nonzero
    );
    assert!(facts.n_facts > 0);
    assert_eq!(
        s1.positive_numerator_nonzero, 0,
        "a raising coefficient did not vanish"
    );
    assert_eq!(s1.undecided.coef_unknown_positive_numerator, 0);
    assert!(s1.terms.dropped_zero >= n_pos as u64);
}
