//! Lane partfix4 (implementer question, 2026-11-15): do the readers of a rule record's `domain_bounds` agree on ONE real
//! table at every key that matters? IGNORED (needs a multi-GB table): run with
//!   PF4_SHIP_TABLE=<table> PF4_KEYS=<keys.jsonl, one JSON array per line> PF4_OUT=<dir> cargo test --release
//!   --test t162_ship_readers -- --ignored --nocapture
//! Per key it writes (PF4_OUT/readers.jsonl): the CORE dispatcher's first SELECTED record (this crate: `domain_bounds` =
//! the engine's exception branches, `domain::bound_branches`; Covered, or Blocked / Undecided `at`) and the first record
//! under the PROOF KERNEL's reading, ported line by line from rustred-proof 28dcc701 instantiate.rs:155-160 (the key
//! given is the case-variable vector kv: `in_case` = fixed slots equal kv, case equations vanish at kv, kv inside the
//! INCLUSIVE `domain_bounds` (core.rs:145 `in_bounds`); `chart_ok` = the pattern at kv (span.rs:590 `pat_at`) in the
//! chart; `branch` = the first written exception branch all of whose members vanish at kv). It also writes
//! PF4_OUT/rrproof_requests.jsonl (an `rr_proof instantiate` request sample for every bounded record: served keys and
//! the keys just inside / outside each bound) with the port's predictions (PF4_OUT/rrproof_expected.jsonl), so that the
//! real kernel can validate the port. Nothing node-specific in the code (lane rule L8): the table and keys are inputs.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::{DomainOptions, TargetSlot, bound_branches, parse_target};
use rustred_campaign_core::import::lines::scan_table;
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::record::TableFile;

