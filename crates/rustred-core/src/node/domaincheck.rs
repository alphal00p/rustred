//! `rr_node domaincheck` (EC-15): the engine's own rule matcher, key by key, without reducing anything.
//!
//! For every key the verb reports what [`Table::reduce_mod`] would decide at it: sector membership
//! ([`Table::in_sector`]), zero sector, master, and the rule [`Table::find_rule`] selects. The DECISION always comes from
//! those unchanged engine functions. A diagnostic walk in this module follows `find_rule`'s steps to count why earlier
//! rules were rejected and to flag places where the engine's numeric tests (two primes, two values of `d`, equations at
//! `d = 0`) could differ from exact ones; its winner is compared with `find_rule`'s on every key and a disagreement is an
//! internal error (exit 3), never a second opinion. Interface: plan/auto-campaign/interfaces/domaincheck.md
//! (schema `rustred.domaincheck.v1`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use serde_json::{Value, json};

use super::mastermap::{self, Key, Table, key_text};
use super::replay::arith::{self, Q};
use super::replay::expr::Expr;
use super::replay::{self as replay_mod};

/// Output schema of the verb.
pub const SCHEMA: &str = "rustred.domaincheck.v1";
/// Options the verb accepts (reported by `rr_node --capabilities`).
pub const OPTIONS: [&str; 6] = [
    "--table",
    "--keys",
    "--out",
    "--masters",
    "--point-valid",
    "--dims",
];

/// The two values of `d` of `find_rule`'s exception test: the engine's own constant (audit-engjob m9).
use super::mastermap::EXCEPTION_D;
/// Further values of `d` of the ambiguity diagnostic.
const EXTRA_D: [Q; 3] = [Q { n: 9, d: 13 }, Q { n: 1, d: 3 }, Q { n: 5, d: 11 }];

/// Arguments of one run.
pub struct DomainCheckArgs {
    pub table: String,
    /// A key file path, or a literal list `K1;K2;..`.
    pub keys: String,
    pub out: String,
    /// Replaces the table's masters, as `rr_node reduce --masters`.
    pub masters: Option<Vec<Key>>,
    /// Evaluate the replay's `point_valid` for the selected rule at its rule point.
    pub point_valid: bool,
    /// d values of `point_valid`.
    pub dims: Vec<Q>,
}

/// Failure classes, mapped to exit codes by [`DomainCheckError::exit_code`].
#[derive(Debug)]
pub enum DomainCheckError {
    /// Table does not load, malformed key line, key of the wrong length (exit 41).
    Input(String),
    /// Writing the output failed (exit 51).
    Io(String),
    /// The diagnostic walk disagrees with `find_rule` (exit 3).
    Internal(String),
}

impl DomainCheckError {
    pub fn exit_code(&self) -> i32 {
        match self {
            DomainCheckError::Input(_) => 41,
            DomainCheckError::Io(_) => 51,
            DomainCheckError::Internal(_) => 3,
        }
    }
    pub fn message(&self) -> &str {
        match self {
            DomainCheckError::Input(m)
            | DomainCheckError::Io(m)
            | DomainCheckError::Internal(m) => m,
        }
    }
}

/// What this module reads from a rule record beyond `Table::load` (metadata for the report).
struct RecordMeta {
    index: Value,
    line: usize,
    chart: Value,
    dispatch: Value,
    witness_elided: bool,
    recenter_nonzero: bool,
    exception_branches: usize,
    /// The raw record, kept only for `--point-valid`.
    raw: Option<Value>,
}

/// Header facts and the per-record metadata, in the order `Table::load` pushes rules.
struct Scan {
    records: Vec<RecordMeta>,
    charts: HashMap<String, Value>,
    node_order_name: Value,
    slot_permutation: Value,
    residuals: usize,
    terminal_ledger: bool,
    skipped_by_dispatch: BTreeMap<String, usize>,
}

