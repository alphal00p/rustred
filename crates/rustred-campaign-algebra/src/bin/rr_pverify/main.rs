//! rr_pverify: INDEPENDENT verifier of a parametric closure certificate of a node table (lane pverify, 2026-11-16).
//!
//! It reads a `table_dump.v1` node table and checks, with its own code and exact arithmetic only:
//! coverage at first match on a region D + r <= R (face recursion, `cover`), strict descent under the header's node
//! order (`descent`), regularity of the coefficients on each rule's selected keys (`poles`), the safety of the engine's
//! two-prime zero test on the region (`radius`), and the input binding (`hashes`, `origins`). The engine semantics
//! it relies on are re-derived from the engine's sources (rustred-core / rustred-order); no code of the producer of
//! the certificate is used. Generic: nothing here knows a node, an arity or a count; all of it comes from the table.

mod coef;
mod cover;
mod descent;
mod dispatch;
mod expr;
mod face;
mod keys;
mod order;
mod origins;
mod poles;
mod radius;
mod rat;
mod reduce;
mod selftest;
mod sha256;
mod symcheck;
mod table;

use std::io::Write;

use serde_json::{Value, json};

pub struct Args {
    v: Vec<String>,
}

impl Args {
    fn take(&mut self, k: &str) -> Option<String> {
        let i = self.v.iter().position(|x| x == k)?;
        if i + 1 >= self.v.len() {
            die(&format!("{k} needs a value"));
        }
        let val = self.v.remove(i + 1);
        self.v.remove(i);
        Some(val)
    }
    fn need(&mut self, k: &str) -> String {
        self.take(k).unwrap_or_else(|| die(&format!("missing {k}")))
    }
    fn flag(&mut self, k: &str) -> bool {
        if let Some(i) = self.v.iter().position(|x| x == k) {
            self.v.remove(i);
            true
        } else {
            false
        }
    }
    fn done(&self) {
        if !self.v.is_empty() {
            die(&format!("unknown arguments {:?}", self.v));
        }
    }
}

pub fn die(m: &str) -> ! {
    eprintln!("rr_pverify: {m}");
    std::process::exit(2)
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn write_json(path: &str, v: &Value) {
    let mut f = std::fs::File::create(path).unwrap_or_else(|e| die(&format!("{path}: {e}")));
    writeln!(f, "{}", serde_json::to_string_pretty(v).unwrap()).unwrap();
}

fn load(path: &str) -> table::Table {
    let t0 = now();
    let t = table::Table::load(path).unwrap_or_else(|e| die(&e));
    eprintln!(
        "loaded {} rules, {} masters, n = {}, end record {} in {:.1} s",
        t.rules.len(),
        t.masters.len(),
        t.n,
        t.has_end,
        now() - t0
    );
    t
}

fn parse_radius(s: &str) -> Option<i128> {
    if s == "none" {
        None
    } else {
        Some(
            s.parse()
                .unwrap_or_else(|_| die("--radius: an integer or none")),
        )
    }
}

fn cmd_cover(mut a: Args) {
    let tp = a.need("--table");
    let r = parse_radius(&a.need("--radius"));
    let out = a.take("--out");
    let no_implicit = a.flag("--no-implicit");
    let drop: Vec<usize> = a
        .take("--drop")
        .map(|s| {
            s.split(',')
                .map(|x| x.parse().unwrap_or_else(|_| die("--drop: positions")))
                .collect()
        })
        .unwrap_or_default();
    a.done();
    let mut t = load(&tp);
    for &d in &drop {
        t.rules[d].skip = true;
    }
    for (i, r) in t.rules.iter().enumerate() {
        if !table::target_is_canonical(r) {
            die(&format!(
                "rule {i}: target is not canonical (identity on free slots, fixed constants): not handled"
            ));
        }
    }
    let t0 = now();
    let mut c = cover::Cover::new(&t, r).unwrap_or_else(|e| die(&e));
    c.close_implicit = !no_implicit;
    let o = c.solve(&face::Face::top(), 0);
    let secs = now() - t0;
    let masters: std::collections::BTreeSet<Vec<i64>> = t.masters.iter().cloned().collect();
    let unc: Vec<&Vec<i64>> = o.uncovered.iter().collect();
    let not_master: Vec<&&Vec<i64>> = unc.iter().filter(|k| !masters.contains(**k)).collect();
    let res = json!({
        "schema": "rr_pverify.cover.v1",
        "table": tp,
        "radius": r.map(|x| x.to_string()),
        "dropped_rules": drop,
        "uncovered": unc,
        "uncovered_not_master": not_master,
        "undecided": o.undecided.iter().map(|&i| c.undecided_log[i].clone()).collect::<Vec<_>>(),
        "stats": c.stats,
        "rules_used": c.used_rules.len(),
        "orphan_faces": c.orphan_log,
        "seconds": secs,
        "verdict": if o.undecided.is_empty() && not_master.is_empty() { "YES" } else { "NO" },
    });
    println!("{}", serde_json::to_string(&res).unwrap());
    if let Some(p) = out {
        write_json(&p, &res);
    }
}

fn cmd_dispatch(mut a: Args) {
    let tp = a.need("--table");
    let kp = a.need("--keys");
    let op = a.need("--out");
    a.done();
    let t = load(&tp);
    let keys = keys::read_keys(&kp).unwrap_or_else(|e| die(&e));
    let mut f =
        std::io::BufWriter::new(std::fs::File::create(&op).unwrap_or_else(|e| die(&e.to_string())));
    let t0 = now();
    for k in &keys {
        if k.len() != t.n {
            die("key arity");
        }
        let p = dispatch::first_match(&t, k);
        let label = p.and_then(|i| t.rules[i].label);
        writeln!(f, "{}", json!({"key": k, "position": p, "label": label})).unwrap();
    }
    eprintln!("dispatched {} keys in {:.1} s", keys.len(), now() - t0);
}

fn cmd_hashes(mut a: Args) {
    let d = a.need("--deliver");
    a.done();
    let r = origins::check_manifest(&d);
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
}

fn main() {
    let mut v: Vec<String> = std::env::args().skip(1).collect();
    if v.is_empty() {
        die(
            "usage: rr_pverify hashes|origins|cover|dispatch|enum|random|compare|descent|poles|radius ...",
        );
    }
    let cmd = v.remove(0);
    let a = Args { v };
    match cmd.as_str() {
        "hashes" => cmd_hashes(a),
        "origins" => origins::cmd(a),
        "cover" => cmd_cover(a),
        "dispatch" => cmd_dispatch(a),
        "enum" => keys::cmd_enum(a),
        "random" => keys::cmd_random(a),
        "compare" => keys::cmd_compare(a),
        "descent" => descent::cmd(a),
        "poles" => poles::cmd(a),
        "radius" => radius::cmd(a),
        "symcheck" => symcheck::cmd(a),
        "samplepoles" => poles::cmd_samplepoles(a),
        "myreduce" => reduce::cmd(a),
        "onelevel" => reduce::cmd_onelevel(a),
        "termcheck" => reduce::cmd_termcheck(a),
        _ => die(&format!("unknown command {cmd}")),
    }
}
