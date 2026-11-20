//! `rr_campaign table-check` (emission contract section 9): REFUSES a table any of whose rule records lacks the
//! `derivation` member, has a body that is not the one its derivation names, index != position, a missing or altered
//! side file, stated stages that differ from the stored checker output, or a stated class that the stored evidence does
//! not give; with `--level recheck` also when a RE-RUN of the checker on the record's bytes (with its stored
//! certificate) gives other stage verdicts. Writes the per-rule CLASS MANIFEST (one line per rule: class + certificate
//! sha256), the metadata a package carries since the native importer drops the `derivation` member. Runs on a delivery
//! directory: the table and its `<stem>.emission/` side directory.
use super::emitter::{Checker, run_checker};
use rustred_campaign_core::emission::body::{string_member, strip_derivation};
use rustred_campaign_core::emission::class::ClassWord;
use rustred_campaign_core::emission::validate::{check_record, compare_rerun};
use rustred_campaign_core::emission::{AREA_CERT, side_dir, side_rel};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// What to check.
#[derive(Clone, Debug, Default)]
pub struct Args {
    pub table: PathBuf,
    pub recheck: Option<Checker>,
    /// recheck every k-th record with a certificate (1 = all)
    pub every: usize,
    pub manifest: Option<PathBuf>,
    pub work: Option<PathBuf>,
    /// RE-RUN the engine's descent test (`BIN descent`, contract section 14) on every emitted record's own bytes and
    /// compare with its stored output (a passing output names no record: this re-run is the strong binding)
    pub descent_recheck: Option<PathBuf>,
}

/// Check a table; returns (summary, every record accepted).
pub fn table_check(a: &Args) -> Result<(Value, bool), String> {
    let side = side_dir(&a.table);
    // procfix9-ok: a content-addressed side file; unreadable = missing evidence, the record is refused (fail closed)
    let lookup = |area: &str, sha: &str| std::fs::read(side.join(side_rel(area, sha))).ok();
    let f = std::fs::File::open(&a.table).map_err(|e| format!("{}: {e}", a.table.display()))?;
    let mut header = String::new();
    let mut pos: BTreeMap<String, u64> = BTreeMap::new();
    let mut classes: BTreeMap<i64, ClassWord> = BTreeMap::new();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut subclasses: BTreeMap<String, u64> = BTreeMap::new();
    let mut refused: Vec<Value> = vec![];
    let mut n_refused = 0u64;
    let mut rules = 0u64;
    let mut rechecked = 0u64;
    let mut descent_rechecked = 0u64;
    let mut notes_n = 0u64;
    let mut notes: Vec<Value> = vec![];
    let mut manifest: Option<std::io::BufWriter<std::fs::File>> = match &a.manifest {
        Some(p) => Some(std::io::BufWriter::new(
            std::fs::File::create(p).map_err(|e| format!("{}: {e}", p.display()))?,
        )),
        None => None,
    };
    let work = a.work.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("table-check-{}", std::process::id()))
    });
    let mut with_cert = 0usize;
    for (ln, l) in std::io::BufReader::new(f).lines().enumerate() {
        let l = l.map_err(|e| format!("line {}: {e}", ln + 1))?;
        if l.trim().is_empty() {
            continue;
        }
        let kind = string_member(&l, "type");
        if header.is_empty() && kind.as_deref() == Some("header") {
            header = l.clone();
            continue;
        }
        if kind.as_deref() != Some("rule") {
            continue;
        }
        rules += 1;
        let chart = string_member(&l, "chart").unwrap_or_default();
        let p = pos.entry(chart.clone()).or_insert(0);
        let position = *p;
        *p += 1;
        let alias = |i: i64| classes.get(&i).copied();
        let mut verdict = check_record(&l, position, &lookup, &alias);
        // recheck: the checker again on the record's own bytes with its stored certificate
        if let (Some(ck), Some(out), Some(cs)) = (
            &a.recheck,
            &verdict.check_output,
            &verdict.certificate_sha256,
        ) {
            with_cert += 1;
            if (with_cert - 1).is_multiple_of(a.every.max(1)) {
                let (body, _) = strip_derivation(&l)?;
                let cert = side.join(side_rel(AREA_CERT, cs));
                let r = run_checker(
                    ck,
                    &header,
                    &body,
                    &cert,
                    &work.join(format!("r{position}")),
                    ck.budget(0.0),
                )?;
                rechecked += 1;
                match r.output {
                    Some(o) => {
                        if let Err(e) = compare_rerun(out, &o) {
                            verdict.problems.push(e);
                        }
                    }
                    None => verdict.problems.push("re-run gave no output".into()),
                }
            }
        }
        // descent re-run: the engine's test again on header + the record's own bytes; equal to the stored output
        if let Some(node) = &a.descent_recheck
            && let (body, Some(raw)) = strip_derivation(&l)?
            && let Ok(d) = serde_json::from_str::<Value>(&raw)
            && let Some(sha) = d["descent"]["output_sha256"].as_str()
        {
            let dir = work.join(format!("descent-{chart}-{position}"));
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let input = dir.join("descent-input.table.jsonl");
            std::fs::write(&input, format!("{}\n{body}\n", header.trim_end()))
                .map_err(|e| format!("{}: {e}", input.display()))?;
            let args: Vec<String> = vec![
                "descent".into(),
                "--table".into(),
                input.display().to_string(),
            ];
            let r = super::child::run(node, &args, &[], &dir, "descent", 600.0)?;
            descent_rechecked += 1;
            let again = r
                .stdout_text()
                .lines()
                .rev()
                .find(|l| l.trim_start().starts_with('{'))
                .map(str::to_string);
            let stored = lookup(rustred_campaign_core::emission::AREA_DESCENT, sha);
            let parse = |b: &[u8]| serde_json::from_slice::<Value>(b).ok();
            match (again, stored) {
                (Some(x), Some(s))
                    if parse(x.as_bytes()).is_some() && parse(x.as_bytes()) == parse(&s) => {}
                (Some(x), _) => verdict.problems.push(format!(
                    "the descent re-run on the record's bytes differs from the stored output: {x}"
                )),
                (None, _) => verdict.problems.push(format!(
                    "the descent re-run gave no output ({})",
                    r.stderr_tail(200)
                )),
            }
        }
        if !verdict.notes.is_empty() {
            notes_n += 1;
            if notes.len() < 50 {
                notes.push(json!({"position": position, "index": verdict.index, "chart": chart, "notes": verdict.notes}));
            }
        }
        let class = verdict.class.unwrap_or(ClassWord::None);
        classes.insert(verdict.index, class);
        *counts.entry(class.word().to_string()).or_default() += 1;
        // the visible sub-class (implementer 2026-11-18T22:00Z): whole domain by an identical statement elsewhere
        if l.contains("\"binding\":\"identical statement") {
            *subclasses
                .entry("whole domain, by identical statement".to_string())
                .or_default() += 1;
        }
        if let Some(m) = manifest.as_mut() {
            let d = strip_derivation(&l)?
                .1
                .and_then(|r| serde_json::from_str::<Value>(&r).ok());
            let row = json!({"chart": chart, "position": position, "index": verdict.index, "body_sha256": verdict.body_sha256,
                "kind": verdict.kind.map(|k| k.word()), "class": class.word(), "certificate_sha256": verdict.certificate_sha256,
                "check_output_sha256": d.as_ref().and_then(|d| d["check"]["output_sha256"].as_str().map(str::to_string)),
                "liveness": d.as_ref().and_then(|d| d["class"]["legacy"]["liveness"]["verdict"].as_str().map(str::to_string)),
                "subclass": d.as_ref().and_then(|d| d["class"]["legacy"]["proof_object"]["binding"].as_str())
                    .filter(|b| b.starts_with("identical")).map(|b| format!("whole domain, by {b}")),
                "accepted": verdict.ok()});
            writeln!(m, "{row}").map_err(|e| e.to_string())?;
        }
        if !verdict.ok() {
            n_refused += 1;
            if refused.len() < 50 {
                refused.push(json!({"position": position, "index": verdict.index, "chart": chart, "problems": verdict.problems}));
            }
        }
    }
    if let Some(mut m) = manifest {
        m.flush().map_err(|e| e.to_string())?;
    }
    let ok = n_refused == 0 && rules > 0;
    Ok((
        json!({"schema": "rustred.campaign.table-check.v1", "table": a.table.display().to_string(),
            "side": side.display().to_string(), "rules": rules, "refused": n_refused, "classes": counts, "subclasses": subclasses,
            "rechecked": rechecked, "descent_rechecked": descent_rechecked,
            "reported": {"records_with_notes": notes_n, "notes": notes,
                         "role": "reported numbers (d = 4 regularity: tie-break only, LANE_RULES rule 21); they refuse nothing"},
            "verdict": if ok { "ACCEPTED" } else { "REFUSED" }, "refusals": refused}),
        ok,
    ))
}

