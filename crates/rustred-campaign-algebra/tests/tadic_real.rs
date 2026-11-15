//! lane tadic, REAL runs (ignored; run by hand under a systemd unit): the system route-pole order p* of one key for
//! a seed system built from a family, a centre offset and shells. Parameters by environment (all TEST DATA):
//! TADIC_FAMILY (family text path), TADIC_HEADER (table header JSON with the node order; key path in the file given
//! by TADIC_HEADER_KEY), TADIC_KEY (comma list), TADIC_CENTRE (comma list of offsets from the key, default 0),
//! TADIC_SHELLS (comma list), TADIC_D0 (default 4), TADIC_OUT (JSONL output, appended), TADIC_SCREEN (1 = also run
//! a truncated screen of order 3), TADIC_MAX_SECONDS.

mod common;

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
    let seeds = seeds_around(&key, &[centre.clone()], &shells, &lines, true);
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
        "key": key, "centre": centre, "shells": env("TADIC_SHELLS"), "d0": d0.text(),
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
