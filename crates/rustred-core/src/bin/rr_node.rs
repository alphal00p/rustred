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
//!                  [--adopt-bank F.refused.jsonl]... [--union-cover-depth K] [--registry DIR:PIECE [--unsafe-shared-registry]] [--no-registry-lease]
//!                  [--lease-wait S] [--pool DIR] [--open-on-refusal] [--initial-presented CASES.json] [--case-seed-rung JSON] [--slim-rules]
//!                  [--seed-schedule spired|symfirst] [--case-seed-schedule JSON] [--no-symbolic-first-rung] [--no-empty-domain-check] [--pose-probe PROBES.jsonl]
//!                  [--seed-pose-rung [--seed-pose-keep C] [--seed-pose-trigger C|none] [--seed-pose-min-rows R]
//!                   [--seed-pose-fixed-dot K] [--seed-pose-fixed-dot-lines K] [--seed-pose-fixed-dot-seeds M]
//!                   [--seed-pose-fixed-dot-first] [--seed-pose-fixed-dot-no-static-skip]]
//!                  [--case-budget S] [--max-attempts K] [--attempt-cap S] [--probe-cap S] [--probe-depth K]
//!                  [--probe-fallback-depth K|none] [--probe-parallel P] [--probe-portfolio K] [--probe-follow-on K]
//!                  [--cover PATH:UPTO]... [--cand-dump] [--strict-numeric-search] [--numeric-depth K] [--numeric-depth-max K]
//!                  [--stop-before-numeric]   (finish symbolic work; numeric endpoints stay OPEN, never searched)
//!                  [--no-witness-guards]   (A/B only: drop the source-weight-pole guards, table guard_version 1)
//!                  [--known-terminals FILE]   (Astra: declared masters, never searched; with --registry: refused)
//!                  [--symmetry] [--symmetry-rows] [--symmetry-symbolic]
//!                  [--seed-centre-offset 'SLOT:DELTA[,..][;SLOT:DELTA[,..]]..'] [--seed-face-floor K]   (seed-centre: shells
//!                  around target + offset, ';' = several centres, seed set = union; per case "seed_centre_offset":[..] or
//!                  [[..],..] in --initial-presented; drop seeds below target - K on a face line; default off)
//! rr_node symmetry --family F.sym [--chart MASK]   (authenticated automorphism group of the chart, JSON)
//! rr_node solve-recipe --family F.sym --order JSON|NAME --recipe RECIPE.json (--keys KEYS.jsonl | --key K,..) --out DIR
//!                  [--threads T] [--max-trace-rows R] [--key-budget-s S] [--expect DIGESTS.jsonl] [--no-proof]
//!                  (R48 recipe rules, node::recipe); job mode: rr_node job RUN/job.json (kind solve-recipe;
//!                  `rr_node solve-recipe --job RUN/job.json` is an alias of the same dispatcher)
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
//!                  [--dump-weights]   (per cell weights-<d>-<P|P2>.json: rows, pivot rows, weights mod p, columns, zero keys)
//!                  [--fixed-d] [--single-prime]   (default: d random in F_p per cell unless --dimensions; primes P and P2 = 2^61-31)
//! rr_node cover-index --table F.table.jsonl --out F.cover.jsonl [--upto K]
//! rr_node obligations --family F.sym --order NAME|JSON --run DIR --out F.obligations.json   (rebuild from F.queue.jsonl)
//! rr_node join     --run DIR[:OBLIGATIONS[:COVER_INDEX]]... --out DIR [--expect-residuals K] [--region-proof F.json]...
//!                  [--union-depth K] [--no-table]
//! rr_node reduce   --table F.table.jsonl --keys 'K1;K2' --out OUT.json [--masters 'K;..'] [--dims 37/7,-11/5,9/13]
//!                  [--primes N] [--lower] [--symbolic [--points 80] [--first-d 1001]]
//!                  (reduce keys with the table's rules to its masters (+ lower-sector keys): exact rationals per d, or
//!                   N(d)/D(d) with --symbolic; node::mastermap)
//! rr_node domaincheck --table F.table.jsonl --keys FILE|'K1;K2' --out OUT.jsonl [--masters 'K;..'] [--point-valid [--dims 37/7,-11/5,9/13]]
//!                  (EC-15, read-only: per key the engine's own decision without reducing: in_sector, zero sector, master,
//!                   the rule find_rule selects with its rule point, rejection counts, ambiguity markers; schema
//!                   rustred.domaincheck.v1, node::domaincheck; exit 0 ok / 3 internal / 41 input / 51 io)
//! rr_node --capabilities   (JSON list of verbs and the domaincheck options)
//! rr_node job RUN/job.json   (child-job contract rustred.child-job.v1: one generate job described by a job spec;
//!                  exit codes of the contract 0/10/20/21/22/30/40/41/42/50/51/70; node::childjob)
//! rr_node version --json | rr_node --version --json   (program, version, build git commit, contract, features)
//! rr_node generate|witness ... --no-elide-witness-loci   (opt-in: witness loci written in full whatever their size)
//! rr_node mastermap --old OLD.table.jsonl --new NEW.table.jsonl --out MAP.json [--old-masters 'K;..'] [--new-masters 'K;..']
//!                  [--dims ..] [--primes N] [--no-symbolic] [--points 80] [--sym-primes 8] [--symbolic-lower]
//!                  [--laporta DOTS,RANK|none] [--check-keys auto|'K;..'|none]
//!                  (exact old <-> new master map, mutual inverse at 3 d and symbolically, independent Laporta, few-integral
//!                   check; exit 0 accepted / 1 not accepted)
//! rr_node mapcheck --old OLD --new NEW --keys auto|'K;..' --out CHECK.json [--old-masters ..] [--new-masters ..] [--dims ..]
//!                  (few-integral cross-check only: old reduction == new reduction mapped back; exit 0 equal / 1 not)
//! rr_node laporta  --family F.sym --mask BITS --target K --basis 'K;..' --box DOTS,RANK --out OUT.json [--dims ..] [--primes N]
//!                  (t266-10: independent IBP-only Laporta of one target modulo subsectors, node::mastermap::laporta)
//! rr_node symmasters (--table F.table.jsonl | --family F.sym --mask BITS) --out OUT.json [--masters 'K;..'] [--prefer 'K;..']
//!                  [--box DOTS,RANK] [--inner T] [--dims ..] [--primes N] [--isp-free-seeds] [--threads N] [--escalate]
//!                  [--no-tail] [--tail-points 80] [--tail-primes 8]
//!                  (masters modulo the top sector's symmetry group: IBP + exact symmetry rows at every exactly mapped
//!                   seed (--isp-free-seeds: ISP-free seeds only), signed-integer trace certificate dim V^G = |V_G basis|
//!                   in the box, exact map of the masters onto the V_G basis; --escalate reruns at (D+1,R), (D,R+1) as
//!                   saturation evidence; with --table the relation with the full subsector tail from the table's
//!                   reductions red(m) - red(g m), symbolic in d, with (d-4) orders; node::symmasters;
//!                   exit 0 certificate holds / 1 not)
//! ```
//! Exit status: join 0 closed / 1 open / 3 rejected. Exit status: generate 0 complete / 1 not complete / 2 usage; replay 0 PASS / 1 FAIL / 2 UNKNOWN; descent 0 pass / 1 fail.

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

