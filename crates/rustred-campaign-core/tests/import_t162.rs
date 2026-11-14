//! Lane import, BIG DATA (ignored): reproduction of the T162 hand statement statement-closure-20261114T0623Z (bank
//! qualified-domain bounds applied, every key certified) from the EXISTING material through the import: the bank set
//! bank-conv/20261114c (qualified, recenter bounds as SELECTION bounds, labels set-unique), then the lane's rules in the
//! statement's stage order (bank, sc, symr, fp .. e, sc-r, p2rows, u) from their run tables, the 2 terminals; evidence:
//! the per-key regularity facts facts-20261114c (whatever files exist: a partial set is reported as such) and the
//! rr_proof span records of cert-closure-0623Z / cert-bankall-0623Z (bound by record_sha256). Tier C, both elided
//! policies; the comparison is typed and complete. (The last column of keys.jsonl is the key's (dots, rank) layer, not a
//! BFS round: the hand rounds are statement.json's total.)
//!
//! Paths (test data only): `RUSTRED_IMPORT_WS`, `RUSTRED_IMPORT_LARGE` (default /common/build-cache/vhirschi/
//! p_integral_dev/bank-conv), scratch `RUSTRED_IMPORT_SCRATCH`.
//! Run: `light_build.sh import test -p rustred-campaign-core --release --test import_t162 -- --ignored --nocapture`.

use rustred_campaign_core::import;

use std::path::{Path, PathBuf};

use import::lines::parse_key_text;
use import::{
    CheckOptions, Evidence, HandFormat, TableSetSpec, check, compare, load, read_hand_statement,
    write_statement,
};
use serde_json::{Value, json};

fn ws() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_WS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/shared/vhirschi/t3code/vakint_dev/p_integral_dev"))
}
fn large() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_LARGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/common/build-cache/vhirschi/p_integral_dev/bank-conv"))
}
fn scratch() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/import",
                std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
            ))
        })
}
fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

fn hand_format() -> HandFormat {
    HandFormat::positional(0, 3, 1, 2)
}

/// SHA256SUMS of a directory: verify every listed file that exists; returns (listed, present, verified).
fn verify_sums(dir: &Path) -> (usize, usize) {
    let (mut listed, mut ok) = (0, 0);
    for l in std::fs::read_to_string(dir.join("SHA256SUMS"))
        .unwrap()
        .lines()
    {
        let (h, n) = l.split_once(char::is_whitespace).unwrap();
        let n = n.trim_start().trim_start_matches('*');
        listed += 1;
        let p = dir.join(n);
        if p.is_file() {
            assert_eq!(
                import::lines::file_sha256(&p).unwrap(),
                h,
                "{n} differs from its SHA256SUMS"
            );
            ok += 1;
        }
    }
    (listed, ok)
}

fn build_spec(ws: &Path, large: &Path, stmt_dir: &Path) -> Value {
    let cat: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt_dir.join("catalogue.json")).unwrap())
            .unwrap();
    let st: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt_dir.join("statement.json")).unwrap())
            .unwrap();
    let runs = ws.join("SCRATCHPAD/t162-box/runs");
    let mut sources = vec![json!({
        "id":"bank","kind":"table_set","class":"bank",
        "priority_file": ws.join("SCRATCHPAD/bank-conv/out/20261114c/DISPATCH-PRIORITY.json").display().to_string(),
        "variant":"qualified","tables_dir": large.join("20261114c").display().to_string(),
        "sha256sums":[{"path": large.join("20261114c/SHA256SUMS").display().to_string(),
                       "sha256":"fbaa6454dc87a4ac4b2ed13067d75ea8183a01018078d455dbf0ad3f8b5aef12"}],
        "label_scope":"t162-set","domain":"qualified_recenter","hand_name":{"template":"bank#{label}"}})];
    // the lane's rules: consecutive catalogue entries of one run = one source (records in catalogue order)
    let mut cur: Option<(String, Vec<i64>)> = None;
    let push = |cur: &mut Option<(String, Vec<i64>)>, sources: &mut Vec<Value>| {
        if let Some((run, recs)) = cur.take() {
            let p = runs.join(&run).join("node/F.table.jsonl");
            assert!(p.is_file(), "{}", p.display());
            sources.push(json!({"id": run, "kind":"engine_table", "class": run.split('-').next().unwrap(),
                "path": p.display().to_string(), "records": recs, "hand_name":{"template":"{source}#{index}"}}));
        }
    };
    let mut n_ours = 0;
    for e in cat["entries"].as_array().unwrap() {
        if e["kind"] == "bank" {
            continue;
        }
        n_ours += 1;
        let name = e["name"].as_str().unwrap();
        let (run, idx) = name.split_once('#').unwrap();
        let idx: i64 = idx.parse().unwrap();
        match &mut cur {
            Some((r, v)) if r == run => v.push(idx),
            _ => {
                push(&mut cur, &mut sources);
                cur = Some((run.to_string(), vec![idx]));
            }
        }
    }
    push(&mut cur, &mut sources);
    assert_eq!(n_ours, 950);
    let terms: Vec<Vec<i32>> = st["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| parse_key_text(t.as_str().unwrap(), 20).unwrap())
        .collect();
    sources.push(json!({"id":"terminals","kind":"terminals","keys": terms}));
    let facts = large.join("facts-20261114c/facts");
    json!({"schema":"rustred.campaign.import-spec.v1","node":"T162","frame_from":"bank","bits":6,
        "start":{"path": stmt_dir.join("keys.jsonl").display().to_string(),"format":"hand_statement",
                 "hand_format": serde_json::to_value(hand_format()).unwrap()},
        "required_evidence":"per-key","policies":{"tier":"auto"},
        "sources": sources,
        "evidence":{"facts":[{"path": facts.display().to_string()}],
                    "proofs":[{"path": ws.join("SCRATCHPAD/t162-box/runs/cert-closure-0623Z").display().to_string()},
                              {"path": ws.join("SCRATCHPAD/t162-box/runs/cert-bankall-0623Z").display().to_string()}]}})
}