fn scan_table(path: &str, keep_raw: bool) -> Result<Scan, String> {
    let f = BufReader::with_capacity(
        1 << 24,
        std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?,
    );
    let mut s = Scan {
        records: vec![],
        charts: HashMap::new(),
        node_order_name: Value::Null,
        slot_permutation: Value::Null,
        residuals: 0,
        terminal_ledger: false,
        skipped_by_dispatch: BTreeMap::new(),
    };
    for (no, line) in f.lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&line).map_err(|e| format!("{path}: {e}"))?;
        match v["type"].as_str().unwrap_or("") {
            "header" => {
                s.node_order_name = v["node_order"]["name"].clone();
                s.slot_permutation = v["node_order"]["slot_permutation"].clone();
            }
            "chart" => {
                let c = v["chart"].as_str().unwrap_or("").to_string();
                s.charts.entry(c).or_insert(v);
            }
            "residual" => s.residuals += 1,
            "composite_terminal_ledger" => s.terminal_ledger = true,
            "rule" => {
                // the condition of `Table::load` for `skip`, verbatim
                let skip = !matches!(
                    v.get("dispatch").and_then(Value::as_str),
                    None | Some("Partition")
                );
                let dispatch = v.get("dispatch").cloned().unwrap_or(Value::Null);
                if skip {
                    let name = dispatch.as_str().unwrap_or_default().to_string();
                    *s.skipped_by_dispatch.entry(name).or_insert(0) += 1;
                }
                let witness_elided = v["witness_loci"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x.as_str() == Some("elided")));
                let recenter_nonzero = v["recenter"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x.as_i64() != Some(0)));
                s.records.push(RecordMeta {
                    index: v["index"].clone(),
                    line: no + 1,
                    chart: v["chart"].clone(),
                    dispatch,
                    witness_elided,
                    recenter_nonzero,
                    exception_branches: v["exceptions"].as_array().map_or(0, Vec::len),
                    raw: keep_raw.then_some(v),
                });
            }
            _ => {}
        }
    }
    Ok(s)
}

/// Per-key result of the diagnostic walk.
#[derive(Default)]
struct Trace {
    winner: Option<(usize, Vec<i64>)>,
    skipped_dispatch: usize,
    fixed_mismatch: usize,
    target_mismatch: usize,
    equation_nonzero: usize,
    equation_error: usize,
    exception_hit: usize,
    exception_eval_error: usize,
    ambiguity: Vec<String>,
}

/// Exact-zero diagnostic of one literal that the engine read as zero at the given engine `d` values: `None` if no
/// further evaluation contradicts it, else the reason class.
fn contradicts_zero(e: &Expr, point: &[i64], engine_d: &[Q]) -> Option<&'static str> {
    if e.has_d() {
        for d in EXTRA_D {
            match e.at_point(arith::P, point, d) {
                Ok(0) => {}
                Ok(_) => return Some("exception_d"),
                Err(_) => return Some("unchecked"),
            }
        }
        None
    } else {
        let d = engine_d[0];
        for &p in &mastermap::PRIMES[2..] {
            match e.at_point(p, point, d) {
                Ok(0) => {}
                Ok(_) => return Some("modular"),
                Err(_) => return Some("unchecked"),
            }
        }
        None
    }
}

