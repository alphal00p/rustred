//! `rr_node`: node-level RustRed commands consolidated from the 5L campaign (rr-consolidate).
//!
//! ```text
//! rr_node generate --family F.sym --out DIR [--order auto|id|NAME|JSON] [--chart MASK] [--initial 'SPEC [EQS]']...
//!                  [--backend targetonlyfactorized|factorized|sparse|targetonly|sampled|sampled-strict] [--max-depth K] [--max-cases K]
//!                  [--native-try S] [--no-content-fallback] [--admit-try S] [--no-screen] [--screen MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS]
//!                  [--on-heavy auto|refuse|sampled|proceed] [--sampled-from-rows R] [--retry-source-orders reverse,rot:7]
//!                  [--symfirst-from-rows R|none] [--sampled-budget S] [--max-box K] [--min-target-block K] [--sampled-threads K]
//!                  [--case-seed-priority symbolic-isp-first|affine-coupled-first] [--affine-rung]
//!                  [--max-rule-monomials K|none] [--fallback-rule-monomials K|none] [--fallback-after K] [--no-refused-bank]
//!                  [--adopt-bank F.refused.jsonl]... [--union-cover-depth K] [--registry DIR:PIECE --unsafe-shared-registry]
//!                  [--seed-schedule spired|symfirst] [--case-seed-schedule JSON] [--no-symbolic-first-rung] [--no-empty-domain-check]
//!                  [--case-budget S] [--max-attempts K] [--attempt-cap S] [--probe-cap S] [--probe-depth K]
//!                  [--probe-fallback-depth K|none] [--probe-parallel P] [--probe-portfolio K] [--probe-follow-on K]
//!                  [--cover PATH:UPTO]... [--cand-dump] [--strict-numeric-search] [--known-terminals FILE] [--numeric-depth K] [--numeric-depth-max K]
//!                  [--no-witness-guards]   (A/B only: drop the source-weight-pole guards, table guard_version 1)
//! rr_node probe    --family F.sym --case 'SPEC [EQS]' --out DIR [--variants all|NAME,..] [--cap S] [--depth K] [--parallel P] [--portfolio K]
//! rr_node variants --family F.sym --case 'SPEC [EQS]'
//! rr_node predict  --family F.sym (--table F.table.jsonl | --cases CASES.jsonl) --out PREDICT.json [--base-order JSON|NAME]
//!                  [--sample-easy K] [--rounds K]   (one coordinate priority for the node; see node::predict)
//! rr_node oracle   --family F.sym --cases CASES.jsonl --orders ORDERS.jsonl   (depth-1 rule per case x order, TSV)
//! rr_node descent  --table F.table.jsonl [--order JSON|NAME]
//! rr_node witness  --table F.table.jsonl --rules 0-9,12 --out DIR [--family F.sym] [--case-budget S]
//!                  (guard-fix: re-lift each rule's case alone under the table's own node options with the witness
//!                  guards ON; per rule DIR/WITNESS.jsonl gets same_rule, witness_loci, hidden_branches = new exception
//!                  branches minus the table's; DIR/LOCI.jsonl feeds `replay --loci`)
//! rr_node replay   --table F.table.jsonl --out DIR [--family F.sym] [--cert F.cert.jsonl] [--cand-dump-dir D]
//!                  [--bindings B.json] [--rules all|0-9,12] [--chart MASK] [--dimensions 17/7 19/7 23/7] [--sample-attempts 128]
//!                  [--extra-points K] [--loci LOCI.jsonl] [--box K [--box-max M]]   (guard-fix: more points per rule; one point ON each
//!                  listed locus; every valid case point with free coordinates in the box |n| <= K, at most M = 2000)
//! ```
//! Exit status: generate 0 complete / 1 not complete / 2 usage; replay 0 PASS / 1 FAIL / 2 UNKNOWN; descent 0 pass / 1 fail.

#![allow(
    clippy::field_reassign_with_default,
    clippy::collapsible_if,
    reason = "flag-by-flag option parsing"
)]

use std::path::{Path, PathBuf};