#[test]
#[ignore = "big data: the T162 hand statement, the bank set, facts and proofs; run with --ignored in release"]
fn t162_reproduction_of_the_hand_statement() {
    let (ws, large) = (ws(), large());
    let stmt = ws.join("SCRATCHPAD/t162-box/runs/statement-closure-20261114T0623Z");
    if !stmt.join("keys.jsonl").is_file() || !large.join("20261114c/SHA256SUMS").is_file() {
        println!("SKIP: T162 material missing");
        return;
    }
    assert_eq!(verify_sums(&stmt), (3, 3));
    let fdir = large.join("facts-20261114c");
    let (fl, fp) = if fdir.join("SHA256SUMS").is_file() {
        verify_sums(&fdir)
    } else {
        (0, 0)
    };
    println!("T162 facts-20261114c: SHA256SUMS lists {fl} files, {fp} present and verified");
    let out = scratch().join("t162");
    std::fs::create_dir_all(&out).unwrap();
    let spec_v = build_spec(&ws, &large, &stmt);
    std::fs::write(
        out.join("spec.T162.json"),
        serde_json::to_string_pretty(&spec_v).unwrap(),
    )
    .unwrap();
    let hand = read_hand_statement(&stmt.join("keys.jsonl"), &hand_format(), 20).unwrap();
    assert_eq!(hand.rows.len(), 65_908);
    let st: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt.join("statement.json")).unwrap())
            .unwrap();
    println!(
        "T162 hand statement.json: keys {} rounds {} edges {}",
        st["keys"], st["rounds"], st["edges"]
    );
    for policy in ["block", "lift_with_per_key_proof"] {
        let t0 = std::time::Instant::now();
        let mut sv = spec_v.clone();
        sv["policies"]["elided"] = json!(policy);
        let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
        let set = load(&spec, Path::new("/")).unwrap();
        let t_load = t0.elapsed().as_secs_f64();
        assert_eq!(set.start.len(), 55_704);
        let t1 = std::time::Instant::now();
        let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
        let t_ev = t1.elapsed().as_secs_f64();
        let res = check(
            &set,
            &ev,
            &CheckOptions {
                threads: 4,
                ..Default::default()
            },
        )
        .unwrap();
        let dir = out.join(format!("statement-{policy}"));
        let timing = json!({"load_s": t_load, "evidence_s": t_ev, "check_s": res.seconds, "vm_hwm_kb": vm_hwm_kb()});
        write_statement(&set, &ev, &res, &dir, Some(timing)).unwrap();
        let cmp = compare(&set, &res, &hand);
        cmp.write(&dir).unwrap();
        let not_disp: usize = set
            .report
            .sources
            .iter()
            .map(|s| s.not_dispatched.len())
            .sum();
        println!(
            "T162 [{policy}]: load {:.1}s evidence {:.1}s check {:.1}s VmHWM {} MB; tables {} records {} (not dispatched {}); \
             evidence {:?}; keys ours {} hand {}; counts {:?}; closure verdict {:?} tier {:?} rounds {} edges {} \
             violations {} undecided {:?}; blocking {:?}",
            t_load,
            t_ev,
            res.seconds,
            vm_hwm_kb() / 1024,
            set.report.tables,
            set.report.records,
            not_disp,
            ev.counts,
            res.rows.len(),
            hand.rows.len(),
            res.counts(),
            res.summary.verdict,
            res.tier,
            res.summary.rounds,
            res.summary.edges,
            res.summary.descent_violations,
            res.summary.undecided,
            res.blocking()
        );
        println!(
            "T162 [{policy}] comparison: key sets equal {} same server {} terminals equal {} ({:?} vs {:?}) rounds {:?} vs {}; \
             differences {:?}; unmapped hand rules {}; label matrix {:?}; requests solve {} facts {} proofs {}",
            cmp.key_sets_equal,
            cmp.same_server,
            cmp.terminals_equal,
            cmp.terminals_hand,
            cmp.terminals_ours,
            cmp.rounds_hand,
            cmp.rounds_ours,
            cmp.counts,
            cmp.unmapped_hand_rules.len(),
            cmp.label_matrix,
            res.solve_requests.len(),
            res.fact_requests.len(),
            res.proof_requests.len()
        );
        for d in cmp.differences.iter().take(12) {
            println!("  diff {:?}", d);
        }
    }
}