/// `find_rule`'s steps with counts (see the module doc: the decision itself is `find_rule`'s).
fn walk(table: &Table, k: &[i64]) -> Trace {
    let n = table.n;
    let mut t = Trace::default();
    'r: for (ri, r) in table.rules.iter().enumerate() {
        if r.skip {
            t.skipped_dispatch += 1;
            continue;
        }
        for (i, fx) in r.fixed.iter().enumerate() {
            if let Some(x) = fx {
                if k[i] != *x {
                    t.fixed_mismatch += 1;
                    continue 'r;
                }
            }
        }
        let mut pt = vec![i64::MIN; n];
        for (j, (a, o)) in r.target.iter().enumerate() {
            match a {
                Some(a) => pt[*a] = k[j] - o,
                None => {
                    if k[j] != *o {
                        t.target_mismatch += 1;
                        continue 'r;
                    }
                }
            }
        }
        for (x, f) in pt.iter_mut().zip(r.fixed.iter()).take(n) {
            if *x == i64::MIN {
                *x = f.unwrap_or(0);
            }
        }
        let mut eq_amb: Vec<String> = vec![];
        for e in &r.equations {
            match e.is_zero_at(&pt, Q::ZERO) {
                Ok(true) => {
                    if let Some(c) = contradicts_zero(e, &pt, &[Q::ZERO]) {
                        let c = if c == "exception_d" { "equation_d" } else { c };
                        eq_amb.push(format!("{c}:{ri}:eq"));
                    }
                }
                Ok(false) => {
                    t.equation_nonzero += 1;
                    continue 'r;
                }
                Err(_) => {
                    t.equation_error += 1;
                    continue 'r;
                }
            }
        }
        for (bi, br) in r.exceptions.iter().enumerate() {
            let mut hit = true;
            'lit: for e in br {
                for d in EXCEPTION_D {
                    match e.is_zero_at(&pt, d) {
                        Ok(true) => {}
                        Ok(false) => {
                            hit = false;
                            break 'lit;
                        }
                        Err(_) => {
                            t.exception_eval_error += 1;
                            hit = false;
                            break 'lit;
                        }
                    }
                }
            }
            if hit {
                t.exception_hit += 1;
                // the engine rejects here; an exact test that does not hit would go on with this rule
                if let Some(c) = br
                    .iter()
                    .find_map(|e| contradicts_zero(e, &pt, &EXCEPTION_D))
                {
                    t.ambiguity.push(format!("{c}:{ri}:{bi}"));
                }
                continue 'r;
            }
        }
        // selected: an equation the engine read as zero that an exact test would not would change the decision
        t.ambiguity.extend(eq_amb);
        t.winner = Some((ri, pt));
        return t;
    }
    t
}

fn parse_key_line(line: &str) -> Result<Option<Key>, String> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return Ok(None);
    }
    if t.starts_with('[') {
        let v: Value = serde_json::from_str(t).map_err(|e| format!("bad key line {t}: {e}"))?;
        let a = v.as_array().ok_or_else(|| format!("bad key line {t}"))?;
        let ints = match a.first() {
            Some(Value::Array(inner)) => inner,
            _ => a,
        };
        return ints
            .iter()
            .map(|x| x.as_i64().ok_or_else(|| format!("bad key line {t}")))
            .collect::<Result<Key, String>>()
            .map(Some);
    }
    mastermap::parse_key(t).map(Some)
}

fn q_text(q: &Q) -> String {
    if q.d == 1 {
        q.n.to_string()
    } else {
        format!("{}/{}", q.n, q.d)
    }
}

