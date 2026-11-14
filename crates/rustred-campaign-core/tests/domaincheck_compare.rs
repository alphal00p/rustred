//! T-DOMAIN (plan 4.1 WP-I1b, engine helper EC-15): the core's `Dispatcher::first_match` against the ENGINE's own
//! matcher (`rr_node domaincheck`, schema `rustred.domaincheck.v1`, interfaces/domaincheck.md), key by key.
//!
//! Every difference must fall into a listed class of interfaces/campaign-core-domain.md section 3 (counted and
//! asserted per class below); anything else is a FINDING (asserted zero).
//!
//! Three steps (all `#[ignore]`):
//!
//! ```text
//! export RUSTRED_CAMPAIGN_FIXTURES=/common/build-cache/$USER/p_integral_dev/implementer-scratch/fixtures
//! export DOMAINCHECK_DIR=/common/build-cache/$USER/p_integral_dev/implementer-scratch/eng/dc   # default
//! cargo test -p rustred-campaign-core --release --test domaincheck_compare -- --ignored prepare   # 1. inputs
//! rr_node domaincheck ... (see DOMAINCHECK_DIR/RUN.txt written by step 1)                         # 2. engine
//! cargo test -p rustred-campaign-core --release --test domaincheck_compare -- --ignored t_domain  # 3. compare
//! ```
//!
//! Inputs (read-only): `$RUSTRED_CAMPAIGN_FIXTURES/T162/RULE-PREDICATES2440.json` (the bank predicates the core's
//! T1 uses), Astra's bank export (`T162_BANK_EXPORT`, default below: native rule bodies + per-origin headers), the 38
//! raw T178 order-B piece tables `$RUSTRED_CAMPAIGN_FIXTURES/eng/T178B_raw/*.F.table.jsonl` (copies of
//! WS/SCRATCHPAD/t178/regenB/**/node/F.table.jsonl; R0 = R0_try1 by h13b's identification; SHA256SUMS beside them)
//! and the case exports `T178B/cases` (to tie each raw table to the fixture the core's T2 uses).
//!
//! T162 core side (int4): the coordinator's CONVERTED bank, interfaces/coord-bank-tables.md v1.2 (set 20261114c:
//! per-origin `table_dump.v1` tables, `index` = SET-UNIQUE label = global id, plus `origin`, `origin_index`), variant
//! `qualified`, read-only through `$RUSTRED_CAMPAIGN_FIXTURES/T162/bank-conv-20261114c` (a link to the converter's
//! output directory; `T162_BANK_TABLES` overrides it); its SHA256SUMS is checked against the interface's digest and
//! every table against SHA256SUMS. (The step-1 matcher table repeats origin-local labels: the core's loader refuses it
//! with `DuplicateLabel`; it stays the ENGINE's input of step 2 only.)

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rustred_campaign_core::accept::rule::sha256_tag;
use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table, WitnessStatus};
use rustred_campaign_core::domain::{DomainOptions, PoleKind, TargetSource, Verdict};
use rustred_campaign_core::key::{Key, NodeFrame, parse_key};
use rustred_campaign_core::record::{
    ReadOptions, RhsMode, TableFile, read_bank_predicates, read_table_file,
};
use serde_json::Value;

const T162_BANK_EXPORT: &str = "/shared/vhirschi/t3code/astra_5L/m5-rule-search/wave1945/wave5/T162-BANK2448-EXPORT-20261113T07";

fn fixtures() -> PathBuf {
    PathBuf::from(
        std::env::var("RUSTRED_CAMPAIGN_FIXTURES").expect("set RUSTRED_CAMPAIGN_FIXTURES"),
    )
}

/// sha256 of the converted set's SHA256SUMS file (interfaces/coord-bank-tables.md v1.2, "Paths").
const BANK_TABLES_SUMS_SHA256: &str =
    "sha256:fbaa6454dc87a4ac4b2ed13067d75ea8183a01018078d455dbf0ad3f8b5aef12";

/// The converted T162 bank set (read-only): `T162_BANK_TABLES`, default `$RUSTRED_CAMPAIGN_FIXTURES/T162/bank-conv-20261114c`.
fn bank_tables_dir() -> PathBuf {
    std::env::var("T162_BANK_TABLES").map_or_else(
        |_| fixtures().join("T162/bank-conv-20261114c"),
        PathBuf::from,
    )
}