/// Lane import2 item 2: the key ours reaches and the hand statement does not (only_ours): which served keys reach it,
/// through which rule (record_sha256, file, byte offset) and which term, and what the evidence says about that term's
/// coefficient at the predecessor key. Prints one line per (predecessor, term); the coefficient TEXT is then read from
/// the named file at the byte offset (the record line) and evaluated exactly (notes/implementer.import2.md).
#[test]
#[ignore = "big data: T162 material; run with --ignored in release"]
fn t162_only_ours_keys_and_their_predecessor_terms() {
    use rustred_campaign_core::closure::{CoefFact, FactSource, RuleId};
    let (ws, large) = (ws(), large());
    let stmt = ws.join("SCRATCHPAD/t162-box/runs/statement-closure-20261114T0623Z");
    if !stmt.join("keys.jsonl").is_file() || !large.join("20261114c/SHA256SUMS").is_file() {
        println!("SKIP: T162 material missing");
        return;
    }
    let mut sv = build_spec(&ws, &large, &stmt);
    sv["policies"]["elided"] = json!("lift_with_per_key_proof");
    let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    let res = check(&set, &ev, &CheckOptions::default()).unwrap();
    let hand = read_hand_statement(&stmt.join("keys.jsonl"), &hand_format(), 20).unwrap();
    let cmp = compare(&set, &res, &hand);
    let targets: std::collections::BTreeSet<Vec<i32>> = cmp
        .differences
        .iter()
        .filter(|d| d.kind == import::DiffKind::OnlyOurs)
        .map(|d| d.key.clone())
        .collect();
    println!("only_ours keys: {targets:?}");
    let hand_rule: std::collections::HashMap<Vec<i32>, Option<String>> = hand
        .rows
        .iter()
        .map(|r| (r.key.clone(), r.rule.clone()))
        .collect();
    let n = set.arity();
    let (mut out, mut cm) = (vec![], vec![]);
    for row in &res.rows {
        let (t, r) = match row.server {
            Some(import::Server::Rule { table, record })
            | Some(import::Server::Credit { table, record }) => (table, record),
            _ => match (row.status, row.open_rule) {
                (_, Some(tr)) => tr,
                _ => continue,
            },
        };
        out.clear();
        cm.clear();
        let rr = set.disp.rule_ref(t, r);
        let Ok(Some(nt)) = set
            .bodies
            .instantiate(&set.disp, rr, &row.key, &mut out, &mut cm)
        else {
            continue;
        };
        for j in 0..nt {
            let tk = &out[j * n..(j + 1) * n];
            if targets.contains(tk) {
                let info = set.rule(t, r);
                let digest = String::new();
                let rid = RuleId {
                    table: &info.path,
                    record: r,
                    label: info.label,
                    digest: &digest,
                };
                let coef = ev.coefficient(&rid, &row.key, j, tk);
                let reg = ev.regularity(&rid, &row.key);
                println!(
                    "PRED key {:?} status {:?} server {:?} hand_rule {:?} -> term {} = {:?}; rule {} label {} index {:?} \
                     sha256 {} file {} offset {} bytes {}; facts: coefficient {:?} regularity {:?} (Zero would drop the edge: {})",
                    row.key,
                    row.status,
                    info.hand_name,
                    hand_rule.get(&row.key),
                    j,
                    tk,
                    info.table_name,
                    info.label,
                    info.index,
                    info.record_sha256,
                    info.path,
                    info.offset,
                    info.bytes,
                    coef,
                    reg,
                    coef == CoefFact::Zero
                );
            }
        }
    }
}

