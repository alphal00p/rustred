//! The independent checker's FROZEN reference tree (lane k3, 2026-11-13) as the test oracle of this crate.
//!
//! Tree: `RUSTRED_K3_FIXTURES`, else `WS/fixtures/helper-k3-20261113` if it exists, else every test prints `SKIP`
//! and passes. Every file read is checked against the tree's own `SHA256SUMS` (pure-Rust SHA-256 in
//! `tests/k3_common`), so a changed tree fails loudly. Format, frames, counts and provenance: the tree's `README.md`.
//!
//! Check ids `V1..V69` are the 69 checks of the tree's `tools/verify.py`, in the order of its `verify.out`; each test
//! names the ids it covers. Three groups:
//! - frozen tree only (always run): counts and per-key verdicts the crate recomputes from the tree itself
//!   (box enumeration, dots/rank layers, the 8 unqualified rules' domains, the 5 empty domains, comparator verdicts);
//! - with the rule SOURCES (the tree holds verdicts, not rule domains): first-match dispatch on the tree's samples
//!   (always run when the sources exist; `RUSTRED_CAMPAIGN_FIXTURES` or the read-only handoff copy, sha256-checked)
//!   and on every key (`#[ignore]`, release: `cargo test -p rustred-campaign-core --release --test k3_oracle --
//!   --ignored`);
//! - placeholders `#[ignore = "needs module X"]` for partition, forward closure and span checks, naming the file and
//!   the expected numbers.
//!
//! Claims of the coordinator's side that the checker did NOT verify (README "Claims ..."): the 2 T162 terminals and
//! the 3 near-corner covers p2rows-c000#40/#41/#47 (F4), which 8 ids are unqualified, the T178B dispatch in cfg order,
//! R0 bodies = R0_try1 bodies, vanishing of numerator-raising terms. They are used as inputs where the README says
//! so and are never asserted here as checker facts.

mod inputs;
mod k3_common;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};

use k3_common::*;
use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::{CaseDomain, DomainOptions, TargetSource};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::poly::{ParseOptions, Poly};
use rustred_campaign_core::record::{ReadOptions, RecordReader, RuleRecord, TableLine};
use serde_json::{Value, json};

macro_rules! oracle_or_skip {
    () => {
        match oracle() {
            Some(o) => o,
            None => return,
        }
    };
}

/// The 8 unqualified T162 rule ids that must NOT be used (README F1; the list itself is the coordinator's export).
const UNQUALIFIED: [i64; 8] = [1603, 1739, 1740, 1845, 1851, 1856, 2102, 2442];

fn t162_frame() -> NodeFrame {
    NodeFrame::without_header_lines_first(20, 9)
}
fn t178_frame() -> NodeFrame {
    NodeFrame::without_header_lines_first(20, 10)
}

/// The caller's statement for the checker's negative-control records (no target): the checker evaluates their
/// domains (fixed + chart/recenter bounds + equations + exceptions) on the key itself.
const NEGCTL_TARGETS: TargetSource = TargetSource::CanonicalByConstruction {
    attestation: "k3 F1_negctl_unqualified8: the checker's domain definition, evaluated on the key",
};

fn export_opts() -> DomainOptions {
    DomainOptions {
        targets: T178B_EXPORT_TARGETS,
        ..Default::default()
    }
}

fn layer(frame: &NodeFrame, k: &[i32]) -> (i64, i64) {
    (
        frame.dots(k).expect("chart key"),
        frame.rank(k).expect("chart key"),
    )
}

/// `F1_box_all.jsonl.gz` rows: key -> smallest covering gid (-1 = uncovered).
fn f1_box_all(o: &Oracle) -> Vec<(Key, i64)> {
    o.text("T162/F1_box_all.jsonl.gz")
        .lines()
        .map(|l| serde_json::from_str::<(Key, i64)>(l).expect("F1_box_all row"))
        .collect()
}

/// `F5_closure_all.jsonl.gz` rows: (key, piece_index, case_i, in_box).
fn f5_closure_all(o: &Oracle) -> Vec<(Key, i64, i64, bool)> {
    o.text("T178B/F5_closure_all.jsonl.gz")
        .lines()
        .map(|l| serde_json::from_str::<(Key, i64, i64, bool)>(l).expect("F5_closure_all row"))
        .collect()
}

fn key_set(o: &Oracle, rel: &str) -> HashSet<Key> {
    o.jsonl(rel)
        .iter()
        .map(|v| as_key(if v.is_object() { &v["key"] } else { v }))
        .collect()
}

fn headers() -> Value {
    serde_json::from_str(include_str!("data/order/headers.json")).unwrap()
}

// ------------------------------------------------------------------------------------------------ integrity

