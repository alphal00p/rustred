//! Ported from audit-contract-B (t4_classifier_matrix.rs; lane contractfix). Regression baseline: 0 panics, 0 unsafe
//! accepts, 0 differences from the document's table; the oracle is updated to the revised table (child-job.md 6.5
//! after lane contractfix: rows 7b, 21b/c, 25-27, 128+N as signals, codes outside the kind's set, S = verified at the
//! layout path with every OUTCOME figure). Ignored by default; run in release in a pid namespace:
//! `LIGHT_PIDNS=1 light_build.sh <lane> test --release -p rustred-campaign --test fake_child_grid -- --ignored`.
//!
//! audit-contract-B experiment 4: the CLASSIFIER over a systematic grid of real run directories
//! W (exit codes of the table + 1,2,9,101,126,127,128+N,255; signals 1,2,6,7,9,11,15,24; unknown) x K (5) x
//! OUTCOME (absent / unparsable / every (exit, stop) pair, valid or not by check(), rules 0/1, per_case variants,
//! elided loci) x table (absent / unsealed / sealed-valid / sealed-digest-mismatch) x RESUME (absent / valid).
//! Checks: totality (no panic), safety (accept only when W, OUTCOME and a verified seal agree), and an independent
//! oracle written from child-job.md 6.5 + its "precisions" (disagreements are listed in results/t4_disagree.txt).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod fake_child_common;
use fake_child_common::drill::*;
use rustred_campaign_contract::classify::{Accept, KillReason, WaitStatus, assess, classify};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::Outcome;
use rustred_campaign_contract::stop::StopCode;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
enum T {
    Absent,
    Unsealed,
    Sealed,
    Mismatch,
}

fn status_for(c: i32) -> &'static str {
    match c {
        0 => "complete",
        10 => "incomplete",
        11 => "decided_negative",
        12 => "undecided",
        20..=22 => "budget",
        30 => "interrupted",
        40 | 42 => "refused",
        _ => "error",
    }
}

#[derive(Clone, Debug)]
struct OVar {
    label: String,
    json: Option<Vec<u8>>,
    exit: Option<i32>,
    stop: Option<String>,
    rules: u64,
    per_case: Vec<&'static str>,
    elided: u64,
}

/// Header, `rules` rule records, end record (the table agrees with OUTCOME's rule count).
fn table_lines(end_kind: &str, rules: u64) -> Vec<String> {
    let mut v = vec!["{\"type\":\"header\",\"x\":1}".to_string()];
    for i in 0..rules {
        v.push(format!("{{\"type\":\"rule\",\"index\":{i},\"rhs\":[]}}"));
    }
    v.push(format!("{{\"type\":\"{end_kind}\",\"rules\":{rules}}}"));
    v
}

fn join(lines: &[String]) -> Vec<u8> {
    lines
        .iter()
        .flat_map(|l| format!("{l}\n").into_bytes())
        .collect()
}

fn canon_of(lines: &[String]) -> String {
    let mut d = rustred_campaign_contract::canon::CanonicalDigest::new();
    for l in lines {
        d.push_line(l.as_bytes()).unwrap();
    }
    d.hex()
}