/// Lane import2 item 2, the FACT: the coefficient of each predecessor term that reaches the only-ours key, evaluated
/// EXACTLY in d at the predecessor key with the core's exact polynomial code (`poly::Poly`, `domain::PoleLocus::
/// vanishes_at` = identically zero in d; the same code as `accept::facts::ExactRecordFacts`). The record lines are
/// read at their byte offsets and checked against their record_sha256 (test data: the run table of sc-c005).
#[test]
#[ignore = "big data: reads two record lines of a T162 run table"]
fn t162_only_ours_key_predecessor_coefficients_exact() {
    use rustred_campaign_core::accept::facts::split_ratio;
    use rustred_campaign_core::closure::sha256::Sha256;
    use rustred_campaign_core::domain::{PoleKind, PoleLocus};
    use rustred_campaign_core::poly::Poly;
    use std::io::{Read, Seek, SeekFrom};
    let ws = ws();
    let table = ws.join("SCRATCHPAD/t162-box/runs/sc-c005/node/F.table.jsonl");
    if !table.is_file() {
        println!("SKIP: {} missing", table.display());
        return;
    }
    let target = [1, 7, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    // (offset, bytes, record_sha256, predecessor key, term index) from t162_only_ours_keys_and_their_predecessor_terms
    let cases: [(u64, usize, &str, [i32; 20], usize); 2] = [
        (
            82_476_924,
            6_553_873,
            "acaa753bf21ede61f76588529f45795b08759fe122d97c43778124df5ae00955",
            [1, 6, 1, 1, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            2,
        ),
        (
            103_562_477,
            23_910_516,
            "8ae0445078a98e92139429b23d779c28874179fc014ac5841687dfa4b6cdf7cb",
            [1, 6, 2, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            0,
        ),
    ];
    let mut f = std::fs::File::open(&table).unwrap();
    for (off, len, sha, key, term) in cases {
        let mut buf = vec![0u8; len];
        f.seek(SeekFrom::Start(off)).unwrap();
        f.read_exact(&mut buf).unwrap();
        let mut h = Sha256::new();
        h.update(&buf);
        assert_eq!(h.finish_hex(), sha, "record line at {off}");
        let v: Value = serde_json::from_slice(&buf).unwrap();
        let t = &v["rhs"][term];
        let pattern = t[0].as_str().unwrap();
        let coef = t[1].as_str().unwrap();
        // the rule point is the key (canonical target); the term's pattern instantiates to the only-ours key
        let slots = rustred_campaign_core::closure::parse_rhs_pattern(pattern, 20).unwrap();
        let inst: Vec<i32> = slots
            .iter()
            .map(|s| match *s {
                rustred_campaign_core::closure::RhsSlot::Param { param, offset } => {
                    key[param] + offset
                }
                rustred_campaign_core::closure::RhsSlot::Const(c) => c,
            })
            .collect();
        assert_eq!(inst, target, "term {term} of {sha} at {key:?}");
        let (n, d) = split_ratio(coef.trim()).unwrap();
        let num = Poly::parse(n.trim(), 20, true).unwrap();
        let den = d.map(|d| Poly::parse(d.trim(), 20, true).unwrap());
        let vals = |p: &Poly| -> Vec<(u32, String)> {
            p.d_coefficients()
                .into_iter()
                .map(|(j, c)| {
                    (
                        j,
                        c.eval_checked_i128(&key)
                            .map(|x| x.to_string())
                            .unwrap_or_else(|_| format!("{:?}", c.eval_big(&key))),
                    )
                })
                .collect()
        };
        let num_zero = PoleLocus::new(PoleKind::Denominator, &num)
            .vanishes_at(&key)
            .unwrap();
        let den_zero = den.as_ref().map(|q| {
            PoleLocus::new(PoleKind::Denominator, q)
                .vanishes_at(&key)
                .unwrap()
        });
        println!(
            "EXACT record {sha} (rule index {}, witness_loci {}) at {key:?}: term {term} {pattern} -> {target:?}; \
             coefficient text {} bytes; numerator d-coefficients at the key {:?} (identically zero in d: {num_zero}); \
             denominator d-coefficients {:?} (identically zero: {den_zero:?})",
            v["index"],
            v["witness_loci"],
            coef.len(),
            vals(&num),
            den.as_ref().map(&vals),
        );
        assert!(
            num_zero,
            "the coefficient does not vanish identically at the key"
        );
        assert_eq!(den_zero, Some(false), "the denominator vanishes at the key");
    }
}

/// The CANONICAL SET of statement-closure-20261114T0623Z (coord-bank-tables.md v1.4; coordinator 2026-11-14): bank
/// 20261114c (qualified, recenter bounds as SELECTION bounds) then the own set t162box-20261114d (514 records, 410 run
/// tables, labels 100000-100513, in its DISPATCH-PRIORITY order = the catalogue's written order), the 2 terminals; bound
/// evidence: proofs-20261114d (one line per (label, key)), facts-20261114d (64,034 regular). Every SHA256SUMS verified.
/// Then, for every key only ours reaches: its predecessors, the rule, the term, the bound facts at that (rule, key) and
/// the term's coefficient evaluated EXACTLY at the key (record line read at its offset in the set file, sha256 checked).
#[test]
#[ignore = "big data: the T162 canonical set, proofs and facts; run with --ignored in release"]
fn t162_canonical_set_0623z_reproduction() {
    use rustred_campaign_core::accept::facts::split_ratio;
    use rustred_campaign_core::closure::{CoefFact, FactSource, RuleId};
    use rustred_campaign_core::domain::{PoleKind, PoleLocus};
    use rustred_campaign_core::poly::Poly;
    use std::io::{Read, Seek, SeekFrom};
    let (ws, large) = (ws(), large());
    let stmt = ws.join("SCRATCHPAD/t162-box/runs/statement-closure-20261114T0623Z");
    let own = large.join("t162box-20261114d");
    if !stmt.join("keys.jsonl").is_file() || !own.join("SHA256SUMS").is_file() {
        println!("SKIP: T162 canonical material missing");
        return;
    }
    assert_eq!(verify_sums(&stmt), (3, 3));
    for d in [
        "20261114c",
        "t162box-20261114d",
        "proofs-20261114d",
        "facts-20261114d",
    ] {
        let (l, p) = verify_sums(&large.join(d));
        println!("T162 {d}: SHA256SUMS lists {l} files, {p} present and verified");
        assert_eq!(l, p, "{d}: every listed file present");
    }
    assert_eq!(
        import::lines::file_sha256(&own.join("SHA256SUMS")).unwrap(),
        "facae32114e3c79feef6671d2742918f5b7bf16f8454dc1f1a5dcb764ecc5547"
    );
    let st: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt.join("statement.json")).unwrap())
            .unwrap();
    let terms: Vec<Vec<i32>> = st["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| parse_key_text(t.as_str().unwrap(), 20).unwrap())
        .collect();
    let out = scratch().join("../import2/t162-canonical");
    std::fs::create_dir_all(&out).unwrap();
    let tpath = out.join("terminals.json");
    std::fs::write(&tpath, serde_json::to_string(&terms).unwrap()).unwrap();
    let sv = json!({"schema":"rustred.campaign.import-spec.v1","node":"T162","frame_from":"bank","bits":6,
        "start":{"path": stmt.join("keys.jsonl").display().to_string(),"format":"hand_statement",
                 "hand_format": serde_json::to_value(hand_format()).unwrap()},
        "required_evidence":"per-key","policies":{"tier":"auto","elided":"lift_with_per_key_proof"},
        "sources":[
          {"id":"bank","kind":"table_set","class":"bank",
           "priority_file": ws.join("SCRATCHPAD/bank-conv/out/20261114c/DISPATCH-PRIORITY.json").display().to_string(),
           "variant":"qualified","tables_dir": large.join("20261114c").display().to_string(),
           "sha256sums":[{"path": large.join("20261114c/SHA256SUMS").display().to_string(),
                          "sha256":"fbaa6454dc87a4ac4b2ed13067d75ea8183a01018078d455dbf0ad3f8b5aef12"}],
           "label_scope":"t162-set","domain":"qualified_recenter","hand_name":{"template":"bank#{label}"}},
          {"id":"own","kind":"table_set","class":"own",
           "priority_file": ws.join("SCRATCHPAD/bank-conv/out/t162box-20261114d/DISPATCH-PRIORITY.json").display().to_string(),
           "variant":"all","tables_dir": own.display().to_string(),
           "sha256sums":[{"path": own.join("SHA256SUMS").display().to_string(),
                          "sha256":"facae32114e3c79feef6671d2742918f5b7bf16f8454dc1f1a5dcb764ecc5547"}],
           "label_scope":"t162-set","hand_name":{"template":"{origin}#{origin_index}","strip":["tb-"]}},
          {"id":"terminals","kind":"terminals","path": tpath.display().to_string()}],
        "evidence":{"facts":[{"path": large.join("facts-20261114d/facts").display().to_string()}],
                    "proofs":[{"path": large.join("proofs-20261114d/proofs").display().to_string()}]}});
    std::fs::write(
        out.join("spec.T162-0623Z-canonical.json"),
        serde_json::to_string_pretty(&sv).unwrap(),
    )
    .unwrap();
    let hand = read_hand_statement(&stmt.join("keys.jsonl"), &hand_format(), 20).unwrap();
    assert_eq!(hand.rows.len(), 65_908);
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let t_load = t0.elapsed().as_secs_f64();
    let t1 = std::time::Instant::now();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    let t_ev = t1.elapsed().as_secs_f64();
    let res = check(&set, &ev, &CheckOptions::default()).unwrap();
    let dir = out.join("statement");
    write_statement(
        &set,
        &ev,
        &res,
        &dir,
        Some(json!({"load_s": t_load, "evidence_s": t_ev, "check_s": res.seconds, "vm_hwm_kb": vm_hwm_kb()})),
    )
    .unwrap();
    let cmp = compare(&set, &res, &hand);
    cmp.write(&dir).unwrap();
    println!(
        "T162C load {t_load:.1}s evidence {t_ev:.1}s check {:.1}s VmHWM {} MB; tables {} records {}; evidence {:?}",
        res.seconds,
        vm_hwm_kb() / 1024,
        set.report.tables,
        set.report.records,
        ev.counts
    );
    println!(
        "T162C keys ours {} hand {}; counts {:?}; by class {:?}; closure verdict {:?} tier {:?} rounds {} edges {} \
         violations {} undecided {:?} per_key_lifted {} dispatch_not_strict {:?} diagnostic {}; blocking {:?}",
        res.rows.len(),
        hand.rows.len(),
        res.counts(),
        res.evidence_by_class(&set),
        res.summary.verdict,
        res.tier,
        res.summary.rounds,
        res.summary.edges,
        res.summary.descent_violations,
        res.summary.undecided,
        res.summary.per_key_lifted.values().sum::<u64>(),
        res.summary.dispatch_not_strict,
        res.summary.diagnostic,
        res.blocking()
    );
    println!(
        "T162C comparison: key sets equal {} same server {} terminals equal {} rounds {:?} vs {}; differences {:?}; \
         unmapped hand rules {:?}; label matrix {:?}; requests facts {} proofs {}",
        cmp.key_sets_equal,
        cmp.same_server,
        cmp.terminals_equal,
        cmp.rounds_hand,
        cmp.rounds_ours,
        cmp.counts,
        cmp.unmapped_hand_rules,
        cmp.label_matrix,
        res.fact_requests.len(),
        res.proof_requests.len()
    );
    for d in &cmp.differences {
        println!("T162C diff {}", serde_json::to_string(d).unwrap());
    }
    // the keys only ours reaches: predecessors, rule, term, facts, exact coefficient
    let targets: std::collections::BTreeSet<Vec<i32>> = cmp
        .differences
        .iter()
        .filter(|d| d.kind == import::DiffKind::OnlyOurs)
        .map(|d| d.key.clone())
        .collect();
    let hand_rule: std::collections::HashMap<Vec<i32>, Option<String>> = hand
        .rows
        .iter()
        .map(|r| (r.key.clone(), r.rule.clone()))
        .collect();
    let n = set.arity();
    let (mut ib, mut cm) = (vec![], vec![]);
    for row in &res.rows {
        let (t, r) = match (&row.server, row.open_rule) {
            (Some(import::Server::Rule { table, record }), _)
            | (Some(import::Server::Credit { table, record }), _) => (*table, *record),
            (None, Some(tr)) => tr,
            _ => continue,
        };
        ib.clear();
        cm.clear();
        let Ok(Some(nt)) = set.bodies.instantiate(
            &set.disp,
            set.disp.rule_ref(t, r),
            &row.key,
            &mut ib,
            &mut cm,
        ) else {
            continue;
        };
        for j in 0..nt {
            let tk = &ib[j * n..(j + 1) * n];
            if !targets.contains(tk) {
                continue;
            }
            let info = set.rule(t, r);
            let digest = String::new();
            let rid = RuleId {
                table: &info.path,
                record: r,
                label: info.label,
                digest: &digest,
            };
            let fact = ev.coefficient(&rid, &row.key, j, tk);
            let reg = ev.regularity(&rid, &row.key);
            let proof = ev.proof(&set, t, r, &row.key).map(|p| p.status.clone());
            // exact coefficient of term j at the key from the record line
            let mut f = std::fs::File::open(&info.path).unwrap();
            let mut buf = vec![0u8; info.bytes as usize];
            f.seek(SeekFrom::Start(info.offset)).unwrap();
            f.read_exact(&mut buf).unwrap();
            let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
            h.update(&buf);
            assert_eq!(h.finish_hex(), info.record_sha256);
            let v: Value = serde_json::from_slice(&buf).unwrap();
            let pattern = v["rhs"][j][0].as_str().unwrap_or("?").to_string();
            let coef = v["rhs"][j][1].as_str().unwrap_or("").to_string();
            let (num_s, den_s) = split_ratio(coef.trim()).unwrap();
            let num = Poly::parse(num_s.trim(), n, true).unwrap();
            let den = den_s.map(|d| Poly::parse(d.trim(), n, true).unwrap());
            // rule point = the key (canonical targets of these records)
            let num_zero = PoleLocus::new(PoleKind::Denominator, &num)
                .vanishes_at(&row.key)
                .unwrap();
            let den_zero = den.as_ref().map(|q| {
                PoleLocus::new(PoleKind::Denominator, q)
                    .vanishes_at(&row.key)
                    .unwrap()
            });
            let dvals = |p: &Poly| -> Vec<String> {
                p.d_coefficients()
                    .into_iter()
                    .map(|(e, c)| {
                        format!(
                            "d^{e}:{}",
                            c.eval_checked_i128(&row.key)
                                .map(|x| x.to_string())
                                .unwrap_or_else(|_| "big".into())
                        )
                    })
                    .collect()
            };
            println!(
                "T162C PRED {} -> {} : row status {:?} server {:?} hand server {:?}; rule {} (label {}, witness {:?}, \
                 record_sha256 {}, file {} offset {} bytes {}); term {j} pattern {pattern}; bound facts: coefficient \
                 {fact:?} regularity {reg:?} proof {proof:?}; EXACT: numerator identically zero in d at the key: {num_zero} \
                 [{}]; denominator identically zero: {den_zero:?} [{}]",
                import::lines::key_text(&row.key),
                import::lines::key_text(tk),
                row.status,
                info.hand_name,
                hand_rule.get(&row.key),
                info.table_name,
                info.label,
                info.witness,
                info.record_sha256,
                info.path,
                info.offset,
                info.bytes,
                dvals(&num).join(" "),
                den.as_ref().map(|q| dvals(q).join(" ")).unwrap_or_default(),
            );
            let _ = CoefFact::Zero;
        }
    }
}

/// Lane import3 item 5, the T162 LEFTOVERS on the canonical set of statement 0623Z:
/// (a) the 19 symmetry keys (own records symr-c000#0..18, whose bound proofs belong to the composite records of
///     side/symr-composite.table.jsonl, not to the set lines) are served through the VERIFIED group credit path
///     instead: the 19 point records are removed from the dispatch (reason recorded) and a `symmetry_credit` source
///     derives the group from the frame header's presented family (re-derived and proved exactly, never trusted); each
///     key is credited I(key) = I(image) with the image served by its own rule, and its evidence level is the image's
///     (per-key when a per-key proof is bound to (image record, image): the closure's image lift, closure.md 17);
/// (b) the two coefficient facts of the extra key's only predecessor terms (sc-c005#2 term 2 at I(1,6,1,1,2,1^4,0^11);
///     sc-c005#4 term 0 at I(1,6,2,1^6,0^11)) are PRODUCED by the exact evaluation (`import::exact_facts`) as compact
///     fact verdict lines, loaded as evidence: the extra key leaves the closure and the key sets become equal.
#[test]
#[ignore = "big data: the T162 canonical set, proofs and facts; run with --ignored in release"]
fn t162_leftovers_verified_credits_and_exact_facts() {
    let (ws, large) = (ws(), large());
    let stmt = ws.join("SCRATCHPAD/t162-box/runs/statement-closure-20261114T0623Z");
    let own = large.join("t162box-20261114d");
    if !stmt.join("keys.jsonl").is_file() || !own.join("SHA256SUMS").is_file() {
        println!("SKIP: T162 canonical material missing");
        return;
    }
    let st: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt.join("statement.json")).unwrap()).unwrap();
    let terms: Vec<Vec<i32>> = st["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| parse_key_text(t.as_str().unwrap(), 20).unwrap())
        .collect();
    let out = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/import3",
                std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
            ))
        })
        .join("t162-leftovers");
    std::fs::create_dir_all(&out).unwrap();
    let tpath = out.join("terminals.json");
    std::fs::write(&tpath, serde_json::to_string(&terms).unwrap()).unwrap();
    let symr: Vec<i64> = (100100..=100118).collect();
    let reason = "symmetry point record: its key is served through the VERIFIED group credit path (re-derived), \
                  its bound proofs belong to the composite records";
    let sv = json!({"schema":"rustred.campaign.import-spec.v1","node":"T162","frame_from":"bank","bits":6,
        "start":{"path": stmt.join("keys.jsonl").display().to_string(),"format":"hand_statement",
                 "hand_format": serde_json::to_value(hand_format()).unwrap()},
        "required_evidence":"per-key","policies":{"tier":"auto","elided":"lift_with_per_key_proof"},
        "sources":[
          {"id":"bank","kind":"table_set","class":"bank",
           "priority_file": ws.join("SCRATCHPAD/bank-conv/out/20261114c/DISPATCH-PRIORITY.json").display().to_string(),
           "variant":"qualified","tables_dir": large.join("20261114c").display().to_string(),
           "sha256sums":[{"path": large.join("20261114c/SHA256SUMS").display().to_string(),
                          "sha256":"fbaa6454dc87a4ac4b2ed13067d75ea8183a01018078d455dbf0ad3f8b5aef12"}],
           "label_scope":"t162-set","domain":"qualified_recenter","hand_name":{"template":"bank#{label}"}},
          {"id":"own","kind":"table_set","class":"own",
           "priority_file": ws.join("SCRATCHPAD/bank-conv/out/t162box-20261114d/DISPATCH-PRIORITY.json").display().to_string(),
           "variant":"all","tables_dir": own.display().to_string(),
           "sha256sums":[{"path": own.join("SHA256SUMS").display().to_string(),
                          "sha256":"facae32114e3c79feef6671d2742918f5b7bf16f8454dc1f1a5dcb764ecc5547"}],
           "removed": symr.iter().map(|l| json!({"label": l, "reason": reason})).collect::<Vec<_>>(),
           "label_scope":"t162-set","hand_name":{"template":"{origin}#{origin_index}","strip":["tb-"]}},
          {"id":"credits","kind":"symmetry_credit","class":"symmetry","group_format":"derive"},
          {"id":"terminals","kind":"terminals","path": tpath.display().to_string()}],
        "evidence":{"facts":[{"path": large.join("facts-20261114d/facts").display().to_string()}],
                    "proofs":[{"path": large.join("proofs-20261114d/proofs").display().to_string()}]}});
    std::fs::write(out.join("spec.json"), serde_json::to_string_pretty(&sv).unwrap()).unwrap();
    let hand = read_hand_statement(&stmt.join("keys.jsonl"), &hand_format(), 20).unwrap();
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&sv.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let t_load = t0.elapsed().as_secs_f64();
    println!(
        "T162L load {t_load:.1}s VmHWM {} MB; records {}; group {:?}; removed {}",
        vm_hwm_kb() / 1024,
        set.report.records,
        set.report.sources.iter().find_map(|s| s.group.clone()).map(|g| (g.elements, g.provenance)),
        set.report.sources[1].removed.len()
    );
    // (b) the two exact fact lines
    let extra = [1, 7, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let extra_digest = format!("{:016x}", import::lines::term_key_digest(&extra));
    let cases: [(&str, [i32; 20]); 2] = [
        (
            "52687224e2a55f9230d86f43a3c7cfc6fce0f8d04749750ed4d8829c1392f9cc",
            [1, 6, 1, 1, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            "c8b5b4c8dd3eb7b42aa430ecad31d035d83f8172f99352a08afc6427a434da3d",
            [1, 6, 2, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
    ];
    let mut lines = String::new();
    for (sha, key) in cases {
        let (t, r) = set.find_sha(sha).expect("record of the canonical set");
        let v = import::exact_facts::fact_verdict_line(&set, t, r, &key).unwrap();
        let nz: Vec<&str> = v["nonzero"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        println!(
            "T162L FACT {} at {}: verdict {} n_terms {} nonzero {} zero {} unknown {}; the extra key's term listed non-zero: {}",
            set.rule(t, r).hand_name.clone().unwrap_or_default(),
            import::lines::key_text(&key),
            v["verdict"],
            v["n_terms"],
            nz.len(),
            v["zero_terms"],
            v["unknown"].as_array().map_or(0, Vec::len),
            nz.contains(&extra_digest.as_str())
        );
        assert!(!nz.contains(&extra_digest.as_str()), "the extra key's term is identically zero");
        lines.push_str(&v.to_string());
        lines.push('\n');
    }
    let fpath = out.join("facts-extra-key.jsonl");
    std::fs::write(&fpath, &lines).unwrap();
    // (a)+(b) the check
    let mut ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    ev.load_facts(
        &set,
        Path::new("/"),
        &import::spec::FileRef {
            path: fpath.display().to_string(),
            sha256: Some(import::lines::file_sha256(&fpath).unwrap()),
        },
    )
    .unwrap();
    let res = check(&set, &ev, &CheckOptions::default()).unwrap();
    let dir = out.join("statement");
    write_statement(&set, &ev, &res, &dir, Some(json!({"load_s": t_load, "vm_hwm_kb": vm_hwm_kb()}))).unwrap();
    let cmp = compare(&set, &res, &hand);
    cmp.write(&dir).unwrap();
    let credited: Vec<&import::KeyRow> = res
        .rows
        .iter()
        .filter(|r| matches!(r.server, Some(import::Server::Credit { .. })))
        .collect();
    let mut by_level: std::collections::BTreeMap<String, usize> = Default::default();
    for r in &credited {
        *by_level
            .entry(format!("{:?}:{}", r.evidence, r.basis.unwrap_or("-")))
            .or_default() += 1;
    }
    println!(
        "T162L keys ours {} hand {} equal {}; same server {}; differences {:?}; credited {} {:?}; counts {:?}; \
         blocking {:?}; VmHWM {} MB",
        res.rows.len(),
        hand.rows.len(),
        cmp.key_sets_equal,
        cmp.same_server,
        cmp.counts,
        credited.len(),
        by_level,
        res.counts(),
        res.blocking(),
        vm_hwm_kb() / 1024
    );
    for d in cmp.differences.iter().take(40) {
        println!("T162L diff {}", serde_json::to_string(d).unwrap());
    }
    for r in &credited {
        println!(
            "T162L CREDIT {} evidence {:?} basis {:?} image rule {:?}",
            import::lines::key_text(&r.key),
            r.evidence,
            r.basis,
            import::statement::server_hand_name(&set, r)
        );
    }
    assert!(cmp.key_sets_equal, "key sets equal after the two exact facts");
}
