//! Item 3 of lane gencheck (interfaces/emission-check.md): the emission path end to end on a SMALL REAL FAMILY (the
//! two-loop kite, 5 lines: TEST DATA) with the REAL independent checker (`rr_proof`, engine build: it generates the
//! family's ordinary IBP rows itself) and a SCRIPTED producer (the lifter returns rules and v1 certificates built from
//! the checker's own rows, as `rr_node witness` would). Every check runs on the exact bytes written.
//!
//! Env: RUSTRED_EMISSION_CHECKER = an engine-linked rr_proof (with `--mode source-rows`); the tests SKIP with a message
//! when it is absent. They spawn children: run with LIGHT_PIDNS=1 (L10). One checker at a time (Symbolica licence).
use rustred_campaign::emission::emitter::{Candidate, Checker, Emitter};
use rustred_campaign::emission::replace::{
    LadderConfig, Lift, Lifter, Outcome, certify_by_replacement,
};
use rustred_campaign::emission::tablecheck;
use rustred_campaign_core::emission::body::{attach_derivation, strip_derivation};
use rustred_campaign_core::emission::class::{ClassWord, Kind, Liveness, Origin, Policy};
use rustred_campaign_core::emission::side_dir;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ONE_CHECKER: Mutex<()> = Mutex::new(());

/// m -> [(offsets, coefficient text)]
type Rows = Vec<(i64, Vec<(Vec<i64>, String)>)>;
/// (m, terms)
type Row = (i64, Vec<(Vec<i64>, String)>);

const FAMILY: &str = "I(\n  name(KITE),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1-q)^2,1),\n  prop(D4,(k2-q)^2,1),\n  prop(D5,(k1-k2)^2,1),\n  gram(q,q,1)\n)\n";

fn checker_bin() -> Option<PathBuf> {
    match std::env::var("RUSTRED_EMISSION_CHECKER") {
        Ok(p) if Path::new(&p).exists() => Some(PathBuf::from(p)),
        _ => {
            eprintln!("SKIP: RUSTRED_EMISSION_CHECKER (engine-linked rr_proof) not set or absent");
            None
        }
    }
}

fn header() -> String {
    json!({"type": "header", "schema": "table_dump.v1", "family": "KITE", "family_source": FAMILY, "loops": 2,
           "externals": 1, "denominators": 5, "parameters": ["d"], "power_shifts": ["0","0","0","0","0"],
           "mask": "11111", "charts": ["11111"]}).to_string()
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gencheck-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The checker's own ordinary rows of the kite: m -> [(offsets, coefficient text)].
fn rows(bin: &Path, dir: &Path) -> Rows {
    let fam = dir.join("kite.family.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let out = std::process::Command::new(bin)
        .args(["--mode", "source-rows", "--family"])
        .arg(&fam)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut v = vec![];
    for l in String::from_utf8_lossy(&out.stdout).lines() {
        let r: Value = serde_json::from_str(l).unwrap();
        let terms = r["terms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                (
                    offsets(t[0].as_str().unwrap()),
                    t[1].as_str().unwrap().to_string(),
                )
            })
            .collect();
        v.push((r["m"].as_i64().unwrap(), terms));
    }
    v
}

/// "I(n0+1,n1,n2-1,..)" -> offsets.
fn offsets(p: &str) -> Vec<i64> {
    p.trim_start_matches("I(")
        .trim_end_matches(')')
        .split(',')
        .map(|s| {
            let s = s.trim();
            let k = s[1..].find(['+', '-']).map(|i| i + 1);
            k.map_or(0, |i| s[i..].trim_start_matches('+').parse().unwrap())
        })
        .collect()
}

/// An ordinary row whose coefficient of the unshifted integral depends on d (a target coefficient without index-only
/// loci), and its terms.
fn pick_row(rows: &[Row]) -> Row {
    rows.iter()
        .find(|(_, t)| {
            t.iter()
                .any(|(o, c)| o.iter().all(|x| *x == 0) && c.contains('d'))
                && t.len() >= 3
        })
        .cloned()
        .expect("a row with a d-dependent diagonal term")
}

