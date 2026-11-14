//! T266 jointest9: keys-only (Tier K, superset) forward closure of box B under first match in rule order
//! (COORDINATOR_REQUIREMENTS R19, R20; the checker's k2 `k2eng close`): 59 rounds, 13,951,428 keys, 201,470,598 edges,
//! 12,501 uncovered = 1,102 + 11,399, used rules 12,658 (2,759 on the box), longest chain 814.
//!
//! Inputs (read-only handoff copy): `HO/t266-jointest9/cases.jsonl` (33,660 case records) and
//! `HO/t266-jointest9/F.keys.jsonl.gz` (132 MB gzip, 37,504,039 RHS patterns; streamed through `gzip -dc`).
//! `#[ignore]`: run in release under the 2 GB light-build scope:
//! `light_build.sh loop test -p rustred-campaign-core --release --test closure_t266 -- --ignored --nocapture`.

mod closure_common;

use std::io::BufReader;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use closure_common::{k3_common, peak_rss};
use rustred_campaign_core::closure::{
    ClosureConfig, HandDowns, KeyCodec, KeySetBuilder, NoFacts, RuleBodies, Tier, closure,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::{DomainOptions, TargetSource};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::record::{ReadOptions, read_table_file};

const T266_TARGETS: TargetSource = TargetSource::CanonicalByConstruction {
    attestation: "T266 jointest9 case export of an engine table_dump.v1 table; the checker's K2a found the key file \
                  targets canonical (target fixed slots = case fixed on 33,660 of 33,660 rules)",
};

/// Box B of T266 (14 lines): D 0..4, r <= 6 if D = 0 else r <= 7, no line-power cap (39 layers, 5,250,168 keys).
fn box_b(codec: KeyCodec) -> rustred_campaign_core::closure::KeySet {
    fn comps(n: usize, total: i32, cur: &mut Vec<i32>, out: &mut dyn FnMut(&[i32])) {
        if cur.len() == n - 1 {
            cur.push(total);
            out(cur);
            cur.pop();
            return;
        }
        for v in 0..=total {
            cur.push(v);
            comps(n, total - v, cur, out);
            cur.pop();
        }
    }
    let mut b = KeySetBuilder::new(codec);
    for d in 0..=4 {
        for r in 0..=(if d == 0 { 6 } else { 7 }) {
            let mut lines: Vec<Vec<i32>> = vec![];
            comps(14, d, &mut vec![], &mut |x| lines.push(x.to_vec()));
            comps(6, r, &mut vec![], &mut |y| {
                for x in &lines {
                    let k: Vec<i32> = x
                        .iter()
                        .map(|e| e + 1)
                        .chain(y.iter().map(|e| -e))
                        .collect();
                    b.push(&k).unwrap();
                }
            });
        }
    }
    b.finish()
}

/// Test-side workaround for d-free exception literals whose coefficients overflow i128 (the core's `Poly::parse`
/// refuses them; REQUEST to the core): the integer zero set of the literal on the codec range [-32, 31]^v (v <= 3
/// variables; a key outside that range is reported `unrepresentable` by the closure, so nothing outside matters) is
/// computed EXACTLY (p(x) mod 8 primes near 2^61, product > 2^480 > |p(x)| for these sizes) and the exception
/// branch holding the literal is replaced by one branch per zero point (the other literals plus `n_v - a` per
/// variable); no zero point: the branch is dropped (it can never hold). Same verdict on every representable key.
fn zero_points(text: &str) -> Option<(Vec<usize>, Vec<Vec<i64>>)> {
    const P: [u64; 8] = [
        2305843009213693951,
        2305843009213693921,
        2305843009213693907,
        2305843009213693723,
        2305843009213693693,
        2305843009213693669,
        2305843009213693613,
        2305843009213693561,
    ];
    let b = text.as_bytes();
    // terms: (neg, coefficient digits, [(var, exp)])
    type Term = (bool, String, Vec<(usize, u32)>);
    let mut terms: Vec<Term> = vec![];
    let mut vars: Vec<usize> = vec![];
    let mut start = 0;
    for i in 1..=b.len() {
        if i == b.len() || ((b[i] == b'+' || b[i] == b'-') && b[i - 1] != b'^' && b[i - 1] != b'*')
        {
            let t = &text[start..i];
            let (neg, t) = match t.as_bytes()[0] {
                b'-' => (true, &t[1..]),
                b'+' => (false, &t[1..]),
                _ => (false, t),
            };
            let nd = t.bytes().take_while(|c| c.is_ascii_digit()).count();
            let (c, rest) = if nd == 0 {
                ("1".to_string(), t)
            } else {
                (t[..nd].to_string(), t[nd..].trim_start_matches('*'))
            };
            let mut mono = vec![];
            for f in rest.split('*').filter(|f| !f.is_empty()) {
                let (v, e) = f
                    .split_once('^')
                    .map(|(v, e)| (v, e.parse::<u32>().ok()))
                    .unwrap_or((f, Some(1)));
                let v: usize = v.strip_prefix('n')?.parse().ok()?;
                if !vars.contains(&v) {
                    vars.push(v);
                }
                mono.push((v, e?));
            }
            terms.push((neg, c, mono));
            start = i;
        }
    }
    if vars.len() > 3 {
        return None;
    }
    vars.sort_unstable();
    let coef: Vec<[u128; 8]> = terms
        .iter()
        .map(|(neg, c, _)| {
            let mut r = [0u128; 8];
            for (j, &p) in P.iter().enumerate() {
                let p = p as u128;
                let v = c
                    .bytes()
                    .fold(0u128, |a, d| (a * 10 + (d - b'0') as u128) % p);
                r[j] = if *neg { (p - v) % p } else { v };
            }
            r
        })
        .collect();
    let mut pts = vec![];
    let n = vars.len();
    let total = 64usize.pow(n as u32);
    for idx in 0..total {
        let x: Vec<i64> = (0..n)
            .map(|q| ((idx / 64usize.pow(q as u32)) % 64) as i64 - 32)
            .collect();
        let zero = P.iter().enumerate().all(|(j, &p)| {
            let p = p as u128;
            let mut sum = 0u128;
            for (t, c) in terms.iter().zip(&coef) {
                let mut v = c[j];
                for &(var, e) in &t.2 {
                    let q = vars.iter().position(|&w| w == var).unwrap();
                    let xm = x[q].rem_euclid(p as i64) as u128;
                    for _ in 0..e {
                        v = v * xm % p;
                    }
                }
                sum = (sum + v) % p;
            }
            sum == 0
        });
        if zero {
            pts.push(x);
        }
    }
    Some((vars, pts))
}

#[test]
#[ignore = "T266 scale run (minutes, ~2 GB): run explicitly with --ignored in release"]
fn t266_tier_k_closure() {
    let dir = PathBuf::from(k3_common::HO).join("t266-jointest9");
    let (cases, keys) = (dir.join("cases.jsonl"), dir.join("F.keys.jsonl.gz"));
    if !cases.is_file() || !keys.is_file() {
        println!("SKIP: T266 inputs not found under {}", dir.display());
        return;
    }
    let t0 = Instant::now();
    let mut tf = read_table_file(&cases, ReadOptions::default()).unwrap();
    // Test-side workaround (REQUEST to the core: Poly::parse refuses coefficients beyond i128): a d-free literal whose
    // coefficients overflow i128 is replaced by its exact zero points (see `zero_points`).
    let mut reduced = 0usize;
    for r in tf.rules.iter_mut() {
        assert!(
            r.equations
                .iter()
                .all(|e| rustred_campaign_core::poly::Poly::parse_d_free(e, 20).is_ok())
        );
        let mut out: Vec<Vec<String>> = vec![];
        for br in &r.exceptions {
            let mut branches: Vec<Vec<String>> = vec![vec![]];
            for lit in br {
                if rustred_campaign_core::poly::Poly::parse_d_free(lit, 20).is_ok() {
                    branches.iter_mut().for_each(|b| b.push(lit.clone()));
                    continue;
                }
                let (vars, pts) = zero_points(lit).expect("big literal with at most 3 variables");
                reduced += 1;
                let mut nb = vec![];
                for b in &branches {
                    for p in &pts {
                        let mut b2 = b.clone();
                        b2.extend(vars.iter().zip(p).map(|(v, a)| format!("n{v}-({a})")));
                        nb.push(b2);
                    }
                }
                branches = nb;
            }
            out.extend(branches);
        }
        r.exceptions = out;
    }
    println!(
        "T266: {reduced} literal(s) replaced by their exact zero points on the codec range (i128 overflow workaround)"
    );
    let header = tf.header.as_ref().unwrap().raw.clone();
    let table = Table::from_table_file(
        "t266-jointest9",
        cases.display().to_string(),
        &tf,
        DomainOptions {
            targets: T266_TARGETS,
            // legacy reference numbers: exports carry no witness loci (re-audit RA-MAJOR-1, lane corefix2)
            witness: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(table.len(), 33_660);
    drop(tf);
    let disp = Dispatcher::new(vec![table], vec![0]).unwrap();
    let cmp = NodeComparator::from_header(&header).unwrap();
    println!(
        "T266: {} rules loaded, comparator {:?} ({:.1} s)",
        disp.tables()[0].len(),
        cmp.selection(),
        t0.elapsed().as_secs_f64()
    );
    let mut child = Command::new("gzip")
        .arg("-dc")
        .arg(&keys)
        .stdout(Stdio::piped())
        .spawn()
        .expect("gzip");
    let mut bodies = RuleBodies::new(&disp);
    let st = bodies
        .load_keys_jsonl(
            &disp,
            0,
            BufReader::with_capacity(1 << 20, child.stdout.take().unwrap()),
            2,
        )
        .unwrap();
    assert!(child.wait().unwrap().success());
    println!(
        "T266 bodies: {} rules, {} patterns, {} entries, {} MB ({:.1} s, peak rss {:?} MB)",
        st.rules,
        st.patterns,
        st.entries,
        bodies.memory_bytes() >> 20,
        t0.elapsed().as_secs_f64(),
        peak_rss().map(|b| b >> 20)
    );
    assert_eq!(st.rules, 33_660);
    assert_eq!(st.patterns, 37_504_039);
    let codec = KeyCodec::new(20, 6).unwrap();
    let bx = box_b(codec);
    assert_eq!(bx.len(), 5_250_168);
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 4,
        hand_downs: HandDowns::Counts,
        longest_chain: std::env::var("T266_NO_CHAIN").is_err(),
        ..Default::default()
    };
    let cl = closure(&disp, &bodies, &cmp, &bx, &cfg, &NoFacts).unwrap();
    let s = cl.summary();
    let nf_out = cl
        .open_keys()
        .iter()
        .filter(|o| !o.in_start && o.key[14..].iter().all(|&x| x == 0))
        .count();
    println!(
        "T266 closure: rounds {} keys {} (box {} + {}) edges {} open {} = {} + {} (numerator-free outside {nf_out}) \
         used rules {} (box {}) violations {} cycles {:?} chain {:?} terms {:?}",
        s.rounds,
        s.keys_total,
        s.keys_in_start,
        s.keys_outside,
        s.edges,
        s.open_total,
        s.open_in_start,
        s.open_outside,
        s.used_rules,
        s.used_rules_in_start,
        s.descent_violations,
        s.cycles,
        s.longest_chain.as_ref().map(|c| c.edges),
        s.terms
    );
    println!(
        "T266 closure: {:.1} s closure ({:.1} s total), memory estimate {} MB, peak rss {:?} MB, 4 threads",
        cl.seconds,
        t0.elapsed().as_secs_f64(),
        cl.memory_estimate_bytes >> 20,
        peak_rss().map(|b| b >> 20)
    );
    assert_eq!(s.rounds, 59);
    assert_eq!(s.keys_total, 13_951_428);
    assert_eq!(s.keys_in_start, 5_250_168);
    assert_eq!(s.edges, 201_470_598);
    assert_eq!(s.terms.top, 201_470_598);
    assert_eq!(s.terms.positive_numerator_top, 36_133_205);
    assert_eq!(
        s.terms.subsector + s.terms.positive_numerator_sub,
        1_076_897_813
    );
    assert_eq!(s.descent_violations, 0);
    assert_eq!(s.open_total, 12_501);
    assert_eq!(s.open_in_start, 1_102);
    assert_eq!(s.open_outside, 11_399);
    assert_eq!(nf_out, 10_169);
    assert_eq!(s.used_rules, 12_658);
    assert_eq!(s.used_rules_in_start, 2_759);
    if cfg.longest_chain {
        assert_eq!(s.cycles, Some(0));
        assert_eq!(s.longest_chain.as_ref().unwrap().edges, 814);
    }
}