/// The converted QUALIFIED tables as ONE core table in first-match order (tables ordered by their first label = global
/// id, file order inside; the labels must then increase strictly), each file checked against the set's SHA256SUMS,
/// which is checked against the interface's digest; all headers agree on the fields the loader reads. Returns the
/// table (labels = global ids, unique in the set) and the labels.
fn bank_tables(opts: DomainOptions) -> (Table, Vec<i64>) {
    let dir = bank_tables_dir();
    let sums_text = std::fs::read(dir.join("SHA256SUMS")).unwrap();
    assert_eq!(
        sha256_tag(&sums_text),
        BANK_TABLES_SUMS_SHA256,
        "SHA256SUMS of the set"
    );
    let sums: BTreeMap<String, String> = String::from_utf8(sums_text)
        .unwrap()
        .lines()
        .map(|l| {
            let (h, f) = l.split_once("  ").unwrap();
            (f.to_string(), format!("sha256:{h}"))
        })
        .collect();
    let mut parts: Vec<(i64, TableFile)> = vec![];
    for (f, want) in sums.iter().filter(|(f, _)| f.starts_with("qualified/")) {
        let path = dir.join(f);
        assert_eq!(&sha256_tag(&std::fs::read(&path).unwrap()), want, "{f}");
        let tf = read_table_file(&path, ReadOptions { rhs: RhsMode::Skip }).unwrap();
        let first = tf.rules.first().and_then(|r| r.index).unwrap();
        parts.push((first, tf));
    }
    parts.sort_by_key(|(first, _)| *first);
    let mut all = TableFile::default();
    let key_fields = |tf: &TableFile| -> Value {
        let h = &tf.header.as_ref().unwrap().raw;
        serde_json::json!([
            h["denominators"],
            h["mask"],
            h["power_shifts"],
            h["node_order"]["slot_permutation"]
        ])
    };
    for (i, (_, tf)) in parts.into_iter().enumerate() {
        if i == 0 {
            all.header = tf.header.clone();
            all.charts = tf.charts.clone();
        } else {
            assert_eq!(key_fields(&tf), key_fields(&all), "table {i}: header");
            assert_eq!(tf.charts, all.charts, "table {i}: chart lines");
        }
        all.rules.extend(tf.rules);
    }
    let labels: Vec<i64> = all.rules.iter().map(|r| r.index.unwrap()).collect();
    assert!(
        labels.windows(2).all(|w| w[0] < w[1]),
        "labels in first-match order, unique"
    );
    let table =
        Table::from_table_file("T162-bank-tables", dir.display().to_string(), &all, opts).unwrap();
    assert_eq!(table.len(), labels.len(), "every bank record is dispatched");
    (table, labels)
}

fn dc_dir() -> PathBuf {
    std::env::var("DOMAINCHECK_DIR").map_or_else(
        |_| {
            let user = std::env::var("USER").unwrap_or_else(|_| "vhirschi".into());
            PathBuf::from(format!(
                "/common/build-cache/{user}/p_integral_dev/implementer-scratch/eng/dc"
            ))
        },
        PathBuf::from,
    )
}

fn box_keys(
    frame: &NodeFrame,
    max_r: impl Fn(u32) -> u32,
    max_line_power: Option<i32>,
) -> Vec<Key> {
    let mut k = vec![];
    for d in 0..=4 {
        for r in 0..=max_r(d) {
            k.extend(frame.chart_layer(d, r, max_line_power));
        }
    }
    k.sort();
    k
}

fn write_keys(path: &Path, keys: &[Key]) {
    let mut w = BufWriter::new(std::fs::File::create(path).unwrap());
    for k in keys {
        let t: Vec<String> = k.iter().map(i32::to_string).collect();
        writeln!(w, "{}", t.join(",")).unwrap();
    }
}

fn piece_names() -> Vec<String> {
    let cfg: Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("T178B/cfg_t178_regenB.json")).unwrap(),
    )
    .unwrap();
    cfg["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            t.as_str()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .trim_end_matches(".jsonl")
                .to_string()
        })
        .collect()
}

