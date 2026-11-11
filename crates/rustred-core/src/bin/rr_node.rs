//! `rr_node`: node-level RustRed commands consolidated from the 5L campaign (rr-consolidate).
//!
//! ```text
//! rr_node generate --family F.sym --out DIR [--order auto|id|NAME|JSON] [--chart MASK] [--initial 'SPEC [EQS]']...
//!                  [--backend targetonlyfactorized|factorized|sparse|targetonly|sampled] [--max-depth K] [--max-cases K]
//!                  [--native-try S] [--no-content-fallback] [--admit-try S] [--no-screen] [--screen MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS]
//!                  [--on-heavy auto|refuse|sampled|proceed] [--sampled-from-rows R] [--retry-source-orders reverse,rot:7]
//!                  [--symfirst-from-rows R|none] [--sampled-budget S] [--max-box K] [--min-target-block K] [--sampled-threads K]
//!                  [--max-rule-monomials K|none] [--seed-schedule spired|symfirst] [--no-symbolic-first-rung] [--no-empty-domain-check]
//!                  [--case-budget S] [--max-attempts K] [--attempt-cap S] [--probe-cap S] [--probe-depth K]
//!                  [--probe-fallback-depth K|none] [--probe-parallel P] [--probe-portfolio K] [--cover PATH:UPTO]... [--cand-dump]
//! rr_node probe    --family F.sym --case 'SPEC [EQS]' --out DIR [--variants all|NAME,..] [--cap S] [--depth K] [--parallel P] [--portfolio K]
//! rr_node variants --family F.sym --case 'SPEC [EQS]'
//! rr_node descent  --table F.table.jsonl [--order JSON|NAME]
//! rr_node replay   --table F.table.jsonl --out DIR [--family F.sym] [--cert F.cert.jsonl] [--cand-dump-dir D]
//!                  [--bindings B.json] [--rules all|0-9,12] [--chart MASK] [--dimensions 17/7 19/7 23/7] [--sample-attempts 128]
//! ```
//! Exit status: generate 0 complete / 1 not complete / 2 usage; replay 0 PASS / 1 FAIL / 2 UNKNOWN; descent 0 pass / 1 fail.

use std::path::{Path, PathBuf};

use rustred::node::auto::{AutoOptions, ProbeOptions, generate_auto, probe};
use rustred::node::descent::check_table_file;
use rustred::node::driver::{CaseSpec, NodeOptions, SeedScheduleName, generate};
use rustred::node::order::{NodeOrder, SourceOrder, variants};
use rustred::node::replay::{self, ReplayArgs};
use rustred::node::screen::{HeavyAction, LiftScreen};

struct Args {
    items: Vec<String>,
}

impl Args {
    fn take(&mut self, flag: &str) -> Option<String> {
        let at = self.items.iter().position(|a| a == flag)?;
        if at + 1 >= self.items.len() {
            die(&format!("{flag} needs a value"));
        }
        self.items.remove(at);
        Some(self.items.remove(at))
    }
    fn take_all(&mut self, flag: &str) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(v) = self.take(flag) {
            out.push(v);
        }
        out
    }
    fn flag(&mut self, flag: &str) -> bool {
        match self.items.iter().position(|a| a == flag) {
            Some(at) => {
                self.items.remove(at);
                true
            }
            None => false,
        }
    }
    fn need(&mut self, flag: &str) -> String {
        self.take(flag).unwrap_or_else(|| die(&format!("missing {flag}")))
    }
    fn parse<T: std::str::FromStr>(&mut self, flag: &str) -> Option<T> {
        self.take(flag).map(|v| v.parse().unwrap_or_else(|_| die(&format!("{flag}: bad value {v}"))))
    }
    fn done(&self) {
        if !self.items.is_empty() {
            die(&format!("unknown arguments {:?}", self.items));
        }
    }
}

fn die(msg: &str) -> ! {
    eprintln!("rr_node: {msg}");
    std::process::exit(2)
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| die(&format!("{path}: {e}")))
}

fn case_arg(text: &str) -> CaseSpec {
    let mut it = text.split_whitespace();
    let spec = it.next().unwrap_or_else(|| die("empty case"));
    let eqs = it.collect::<Vec<_>>().join("");
    CaseSpec::parse(spec, &eqs).unwrap_or_else(|e| die(&e))
}