#[test]
#[ignore = "grid: release + LIGHT_PIDNS"]
fn classifier_grid() {
    // template OUTCOME from a real fake-child run
    let root = fresh("t4tpl");
    let rd0 = rustred_campaign_contract::layout::attempt_dir(&root, "j", 1);
    write_spec(
        &rd0,
        &spec_json(
            &rd0,
            "j",
            1,
            json!({"script":[rule_case(0,1,None),{"op":"end"}]}),
        ),
    );
    assert_eq!(
        run_to_end(&rd0, &SpawnOpt::default(), Duration::from_secs(20)),
        WaitStatus::Exited(0)
    );
    let tpl: Value =
        serde_json::from_slice(&std::fs::read(RunLayout::new(&rd0).outcome()).unwrap()).unwrap();

    let codes = [0, 10, 11, 12, 20, 21, 22, 30, 40, 41, 42, 50, 51, 70];
    let stops: Vec<Option<String>> = std::iter::once(None)
        .chain(StopCode::ALL.iter().map(|s| {
            Some(
                serde_json::to_value(s)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            )
        }))
        .collect();
    let mut ovars = vec![
        OVar {
            label: "absent".into(),
            json: None,
            exit: None,
            stop: None,
            rules: 0,
            per_case: vec![],
            elided: 0,
        },
        OVar {
            label: "unparsable".into(),
            json: Some(b"{\"job\": {".to_vec()),
            exit: None,
            stop: None,
            rules: 0,
            per_case: vec![],
            elided: 0,
        },
    ];
    for &c in &codes {
        for s in &stops {
            let variants: Vec<(u64, Vec<&'static str>)> =
                if c == 10 && s.as_deref() == Some("max_cases") {
                    vec![
                        (1, vec![]),
                        (0, vec![]),
                        (0, vec!["covered"]),
                        (0, vec!["rule"]),
                        (0, vec!["covered", "refused"]),
                    ]
                } else {
                    vec![(1, vec![])]
                };
            for (rules, pc) in variants {
                ovars.push(OVar {
                    label: format!("exit {c} stop {s:?} rules {rules} per_case {pc:?}"),
                    json: Some(vec![]),
                    exit: Some(c),
                    stop: s.clone(),
                    rules,
                    per_case: pc,
                    elided: 0,
                });
            }
        }
    }
    ovars.push(OVar {
        label: "exit 0 elided 3".into(),
        json: Some(vec![]),
        exit: Some(0),
        stop: None,
        rules: 1,
        per_case: vec![],
        elided: 3,
    });
    // valid-looking but wrong status
    ovars.push(OVar {
        label: "exit 0 status interrupted (inconsistent)".into(),
        json: Some(b"STATUS".to_vec()),
        exit: Some(0),
        stop: None,
        rules: 1,
        per_case: vec![],
        elided: 0,
    });

    let mut waits: Vec<WaitStatus> = codes.iter().map(|&c| WaitStatus::Exited(c)).collect();
    for c in [1, 2, 9, 101, 126, 127, 129, 130, 137, 143, 255] {
        waits.push(WaitStatus::Exited(c));
    }
    for s in [1, 2, 6, 7, 9, 11, 15, 24] {
        waits.push(WaitStatus::Signaled(s));
    }
    waits.push(WaitStatus::Unknown);
    let kills = [
        KillReason::None,
        KillReason::Time,
        KillReason::Memory,
        KillReason::Stop,
        KillReason::Unresponsive,
    ];
    let tables = [T::Absent, T::Unsealed, T::Sealed, T::Mismatch];

    let mut cells = 0u64;
    let mut valid_o = 0u64;
    let mut unsafe_cells = vec![];
    let mut disagree: BTreeMap<String, u64> = BTreeMap::new();
    let mut rows: BTreeMap<u8, u64> = BTreeMap::new();
    let mut panics = 0u64;
    let base = fresh("t4");
    let mut n = 0u64;
    for ov in &ovars {
        for &tb in &tables {
            if ov.json.is_none() && tb != T::Absent {
                continue;
            } // table irrelevant without OUTCOME? still test one
            for resume in [false, true] {
                n += 1;
                let rd = base.join(format!("c{n}"));
                std::fs::create_dir_all(rd.join("out")).unwrap();
                write_spec(
                    &rd,
                    &spec_json(&rd, "j", 1, json!({"script":[{"op":"end"}]})),
                );
                let sdg = spec_digest(&rd);
                let l = RunLayout::new(&rd);
                let end_kind = if ov.stop.is_none() { "end" } else { "partial" };
                let tl = table_lines(end_kind, ov.rules);
                let tbytes = join(&tl);
                let digest = rustred_campaign_contract::blake3_hex(&tbytes);
                match tb {
                    T::Absent => {}
                    T::Unsealed => std::fs::write(l.table(), &tbytes[..tbytes.len() - 5]).unwrap(),
                    T::Sealed => std::fs::write(l.table(), &tbytes).unwrap(),
                    T::Mismatch => {
                        let mut b = tbytes.clone();
                        b[12] ^= 1;
                        std::fs::write(l.table(), &b).unwrap()
                    }
                }
                if let Some(j) = &ov.json {
                    let bytes = if j.is_empty() || j == b"STATUS" {
                        let mut o = tpl.clone();
                        let c = ov.exit.unwrap();
                        o["job"]["exit_code"] = json!(c);
                        o["job"]["spec_blake3"] = json!(sdg);
                        o["job"]["status"] = json!(if j == b"STATUS" {
                            "interrupted"
                        } else {
                            status_for(c)
                        });
                        o["job"]["stop"]["code"] = json!(ov.stop);
                        o["job"]["time_decided"] = json!(ov.stop.as_deref() == Some("budget_wall"));
                        o["job"]["counters"]["rules_committed"] = json!(ov.rules);
                        o["job"]["per_case"] = json!(ov.per_case.iter().enumerate().map(|(i, r)| json!({"ordinal":i,"case":{},"result":r,"rule":null,"children_open":0})).collect::<Vec<_>>());
                        let t = &mut o["job"]["outputs"]["table"];
                        t["rules_committed"] = json!(ov.rules);
                        t["witness_loci_elided"] = json!(ov.elided);
                        if tb == T::Unsealed {
                            t["sealed"] = json!(false);
                            for k in [
                                "bytes",
                                "blake3",
                                "records",
                                "rules",
                                "end",
                                "canonical_blake3",
                            ] {
                                t.as_object_mut().unwrap().remove(k);
                            }
                        } else {
                            t["sealed"] = json!(true);
                            t["bytes"] = json!(tbytes.len());
                            t["blake3"] = json!(digest);
                            t["records"] = json!(tl.len());
                            t["rules"] = json!(ov.rules);
                            t["canonical_blake3"] = json!(canon_of(&tl));
                            t["end"] = json!(end_kind);
                        }
                        serde_json::to_vec_pretty(&o).unwrap()
                    } else {
                        j.clone()
                    };
                    std::fs::write(l.outcome(), bytes).unwrap();
                }
                if resume {
                    // RESUME at the commit point before the end record (valid iff the table file is the sealed one)
                    let pre = &tl[..tl.len() - 1];
                    let pb = join(pre);
                    let r = json!({"schema":"rustred.child-job.resume.v1","job_id":"j","attempt":1,"spec_blake3":sdg,
                        "chart":"C","frame":"PRESENTED","rules_committed":ov.rules,
                        "table":{"path":"out/F.table.jsonl","bytes":pb.len(),"blake3":rustred_campaign_contract::blake3_hex(&pb),
                                 "records":pre.len(),"rules":ov.rules,"canonical_blake3":canon_of(pre)},
                        "current":null,"pending":[],"counters_at_commit":{"rules_committed":ov.rules}});
                    std::fs::write(l.resume(), serde_json::to_vec(&r).unwrap()).unwrap();
                }
                let spec = parse_spec(&rd);
                // O validity as the crate sees it, for the safety check
                let o_parsed = std::fs::read(l.outcome())
                    .ok()
                    .and_then(|b| Outcome::from_slice(&b).ok());
                let o_valid = o_parsed
                    .as_ref()
                    .is_some_and(|o| o.job.check().is_ok() && o.job.matches(&spec, &sdg));
                if o_valid {
                    valid_o += 1;
                }
                for &w in &waits {
                    for &k in &kills {
                        cells += 1;
                        let e = assess(&l, &spec, &sdg, w, k, false);
                        let res = std::panic::catch_unwind(|| classify(&e));
                        let Ok(c) = res else {
                            panics += 1;
                            continue;
                        };
                        *rows.entry(c.row).or_default() += 1;
                        // SAFETY property
                        if c.accept != Accept::Nothing {
                            let oc = o_parsed.as_ref().map(|o| o.job.exit_code.code());
                            let w_ok = match w {
                                WaitStatus::Exited(x) => Some(x) == oc,
                                WaitStatus::Unknown => true,
                                _ => false,
                            };
                            let ok = o_valid
                                && w_ok
                                && tb == T::Sealed
                                && matches!(oc, Some(0 | 10 | 20))
                                && ov.elided == 0;
                            if !ok {
                                unsafe_cells.push(format!(
                                    "{} T={tb:?} R={resume} W={w:?} K={k:?} -> row {} {:?}",
                                    ov.label, c.row, c.accept
                                ));
                            }
                        }
                        // ORACLE from the document
                        let exp =
                            oracle(w, k, ov, o_valid, tb == T::Sealed && ov.elided == 0, resume);
                        if exp != c.row {
                            let key = format!(
                                "W={} O={} T={tb:?} K={k:?}: doc row {exp} impl row {}",
                                wclass(w),
                                oclass(ov, o_valid),
                                c.row
                            );
                            *disagree.entry(key).or_default() += 1;
                        }
                    }
                }
                let _ = std::fs::remove_dir_all(&rd);
            }
        }
    }
    let mut s = format!(
        "cells {cells}\nrun dirs {n}\nvalid-O dirs {valid_o}\npanics {panics}\nunsafe {}\nrows {rows:?}\n",
        unsafe_cells.len()
    );
    for u in unsafe_cells.iter().take(50) {
        s.push_str(&format!("UNSAFE {u}\n"));
    }
    for (k, v) in &disagree {
        s.push_str(&format!("DISAGREE x{v} {k}\n"));
    }
    std::fs::create_dir_all(scratch().join("results")).unwrap();
    std::fs::write(scratch().join("results/t4_classifier.txt"), &s).unwrap();
    eprintln!("{}", &s[..s.len().min(6000)]);
    assert_eq!(panics, 0);
    assert!(unsafe_cells.is_empty());
    assert!(
        disagree.is_empty(),
        "classifier differs from the document's table"
    );
}

fn wclass(w: WaitStatus) -> String {
    format!("{w:?}")
}
fn oclass(o: &OVar, valid: bool) -> String {
    format!("{}{}", o.label, if valid { "" } else { " [invalid]" })
}

/// child-job.md 6.5 as revised by lane contractfix (kind `fake`: a producing kind with a sealing rule), written
/// independently of classify.rs. `sealed` = the table is the sealed file AND agrees with every OUTCOME figure.
fn oracle(w: WaitStatus, k: KillReason, ov: &OVar, o_valid: bool, sealed: bool, r: bool) -> u8 {
    let _ = r;
    let mut ovalid = o_valid;
    let present = ov.json.is_some();
    let w = match w {
        WaitStatus::Exited(c) if (129..=192).contains(&c) => WaitStatus::Signaled(c - 128),
        x => x,
    };
    if ovalid
        && let (WaitStatus::Exited(c), Some(e)) = (w, ov.exit)
        && c != e
    {
        ovalid = false;
    }
    let w = match w {
        WaitStatus::Unknown if ovalid => WaitStatus::Exited(ov.exit.unwrap()),
        WaitStatus::Unknown if !present => return 21,
        WaitStatus::Unknown => return 22,
        x => x,
    };
    let stop = ov.stop.as_deref();
    if let WaitStatus::Exited(c) = w {
        if ovalid {
            if matches!(c, 0 | 10 | 20) {
                if !sealed {
                    return 22;
                }
                return match (c, stop) {
                    (0, _) => 1,
                    (10, Some("max_cases")) => {
                        let nr = ["covered", "empty", "finite", "declared_terminal"];
                        if ov.rules >= 1 {
                            2
                        } else if !ov.per_case.is_empty()
                            && ov.per_case.iter().all(|p| nr.contains(p))
                        {
                            3
                        } else {
                            24
                        }
                    }
                    (
                        10,
                        Some(
                            "lift_refused"
                            | "guard_timeout"
                            | "numeric_search_refused"
                            | "before_numeric",
                        ),
                    ) => 4,
                    (10, Some("open_regions" | "open_obligations")) => {
                        if ov.rules >= 1 {
                            5
                        } else {
                            27
                        }
                    }
                    (10, Some("not_lower")) => 6,
                    (20, _) => 8,
                    _ => 22,
                };
            }
            match c {
                11 | 12 => return 22, // verdict codes of a producing kind
                21 => return 9,
                22 => return 10,
                30 => return 11,
                _ => {}
            }
        }
        match c {
            40 => return 12,
            41 => return 13,
            42 => return 14,
            50 => return 15,
            51 => return 16,
            70 if ovalid => return 17,
            _ => {}
        }
    }
    if let WaitStatus::Signaled(s) = w {
        let stopsig = matches!(s, 9 | 15 | 2 | 1);
        if s == 9 && k == KillReason::Memory {
            return 18;
        }
        if stopsig && matches!(k, KillReason::Time | KillReason::Unresponsive) {
            return 19;
        }
        if stopsig && k == KillReason::Stop {
            return 20;
        }
        if s == 9 && k == KillReason::None && !present {
            return 21;
        }
    }
    if present && !ovalid {
        return 22;
    }
    23
}

#[allow(dead_code)]
fn _p(_: &Path) {}
#[allow(dead_code)]
fn _e(_: ExitCode) {}