fn raw_piece(name: &str) -> PathBuf {
    fixtures()
        .join("eng/T178B_raw")
        .join(format!("{name}.F.table.jsonl"))
}

/// A core table from a raw `table_dump.v1` file, plus core record -> engine rule position (`Table::load` keeps every
/// rule record; the core keeps the dispatched ones).
fn core_raw(path: &Path, name: &str) -> (Table, Vec<usize>, NodeFrame) {
    let tf = read_table_file(path, ReadOptions::default()).unwrap();
    let frame = NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap();
    let table = Table::from_table_file(
        name,
        path.display().to_string(),
        &tf,
        DomainOptions::default(),
    )
    .unwrap();
    let kept: Vec<usize> = tf
        .rules
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r.dispatch.as_deref(), None | Some("Partition")))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(kept.len(), table.len(), "{name}: dispatched records");
    (table, kept, frame)
}

/// Step 1: key files, the T162 matcher table, the run script.
#[test]
#[ignore = "T-DOMAIN step 1: writes inputs under DOMAINCHECK_DIR"]
fn prepare() {
    let dc = dc_dir();
    std::fs::create_dir_all(dc.join("t162")).unwrap();
    std::fs::create_dir_all(dc.join("t178")).unwrap();
    let fx = fixtures();

    // T162: one engine-loadable MATCHER table = origin000's header + the 2,440 qualified native bodies in bank-id
    // order (byte slices of NATIVE-RULE-BODIES.jsonl). Table::load reads only denominators / mask / power_shifts of a
    // header; all 56 origins agree on them and on the presentation (checked here). Not an accepted table: the export's
    // README forbids loading the flat stream as one; this file serves the matcher comparison only.
    let export = PathBuf::from(
        std::env::var("T162_BANK_EXPORT").unwrap_or_else(|_| T162_BANK_EXPORT.into()),
    );
    let mut bank = read_bank_predicates(fx.join("T162/RULE-PREDICATES2440.json")).unwrap();
    bank.sort_by_key(|p| p.global_ordinal0);
    let index: BTreeMap<i64, Value> =
        std::io::BufReader::new(std::fs::File::open(export.join("bank/RULE-INDEX.jsonl")).unwrap())
            .lines()
            .map(|l| {
                let v: Value = serde_json::from_str(&l.unwrap()).unwrap();
                (v["study_ordinal0"].as_i64().unwrap(), v)
            })
            .collect();
    let header_of = |rel: &str| -> Value {
        let text = std::fs::read_to_string(export.join(rel)).unwrap();
        text.lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .find(|v| v["type"] == "header")
            .unwrap()
    };
    let h0_text = std::fs::read_to_string(export.join("origins/origin000/HEADER.jsonl")).unwrap();
    let h0_line = h0_text
        .lines()
        .find(|l| l.contains("\"type\":\"header\""))
        .unwrap()
        .to_string();
    let h0: Value = serde_json::from_str(&h0_line).unwrap();
    let same = |h: &Value| {
        ["denominators", "mask", "power_shifts"]
            .iter()
            .all(|f| h[f] == h0[f])
            && h["node_order"]["slot_permutation"] == h0["node_order"]["slot_permutation"]
    };
    let mut bodies = std::fs::File::open(export.join("bank/NATIVE-RULE-BODIES.jsonl")).unwrap();
    let out = dc.join("t162/bank2440_matcher.table.jsonl");
    let mut w = BufWriter::new(std::fs::File::create(&out).unwrap());
    writeln!(w, "{h0_line}").unwrap();
    let mut origins = HashSet::new();
    for p in &bank {
        let r = &index[&p.global_ordinal0];
        let origin = r["local_header"].as_str().unwrap();
        if origins.insert(origin.to_string()) {
            assert!(
                same(&header_of(origin)),
                "{origin}: header differs from origin000"
            );
        }
        bodies
            .seek(SeekFrom::Start(r["local_body_offset"].as_u64().unwrap()))
            .unwrap();
        let mut buf = vec![0u8; r["local_body_bytes"].as_u64().unwrap() as usize];
        bodies.read_exact(&mut buf).unwrap();
        let v: Value = serde_json::from_slice(&buf).unwrap();
        let fixed: Vec<Option<i64>> = v["fixed"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::as_i64)
            .collect();
        assert_eq!(fixed, p.raw_fixed20, "gid {}", p.global_ordinal0);
        w.write_all(&buf).unwrap();
        if buf.last() != Some(&b'\n') {
            w.write_all(b"\n").unwrap();
        }
    }
    drop(w);
    println!(
        "T162 matcher table: {} rules from {} origins -> {}",
        bank.len(),
        origins.len(),
        out.display()
    );
    let frame162 = NodeFrame::without_header_lines_first(20, 9);
    let keys162 = box_keys(&frame162, |d| if d == 0 { 1 } else { 2 }, Some(5));
    assert_eq!(keys162.len(), 55_704);
    write_keys(&dc.join("t162/keys.txt"), &keys162);

    // T178: box keys; per piece 10,000 keys = up to 5,000 the piece alone covers (evenly spaced) + evenly spaced rest
    let names = piece_names();
    let (_, _, frame) = core_raw(&raw_piece(&names[0]), &names[0]);
    let keys = box_keys(&frame, |d| if d == 0 { 2 } else { 3 }, None);
    assert_eq!(keys.len(), 286_066);
    write_keys(&dc.join("t178/keys_all.txt"), &keys);
    let mut run = String::new();
    let bin = "${RR_NODE_BIN:?}";
    run += &format!(
        "{bin} domaincheck --point-valid --table {} --keys {} --out {}\n",
        out.display(),
        dc.join("t162/keys.txt").display(),
        dc.join("t162/out.jsonl").display()
    );
    for name in &names {
        let (table, _, _) = core_raw(&raw_piece(name), name);
        let disp = Dispatcher::new(vec![table], vec![0]).unwrap();
        let rep = disp.batch(&keys, 4).unwrap();
        let covered: Vec<usize> = (0..keys.len())
            .filter(|&i| rep.results[i].is_covered())
            .collect();
        let take = covered.len().min(5_000);
        let mut chosen: Vec<usize> = (0..take)
            .map(|j| covered[j * covered.len() / take.max(1)])
            .collect();
        let set: HashSet<usize> = chosen.iter().copied().collect();
        let rest: Vec<usize> = (0..keys.len()).filter(|i| !set.contains(i)).collect();
        let fill = 10_000 - take;
        chosen.extend((0..fill).map(|j| rest[j * rest.len() / fill]));
        chosen.sort_unstable();
        let ks: Vec<Key> = chosen.iter().map(|&i| keys[i].clone()).collect();
        write_keys(&dc.join(format!("t178/keys_{name}.txt")), &ks);
        run += &format!(
            "{bin} domaincheck --point-valid --table {} --keys {} --out {}\n",
            raw_piece(name).display(),
            dc.join(format!("t178/keys_{name}.txt")).display(),
            dc.join(format!("t178/out_{name}.jsonl")).display()
        );
        println!(
            "piece {name}: covers {} of the box alone; {} covered keys chosen",
            covered.len(),
            take
        );
    }
    for name in ["R0_try1", "pool1.pieces_rgq-g0-10"] {
        run += &format!(
            "{bin} domaincheck --point-valid --table {} --keys {} --out {}\n",
            raw_piece(name).display(),
            dc.join("t178/keys_all.txt").display(),
            dc.join(format!("t178/out_full_{name}.jsonl")).display()
        );
    }
    std::fs::write(dc.join("RUN.txt"), run).unwrap();
}