use rustred::node::auto::{AutoOptions, ProbeOptions, generate_auto, probe};
use rustred::node::descent::check_table_file;
use rustred::node::driver::{
    CaseSeedPriority, CaseSpec, KnownTerminalsFile, NodeOptions, SeedScheduleName, generate,
    parse_case_seed_schedule,
};
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
        self.take(flag)
            .unwrap_or_else(|| die(&format!("missing {flag}")))
    }
    fn parse<T: std::str::FromStr>(&mut self, flag: &str) -> Option<T> {
        self.take(flag).map(|v| {
            v.parse()
                .unwrap_or_else(|_| die(&format!("{flag}: bad value {v}")))
        })
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
    let all_free = CaseSpec {
        fixed: vec![None; n],
        equations: vec![],
    };
    let c = case.unwrap_or(&all_free);
    variants(family, &c.fixed, &c.equations, 1000)
        .into_iter()
        .find(|o| o.name == text)
        .unwrap_or_else(|| {
            die(&format!(
                "unknown order {text} (id, auto, a JSON order, or a variant name)"
            ))
        })
}

fn node_options(a: &mut Args, family: &str) -> NodeOptions {
    let mut o = NodeOptions::default();
    o.chart = a.take("--chart");
    o.initial = a
        .take_all("--initial")
        .iter()
        .map(|s| case_arg(s))
        .collect();
    if let Some(b) = a.take("--backend") {
        o.exact_backend = b;
    }
    o.max_depth = a.parse("--max-depth");
    o.max_cases = a.parse("--max-cases");
    if let Some(s) = a.take("--native-try") {
        o.native_try = if s == "none" {
            None
        } else {
            Some(s.parse().unwrap_or_else(|_| die("--native-try")))
        };
    }
    if let Some(s) = a.take("--admit-try") {
        o.admit_try = if s == "none" {
            None
        } else {
            Some(s.parse().unwrap_or_else(|_| die("--admit-try")))
        };
    }
    if a.flag("--no-content-fallback") {
        o.guard_content_fallback = false;
    }
    if a.flag("--no-gsplit") {
        o.gsplit = false;
    }
    // default heavy action: gate on box size and oracle cost (sampled when feasible, else refuse = class B)
    let mut screen = LiftScreen {
        on_heavy: HeavyAction::Auto,
        symbolic_first_from_rows: Some(1200),
        sampled_from_rows: Some(1200),
        ..LiftScreen::default()
    };
    if let Some(s) = a.take("--screen") {
        let v: Vec<usize> = s
            .split(',')
            .map(|x| {
                x.parse()
                    .unwrap_or_else(|_| die("--screen MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS"))
            })
            .collect();
        if v.len() != 4 {
            die("--screen MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS");
        }
        (
            screen.min_rows,
            screen.max_points,
            screen.max_total_degree,
            screen.max_rhs_terms,
        ) = (v[0], v[1], v[2], v[3]);
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
    if let Some(v) = a.take("--case-seed-schedule") {
        o.case_seed_schedule = Some(
            parse_case_seed_schedule(&v, rustred::node::order::family_props(family).len())
                .unwrap_or_else(|e| die(&format!("--case-seed-schedule: {e}"))),
        );
    }
    if let Some(v) = a.take("--case-seed-priority") {
        o.case_seed_priority = Some(match v.as_str() {
            "symbolic-isp-first" => CaseSeedPriority::SymbolicIspFirst,
            "affine-coupled-first" => CaseSeedPriority::AffineCoupledFirst,
            _ => die("--case-seed-priority symbolic-isp-first|affine-coupled-first"),
        });
    }
    if let Some(priority) = o.case_seed_priority {
        priority
            .validate_template(
                o.case_seed_schedule.as_ref(),
                rustred::node::order::family_props(family).len(),
            )
            .unwrap_or_else(|e| die(&format!("--case-seed-priority: {e}")));
    }
    if a.flag("--no-symbolic-first-rung") {
        o.symbolic_first_rung = false;
    }
    // per-rule size RULING (coordinator 2026-11-11T09:53Z): <= 1,000,000 coefficient monomials accepted; up to
    // 10,000,000 kept as a flagged fallback adopted after <= 3 other variants; above never adopted ('none' = off)
    o.max_rule_monomials = match a.take("--max-rule-monomials") {
        Some(v) if v == "none" => None,
        Some(v) => Some(
            v.parse()
                .unwrap_or_else(|_| die("--max-rule-monomials K|none")),
        ),
        None => Some(1_000_000),
    };
    o.fallback_rule_monomials = match a.take("--fallback-rule-monomials") {
        Some(v) if v == "none" => None,
        Some(v) => Some(
            v.parse()
                .unwrap_or_else(|_| die("--fallback-rule-monomials K|none")),
        ),
        None => Some(10_000_000),
    };
    if let Some(k) = a.parse("--fallback-after") {
        o.fallback_after = k;
    }
    if a.flag("--no-refused-bank") {
        o.refused_bank = false;
    }
    o.adopt_banks = a.take_all("--adopt-bank");
    if let Some(d) = a.parse("--union-cover-depth") {
        o.union_cover_depth = d;
    }
    if let Some(r) = a.take("--registry") {
        let (dir, piece) = r
            .rsplit_once(':')
            .unwrap_or_else(|| die("--registry DIR:PIECE"));
        o.registry = Some((dir.to_string(), piece.to_string()));
    }
    // M5 batch 3: the shared case registry is experimental and refused without this flag.
    if a.flag("--unsafe-shared-registry") {
        o.unsafe_shared_registry = true;
    }
    if a.flag("--no-empty-domain-check") {
        o.empty_domain_check = false;
    }
    if let Some(v) = a.take("--sampled-from-rows") {
        screen.sampled_from_rows = if v == "none" {
            None
        } else {
            Some(
                v.parse()
                    .unwrap_or_else(|_| die("--sampled-from-rows R|none")),
            )
        };
    }
    if let Some(v) = a.take("--symfirst-from-rows") {
        screen.symbolic_first_from_rows = if v == "none" {
            None
        } else {
            Some(
                v.parse()
                    .unwrap_or_else(|_| die("--symfirst-from-rows R|none")),
            )
        };
    }
    o.screen = if a.flag("--no-screen") {
        None
    } else {
        Some(screen)
    };
    if let Some(s) = a.take("--retry-source-orders") {
        o.retry_source_orders = if s == "none" {
            vec![]
        } else {
            s.split(',')
                .map(|x| SourceOrder::parse(x).unwrap_or_else(|e| die(&e)))
                .collect()
        };
    }
    o.cover_tables = a
        .take_all("--cover")
        .into_iter()
        .map(|c| {
            let (p, u) = c
                .rsplit_once(':')
                .unwrap_or_else(|| die("--cover PATH:UPTO"));
            (
                p.to_string(),
                u.parse().unwrap_or_else(|_| die("--cover PATH:UPTO")),
            )
        })
        .collect();
    o.cand_dump = a.flag("--cand-dump");
    o.strict_numeric_search = a.flag("--strict-numeric-search");
    if let Some(path) = a.take("--known-terminals") {
        o.known_terminals = Some(KnownTerminalsFile::read(path).unwrap_or_else(|e| die(&e)));
    }
    // guard-fix: witness-pole guards are ON by default; this restores the old RHS-only guards (A/B only)
    o.no_witness_guards = a.flag("--no-witness-guards");
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
        p.fallback_depth = if f == "none" {
            None
        } else {
            Some(f.parse().unwrap_or_else(|_| die("--probe-fallback-depth")))
        };
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
    if let Some(k) = a.parse("--probe-follow-on") {
        p.follow_on = k;
    }
    if a.flag("--affine-rung") {
        p.affine_rung = true;
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
        let mut auto = AutoOptions {
            probe: probe_options(&mut a),
            ..Default::default()
        };
        if let Some(k) = a.parse("--max-attempts") {
            auto.max_attempts = k;
        }
        if let Some(s) = a.parse("--case-budget") {
            auto.case_budget = s;
        }
        auto.attempt_cap = a.parse("--attempt-cap");
        // order-study: R1 (boundary-first) candidate + depth-1-oracle ranking of the probe candidates (default off)
        auto.oracle_rank = a.flag("--oracle-rank");
        auto.oracle_top = a.parse("--oracle-top");
        if let Some(s) = a.take("--start-order") {
            opts.order = order_arg(&s, &family, None);
        }
        a.done();
        let r = generate_auto(&family, &family_path, &out, &opts, &auto, &mut |m| {
            eprintln!("[rr_node {}] {m}", utc())
        })
        .unwrap_or_else(|e| die(&e));
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
            eprintln!(
                "[rr_node] descent under {}: {} rules, {} terms, {} not lower",
                opts.order.name,
                rep.rules,
                rep.terms,
                rep.not_lower.len()
            );
        }
    }
    let mut summary = serde_json::json!({"complete": outcome.complete, "rules": outcome.rules, "residuals": outcome.residuals, "stop": outcome.stop, "seconds": outcome.seconds, "table": outcome.table});
    if let Some(ledger) = &outcome.terminal_ledger {
        summary["terminal_ledger"] = serde_json::to_value(ledger).expect("terminal provenance");
    }
    println!("{}", serde_json::to_string(&summary).unwrap());
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
    let chosen: Vec<NodeOrder> = if names == "all" {
        all
    } else {
        names
            .split(',')
            .map(|n| order_arg(n, &family, Some(&case)))
            .collect()
    };
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    let results = probe(
        &family,
        &family_path,
        &case,
        &chosen,
        &base,
        &popts,
        depth,
        &out,
    );
    let best = rustred::node::auto::best(&results).map(|p| p.order.name.clone());
    let doc = serde_json::json!({"case": case, "depth": depth, "best": best, "probes": results});
    let _ = std::fs::write(
        out.join("PROBES.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    );
    for p in &results {
        println!(
            "{}\t{}\t{:?}\t{:.1}s",
            p.order.name,
            p.status,
            p.score
                .as_ref()
                .map(|s| (s.max_degree, s.rhs_terms, s.trace_rows, s.lift_seconds)),
            p.seconds
        );
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
        let header: serde_json::Value = read(&table)
            .lines()
            .next()
            .and_then(|l| serde_json::from_str(l).ok())
            .unwrap_or_default();
        order_arg(
            &o,
            header["family_source"].as_str().unwrap_or_default(),
            None,
        )
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
        rules: ReplayArgs::parse_rules(&a.take("--rules").unwrap_or_else(|| "all".into()))
            .unwrap_or_else(|e| die(&e)),
        chart: a.take("--chart"),
        out: PathBuf::from(a.need("--out")),
        dimensions: dims,
        sample_attempts: a.parse("--sample-attempts").unwrap_or(128),
        extra_points: a.parse("--extra-points").unwrap_or(0),
        loci: a.take("--loci").map(PathBuf::from),
        box_size: a.parse("--box").unwrap_or(0),
        box_max: a.parse("--box-max").unwrap_or(2000),
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
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{:02}:{:02}:{:02}Z", s / 3600 % 24, s / 60 % 60, s % 60)
}

fn read_cases(path: &str) -> Vec<rustred::node::predict::WeightedCase> {
    read(path)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| die(&format!("{path}: {e}"))))
        .collect()
}

fn cmd_predict(mut a: Args) -> i32 {
    use rustred::node::predict::{OracleFamily, PredictOptions, predict, table_cases};
    let family_path = a.need("--family");
    let family = read(&family_path);
    let out = a.need("--out");
    let table = a.take("--table");
    let cases_path = a.take("--cases");
    let base_order = a.take("--base-order");
    let mut opts = PredictOptions::default();
    if let Some(k) = a.parse("--sample-easy") {
        opts.sample_easy = k;
    }
    if let Some(k) = a.parse("--rounds") {
        opts.rounds = k;
    }
    opts.size_stage = a.flag("--size-stage");
    // --iterate K --iter-out DIR [--family-path P] [generate flags]: iterated prediction with bounded runs
    let iterate: Option<usize> = a.parse("--iterate");
    let iter_setup = iterate.map(|k| {
        let mut io = rustred::node::auto::IterateOptions {
            iterations: k,
            ..Default::default()
        };
        if let Some(v) = a.parse("--run-cases") {
            io.run_cases = v;
        }
        if let Some(v) = a.parse("--run-cap") {
            io.run_cap = v;
        }
        if let Some(v) = a.parse("--run-case-budget") {
            io.case_budget = v;
        }
        if let Some(v) = a.parse("--keep-easy") {
            io.keep_easy = v;
        }
        let dir = PathBuf::from(a.need("--iter-out"));
        let nopts = node_options(&mut a, &family);
        (io, dir, nopts)
    });
    if a.flag("--affine-depth2") {
        opts.affine_depth2 = true;
    }
    a.done();
    let fam = OracleFamily::parse(&family).unwrap_or_else(|e| die(&e));
    let cases = match (table, cases_path) {
        (Some(t), None) => table_cases(&t).unwrap_or_else(|e| die(&e)).1,
        (None, Some(c)) => read_cases(&c),
        _ => die("give exactly one of --table, --cases"),
    };
    let cases: Vec<_> = cases
        .into_iter()
        .filter(|c| c.fixed.len() == fam.n)
        .collect();
    let base: Vec<usize> = match base_order {
        None => (0..fam.n).collect(),
        Some(s) => {
            let o = order_arg(&s, &family, None);
            if o.slot_permutation.is_some() {
                die(
                    "--base-order: a presentation change; pass the presented family and its comparator order instead",
                );
            }
            o.coordinate_priority
                .unwrap_or_else(|| (0..fam.n).collect())
        }
    };
    if let Some((io, dir, nopts)) = iter_setup {
        let r = rustred::node::auto::predict_iterate(
            &family,
            &family_path,
            &dir,
            &nopts,
            &cases,
            &base,
            &opts,
            &io,
            &mut |m| eprintln!("[rr_node predict-iterate {}] {m}", utc()),
        )
        .unwrap_or_else(|e| die(&e));
        let _ = std::fs::write(&out, serde_json::to_string_pretty(&r).unwrap());
        println!("{}", r.order.to_json());
        eprintln!(
            "[rr_node predict-iterate] converged {} after {} rounds, {:.0} s",
            r.converged,
            r.rounds.len(),
            r.seconds
        );
        return i32::from(!r.converged);
    }
    let p = predict(&fam, &cases, &base, &opts, &mut |m| {
        eprintln!("[rr_node predict {}] {m}", utc())
    })
    .unwrap_or_else(|e| die(&e));
    let _ = std::fs::write(&out, serde_json::to_string_pretty(&p).unwrap());
    println!("{}", p.order.to_json());
    eprintln!(
        "[rr_node predict] affine cases without a small depth-1/depth-2 relation: base {} -> predicted {}",
        p.affine_not_small[0], p.affine_not_small[1]
    );
    eprintln!(
        "[rr_node predict] cases {} hard {} | no depth-1: base {} -> predicted {} (hard {} -> {}, big {} -> {})",
        p.cases,
        p.hard,
        p.no_depth1[0],
        p.no_depth1[1],
        p.no_depth1_hard[0],
        p.no_depth1_hard[1],
        p.no_depth1_big[0],
        p.no_depth1_big[1]
    );
    0
}

fn cmd_oracle(mut a: Args) -> i32 {
    use rustred::node::predict::{OracleCase, OracleFamily, relation_profile};
    let family = read(&a.need("--family"));
    let cases = read_cases(&a.need("--cases"));
    let orders_path = a.need("--orders");
    // --profile: JSON lines with the selected relation's size profile (degrees by Thiele mod p);
    // --per-symbol: also per-symbol degrees; --all-pivots: profile every eligible pivot, not only the oracle's choice
    let profile = a.flag("--profile");
    let per_symbol = a.flag("--per-symbol");
    let all_pivots = a.flag("--all-pivots");
    let max_points: usize = a.parse("--max-points").unwrap_or(80);
    a.done();
    let fam = OracleFamily::parse(&family).unwrap_or_else(|e| die(&e));
    let orders: Vec<NodeOrder> = read(&orders_path)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| order_arg(l, &family, None))
        .collect();
    for (i, c) in cases.iter().enumerate() {
        let oc =
            OracleCase::new(&fam, &c.fixed, &c.equations, 1 + i as u64).unwrap_or_else(|e| die(&e));
        for o in &orders {
            if o.slot_permutation.is_some() && o.coordinate_priority.is_some() {
                die("oracle: an order with both a presentation and a priority");
            }
            let prio = o
                .slot_permutation
                .clone()
                .or_else(|| o.coordinate_priority.clone())
                .unwrap_or_else(|| (0..fam.n).collect());
            if !profile {
                let d = oc.depth1(&prio);
                println!(
                    "{i}\t{}\t{}\t{}\t{}",
                    o.name,
                    u8::from(d.rule),
                    d.trace,
                    d.rhs
                );
                continue;
            }
            let pivots: Vec<Option<Vec<i32>>> = if all_pivots {
                oc.eligible_pivots(&prio)
                    .into_iter()
                    .map(|p| Some(p.offset))
                    .collect()
            } else {
                vec![None]
            };
            for (k, pv) in pivots.iter().enumerate() {
                let r = relation_profile(
                    &fam,
                    &c.fixed,
                    &c.equations,
                    &prio,
                    pv.as_deref(),
                    per_symbol,
                    max_points,
                    1 + i as u64,
                )
                .unwrap_or_else(|e| die(&e));
                println!(
                    "{}",
                    serde_json::json!({"case": i, "order": o.name, "pivot_index": k, "profile": r})
                );
            }
        }
    }
    0
}