/// An order given as JSON, `id`, or a variant name of the family's whole top chart / the given case.
fn order_arg(text: &str, family: &str, case: Option<&CaseSpec>) -> NodeOrder {
    if text.trim_start().starts_with('{') {
        return NodeOrder::from_json(text).unwrap_or_else(|e| die(&e));
    }
    if text == "id" {
        return NodeOrder::identity();
    }
    let n = rustred::node::order::family_props(family).len();
    let all_free = CaseSpec { fixed: vec![None; n], equations: vec![] };
    let c = case.unwrap_or(&all_free);
    variants(family, &c.fixed, &c.equations, 1000)
        .into_iter()
        .find(|o| o.name == text)
        .unwrap_or_else(|| die(&format!("unknown order {text} (id, auto, a JSON order, or a variant name)")))
}

fn node_options(a: &mut Args, family: &str) -> NodeOptions {
    let mut o = NodeOptions::default();
    o.chart = a.take("--chart");
    o.initial = a.take_all("--initial").iter().map(|s| case_arg(s)).collect();
    if let Some(b) = a.take("--backend") {
        o.exact_backend = b;
    }
    o.max_depth = a.parse("--max-depth");
    o.max_cases = a.parse("--max-cases");
    if let Some(s) = a.take("--native-try") {
        o.native_try = if s == "none" { None } else { Some(s.parse().unwrap_or_else(|_| die("--native-try"))) };
    }
    if let Some(s) = a.take("--admit-try") {
        o.admit_try = if s == "none" { None } else { Some(s.parse().unwrap_or_else(|_| die("--admit-try"))) };
    }
    if a.flag("--no-content-fallback") {
        o.guard_content_fallback = false;
    }
    if a.flag("--no-gsplit") {
        o.gsplit = false;
    }
    // default heavy action: gate on box size and oracle cost (sampled when feasible, else refuse = class B)
    let mut screen = LiftScreen { on_heavy: HeavyAction::Auto, symbolic_first_from_rows: Some(1200), sampled_from_rows: Some(1200), ..LiftScreen::default() };
    if let Some(s) = a.take("--screen") {
        let v: Vec<usize> = s.split(',').map(|x| x.parse().unwrap_or_else(|_| die("--screen MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS"))).collect();
        if v.len() != 4 {
            die("--screen MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS");
        }
        (screen.min_rows, screen.max_points, screen.max_total_degree, screen.max_rhs_terms) = (v[0], v[1], v[2], v[3]);
    }
    if let Some(h) = a.take("--on-heavy") {
        screen.on_heavy = match h.as_str() {
            "refuse" => HeavyAction::Refuse,
            "sampled" => HeavyAction::Sampled,
            "proceed" => HeavyAction::Proceed,
            "auto" => HeavyAction::Auto,
            _ => die("--on-heavy auto|refuse|sampled|proceed"),
        };
    }
    if let Some(v) = a.parse("--sampled-budget") {
        screen.sampled_budget_seconds = v;
    }
    if let Some(v) = a.parse("--max-box") {
        screen.max_box = v;
    }
    if let Some(v) = a.parse("--min-target-block") {
        screen.min_target_block = v;
    }
    if let Some(v) = a.parse("--sampled-threads") {
        screen.sampled_threads = v;
    }
    if let Some(v) = a.take("--seed-schedule") {
        o.seed_schedule = match v.as_str() {
            "spired" => SeedScheduleName::Spired,
            "symfirst" | "symbolic-first" => SeedScheduleName::SymbolicFirst,
            _ => die("--seed-schedule spired|symfirst"),
        };
    }
    if a.flag("--no-symbolic-first-rung") {
        o.symbolic_first_rung = false;
    }
    // per-rule size budget (coefficient monomials over the RHS); default 2,000,000, 'none' = off
    o.max_rule_monomials = match a.take("--max-rule-monomials") {
        Some(v) if v == "none" => None,
        Some(v) => Some(v.parse().unwrap_or_else(|_| die("--max-rule-monomials K|none"))),
        None => Some(2_000_000),
    };
    if a.flag("--no-empty-domain-check") {
        o.empty_domain_check = false;
    }
    if let Some(v) = a.take("--sampled-from-rows") {
        screen.sampled_from_rows = if v == "none" { None } else { Some(v.parse().unwrap_or_else(|_| die("--sampled-from-rows R|none"))) };
    }
    if let Some(v) = a.take("--symfirst-from-rows") {
        screen.symbolic_first_from_rows = if v == "none" { None } else { Some(v.parse().unwrap_or_else(|_| die("--symfirst-from-rows R|none"))) };
    }
    o.screen = if a.flag("--no-screen") { None } else { Some(screen) };
    if let Some(s) = a.take("--retry-source-orders") {
        o.retry_source_orders = if s == "none" { vec![] } else { s.split(',').map(|x| SourceOrder::parse(x).unwrap_or_else(|e| die(&e))).collect() };
    }
    o.cover_tables = a
        .take_all("--cover")
        .into_iter()
        .map(|c| {
            let (p, u) = c.rsplit_once(':').unwrap_or_else(|| die("--cover PATH:UPTO"));
            (p.to_string(), u.parse().unwrap_or_else(|_| die("--cover PATH:UPTO")))
        })
        .collect();
    o.cand_dump = a.flag("--cand-dump");
    if let Some(d) = a.parse("--numeric-depth") {
        o.numeric_depth = d;
    }
    let _ = family;
    o
}