/// Difference classes (interfaces/campaign-core-domain.md s.3) and the comparison tally of one run.
#[derive(Default, Debug)]
struct Tally {
    keys: usize,
    agree_rule: usize,
    agree_none: usize,
    /// s.3 item 6: the engine selects a record with elided witness loci; the core blocks (ruling F1)
    elided: usize,
    /// s.3 item 6: a witness locus of the selected record vanishes identically in d; the core blocks
    witness_loci: usize,
    /// s.3 item 6: bank poles (written denominators, source conditions, cached poles); the core blocks
    poles: usize,
    /// s.3 item 6 / bank GE atoms: recenter (and chart) bounds the engine does not test
    recenter_bounds: usize,
    /// s.3 item 5: chart
    chart: usize,
    /// s.3 items 1-3: the engine's numeric tests (marked ambiguous by the verb) or a core `Undecided`
    numeric_vs_exact: usize,
    /// s.2 (T162 bank domains): cached loci the bank adds to the written exceptions
    bank_cached_loci: usize,
    /// s.3 item 6 (re-audit RA-MAJOR-1): the selected record carries no witness loci (`WitnessStatus::NotCarried`, no
    /// attestation); the core blocks (`Verdict::WitnessNotCarried`), the engine's matcher does not look
    witness_not_carried: usize,
    /// keys where the ENGINE's selected record is `NotCarried` in the core table (an independent count of the above)
    engine_on_not_carried: usize,
    findings: Vec<String>,
    /// T-DOMAIN (b): on keys where both select the same rule, the engine's `point_valid` verdicts
    point_valid: BTreeMap<String, usize>,
}

