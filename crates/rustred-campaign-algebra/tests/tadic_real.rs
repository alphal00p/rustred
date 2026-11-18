//! lane tadic, REAL runs (ignored; run by hand under a systemd unit): the system route-pole order p* of one key for
//! a seed system built from a family, a centre offset and shells. Parameters by environment (all TEST DATA):
//! TADIC_FAMILY (family text path), TADIC_HEADER (table header JSON with the node order; key path in the file given
//! by TADIC_HEADER_KEY), TADIC_KEY (comma list), TADIC_CENTRE (comma list of offsets from the key, default 0),
//! TADIC_SHELLS (comma list), TADIC_D0 (default 4), TADIC_OUT (JSONL output, appended), TADIC_SCREEN (1 = also run
//! a truncated screen of order 3), TADIC_MAX_SECONDS.

mod common;
mod inputs;

use std::io::Write;

use rustred_campaign_algebra::tadic::SymbolicaTadic;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::tadic::{
    Arith, Interest, PoleOrderOracle, PoleQuery, Rat, TadicBudget, build_ibp_system, parse_shell,
    seeds_around,
};

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|s| !s.is_empty())
}

fn ints(s: &str) -> Vec<i64> {
    s.split(',')
        .map(|x| x.trim().parse().expect("integer"))
        .collect()
}

#[test]
#[ignore]
fn tadic_real_run() {
    if !common::licensed_or_skip("tadic_real_run") {
        return;
    }
    let fam_text =
        std::fs::read_to_string(env("TADIC_FAMILY").expect("TADIC_FAMILY")).expect("family");
    let fam = Family::parse(&fam_text).expect("family parse");
    let n = fam.n();
    let headers: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(env("TADIC_HEADER").expect("TADIC_HEADER")).unwrap(),
    )
    .unwrap();
    let hk = env("TADIC_HEADER_KEY").unwrap_or_default();
    let header = if hk.is_empty() {
        headers
    } else {
        headers[hk.as_str()].clone()
    };
    let cmp = NodeComparator::from_header(&header).expect("comparator");
    let key = ints(&env("TADIC_KEY").expect("TADIC_KEY"));
    let centre = env("TADIC_CENTRE")
        .map(|s| ints(&s))
        .unwrap_or_else(|| vec![0; n]);
    let shells: Vec<_> = env("TADIC_SHELLS")
        .expect("TADIC_SHELLS")
        .split(',')
        .map(|s| parse_shell(s.trim()).unwrap())
        .collect();
    let d0 = Rat::parse(&env("TADIC_D0").unwrap_or_else(|| "4".into())).expect("d0");
    // lines = the family's propagator flags (prop(..., 1))
    let lines: Vec<bool> = fam_text
        .lines()
        .filter(|l| l.trim_start().starts_with("prop("))
        .map(|l| l.trim_end().trim_end_matches(',').ends_with(",1)"))
        .collect();
    assert_eq!(lines.len(), n);
    let t0 = std::time::Instant::now();
    let filter = env("TADIC_SECTOR_FILTER").as_deref() != Some("0");
    // explicit seed offsets from the key (JSON array of arrays), replacing centre + shells when given
    let explicit: Option<Vec<Vec<i64>>> = env("TADIC_OFFSETS").map(|p| {
        serde_json::from_str(&std::fs::read_to_string(p).expect("offsets file"))
            .expect("offsets JSON")
    });
    let seeds = match &explicit {
        Some(offs) => offs
            .iter()
            .map(|o| (0..n).map(|j| key[j] + o[j]).collect())
            .collect(),
        None => seeds_around(&key, std::slice::from_ref(&centre), &shells, &lines, filter),
    };
    let sys = build_ibp_system(&fam, &seeds, true).expect("system");
    let build_s = t0.elapsed().as_secs_f64();
    let k32: Vec<i32> = key.iter().map(|&x| x as i32).collect();
    let low = |k: &[i64]| {
        let a: Vec<i32> = k.iter().map(|&x| x as i32).collect();
        cmp.lower(&a, &k32)
    };
    let budget = TadicBudget {
        max_seconds: env("TADIC_MAX_SECONDS").map(|s| s.parse().unwrap()),
        ..TadicBudget::default()
    };
    let q = PoleQuery {
        target: &key,
        lower: &low,
        d0,
        interest: Interest::AllLower,
        arith: Arith::Exact,
        budget: budget.clone(),
        want_relation: true,
    };
    let o = SymbolicaTadic::new().unwrap();
    let r = o.best_pole_order(&sys, &q);
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM"))
                .map(|l| l.to_string())
        })
        .unwrap_or_default();
    let mut out = serde_json::json!({
        "what": "best route-pole order in system",
        "key": key, "centre": centre, "sector_filter": filter, "shells": env("TADIC_SHELLS"), "d0": d0.text(),
        "seeds": sys.id.seeds, "rows": sys.id.rows, "columns": sys.id.columns,
        "seed_set_sha256": sys.id.seed_set_sha256, "family_sha256": sys.id.family_sha256,
        "build_seconds": build_s, "verdict": r.verdict, "p_star": r.p_star, "rank": r.rank,
        "screen_at_d0": r.screen_at_d0, "seconds": r.seconds, "vmhwm": rss, "notes": r.notes,
        "relation_terms": r.relation.as_ref().map(|x| x.terms.len()),
        "relation_profile": r.relation.as_ref().map(|x| x.profile.clone()),
    });
    if env("TADIC_SCREEN").as_deref() == Some("1") {
        let qs = PoleQuery {
            arith: Arith::Screen {
                order: 3,
                prime: 2_305_843_009_213_693_951,
            },
            ..q
        };
        let rs = o.best_pole_order(&sys, &qs);
        out["screen3"] = serde_json::json!({"verdict": rs.verdict, "p_star": rs.p_star, "seconds": rs.seconds, "notes": rs.notes});
    }
    println!("{out}");
    if let Some(p) = env("TADIC_OUT") {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .unwrap();
        writeln!(f, "{out}").unwrap();
        if let Some(rel) = &r.relation {
            let rp = format!(
                "{}.relation.{}.json",
                env("TADIC_OUT").unwrap(),
                &sys.id.seed_set_sha256[..12]
            );
            std::fs::write(rp, serde_json::to_string(rel).unwrap()).unwrap();
        }
    }
}