fn probe_options(a: &mut Args) -> ProbeOptions {
    let mut p = ProbeOptions::default();
    if let Some(c) = a.parse("--probe-cap") {
        p.cap_seconds = c;
    }
    if let Some(d) = a.parse("--probe-depth") {
        p.max_depth = d;
    }
    if let Some(f) = a.take("--probe-fallback-depth") {
        p.fallback_depth = if f == "none" { None } else { Some(f.parse().unwrap_or_else(|_| die("--probe-fallback-depth"))) };
    }
    if let Some(k) = a.parse("--probe-parallel") {
        p.parallel = k;
    }
    if let Some(k) = a.parse("--max-phys-pairs") {
        p.max_phys_pairs = k;
    }
    if let Some(k) = a.parse("--probe-portfolio") {
        p.portfolio = k;
    }
    p
}

fn cmd_generate(mut a: Args) -> i32 {
    let family_path = a.need("--family");
    let family = read(&family_path);
    let out = PathBuf::from(a.need("--out"));
    let order = a.take("--order").unwrap_or_else(|| "auto".into());
    let mut opts = node_options(&mut a, &family);
    if order == "auto" {
        let mut auto = AutoOptions { probe: probe_options(&mut a), ..Default::default() };
        if let Some(k) = a.parse("--max-attempts") {
            auto.max_attempts = k;
        }
        if let Some(s) = a.parse("--case-budget") {
            auto.case_budget = s;
        }
        auto.attempt_cap = a.parse("--attempt-cap");
        if let Some(s) = a.take("--start-order") {
            opts.order = order_arg(&s, &family, None);
        }
        a.done();
        let r = generate_auto(&family, &family_path, &out, &opts, &auto, &mut |m| eprintln!("[rr_node {}] {m}", utc())).unwrap_or_else(|e| die(&e));
        println!("{}", serde_json::to_string(&serde_json::json!({
            "complete": r.complete, "order": r.order, "table": r.table, "attempts": r.attempts.len(),
            "descent_rules": r.descent_rules, "descent_not_lower": r.descent_not_lower, "failure": r.failure, "seconds": r.seconds,
            "rules": r.attempts.last().and_then(|x| x.outcome.as_ref()).map(|o| o.rules),
            "residuals": r.attempts.last().and_then(|x| x.outcome.as_ref()).map(|o| o.residuals),
        })).unwrap());
        return i32::from(!r.complete);
    }
    opts.order = order_arg(&order, &family, None);
    opts.case_budget_exit = a.parse("--case-budget");
    a.done();
    let mut events = std::fs::File::create(out.join("EVENTS.jsonl")).ok();
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    if events.is_none() {
        events = std::fs::File::create(out.join("EVENTS.jsonl")).ok();
    }
    let outcome = generate(&family, &family_path, &out, &opts, &mut |e| {
        use std::io::Write as _;
        if let Some(f) = events.as_mut() {
            let _ = writeln!(f, "{}", serde_json::to_string(e).unwrap_or_default());
        }
    })
    .unwrap_or_else(|e| die(&e));
    let text = serde_json::to_string_pretty(&outcome).unwrap();
    let _ = std::fs::write(out.join("OUTCOME.json"), &text);
    if outcome.complete {
        if let Ok((_, rep)) = check_table_file(&outcome.table, Some(opts.order.clone())) {
            eprintln!("[rr_node] descent under {}: {} rules, {} terms, {} not lower", opts.order.name, rep.rules, rep.terms, rep.not_lower.len());
        }
    }
    println!("{}", serde_json::to_string(&serde_json::json!({"complete": outcome.complete, "rules": outcome.rules, "residuals": outcome.residuals, "stop": outcome.stop, "seconds": outcome.seconds, "table": outcome.table})).unwrap());
    i32::from(!outcome.complete)
}