fn pole_class(kind: &PoleKind) -> &'static str {
    match kind {
        PoleKind::Witness => "witness_loci",
        _ => "poles",
    }
}

impl Tally {
    fn bump(&mut self, class: &str) {
        match class {
            "elided" => self.elided += 1,
            "witness_loci" => self.witness_loci += 1,
            "poles" => self.poles += 1,
            "recenter_bounds" => self.recenter_bounds += 1,
            "chart" => self.chart += 1,
            "numeric_vs_exact" => self.numeric_vs_exact += 1,
            "bank_cached_loci" => self.bank_cached_loci += 1,
            "witness_not_carried" => self.witness_not_carried += 1,
            other => panic!("class {other}"),
        }
    }
    fn differences(&self) -> usize {
        self.elided
            + self.witness_loci
            + self.poles
            + self.recenter_bounds
            + self.chart
            + self.numeric_vs_exact
            + self.bank_cached_loci
            + self.witness_not_carried
            + self.findings.len()
    }
    fn line(&self, name: &str) -> String {
        format!(
            "{name}: keys {} agree {} (rule {}, none {}) differences {} [elided {}, witness_loci {}, poles {}, recenter_bounds {}, chart {}, numeric_vs_exact {}, bank_cached_loci {}, witness_not_carried {}, FINDINGS {}] point_valid {:?}",
            self.keys,
            self.agree_rule + self.agree_none,
            self.agree_rule,
            self.agree_none,
            self.differences(),
            self.elided,
            self.witness_loci,
            self.poles,
            self.recenter_bounds,
            self.chart,
            self.numeric_vs_exact,
            self.bank_cached_loci,
            self.witness_not_carried,
            self.findings.len(),
            self.point_valid
        )
    }
}