/// Cost of the seed-minimisation acceptance hook on a real system with a relation: the T178 recipe results_13_x8_C
/// (231 seeds, the engine found a relation) under the node order B of the fixtures; p* of the full system once, then
/// `keeps_regularity` for single-seed drops (the first TADIC_DROPS seeds, default 6), timed.
#[test]
#[ignore]
fn tadic_keeps_regularity_cost_t178() {
    use rustred_campaign_core::order::NodeOrderRecord;
    use rustred_campaign_core::tadic::{PStar, Verdict, keeps_regularity};
    if !common::licensed_or_skip("tadic_keeps_cost") {
        return;
    }
    let dir = std::env::var("RR_RECIPE_FIXTURES").unwrap_or_else(|_| {
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures/recipe-solve"
            .into()
    });
    let text = std::fs::read_to_string(format!("{dir}/T178.sym")).expect("fixtures");
    let fam = Family::parse(&text).unwrap();
    let n = fam.n();
    let ord: NodeOrderRecord =
        serde_json::from_str(&std::fs::read_to_string(format!("{dir}/order.json")).unwrap())
            .unwrap();
    let cmp = NodeComparator::from_node_order(&ord, n).unwrap();
    let recipe: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{dir}/recipes/results_13_x8_C_json.json")).unwrap(),
    )
    .unwrap();
    let key: Vec<i64> = vec![1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let lines: Vec<bool> = recipe["chart"]
        .as_str()
        .unwrap()
        .chars()
        .map(|c| c == '1')
        .collect();
    let centres: Vec<Vec<i64>> =
        serde_json::from_value(recipe["seeds"]["centres"].clone()).unwrap();
    let shells: Vec<_> = ["S0", "S1", "Sm1", "S1m1", "S2"]
        .iter()
        .map(|s| parse_shell(s).unwrap())
        .collect();
    let mut seeds = seeds_around(&key, &centres, &shells, &lines, true);
    seeds.retain(|s| s[8] - key[8] >= -1);
    let full = build_ibp_system(&fam, &seeds, true).unwrap();
    let k32: Vec<i32> = key.iter().map(|&x| x as i32).collect();
    let low = |k: &[i64]| {
        let a: Vec<i32> = k.iter().map(|&x| x as i32).collect();
        cmp.lower(&a, &k32)
    };
    let q = PoleQuery {
        target: &key,
        lower: &low,
        d0: Rat::int(4),
        interest: Interest::AllLower,
        arith: Arith::Exact,
        budget: TadicBudget::default(),
        want_relation: true,
    };
    let o = SymbolicaTadic::new().unwrap();
    let t0 = std::time::Instant::now();
    let r = o.best_pole_order(&full, &q);
    println!(
        "T178 results_13 full: seeds {} rows {} cols {} verdict {:?} p* {:?} rank {:?} relation terms {:?} profile {:?} screen {:?} {:.1}s",
        full.id.seeds,
        full.id.rows,
        full.id.columns,
        r.verdict,
        r.p_star,
        r.rank.rank_rows,
        r.relation.as_ref().map(|x| x.terms.len()),
        r.relation.as_ref().map(|x| x.profile.clone()),
        r.screen_at_d0,
        t0.elapsed().as_secs_f64()
    );
    assert_eq!(r.verdict, Verdict::Exact);
    let bound = r.p_star.unwrap();
    println!(
        "relation seeds: {} of {}",
        r.relation_seeds.as_ref().map_or(0, |v| v.len()),
        full.seeds.len()
    );
    let drops: usize = std::env::var("TADIC_DROPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let all: Vec<usize> = (0..full.seeds.len()).collect();
    for d in 0..drops.min(full.seeds.len()) {
        let keep: Vec<usize> = all.iter().copied().filter(|&s| s != d).collect();
        let t1 = std::time::Instant::now();
        let after = full.restrict_to_seeds(&keep);
        let v = keeps_regularity(&o, &full, &after, &q, Some(&r));
        println!(
            "drop seed {d} {:?}: kept {:?} by_trace {} p*(after) {:?} {:.2}s",
            full.seeds[d],
            v.kept,
            v.by_trace,
            v.after.p_star,
            t1.elapsed().as_secs_f64()
        );
        if bound != PStar::NoRelation {
            assert!(v.kept.is_some());
        }
    }
}