/// CLI: `table-check --table T [--recheck --checker BIN [--every K] [--no-recover-rows] [--family F]] [--descent-node BIN]
/// [--manifest OUT] [--work DIR]`. Exit 0 accepted, 2 refused. `--descent-node`: re-run the descent test of every emitted
/// record (contract section 14).
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let mut a = Args {
        every: 1,
        ..Default::default()
    };
    let mut recheck = false;
    let mut checker: Option<PathBuf> = None;
    let mut recover = true;
    let mut family = None;
    let mut i = 0;
    let val = |i: usize| {
        rest.get(i + 1)
            .cloned()
            .ok_or(format!("{} needs a value", rest[i]))
    };
    while i < rest.len() {
        match rest[i].as_str() {
            "--table" => a.table = val(i)?.into(),
            "--manifest" => a.manifest = Some(val(i)?.into()),
            "--work" => a.work = Some(val(i)?.into()),
            "--checker" => checker = Some(val(i)?.into()),
            "--every" => a.every = val(i)?.parse().map_err(|_| "--every: a number")?,
            "--family" => family = Some(PathBuf::from(val(i)?)),
            "--descent-node" => a.descent_recheck = Some(PathBuf::from(val(i)?)),
            "--recheck" => {
                recheck = true;
                i += 1;
                continue;
            }
            "--no-recover-rows" => {
                recover = false;
                i += 1;
                continue;
            }
            o => return Err(format!("unknown argument {o}")),
        }
        i += 2;
    }
    if a.table.as_os_str().is_empty() {
        return Err("--table is required".into());
    }
    if recheck {
        let b = checker.ok_or("--recheck needs --checker")?;
        let mut c = Checker::new(Path::new(&b))?;
        c.recover_rows = recover;
        c.family = family;
        a.recheck = Some(c);
    }
    let (s, ok) = table_check(&a)?;
    Ok((s.to_string(), ok))
}