/// Compare one engine output with the core on the same keys. `to_engine[r]` = engine rule position of core record
/// `r`; `written_exceptions[r]` = number of written exception branches of core record `r` (larger branch indices are
/// cached bank loci).
fn compare(
    out: &Path,
    table: &Table,
    to_engine: &[usize],
    written_exceptions: Option<&[usize]>,
) -> Tally {
    let disp = Dispatcher::new(vec![table.clone()], vec![0]).unwrap();
    let mut from_engine = vec![usize::MAX; to_engine.iter().max().map_or(0, |m| m + 1)];
    for (r, &e) in to_engine.iter().enumerate() {
        from_engine[e] = r;
    }
    let mut t = Tally::default();
    let f = std::io::BufReader::new(
        std::fs::File::open(out).unwrap_or_else(|e| panic!("{}: {e} (run step 2)", out.display())),
    );
    for line in f.lines() {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        match v["type"].as_str() {
            Some("header") => {
                assert_eq!(v["schema"], "rustred.domaincheck.v1");
                continue;
            }
            Some("summary") => {
                assert_eq!(v["keys"].as_u64().unwrap() as usize, t.keys);
                continue;
            }
            _ => {}
        }
        t.keys += 1;
        let key = parse_key(v["key"].as_str().unwrap()).unwrap();
        let e_sel = v["selected"]["rule"].as_u64().map(|x| x as usize);
        let ambiguous = v["ambiguous"].as_bool().unwrap();
        assert_eq!(
            v["in_sector"], true,
            "box key out of the engine's sector: {}",
            v["key"]
        );
        let c = disp.first_match(&key).unwrap();
        if let Some(es) = e_sel
            && table.meta[from_engine[es]].witness == WitnessStatus::NotCarried
        {
            t.engine_on_not_carried += 1;
        }
        let same = |r: usize| Some(to_engine[r]) == e_sel;
        let finding = |t: &mut Tally, what: String| {
            t.findings
                .push(format!("{} engine {:?} core {what}", v["key"], e_sel));
        };
        match &c {
            Dispatch::Covered(r) if same(r.record) => {
                t.agree_rule += 1;
                *t.point_valid
                    .entry(v["point_valid"].as_str().unwrap_or("null").to_string())
                    .or_insert(0) += 1;
            }
            Dispatch::Uncovered if e_sel.is_none() => t.agree_none += 1,
            Dispatch::Blocked { at, reason } if same(at.record) => match reason {
                Verdict::Elided => t.bump("elided"),
                Verdict::OnPole { kind, .. } => t.bump(pole_class(kind)),
                Verdict::RecenterBound(_) => t.bump("recenter_bounds"),
                Verdict::ChartMismatch { .. } => t.bump("chart"),
                Verdict::WitnessNotCarried => t.bump("witness_not_carried"),
                other => finding(&mut t, format!("blocked {other:?}")),
            },
            Dispatch::Undecided { at, .. } if same(at.record) => t.bump("numeric_vs_exact"),
            _ if ambiguous => t.bump("numeric_vs_exact"),
            _ => {
                // the core passed over the engine's rule (or the engine passed over the core's): why, in the core?
                let core_pos = c.winner().map(|w| to_engine[w.record]);
                match e_sel {
                    Some(es) if core_pos.is_none_or(|cp| cp > es) => {
                        let r = from_engine[es];
                        match table.domains[r].applies(&key) {
                            Verdict::NotInCase(
                                rustred_campaign_core::domain::NotInCase::Bound(_),
                            ) => t.bump("recenter_bounds"),
                            Verdict::OnException { branch }
                                if written_exceptions.is_some_and(|w| branch >= w[r]) =>
                            {
                                t.bump("bank_cached_loci")
                            }
                            other => finding(
                                &mut t,
                                format!("{c:?}; engine rule in the core: {other:?}"),
                            ),
                        }
                    }
                    _ => finding(&mut t, format!("{c:?}")),
                }
            }
        }
    }
    t
}

fn report(name: &str, t: &Tally) {
    println!("{}", t.line(name));
    for f in t.findings.iter().take(10) {
        println!("  FINDING {f}");
    }
}