/// Pattern text at a seed (slot 0 numeric `n0v` when given) with offsets.
fn pat(off: &[i64], n0v: Option<i64>) -> String {
    let s: Vec<String> = off
        .iter()
        .enumerate()
        .map(|(i, o)| match (i, n0v) {
            (0, Some(v)) => (v + o).to_string(),
            _ if *o == 0 => format!("n{i}"),
            _ => format!("n{i}{o:+}"),
        })
        .collect();
    format!("I({})", s.join(","))
}

fn at(c: &str, n0v: Option<i64>) -> String {
    match n0v {
        Some(v) => c.replace("n0", &format!("({v})")),
        None => c.to_string(),
    }
}

/// The rule of row `terms` at the seed (slot 0 fixed to `n0v` when given), its v1 certificate with weight `w`, the record.
fn rule_and_cert(
    terms: &[(Vec<i64>, String)],
    m: i64,
    w: &str,
    n0v: Option<i64>,
    exceptions: Value,
    index: i64,
) -> (Value, Value) {
    let (_, ct) = terms
        .iter()
        .find(|(o, _)| o.iter().all(|x| *x == 0))
        .unwrap();
    let ct = at(ct, n0v);
    let target = pat(&[0, 0, 0, 0, 0], n0v);
    let rhs: Vec<Value> = terms
        .iter()
        .filter(|(o, _)| !o.iter().all(|x| *x == 0))
        .map(|(o, c)| json!([pat(o, n0v), format!("-({})/({})", at(c, n0v), ct)]))
        .collect();
    let fixed = json!([n0v, null, null, null, null]);
    let case = match n0v {
        Some(v) => {
            format!("Coordinate(CoordinateCase {{ fixed: [Some({v}), None, None, None, None] }})")
        }
        None => "Coordinate(CoordinateCase { fixed: [None, None, None, None, None] })".to_string(),
    };
    let rec = json!({"type": "rule", "chart": "11111", "index": index, "case": case, "fixed": fixed, "equations": [],
        "target": target, "rhs": rhs, "exceptions": exceptions, "recenter": [0, 0, 0, 0, 0], "lift_seconds": 0.01});
    let row: Vec<Value> = terms
        .iter()
        .map(|(o, c)| json!([pat(o, n0v), format!("({})*({})", at(c, n0v), w)]))
        .collect();
    let cert = json!({"schema": "rustred.weight-cert.v1", "basis": "ordinary", "case": case, "target": target,
        "ordinary_terms": [{"seed": pat(&[0, 0, 0, 0, 0], n0v), "m": m, "w": w}], "row": row});
    (rec, cert)
}

fn emitter(bin: &Path, dir: &Path) -> Emitter {
    let mut c = Checker::new(bin).unwrap();
    c.floor_s = 300.0;
    Emitter::new(
        c,
        Policy::default(),
        &side_dir(&dir.join("t.table.jsonl")),
        &dir.join("work"),
    )
    .unwrap()
}

fn candidate(rec: &Value, cert: &Value, dir: &Path, tag: &str) -> Candidate {
    let cp = dir.join(format!("{tag}.cert.json"));
    std::fs::write(&cp, cert.to_string()).unwrap();
    Candidate {
        header: header(),
        line: rec.to_string(),
        kind: Kind::Lifted,
        origin: Origin::Lift,
        cert: Some(cp),
        provenance: Some((
            "test-producer".into(),
            json!({"tool": "scripted test lifter"}),
        )),
        lift_seconds: 0.01,
        ladder: None,
        cone: None,
        sample: None,
        tag: tag.into(),
    }
}

