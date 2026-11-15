//! Eps-shifted (two-sided) families in the frame, the domains and first-match dispatch (lane epsframe).
//!
//! Test data `tests/data/eps/` (MANIFEST.sha256): three small real tables (T012 a0-td-e: shift (12-3d)/2 = 3 eps;
//! T041 min: (4-d)/2; T047 min: 4-d = 2 eps), five real headers (T002 mixed 4-d / (4-d)/2, T003 two axes, T011 three
//! axes, T064, T082 order n_e_t), and per table the ENGINE's first match on a box of keys: rr_proof instantiate facts
//! (frozen 20261114T2332Z-engine-line7a), one job per chart, condensed to "first record in file order with in_case,
//! chart_ok and no exception branch" (`mkreq.py`, `condense.py` next to the data). rr_node's own first-match /
//! domaincheck verbs refuse eps tables (mastermap.rs:300-305), so rr_proof's per-record facts are the engine reference.

use std::path::PathBuf;

use rustred_campaign_core::cover::{
    CoverError, Dispatch, Dispatcher, EPS_LABEL_STRIDE, LoadError, Table, eps_label,
};
use rustred_campaign_core::domain::{CaseDomain, DomainError, DomainOptions};
use rustred_campaign_core::key::{FrameError, KeyClass, NodeFrame};
use rustred_campaign_core::record::{
    ReadOptions, RecordReader, TableFile, TableHeader, TableLine, read_table_file,
};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/eps")
        .join(name)
}

fn header_of(text: &str) -> TableHeader {
    let line = text.lines().next().unwrap();
    let mut rd = RecordReader::new(line.as_bytes(), ReadOptions::default());
    match rd.next().unwrap().unwrap() {
        TableLine::Header(h) => *h,
        _ => panic!("not a header"),
    }
}

fn header_file(name: &str) -> TableHeader {
    header_of(&std::fs::read_to_string(data(name)).unwrap())
}

/// (file, expected kshift (slot, k) entries)
const REAL: &[(&str, &[(usize, i64)])] = &[
    ("T012_a0-td-e.table.jsonl", &[(0, 3)]),
    ("T041_min.table.jsonl", &[(5, 1)]),
    ("T047_min.table.jsonl", &[(5, 2)]),
    ("T002_min.header.json", &[]),
    ("T003_min.header.json", &[]),
    ("T011_min.header.json", &[]),
    ("T064_min.header.json", &[(7, 1)]),
    ("T082_min.header.json", &[(3, 1)]),
];