/// Run the verb. Returns the number of keys written.
pub fn run(args: &DomainCheckArgs) -> Result<usize, DomainCheckError> {
    use DomainCheckError::{Input, Internal, Io};
    let mut table = Table::load(&args.table).map_err(Input)?;
    let scan = scan_table(&args.table, args.point_valid).map_err(Input)?;
    if scan.records.len() != table.rules.len() {
        return Err(Internal(format!(
            "{}: {} rule records scanned, Table::load holds {}",
            args.table,
            scan.records.len(),
            table.rules.len()
        )));
    }
    let skipped_load = table.rules.iter().filter(|r| r.skip).count();
    if skipped_load != scan.skipped_by_dispatch.values().sum::<usize>() {
        return Err(Internal(format!(
            "{}: Table::load skips {skipped_load} rules, the scan counts {:?}",
            args.table, scan.skipped_by_dispatch
        )));
    }
    let masters_source = if args.masters.is_some() {
        "--masters"
    } else if scan.residuals > 0 {
        "residual"
    } else if scan.terminal_ledger && !table.masters.is_empty() {
        "terminal_ledger"
    } else {
        "none"
    };
    if let Some(m) = &args.masters {
        table.masters = m.clone();
    }
    let masters: HashSet<Key> = table.masters.iter().cloned().collect();
    let bytes = std::fs::read(&args.table).map_err(|e| Input(format!("{}: {e}", args.table)))?;
    let digest = blake3::hash(&bytes).to_hex().to_string();
    drop(bytes);

    let tmp = format!("{}.tmp", args.out);
    let file = std::fs::File::create(&tmp).map_err(|e| Io(format!("{tmp}: {e}")))?;
    let mut w = BufWriter::new(file);
    let put = |v: &Value, w: &mut BufWriter<std::fs::File>| -> Result<(), DomainCheckError> {
        writeln!(w, "{v}").map_err(|e| Io(format!("{tmp}: {e}")))
    };
    let header = json!({
        "schema": SCHEMA, "type": "header", "table": args.table, "table_blake3": digest,
        "denominators": table.n,
        "mask": table.mask.iter().map(|&b| if b { '1' } else { '0' }).collect::<String>(),
        "node_order_name": scan.node_order_name, "slot_permutation": scan.slot_permutation,
        "rules": table.rules.len(),
        "rules_skipped_at_load": {"by_dispatch": scan.skipped_by_dispatch},
        "masters": table.masters.len(), "masters_source": masters_source,
        "zero_sectors": table.zero.len(),
        "exception_d": EXCEPTION_D.iter().map(q_text).collect::<Vec<_>>(),
        "equation_d": "0", "zero_test_primes": 2,
        "point_valid_dims": if args.point_valid {
            json!(args.dims.iter().map(q_text).collect::<Vec<_>>())
        } else {
            Value::Null
        },
    });
    put(&header, &mut w)?;

    // keys: a file, or the literal list of `reduce --keys`
    let key_source: Box<dyn Iterator<Item = Result<Option<Key>, String>>> =
        if Path::new(&args.keys).is_file() {
            let f = std::fs::File::open(&args.keys)
                .map_err(|e| Input(format!("{}: {e}", args.keys)))?;
            Box::new(BufReader::new(f).lines().map(|l| match l {
                Ok(l) => parse_key_line(&l),
                Err(e) => Err(e.to_string()),
            }))
        } else {
            let ks = mastermap::parse_keys(&args.keys).map_err(Input)?;
            Box::new(ks.into_iter().map(|k| Ok(Some(k))))
        };

    let mut pv_cache: HashMap<usize, (replay_mod::Rule, Vec<Expr>)> = HashMap::new();
    let mut by_decision: BTreeMap<&str, usize> = BTreeMap::new();
    let mut wins: BTreeMap<usize, usize> = BTreeMap::new();
    let (mut count, mut ambiguous, mut undecided) = (0usize, 0usize, 0usize);
    for item in key_source {
        let Some(k) = item.map_err(Input)? else {
            continue;
        };
        count += 1;
        if k.len() != table.n {
            return Err(Input(format!(
                "key {} (number {count}) has {} indices, the table {}",
                key_text(&k),
                k.len(),
                table.n
            )));
        }
        let mut rec = json!({"type": "key", "n": count, "key": key_text(&k), "in_sector": Value::Null,
            "sector_error": Value::Null, "zero_sector": false, "master": false, "selected": Value::Null,
            "trace": Value::Null, "decided": true, "ambiguous": false, "ambiguity": [], "point_valid": Value::Null});
        let decision = match table.in_sector(&k) {
            Err(e) => {
                rec["sector_error"] = json!(e);
                "error"
            }
            Ok(false) => {
                rec["in_sector"] = json!(false);
                let zero = table
                    .zero
                    .contains(&k.iter().map(|&x| x > 0).collect::<Vec<bool>>());
                rec["zero_sector"] = json!(zero);
                if zero { "zero" } else { "lower" }
            }
            Ok(true) => {
                rec["in_sector"] = json!(true);
                let is_master = masters.contains(&k);
                rec["master"] = json!(is_master);
                let engine = table.find_rule(&k);
                let tr = walk(&table, &k);
                if tr.winner != engine {
                    return Err(Internal(format!(
                        "diagnostic walk disagrees with find_rule at {}: walk {:?}, find_rule {:?}",
                        key_text(&k),
                        tr.winner.as_ref().map(|w| w.0),
                        engine.as_ref().map(|w| w.0)
                    )));
                }
                rec["trace"] = json!({"skipped_dispatch": tr.skipped_dispatch, "fixed_mismatch": tr.fixed_mismatch,
                    "target_mismatch": tr.target_mismatch, "equation_nonzero": tr.equation_nonzero,
                    "equation_error": tr.equation_error, "exception_hit": tr.exception_hit,
                    "exception_eval_error": tr.exception_eval_error});
                let decided = tr.equation_error + tr.exception_eval_error == 0;
                rec["decided"] = json!(decided);
                undecided += usize::from(!decided);
                if !tr.ambiguity.is_empty() {
                    ambiguous += 1;
                    rec["ambiguous"] = json!(true);
                    rec["ambiguity"] = json!(tr.ambiguity);
                }
                if let Some((ri, pt)) = &engine {
                    let m = &scan.records[*ri];
                    rec["selected"] = json!({"rule": ri, "index": m.index, "line": m.line, "chart": m.chart,
                        "dispatch": m.dispatch, "point": pt, "witness_elided": m.witness_elided,
                        "recenter_nonzero": m.recenter_nonzero, "exception_branches": m.exception_branches});
                    *wins.entry(*ri).or_insert(0) += 1;
                    if args.point_valid {
                        rec["point_valid"] = json!(point_valid_text(
                            &scan,
                            &mut pv_cache,
                            *ri,
                            table.n,
                            pt,
                            &args.dims
                        ));
                    }
                }
                if is_master {
                    "master"
                } else if engine.is_some() {
                    "rule"
                } else {
                    "none"
                }
            }
        };
        rec["decision"] = json!(decision);
        *by_decision.entry(decision).or_insert(0) += 1;
        put(&rec, &mut w)?;
    }
    let summary = json!({"type": "summary", "keys": count, "by_decision": by_decision, "ambiguous": ambiguous,
        "undecided": undecided,
        "rule_wins": wins.iter().map(|(r, c)| (r.to_string(), json!(c))).collect::<serde_json::Map<_, _>>()});
    put(&summary, &mut w)?;
    w.flush().map_err(|e| Io(format!("{tmp}: {e}")))?;
    drop(w);
    std::fs::rename(&tmp, &args.out).map_err(|e| Io(format!("{}: {e}", args.out)))?;
    Ok(count)
}