fn write_table(dir: &Path, lines: &[String]) -> PathBuf {
    let t = dir.join("t.table.jsonl");
    let mut s = header();
    s.push('\n');
    for l in lines {
        s.push_str(l);
        s.push('\n');
    }
    std::fs::write(&t, s).unwrap();
    t
}

fn check_table(t: &Path, bin: &Path, recheck: bool) -> (Value, bool) {
    let mut a = tablecheck::Args {
        table: t.to_path_buf(),
        every: 1,
        manifest: Some(t.with_extension("manifest.jsonl")),
        work: Some(t.with_extension("tc-work")),
        ..Default::default()
    };
    if recheck {
        a.recheck = Some(Checker::new(bin).unwrap());
    }
    tablecheck::table_check(&a).unwrap()
}

#[test]
fn correct_certificate_gives_whole_domain_and_the_validator_accepts_and_rechecks() {
    let Some(bin) = checker_bin() else { return };
    let _g = ONE_CHECKER.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tmpdir("whole");
    let r = rows(&bin, &dir);
    let (m, terms) = pick_row(&r);
    let (rec, cert) = rule_and_cert(&terms, m, "1", None, json!([]), 0);
    let em = emitter(&bin, &dir);
    let e = em
        .emit(&candidate(&rec, &cert, &dir, "ok"), Liveness::Live, false)
        .unwrap();
    assert_eq!(
        e.classed.class,
        ClassWord::WholeDomain,
        "{:?} {:?}",
        e.classed,
        e.summary
    );
    let line = e.line.expect("admitted");
    assert!(
        line.len() - rec.to_string().len() < 1024,
        "derivation member under 1 kB"
    );
    let t = write_table(&dir, std::slice::from_ref(&line));
    let (s, ok) = check_table(&t, &bin, true);
    assert!(ok, "{s}");
    assert_eq!(s["rechecked"], json!(1));
    let manifest = std::fs::read_to_string(t.with_extension("manifest.jsonl")).unwrap();
    assert!(
        manifest.contains("\"class\":\"whole domain\""),
        "{manifest}"
    );
    // bind-check: the checker output re-binds to the record at its own position of the table the checker read; the same
    // output against a table with one more record before it does not
    let cdir = em.work.join("ok");
    let item = rustred_campaign::emission::bindcheck::Item {
        label: "ok".into(),
        check: cdir.join("check.out"),
        cert: dir.join("ok.cert.json"),
    };
    let input = cdir.join("check-input.table.jsonl");
    let (sum, rows, man) = rustred_campaign::emission::bindcheck::bind_check(
        &input,
        std::slice::from_ref(&item),
        None,
    )
    .unwrap();
    assert_eq!(sum["binding_failures"], json!(0), "{sum} {rows:?}");
    assert_eq!(rows[0]["class_now"], json!("whole domain"));
    assert_eq!(man.len(), 1);
    let shifted = dir.join("shifted.table.jsonl");
    let mut other = rec.clone();
    other["target"] = json!("I(n0,n1,n2,n3,n4+1)");
    std::fs::write(
        &shifted,
        format!(
            "{}\n{}\n{}\n",
            header(),
            other,
            std::fs::read_to_string(&input)
                .unwrap()
                .lines()
                .nth(1)
                .unwrap()
        ),
    )
    .unwrap();
    let (sum, rows, _) =
        rustred_campaign::emission::bindcheck::bind_check(&shifted, &[item], None).unwrap();
    assert_eq!(sum["binding_failures"], json!(1), "{sum}");
    assert_eq!(rows[0]["class_now"], json!("none"));
    // THE VALIDATOR CATCHES A HAND-EDITED CLASS and an edited stage
    let weak = line.replace("\"word\":\"whole domain\"", "\"word\":\"sampled\"");
    let (s, ok) = check_table(&write_table(&dir, &[weak]), &bin, false);
    assert!(!ok, "{s}");
    assert!(s.to_string().contains("stated class"), "{s}");
    let staged = line.replace(
        "\"identity\":\"IDENTITY_EXACT\"",
        "\"identity\":\"IDENTITY_NOT_CHECKABLE\"",
    );
    let (_, ok) = check_table(&write_table(&dir, &[staged]), &bin, false);
    assert!(!ok);
    // a table with a record WITHOUT a derivation is refused
    let (_, ok) = check_table(&write_table(&dir, &[rec.to_string()]), &bin, false);
    assert!(!ok);
}

