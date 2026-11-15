//! B1 (interfaces/parametric.md section 1): per-rule descent on the WHOLE domain. Constructed records with known
//! hand-downs, positive numerators (with / without zero proofs), non-descending and self terms, vacuous domains, and a
//! seeded differential against brute-force enumeration of small boxes (legacy and program comparators).

use std::cmp::Ordering;

use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::loci::contain::ProofBudget;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::parametric::descent::{
    DescentContext, EmptyState, Rel, RuleInput, TermVerdict, ZeroIndex, read_record, rule_descent,
};
use rustred_campaign_core::poly::Poly;
use serde_json::json;

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn poly(s: &str, n: usize) -> Poly {
    Poly::parse_d_free_exact(s, n).unwrap()
}

fn domain(frame: &NodeFrame, fixed: &[Option<i32>], exceptions: &[&[&str]]) -> CaseDomain {
    let n = frame.arity();
    let mut d = CaseDomain::free(n, Some(frame.mask().to_vec()));
    d.fixed = fixed.to_vec();
    d.exceptions = exceptions
        .iter()
        .map(|b| b.iter().map(|m| poly(m, n)).collect())
        .collect();
    d
}

fn run(
    frame: &NodeFrame,
    cmp: &NodeComparator,
    zeros: Option<&ZeroIndex>,
    dom: &CaseDomain,
    target: &str,
    rhs: &[&str],
) -> rustred_campaign_core::parametric::descent::RuleDescent {
    let ctx = DescentContext {
        frame,
        cmp,
        zeros,
        budget: ProofBudget::default(),
    };
    let rhs: Vec<String> = rhs.iter().map(|s| s.to_string()).collect();
    rule_descent(
        &ctx,
        &RuleInput {
            domain: dom,
            target,
            rhs: &rhs,
            record_sha256: Some(SHA),
        },
    )
}

fn frame21() -> (NodeFrame, NodeComparator) {
    (
        NodeFrame::without_header_lines_first(3, 2),
        NodeComparator::from_descriptor(rustred_order::OrderDescriptor::spired_uncut(3)).unwrap(),
    )
}

#[test]
fn in_chart_descent_with_a_hand_down_plane() {
    let (f, c) = frame21();
    let d = domain(&f, &[None, None, None], &[]);
    let r = run(&f, &c, None, &d, "I(n0,n1,n2)", &["I(n0-1,n1,n2)"]);
    match &r.terms[0] {
        TermVerdict::Descends {
            hand_down,
            discharged,
        } => {
            assert!(discharged.is_empty());
            assert_eq!(hand_down.len(), 1);
            assert_eq!(
                (hand_down[0].slot, hand_down[0].relation, hand_down[0].value),
                (0, Rel::Eq, 1)
            );
            assert_eq!(hand_down[0].emptiness, EmptyState::NonEmpty);
        }
        o => panic!("{o:?}"),
    }
    // the record excepts n0 = 1: the term never hands down
    let d = domain(&f, &[None, None, None], &[&["n0-1"]]);
    let r = run(&f, &c, None, &d, "I(n0,n1,n2)", &["I(n0-1,n1,n2)"]);
    assert_eq!(
        r.terms[0],
        TermVerdict::Descends {
            hand_down: vec![],
            discharged: vec![]
        }
    );
    assert_eq!(r.worst, "descends");
}