#[path = "rr_node_job/job.rs"]
mod job;

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
    job::on_die(msg);
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
    // t266-tight: seed layer cap D,R[,B] (default off): seeds above the target by more than D dots (D+B when the
    // target is dot-free) or R ranks are not offered (solver::SeedLayerCap); needs a finite --max-depth
    if let Some(v) = a.take("--seed-layer-cap") {
        let p: Vec<i32> = v
            .split(',')
            .map(|x| {
                x.trim()
                    .parse()
                    .unwrap_or_else(|_| die("--seed-layer-cap D,R[,B]"))
            })
            .collect();
        if !(2..=3).contains(&p.len()) {
            die("--seed-layer-cap D,R[,B]");
        }
        let cap = rustred::solver::SeedLayerCap {
            dots_above: p[0],
            rank_above: p[1],
            dot_free_bonus: p.get(2).copied().unwrap_or(1),
        };
        if !cap.install() {
            die("--seed-layer-cap given twice with different values");
        }
        o.seed_layer_cap = Some([cap.dots_above, cap.rank_above, cap.dot_free_bonus]);
        if o.max_depth.is_none() {
            die("--seed-layer-cap needs a finite --max-depth");
        }
        eprintln!("seed_layer_cap {cap:?}");
    }
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
            "dotsfirst" | "dots-first" => SeedScheduleName::DotsFirst,
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
    // seed-pose probe mode: JSON-lines probes, refuse-all screen, results in OUT/F.poses.jsonl (no table)
    o.pose_probe = a.take("--pose-probe");
    // automatic seed-pose rung (default off): measure the default pose, then a small portfolio when it is heavy
    if a.flag("--seed-pose-rung") {
        o.seed_pose_rung = true;
    }
    if let Some(k) = a.parse("--seed-pose-keep") {
        o.seed_pose_keep = k;
    }
    // seed-rung: portfolio only when the default relation's cost exceeds K (measured by the normal search's gated
    // screen); 'none' = measure every case first (the seed-pose agent's rung)
    if let Some(v) = a.take("--seed-pose-trigger") {
        o.seed_pose_trigger = if v == "none" {
            None
        } else {
            Some(
                v.parse()
                    .unwrap_or_else(|_| die("--seed-pose-trigger K|none")),
            )
        };
    }
    if let Some(k) = a.parse("--seed-pose-min-rows") {
        o.seed_pose_min_rows = k;
    }
    // seed-fixed-dot (inside the rung): the free-first shell-3 lead (<= M seeds, default 1500), then K fixed-line
    // poses (opt-in: --seed-pose-fixed-dot-lines K, default 0); --seed-pose-fixed-dot 0 = off (the old rung), K >= 1
    // = on with K line poses; cases whose free lines carry a loop momentum (Y9 cyc >= 1) skip them unless
    // --seed-pose-fixed-dot-no-static-skip; --seed-pose-fixed-dot-first measures them before the portfolio on cases
    // with a dotted fixed line
    if let Some(k) = a.parse("--seed-pose-fixed-dot") {
        o.seed_pose_fixed_dot = Some(k);
    }
    if let Some(k) = a.parse("--seed-pose-fixed-dot-lines") {
        o.seed_pose_fixed_dot_lines = Some(k);
    }
    if a.flag("--seed-pose-fixed-dot-no-static-skip") {
        o.seed_pose_fixed_dot_no_static_skip = true;
    }
    if let Some(m) = a.parse("--seed-pose-fixed-dot-seeds") {
        o.seed_pose_fixed_dot_seeds = Some(m);
    }
    if a.flag("--seed-pose-fixed-dot-first") {
        o.seed_pose_fixed_dot_first = true;
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
    // the M5 batch 3 opt-in of the old registry: accepted, without effect (the registry is proof-bearing now)
    if a.flag("--unsafe-shared-registry") {
        o.unsafe_shared_registry = true;
    }
    if a.flag("--no-registry-lease") {
        o.registry_lease = false;
    }
    if let Some(v) = a.parse("--lease-wait") {
        o.lease_wait = v;
    }
    o.pool = a.take("--pool");
    if a.flag("--slim-rules") {
        o.slim_rules = true;
    }
    if a.flag("--open-on-refusal") {
        o.open_on_refusal = true;
    }
    if let Some(f) = a.take("--initial-presented") {
        let text = read(&f);
        o.initial_presented = serde_json::from_str(&text)
            .unwrap_or_else(|e| die(&format!("--initial-presented {f}: {e}")));
        // seed-centre: optional per-case field "seed_centre_offset" ([ints], PRESENTED slots)
        let raw: Vec<serde_json::Value> = serde_json::from_str(&text)
            .unwrap_or_else(|e| die(&format!("--initial-presented {f}: {e}")));
        for (spec, v) in o.initial_presented.iter().zip(&raw) {
            if let Some(off) = v.get("seed_centre_offset").filter(|x| !x.is_null()) {
                // one centre [ints] or several [[ints], ..] (union of their shells)
                let mut offs: Vec<Vec<i64>> = match serde_json::from_value::<Vec<i64>>(off.clone())
                {
                    Ok(one) => vec![one],
                    Err(_) => serde_json::from_value(off.clone()).unwrap_or_else(|e| {
                        die(&format!("--initial-presented {f}: seed_centre_offset: {e}"))
                    }),
                };
                if offs.is_empty() {
                    die(&format!(
                        "--initial-presented {f}: empty seed_centre_offset"
                    ));
                }
                let first = offs.remove(0);
                o.case_seed_centres
                    .push(rustred::node::driver::CaseSeedCentre {
                        case: spec.clone(),
                        seed_centre_offset: first,
                        extra: offs,
                    });
            }
        }
    }
    // seed-centre: global shell centre offset and face floor (default off)
    if let Some(v) = a.take("--seed-centre-offset") {
        let mut all = rustred::node::driver::parse_seed_centre_offsets(
            &v,
            rustred::node::order::family_props(family).len(),
        )
        .unwrap_or_else(|e| die(&format!("--seed-centre-offset: {e}")));
        o.seed_centre_offset = Some(all.remove(0));
        o.seed_centre_extra = all;
    }
    if let Some(v) = a.take("--seed-face-floor") {
        o.seed_face_floor = Some(v.parse().unwrap_or_else(|_| die("--seed-face-floor <k>")));
    }
    // t266-regions: a region-restricted run on ONE deferred region {chart (PRESENTED), conjunction}; the chart is
    // added to the initial cases and the run owns that region only
    if let Some(f) = a.take("--region") {
        let v: serde_json::Value =
            serde_json::from_str(&read(&f)).unwrap_or_else(|e| die(&format!("--region {f}: {e}")));
        let chart: rustred::node::driver::CaseSpec = serde_json::from_value(v["chart"].clone())
            .unwrap_or_else(|e| die(&format!("--region {f}: chart: {e}")));
        let conj: Vec<String> = serde_json::from_value(v["conjunction"].clone())
            .unwrap_or_else(|e| die(&format!("--region {f}: conjunction: {e}")));
        if conj.is_empty() {
            die(&format!("--region {f}: empty conjunction"));
        }
        o.initial_presented.push(chart);
        o.region = Some(conj);
    }
    if let Some(v) = a.take("--case-seed-rung") {
        o.case_seed_rung = Some(
            parse_case_seed_schedule(&v, rustred::node::order::family_props(family).len())
                .unwrap_or_else(|e| die(&format!("--case-seed-rung: {e}"))),
        );
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
    o.stop_before_numeric = a.flag("--stop-before-numeric");
    if let Some(path) = a.take("--known-terminals") {
        o.known_terminals = Some(KnownTerminalsFile::read(path).unwrap_or_else(|e| die(&e)));
    }
    // guard-fix: witness-pole guards are ON by default; this restores the old RHS-only guards (A/B only)
    o.no_witness_guards = a.flag("--no-witness-guards");
    if let Some(d) = a.parse("--numeric-depth") {
        o.numeric_depth = d;
    }
    if let Some(d) = a.parse("--numeric-depth-max") {
        o.numeric_depth_max = Some(d);
    }
    // sym-gen (default off): symmetry rules / zero records for cases the chart's automorphisms lower
    o.symmetry = a.flag("--symmetry");
    // sym-gen step 2 (default off): exact one-term symmetry rows in the shared numeric bulk solve
    o.symmetry_rows = a.flag("--symmetry-rows");
    // sym-7master (default off): symmetry rows also in the symbolic case search (case-stabilising elements)
    o.symmetry_symbolic = a.flag("--symmetry-symbolic");
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
    if a.flag("--no-elide-witness-loci") {
        rustred::node::childjob::set_legacy_no_elision();
    }
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
    // guard-fix: without --dimensions each cell draws d at random in F_p (3 cells); --fixed-d keeps 17/7 19/7 23/7
    let fixed_d = a.flag("--fixed-d");
    let random_d = dims.is_empty() && !fixed_d;
    if dims.is_empty() {
        dims = replay::DIMS.iter().map(|s| s.to_string()).collect();
    }
    let single_prime = a.flag("--single-prime");
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
        dump_weights: a.flag("--dump-weights"),
        random_d,
        single_prime,
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

/// `rr_node locus-rank --table T --out DIR [--rules all|I,A-Z,CHART:I] [--chart C] [--family F] [--max-flats 64]
/// [--full] [--extra-loci FILE] [--keep-cells] [--no-replay] [--point-tries 4000] [--dimensions D..]
/// [--no-low-layers] [--no-isp-zero-intersections] [--sym-census census_all.jsonl]` (batch 6: low layers and every
/// intersection of the ISP-zero layers by default, symmetry layers from a slot-action census):
/// guard-theory's decisive locus test (seed-coordinate suspect flats, one full-seed-system replay per flat);
/// writes DIR/RESULTS.jsonl + DIR/SUMMARY.json. Exit 0 = every rule PASS_ON_TESTED_FLATS, 1 = some FAIL, 2 = otherwise.
fn cmd_locus_rank(mut a: Args) -> i32 {
    use rustred::node::replay::locus::{LocusArgs, run};
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
    let args = LocusArgs {
        family: a.take("--family").map(PathBuf::from),
        table: PathBuf::from(a.need("--table")),
        out: PathBuf::from(a.need("--out")),
        rules: LocusArgs::parse_rules(&a.take("--rules").unwrap_or_else(|| "all".into()))
            .unwrap_or_else(|e| die(&e)),
        chart: a.take("--chart"),
        dimensions: dims,
        max_flats: a.parse("--max-flats").unwrap_or(64),
        full: a.flag("--full"),
        extra_loci: a.take("--extra-loci").map(PathBuf::from),
        keep_cells: a.flag("--keep-cells"),
        no_replay: a.flag("--no-replay"),
        point_tries: a.parse("--point-tries").unwrap_or(4000),
        // batch 6: the final suspect list (helper Y18) is the default; --no-low-layers / --no-isp-zero-intersections
        // reproduce the locus_rank.py defaults
        low_layers: !a.flag("--no-low-layers"),
        isp_zero_intersections: !a.flag("--no-isp-zero-intersections"),
        sym_census: a.take("--sym-census").map(PathBuf::from),
    };
    a.done();
    let (status, summary) = run(&args).unwrap_or_else(|e| die(&e));
    println!(
        "{}",
        serde_json::json!({"status": status, "verdicts": summary["verdicts"], "rules": summary["rules"], "seconds": summary["seconds"], "out": args.out})
    );
    match status.as_str() {
        "PASS_ON_TESTED_FLATS" => 0,
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

fn cmd_cover_index(mut a: Args) -> i32 {
    let table = a.need("--table");
    let out = a.need("--out");
    let upto = a.parse("--upto").unwrap_or(usize::MAX);
    a.done();
    let k =
        rustred::node::registry::write_cover_index(&table, upto, &out).unwrap_or_else(|e| die(&e));
    println!("{}", serde_json::json!({"cover_index": out, "rules": k}));
    0
}

fn cmd_obligations(mut a: Args) -> i32 {
    let family = read(&a.need("--family"));
    let order = a.need("--order");
    let run = PathBuf::from(a.need("--run"));
    let out = PathBuf::from(a.need("--out"));
    a.done();
    let order = order_arg(&order, &family, None);
    let o = rustred::node::registry::obligations_from_queue(&family, &order, &run)
        .unwrap_or_else(|e| die(&e));
    o.write(&out).unwrap_or_else(|e| die(&e));
    let mut kinds: std::collections::BTreeMap<&str, usize> = Default::default();
    for x in &o.open {
        *kinds.entry(x.kind.as_str()).or_default() += 1;
    }
    println!(
        "{}",
        serde_json::json!({"state": o.state, "open": o.open.len(), "kinds": kinds, "rules": o.rules, "math_key": o.math_key, "stop": o.stop})
    );
    0
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

fn cmd_join(mut a: Args) -> i32 {
    use rustred::node::registry::{JoinOptions, JoinRun, join};
    let runs: Vec<JoinRun> = a
        .take_all("--run")
        .iter()
        .map(|r| {
            let mut it = r.split(':');
            let dir = PathBuf::from(it.next().unwrap_or_default());
            let obligations = it
                .next()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| dir.join("F.obligations.json"));
            let cover_index = it.next().filter(|s| !s.is_empty()).map(PathBuf::from);
            JoinRun {
                dir,
                obligations,
                cover_index,
            }
        })
        .collect();
    let opts = JoinOptions {
        runs,
        out: PathBuf::from(a.need("--out")),
        expect_residuals: a.parse("--expect-residuals"),
        region_proofs: a
            .take_all("--region-proof")
            .into_iter()
            .map(PathBuf::from)
            .collect(),
        union_depth: a.parse("--union-depth").unwrap_or(3),
        write_table: !a.flag("--no-table"),
    };
    a.done();
    let r = join(&opts).unwrap_or_else(|e| die(&e));
    println!(
        "{}",
        serde_json::json!({"verdict": r.verdict, "rules": r.rules, "unresolved": r.unresolved.len(),
        "cycles": r.cycles.len(), "residuals": r.residual_keys.len(), "residuals_ok": r.residuals_ok,
        "discharged": r.discharged, "reasons": r.reasons, "table": r.table, "seconds": r.seconds})
    );
    match r.verdict.as_str() {
        "closed" => 0,
        "open" => 1,
        _ => 3,
    }
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
    if a.flag("--no-elide-witness-loci") {
        rustred::node::childjob::set_legacy_no_elision();
    }
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
        die(
            "witness: tables with a (non-identity) slot permutation are not supported (case records are in presented slots)",
        );
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
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
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
                .filter_map(|b| {
                    b.as_array()
                        .filter(|b| b.len() == 1)
                        .and_then(|b| b[0].as_str())
                })
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

/// `rr_node symmetry --family F.sym [--chart MASK]`: the authenticated automorphism group of the chart (default: the
/// physical lines of the family) as JSON (sym-gen census).
fn cmd_symmetry(mut a: Args) -> i32 {
    let text = read(&a.need("--family"));
    let chart = a.take("--chart");
    a.done();
    let chart: Vec<bool> = match chart {
        Some(m) => m.chars().map(|c| c == '1').collect(),
        None => rustred::node::order::physical_slots(&text),
    };
    let t = std::time::Instant::now();
    match rustred::node::symmetry::node_group(&text, &chart) {
        Ok(g) => {
            let json = g.to_json();
            println!(
                "{{\"ok\":true,\"seconds\":{:.3},\"group\":{json}}}",
                t.elapsed().as_secs_f64()
            );
            0
        }
        Err(e) => {
            println!("{}", serde_json::json!({"ok": false, "error": e}));
            1
        }
    }
}

/// `rr_node solve-recipe` (engine line, R48 recipe rules): one recipe, many keys; see node::recipe and
/// plan/auto-campaign/interfaces/coord-recipe-solve.md. Exit 0 all SOLVED, 10 some not (typed records), 30
/// interrupted (sealed partial), 40 usage, 41 input, 42 refused (job mode), 70 internal.
/// Job mode: `rr_node solve-recipe --job RUN/job.json` (child-job contract subset, kind "solve-recipe").
fn cmd_solve_recipe(mut a: Args) -> i32 {
    use rustred::node::recipe;
    // lane engbase2: ONE job entry. `solve-recipe --job RUN/job.json` is the same dispatcher as `rr_node job
    // RUN/job.json` (contract preflight, signal flags, sealing, spec-digest binding); the verb's own job reader below
    // is no longer reached.
    if let Some(jp) = a.take("--job") {
        a.done();
        return job::cmd_job_at(&jp);
    }
    let fail = |code: i32, m: &str| -> ! {
        eprintln!("rr_node solve-recipe: {m}");
        std::process::exit(code)
    };
    let usage = |m: &str| -> ! { fail(40, m) };
    let input = |m: &str| -> ! { fail(41, m) };
    let digest = |p: &str| -> (u64, String) {
        let b = std::fs::read(p).unwrap_or_else(|e| input(&format!("{p}: {e}")));
        (b.len() as u64, blake3::hash(&b).to_hex().to_string())
    };
    let job_path = a.take("--job");
    // (family, order, recipe, keys, key inline, expect, out, threads, max trace rows, key budget, no proof, job, outcome)
    let (
        family_path,
        order_text,
        recipe_path,
        keys_path,
        key_inline,
        expect_path,
        out,
        threads,
        max_trace_rows,
        key_budget_s,
        no_proof,
        job,
        outcome_path,
    );
    // job mode: the stop file RUN/STOP, the grace and heartbeat periods of the spec, paths relative to RUN
    let (mut stop_file, mut stop_grace_s, mut heartbeat_s, mut path_base, mut events_path) =
        (None, 20.0f64, 10.0f64, None, None);
    if let Some(jp) = job_path {
        a.done();
        let spec_text =
            std::fs::read_to_string(&jp).unwrap_or_else(|e| usage(&format!("{jp}: {e}")));
        let spec: serde_json::Value =
            serde_json::from_str(&spec_text).unwrap_or_else(|e| usage(&format!("job spec: {e}")));
        if spec["schema"] != "rustred.child-job.v1" || spec["kind"] != "solve-recipe" {
            usage("job spec: schema rustred.child-job.v1 and kind solve-recipe required");
        }
        let run_dir = std::path::PathBuf::from(
            spec["run_dir"]
                .as_str()
                .unwrap_or_else(|| usage("job spec: run_dir")),
        );
        let spec_dir = std::fs::canonicalize(
            std::path::Path::new(&jp)
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )
        .unwrap_or_default();
        if !run_dir.is_absolute() || std::fs::canonicalize(&run_dir).ok() != Some(spec_dir) {
            fail(
                42,
                "job spec: run_dir must be the absolute directory of job.json",
            );
        }
        if run_dir.join("OUTCOME.json").exists() {
            fail(42, "job: OUTCOME.json exists (a finished job is final)");
        }
        let resolve = |p: &str| -> String {
            let pp = std::path::Path::new(p);
            if pp.is_absolute() {
                p.to_string()
            } else {
                run_dir.join(pp).display().to_string()
            }
        };
        let mut roles: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        let mut echo = Vec::new();
        for i in spec["inputs"].as_array().cloned().unwrap_or_default() {
            let role = i["role"]
                .as_str()
                .unwrap_or_else(|| usage("job spec: input role"))
                .to_string();
            if !["family", "recipe", "keys", "expect"].contains(&role.as_str()) {
                usage(&format!(
                    "job spec: input role {role} (family, recipe, keys, expect)"
                ));
            }
            let p = resolve(
                i["path"]
                    .as_str()
                    .unwrap_or_else(|| usage("job spec: input path")),
            );
            let (bytes, d) = digest(&p);
            if let Some(want) = i["blake3"].as_str() {
                if want != d {
                    input(&format!(
                        "job: input {role} {p}: blake3 {d} differs from the spec's {want}"
                    ));
                }
            }
            echo.push(serde_json::json!({"role": role, "path": p, "bytes": bytes, "blake3": d, "verified": i["blake3"].is_string()}));
            roles.insert(role, p);
        }
        let mut args = Args {
            items: spec["args"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|v| {
                    v.as_str()
                        .unwrap_or_else(|| usage("job spec: args are strings"))
                        .to_string()
                })
                .collect(),
        };
        order_text = args
            .take("--order")
            .unwrap_or_else(|| usage("job: --order in args"));
        threads = args.parse("--threads").unwrap_or(1);
        max_trace_rows = args.parse("--max-trace-rows");
        key_budget_s = args
            .parse::<f64>("--key-budget-s")
            .or_else(|| spec["budgets"]["wall"]["case_s"].as_f64());
        no_proof = false;
        if !args.items.is_empty() {
            usage(&format!(
                "job: args outside the solve-recipe allow-list (--order, --threads, --max-trace-rows, --key-budget-s): {:?}",
                args.items
            ));
        }
        family_path = roles
            .get("family")
            .cloned()
            .unwrap_or_else(|| usage("job: input role family"));
        recipe_path = roles
            .get("recipe")
            .cloned()
            .unwrap_or_else(|| usage("job: input role recipe"));
        keys_path = Some(
            roles
                .get("keys")
                .cloned()
                .unwrap_or_else(|| usage("job: input role keys")),
        );
        key_inline = None;
        expect_path = roles.get("expect").cloned();
        out = run_dir.join("out").display().to_string();
        outcome_path = Some(run_dir.join("OUTCOME.json"));
        for i in echo.iter_mut() {
            // inputs echoed relative to the run directory (no absolute paths in the job's records)
            let p = i["path"].as_str().unwrap_or("").to_string();
            i["path"] = serde_json::json!(relative_to(&run_dir, &p));
        }
        job = Some(
            serde_json::json!({"schema": "rustred.child-job.outcome.v1", "job_id": spec["job_id"],
            "attempt": spec["attempt"], "kind": "solve-recipe",
            "spec_blake3": blake3::hash(spec_text.as_bytes()).to_hex().to_string(), "inputs": echo,
            "args": spec["args"], "started_utc": recipe::utc_now()}),
        );
        stop_file = Some(run_dir.join("STOP"));
        stop_grace_s = spec["stop_grace_s"].as_f64().unwrap_or(20.0);
        heartbeat_s = spec["heartbeat_s"].as_f64().unwrap_or(10.0);
        path_base = Some(run_dir.clone());
        events_path = Some(run_dir.join("EVENTS.jsonl"));
    } else {
        family_path = a
            .take("--family")
            .unwrap_or_else(|| usage("missing --family"));
        order_text = a
            .take("--order")
            .unwrap_or_else(|| usage("missing --order"));
        recipe_path = a
            .take("--recipe")
            .unwrap_or_else(|| usage("missing --recipe"));
        keys_path = a.take("--keys");
        key_inline = a.take("--key");
        expect_path = a.take("--expect");
        out = a.take("--out").unwrap_or_else(|| usage("missing --out"));
        threads = a.parse("--threads").unwrap_or(1);
        max_trace_rows = a.parse("--max-trace-rows");
        key_budget_s = a.parse("--key-budget-s");
        no_proof = a.flag("--no-proof");
        job = None;
        outcome_path = None;
        a.done();
    }
    let family = std::fs::read_to_string(&family_path)
        .unwrap_or_else(|e| input(&format!("{family_path}: {e}")));
    let recipe_text = std::fs::read_to_string(&recipe_path)
        .unwrap_or_else(|e| input(&format!("{recipe_path}: {e}")));
    let keys = match (&keys_path, key_inline) {
        (Some(p), None) => {
            let text = std::fs::read_to_string(p).unwrap_or_else(|e| input(&format!("{p}: {e}")));
            recipe::parse_keys(&text).unwrap_or_else(|e| input(&e))
        }
        (None, Some(k)) => vec![recipe::KeyInput {
            key: k
                .split(',')
                .map(|x| {
                    x.trim()
                        .parse::<i64>()
                        .unwrap_or_else(|_| usage("--key: integers"))
                })
                .collect(),
            tag: None,
        }],
        _ => usage("give exactly one of --keys FILE / --key K1,K2,.."),
    };
    // --expect FILE: certified digests, JSONL {"key":[..],"relation_digest":".."} (results.jsonl lines qualify)
    let mut expect = std::collections::BTreeMap::new();
    if let Some(p) = &expect_path {
        let text = std::fs::read_to_string(p).unwrap_or_else(|e| input(&format!("{p}: {e}")));
        for (i, l) in text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            let v: serde_json::Value =
                serde_json::from_str(l).unwrap_or_else(|e| input(&format!("{p}:{}: {e}", i + 1)));
            if let (Some(k), Some(d)) = (v.get("key"), v["relation_digest"].as_str()) {
                let k: Vec<i64> = serde_json::from_value(k.clone())
                    .unwrap_or_else(|e| input(&format!("{p}:{}: {e}", i + 1)));
                expect.insert(k, d.to_string());
            }
        }
    }
    let outp = std::path::Path::new(&out);
    if outp.exists()
        && std::fs::read_dir(outp)
            .map(|mut d| d.next().is_some())
            .unwrap_or(true)
    {
        usage(&format!("--out {out} exists and is not empty"));
    }
    if let Err(e) = recipe::Recipe::parse(&recipe_text) {
        usage(&e);
    }
    let order = order_arg(&order_text, &family, None);
    recipe::install_stop_handler();
    let canon = |p: &str| {
        std::fs::canonicalize(p)
            .map(|p| p.display().to_string())
            .unwrap_or(p.to_string())
    };
    let mut inputs = vec![];
    for (role, p) in [
        ("family", Some(family_path.clone())),
        ("recipe", Some(recipe_path.clone())),
        ("keys", keys_path.clone()),
        ("expect", expect_path.clone()),
    ] {
        if let Some(p) = p {
            let (bytes, d) = digest(&p);
            inputs.push(
                serde_json::json!({"role": role, "path": canon(&p), "bytes": bytes, "blake3": d}),
            );
        }
    }
    let opts = recipe::BatchOptions {
        threads,
        max_trace_rows,
        proof: !no_proof,
        family_path: canon(&family_path),
        recipe_path: canon(&recipe_path),
        order_text: order.to_json(),
        binary: recipe::binary_identity(),
        key_budget_s,
        expect,
        inputs: serde_json::json!(inputs),
        job,
        outcome_path: outcome_path.clone(),
        stop: None,
        stop_file: stop_file.clone(),
        stop_grace_s,
        path_base: path_base.clone(),
        events_path: events_path.clone(),
        heartbeat_s,
        on_end: None,
    };
    let single = keys.len() == 1;
    match recipe::run_batch(&family, &order, &recipe_text, &keys, outp, &opts) {
        Ok(code) => {
            if single && code != 0 {
                // stand-alone reproduction: the failure object on stderr as one line
                if let Ok(text) = std::fs::read_to_string(outp.join("results.jsonl")) {
                    if let Some(first) = text.lines().next() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(first) {
                            eprintln!("{}", v["failure"]);
                        }
                    }
                }
            }
            let op = outcome_path.unwrap_or_else(|| outp.join("OUTCOME.json"));
            println!(
                "{}",
                std::fs::read_to_string(op)
                    .map(|t| serde_json::from_str::<serde_json::Value>(&t)
                        .map(|v| serde_json::json!({"exit_code": v["exit_code"], "counts": v["counts"], "out": out}).to_string())
                        .unwrap_or_default())
                    .unwrap_or_default()
            );
            code
        }
        Err(e) => {
            eprintln!("rr_node solve-recipe: {e}");
            if e.starts_with("recipe") || e.contains("arity") || e.contains("two-sided") {
                41
            } else {
                70
            }
        }
    }
}

/// `p` relative to `base` (`..` where needed; both made absolute).
fn relative_to(base: &std::path::Path, p: &str) -> String {
    let abs = |x: &std::path::Path| std::fs::canonicalize(x).unwrap_or_else(|_| x.to_path_buf());
    let (b, t) = (abs(base), abs(std::path::Path::new(p)));
    let bc: Vec<_> = b.components().collect();
    let tc: Vec<_> = t.components().collect();
    let common = bc.iter().zip(&tc).take_while(|(x, y)| x == y).count();
    let mut out = std::path::PathBuf::new();
    for _ in common..bc.len() {
        out.push("..");
    }
    for c in &tc[common..] {
        out.push(c.as_os_str());
    }
    out.display().to_string()
}

fn dims_arg(a: &mut Args) -> Vec<rustred::node::replay::arith::Q> {
    use rustred::node::replay::arith::Q;
    match a.take("--dims") {
        Some(t) => t
            .split(',')
            .map(|x| Q::parse(x).unwrap_or_else(|e| die(&e)))
            .collect(),
        None => rustred::node::mastermap::check_dims(),
    }
}

fn primes_arg(a: &mut Args, default: usize) -> Vec<u64> {
    use rustred::node::mastermap::PRIMES;
    let k: usize = a.parse("--primes").unwrap_or(default);
    if !(2..=PRIMES.len()).contains(&k) {
        die(&format!("--primes must be 2..={}", PRIMES.len()));
    }
    PRIMES[..k].to_vec()
}

fn keys_arg(t: &str) -> Vec<Vec<i64>> {
    rustred::node::mastermap::parse_keys(t).unwrap_or_else(|e| die(&e))
}

fn load_table(path: &str, masters: Option<String>) -> rustred::node::mastermap::Table {
    let mut t = rustred::node::mastermap::Table::load(path).unwrap_or_else(|e| die(&e));
    if let Some(m) = masters {
        t.masters = keys_arg(&m);
    }
    t
}

fn write_json(path: &str, v: &serde_json::Value) {
    std::fs::write(path, serde_json::to_string_pretty(v).expect("json"))
        .unwrap_or_else(|e| die(&format!("{path}: {e}")));
}

/// `rr_node laporta` (t266-10): independent IBP-only Laporta of ONE target in the top sector of MASK modulo
/// subsectors (node::mastermap::laporta), at every --dims value and the first --primes primes; seeds: lines >= 1 with at
/// most DOTS dots, ISP rank <= RANK. Prints/writes per (d, p): status ("ok" = target eliminated onto BASIS + the other
/// free columns listed as extra terms), the basis coefficients mod p, system size. Exit 0 if every cell is ok with no
/// extra free column, else 1.
fn cmd_laporta(mut a: Args) -> i32 {
    let fam_path = a.need("--family");
    let mask_s = a.need("--mask");
    let target = keys_arg(&a.need("--target"));
    let basis = keys_arg(&a.need("--basis"));
    let bx = a.need("--box");
    let out = a.need("--out");
    let dims = dims_arg(&mut a);
    let primes = primes_arg(&mut a, 2);
    a.done();
    let (dots, rank) = bx
        .split_once(',')
        .and_then(|(x, y)| Some((x.parse::<i64>().ok()?, y.parse::<i64>().ok()?)))
        .unwrap_or_else(|| die("--box DOTS,RANK"));
    let mask: Vec<bool> = mask_s.chars().map(|c| c == '1').collect();
    let text =
        std::fs::read_to_string(&fam_path).unwrap_or_else(|e| die(&format!("{fam_path}: {e}")));
    let fam = rustred::node::replay::family::Family::parse(&text).unwrap_or_else(|e| die(&e));
    if target.len() != 1 {
        die("--target takes exactly one key");
    }
    let t0 = std::time::Instant::now();
    let mut cells = vec![];
    let mut ok = true;
    for d in &dims {
        for &p in &primes {
            let r = rustred::node::mastermap::laporta(
                &fam, &mask, &target[0], &basis, dots, rank, d, p,
            )
            .unwrap_or_else(|e| die(&e));
            ok &= r["status"] == "ok" && r["extra_terms"] == 0;
            eprintln!(
                "[laporta] d={} p={p} status={} extra_terms={} rows={} cols={} {:.1}s",
                r["d"],
                r["status"],
                r["extra_terms"],
                r["rows"],
                r["cols"],
                t0.elapsed().as_secs_f64()
            );
            cells.push(r);
        }
    }
    write_json(
        &out,
        &serde_json::json!({"tool": "rustred rr_node laporta (t266-10)", "family": fam_path, "mask": mask_s,
        "box": [dots, rank], "target": rustred::node::mastermap::key_text(&target[0]),
        "basis": basis.iter().map(|k| rustred::node::mastermap::key_text(k)).collect::<Vec<_>>(),
        "cells": cells, "all_ok": ok, "seconds": t0.elapsed().as_secs_f64()}),
    );
    if ok { 0 } else { 1 }
}

/// Every subcommand `main` dispatches (reported by `rr_node --capabilities`).
const VERBS: [&str; 23] = [
    "generate",
    "probe",
    "variants",
    "descent",
    "replay",
    "cover-index",
    "obligations",
    "join",
    "predict",
    "oracle",
    "affine",
    "witness",
    "locus-rank",
    "symmetry",
    "solve-recipe",
    "reduce",
    "mastermap",
    "mapcheck",
    "symmasters",
    "laporta",
    "domaincheck",
    "job",
    "version",
];

/// `rr_node --capabilities`: see the header.
fn cmd_capabilities(a: Args) -> i32 {
    use rustred::node::domaincheck::{OPTIONS, SCHEMA};
    a.done();
    println!(
        "{}",
        serde_json::json!({"schema": "rustred.capabilities.v1", "program": "rr_node", "verbs": VERBS,
            "domaincheck": {"schema": SCHEMA, "options": OPTIONS}, "version": job::version_json()})
    );
    0
}

/// `rr_node domaincheck` (EC-15): see the header and node::domaincheck.
fn cmd_domaincheck(mut a: Args) -> i32 {
    use rustred::node::domaincheck::{DomainCheckArgs, run};
    let table = a.need("--table");
    let keys = a.need("--keys");
    let out = a.need("--out");
    let masters = a.take("--masters").map(|m| keys_arg(&m));
    let point_valid = a.flag("--point-valid");
    let dims = dims_arg(&mut a);
    a.done();
    let args = DomainCheckArgs {
        table,
        keys,
        out,
        masters,
        point_valid,
        dims,
    };
    match run(&args) {
        Ok(n) => {
            eprintln!("[domaincheck] {n} keys -> {}", args.out);
            0
        }
        Err(e) => {
            eprintln!("rr_node domaincheck: {}", e.message());
            e.exit_code()
        }
    }
}

/// `rr_node reduce`: see the header.
fn cmd_reduce(mut a: Args) -> i32 {
    use rustred::node::mastermap::{RatFun, key_text};
    use rustred::node::replay::arith::Q;
    let table = a.need("--table");
    let keys = keys_arg(&a.need("--keys"));
    let out = a.need("--out");
    let masters = a.take("--masters");
    let lower = a.flag("--lower");
    let symbolic = a.flag("--symbolic");
    let points: i64 = a.parse("--points").unwrap_or(80);
    let first: i64 = a.parse("--first-d").unwrap_or(1001);
    let dims = if symbolic {
        (0..points).map(|k| Q::int((first + k) as i128)).collect()
    } else {
        dims_arg(&mut a)
    };
    let primes = primes_arg(&mut a, if symbolic { 8 } else { 6 });
    a.done();
    let mut t = load_table(&table, masters);
    let r = t
        .reduce_mod(&keys, &primes, &dims, lower)
        .unwrap_or_else(|e| die(&e));
    let mut doc = serde_json::json!({"tool": "rustred rr_node reduce", "table": table, "masters": t.masters.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
        "seeds": keys.iter().map(|k| key_text(k)).collect::<Vec<_>>(), "primes": primes.len(), "graph": r.stats});
    let fmt = |m: &std::collections::BTreeMap<Vec<i64>, RatFun>| -> serde_json::Value {
        serde_json::Value::Object(m.iter().map(|(k, f)| (key_text(k), f.to_json())).collect())
    };
    let mut ok = true;
    if symbolic {
        let (fm, f1) = r.fit_part(false).unwrap_or_else(|e| die(&e));
        doc["masters_symbolic"] = serde_json::json!(fm.iter().map(&fmt).collect::<Vec<_>>());
        let mut fails: Vec<String> = f1
            .iter()
            .map(|(k, e)| format!("{}: {e}", key_text(k)))
            .collect();
        if lower {
            let (fl, f2) = r.fit_part(true).unwrap_or_else(|e| die(&e));
            doc["lower_symbolic"] = serde_json::json!(fl.iter().map(&fmt).collect::<Vec<_>>());
            fails.extend(
                f2.iter()
                    .map(|(k, e)| format!("lower {}: {e}", key_text(k))),
            );
        }
        ok = fails.is_empty();
        doc["fit_failures"] = serde_json::json!(fails);
        doc["first_d"] = serde_json::json!(first);
        doc["points"] = serde_json::json!(points);
    } else {
        doc["dims"] = serde_json::json!(
            dims.iter()
                .map(|d| format!("{}/{}", d.n, d.d))
                .collect::<Vec<_>>()
        );
        type PerSeed =
            Vec<std::collections::BTreeMap<Vec<i64>, Vec<symbolica::domains::rational::Rational>>>;
        let ex = |m: Result<PerSeed, String>| -> serde_json::Value {
            let m = m.unwrap_or_else(|e| die(&e));
            serde_json::json!(
                m.iter()
                    .map(|s| s
                        .iter()
                        .map(|(k, v)| (
                            key_text(k),
                            serde_json::json!(v.iter().map(|x| x.to_string()).collect::<Vec<_>>())
                        ))
                        .collect::<serde_json::Map<_, _>>())
                    .collect::<Vec<_>>()
            )
        };
        doc["masters_exact"] = ex(r.exact_masters());
        if lower {
            doc["lower_exact"] = ex(r.exact_lower());
        }
    }
    write_json(&out, &doc);
    eprintln!(
        "[reduce] {} seeds, {} keys, {} edges, {:.1}s -> {out}",
        keys.len(),
        r.stats["keys"],
        r.stats["edges"],
        r.stats["seconds"].as_f64().unwrap_or(0.0)
    );
    i32::from(!ok)
}

fn map_options(a: &mut Args) -> rustred::node::mastermap::MapOptions {
    use rustred::node::mastermap::MapOptions;
    let mut o = MapOptions {
        dims: dims_arg(a),
        primes: primes_arg(a, 6),
        ..MapOptions::default()
    };
    if a.flag("--no-symbolic") {
        o.symbolic = false;
    }
    o.symbolic_lower = a.flag("--symbolic-lower");
    if let Some(k) = a.parse("--points") {
        o.sym_points = k;
    }
    if let Some(k) = a.parse("--sym-primes") {
        o.sym_primes = k;
    }
    if let Some(t) = a.take("--laporta") {
        o.laporta = if t == "none" {
            None
        } else {
            let (x, y) = t
                .split_once(',')
                .unwrap_or_else(|| die("--laporta DOTS,RANK"));
            Some((
                x.parse().unwrap_or_else(|_| die("--laporta")),
                y.parse().unwrap_or_else(|_| die("--laporta")),
            ))
        };
    }
    match a.take("--check-keys").as_deref() {
        None | Some("auto") => {}
        Some("none") => o.no_check = true,
        Some(t) => o.check_keys = Some(keys_arg(t)),
    }
    o
}

/// `rr_node mastermap`: see the header.
fn cmd_mastermap(mut a: Args) -> i32 {
    let old = a.need("--old");
    let new = a.need("--new");
    let out = a.need("--out");
    let om = a.take("--old-masters");
    let nm = a.take("--new-masters");
    let opt = map_options(&mut a);
    a.done();
    let mut o = load_table(&old, om);
    let mut n = load_table(&new, nm);
    let doc = rustred::node::mastermap::build_map(&mut o, &mut n, &opt).unwrap_or_else(|e| die(&e));
    write_json(&out, &doc);
    let acc = doc["accepted"] == true;
    eprintln!(
        "[mastermap] accepted={acc} new_only={} old_only={} -> {out}",
        doc["new_only"], doc["old_only"]
    );
    i32::from(!acc)
}

/// `rr_node symmasters`: see the header.
fn cmd_symmasters(mut a: Args) -> i32 {
    use rustred::node::replay::family::Family;
    use rustred::node::symmasters::{Options, run};
    // parsed once here to fail early on a bad family
    let out = a.need("--out");
    let table = a.take("--table");
    let family = a.take("--family");
    let mask_arg = a.take("--mask");
    let masters = a.take("--masters");
    let prefer = a.take("--prefer").map(|t| keys_arg(&t)).unwrap_or_default();
    let (dots, rank) = match a.take("--box") {
        None => (1, 2),
        Some(t) => {
            let (x, y) = t.split_once(',').unwrap_or_else(|| die("--box DOTS,RANK"));
            (
                x.parse().unwrap_or_else(|_| die("--box")),
                y.parse().unwrap_or_else(|_| die("--box")),
            )
        }
    };
    let inner: Option<i64> = a.parse("--inner");
    let dims = dims_arg(&mut a);
    let primes = primes_arg(&mut a, 3);
    // exact rows at every exactly mapped seed are the default (TD-REVIEW 2); --isp-free-seeds is the fallback mode
    let _ = a.flag("--isp-exact-seeds");
    let isp_exact_seeds = !a.flag("--isp-free-seeds");
    let threads: usize = a.parse("--threads").unwrap_or(1);
    let escalate = a.flag("--escalate");
    let no_tail = a.flag("--no-tail");
    let tail_points: usize = a.parse("--tail-points").unwrap_or(80);
    let tail_primes = a.parse::<usize>("--tail-primes").unwrap_or(8);
    a.done();
    let mut tab = None;
    let (src, mask, mut mlist, origin) = match (table, family) {
        (Some(t), None) => {
            let tb = load_table(&t, masters.clone());
            let src = tb.family_text().unwrap_or_else(|e| die(&e));
            let mask = match &mask_arg {
                Some(m) => m.chars().map(|c| c == '1').collect(),
                None => tb.mask.clone(),
            };
            let ms = tb.masters.clone();
            tab = Some(tb);
            (src, mask, ms, t)
        }
        (None, Some(f)) => {
            let src = std::fs::read_to_string(&f).unwrap_or_else(|e| die(&format!("{f}: {e}")));
            let mask: Vec<bool> = mask_arg
                .unwrap_or_else(|| die("--family needs --mask"))
                .chars()
                .map(|c| c == '1')
                .collect();
            (
                src,
                mask,
                masters.as_deref().map(keys_arg).unwrap_or_default(),
                f,
            )
        }
        _ => die("give exactly one of --table / --family"),
    };
    Family::parse(&src).unwrap_or_else(|e| die(&e));
    let in_sector = |k: &Vec<i64>| {
        k.len() == mask.len()
            && k.iter()
                .zip(&mask)
                .all(|(&x, &m)| if m { x >= 1 } else { x <= 0 })
    };
    let outside: Vec<String> = mlist
        .iter()
        .filter(|k| !in_sector(k))
        .map(|k| rustred::node::mastermap::key_text(k))
        .collect();
    mlist.retain(|k| in_sector(k));
    let mask_l = mask.clone();
    let dr = move |k: &Vec<i64>| -> i64 {
        k.iter()
            .zip(&mask_l)
            .map(|(&x, &m)| if m { x - 1 } else { -x })
            .sum()
    };
    let inner = inner.unwrap_or_else(|| mlist.iter().map(&dr).max().unwrap_or(1).max(1));
    let opt = Options {
        dots,
        rank,
        inner,
        dims,
        primes,
        masters: mlist,
        prefer,
        isp_exact_seeds,
        threads,
    };
    let (mut doc, ok) = run(&src, &mask, &opt).unwrap_or_else(|e| die(&e));
    if escalate {
        doc["escalation"] = rustred::node::symmasters::escalate(&src, &mask, &opt, &doc)
            .unwrap_or_else(|e| serde_json::json!({"status": format!("error: {e}")}));
        doc["saturation"] = doc["escalation"]["status"].clone();
    }
    if let (Some(tb), false) = (tab.as_mut(), no_tail) {
        if ok && !outside.is_empty() {
            doc["tail"] =
                serde_json::json!({"status": "skipped: table masters outside the sector"});
        } else if ok && tb.mask == mask {
            let basis: Vec<Vec<i64>> = doc["VG_basis"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| k.as_str())
                .map(|k| rustred::node::mastermap::parse_key(k).unwrap_or_else(|e| die(&e)))
                .collect();
            let ps = &rustred::node::mastermap::PRIMES[..tail_primes.min(10)];
            doc["tail"] = rustred::node::symmasters::table_tail(
                tb,
                &src,
                &basis,
                &doc["exact"],
                tail_points,
                ps,
            )
            .unwrap_or_else(|e| serde_json::json!({"status": format!("error: {e}")}));
        }
    }
    doc["source"] = serde_json::json!(origin);
    doc["masters_outside_sector"] = serde_json::json!(outside);
    write_json(&out, &doc);
    eprintln!(
        "[symmasters] |G|={} V={} V^G(trace)={} V_G={} ok={ok} {:.1}s -> {out}",
        doc["group"]["order_lines"],
        doc["dim_V"],
        doc["dim_VG_trace"],
        doc["dim_VG"],
        doc["seconds"].as_f64().unwrap_or(0.0)
    );
    i32::from(!ok)
}

/// `rr_node mapcheck`: see the header.
fn cmd_mapcheck(mut a: Args) -> i32 {
    let keys = a.need("--keys");
    let mut items = vec![
        "--laporta".to_string(),
        "none".into(),
        "--no-symbolic".into(),
        "--check-keys".into(),
        keys,
    ];
    items.append(&mut a.items);
    cmd_mastermap(Args { items })
}

fn main() {
    let mut items: Vec<String> = std::env::args().skip(1).collect();
    if items.is_empty() {
        die(
            "usage: rr_node generate|probe|variants|descent|replay|cover-index|obligations|join|predict|oracle|affine|witness|locus-rank|symmetry|solve-recipe|reduce|mastermap|mapcheck|symmasters|laporta|domaincheck|job|version|--capabilities ... (see the source header)",
        );
    }
    let cmd = items.remove(0);
    let a = Args { items };
    let code = match cmd.as_str() {
        "generate" => cmd_generate(a),
        "probe" => cmd_probe(a),
        "variants" => cmd_variants(a),
        "descent" => cmd_descent(a),
        "replay" => cmd_replay(a),
        "cover-index" => cmd_cover_index(a),
        "obligations" => cmd_obligations(a),
        "join" => cmd_join(a),
        "predict" => cmd_predict(a),
        "oracle" => cmd_oracle(a),
        "affine" => cmd_affine(a),
        "witness" => cmd_witness(a),
        "locus-rank" => cmd_locus_rank(a),
        "reduce" => cmd_reduce(a),
        "mastermap" => cmd_mastermap(a),
        "mapcheck" => cmd_mapcheck(a),
        "symmasters" => cmd_symmasters(a),
        "laporta" => cmd_laporta(a),
        "symmetry" => cmd_symmetry(a),
        "solve-recipe" => cmd_solve_recipe(a),
        "domaincheck" => cmd_domaincheck(a),
        "--capabilities" => cmd_capabilities(a),
        "job" => job::cmd_job(a),
        "version" | "--version" => job::cmd_version(a),
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
    fn stop_before_numeric_cli_is_opt_in() {
        for enabled in [false, true] {
            let mut args = Args {
                items: if enabled {
                    vec!["--stop-before-numeric".into()]
                } else {
                    vec![]
                },
            };
            let options = node_options(&mut args, "");
            args.done();
            assert_eq!(options.stop_before_numeric, enabled);
            assert!(!options.strict_numeric_search);
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