#[test]
fn corrupted_weight_is_refused() {
    let Some(bin) = checker_bin() else { return };
    let _g = ONE_CHECKER.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tmpdir("corrupt");
    let r = rows(&bin, &dir);
    let (m, terms) = pick_row(&r);
    let (rec, mut cert) = rule_and_cert(&terms, m, "1", None, json!([]), 0);
    cert["ordinary_terms"][0]["w"] = json!("2");
    let em = emitter(&bin, &dir);
    let e = em
        .emit(&candidate(&rec, &cert, &dir, "bad"), Liveness::Live, false)
        .unwrap();
    assert_eq!(e.classed.class, ClassWord::None);
    assert!(
        e.line.is_none(),
        "a live lifted record with class none is refused"
    );
    assert!(e.refused.is_some());
    assert_eq!(e.summary.as_ref().unwrap().identity, "IDENTITY_FAIL");
    let refused = std::fs::read_to_string(em.side.join("refused.jsonl")).unwrap();
    assert!(refused.contains("identity_fail"), "{refused}");
}

#[test]
fn certificate_of_a_twin_with_another_domain_is_refused() {
    // the T162 1541 case rebuilt: X and Y share target + rhs; the certificate's weight pole n0 = 3 is an exception of X
    // only. X gets whole domain; Y, checked on ITS OWN bytes, does not; Y carrying X's derivation is refused.
    let Some(bin) = checker_bin() else { return };
    let _g = ONE_CHECKER.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tmpdir("twin");
    let r = rows(&bin, &dir);
    let (m, terms) = pick_row(&r);
    let (x, cert) = rule_and_cert(&terms, m, "1/(n0-3)", None, json!([["-3+n0"]]), 0);
    let (y, _) = rule_and_cert(&terms, m, "1/(n0-3)", None, json!([]), 0);
    let em = emitter(&bin, &dir);
    let ex = em
        .emit(&candidate(&x, &cert, &dir, "x"), Liveness::Live, false)
        .unwrap();
    assert_eq!(ex.classed.class, ClassWord::WholeDomain, "{:?}", ex.summary);
    let ey = em
        .emit(&candidate(&y, &cert, &dir, "y"), Liveness::Live, false)
        .unwrap();
    assert_eq!(ey.summary.as_ref().unwrap().checker_class, "OPEN_LOCI");
    assert_eq!(ey.classed.class, ClassWord::None);
    assert!(ey.line.is_none());
    // the twin carrying X's derivation member (a class transferred by relation hash): refused by the validator
    let (_, xd) = strip_derivation(ex.line.as_ref().unwrap()).unwrap();
    let xd: Value = serde_json::from_str(&xd.unwrap()).unwrap();
    let forged = attach_derivation(&y.to_string(), &xd).unwrap();
    let (s, ok) = check_table(&write_table(&dir, &[forged]), &bin, false);
    assert!(!ok, "{s}");
    assert!(s.to_string().contains("body_sha256"), "{s}");
}

/// A scripted producer: the case with slot 0 free gets weight `w_free`, the case with slot 0 fixed gets `w_fixed`.
struct Scripted {
    terms: Vec<(Vec<i64>, String)>,
    m: i64,
    w_free: String,
    w_fixed: String,
}