#[test]
fn positive_numerator_planes_need_a_zero_proof_on_that_plane() {
    let (f, c) = frame21();
    let d = domain(&f, &[None, None, None], &[]);
    let rhs = ["I(n0,n1-1,n2+1)"];
    let r = run(&f, &c, None, &d, "I(n0,n1,n2)", &rhs);
    match &r.terms[0] {
        TermVerdict::DescendsExcept { loci, .. } => {
            assert_eq!(loci.len(), 1);
            assert_eq!(
                (loci[0].slot, loci[0].relation, loci[0].value),
                (2, Rel::Eq, 0)
            );
            assert_eq!(loci[0].emptiness, EmptyState::NonEmpty);
            let w = loci[0].witness.as_ref().unwrap();
            assert_eq!(w[2], 0);
        }
        o => panic!("{o:?}"),
    }
    assert_eq!(r.worst, "descends_except");
    // an exception branch removes the plane
    let d2 = domain(&f, &[None, None, None], &[&["n2"]]);
    assert_eq!(
        run(&f, &c, None, &d2, "I(n0,n1,n2)", &rhs).worst,
        "descends"
    );
    // a zero proof of ANOTHER plane / term does not discharge; of this plane it does
    let mut z = ZeroIndex::new();
    for (term, plane) in [(1, [2, 0]), (0, [2, -1])] {
        z.insert_value(&json!({"schema":"rustred.campaign.zero-proof.v1","record_sha256":SHA,"rule":0,"term":term,
            "locus":{"plane":plane},"verdict":"zero_on_locus","denominator":"nowhere_zero","tool":"t@1"}))
            .unwrap();
    }
    assert_eq!(
        run(&f, &c, Some(&z), &d, "I(n0,n1,n2)", &rhs).worst,
        "descends_except"
    );
    z.insert_value(&json!({"schema":"rustred.campaign.zero-proof.v1","record_sha256":SHA,"rule":0,"term":0,
        "locus":{"plane":[2,0]},"verdict":"zero_on_locus","denominator":"where_regular","tool":"t@1"}))
        .unwrap();
    match &run(&f, &c, Some(&z), &d, "I(n0,n1,n2)", &rhs).terms[0] {
        TermVerdict::Descends { discharged, .. } => {
            assert_eq!(discharged.len(), 1);
            assert_eq!(discharged[0].zero_proof, Some("where_regular"));
        }
        o => panic!("{o:?}"),
    }
    // a not-proved line never discharges
    let mut z2 = ZeroIndex::new();
    z2.insert_value(
        &json!({"schema":"rustred.campaign.zero-proof.v1","record_sha256":SHA,"rule":0,"term":0,
        "locus":"domain","verdict":"not_proved","reason":"x","tool":"t@1"}),
    )
    .unwrap();
    assert_eq!(
        run(&f, &c, Some(&z2), &d, "I(n0,n1,n2)", &rhs).worst,
        "descends_except"
    );
}

#[test]
fn non_descending_and_self_terms_have_verified_witnesses() {
    let (f, c) = frame21();
    let d = domain(&f, &[None, None, None], &[&["n1-3"]]);
    let r = run(
        &f,
        &c,
        None,
        &d,
        "I(n0,n1,n2)",
        &["I(n0+1,n1,n2)", "I(n0,n1,n2)", "I(n0,n1-1,n2)"],
    );
    for (j, want) in [(0, "Greater"), (1, "Equal")] {
        match &r.terms[j] {
            TermVerdict::DoesNotDescend {
                witness,
                comparison,
            } => {
                assert_eq!(comparison, want);
                assert!(d.applies(witness).applies());
            }
            o => panic!("term {j}: {o:?}"),
        }
    }
    assert_eq!(r.worst, "does_not_descend");
    assert_eq!(r.counts.get("does_not_descend"), Some(&2));
    assert_eq!(r.counts.get("descends"), Some(&1));
}

#[test]
fn a_non_lower_pattern_that_always_hands_down_descends() {
    // n1 fixed to 1: the term's slot 1 is 0 on the whole domain (a hand-down everywhere), so the in-chart part is empty
    let (f, c) = frame21();
    let d = domain(&f, &[None, Some(1), None], &[]);
    let r = run(&f, &c, None, &d, "I(n0,1,n2)", &["I(n0+2,0,n2)"]);
    match &r.terms[0] {
        TermVerdict::Descends { hand_down, .. } => {
            assert_eq!(hand_down[0].relation, Rel::Domain);
        }
        o => panic!("{o:?}"),
    }
}

#[test]
fn index_dependent_comparisons_are_undecided_and_empty_domains_vacuous() {
    let (f, c) = frame21();
    let d = domain(&f, &[None, None, None], &[]);
    let r = run(&f, &c, None, &d, "I(n0,n1,n2)", &["I(2,n1,n2)"]);
    assert!(matches!(r.terms[0], TermVerdict::Undecided { .. }), "{r:?}");
    assert_eq!(r.worst, "undecided");
    let e = domain(&f, &[Some(1), None, None], &[&["n0-1"]]);
    let r = run(&f, &c, None, &e, "I(1,n1,n2)", &["I(2,n1,n2)"]);
    assert_eq!(r.domain, EmptyState::Empty);
    assert_eq!(r.worst, "descends");
    // unparsable / foreign-symbol patterns
    let r = run(&f, &c, None, &d, "I(n0,n1,n2)", &["I(n1,n0,n2)"]);
    assert!(matches!(r.terms[0], TermVerdict::Undecided { .. }));
}