/// The replay's `point_valid` verdict for rule `ri` at `pt` ("ok" or the engine's text).
fn point_valid_text(
    scan: &Scan,
    cache: &mut HashMap<usize, (replay_mod::Rule, Vec<Expr>)>,
    ri: usize,
    n: usize,
    pt: &[i64],
    dims: &[Q],
) -> String {
    let m = &scan.records[ri];
    let Some(raw) = &m.raw else {
        return "raw record not kept".into();
    };
    if let std::collections::hash_map::Entry::Vacant(slot) = cache.entry(ri) {
        let rule = match replay_mod::rule_from(raw) {
            Ok(r) => r,
            Err(e) => return format!("rule record: {e}"),
        };
        let coefs = raw["rhs"].as_array().map_or_else(
            || Err("rule rhs".to_string()),
            |a| {
                a.iter()
                    .map(|t| Expr::parse(t[1].as_str().unwrap_or(""), n))
                    .collect::<Result<Vec<_>, _>>()
            },
        );
        match coefs {
            Ok(c) => {
                slot.insert((rule, c));
            }
            Err(e) => return format!("coefficient: {e}"),
        }
    }
    let (rule, coefs) = &cache[&ri];
    let chart = m.chart.as_str().and_then(|c| scan.charts.get(c));
    match replay_mod::point_valid(rule, pt, dims, chart, coefs) {
        Ok(()) => "ok".into(),
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = r#"{"type":"header","schema":"table_dump.v1","denominators":3,"mask":"110","power_shifts":["0","0","0"]}
{"type":"zero_sectors","masks":["010"]}
{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"]}
{"type":"rule","chart":"110","index":0,"fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)","rhs":[["I(n0-1,n1,n2)","1"]],"exceptions":[],"dispatch":"AfterBaselinePartitionWholePiece"}
{"type":"rule","chart":"110","index":1,"fixed":[2,null,null],"equations":[],"target":"I(2,n1,n2)","rhs":[["I(1,n1,n2)","1"]],"exceptions":[["n1-1"]],"dispatch":"Partition"}
{"type":"rule","chart":"110","index":2,"fixed":[null,null,null],"equations":["n0-n1"],"target":"I(n0,n1,n2)","rhs":[["I(n0-1,n1,n2)","n0"]],"exceptions":[],"witness_loci":["elided"]}
{"type":"rule","chart":"110","index":3,"fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)","rhs":[["I(n0,n1-1,n2)","1"]],"exceptions":[["n2"]],"recenter":[0,0,-1]}
{"type":"residual","chart":"110","key":"I(1,1,-1)"}
"#;

    fn run_on(keys: &str) -> Vec<Value> {
        let dir = std::env::temp_dir().join(format!(
            "rr-domaincheck-{}-{}",
            std::process::id(),
            keys.len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let t = dir.join("F.table.jsonl");
        std::fs::write(&t, TABLE).unwrap();
        let out = dir.join("OUT.jsonl");
        let args = DomainCheckArgs {
            table: t.display().to_string(),
            keys: keys.into(),
            out: out.display().to_string(),
            masters: None,
            point_valid: true,
            dims: mastermap::check_dims(),
        };
        run(&args).unwrap();
        let v = std::fs::read_to_string(&out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        v
    }

    #[test]
    fn decisions_and_reasons() {
        let v = run_on(
            "I(2,2,0);I(2,1,0);I(3,3,0);I(3,2,0);I(3,2,-1);I(1,1,-1);I(0,1,0);I(1,0,0);I(1,1,1)",
        );
        assert_eq!(v[0]["rules"], 4);
        assert_eq!(
            v[0]["rules_skipped_at_load"]["by_dispatch"]["AfterBaselinePartitionWholePiece"],
            1
        );
        let sel = |i: usize| v[i]["selected"]["rule"].as_u64();
        // fixed slot + exception not hit
        assert_eq!(sel(1), Some(1));
        assert_eq!(v[1]["trace"]["skipped_dispatch"], 1);
        // exception n1-1 hits at n1 = 1 -> rule 2 equation n0-n1 fails (2 != 1) -> rule 3, exception n2 hits at 0
        assert_eq!(sel(2), None);
        assert_eq!(v[2]["decision"], "none");
        assert_eq!(v[2]["trace"]["exception_hit"], 2);
        // equation n0-n1 holds: elided record selected (the engine does not consult witness loci)
        assert_eq!(sel(3), Some(2));
        assert_eq!(v[3]["selected"]["witness_elided"], true);
        // equation fails, exception n2 hits
        assert_eq!(sel(4), None);
        assert_eq!(v[4]["trace"]["equation_nonzero"], 1);
        assert_eq!(sel(5), Some(3));
        assert_eq!(v[5]["selected"]["recenter_nonzero"], true);
        assert_eq!(v[5]["point_valid"], "ok");
        // master: find_rule still reported
        assert_eq!(v[6]["decision"], "master");
        assert_eq!(sel(6), Some(2));
        // lower sector (n0 = 0): mask of positives 010 is a zero sector
        assert_eq!(v[7]["decision"], "zero");
        assert_eq!(v[8]["decision"], "lower");
        // ISP in a denominator
        assert_eq!(v[9]["decision"], "error");
        assert_eq!(v.last().unwrap()["keys"], 9);
    }

    #[test]
    fn key_line_forms() {
        assert_eq!(parse_key_line("1,2,0").unwrap(), Some(vec![1, 2, 0]));
        assert_eq!(parse_key_line("I(1,2,0)").unwrap(), Some(vec![1, 2, 0]));
        assert_eq!(parse_key_line("[1,2,0]").unwrap(), Some(vec![1, 2, 0]));
        assert_eq!(
            parse_key_line("[[1,2,0],17,3]").unwrap(),
            Some(vec![1, 2, 0])
        );
        assert_eq!(parse_key_line("  # c").unwrap(), None);
        assert!(parse_key_line("[1,x]").is_err());
    }
}