/// `rr_node affine --family F --case 'SPEC EQS' [--order ORD]...`: depth-1 relation at the case point and the
/// displaced-ball depth-2 relations ([`rustred::node::affine`]) under each order and under the case's own R1 order.
fn cmd_affine(mut a: Args) -> i32 {
    use rustred::node::affine::AffinePrepared;
    use rustred::node::predict::{OracleFamily, WeightedCase, bfi_case};
    let family = read(&a.need("--family"));
    let case = case_arg(&a.need("--case"));
    let order_texts = a.take_all("--order");
    a.done();
    let fam = OracleFamily::parse(&family).unwrap_or_else(|e| die(&e));
    let mut orders: Vec<(String, Vec<usize>)> = order_texts
        .iter()
        .map(|t| {
            let o = order_arg(t, &family, Some(&case));
            let prio = o
                .slot_permutation
                .clone()
                .or_else(|| o.coordinate_priority.clone())
                .unwrap_or_else(|| (0..fam.n).collect());
            (o.name, prio)
        })
        .collect();
    let wc = WeightedCase {
        fixed: case.fixed.clone(),
        equations: case.equations.clone(),
        bytes: 0,
    };
    orders.push((
        "r1_case".into(),
        bfi_case(&wc, &fam.phys, &(0..fam.n).collect::<Vec<_>>()),
    ));
    let prep =
        AffinePrepared::new(&fam, &case.fixed, &case.equations, 1).unwrap_or_else(|e| die(&e));
    for (name, prio) in &orders {
        let d = prep.diagnose(prio);
        println!(
            "{}",
            serde_json::json!({"order": name, "priority": prio, "small": d.small(), "diagnosis": d})
        );
    }
    0
}