#[test]
fn hand_down_excess_bound_is_exact_on_a_small_case() {
    let (f, c) = frame21();
    let d = domain(&f, &[None, None, None], &[]);
    let r = run(
        &f,
        &c,
        None,
        &d,
        "I(n0,n1,n2)",
        &["I(n0-1,n1+1,n2)", "I(n0,n1-1,n2-1)"],
    );
    // term 0 at n0 = 1: sub-sector excess n1 - n2, target n1 - 1 - n2 + 0: +1 (in chart: -1 + 1 = 0)
    assert_eq!(r.handdown_excess_bound, Some(1));
    assert!(!r.handdown_excess_unbounded);
}

#[test]
fn read_record_returns_target_rhs_and_line_sha() {
    let dir = std::env::temp_dir().join(format!("parametric_rr_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("t.jsonl");
    let l0 = r#"{"type":"header"}"#;
    let l1 = r#"{"type":"rule","index":0,"target":"I(n0,n1)","rhs":[["I(n0-1,n1)","2"],["I(n0,n1+1)","d"]]}"#;
    std::fs::write(&p, format!("{l0}\n{l1}\n")).unwrap();
    let off = (l0.len() + 1) as u64;
    let (t, rhs, sha) = read_record(&p, off, l1.len() as u64).unwrap();
    assert_eq!(t, "I(n0,n1)");
    assert_eq!(
        rhs,
        vec!["I(n0-1,n1)".to_string(), "I(n0,n1+1)".to_string()]
    );
    let (_, _, sha2) = read_record(&p, off, 0).unwrap();
    assert_eq!(sha, sha2);
    assert_eq!(sha.len(), 64);
    std::fs::remove_file(&p).unwrap();
    std::fs::remove_dir(&dir).unwrap();
}

// ---------------------------------------------------------------- seeded differential against brute force

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1) as u64) as i32
    }
}

fn pat(slots: &[Result<i32, i32>]) -> String {
    // Ok(c) = constant, Err(o) = n_s + o
    let s: Vec<String> = slots
        .iter()
        .enumerate()
        .map(|(i, x)| match x {
            Ok(c) => c.to_string(),
            Err(0) => format!("n{i}"),
            Err(o) if *o > 0 => format!("n{i}+{o}"),
            Err(o) => format!("n{i}{o}"),
        })
        .collect();
    format!("I({})", s.join(","))
}

/// Brute force over the box: per term, (in-chart key where the term is not lower, positive-numerator key,
/// hand-down key that is NOT lower, in-chart key where the term IS lower).
type Brute = (
    Option<Vec<i32>>,
    Option<Vec<i32>>,
    Option<Vec<i32>>,
    Option<Vec<i32>>,
);

fn brute(
    f: &NodeFrame,
    c: &NodeComparator,
    d: &CaseDomain,
    terms: &[Vec<Result<i32, i32>>],
    lmax: i32,
    nmin: i32,
) -> Vec<Brute> {
    let n = f.arity();
    let mut out = vec![(None, None, None, None); terms.len()];
    let mut key = vec![0i32; n];
    let lo: Vec<i32> = (0..n)
        .map(|s| if f.is_line(s) { 1 } else { nmin })
        .collect();
    let hi: Vec<i32> = (0..n)
        .map(|s| if f.is_line(s) { lmax } else { 0 })
        .collect();
    key.copy_from_slice(&lo);
    loop {
        if d.applies(&key).applies() {
            for (j, t) in terms.iter().enumerate() {
                let tk: Vec<i32> = t
                    .iter()
                    .enumerate()
                    .map(|(s, x)| match x {
                        Ok(c) => *c,
                        Err(o) => key[s] + o,
                    })
                    .collect();
                let pos = (0..n).any(|s| !f.is_line(s) && tk[s] >= 1);
                let drop = (0..n).any(|s| f.is_line(s) && tk[s] <= 0);
                let lower = c.cmp(&tk, &key) == Ordering::Less;
                let e = &mut out[j];
                if pos {
                    e.1.get_or_insert(key.clone());
                } else if drop {
                    if !lower {
                        e.2.get_or_insert(key.clone());
                    }
                } else if !lower {
                    e.0.get_or_insert(key.clone());
                } else {
                    e.3.get_or_insert(key.clone());
                }
            }
        }
        let mut s = 0;
        loop {
            if s == n {
                return out;
            }
            if key[s] < hi[s] {
                key[s] += 1;
                break;
            }
            key[s] = lo[s];
            s += 1;
        }
    }
}