/// (a) T162 box B, the engine's matcher (step 2) against: (a1) the core reading the coordinator's CONVERTED bank tables
/// (v1.2, set-unique labels; the same 2,440 qualified records in the same first-match order), strict witness status;
/// (a2) the core's bank predicates (its T1 input); (a3) core against core: the bank predicates against our domain code
/// on the converted records, every box key.
#[test]
#[ignore = "T-DOMAIN step 3: needs DOMAINCHECK_DIR/t162/out.jsonl and the converted bank set"]
fn t_domain_t162() {
    let dc = dc_dir();
    let out = dc.join("t162/out.jsonl");
    let mut bank = read_bank_predicates(fixtures().join("T162/RULE-PREDICATES2440.json")).unwrap();
    bank.sort_by_key(|p| p.global_ordinal0);
    let (table, labels) = bank_tables(DomainOptions::default());
    // the engine's rule positions are the bank's first-match order: the converted set is that order, record for record
    assert_eq!(
        labels,
        bank.iter().map(|p| p.global_ordinal0).collect::<Vec<_>>(),
        "converted qualified set = the bank predicates' records in global-id order"
    );
    let ident: Vec<usize> = (0..bank.len()).collect();
    let a1 = compare(&out, &table, &ident, None);
    report("T162 (a1) converted bank tables (strict witness)", &a1);

    let bt = Table::from_bank(
        "T162-bank",
        "T162/RULE-PREDICATES2440.json",
        &bank,
        20,
        TargetSource::CanonicalByConstruction {
            attestation: "T162 bank: no target field, presented slots, no recenter or permutation",
        },
    )
    .unwrap();
    let written: Vec<usize> = bank
        .iter()
        .map(|p| p.exceptions.as_ref().map_or(0, Vec::len))
        .collect();
    let a2 = compare(&out, &bt, &ident, Some(&written));
    report("T162 (a2) bank predicates", &a2);
    // (a2) measured 2026-11-14 (engine rr_node sha256 dafdce8a... and eedb3092..., byte-identical outputs): identical
    // decisions on every key, both ways
    assert!(a2.findings.is_empty(), "findings: see the report");
    assert_eq!(
        (a2.keys, a2.agree_rule, a2.agree_none),
        (55_704, 54_714, 990)
    );
    assert_eq!(a2.differences(), 0);
    assert_eq!(a2.point_valid.get("ok"), Some(&54_714));
    // (a1) 2,437 of the 2,440 qualified records carry no witness loci (coord-bank-tables.md v1.2 s.0): the core blocks
    // every key whose selected record is one of them (WitnessNotCarried) and agrees on every other key; nothing else
    let without: usize = table
        .meta
        .iter()
        .filter(|m| m.witness == WitnessStatus::NotCarried)
        .count();
    println!(
        "converted qualified records without witness loci: {without} of {}",
        table.len()
    );
    assert_eq!((without, table.len()), (2_437, 2_440));
    assert!(a1.findings.is_empty(), "findings: see the report");
    assert_eq!((a1.keys, a1.agree_none), (55_704, 990));
    assert_eq!(a1.witness_not_carried, a1.engine_on_not_carried);
    assert_eq!(
        a1.differences(),
        a1.witness_not_carried,
        "the ONLY difference class"
    );
    assert_eq!(a1.agree_rule + a1.witness_not_carried, 54_714);
    assert_eq!(
        a1.point_valid.get("ok").copied().unwrap_or(0),
        a1.agree_rule
    );
    // (a3) core vs core on every box key: bank predicates vs our domain code on the converted records, (i) with the
    // witness check off: the same first match everywhere; (ii) strict: differs only where the converted record is
    // NotCarried and blocks
    let frame = NodeFrame::without_header_lines_first(20, 9);
    let keys = box_keys(&frame, |d| if d == 0 { 1 } else { 2 }, Some(5));
    assert_eq!(keys.len(), 55_704);
    let (loose, _) = bank_tables(DomainOptions {
        witness: false,
        ..Default::default()
    });
    let db = Dispatcher::new(vec![bt], vec![0]).unwrap();
    let dl = Dispatcher::new(vec![loose], vec![0]).unwrap();
    let ds = Dispatcher::new(vec![table], vec![0]).unwrap();
    let (mut same, mut covered, mut blocked_nc) = (0usize, 0usize, 0usize);
    for k in &keys {
        let b = db.first_match(k).unwrap();
        let l = dl.first_match(k).unwrap();
        let s = ds.first_match(k).unwrap();
        let pos = |d: &Dispatch| d.winner().map(|w| w.record);
        assert_eq!(
            pos(&b),
            pos(&l),
            "{k:?}: bank predicates {b:?} vs domain code {l:?}"
        );
        assert_eq!(b.is_covered(), l.is_covered(), "{k:?}");
        same += 1;
        covered += usize::from(b.is_covered());
        match (&l, &s) {
            (Dispatch::Covered(x), Dispatch::Covered(y)) if x.record == y.record => {}
            (Dispatch::Uncovered, Dispatch::Uncovered) => {}
            (
                Dispatch::Covered(x),
                Dispatch::Blocked {
                    at,
                    reason: Verdict::WitnessNotCarried,
                },
            ) if x.record == at.record => blocked_nc += 1,
            other => panic!("{k:?}: loose vs strict {other:?}"),
        }
    }
    println!(
        "T162 (a3) bank predicates vs domain code: {same} keys identical ({covered} covered); strict witness: {blocked_nc} WitnessNotCarried"
    );
    assert_eq!((same, covered), (55_704, 54_714));
    assert_eq!(blocked_nc, a1.witness_not_carried);
    assert_eq!(
        (a1.witness_not_carried, a1.agree_rule),
        T162_WITNESS_NOT_CARRIED,
        "measured split (WitnessNotCarried, agree on a rule)"
    );
}

/// (a1) measured 2026-11-14 on the converted set 20261114c (int4): keys whose selected record carries no witness loci
/// (blocked by the core) and keys where both select the same rule.
const T162_WITNESS_NOT_CARRIED: (usize, usize) = (54_712, 2);