impl Lifter for Scripted {
    fn lift(&self, _header: &str, case: &Value, dir: &Path) -> Result<Lift, String> {
        std::fs::create_dir_all(dir).unwrap();
        let n0 = case["fixed"][0].as_i64();
        let w = if n0.is_some() {
            &self.w_fixed
        } else {
            &self.w_free
        };
        let (rec, cert) = rule_and_cert(
            &self.terms,
            self.m,
            w,
            n0,
            json!([]),
            case["index"].as_i64().unwrap(),
        );
        let cp = dir.join("cert.json");
        std::fs::write(&cp, cert.to_string()).unwrap();
        Ok(Lift {
            record: rec,
            certs: vec![cp],
            seconds: 0.01,
            peak_mb: 1.0,
            binary_sha256: "scripted".into(),
            provenance: json!({"tool": "scripted test lifter", "case": case["fixed"]}),
        })
    }
}

#[test]
fn open_locus_becomes_a_ladder_of_two_pieces_that_partition_the_domain() {
    let Some(bin) = checker_bin() else { return };
    let _g = ONE_CHECKER.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tmpdir("ladder");
    let r = rows(&bin, &dir);
    let (m, terms) = pick_row(&r);
    let (orig, _) = rule_and_cert(&terms, m, "1", None, json!([]), 7);
    let lifter = Scripted {
        terms: terms.clone(),
        m,
        w_free: "1/(n0-3)".into(),
        w_fixed: "1".into(),
    };
    let em = emitter(&bin, &dir);
    let cfg = LadderConfig {
        witness_modes: vec![false],
        ..Default::default()
    };
    let rep = certify_by_replacement(&header(), &[], &orig, 7, &lifter, &em, &cfg, &dir.join("l"))
        .unwrap();
    assert_eq!(rep.outcome, Outcome::Closed, "{:?}", rep.log);
    assert_eq!(rep.pieces.len(), 2);
    assert_eq!(rep.pieces[0].record["exceptions"], json!([["-3+n0"]]));
    assert_eq!(rep.pieces[1].record["fixed"][0], json!(3));
    assert!(
        rep.pieces
            .iter()
            .all(|p| p.emission.classed.class == ClassWord::WholeDomain)
    );
    assert!(rep.partition.as_ref().unwrap().0);
    let lines = rep.lines(7).unwrap();
    assert!(lines[1].contains("\"index\":8"));
    // the positioned pieces pass the validator
    let pos: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(k, l)| rustred_campaign_core::economy::improve::set_index(l, k as i64).unwrap())
        .collect();
    // (the emitter's side directory is the one of DIR/t.table.jsonl: the table must be written there)
    let (s, ok) = check_table(&write_table(&dir, &pos), &bin, false);
    assert!(ok, "{s}");
    // the same lines in another directory have no side files: refused
    let (_, ok) = check_table(&write_table(&dir.join("l"), &pos), &bin, false);
    assert!(!ok);
}

#[test]
fn a_ladder_that_does_not_close_within_its_depth_writes_nothing() {
    let Some(bin) = checker_bin() else { return };
    let _g = ONE_CHECKER.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tmpdir("depth");
    let r = rows(&bin, &dir);
    let (m, terms) = pick_row(&r);
    let (orig, _) = rule_and_cert(&terms, m, "1", None, json!([]), 0);
    // the child (n0 = 3) has an open locus of its own (n1 = 2): depth 1 does not close
    let lifter = Scripted {
        terms,
        m,
        w_free: "1/(n0-3)".into(),
        w_fixed: "1/(n1-2)".into(),
    };
    let em = emitter(&bin, &dir);
    let cfg = LadderConfig {
        max_depth: 1,
        witness_modes: vec![false],
        ..Default::default()
    };
    let rep = certify_by_replacement(&header(), &[], &orig, 0, &lifter, &em, &cfg, &dir.join("l"))
        .unwrap();
    assert!(
        matches!(rep.outcome, Outcome::DepthExceeded { .. }),
        "{:?}",
        rep.outcome
    );
    assert!(rep.lines(0).is_err(), "nothing is written as whole domain");
}