fn differential(seed: u64, cases: usize, program: bool) {
    let mut rng = Rng(seed);
    let (nl, nn) = (2usize, 2usize);
    let n = nl + nn;
    let f = NodeFrame::without_header_lines_first(n, nl);
    let c = if program {
        NodeComparator::from_descriptor(rustred_order::OrderDescriptor::spired_uncut(n)).unwrap()
    } else {
        NodeComparator::legacy(n, None).unwrap()
    };
    let (mut seen_dnd, mut seen_except, mut seen_hd) = (0, 0, 0);
    for case in 0..cases {
        let fixed: Vec<Option<i32>> = (0..n)
            .map(|s| {
                (rng.below(4) == 0).then(|| {
                    if f.is_line(s) {
                        rng.range(1, 3)
                    } else {
                        rng.range(-2, 0)
                    }
                })
            })
            .collect();
        let mut exc: Vec<Vec<String>> = vec![];
        for _ in 0..rng.below(3) {
            let a = rng.below(n as u64) as usize;
            let b = rng.below(n as u64) as usize;
            let v = rng.range(-2, 3);
            exc.push(vec![if rng.below(3) == 0 && a != b {
                format!("n{a}-n{b}-({v})")
            } else {
                format!("n{a}-({v})")
            }]);
        }
        let exc_refs: Vec<Vec<&str>> = exc
            .iter()
            .map(|b| b.iter().map(String::as_str).collect())
            .collect();
        let exc_slices: Vec<&[&str]> = exc_refs.iter().map(Vec::as_slice).collect();
        let d = domain(&f, &fixed, &exc_slices);
        let tslots: Vec<Result<i32, i32>> = (0..n)
            .map(|s| match fixed[s] {
                Some(v) => Ok(v),
                None => Err(0),
            })
            .collect();
        let target = pat(&tslots);
        let terms: Vec<Vec<Result<i32, i32>>> = (0..1 + rng.below(4))
            .map(|_| {
                (0..n)
                    .map(|s| match fixed[s] {
                        Some(v) => Ok(v + rng.range(-1, 1)),
                        None => Err(rng.range(-2, 2)),
                    })
                    .collect()
            })
            .collect();
        let rhs: Vec<String> = terms.iter().map(|t| pat(t)).collect();
        let rhs_refs: Vec<&str> = rhs.iter().map(String::as_str).collect();
        let r = run(&f, &c, None, &d, &target, &rhs_refs);
        let b = brute(&f, &c, &d, &terms, 7, -6);
        // top_excess: at every in-chart key of the box the concrete D + r difference is the index-free value
        let tp = rustred_campaign_core::order::parse_pattern(&target).unwrap();
        let ranges = rustred_campaign_core::accept::descent::point_ranges(&d);
        let dr = |k: &[i32]| -> i64 {
            (0..n)
                .map(|s| {
                    if f.is_line(s) {
                        i64::from(k[s]) - 1
                    } else {
                        -i64::from(k[s])
                    }
                })
                .sum()
        };
        for (j, t) in terms.iter().enumerate() {
            let te =
                rustred_campaign_core::parametric::descent::top_excess(&tp, &rhs[j], &ranges, &f);
            for a0 in 1..=4 {
                for a1 in 1..=4 {
                    for a2 in -3..=0 {
                        for a3 in -3..=0 {
                            let key = vec![a0, a1, a2, a3];
                            if !d.applies(&key).applies() {
                                continue;
                            }
                            let tk: Vec<i32> = t
                                .iter()
                                .enumerate()
                                .map(|(s, x)| x.map_or_else(|o| key[s] + o, |c| c))
                                .collect();
                            let in_chart =
                                (0..n).all(|s| if f.is_line(s) { tk[s] >= 1 } else { tk[s] <= 0 });
                            if in_chart {
                                assert_eq!(
                                    te,
                                    Some(dr(&tk) - dr(&key)),
                                    "case {case} term {j} at {key:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
        for (j, v) in r.terms.iter().enumerate() {
            let (nl_key, pos_key, bad_hd, lower_key) = &b[j];
            assert!(
                bad_hd.is_none(),
                "case {case}: hand-down key {bad_hd:?} not lower (support-first violated?)"
            );
            match v {
                TermVerdict::Descends { hand_down, .. } => {
                    assert!(
                        nl_key.is_none() && pos_key.is_none(),
                        "case {case} term {j} {}: B1 says descends, brute force {nl_key:?} / {pos_key:?}; {target} {:?}",
                        rhs[j],
                        exc
                    );
                    seen_hd += usize::from(!hand_down.is_empty());
                }
                TermVerdict::DescendsExcept { loci, .. } => {
                    assert!(
                        nl_key.is_none(),
                        "case {case} term {j}: in-chart non-lower {nl_key:?}"
                    );
                    for l in loci {
                        if let Some(w) = &l.witness {
                            assert!(d.applies(w).applies());
                            let tk = match terms[j][l.slot] {
                                Ok(c) => c,
                                Err(o) => w[l.slot] + o,
                            };
                            assert!(
                                tk >= 1,
                                "case {case}: listed witness has no positive numerator"
                            );
                        }
                    }
                    seen_except += 1;
                }
                TermVerdict::DoesNotDescend { witness, .. } => {
                    assert!(d.applies(witness).applies());
                    let tk: Vec<i32> = terms[j]
                        .iter()
                        .enumerate()
                        .map(|(s, x)| x.map_or_else(|o| witness[s] + o, |c| c))
                        .collect();
                    assert_ne!(c.cmp(&tk, witness), Ordering::Less);
                    assert!(
                        lower_key.is_none(),
                        "case {case} term {j}: B1 says not lower in the chart, brute force finds {lower_key:?} lower"
                    );
                    seen_dnd += 1;
                }
                TermVerdict::Undecided { reason } => {
                    // allowed only for constant-vs-symbolic patterns in this generator (never generated) or budgets
                    panic!("case {case} term {j}: undecided {reason}");
                }
            }
        }
    }
    assert!(
        seen_dnd > 0 && seen_except > 0 && seen_hd > 0,
        "generator too narrow: {seen_dnd} {seen_except} {seen_hd}"
    );
}

#[test]
fn differential_against_brute_force_program_order() {
    differential(20_261_115, 300, true);
}

#[test]
fn differential_against_brute_force_legacy_order() {
    differential(0x5eed_0002, 300, false);
}

#[test]
fn zero_proofs_load_only_under_a_pinned_clean_verify_summary() {
    use rustred_campaign_core::closure::sha256::Sha256;
    let sha = |b: &[u8]| {
        let mut h = Sha256::new();
        h.update(b);
        h.finish_hex()
    };
    let dir = std::env::temp_dir().join(format!("parametric_zv_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let proof = dir.join("p.jsonl");
    let line = json!({"schema":"rustred.campaign.zero-proof.v1","record_sha256":SHA,"rule":0,"term":0,
        "locus":"domain","verdict":"zero_on_locus","denominator":"nowhere_zero","tool":"t@1"});
    std::fs::write(
        &proof,
        format!("{{\"schema\":\"rustred.campaign.zero-proof-header.v1\"}}\n{line}\n"),
    )
    .unwrap();
    let psha = sha(&std::fs::read(&proof).unwrap());
    let write_summary = |name: &str, v: serde_json::Value| -> (std::path::PathBuf, String) {
        let p = dir.join(name);
        let t = serde_json::to_string(&v).unwrap();
        std::fs::write(&p, &t).unwrap();
        (p, sha(t.as_bytes()))
    };
    let good = json!({"schema":"rustred.campaign.zero-proof-verify.v1","verified":true,"header_mismatch":[],
        "counts":{"mismatches":0,"claims_without_record":0},"proofs_sha256":[psha]});
    let (sp, ss) = write_summary("good.json", good.clone());
    let z = ZeroIndex::load_verified(&[proof.as_path()], &sp, &ss).unwrap();
    assert_eq!(z.proved_lines, 1);
    assert!(
        z.proof(
            SHA,
            0,
            rustred_campaign_core::import::zero_proofs::Locus::Domain
        )
        .is_some()
    );
    // wrong pin
    let e = ZeroIndex::load_verified(&[proof.as_path()], &sp, &"0".repeat(64)).unwrap_err();
    assert!(e.starts_with("zero_proofs_unverified"), "{e}");
    // the file is not listed
    let mut other = good.clone();
    other["proofs_sha256"] = json!(["ab".repeat(32)]);
    let (sp2, ss2) = write_summary("other.json", other);
    assert!(ZeroIndex::load_verified(&[proof.as_path()], &sp2, &ss2).is_err());
    // an unclean verification / no proofs_sha256
    let mut unclean = good.clone();
    unclean["counts"]["mismatches"] = json!(1);
    let (sp3, ss3) = write_summary("unclean.json", unclean);
    assert!(ZeroIndex::load_verified(&[proof.as_path()], &sp3, &ss3).is_err());
    let mut old = good;
    old.as_object_mut().unwrap().remove("proofs_sha256");
    let (sp4, ss4) = write_summary("old.json", old);
    let e = ZeroIndex::load_verified(&[proof.as_path()], &sp4, &ss4).unwrap_err();
    assert!(e.contains("proofs_sha256"), "{e}");
    for f in [
        "p.jsonl",
        "good.json",
        "other.json",
        "unclean.json",
        "old.json",
    ] {
        std::fs::remove_file(dir.join(f)).unwrap();
    }
    std::fs::remove_dir(&dir).unwrap();
}