#[test]
#[ignore]
fn t162_ship_three_readers() {
    let table = PathBuf::from(std::env::var("PF4_SHIP_TABLE").expect("PF4_SHIP_TABLE"));
    let keys_path = PathBuf::from(std::env::var("PF4_KEYS").expect("PF4_KEYS"));
    let out = PathBuf::from(std::env::var("PF4_OUT").expect("PF4_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let t0 = std::time::Instant::now();
    let raw = scan_table("ship", &table, false, &|_, _| true).expect("scan");
    eprintln!(
        "scanned {} records in {:.1} s",
        raw.records.len(),
        t0.elapsed().as_secs_f64()
    );
    let frame = NodeFrame::from_header(&raw.header).expect("frame");
    let n = frame.arity();
    let mask: Vec<bool> = (0..n).map(|i| frame.is_line(i)).collect();
    // members outside the closed list (record::RULE_RECORD_MEMBERS) are REPORTED and cleared here: this test compares
    // the readers' DISPATCH (fixed slots, target, equations, exceptions, domain_bounds); a member the closed list does
    // not know is a separate finding (summary `unknown_members`)
    let mut unknown: BTreeMap<String, usize> = BTreeMap::new();
    for r in &raw.records {
        for m in &r.rec.unknown_members {
            *unknown.entry(m.clone()).or_default() += 1;
        }
    }
    eprintln!("unknown members (cleared for the comparison): {unknown:?}");
    let tf = TableFile {
        header: Some(raw.header.clone()),
        rules: raw
            .records
            .iter()
            .map(|r| {
                let mut x = r.rec.clone();
                x.unknown_members.clear();
                x
            })
            .collect(),
        charts: raw.charts.clone(),
        others: vec![],
    };
    let t = Table::from_table_file(
        "ship",
        table.display().to_string(),
        &tf,
        DomainOptions::default(),
    )
    .expect("core load");
    assert_eq!(t.len(), raw.records.len(), "every record dispatched");
    let disp = Dispatcher::new(vec![t], vec![0])
        .unwrap()
        .with_chart(mask.clone())
        .unwrap();
    // the proof kernel's reading of each record (in file order = dispatch order)
    struct Kr {
        label: i64,
        fixed: Vec<Option<i64>>,
        pat: Vec<(bool, i64)>,
        min: Vec<Option<i64>>,
        max: Vec<Option<i64>>,
        written_exc: usize,
        offset: u64,
        bounded: bool,
        shape: String,
    }
    let dom = &disp.tables()[0].domains;
    let mut kr: Vec<Kr> = vec![];
    let mut shapes: BTreeMap<String, usize> = BTreeMap::new();
    for (i, r) in raw.records.iter().enumerate() {
        let rec = &r.rec;
        let slots = parse_target(rec.target.as_deref().unwrap(), n).unwrap();
        // rr_proof core.rs:367-378: `n<i>` only at slot i (else the kernel refuses), an offset, or a value
        let mut kernel_reads = true;
        let pat: Vec<(bool, i64)> = slots
            .iter()
            .enumerate()
            .map(|(j, s)| match *s {
                TargetSlot::Param { param, offset } => {
                    if param != j {
                        kernel_reads = false;
                    }
                    (true, offset)
                }
                TargetSlot::Const(c) => (false, c),
            })
            .collect();
        let (min, max, nb) = match &rec.domain_bounds {
            Some(b) => {
                let chart: Vec<bool> = rec
                    .chart
                    .as_deref()
                    .unwrap()
                    .chars()
                    .map(|c| c == '1')
                    .collect();
                let nb = bound_branches(b, &chart, &rec.fixed).unwrap().len();
                (b.min.clone(), b.max.clone(), nb)
            }
            None => (vec![None; n], vec![None; n], 0),
        };
        let bounded = rec.domain_bounds.is_some();
        // shape: canonical (n_j at free slot j, the fixed value as constant at a fixed slot), offsets, permutation;
        // bounds on free / fixed slots
        let canonical = slots
            .iter()
            .enumerate()
            .all(|(j, s)| match (*s, rec.fixed[j]) {
                (TargetSlot::Param { param, offset: 0 }, _) => param == j,
                (TargetSlot::Const(c), Some(f)) => c == f,
                _ => false,
            });
        let consts = slots
            .iter()
            .filter(|s| matches!(s, TargetSlot::Const(_)))
            .count();
        let bound_on_fixed =
            (0..n).any(|j| rec.fixed[j].is_some() && (min[j].is_some() || max[j].is_some()));
        let shape = format!(
            "bounds={} target={} consts={} bound_on_fixed_slot={} kernel_reads={}",
            if bounded { "yes" } else { "no" },
            if canonical { "canonical" } else { "pattern" },
            if consts > 0 { "yes" } else { "no" },
            bound_on_fixed,
            kernel_reads
        );
        *shapes.entry(shape.clone()).or_default() += 1;
        assert_eq!(dom[i].exceptions.len(), rec.exceptions.len() + nb);
        kr.push(Kr {
            label: rec.index.unwrap(),
            fixed: rec.fixed.clone(),
            pat,
            min,
            max,
            written_exc: rec.exceptions.len(),
            offset: r.offset,
            bounded,
            shape,
        });
    }
    eprintln!("shapes: {shapes:#?}");
    let kernel_first = |k: &[i32], i: usize| -> bool {
        let r = &kr[i];
        let kv: Vec<i64> = k.iter().map(|&x| i64::from(x)).collect();
        let d = &dom[i];
        let in_case = (0..n).all(|s| r.fixed[s].is_none_or(|f| f == kv[s]))
            && d.equations.iter().all(|e| e.eval(k).unwrap().is_zero())
            && (0..n).all(|s| {
                r.min[s].is_none_or(|m| kv[s] >= m) && r.max[s].is_none_or(|m| kv[s] <= m)
            });
        let tv: Vec<i64> = r
            .pat
            .iter()
            .enumerate()
            .map(|(s, &(sym, o))| if sym { kv[s] + o } else { o })
            .collect();
        let chart_ok = (0..n).all(|s| if mask[s] { tv[s] >= 1 } else { tv[s] <= 0 });
        let branch = d.exceptions[..r.written_exc]
            .iter()
            .any(|br| !br.is_empty() && br.iter().all(|p| p.eval(k).unwrap().is_zero()));
        in_case && chart_ok && !branch
    };
    let keys: Vec<Vec<i32>> = std::fs::read_to_string(&keys_path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let nobounds_on = std::env::var_os("PF4_NOBOUNDS").is_some();
    let mut w = std::io::BufWriter::new(std::fs::File::create(out.join("readers.jsonl")).unwrap());
    let (mut agree, mut differ) = (0usize, 0usize);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut served: BTreeMap<usize, Vec<Vec<i32>>> = BTreeMap::new();
    for k in &keys {
        let (core, kind) = match disp.first_match(k).unwrap() {
            Dispatch::Covered(r) => (Some(r.record), "covered".to_string()),
            Dispatch::Blocked { at, reason } => (Some(at.record), format!("blocked:{reason:?}")),
            Dispatch::Undecided { at, verdict } => {
                (Some(at.record), format!("undecided:{verdict:?}"))
            }
            Dispatch::Uncovered => (None, "uncovered".to_string()),
            Dispatch::OutsideChart { .. } => (None, "outside_chart".to_string()),
        };
        let kind_word = kind
            .split(':')
            .next()
            .unwrap()
            .split('(')
            .next()
            .unwrap()
            .to_string();
        *kinds.entry(kind_word).or_default() += 1;
        let kern = (0..kr.len()).find(|&i| kernel_first(k, i));
        // optional: the first record SELECTED when every record's domain_bounds branches are ignored
        let nobounds = nobounds_on.then(|| {
            (0..kr.len()).find(|&i| {
                let mut d = dom[i].clone();
                d.exceptions.truncate(kr[i].written_exc);
                d.applies(k).stage() != rustred_campaign_core::domain::Stage::NotSelected
            })
        });
        if core == kern {
            agree += 1;
        } else {
            differ += 1;
        }
        if let Some(i) = core {
            served.entry(i).or_default().push(k.clone());
        }
        let lab = |x: Option<usize>| x.map(|i| kr[i].label);
        writeln!(
            w,
            "{}",
            serde_json::json!({"key": k, "core": lab(core), "core_kind": kind, "kernel": lab(kern),
                "nobounds": nobounds.map(lab)})
        )
        .unwrap();
    }
    w.flush().unwrap();
    // the validation sample for the real kernel: every bounded record with served keys, its served keys (<= 3) and
    // for each bounded slot the keys just inside and just outside the bound (from its first served key)
    let mut req =
        std::io::BufWriter::new(std::fs::File::create(out.join("rrproof_requests.jsonl")).unwrap());
    let mut exp =
        std::io::BufWriter::new(std::fs::File::create(out.join("rrproof_expected.jsonl")).unwrap());
    let mut n_req_keys = 0usize;
    for (&i, ks) in &served {
        let r = &kr[i];
        if !r.bounded {
            continue;
        }
        let mut sample: Vec<Vec<i32>> = ks.iter().take(3).cloned().collect();
        let base = ks[0].clone();
        for s in 0..n {
            for v in [r.min[s].map(|m| [m - 1, m]), r.max[s].map(|m| [m, m + 1])]
                .into_iter()
                .flatten()
                .flatten()
            {
                let mut k2 = base.clone();
                k2[s] = i32::try_from(v).unwrap();
                if (mask[s] && k2[s] >= 1) || (!mask[s] && k2[s] <= 0) {
                    sample.push(k2);
                }
            }
        }
        sample.sort();
        sample.dedup();
        for k in &sample {
            writeln!(
                exp,
                "{}",
                serde_json::json!({"rule": r.label, "key": k, "ok": kernel_first(k, i)})
            )
            .unwrap();
        }
        n_req_keys += sample.len();
        writeln!(
            req,
            "{}",
            serde_json::json!({"table": table.display().to_string(), "offset": r.offset, "rule": r.label, "keys": sample})
        )
        .unwrap();
    }
    req.flush().unwrap();
    exp.flush().unwrap();
    let summary = serde_json::json!({
        "table": table.display().to_string(), "records": kr.len(), "keys": keys.len(), "unknown_members": unknown,
        "core_vs_kernel_agree": agree, "core_vs_kernel_differ": differ, "core_kinds": kinds,
        "shapes": shapes, "rrproof_sample_records": served.keys().filter(|&&i| kr[i].bounded).count(),
        "rrproof_sample_keys": n_req_keys, "seconds": t0.elapsed().as_secs_f64(),
        "shape_of_each_bounded_record_kept": kr.iter().filter(|r| r.bounded).map(|r| r.shape.clone()).collect::<std::collections::BTreeSet<_>>(),
    });
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
    eprintln!("{summary}");
}