/// guard-fix: hidden witness poles of existing table rules. Each selected rule's case is lifted again ALONE (its
/// fixed values + equations as the only initial case, max one case, no covers/registry/banks/dumps) under the node
/// options recorded in the table header, with the witness guards on; when the new rule equals the table rule (same
/// target, RHS and seeds) its witness poles are the table rule's hidden poles.
fn cmd_witness(mut a: Args) -> i32 {
    let table = a.need("--table");
    let out = PathBuf::from(a.need("--out"));
    let rules = ReplayArgs::parse_rules(&a.need("--rules")).unwrap_or_else(|e| die(&e));
    let family_override = a.take("--family");
    let budget: Option<u64> = a.parse("--case-budget");
    a.done();
    let text = read(&table);
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: serde_json::Value = serde_json::from_str(lines.next().unwrap_or_default())
        .unwrap_or_else(|e| die(&format!("{table}: header: {e}")));
    let family_path = family_override
        .or_else(|| header["family_file"].as_str().map(str::to_string))
        .unwrap_or_else(|| die("table header has no family_file; pass --family"));
    let family = read(&family_path);
    let base: NodeOptions = serde_json::from_value(header["node_options"].clone())
        .unwrap_or_else(|e| die(&format!("{table}: node_options: {e}")));
    if base
        .order
        .slot_permutation
        .as_ref()
        .is_some_and(|p| p.iter().enumerate().any(|(i, &x)| i != x))
    {
        die("witness: tables with a (non-identity) slot permutation are not supported (case records are in presented slots)");
    }
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    let mut report = String::new();
    let mut loci = String::new();
    for line in lines {
        let r: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if r["type"] != "rule" {
            continue;
        }
        let index = r["index"].as_u64().unwrap_or(u64::MAX) as usize;
        if rules.as_ref().is_some_and(|w| !w.contains(&index)) {
            continue;
        }
        let fixed: Vec<Option<i64>> = r["fixed"]
            .as_array()
            .map(|v| v.iter().map(|x| x.as_i64()).collect())
            .unwrap_or_default();
        let equations: Vec<String> = r["equations"]
            .as_array()
            .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let mut opts = base.clone();
        opts.initial = vec![CaseSpec { fixed, equations }];
        opts.max_cases = Some(1);
        opts.cover_tables = Vec::new();
        opts.cand_dump = false;
        opts.registry = None;
        opts.unsafe_shared_registry = false;
        opts.adopt_banks = Vec::new();
        opts.refused_bank = false;
        opts.no_witness_guards = false;
        if budget.is_some() {
            opts.case_budget_exit = budget;
        }
        let dir = out.join(format!("r{index}"));
        let _ = std::fs::create_dir_all(&dir);
        let outcome = generate(&family, &family_path, &dir, &opts, &mut |_| {});
        let new_rule: Option<serde_json::Value> = outcome.as_ref().ok().and_then(|o| {
            std::fs::read_to_string(&o.table).ok().and_then(|t| {
                t.lines()
                    .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                    .find(|v| v["type"] == "rule")
            })
        });
        let same = new_rule.as_ref().is_some_and(|n| {
            n["target"] == r["target"] && n["rhs"] == r["rhs"] && n["seeds"] == r["seeds"]
        });
        let old_exc = r["exceptions"].as_array().cloned().unwrap_or_default();
        let new_exc = new_rule
            .as_ref()
            .and_then(|n| n["exceptions"].as_array().cloned())
            .unwrap_or_default();
        let hidden: Vec<serde_json::Value> = new_exc
            .iter()
            .filter(|b| !old_exc.contains(b))
            .cloned()
            .collect();
        let witness_loci = new_rule
            .as_ref()
            .map(|n| n["witness_loci"].clone())
            .unwrap_or(serde_json::Value::Null);
        let rec = serde_json::json!({
            "chart": r["chart"], "index": index, "same_rule": same,
            "error": outcome.as_ref().err(),
            "witness_poles": new_rule.as_ref().map(|n| n["witness_poles"].clone()),
            "witness_loci": witness_loci, "hidden_branches": hidden,
            "exceptions_table": old_exc, "exceptions_witnessed": new_exc,
        });
        eprintln!("{}", serde_json::to_string(&rec).unwrap());
        report.push_str(&serde_json::to_string(&rec).unwrap());
        report.push('\n');
        if same {
            // single-polynomial hidden branches are loci the replay can sample on
            let list: Vec<&str> = hidden
                .iter()
                .filter_map(|b| b.as_array().filter(|b| b.len() == 1).and_then(|b| b[0].as_str()))
                .collect();
            if !list.is_empty() {
                loci.push_str(
                    &serde_json::to_string(
                        &serde_json::json!({"chart": r["chart"], "index": index, "loci": list}),
                    )
                    .unwrap(),
                );
                loci.push('\n');
            }
        }
    }
    std::fs::write(out.join("WITNESS.jsonl"), report).unwrap_or_else(|e| die(&e.to_string()));
    std::fs::write(out.join("LOCI.jsonl"), loci).unwrap_or_else(|e| die(&e.to_string()));
    0
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
        "predict" => cmd_predict(a),
        "oracle" => cmd_oracle(a),
        "affine" => cmd_affine(a),
        "witness" => cmd_witness(a),
        other => die(&format!("unknown command {other}")),
    };
    let _ = Path::new(".");
    std::process::exit(code);
}