#[test]
fn eps_frame_parses_real_headers() {
    for (file, expect) in REAL {
        let h = header_file(file);
        // integer-only consumers refuse, naming the refusal
        match NodeFrame::from_header(&h) {
            Err(FrameError::Unsupported(m)) => assert!(m.contains("integer-only"), "{file}: {m}"),
            o => panic!("{file}: {o:?}"),
        }
        let f = NodeFrame::from_header_eps_aware(&h).unwrap_or_else(|e| panic!("{file}: {e}"));
        let raw: Vec<String> = h.raw["power_shifts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        assert_eq!(f.power_shifts(), raw.as_slice(), "{file}");
        // exact values: (4-d)/2 -> 1, 4-d -> 2, (12-3*d)/2 -> 3
        for (i, s) in raw.iter().enumerate() {
            let k = match s.as_str() {
                "0" => 0,
                "(4-d)/2" => 1,
                "4-d" => 2,
                "(12-3*d)/2" => 3,
                o => panic!("{file}: unexpected shift {o}"),
            };
            assert_eq!(f.kshift()[i], k, "{file} slot {i}");
        }
        for &(slot, k) in *expect {
            assert_eq!(f.kshift()[slot], k, "{file}");
        }
        // eps axes = the header's two-sided axes, every eps axis is a line, 2^k charts
        let ts: Vec<usize> = (0..h.two_sided.len()).filter(|&i| h.two_sided[i]).collect();
        assert_eq!(f.eps_axes(), ts, "{file}");
        assert!(f.eps_axes().iter().all(|&a| f.is_line(a)));
        assert_eq!(f.charts().len(), 1 << f.eps_axes().len(), "{file}");
        assert!(f.has_eps_axes() && !f.is_integer_only());
    }
    // the multi-axis headers
    let f = NodeFrame::from_header_eps_aware(&header_file("T002_min.header.json")).unwrap();
    let mut ks: Vec<i64> = f.eps_axes().iter().map(|&a| f.kshift()[a]).collect();
    ks.sort();
    assert_eq!(ks, vec![1, 2]);
    assert_eq!(
        NodeFrame::from_header_eps_aware(&header_file("T003_min.header.json"))
            .unwrap()
            .eps_axes()
            .len(),
        2
    );
    assert_eq!(
        NodeFrame::from_header_eps_aware(&header_file("T011_min.header.json"))
            .unwrap()
            .charts()
            .len(),
        8
    );
}

#[test]
fn chart_lines_of_real_tables_are_frame_charts() {
    for file in [
        "T012_a0-td-e.table.jsonl",
        "T041_min.table.jsonl",
        "T047_min.table.jsonl",
    ] {
        let tf = read_table_file(data(file), ReadOptions::default()).unwrap();
        let f = NodeFrame::from_header_eps_aware(tf.header.as_ref().unwrap()).unwrap();
        assert_eq!(tf.charts.len(), f.charts().len(), "{file}");
        for (c, guards) in &tf.charts {
            let bits: Vec<bool> = c.chars().map(|x| x == '1').collect();
            assert!(f.charts().contains(&bits), "{file} {c}");
            assert_eq!(f.chart_guards(&bits).as_ref(), Some(guards), "{file} {c}");
        }
    }
}

fn h5(shifts: &str, two_sided: &str) -> TableHeader {
    header_of(&format!(
        r#"{{"type":"header","denominators":5,"mask":"11110","power_shifts":{shifts},"order":{{"two_sided":{two_sided}}}}}"#
    ))
}

#[test]
fn shift_forms_exact_and_refusals() {
    let ts1 = "[true,false,false,false,false]";
    for (s, k) in [
        ("(4-d)/2", 1),
        ("2-d/2", 1),
        ("(d-4)/2", -1),
        ("4-d", 2),
        ("(8-2*d)/2", 2),
        ("6-3*d/2", 3),
        ("(12-3*d)/2", 3),
    ] {
        let h = h5(&format!(r#"["{s}","0","0","0","0"]"#), ts1);
        let f = NodeFrame::from_header_eps_aware(&h).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(f.kshift(), &[k, 0, 0, 0, 0], "{s}");
        assert!(matches!(
            NodeFrame::from_header(&h),
            Err(FrameError::Unsupported(_))
        ));
    }
    // not a nonzero integer multiple of (4-d)/2, not a string, padded zero: refused, never read as 0
    for s in [
        r#""1""#,
        r#""eps""#,
        r#""(3-d)/2""#,
        r#""d""#,
        r#""0 ""#,
        r#""4-d+n1""#,
        "0",
        "1",
    ] {
        let h = h5(&format!(r#"[{s},"0","0","0","0"]"#), ts1);
        assert!(
            matches!(
                NodeFrame::from_header_eps_aware(&h),
                Err(FrameError::Unsupported(_))
            ),
            "{s}"
        );
    }
    // an eps shift on a numerator slot
    let h = h5(
        r#"["0","0","0","0","(4-d)/2"]"#,
        "[false,false,false,false,true]",
    );
    assert!(matches!(
        NodeFrame::from_header_eps_aware(&h),
        Err(FrameError::Unsupported(_))
    ));
    // eps axes differ from the two-sided order axes (or the order has none)
    for ts in ["[false,true,false,false,false]", "[]", "null"] {
        let h = h5(r#"["(4-d)/2","0","0","0","0"]"#, ts);
        assert!(
            matches!(
                NodeFrame::from_header_eps_aware(&h),
                Err(FrameError::Unsupported(_))
            ),
            "{ts}"
        );
    }
    // an integer header reads the same through both constructors
    let h = h5(r#"["0","0","0","0","0"]"#, "null");
    assert_eq!(
        NodeFrame::from_header(&h).unwrap(),
        NodeFrame::from_header_eps_aware(&h).unwrap()
    );
}

#[test]
fn forced_axis_semantics() {
    // slot 0 eps (k = 1), slots 1..3 lines, slot 4 numerator
    let f = NodeFrame::from_header_eps_aware(&h5(
        r#"["(4-d)/2","0","0","0","0"]"#,
        "[true,false,false,false,false]",
    ))
    .unwrap();
    let neg = [-2, 1, 1, 1, 0];
    let pos = [2, 1, 1, 1, -1];
    // the eps axis is in every sector, never a removed line
    assert_eq!(f.sector(&neg), 0b1111);
    assert_eq!(f.sector_slots(&neg), vec![0, 1, 2, 3]);
    assert!(f.is_top_sector(&neg) && f.in_chart_orthant(&neg));
    assert_eq!(f.chart_violation(&neg), None);
    assert_eq!(f.classify(&neg).unwrap(), KeyClass::Chart);
    // an integer line <= 0 still removes the line; a positive numerator still leaves the chart
    assert_eq!(
        f.classify(&[0, 0, 1, 1, 0]).unwrap(),
        KeyClass::Subsector {
            removed_lines: vec![1],
            positive_numerators: vec![]
        }
    );
    assert_eq!(f.sector(&[0, 0, 1, 1, 0]), 0b1101);
    assert_eq!(f.chart_violation(&[0, 1, 1, 1, 1]), Some(4));
    // charts and the chart of a key
    assert_eq!(
        f.charts(),
        vec![
            vec![false, true, true, true, false],
            vec![true, true, true, true, false]
        ]
    );
    assert_eq!(
        f.chart_of(&neg).unwrap(),
        vec![false, true, true, true, false]
    );
    assert_eq!(
        f.chart_of(&pos).unwrap(),
        vec![true, true, true, true, false]
    );
    assert_eq!(f.chart_of(&[1, 1]), None);
    assert_eq!(
        f.chart_guards(&[false, true, true, true, false]).unwrap(),
        vec!["two_sided:n<=0", "n>=1", "n>=1", "n>=1", "n<=0"]
    );
    assert_eq!(f.chart_guards(&[true, false, true, true, false]), None);
    // dots / rank / layers: integer frames only (the eps box convention is spec data)
    assert_eq!(f.dots(&pos), None);
    assert_eq!(f.rank(&pos), None);
    assert!(f.chart_layer(1, 1, None).is_empty());
    assert!(f.require_integer_only_for("the box").is_err());
    // integer frame: unchanged
    let g = NodeFrame::without_header_from_mask("11110").unwrap();
    assert_eq!(g.sector(&neg), 0b1110);
    assert!(!g.in_chart_orthant(&neg));
    assert_eq!(g.charts(), vec![g.mask().to_vec()]);
    assert_eq!(g.dots(&pos), Some(1));
}

/// (key, engine first match as (chart, index)).
type EngineFirst = (Vec<i32>, Option<(String, i64)>);

fn engine_first(file: &str) -> Vec<EngineFirst> {
    std::fs::read_to_string(data(file))
        .unwrap()
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            let key = v["key"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            let first = v["first"]
                .as_array()
                .map(|a| (a[0].as_str().unwrap().to_string(), a[1].as_i64().unwrap()));
            (key, first)
        })
        .collect()
}

fn load_eps(file: &str) -> (TableFile, Dispatcher) {
    let tf = read_table_file(data(file), ReadOptions::default()).unwrap();
    // the witness status is not what is compared here (minimised tables may carry no loci)
    let opts = DomainOptions {
        witness: false,
        ..Default::default()
    };
    let t = Table::from_table_file_eps_aware(file, file, &tf, opts).unwrap();
    assert_eq!(t.skipped_count(), 0);
    assert!(t.eps_chart().is_some());
    (tf, Dispatcher::new(vec![t], vec![0]).unwrap())
}

#[test]
fn first_match_agrees_with_engine_facts() {
    for (table, facts) in [
        ("T012_a0-td-e.table.jsonl", "T012.engine_first.jsonl"),
        ("T041_min.table.jsonl", "T041.engine_first.jsonl"),
        ("T047_min.table.jsonl", "T047.engine_first.jsonl"),
    ] {
        let (tf, disp) = load_eps(table);
        let frame = NodeFrame::from_header_eps_aware(tf.header.as_ref().unwrap()).unwrap();
        let rule_of = |source_index: usize| {
            let r = &tf.rules[source_index];
            (r.chart.clone().unwrap(), r.index.unwrap())
        };
        let (mut agree, mut nonpos) = (0usize, 0usize);
        for (key, want) in engine_first(facts) {
            if frame.eps_axes().iter().any(|&a| key[a] <= 0) {
                nonpos += 1;
            }
            let got = disp.first_match(&key).unwrap();
            let fast_slow = disp.first_match_slow(&key).unwrap();
            assert_eq!(got, fast_slow, "{table} {key:?}");
            let sel = match &got {
                Dispatch::Covered(r) => Some(rule_of(r.source_index)),
                Dispatch::Blocked { at, .. } => Some(rule_of(at.source_index)),
                Dispatch::Uncovered => None,
                o => panic!("{table} {key:?}: {o:?}"),
            };
            assert_eq!(sel, want, "{table} {key:?}: {got:?}");
            // the selected rule is of the key's chart
            if let Some((c, _)) = &sel {
                let bits: Vec<bool> = c.chars().map(|x| x == '1').collect();
                assert_eq!(Some(bits), frame.chart_of(&key), "{table} {key:?}");
            }
            agree += 1;
        }
        assert!(
            agree > 500 && nonpos > 200,
            "{table}: {agree} keys, {nonpos} with eps <= 0"
        );
    }
}

#[test]
fn integer_only_consumers_still_refuse_eps_tables() {
    let tf = read_table_file(data("T041_min.table.jsonl"), ReadOptions::default()).unwrap();
    assert!(matches!(
        Table::from_table_file("t", "t", &tf, DomainOptions::default()),
        Err(LoadError::Frame(FrameError::Unsupported(_)))
    ));
    // recenter bounds are not defined on an eps axis (floors assume n >= 1)
    let opts = DomainOptions {
        recenter_bounds: true,
        witness: false,
        ..Default::default()
    };
    assert!(matches!(
        Table::from_table_file_eps_aware("t", "t", &tf, opts),
        Err(LoadError::Record {
            err: DomainError::Frame(FrameError::Unsupported(_)),
            ..
        })
    ));
    // an eps table never shares a dispatcher with integer-chart tables
    let (_, disp) = load_eps("T041_min.table.jsonl");
    let eps_t = disp.tables()[0].clone();
    let mut int_t = Table::new("int", eps_t.arity());
    int_t
        .push(
            0,
            CaseDomain::free(eps_t.arity(), Some(vec![true; eps_t.arity()])),
        )
        .unwrap();
    assert!(matches!(
        Dispatcher::new(vec![eps_t, int_t], vec![0, 1]),
        Err(CoverError::Invalid(_))
    ));
}

#[test]
fn eps_records_need_a_frame_chart_and_unique_labels_per_chart() {
    let head = r#"{"type":"header","denominators":5,"mask":"11110","power_shifts":["(4-d)/2","0","0","0","0"],"order":{"two_sided":[true,false,false,false,false]}}"#;
    let rule = |chart: &str, index: i64, fixed0: &str| {
        format!(
            r#"{{"type":"rule","chart":"{chart}","index":{index},"fixed":[{fixed0},null,null,null,null],"equations":[],"target":"I(n0,n1,n2,n3,n4)","rhs":[],"exceptions":[]}}"#
        )
    };
    let load = |lines: &[String]| {
        let text = format!("{head}\n{}\n", lines.join("\n"));
        let path = std::env::temp_dir().join(format!(
            "eps_frame_{}_{}.jsonl",
            std::process::id(),
            lines.len() * 31 + lines.iter().map(|l| l.len()).sum::<usize>()
        ));
        std::fs::write(&path, text).unwrap();
        let tf = read_table_file(&path, ReadOptions::default()).unwrap();
        std::fs::remove_file(&path).ok();
        Table::from_table_file_eps_aware(
            "c",
            "c",
            &tf,
            DomainOptions {
                witness: false,
                ..Default::default()
            },
        )
    };
    // the same index in two charts: two rules
    let t = load(&[rule("11110", 0, "null"), rule("01110", 0, "null")]).unwrap();
    assert_eq!(t.len(), 2);
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    for (key, src) in [
        ([1, 1, 1, 1, 0], 0usize),
        ([0, 1, 1, 1, 0], 1),
        ([-3, 1, 1, 1, -1], 1),
    ] {
        match d.first_match(&key).unwrap() {
            Dispatch::Covered(r) => assert_eq!(r.source_index, src, "{key:?}"),
            o => panic!("{key:?}: {o:?}"),
        }
    }
    // integer slots keep the node orthant; the eps axis is free there
    assert!(matches!(
        d.first_match(&[1, 0, 1, 1, 0]).unwrap(),
        Dispatch::OutsideChart { slot: 1 }
    ));
    assert_eq!(d.chart_free(), &[0]);
    // the same index twice in one chart: refused
    assert!(matches!(
        load(&[rule("11110", 0, "null"), rule("11110", 0, "2")]),
        Err(LoadError::DuplicateLabel { .. })
    ));
    // a chart that is not a chart of the frame, a fixed eps value outside the record's chart
    assert!(load(&[rule("11100", 0, "null")]).is_err());
    assert!(load(&[rule("01110", 0, "2")]).is_err());
    // a record without chart in an eps family
    let nochart = rule("11110", 0, "null").replace(r#""chart":"11110","#, "");
    assert!(load(&[nochart]).is_err());
}

/// Coordinator requirement (2026-11-15, kernel defect be01c25e): on a multi-chart table a record is never identified
/// by its index alone. Two charts, both with a rule `index` 0, with DIFFERENT bodies: every lookup returns the right
/// record (dispatch, label, label decoding) or refuses.
#[test]
fn two_charts_with_index_zero_are_never_confused() {
    let head = r#"{"type":"header","denominators":5,"mask":"11110","power_shifts":["(4-d)/2","0","0","0","0"],"order":{"two_sided":[true,false,false,false,false]}}"#;
    // chart 11110 rule 0: case n1 = 2; chart 01110 rule 0: case n1 = 3; then a catch-all rule 1 in each chart
    let rec = |chart: &str, index: i64, eq: &str| {
        format!(
            r#"{{"type":"rule","chart":"{chart}","index":{index},"fixed":[null,null,null,null,null],"equations":[{eq}],"target":"I(n0,n1,n2,n3,n4)","rhs":[],"exceptions":[]}}"#
        )
    };
    let text = format!(
        "{head}\n{}\n{}\n{}\n{}\n",
        rec("11110", 0, r#""n1-2""#),
        rec("11110", 1, ""),
        rec("01110", 0, r#""n1-3""#),
        rec("01110", 1, ""),
    );
    let path = std::env::temp_dir().join(format!("eps_two_charts_{}.jsonl", std::process::id()));
    std::fs::write(&path, text).unwrap();
    let tf = read_table_file(&path, ReadOptions::default()).unwrap();
    std::fs::remove_file(&path).ok();
    let opts = DomainOptions {
        witness: false,
        ..Default::default()
    };
    let t = Table::from_table_file_eps_aware("two", "two", &tf, opts).unwrap();
    // labels: unique, each decodes to its own (chart, index)
    let mut uniq = t.labels.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), 4);
    for (k, &l) in t.labels.iter().enumerate() {
        let r = &tf.rules[t.meta[k].source_index];
        assert_eq!(
            t.eps_rule_of_label(l),
            Some((r.chart.clone().unwrap(), r.index.unwrap())),
            "label {l}"
        );
    }
    assert_eq!(t.labels[0], eps_label(1, 0).unwrap()); // 11110 is chart number 1 (charts sorted)
    assert_eq!(t.labels[2], eps_label(0, 0).unwrap());
    assert_ne!(t.labels[0], t.labels[2]);
    assert_eq!(t.eps_rule_of_label(eps_label(2, 0).unwrap()), None); // no third chart
    assert_eq!(eps_label(0, -1), None);
    assert_eq!(eps_label(0, EPS_LABEL_STRIDE), None);
    // dispatch: each key gets the index-0 rule of ITS chart, or the chart's rule 1
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let src = |key: [i32; 5]| match d.first_match(&key).unwrap() {
        Dispatch::Covered(r) => (
            tf.rules[r.source_index].chart.clone().unwrap(),
            tf.rules[r.source_index].index.unwrap(),
        ),
        o => panic!("{key:?}: {o:?}"),
    };
    assert_eq!(src([1, 2, 1, 1, 0]), ("11110".into(), 0));
    assert_eq!(src([1, 3, 1, 1, 0]), ("11110".into(), 1));
    assert_eq!(src([0, 3, 1, 1, 0]), ("01110".into(), 0));
    assert_eq!(src([-1, 2, 1, 1, 0]), ("01110".into(), 1));
    // the integer-only loader refuses the table outright (no index-keyed consumer ever sees it)
    assert!(matches!(
        Table::from_table_file("two", "two", &tf, opts),
        Err(LoadError::Frame(FrameError::Unsupported(_)))
    ));
}

/// The load sweep's eps tables through the import's CLASSIFY-ONLY load (`import::sweep::classify_table`, the path of
/// lane import5's sweep). Needs EPSFRAME_SWEEP_LIST (one path per line); appends one JSON line per table to
/// EPSFRAME_SWEEP_OUT if set (run one table per process under a memory cap for tables of several GB).
#[test]
#[ignore = "real five-loop tables (up to GBs); EPSFRAME_SWEEP_LIST=<list>; run with --ignored"]
fn eps_sweep_classify_only() {
    use rustred_campaign_core::import::sweep::{ClassifyOptions, classify_table};
    use std::io::Write;
    let list = std::env::var("EPSFRAME_SWEEP_LIST").expect("EPSFRAME_SWEEP_LIST");
    let mut out = std::env::var("EPSFRAME_SWEEP_OUT").ok().map(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .unwrap()
    });
    let opts = ClassifyOptions {
        scratch: Some(std::env::temp_dir()),
        ..Default::default()
    };
    let (mut loaded, mut refused) = (0usize, 0usize);
    for path in std::fs::read_to_string(list).unwrap().lines() {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        let c = classify_table(std::path::Path::new(path), &opts);
        if c.outcome == "loaded" {
            loaded += 1;
        } else {
            refused += 1;
        }
        let line = serde_json::json!({"path": path, "outcome": c.outcome, "reason": c.reason,
            "detail": c.detail, "records": c.records, "dispatched": c.dispatched, "seconds": c.seconds});
        if let Some(f) = out.as_mut() {
            writeln!(f, "{line}").unwrap();
        }
    }
    eprintln!("eps sweep: loaded {loaded}, refused {refused}");
}

/// The import (src/import): the classify-only load (import::sweep) reads eps tables (frame, charts, labels as
/// (chart, index), domains, dispatcher, bodies parsed); a campaign load refuses them, typed, naming why.
#[test]
fn import_classify_only_loads_eps_tables_and_campaign_load_refuses() {
    use rustred_campaign_core::import::sweep::{ClassifyOptions, classify_table};
    use rustred_campaign_core::import::{TableSetSpec, load};
    let scratch = std::env::temp_dir().join(format!("eps_import_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let opts = ClassifyOptions {
        scratch: Some(scratch.clone()),
        ..Default::default()
    };
    for file in [
        "T012_a0-td-e.table.jsonl",
        "T041_min.table.jsonl",
        "T047_min.table.jsonl",
    ] {
        let c = classify_table(&data(file), &opts);
        assert_eq!(c.outcome, "loaded", "{file}: {:?} {:?}", c.reason, c.detail);
        let tf = read_table_file(data(file), ReadOptions::default()).unwrap();
        assert_eq!(c.records, tf.rules.len(), "{file}");
        assert_eq!(c.dispatched + c.not_dispatched, tf.rules.len(), "{file}");
    }
    // a campaign load (with a closure to come) refuses the eps family with a typed reason
    let start = scratch.join("start.jsonl");
    std::fs::write(&start, b"").unwrap();
    let spec = serde_json::json!({
        "schema": rustred_campaign_core::import::spec::SPEC_SCHEMA,
        "frame_from": "table",
        "start": {"path": start.display().to_string(), "format": "keys_jsonl"},
        "sources": [{"id": "table", "kind": "engine_table", "path": data("T041_min.table.jsonl").display().to_string()}],
    });
    let spec = TableSetSpec::from_json(&spec.to_string()).unwrap();
    match load(&spec, &scratch) {
        Err(e) => assert!(e.to_string().contains("campaign load"), "{e}"),
        Ok(_) => panic!("a campaign load of an eps family must refuse"),
    }
    std::fs::remove_dir_all(&scratch).ok();
}

/// THE CLOSURE WALK on eps tables (first consumer): forced axes never dropped, keys with an eps axis <= 0 inside the
/// node (never OutsideChart), the two-sided order (every followed edge strictly lower: the engine certified these
/// tables' descent), bodies matched by (chart, index). Start = the box of the engine facts; reference: the engine's
/// first match (rr_proof instantiate facts) at every reached key of that box, and the open keys are exactly the
/// table's residuals.
#[test]
fn closure_walk_on_eps_tables() {
    use rustred_campaign_core::closure::{
        ClosureConfig, HandDowns, KeyCodec, KeySet, NoFacts, OpenStatus, RuleBodies, Tier, closure,
    };
    use rustred_campaign_core::order::NodeComparator;
    use rustred_campaign_core::record::RhsMode;
    for (file, facts) in [
        ("T012_a0-td-e.table.jsonl", "T012.engine_first.jsonl"),
        ("T041_min.table.jsonl", "T041.engine_first.jsonl"),
        ("T047_min.table.jsonl", "T047.engine_first.jsonl"),
    ] {
        let tf = read_table_file(
            data(file),
            ReadOptions {
                rhs: RhsMode::KeysOnly,
            },
        )
        .unwrap();
        let header = tf.header.clone().unwrap();
        let frame = NodeFrame::from_header_eps_aware(&header).unwrap();
        let opts = DomainOptions {
            witness: false,
            ..Default::default()
        };
        let t = Table::from_table_file_eps_aware(file, file, &tf, opts).unwrap();
        let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
        let mut bodies = RuleBodies::new(&disp);
        let st = bodies.load_table_records(&disp, 0, &tf).unwrap();
        assert_eq!(
            st.rules,
            tf.rules.len(),
            "{file}: every record has its own body"
        );
        let cmp = NodeComparator::from_header(&header.raw).unwrap();
        let first = engine_first(facts);
        let codec = KeyCodec::new(frame.arity(), 8).unwrap();
        let start = KeySet::from_keys(codec, first.iter().map(|(k, _)| k.as_slice())).unwrap();
        let cfg = ClosureConfig {
            tier: Tier::K,
            threads: 2,
            bits: 8,
            hand_downs: HandDowns::Keys,
            table_digests: [(file.to_string(), "sha256:test".to_string())].into(),
            ..Default::default()
        };
        let cl = closure(&disp, &bodies, &cmp, &start, &cfg, &NoFacts).unwrap();
        let s = cl.summary();
        assert_eq!(s.descent_violations, 0, "{file}: two-sided order");
        assert_eq!(s.unrepresentable, 0, "{file}");
        assert!(!s.truncated, "{file}");
        // the open keys: exactly residuals of the table (uncovered), nothing outside the chart, no missing body
        let residuals: std::collections::BTreeSet<Vec<i32>> = std::fs::read_to_string(data(file))
            .unwrap()
            .lines()
            .filter_map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).ok()?;
                (v["type"] == "residual").then(|| {
                    rustred_campaign_core::key::parse_key(v["key"].as_str().unwrap()).unwrap()
                })
            })
            .collect();
        for o in cl.open_keys() {
            assert_eq!(
                o.status,
                OpenStatus::Uncovered,
                "{file} {:?}: {}",
                o.key,
                o.detail
            );
            assert!(
                residuals.contains(&o.key),
                "{file}: open non-residual {:?}",
                o.key
            );
        }
        // the walk lives on both sides of the eps axes
        let eps = frame.eps_axes();
        let nonpos = (0..cl.len())
            .filter(|&i| eps.iter().any(|&a| cl.key(i)[a] <= 0))
            .count();
        assert!(nonpos > 0, "{file}: no reached key with an eps axis <= 0");
        // winners agree with the engine's first match on the facts box
        let tab = &disp.tables()[0];
        let mut compared = 0;
        for (key, want) in &first {
            let Some(i) = cl.index_of(key) else { continue };
            let got = cl
                .winner(i)
                .map(|w| tab.eps_rule_of_label(w.label).expect("packed label"));
            assert_eq!(&got, want, "{file} {key:?}");
            compared += 1;
        }
        assert_eq!(compared, first.len(), "{file}");
        eprintln!(
            "{file}: keys {} (eps <= 0: {nonpos}), edges {}, open {}, hand-down children {}",
            s.keys_total,
            s.edges,
            s.open_total,
            s.hand_downs.len()
        );
    }
}

/// Small start sets for the import checks: every integer line at 1 or one of them at 2, the eps axes in -1..=2,
/// numerators at 0 or one of them at -1.
fn small_start(frame: &NodeFrame) -> Vec<Vec<i32>> {
    let n = frame.arity();
    let eps = frame.eps_axes();
    let lines: Vec<usize> = (0..n)
        .filter(|&i| frame.is_line(i) && !frame.is_eps_axis(i))
        .collect();
    let nums: Vec<usize> = (0..n).filter(|&i| !frame.is_line(i)).collect();
    let mut base = vec![0i32; n];
    for &l in &lines {
        base[l] = 1;
    }
    let mut out = vec![];
    let line_vars: Vec<Option<usize>> = std::iter::once(None)
        .chain(lines.iter().map(|&l| Some(l)))
        .collect();
    let num_vars: Vec<Option<usize>> = std::iter::once(None)
        .chain(nums.iter().map(|&l| Some(l)))
        .collect();
    let combos = 4usize.pow(eps.len() as u32);
    for lv in &line_vars {
        for nv in &num_vars {
            for c in 0..combos {
                let mut k = base.clone();
                if let Some(l) = lv {
                    k[*l] = 2;
                }
                if let Some(m) = nv {
                    k[*m] = -1;
                }
                let mut c = c;
                for &a in &eps {
                    k[a] = (c % 4) as i32 - 1;
                    c /= 4;
                }
                out.push(k);
            }
        }
    }
    out
}

const EPS_CHECK_TABLES: &[&str] = &[
    "T012_a0-td-e.table.jsonl",
    "T041_min.table.jsonl",
    "T047_min.table.jsonl",
];

/// The import spec of a one-table eps node: start = [`small_start`], table residuals as declared terminals, facts =
/// `facts` (rr_proof instantiate answers, byte offsets, bound by record_sha256).
fn eps_check(
    file: &str,
    facts: Option<&std::path::Path>,
    dir: &std::path::Path,
) -> rustred_campaign_core::import::CheckResult {
    eps_check_full(file, facts, dir).2
}

fn eps_check_full(
    file: &str,
    facts: Option<&std::path::Path>,
    dir: &std::path::Path,
) -> (
    rustred_campaign_core::import::ImportedSet,
    rustred_campaign_core::import::Evidence,
    rustred_campaign_core::import::CheckResult,
) {
    use rustred_campaign_core::import::load::load_with_eps;
    use rustred_campaign_core::import::{CheckOptions, Evidence, LoadOptions, TableSetSpec, check};
    let tf = read_table_file(data(file), ReadOptions::default()).unwrap();
    let frame = NodeFrame::from_header_eps_aware(tf.header.as_ref().unwrap()).unwrap();
    std::fs::create_dir_all(dir).unwrap();
    let start = dir.join(format!("{file}.start.jsonl"));
    let text: String = small_start(&frame)
        .iter()
        .map(|k| format!("{}\n", serde_json::to_string(k).unwrap()))
        .collect();
    std::fs::write(&start, text).unwrap();
    let mut spec = serde_json::json!({
        "schema": rustred_campaign_core::import::spec::SPEC_SCHEMA,
        "frame_from": "table",
        "start": {"path": start.display().to_string(), "format": "keys_jsonl"},
        "sources": [{"id": "table", "kind": "engine_table", "path": data(file).display().to_string(),
                     "table_terminals": "ledger_and_residuals"}],
    });
    if let Some(f) = facts {
        spec["evidence"] = serde_json::json!({"facts": [{"path": f.display().to_string()}]});
    }
    let spec = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load_with_eps(&spec, dir, &LoadOptions::default()).unwrap();
    assert!(set.frame.has_eps_axes());
    let ev = Evidence::from_spec(&set, dir).unwrap();
    let r = check(&set, &ev, &CheckOptions::default()).unwrap();
    (set, ev, r)
}

/// Writes the fact requests of the eps import checks (one JSONL per table) to EPSFRAME_REQ_DIR: the input of the
/// rr_proof run that produced `tests/data/eps/*.check_facts.jsonl` (see MANIFEST.sha256).
#[test]
#[ignore = "test-data producer: EPSFRAME_REQ_DIR=<dir>"]
fn eps_check_write_fact_requests() {
    let dir =
        std::path::PathBuf::from(std::env::var("EPSFRAME_REQ_DIR").expect("EPSFRAME_REQ_DIR"));
    for file in EPS_CHECK_TABLES {
        // iterate: facts answered so far (EPSFRAME_REQ_DIR/<file>.check_facts.jsonl) unlock further keys
        let facts = dir.join(format!("{file}.check_facts.jsonl"));
        let r = eps_check(file, facts.exists().then_some(facts.as_path()), &dir);
        let text: String = r
            .fact_requests
            .iter()
            .map(|q| format!("{}\n", serde_json::to_string(q).unwrap()))
            .collect();
        std::fs::write(dir.join(format!("{file}.fact_requests.jsonl")), text).unwrap();
        eprintln!(
            "{file}: {} records requested, verdict {:?}",
            r.fact_requests.len(),
            r.summary.verdict
        );
    }
}

/// A REAL CLOSURE CHECK of three small eps nodes through the import (load_with_eps -> check, Tier C): T012 (shift
/// 3 eps, 2L_eps, P5L_STATUS: certified by 18 certificates, 1 residual master), T041 / T047 (shift 1 / 2 eps, 3L_eps,
/// certified, 0 residuals). Facts = the engine's rr_proof instantiate answers to the check's own fact requests (byte
/// offsets, one job per chart, every answer bound by record_sha256: `split_req.py`, `iterate.sh`). Expected: Closed,
/// nothing open, the walk on both sides of the eps axes, the residual terminals = P5L_STATUS's residual masters.
#[test]
fn eps_import_check_closes_with_engine_facts() {
    use rustred_campaign_core::closure::Verdict;
    let dir = std::env::temp_dir().join(format!("eps_check_{}", std::process::id()));
    for (file, residual_masters) in [
        ("T012_a0-td-e.table.jsonl", 1u64),
        ("T041_min.table.jsonl", 0),
        ("T047_min.table.jsonl", 0),
    ] {
        let stem = file.trim_end_matches(".table.jsonl");
        // without facts: open (witness-absent records wait for their regular facts), with requests
        let r0 = eps_check(file, None, &dir);
        assert_eq!(r0.summary.verdict, Verdict::Open, "{file}");
        assert!(!r0.fact_requests.is_empty(), "{file}");
        // with the engine's facts: closed
        let facts = data(&format!("{stem}.check_facts.jsonl"));
        let (set, ev, r) = eps_check_full(file, Some(&facts), &dir);
        // the statement: every served row names its rule by (chart, index) besides label / ordinal / record_sha256
        let sdir = dir.join(format!("{stem}.statement"));
        rustred_campaign_core::import::write_statement(&set, &ev, &r, &sdir, None).unwrap();
        let tf_raw = read_table_file(data(file), ReadOptions::default()).unwrap();
        let mut served = 0;
        for l in std::fs::read_to_string(sdir.join("statement.jsonl"))
            .unwrap()
            .lines()
        {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            if v["status"] != "served" {
                continue;
            }
            let sv = &v["server"];
            let rec = &tf_raw.rules[sv["ordinal"].as_u64().unwrap() as usize];
            assert_eq!(sv["chart"].as_str(), rec.chart.as_deref(), "{file}: {sv}");
            assert_eq!(sv["index"].as_i64(), rec.index, "{file}: {sv}");
            assert!(
                sv["hand_name"].as_str().unwrap().ends_with(&format!(
                    "#{}:{}",
                    rec.chart.as_deref().unwrap(),
                    rec.index.unwrap()
                )),
                "{file}: {sv}"
            );
            served += 1;
        }
        assert!(served > 0, "{file}");
        let s = &r.summary;
        assert_eq!(s.verdict, Verdict::Closed, "{file}: {s:?}");
        assert_eq!(s.open_total, 0, "{file}");
        assert_eq!(s.descent_violations, 0, "{file}");
        assert!(r.fact_requests.is_empty(), "{file}");
        assert_eq!(
            s.terminals_other + s.terminals_preferred,
            residual_masters,
            "{file}"
        );
        let tf = read_table_file(data(file), ReadOptions::default()).unwrap();
        let frame = NodeFrame::from_header_eps_aware(tf.header.as_ref().unwrap()).unwrap();
        let eps = frame.eps_axes();
        assert_eq!(r.rows.len() as u64, s.keys_total, "{file}");
        let nonpos = r
            .rows
            .iter()
            .filter(|row| eps.iter().any(|&a| row.key[a] <= 0))
            .count();
        assert!(nonpos > 0, "{file}");
        eprintln!(
            "{file}: CLOSED, keys {} (eps <= 0: {nonpos}), edges {}, terminals {}, facts {} lines",
            s.keys_total,
            s.edges,
            s.terminals_other + s.terminals_preferred,
            std::fs::read_to_string(&facts).unwrap().lines().count()
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Guard-less chart lines (engine lane verdict 2026-11-15T03:22Z, interfaces/eps-families.md section 4): an absent
/// guard list means the guards of the chart's own mask; it is read on an explicit path and recorded
/// (`chart_guards_implied`). The same table with the guards removed from one chart line dispatches identically; an
/// explicit empty guard list is refused.
#[test]
fn guardless_chart_line_means_the_mask_guards() {
    use rustred_campaign_core::import::sweep::{ClassifyOptions, classify_table};
    let text = std::fs::read_to_string(data("T012_a0-td-e.table.jsonl")).unwrap();
    let stripped: String = text
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            if v["type"] == "chart" && v["chart"] == "01111" {
                let mut v = v.clone();
                v.as_object_mut().unwrap().remove("guards");
                format!("{v}\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    assert_ne!(stripped, text);
    let dir = std::env::temp_dir().join(format!("eps_guardless_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p_strip = dir.join("stripped.table.jsonl");
    std::fs::write(&p_strip, &stripped).unwrap();
    let opts = DomainOptions {
        witness: false,
        ..Default::default()
    };
    let tf_full =
        read_table_file(data("T012_a0-td-e.table.jsonl"), ReadOptions::default()).unwrap();
    let tf_strip = read_table_file(&p_strip, ReadOptions::default()).unwrap();
    let t_full = Table::from_table_file_eps_aware("f", "f", &tf_full, opts).unwrap();
    let t_strip = Table::from_table_file_eps_aware("s", "s", &tf_strip, opts).unwrap();
    assert!(t_full.provenance.chart_guards_implied.is_empty());
    assert_eq!(
        t_strip.provenance.chart_guards_implied,
        vec!["01111".to_string()]
    );
    let d_full = Dispatcher::new(vec![t_full], vec![0]).unwrap();
    let d_strip = Dispatcher::new(vec![t_strip], vec![0]).unwrap();
    let mut nonpos = 0;
    for (key, _) in engine_first("T012.engine_first.jsonl") {
        nonpos += usize::from(key[0] <= 0);
        assert_eq!(
            d_full.first_match(&key).unwrap(),
            d_strip.first_match(&key).unwrap(),
            "{key:?}"
        );
    }
    assert!(nonpos > 100);
    // the import's classify-only load reads it and reports it
    let c = classify_table(
        &p_strip,
        &ClassifyOptions {
            scratch: Some(dir.clone()),
            ..Default::default()
        },
    );
    assert_eq!(c.outcome, "loaded", "{:?}", c.detail);
    assert_eq!(c.chart_guards_implied, vec!["01111".to_string()]);
    // an explicit empty guard list is not a guard list: refused
    let empty: String = text.replace(
        r#""chart": "01111", "guards": ["#,
        r#""chart": "01111", "guards": [], "x": ["#,
    );
    if empty != text {
        let p_empty = dir.join("empty.table.jsonl");
        std::fs::write(&p_empty, &empty).unwrap();
        assert!(read_table_file(&p_empty, ReadOptions::default()).is_err());
    } else {
        let line = r#"{"type":"chart","chart":"01111","guards":[],"zero":false}"#;
        let mut rd = RecordReader::new(line.as_bytes(), ReadOptions::default());
        assert!(rd.next().unwrap().is_err());
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Keys of the node's charts whose order weight is at most `layer`: sum over integer lines of (n - 1), over numerators
/// of (-n), over eps axes of the two-sided excess e(n) (n = 1, 0, 2, -1, ... -> 0, 1, 2, 3, ...;
/// `rustred_order::two_sided_excess`, the comparator's own measure). Generic in the frame (no node knowledge).
fn layer_keys(frame: &NodeFrame, layer: u32) -> Vec<Vec<i32>> {
    let n = frame.arity();
    // per slot: the values with their weights, by increasing weight
    let slot_values = |i: usize, w: u32| -> i32 {
        if frame.is_eps_axis(i) {
            // inverse of e(n): 0 -> 1, 1 -> 0, 2 -> 2, 3 -> -1, ...
            if w % 2 == 0 {
                1 + (w / 2) as i32
            } else {
                -((w / 2) as i32)
            }
        } else if frame.is_line(i) {
            1 + w as i32
        } else {
            -(w as i32)
        }
    };
    let mut out = vec![];
    let mut cur = vec![0u32; n];
    fn rec(
        i: usize,
        left: u32,
        cur: &mut Vec<u32>,
        out: &mut Vec<Vec<i32>>,
        f: &dyn Fn(usize, u32) -> i32,
    ) {
        if i == cur.len() {
            out.push((0..cur.len()).map(|s| f(s, cur[s])).collect());
            return;
        }
        for w in 0..=left {
            cur[i] = w;
            rec(i + 1, left - w, cur, out, f);
        }
        cur[i] = 0;
    }
    rec(0, layer, &mut cur, &mut out, &slot_values);
    out.sort();
    out
}

#[test]
fn layer_keys_are_the_weight_layers() {
    let f = NodeFrame::from_header_eps_aware(&h5(
        r#"["(4-d)/2","0","0","0","0"]"#,
        "[true,false,false,false,false]",
    ))
    .unwrap();
    let l0 = layer_keys(&f, 0);
    assert_eq!(l0, vec![vec![1, 1, 1, 1, 0]]);
    let l1 = layer_keys(&f, 1);
    assert_eq!(l1.len(), 1 + 5);
    assert!(l1.contains(&vec![0, 1, 1, 1, 0])); // eps axis at 0 (weight 1)
    assert!(l1.contains(&vec![1, 1, 1, 1, -1]));
    assert!(layer_keys(&f, 3).contains(&vec![-1, 1, 1, 1, 0])); // eps axis at -1 (weight 3)
}

fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).map(|x| x.parse().unwrap_or(0)))
        })
        .unwrap_or(0)
}

/// ONE NODE OF THE EPS CAMPAIGN SWEEP (driver: tests/data/eps/campaign.sh; lane epsframe). Env: EPSC_NODE, EPSC_TABLE,
/// EPSC_DIR (work dir), EPSC_LAYER (start = [`layer_keys`]), EPSC_MODE = `k` (keys-only superset: witness check off,
/// diagnostic) | `c` (strict, Tier C with EPSC_FACTS), EPSC_OUT (one JSON result line appended). Writes the check's
/// fact requests to EPSC_DIR/requests.jsonl.
#[test]
#[ignore = "eps campaign step (driver campaign.sh)"]
fn eps_campaign_node() {
    use rustred_campaign_core::import::load::load_with_eps;
    use rustred_campaign_core::import::{CheckOptions, Evidence, LoadOptions, TableSetSpec, check};
    use std::io::Write;
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k}"));
    let (node, table, dir, mode) = (
        env("EPSC_NODE"),
        env("EPSC_TABLE"),
        env("EPSC_DIR"),
        env("EPSC_MODE"),
    );
    let layer: u32 = env("EPSC_LAYER").parse().unwrap();
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let t0 = std::time::Instant::now();
    // the start set is a PROBE box chosen by this lane (not the campaign's box): every line says so
    let mut line = serde_json::json!({"node": node, "table": table, "mode": mode, "layer": layer,
        "box": format!("probe(weight<={layer})")});
    let res = (|| -> Result<(), String> {
        let head = {
            let f = std::fs::File::open(&table).map_err(|e| e.to_string())?;
            let mut s = String::new();
            std::io::BufRead::read_line(&mut std::io::BufReader::new(f), &mut s)
                .map_err(|e| e.to_string())?;
            s
        };
        let frame =
            NodeFrame::from_header_eps_aware(&header_of(&head)).map_err(|e| e.to_string())?;
        let start = layer_keys(&frame, layer);
        let sp = dir.join("start.jsonl");
        let text: String = start
            .iter()
            .map(|k| format!("{}\n", serde_json::to_string(k).unwrap()))
            .collect();
        std::fs::write(&sp, text).map_err(|e| e.to_string())?;
        let mut spec = serde_json::json!({
            "schema": rustred_campaign_core::import::spec::SPEC_SCHEMA,
            "frame_from": "table",
            "start": {"path": sp.display().to_string(), "format": "keys_jsonl"},
            "sources": [{"id": "table", "kind": "engine_table", "path": table, "table_terminals": "ledger_and_residuals"}],
        });
        if mode == "k" {
            spec["policies"] = serde_json::json!({"witness_check": "off_diagnostic", "tier": "K"});
        } else if let Ok(f) = std::env::var("EPSC_FACTS") {
            spec["evidence"] = serde_json::json!({"facts": [{"path": f}]});
        }
        let spec = TableSetSpec::from_json(&spec.to_string()).map_err(|e| e.to_string())?;
        let set = load_with_eps(&spec, &dir, &LoadOptions::default())
            .map_err(|e| format!("load: {e}"))?;
        let ev = Evidence::from_spec(&set, &dir).map_err(|e| format!("evidence: {e}"))?;
        let r = check(&set, &ev, &CheckOptions::default()).map_err(|e| format!("check: {e}"))?;
        let s = &r.summary;
        let eps = frame.eps_axes();
        let nonpos = r
            .rows
            .iter()
            .filter(|row| eps.iter().any(|&a| row.key[a] <= 0))
            .count();
        let req_keys: usize = r.fact_requests.iter().map(|q| q.keys.len()).sum();
        let text: String = r
            .fact_requests
            .iter()
            .map(|q| format!("{}\n", serde_json::to_string(q).unwrap()))
            .collect();
        std::fs::write(dir.join("requests.jsonl"), text).map_err(|e| e.to_string())?;
        line["loaded"] = serde_json::json!(true);
        line["records"] = serde_json::json!(set.rules.iter().map(Vec::len).sum::<usize>());
        line["start"] = serde_json::json!(start.len());
        line["verdict"] = serde_json::to_value(s.verdict).unwrap();
        line["keys"] = serde_json::json!(s.keys_total);
        line["keys_eps_nonpos"] = serde_json::json!(nonpos);
        line["open"] = serde_json::json!(s.open_total);
        line["open_by_status"] = serde_json::json!(s.open_by_status);
        line["terminals"] = serde_json::json!(s.terminals_other + s.terminals_preferred);
        line["descent_violations"] = serde_json::json!(s.descent_violations);
        line["request_records"] = serde_json::json!(r.fact_requests.len());
        line["request_keys"] = serde_json::json!(req_keys);
        Ok(())
    })();
    if let Err(e) = res {
        line["loaded"] = serde_json::json!(false);
        line["error"] = serde_json::json!(e);
    }
    line["seconds"] = serde_json::json!(t0.elapsed().as_secs_f64());
    line["peak_rss_kb"] = serde_json::json!(vm_hwm_kb());
    let out = env("EPSC_OUT");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out)
        .unwrap();
    writeln!(f, "{line}").unwrap();
}