fn cmd_probe(mut a: Args) -> i32 {
    let family_path = a.need("--family");
    let family = read(&family_path);
    let case = case_arg(&a.need("--case"));
    let out = PathBuf::from(a.need("--out"));
    let names = a.take("--variants").unwrap_or_else(|| "all".into());
    let mut popts = ProbeOptions::default();
    if let Some(c) = a.parse("--cap") {
        popts.cap_seconds = c;
    }
    if let Some(k) = a.parse("--parallel") {
        popts.parallel = k;
    }
    if let Some(k) = a.parse("--portfolio") {
        popts.portfolio = k;
    }
    let depth: u32 = a.parse("--depth").unwrap_or(1);
    let base = node_options(&mut a, &family);
    a.done();
    let all = variants(&family, &case.fixed, &case.equations, 10);
    let chosen: Vec<NodeOrder> = if names == "all" { all } else { names.split(',').map(|n| order_arg(n, &family, Some(&case))).collect() };
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    let results = probe(&family, &family_path, &case, &chosen, &base, &popts, depth, &out);
    let best = rustred::node::auto::best(&results).map(|p| p.order.name.clone());
    let doc = serde_json::json!({"case": case, "depth": depth, "best": best, "probes": results});
    let _ = std::fs::write(out.join("PROBES.json"), serde_json::to_string_pretty(&doc).unwrap());
    for p in &results {
        println!("{}\t{}\t{:?}\t{:.1}s", p.order.name, p.status, p.score.as_ref().map(|s| (s.max_degree, s.rhs_terms, s.trace_rows, s.lift_seconds)), p.seconds);
    }
    println!("best\t{}", best.as_deref().unwrap_or("none"));
    i32::from(best.is_none())
}

fn cmd_variants(mut a: Args) -> i32 {
    let family = read(&a.need("--family"));
    let case = case_arg(&a.need("--case"));
    a.done();
    for o in variants(&family, &case.fixed, &case.equations, 10) {
        println!("{}", o.to_json());
    }
    0
}

fn cmd_descent(mut a: Args) -> i32 {
    let table = a.need("--table");
    let order = a.take("--order");
    a.done();
    let order = order.map(|o| {
        let header: serde_json::Value = read(&table).lines().next().and_then(|l| serde_json::from_str(l).ok()).unwrap_or_default();
        order_arg(&o, header["family_source"].as_str().unwrap_or_default(), None)
    });
    match check_table_file(&table, order) {
        Ok((o, rep)) => {
            println!("{}", serde_json::to_string(&serde_json::json!({"order": o, "rules": rep.rules, "terms": rep.terms, "not_lower": rep.not_lower.len(), "failing_rules": rep.failing_rules(), "first": rep.not_lower.first().map(|x| format!("{:?}", x))})).unwrap());
            i32::from(!rep.passed())
        }
        Err(e) => die(&e),
    }
}

fn cmd_replay(mut a: Args) -> i32 {
    let mut dims = Vec::new();
    if let Some(at) = a.items.iter().position(|x| x == "--dimensions") {
        a.items.remove(at);
        while at < a.items.len() && !a.items[at].starts_with("--") {
            dims.push(a.items.remove(at));
        }
    }
    if dims.is_empty() {
        dims = replay::DIMS.iter().map(|s| s.to_string()).collect();
    }
    let args = ReplayArgs {
        family: a.take("--family").map(PathBuf::from),
        table: PathBuf::from(a.need("--table")),
        cert: a.take("--cert").map(PathBuf::from),
        cand_dump_dir: a.take("--cand-dump-dir").map(PathBuf::from),
        bindings: a.take("--bindings").map(PathBuf::from),
        rules: ReplayArgs::parse_rules(&a.take("--rules").unwrap_or_else(|| "all".into())).unwrap_or_else(|e| die(&e)),
        chart: a.take("--chart"),
        out: PathBuf::from(a.need("--out")),
        dimensions: dims,
        sample_attempts: a.parse("--sample-attempts").unwrap_or(128),
    };
    a.done();
    let (status, result) = replay::run(&args).unwrap_or_else(|e| die(&e));
    println!("{}", serde_json::to_string(&serde_json::json!({"status": status, "counts": result["counts"], "reason": result.get("reason"), "out": args.out})).unwrap());
    match status.as_str() {
        "PASS_RS2_RS3_SAMPLED" => 0,
        "FAIL" => 1,
        _ => 2,
    }
}

fn utc() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{:02}:{:02}:{:02}Z", s / 3600 % 24, s / 60 % 60, s % 60)
}

fn main() {
    let mut items: Vec<String> = std::env::args().skip(1).collect();
    if items.is_empty() {
        die("usage: rr_node generate|probe|variants|descent|replay ... (see the source header)");
    }
    let cmd = items.remove(0);
    let a = Args { items };
    let code = match cmd.as_str() {
        "generate" => cmd_generate(a),
        "probe" => cmd_probe(a),
        "variants" => cmd_variants(a),
        "descent" => cmd_descent(a),
        "replay" => cmd_replay(a),
        other => die(&format!("unknown command {other}")),
    };
    let _ = Path::new(".");
    std::process::exit(code);
}