#[cfg(test)]
mod strict_numeric_tests {
    use super::*;

    #[test]
    fn strict_numeric_cli_is_opt_in() {
        for enabled in [false, true] {
            let mut args = Args {
                items: if enabled {
                    vec!["--strict-numeric-search".into()]
                } else {
                    vec![]
                },
            };
            let options = node_options(&mut args, "");
            args.done();
            assert_eq!(options.strict_numeric_search, enabled);
        }
    }

    #[test]
    fn known_terminals_cli_loads_actual_file_and_is_opt_in() {
        let path =
            std::env::temp_dir().join(format!("rr-node-known-cli-{}.json", std::process::id()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({
            "schema":"rustred.node.known-terminals.v1", "frame":"ORIGINAL",
            "original_family_source_sha256":"a".repeat(64), "node_order":NodeOrder::identity(),
            "chart_presented":"1", "terminals":[{"powers":[1], "sources":[{
                "kind":"lp-count", "reference":"test caller declaration", "sha256":"b".repeat(64)}]}]
        })).unwrap()).unwrap();
        let mut args = Args {
            items: vec!["--known-terminals".into(), path.display().to_string()],
        };
        let options = node_options(&mut args, "");
        args.done();
        assert!(options.known_terminals.is_some());
        assert_eq!(
            serde_json::to_value(options).unwrap()["known_terminals"],
            path.display().to_string()
        );
        let mut args = Args { items: vec![] };
        assert!(node_options(&mut args, "").known_terminals.is_none());
        std::fs::remove_file(path).unwrap();
    }
}