/// (b) every T178 order-B piece as its own table, 10,000 box keys each; the raw table's domains equal the case
/// export the core's T2 reads, record by record.
#[test]
#[ignore = "T-DOMAIN step 3: needs DOMAINCHECK_DIR/t178/out_<piece>.jsonl"]
fn t_domain_t178_pieces() {
    let dc = dc_dir();
    let mut total = Tally::default();
    let (mut raw_elided, mut raw_witness_poles) = (0usize, 0usize);
    for name in piece_names() {
        let (table, kept, _) = core_raw(&raw_piece(&name), &name);
        let tf = read_table_file(
            fixtures().join(format!("T178B/cases/{name}.jsonl")),
            ReadOptions::default(),
        )
        .unwrap();
        let export = Table::from_table_file(
            name.clone(),
            "case export",
            &tf,
            DomainOptions {
                targets: TargetSource::CanonicalByConstruction {
                    attestation: "T178B case exports of engine table_dump.v1 tables",
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(export.len(), table.len(), "{name}: records");
        // the case export carries no witness loci (s.2: WitnessProvenance::NotCarried): compare without them
        for (a, b) in table.domains.iter().zip(&export.domains) {
            let (mut ca, cb) = (a.canonical_form(), b.canonical_form());
            raw_elided += usize::from(ca.witness_elided);
            raw_witness_poles += ca.poles.iter().filter(|p| p.0 == 2).count();
            ca.witness_elided = false;
            ca.poles.retain(|p| p.0 != 2);
            assert_eq!(ca, cb, "{name}: raw table and case export differ");
        }
        let t = compare(
            &dc.join(format!("t178/out_{name}.jsonl")),
            &table,
            &kept,
            None,
        );
        report(&name, &t);
        total.keys += t.keys;
        total.agree_rule += t.agree_rule;
        total.agree_none += t.agree_none;
        total.elided += t.elided;
        total.witness_loci += t.witness_loci;
        total.poles += t.poles;
        total.recenter_bounds += t.recenter_bounds;
        total.chart += t.chart;
        total.numeric_vs_exact += t.numeric_vs_exact;
        total.findings.extend(t.findings);
        for (k, c) in t.point_valid {
            *total.point_valid.entry(k).or_insert(0) += c;
        }
    }
    report("T178 pieces total", &total);
    println!(
        "raw tables vs case exports: equal up to witness loci ({raw_elided} elided records, {raw_witness_poles} witness pole loci in the raw tables)"
    );
    // measured 2026-11-14: the only differences are records with elided witness loci (ruling F1), 8 keys
    assert!(total.findings.is_empty(), "findings: see the report");
    assert_eq!(
        (total.keys, total.agree_rule, total.agree_none),
        (380_000, 127_901, 252_091)
    );
    assert_eq!(total.elided, 8);
    assert_eq!(total.differences(), 8);
    assert_eq!(total.point_valid.get("ok"), Some(&127_901));
    assert_eq!((raw_elided, raw_witness_poles), (16, 10_438));
}

/// (c) the two raw tables the core already reads (R0_try1, rgq-g0-10), every T178 box key.
#[test]
#[ignore = "T-DOMAIN step 3: needs DOMAINCHECK_DIR/t178/out_full_*.jsonl"]
fn t_domain_raw_full() {
    let dc = dc_dir();
    // measured 2026-11-14: (name, engine rule, engine none, elided differences)
    for (name, rule, none, elided) in [
        ("R0_try1", 264_234, 21_832, 0),
        ("pool1.pieces_rgq-g0-10", 10_140, 275_905, 21),
    ] {
        let (table, kept, _) = core_raw(&raw_piece(name), name);
        let t = compare(
            &dc.join(format!("t178/out_full_{name}.jsonl")),
            &table,
            &kept,
            None,
        );
        report(name, &t);
        assert!(t.findings.is_empty(), "findings: see the report");
        assert_eq!(t.keys, 286_066);
        assert_eq!((t.agree_rule, t.agree_none), (rule, none));
        assert_eq!((t.elided, t.differences()), (elided, elided));
        assert_eq!(t.point_valid.get("ok"), Some(&rule));
    }
}