/// V1, V2: every file listed in `SHA256SUMS` has its sha256 (pure-Rust SHA-256, checked against a known vector
/// first). V2 as written in verify.py ("SHA256SUMS lists exactly the files") assumes the tools sit OUTSIDE the tree;
/// in this frozen copy `tools/` was copied inside and is not listed, so the check here is: every file outside
/// `tools/` is listed, and the listed set is exactly 38 files.
#[test]
fn k3_sha256sums_every_file() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let o = oracle_or_skip!();
    assert_eq!(o.sums.len(), 38);
    for (_, f) in &o.sums {
        o.bytes(f); // asserts the hash
    }
    let mut present = vec![];
    let mut stack = vec![o.root.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                present.push(
                    p.strip_prefix(&o.root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    let listed: HashSet<&str> = o.sums.iter().map(|(_, f)| f.as_str()).collect();
    let unlisted: Vec<&String> = present
        .iter()
        .filter(|f| *f != "SHA256SUMS" && !f.starts_with("tools/") && !listed.contains(f.as_str()))
        .collect();
    assert!(
        unlisted.is_empty(),
        "files outside tools/ not covered by SHA256SUMS: {unlisted:?}"
    );
    let missing: Vec<&&str> = listed
        .iter()
        .filter(|f| !present.iter().any(|p| p == **f))
        .collect();
    assert!(missing.is_empty(), "listed but missing: {missing:?}");
}

// ------------------------------------------------------------------------------------------------ T162 F1

/// V3-V10, V22 (data part): box B enumerated by the crate (`NodeFrame::chart_layer`) = 55,704 keys = the rows of
/// `F1_box_all.jsonl.gz` (uncompressed sha256 = `F1_summary`); 990 rows with -1 = `F1_uncovered_990.jsonl`; 54,714
/// covered; no first_gid is unqualified; ids in 0..2447; uncovered per (D, r) layer (crate `dots`/`rank`) as in the
/// README and `F1_summary.by_layer_D_r`; the 2,000-key sample agrees with the full list.
#[test]
fn k3_t162_box_b_and_f1_lists() {
    let o = oracle_or_skip!();
    let frame = t162_frame();
    let boxb = t162_box_b();
    assert_eq!(boxb.len(), 55_704);
    assert_eq!(boxb.iter().collect::<HashSet<_>>().len(), 55_704);

    let raw = gunzip(&o.bytes("T162/F1_box_all.jsonl.gz"));
    let s1 = o.json("T162/F1_summary.json");
    assert_eq!(
        sha256_hex(&raw),
        s1["sha256_full_list_uncompressed"].as_str().unwrap()
    );
    let rows = f1_box_all(&o);
    assert_eq!(rows.len(), 55_704);
    let row_keys: Vec<Key> = rows.iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(
        row_keys, boxb,
        "F1_box_all rows (sorted by key) = crate box B"
    );

    let u = key_set(&o, "T162/F1_uncovered_990.jsonl");
    assert_eq!(u.len(), 990);
    assert_eq!(
        u,
        rows.iter()
            .filter(|(_, g)| *g < 0)
            .map(|(k, _)| k.clone())
            .collect()
    );
    assert_eq!(rows.iter().filter(|(_, g)| *g >= 0).count(), 54_714);
    assert!(rows.iter().all(|(_, g)| !UNQUALIFIED.contains(g)));
    assert!(rows.iter().all(|(_, g)| *g < 0 || (0..=2447).contains(g)));
    assert_eq!(s1["box_keys"], 55_704);
    assert_eq!(s1["uncovered"], 990);
    assert_eq!(s1["covered"], 54_714);

    let mut lu: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    let mut lb: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    for k in &boxb {
        *lb.entry(layer(&frame, k)).or_default() += 1;
    }
    for k in &u {
        *lu.entry(layer(&frame, k)).or_default() += 1;
    }
    let readme: BTreeMap<(i64, i64), usize> = [
        ((0, 0), 1),
        ((0, 1), 8),
        ((1, 0), 7),
        ((1, 1), 23),
        ((1, 2), 37),
        ((2, 0), 33),
        ((2, 1), 53),
        ((2, 2), 69),
        ((3, 0), 93),
        ((3, 1), 84),
        ((3, 2), 101),
        ((4, 0), 196),
        ((4, 1), 135),
        ((4, 2), 150),
    ]
    .into();
    assert_eq!(lu, readme);
    for (name, v) in s1["by_layer_D_r"].as_object().unwrap() {
        let (d, r) = name.split_once(',').unwrap();
        let l = (d.parse::<i64>().unwrap(), r.parse::<i64>().unwrap());
        assert_eq!(v[0], lb[&l], "box count of layer {l:?}");
        assert_eq!(v[1], lu[&l], "uncovered count of layer {l:?}");
    }

    let first: HashMap<&Key, i64> = rows.iter().map(|(k, g)| (k, *g)).collect();
    let samp = o.jsonl("T162/F1_covered_sample2000.jsonl");
    assert_eq!(samp.len(), 2000);
    for x in &samp {
        let g = x["first_gid"].as_i64().unwrap();
        assert!(g >= 0);
        assert_eq!(first[&as_key(&x["key"])], g);
    }
}

fn unqualified_tables(o: &Oracle) -> (Vec<Value>, Vec<(i64, CaseDomain)>) {
    let frame = t162_frame();
    let neg = o.jsonl("T162/F1_negctl_unqualified8.jsonl");
    let doms = neg
        .iter()
        .map(|v| {
            let rec = RuleRecord {
                chart: v["chart"].as_str().map(str::to_string),
                fixed: v["fixed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_i64())
                    .collect(),
                equations: serde_json::from_value(v["equations"].clone()).unwrap(),
                exceptions: serde_json::from_value(v["exceptions_OR_of_AND"].clone()).unwrap(),
                recenter: serde_json::from_value(v["recenter"].clone()).unwrap(),
                ..Default::default()
            };
            let opts = DomainOptions {
                recenter_bounds: true,
                targets: NEGCTL_TARGETS,
                ..Default::default()
            };
            (
                v["gid"].as_i64().unwrap(),
                CaseDomain::from_rule_record(&rec, &frame, opts).unwrap(),
            )
        })
        .collect();
    (neg, doms)
}

/// V11-V21 (negative control, frozen tree only): the 8 unqualified rules' domains (fixed + chart/recenter bounds +
/// equations + exceptions, from their records in the tree; poles not re-tested, as in verify.py) evaluated by the
/// crate on box B: per rule, the number of box keys in its domain = `box_keys_in_domain` (0, 5, 10, 1, 2, 20, 0, 15)
/// and the uncovered ones = `newly_covered_box_keys` key by key (1740: 3, 1845: 1, 1851: 2, 2442: 15); together 21
/// keys, all numerator-free, layers (2,0)1 (3,0)6 (4,0)14; 990 - 21 = 969 = `F1_uncovered_with_unqualified8.jsonl`.
/// An implementation that admitted the 8 would report 969, not 990.
#[test]
fn k3_t162_negctl_unqualified8_domains() {
    let o = oracle_or_skip!();
    let frame = t162_frame();
    let boxb = t162_box_b();
    let u = key_set(&o, "T162/F1_uncovered_990.jsonl");
    let (neg, doms) = unqualified_tables(&o);
    assert_eq!(
        neg.iter()
            .map(|x| x["gid"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        UNQUALIFIED
    );
    let mut newc: HashSet<Key> = HashSet::new();
    let mut per_rule = vec![];
    for (x, (gid, d)) in neg.iter().zip(&doms) {
        let dom: HashSet<&Key> = boxb.iter().filter(|k| d.applies(k).applies()).collect();
        let unc: HashSet<Key> = dom
            .iter()
            .filter(|k| u.contains(**k))
            .map(|k| (*k).clone())
            .collect();
        let want: HashSet<Key> = keys(&x["newly_covered_box_keys"]).into_iter().collect();
        assert_eq!(
            dom.len() as i64,
            x["box_keys_in_domain"].as_i64().unwrap(),
            "gid {gid}: box keys in domain"
        );
        assert_eq!(unc, want, "gid {gid}: newly covered keys");
        per_rule.push((*gid, dom.len(), unc.len()));
        newc.extend(unc);
    }
    println!("k3 F1 negctl (gid, box keys in domain, uncovered among them): {per_rule:?}");
    assert_eq!(
        per_rule.iter().map(|&(g, _, n)| (g, n)).collect::<Vec<_>>(),
        vec![
            (1603, 0),
            (1739, 0),
            (1740, 3),
            (1845, 1),
            (1851, 2),
            (1856, 0),
            (2102, 0),
            (2442, 15)
        ]
    );
    assert_eq!(newc.len(), 21);
    let mut lay: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    for k in &newc {
        *lay.entry(layer(&frame, k)).or_default() += 1;
    }
    assert_eq!(lay, [((2, 0), 1), ((3, 0), 6), ((4, 0), 14)].into());
    let u8: HashSet<Key> = key_set(&o, "T162/F1_uncovered_with_unqualified8.jsonl");
    assert_eq!(u8.len(), 969);
    assert_eq!(u8, u.difference(&newc).cloned().collect());
    assert_eq!(
        o.json("T162/F1_summary.json")["uncovered_with_unqualified8"],
        969
    );
}

// ------------------------------------------------------------------------------------------------ T162 F2

fn face_of(k: &[i32]) -> (Vec<usize>, Vec<usize>) {
    (
        (0..9).filter(|&i| k[i] >= 2).collect(),
        (9..20).filter(|&j| k[j] <= -1).collect(),
    )
}

fn cell_state(i: usize, v: i32) -> Value {
    if i < 9 {
        if v == 2 { json!(2) } else { json!(">=3") }
    } else if v >= -2 {
        json!(v)
    } else {
        json!("<=-3")
    }
}

type FaceIndex = HashMap<(Vec<usize>, Vec<usize>), Value>;

fn face_index(o: &Oracle) -> FaceIndex {
    o.jsonl("T162/F2_faces_226.jsonl")
        .into_iter()
        .map(|f| {
            let sd: Vec<usize> = serde_json::from_value(f["Sd"].clone()).unwrap();
            let sn: Vec<usize> = serde_json::from_value(f["Sn"].clone()).unwrap();
            ((sd, sn), f)
        })
        .collect()
}

fn in_generic_gap_cell(fidx: &FaceIndex, k: &[i32]) -> bool {
    let (sd, sn) = face_of(k);
    let Some(f) = fidx.get(&(sd.clone(), sn.clone())) else {
        return false;
    };
    let cell: Vec<Value> = sd.iter().chain(&sn).map(|&i| cell_state(i, k[i])).collect();
    f["generic_gap_cells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c.as_array().unwrap() == &cell)
}

/// V25-V30 as DATA checks (the crate has no partition module yet; the face/cell arithmetic is in this test, the
/// locus equations are evaluated with the crate's `Poly`): 226 faces = 217 generic + 9 locus-only; every one of the
/// 990 uncovered keys lies on a listed face; 972 box keys lie in a generic gap cell, 968 of them uncovered and 4
/// covered (= the 4 keys listed in `F2_summary.of_which_covered`); 37 locus points, all uncovered, containing the 22
/// non-generic gaps, 15 also in generic cells; every locus point satisfies its locus equation (crate `Poly::parse` of
/// "a = b", exact) and lies on its face; 14 loci, all on listed faces. Covered/uncovered is read from F1 here; the
/// crate's own dispatch reproduces it in `k3_t162_dispatch_samples` and `k3_t162_full_box_dispatch`.
#[test]
fn k3_t162_f2_faces_and_locus_points_data() {
    let o = oracle_or_skip!();
    let fidx = face_index(&o);
    assert_eq!(fidx.len(), 226);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for f in fidx.values() {
        *kinds
            .entry(f["kind"].as_str().unwrap().to_string())
            .or_default() += 1;
    }
    assert_eq!(
        kinds,
        [("generic".to_string(), 217), ("locus-only".to_string(), 9)].into()
    );
    let u = key_set(&o, "T162/F1_uncovered_990.jsonl");
    assert!(u.iter().all(|k| fidx.contains_key(&face_of(k))));
    let boxb = t162_box_b();
    let g: Vec<&Key> = boxb
        .iter()
        .filter(|k| in_generic_gap_cell(&fidx, k))
        .collect();
    let g_cov: HashSet<Key> = g
        .iter()
        .filter(|k| !u.contains(**k))
        .map(|k| (*k).clone())
        .collect();
    assert_eq!(g.len(), 972);
    assert_eq!(g.len() - g_cov.len(), 968);
    let s2 = o.json("T162/F2_summary.json");
    assert_eq!(g_cov, keys(&s2["of_which_covered"]).into_iter().collect());

    let lp = o.jsonl("T162/F2_locus_points_boxB.jsonl");
    let lpk: HashSet<Key> = lp.iter().map(|x| as_key(&x["key"])).collect();
    assert_eq!(lpk.len(), 37);
    assert!(lpk.is_subset(&u));
    assert!(
        u.iter()
            .filter(|k| !in_generic_gap_cell(&fidx, k))
            .all(|k| lpk.contains(k))
    );
    assert_eq!(
        u.iter().filter(|k| !in_generic_gap_cell(&fidx, k)).count(),
        22
    );
    assert_eq!(
        lpk.iter().filter(|k| in_generic_gap_cell(&fidx, k)).count(),
        15
    );
    for x in &lp {
        let k = as_key(&x["key"]);
        // the locus files write equations `a = b`: the explicit lenient parser option
        let eq = Poly::parse_with(
            x["locus"].as_str().unwrap(),
            20,
            ParseOptions {
                allow_d: false,
                allow_equation: true,
            },
        )
        .unwrap();
        assert!(
            eq.is_zero_at(&k).unwrap(),
            "locus equation {} at {k:?}",
            x["locus"]
        );
        let sd: Vec<usize> = serde_json::from_value(x["face"]["Sd"].clone()).unwrap();
        let sn: Vec<usize> = serde_json::from_value(x["face"]["Sn"].clone()).unwrap();
        assert_eq!(face_of(&k), (sd, sn));
        assert_eq!(x["uncovered"], true);
    }
    let l14 = o.json("T162/F2_loci_14.json");
    let l14 = l14.as_array().unwrap();
    assert_eq!(l14.len(), 14);
    for l in l14 {
        let sd: Vec<usize> = serde_json::from_value(l["face"]["Sd"].clone()).unwrap();
        let sn: Vec<usize> = serde_json::from_value(l["face"]["Sn"].clone()).unwrap();
        assert!(fidx.contains_key(&(sd, sn)));
    }
}

// ------------------------------------------------------------------------------------------------ T162 F3

/// V32-V35: the comparator the T162 bank header selects (legacy identity, presented frame; `headers.json`
/// `t162_bank`) says `lower(rhs, target)` exactly when `expect_lower` (= the checker's `lowerC` = its compare.rs port)
/// on all 1,430 pairs: 1,000 bank pairs lower, 400 swapped and 30 identical controls not lower; 600 bank pairs have a
/// top-sector RHS key (crate `in_chart_orthant`). V35 (the full O1 counts, 380,314 top + 1,163,047 sub lower) is
/// recomputed by `order_fixtures::order_o1_t162_bank_all_rules` (ignored, 65 MB input); here the tree's
/// `F3_summary` is asserted to carry exactly those numbers.
#[test]
fn k3_t162_f3_comparator_pairs() {
    let o = oracle_or_skip!();
    let c = NodeComparator::from_header(&headers()["t162_bank"]).unwrap();
    let frame = t162_frame();
    let p3 = o.jsonl("T162/F3_comparator_pairs.jsonl");
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let (mut lower, mut top_rhs) = (0, 0);
    for x in &p3 {
        let kind = x["kind"].as_str().unwrap().to_string();
        let (t, r) = (as_key(&x["target"]), as_key(&x["rhs"]));
        let want = x["expect_lower"].as_bool().unwrap();
        assert_eq!(c.lower(&r, &t), want, "{kind} {t:?} -> {r:?}");
        assert_eq!(x["lowerC"], want);
        assert_eq!(x["port_lower"], want);
        lower += want as usize;
        if kind == "bank_pair" {
            assert!(want);
            assert_eq!(
                frame.in_chart_orthant(&r),
                x["rhs_top_sector"].as_bool().unwrap()
            );
            top_rhs += frame.in_chart_orthant(&r) as usize;
        } else {
            assert!(!want);
        }
        *kinds.entry(kind).or_default() += 1;
    }
    assert_eq!(
        kinds,
        [
            ("bank_pair".to_string(), 1000),
            ("negctl_identical".to_string(), 30),
            ("negctl_swapped".to_string(), 400)
        ]
        .into()
    );
    assert_eq!((lower, top_rhs), (1000, 600));
    let s3 = o.json("T162/F3_summary.json");
    let a = &s3["all2448"];
    assert_eq!(
        (a["top"].as_i64(), a["top_lowerC"].as_i64()),
        (Some(380_314), Some(380_314))
    );
    assert_eq!(
        (a["sub"].as_i64(), a["sub_lowerC"].as_i64()),
        (Some(1_163_047), Some(1_163_047))
    );
    assert_eq!(
        (a["pts"].as_i64(), a["port_mismatch"].as_i64()),
        (Some(7341), Some(0))
    );
}

// ------------------------------------------------------------------------------------------------ T162 F4

/// V36-V39: 21 near-corner keys = corner + 11 single numerators (-1) + 9 single dots (2); 16 `verified_by_us`;
/// `box_gap` = membership in the F1 uncovered list (16 gaps, 5 bank-covered = `F4_summary.bank_covered_nearcorner`).
///
/// **[CLAIM] — not a checker fact:** the 2 terminals of `F4_terminals.json` (corner I(1^9,0^11) and spoke dot
/// I(2,1^8,0^11)) are the coordinator's PROVISIONAL terminal list. This test asserts only what the checker computed
/// about them: the list holds exactly these 2 keys and both are box-B gaps. It does NOT assert that they are
/// terminals; a closure test must adopt that claim explicitly. The 3 covers p2rows-c000#40/#41/#47 are also [CLAIM]
/// (`F4_summary.claimed_not_verified`) and are not used.
#[test]
fn k3_t162_f4_nearcorner_and_terminal_claim() {
    let o = oracle_or_skip!();
    let u = key_set(&o, "T162/F1_uncovered_990.jsonl");
    let nc = o.jsonl("T162/F4_nearcorner.jsonl");
    assert_eq!(nc.len(), 21);
    assert_eq!(nc.iter().filter(|x| x["claimed"] == "terminal").count(), 2);
    assert_eq!(
        nc.iter().filter(|x| x["verified_by_us"] == true).count(),
        16
    );
    for x in &nc {
        assert_eq!(
            x["box_gap"].as_bool().unwrap(),
            u.contains(&as_key(&x["key"])),
            "{}",
            x["key"]
        );
    }
    assert_eq!(nc.iter().filter(|x| x["box_gap"] == true).count(), 16);
    let corner: Key = [vec![1; 9], vec![0; 11]].concat();
    let mut want: HashSet<Key> = HashSet::from([corner.clone()]);
    for j in 9..20 {
        let mut k = corner.clone();
        k[j] = -1;
        want.insert(k);
    }
    for i in 0..9 {
        let mut k = corner.clone();
        k[i] = 2;
        want.insert(k);
    }
    assert_eq!(
        nc.iter().map(|x| as_key(&x["key"])).collect::<HashSet<_>>(),
        want
    );
    let s4 = o.json("T162/F4_summary.json");
    let covered: HashSet<Key> = nc
        .iter()
        .filter(|x| x["box_gap"] == false)
        .map(|x| as_key(&x["key"]))
        .collect();
    assert_eq!(
        covered,
        keys(&s4["bank_covered_nearcorner"]).into_iter().collect()
    );

    let tt = o.json("T162/F4_terminals.json");
    let tk: Vec<Key> = tt["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| as_key(&t["key"]))
        .collect();
    let mut spoke = corner.clone();
    spoke[0] = 2;
    assert_eq!(tk, vec![corner, spoke]);
    assert!(
        tk.iter().all(|k| u.contains(k)),
        "both claimed terminals are box-B gaps"
    );
    assert!(
        tt["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["claim_of"].as_str().unwrap().contains("provisional"))
    );
}

// ------------------------------------------------------------------------------------------------ T178B F5

/// V40-V45, V47, V48 (data part): the T178B box enumerated by the crate = 286,066 keys = the `in_box` rows of
/// `F5_closure_all.jsonl.gz` (in_box flags recomputed with the crate's `dots`/`rank`); 330,287 distinct closure keys,
/// all in the chart orthant; 753 rows with piece -1 = `F5_closure_uncovered_753.jsonl`; 409 of them in the box =
/// `F5_box_uncovered_409.jsonl`, 344 outside; per-piece box wins from the rows = `F5_summary.box_wins_per_piece`
/// (sum 285,657, README table); the 3,000-key first-match sample agrees with the rows.
#[test]
fn k3_t178b_box_and_closure_list_data() {
    let o = oracle_or_skip!();
    let frame = t178_frame();
    let in_box = |k: &[i32]| {
        let (d, r) = layer(&frame, k);
        (d == 0 && r <= 2) || ((1..=4).contains(&d) && r <= 3)
    };
    let boxk = t178_box();
    assert_eq!(boxk.len(), 286_066);
    let rows = f5_closure_all(&o);
    assert_eq!(rows.len(), 330_287);
    let ck: HashMap<&Key, (i64, i64, bool)> =
        rows.iter().map(|(k, p, i, b)| (k, (*p, *i, *b))).collect();
    assert_eq!(ck.len(), 330_287);
    assert!(rows.iter().all(|(k, _, _, _)| frame.in_chart_orthant(k)));
    assert!(rows.iter().all(|(k, _, _, b)| *b == in_box(k)));
    let box_rows: HashSet<&Key> = rows.iter().filter(|r| r.3).map(|r| &r.0).collect();
    assert_eq!(box_rows, boxk.iter().collect::<HashSet<_>>());

    let u753: HashSet<Key> = key_set(&o, "T178B/F5_closure_uncovered_753.jsonl");
    assert_eq!(u753.len(), 753);
    assert_eq!(
        u753,
        rows.iter()
            .filter(|r| r.1 < 0)
            .map(|r| r.0.clone())
            .collect()
    );
    for x in o.jsonl("T178B/F5_closure_uncovered_753.jsonl") {
        assert_eq!(x["in_box"].as_bool().unwrap(), in_box(&as_key(&x["key"])));
    }
    let b409 = key_set(&o, "T178B/F5_box_uncovered_409.jsonl");
    assert_eq!(b409.len(), 409);
    assert_eq!(b409, u753.iter().filter(|k| in_box(k)).cloned().collect());
    assert_eq!(u753.len() - b409.len(), 344);

    let fo = o.json("T178B/F5_frame_and_order.json");
    let pieces: Vec<String> = serde_json::from_value(fo["pieces_cfg_order"].clone()).unwrap();
    assert_eq!(pieces.len(), 38);
    let s5 = o.json("T178B/F5_summary.json");
    let mut wins = vec![0usize; 38];
    for (_, p, _, b) in &rows {
        if *b && *p >= 0 {
            wins[*p as usize] += 1;
        }
    }
    for (i, name) in pieces.iter().enumerate() {
        assert_eq!(s5["box_wins_per_piece"][name], wins[i], "wins of {name}");
    }
    assert_eq!(wins.iter().sum::<usize>(), 285_657);
    assert_eq!(
        (wins[36], wins[2], wins[37]),
        (248_858, 10_160, 0),
        "README: R0, rgq-g0-10, R0_try1"
    );
    assert_eq!(s5["closure_keys"], 330_287);
    assert_eq!(s5["closure_uncovered"], 753);

    let fm = o.jsonl("T178B/F5_box_firstmatch_sample3000.jsonl");
    assert_eq!(fm.len(), 3000);
    for x in &fm {
        let k = as_key(&x["key"]);
        let (p, i, b) = ck[&k];
        assert!(b);
        assert_eq!(
            (p, i),
            (
                x["piece_index"].as_i64().unwrap(),
                x["case_i"].as_i64().unwrap()
            )
        );
        if p >= 0 {
            assert_eq!(x["piece"].as_str().unwrap(), pieces[p as usize]);
        }
    }
}

/// V46, V49-V52 (comparator part): the comparator the T178 order-B table header selects (`headers.json`
/// `t178B_table`, node_order coordinate priority = `F5_frame_and_order.coordinate_priority`) says lower exactly when
/// `expect_lower_M1` (= recorded `lowerM1`) on all 1,470 pairs (1,000 closure edges, 250 swapped + 20 identical
/// controls, 200 priority discriminators); the identity-priority comparator (`spired_uncut(20)`) agrees with the
/// recorded `lower_identity_order` on every pair and says NOT lower on all 200 discriminators (an identity-order
/// implementation fails them). The 300 successor sets: every successor is a closure key, strictly lower than its
/// source under order B, and `uncovered_successors` = the successors that are among the 753 uncovered keys.
#[test]
fn k3_t178b_f5_comparator_and_successors() {
    let o = oracle_or_skip!();
    let h = headers();
    let c = NodeComparator::from_header(&h["t178B_table"]).unwrap();
    let fo = o.json("T178B/F5_frame_and_order.json");
    let prio: Vec<usize> = serde_json::from_value(fo["coordinate_priority"].clone()).unwrap();
    let hdr_prio: Vec<usize> =
        serde_json::from_value(h["t178B_table"]["node_order"]["coordinate_priority"].clone())
            .unwrap();
    assert_eq!(prio, hdr_prio);
    assert_eq!(
        prio,
        [
            6, 4, 9, 8, 5, 2, 1, 0, 7, 3, 11, 12, 10, 13, 15, 14, 16, 17, 18, 19
        ]
    );
    let ident =
        NodeComparator::from_descriptor(rustred_order::OrderDescriptor::spired_uncut(20)).unwrap();
    let p5 = o.jsonl("T178B/F5_comparator_pairs.jsonl");
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for x in &p5 {
        let kind = x["kind"].as_str().unwrap().to_string();
        let (t, r) = (as_key(&x["target"]), as_key(&x["rhs"]));
        let want = x["expect_lower_M1"].as_bool().unwrap();
        assert_eq!(c.lower(&r, &t), want, "{kind} {t:?} -> {r:?}");
        assert_eq!(x["lowerM1"], want);
        assert_eq!(
            ident.lower(&r, &t),
            x["lower_identity_order"].as_bool().unwrap(),
            "identity {kind} {t:?}"
        );
        match kind.as_str() {
            "closure_edge" => assert!(want),
            "priority_discriminator" => assert!(want && !ident.lower(&r, &t)),
            _ => assert!(!want),
        }
        *kinds.entry(kind).or_default() += 1;
    }
    assert_eq!(
        kinds,
        [
            ("closure_edge".to_string(), 1000),
            ("negctl_identical".to_string(), 20),
            ("negctl_swapped".to_string(), 250),
            ("priority_discriminator".to_string(), 200)
        ]
        .into()
    );

    let rows = f5_closure_all(&o);
    let ck: HashSet<&Key> = rows.iter().map(|r| &r.0).collect();
    let u753 = key_set(&o, "T178B/F5_closure_uncovered_753.jsonl");
    let su = o.jsonl("T178B/F5_closure_successors_sample300.jsonl");
    assert_eq!(su.len(), 300);
    let mut n_edges = 0;
    for x in &su {
        let k = as_key(&x["key"]);
        let succ = keys(&x["top_successors"]);
        assert_eq!(succ.len() as i64, x["n_top_successors"].as_i64().unwrap());
        for s in &succ {
            assert!(ck.contains(s));
            assert!(c.lower(s, &k), "successor {s:?} not lower than {k:?}");
            n_edges += 1;
        }
        let unc: HashSet<Key> = keys(&x["uncovered_successors"]).into_iter().collect();
        assert_eq!(
            unc,
            succ.iter().filter(|s| u753.contains(*s)).cloned().collect()
        );
    }
    println!("k3 F5: {n_edges} sampled successor edges, all strictly lower under order B");
}

/// V53: the 5 rules whose case has no domain point (pool1 rgq-g0-1 case 34, rgs-g0-6 case 45, rgs-g0-8 cases 27 and
/// 78, rgs-g1-15 case 121): their verbatim case records, read by the crate's `RecordReader` and turned into
/// `CaseDomain`s, apply at 0 points of the window (lines 1..12, numerators -12..0 on the free slots). Positive control:
/// with the exception branches removed every one of them has points in the same window (the exceptions are what
/// empties them, as the README's proof says).
#[test]
fn k3_t178b_empty_domain_rules() {
    let o = oracle_or_skip!();
    let em = o.json("T178B/F5_empty_domain_rules.json");
    let em = em.as_array().unwrap();
    assert_eq!(em.len(), 5);
    let mut found = vec![];
    for x in em {
        let line = serde_json::to_string(&x["case_record"]).unwrap();
        let mut rd = RecordReader::new(
            std::io::Cursor::new(line.into_bytes()),
            ReadOptions::default(),
        );
        let Some(Ok(TableLine::Rule(rec))) = rd.next() else {
            panic!("case record not read as a rule")
        };
        let frame = NodeFrame::without_header_from_mask(rec.chart.as_deref().unwrap()).unwrap();
        let d = CaseDomain::from_rule_record(&rec, &frame, export_opts()).unwrap();
        let free: Vec<usize> = (0..20).filter(|s| rec.fixed[*s].is_none()).collect();
        assert_eq!(
            free,
            serde_json::from_value::<Vec<usize>>(x["free_slots"].clone()).unwrap()
        );
        let mut no_exc_rec = (*rec).clone();
        no_exc_rec.exceptions.clear();
        let d0 = CaseDomain::from_rule_record(&no_exc_rec, &frame, export_opts()).unwrap();
        let (mut n, mut n0) = (0usize, 0usize);
        let mut idx = vec![0usize; free.len()];
        let ranges: Vec<Vec<i32>> = free
            .iter()
            .map(|&s| {
                if s < 10 {
                    (1..=12).collect()
                } else {
                    (-12..=0).collect()
                }
            })
            .collect();
        'outer: loop {
            let mut k: Key = rec.fixed.iter().map(|f| f.unwrap_or(0) as i32).collect();
            for (j, &s) in free.iter().enumerate() {
                k[s] = ranges[j][idx[j]];
            }
            n += d.applies(&k).applies() as usize;
            n0 += d0.applies(&k).applies() as usize;
            for j in 0..free.len() {
                idx[j] += 1;
                if idx[j] < ranges[j].len() {
                    continue 'outer;
                }
                idx[j] = 0;
            }
            break;
        }
        assert_eq!(
            n, 0,
            "{} case {}: domain not empty",
            x["piece"], x["case_i"]
        );
        assert!(
            n0 > 0,
            "{} case {}: positive control found no point",
            x["piece"],
            x["case_i"]
        );
        found.push(n0);
    }
    assert_eq!(
        o.json("T178B/F5_summary.json")["empty_domain_points_found"],
        json!([0, 0, 0, 0, 0])
    );
    println!(
        "k3 F5 empty domains: 0 points each; without exceptions {found:?} points in the window"
    );
}

// ------------------------------------------------------------------------------------------------ span (F6) data

/// V55-V69 (data and integrity only; the span check itself is the placeholder `k3_f6_span_verdicts`): 11 span cases;
/// every clean rule C1 pass with census verdicts covered_regular / excepted / not_in_case; every corrupted control C1
/// fail with no covered_regular point; 58 covered_regular census points of rule 0 clean = 58 span_fail in the
/// controls; the 6 inline records have the recorded line sha256; the 5 records over 60 kB are read from the handoff
/// copy by byte offset and length and have the recorded line sha256 (skipped with a message if the copy is absent).
#[test]
fn k3_f6_span_cases_data_and_record_hashes() {
    let o = oracle_or_skip!();
    let sp = o.jsonl("span/F6_span_cases.jsonl");
    assert_eq!(sp.len(), 11);
    assert_eq!(
        sp.iter()
            .map(|x| x["id"].as_str().unwrap())
            .collect::<HashSet<_>>()
            .len(),
        11
    );
    let pts = |r: &Value| -> Vec<Value> {
        let mut v = r["C2"].as_array().cloned().unwrap_or_default();
        v.extend(r["C5"].as_array().cloned().unwrap_or_default());
        v
    };
    let good = ["covered_regular", "excepted", "not_in_case"];
    let (mut ncov, mut nfail, mut inline, mut source, mut skipped) = (0, 0, 0, 0, 0);
    for x in &sp {
        for r in x["clean"].as_array().unwrap() {
            assert_eq!(r["status"], "done");
            assert_eq!(r["C1"]["verdict"], "pass");
            assert!(
                pts(r)
                    .iter()
                    .all(|p| good.contains(&p["verdict"].as_str().unwrap()))
            );
            assert!(r["C1_int"].is_null() || r["C1_int"]["verdict"] == "covered_regular");
        }
        let ctl = &x["control"]["result"];
        assert_eq!(ctl["C1"]["verdict"], "fail");
        assert!(pts(ctl).iter().all(|p| p["verdict"] != "covered_regular"));
        ncov += pts(&x["clean"][0])
            .iter()
            .filter(|p| p["verdict"] == "covered_regular")
            .count();
        nfail += pts(ctl)
            .iter()
            .filter(|p| p["verdict"] == "span_fail")
            .count();
        let want = x["line_sha256"].as_str().unwrap();
        if let Some(rel) = x["inline_record"].as_str() {
            assert_eq!(
                sha256_hex(&o.bytes(&format!("span/{rel}"))),
                want,
                "{} inline record",
                x["id"]
            );
            inline += 1;
        } else {
            let p = std::path::Path::new(HO).join(x["file"].as_str().unwrap());
            match std::fs::File::open(&p) {
                Ok(mut f) => {
                    f.seek(SeekFrom::Start(x["byte_offset"].as_u64().unwrap()))
                        .unwrap();
                    let mut buf = vec![0u8; x["byte_len"].as_u64().unwrap() as usize];
                    f.read_exact(&mut buf).unwrap();
                    assert_eq!(sha256_hex(&buf), want, "{} source line at offset", x["id"]);
                    source += 1;
                }
                Err(e) => {
                    inputs::missing(format_args!(
                        "SKIP {}: {} unreadable ({e})",
                        x["id"],
                        p.display()
                    ));
                    skipped += 1;
                }
            }
        }
    }
    assert_eq!((ncov, nfail), (58, 58));
    assert_eq!(inline, 6);
    println!(
        "k3 F6: 11 cases, 58/58 census points, {inline} inline + {source} source record hashes ({skipped} skipped)"
    );
}

// ------------------------------------------------------------------------------------------------ dispatch (samples)

/// V22, V37 (crate part), V27 (the 4 covered generic-cell keys), V28 (locus points uncovered): first-match dispatch
/// of the 2,440 qualified bank rules in ascending id (= smallest covering id; `first_gid` is a reference index, NOT a
/// dispatch claim of the engine) on the tree's samples: the 2,000 covered sample keys get exactly their `first_gid`;
/// the 990 uncovered keys are uncovered (none undecided); the 21 near-corner keys get their `bank_first_gid` (-1 for
/// the 16 box gaps); the 4 keys of `F2_summary.of_which_covered` are covered; the 37 locus points are uncovered.
/// Rule source: see `k3_common::t162_bank_table` (skips with a message if absent).
#[test]
fn k3_t162_dispatch_samples() {
    let o = oracle_or_skip!();
    let Some(table) = t162_bank_table() else {
        return;
    };
    let disp = Dispatcher::new(vec![table], vec![0]).unwrap();
    let gid = |k: &[i32]| match disp.first_match(k).unwrap() {
        Dispatch::Covered(r) => r.label,
        Dispatch::Uncovered => -1,
        d => panic!("open but not uncovered at {k:?}: {d:?}"),
    };
    for x in o.jsonl("T162/F1_covered_sample2000.jsonl") {
        assert_eq!(
            gid(&as_key(&x["key"])),
            x["first_gid"].as_i64().unwrap(),
            "{}",
            x["key"]
        );
    }
    let u = key_set(&o, "T162/F1_uncovered_990.jsonl");
    assert!(u.iter().all(|k| gid(k) == -1));
    for x in o.jsonl("T162/F4_nearcorner.jsonl") {
        let k = as_key(&x["key"]);
        assert_eq!(
            gid(&k),
            x["bank_first_gid"].as_i64().unwrap(),
            "near-corner {k:?}"
        );
        assert_eq!(gid(&k) < 0, x["box_gap"].as_bool().unwrap());
    }
    for k in keys(&o.json("T162/F2_summary.json")["of_which_covered"]) {
        assert!(gid(&k) >= 0, "{k:?} should be covered");
    }
    for x in o.jsonl("T162/F2_locus_points_boxB.jsonl") {
        assert_eq!(gid(&as_key(&x["key"])), -1);
    }
}

fn t178_dispatcher(o: &Oracle) -> Option<(Vec<String>, Dispatcher)> {
    let (names, tables, frame) = t178_tables()?;
    assert_eq!(frame.n_lines(), 10);
    let fo = o.json("T178B/F5_frame_and_order.json");
    let pieces: Vec<String> = serde_json::from_value(fo["pieces_cfg_order"].clone()).unwrap();
    assert_eq!(names, pieces, "case-export cfg order = k3 piece order");
    Some((names, Dispatcher::new(tables, (0..38).collect()).unwrap()))
}

fn winner(d: &Dispatcher, k: &[i32]) -> (i64, i64) {
    match d.first_match(k).unwrap() {
        Dispatch::Covered(r) => (r.table as i64, r.label),
        Dispatch::Uncovered => (-1, -1),
        x => panic!("open but not uncovered at {k:?}: {x:?}"),
    }
}

/// V48, V44-V45 (crate part), V52 (winners of the successor-sample keys), V53 (records): first match over the 38
/// order-B pieces in cfg order (the order itself is a [CLAIM] of the coordinator side, used as input) on the tree's
/// samples: the 3,000 box sample keys get exactly (piece_index, case_i); the 753 closure-uncovered keys (409 box +
/// 344 outside) are uncovered; each of the 300 successor-sample keys gets its (piece, case_i); the 5 empty-domain
/// records of the tree are the records at (piece_index, case_i) of the case exports (same canonical domain). Rule
/// source: see `k3_common::t178_tables` (skips with a message if absent).
#[test]
fn k3_t178b_dispatch_samples() {
    let o = oracle_or_skip!();
    let Some((names, disp)) = t178_dispatcher(&o) else {
        return;
    };
    for x in o.jsonl("T178B/F5_box_firstmatch_sample3000.jsonl") {
        let k = as_key(&x["key"]);
        assert_eq!(
            winner(&disp, &k),
            (
                x["piece_index"].as_i64().unwrap(),
                x["case_i"].as_i64().unwrap()
            ),
            "{k:?}"
        );
    }
    for x in o.jsonl("T178B/F5_closure_uncovered_753.jsonl") {
        assert_eq!(winner(&disp, &as_key(&x["key"])), (-1, -1));
    }
    for x in o.jsonl("T178B/F5_closure_successors_sample300.jsonl") {
        let (p, i) = winner(&disp, &as_key(&x["key"]));
        assert!(p >= 0);
        assert_eq!(
            (names[p as usize].as_str(), i),
            (x["piece"].as_str().unwrap(), x["case_i"].as_i64().unwrap())
        );
    }
    for x in o
        .json("T178B/F5_empty_domain_rules.json")
        .as_array()
        .unwrap()
    {
        let t: &Table = &disp.tables()[x["piece_index"].as_u64().unwrap() as usize];
        let pos = t
            .labels
            .iter()
            .position(|&l| l == x["case_i"].as_i64().unwrap())
            .expect("record in the export");
        let line = serde_json::to_string(&x["case_record"]).unwrap();
        let mut rd = RecordReader::new(
            std::io::Cursor::new(line.into_bytes()),
            ReadOptions::default(),
        );
        let Some(Ok(TableLine::Rule(rec))) = rd.next() else {
            panic!()
        };
        let d = CaseDomain::from_rule_record(&rec, &t178_frame(), export_opts()).unwrap();
        assert_eq!(t.domains[pos].canonical_form(), d.canonical_form());
    }
}

// ------------------------------------------------------------------------------------------------ dispatch (every key)

/// V6, V7, V10, V11, V23, V27, V37 on EVERY key (release recommended): first match of the 2,440 qualified rules on
/// all 55,704 box-B keys = `F1_box_all` first_gid key by key; 54,714 covered / 990 uncovered = `F1_uncovered_990`;
/// uncovered per layer as in the README; adding the 8 unqualified rules leaves 969 = the file (negative control:
/// the test asserts 990 without them); 972 generic-gap-cell keys of which the crate covers exactly the 4 listed;
/// near-corner `box_gap` = crate-uncovered.
#[test]
#[ignore = "full box B dispatch (55,704 keys); run with --release -- --ignored"]
fn k3_t162_full_box_dispatch() {
    let o = oracle_or_skip!();
    let Some(table) = t162_bank_table() else {
        return;
    };
    let frame = t162_frame();
    let rows = f1_box_all(&o);
    let boxb: Vec<Key> = rows.iter().map(|r| r.0.clone()).collect();
    assert_eq!(boxb, t162_box_b());
    let disp = Dispatcher::new(vec![table.clone()], vec![0]).unwrap();
    let rep = disp.batch(&boxb, 4).unwrap();
    println!(
        "k3 T162 full: covered {} uncovered {} blocked {} undecided {} outside chart {} in {:.2}s",
        rep.covered, rep.uncovered, rep.blocked, rep.undecided, rep.outside_chart, rep.seconds
    );
    assert_eq!(
        (
            rep.covered,
            rep.uncovered,
            rep.blocked,
            rep.undecided,
            rep.outside_chart
        ),
        (54_714, 990, 0, 0, 0)
    );
    let mut mism = 0;
    for ((_, g), r) in rows.iter().zip(&rep.results) {
        mism += (r.winner().map_or(-1, |w| w.label) != *g) as usize;
    }
    assert_eq!(mism, 0, "first_gid mismatches");
    let mine: HashSet<Key> = boxb
        .iter()
        .zip(&rep.results)
        .filter(|(_, r)| r.winner().is_none())
        .map(|(k, _)| k.clone())
        .collect();
    assert_eq!(mine, key_set(&o, "T162/F1_uncovered_990.jsonl"));
    let mut lu: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    for k in &mine {
        *lu.entry(layer(&frame, k)).or_default() += 1;
    }
    assert_eq!(lu[&(4, 0)], 196);
    assert_eq!(lu[&(3, 2)], 101);
    assert_eq!(lu.values().sum::<usize>(), 990);

    let (_, doms) = unqualified_tables(&o);
    let mut bad = Table::new("T162-unqualified8", 20);
    for (g, d) in doms {
        bad.push(g, d).unwrap();
    }
    let disp8 = Dispatcher::new(vec![table, bad], vec![0, 1]).unwrap();
    let rep8 = disp8.batch(&boxb, 4).unwrap();
    println!(
        "k3 T162 with the 8: covered {} uncovered {} blocked {} undecided {}",
        rep8.covered, rep8.uncovered, rep8.blocked, rep8.undecided
    );
    let mine8: HashSet<Key> = boxb
        .iter()
        .zip(&rep8.results)
        .filter(|(_, r)| r.winner().is_none())
        .map(|(k, _)| k.clone())
        .collect();
    assert_eq!((rep8.open(), rep8.wins[1]), (969, 21));
    assert_eq!(
        mine8,
        key_set(&o, "T162/F1_uncovered_with_unqualified8.jsonl")
    );

    let fidx = face_index(&o);
    let gcov: HashSet<Key> = boxb
        .iter()
        .zip(&rep.results)
        .filter(|(k, r)| in_generic_gap_cell(&fidx, k) && r.winner().is_some())
        .map(|(k, _)| k.clone())
        .collect();
    assert_eq!(
        boxb.iter()
            .filter(|k| in_generic_gap_cell(&fidx, k))
            .count(),
        972
    );
    assert_eq!(
        gcov,
        keys(&o.json("T162/F2_summary.json")["of_which_covered"])
            .into_iter()
            .collect()
    );
    for x in o.jsonl("T162/F4_nearcorner.jsonl") {
        assert_eq!(
            x["box_gap"].as_bool().unwrap(),
            mine.contains(&as_key(&x["key"]))
        );
    }
}

/// V44, V45, V47 and the first-match half of the F5 closure on EVERY closure key (release recommended): first match
/// over the 38 pieces on all 330,287 closure keys = (piece_index, case_i) of `F5_closure_all` row by row (box keys and
/// the 44,221 outside keys); 753 uncovered, 409 in the box; per-piece box wins = `F5_summary.box_wins_per_piece`.
/// Generating the closure itself (successors from the RHS keys) is the placeholder `k3_f5_forward_closure`.
#[test]
#[ignore = "full T178B first match on 330,287 closure keys; run with --release -- --ignored"]
fn k3_t178b_full_closure_first_match() {
    let o = oracle_or_skip!();
    let Some((names, disp)) = t178_dispatcher(&o) else {
        return;
    };
    let rows = f5_closure_all(&o);
    let ks: Vec<Key> = rows.iter().map(|r| r.0.clone()).collect();
    let rep = disp.batch(&ks, 4).unwrap();
    println!(
        "k3 T178B closure first match: covered {} uncovered {} blocked {} undecided {} outside chart {} in {:.2}s",
        rep.covered, rep.uncovered, rep.blocked, rep.undecided, rep.outside_chart, rep.seconds
    );
    assert_eq!(
        (rep.uncovered, rep.blocked, rep.undecided, rep.outside_chart),
        (753, 0, 0, 0)
    );
    let mut mism = 0;
    let mut wins = vec![0usize; 38];
    let mut box_unc = 0;
    for ((_, p, i, b), r) in rows.iter().zip(&rep.results) {
        let w = r.winner().map_or((-1, -1), |w| (w.table as i64, w.label));
        mism += (w != (*p, *i)) as usize;
        if *b {
            match r.winner() {
                Some(w) => wins[w.table] += 1,
                None => box_unc += 1,
            }
        }
    }
    assert_eq!(mism, 0, "winner mismatches vs F5_closure_all");
    assert_eq!(box_unc, 409);
    let s5 = o.json("T178B/F5_summary.json");
    for (i, n) in names.iter().enumerate() {
        assert_eq!(s5["box_wins_per_piece"][n], wins[i], "wins of {n}");
    }
}

// ------------------------------------------------------------------------------------------------ placeholders

/// A placeholder checks nothing: it prints what a later module must reproduce and passes, so that `-- --ignored`
/// runs of the implemented full-data tests stay meaningful. Its `#[ignore]` reason names the missing module.
fn placeholder(what: &str) {
    println!("PLACEHOLDER (nothing checked yet): {what}");
}

/// Placeholder (F2 assertion 1): the partition of the 2,440 qualified domains into gap faces must give exactly the 226
/// faces of `T162/F2_faces_226.jsonl` (217 generic + 9 locus-only) with the same `kind`, `T_line`, `T_num`,
/// `unbounded_in_dots/rank`, `generic_gap_cells` and `locus_gaps`; the 14 coupled loci of `T162/F2_loci_14.json`;
/// on box B: 972 keys in generic gap cells (968 uncovered, 4 covered), 22 further uncovered keys on loci (968 + 22 =
/// 990), 178 faces with an uncovered box key (`F2_summary.faces_with_uncovered_box_key`). Completeness of the locus
/// list is TESTED by the checker, not proved (README).
#[test]
#[ignore = "needs module partition (gap faces from case domains)"]
fn k3_f2_partition_reproduces_226_faces() {
    placeholder(
        "partition module: compare with T162/F2_faces_226.jsonl, F2_loci_14.json, F2_summary.json",
    );
}

/// Placeholder (F5 assertion 3): forward closure of the T178B box under first match (top-sector RHS keys followed,
/// subsector keys and keys with a numerator > 0 not followed; RHS keys from `HO/t178-orderB/keys/*.keys.jsonl`, rule
/// key i = case record i + 2; R0 bodies = R0_try1 bodies [checker's identification]): 330,287 keys (286,066 box +
/// 44,221 outside), 753 uncovered (409 + 344), 4,387,847 distinct top-sector edges, 0 not lower under order B,
/// acyclic, 1,144,837 edges NOT lower under the identity order, 0 edges from uncovered sources; the successor sets of
/// the 300 keys of `T178B/F5_closure_successors_sample300.jsonl` match; the key set = `F5_closure_all.jsonl.gz`.
#[test]
#[ignore = "needs module closure (forward closure with RHS keys)"]
fn k3_f5_forward_closure() {
    placeholder(
        "closure module: compare with T178B/F5_closure_all.jsonl.gz, F5_closure_successors_sample300.jsonl, F5_summary.json",
    );
}

/// Placeholder (F6): the span/acceptance check on the 11 cases of `span/F6_span_cases.jsonl` (records inline in
/// `span/records/` or by offset in `HO/<file>`): C1 pass for every clean rule at both primes 2^61-1 and 2^31-1; per
/// census key the listed verdict (f111 covered_regular 1; f046 2; f000 3 + excepted 1; fs023 9; fs034 27; fp008 1 +
/// not_in_case 7; fa001 none; f1003 1; fd006 12 + excepted 18; bp010 1; bp002 1; C1_int covered_regular for the
/// point rules fp008, fa001, f1003, bp010, bp002); every corrupted control (`*.corrupt.json`, first RHS coefficient
/// numerator doubled) C1 FAIL and its 58 previously covered_regular keys span_fail.
#[test]
#[ignore = "needs module span (IBP span / acceptance check)"]
fn k3_f6_span_verdicts() {
    placeholder("span module: compare with span/F6_span_cases.jsonl");
}

/// Placeholder (F4): the span check must accept the 16 `verified_by_us` near-corner covers of
/// `T162/F4_nearcorner.jsonl` with the named rule (`rule_file`, `rule_line0`, `sym_rows`) at both primes, every
/// top-sector RHS key lower under the T162 comparator. The 3 covers p2rows-c000#40/#41/#47 are [CLAIM] only and the 2
/// terminals are the coordinator's provisional list ([CLAIM]); neither may be asserted as a checker fact.
#[test]
#[ignore = "needs module span (acceptance of the 16 verified near-corner covers)"]
fn k3_f4_nearcorner_span_16_verified() {
    placeholder("span module: compare with T162/F4_nearcorner.jsonl verified_by_us entries");
}
