//! rr_regbasis: construct and certify a REGULAR terminal set R of a closed node on its band (lane regbasis;
//! interfaces/regbasis.md), write the export object and the tower record. Nothing about any node is known here: the
//! node, its closure statement, its box, its published-master map and the user's eps box are INPUT DATA.
//!
//! Usage:
//!   rr_regbasis --import-spec SPEC.json --statement-dir DIR --out DIR
//!               [--import-base /] [--index-dir DIR] [--cache-mb 4096] [--spec JSON|PATH] [--node NAME]
//!               [--box BOXDEF.json] [--published MAP.json] [--eps-box JSON|PATH] [--identity-digest TEXT]
//!               [--max-depth 32] [--exact-max-keys 200000] [--no-tower] [--tower-only [--tower-r-max N]]
//! `--tower-only` (lane polefix, audit r2-poles F7): no construction; the tower test alone, UNSEEDED, on the DOT-FREE
//! keys composed through the statement, ranks 0 ..= N (default: the band's highest rank + 1, so the record shows
//! whether the statement serves any dot-free key beyond its band: a key it does not serve is `undecided`).
//! The band = the statement's start keys in the top sector (the box the closure was run on); `--box` gives the box's
//! r_max and its digest for the binding (else the band itself is digested). Needs the Symbolica licence
//! (`source WS/ops/env.sh`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rustred_campaign_algebra::epsdepth::record_source::{RecordSource, file_sha256};
use rustred_campaign_algebra::epsdepth::{SymbolicaExact, SymbolicaLaurent, TCoeff};
use rustred_campaign_algebra::regbasis::adapter::EpsdepthCompose;
use rustred_campaign_algebra::regbasis::build::{BuildInput, Published, construct};
use rustred_campaign_algebra::regbasis::tower::{TowerInput, TowerSeed, tower};
use rustred_campaign_algebra::{SymbolicaAlgebra, licensed};
use rustred_campaign_core::boxdef::{BoxDef, blake3_hex};
use rustred_campaign_core::epsdepth::{
    ComposeParams, EpsBox, EpsBoxChecked, ReductionSource, compose_streaming,
};
use rustred_campaign_core::import::{self, LoadOptions, TableSetSpec};
use rustred_campaign_core::key::Key;
use rustred_campaign_core::regbasis::record::Binding;
use rustred_campaign_core::regbasis::{Availability, Compose, RegBasisSpec, rank_of};
use serde_json::{Value, json};
use symbolica::domains::rational::Q as QF;
type QPc = rustred_campaign_algebra::regbasis::series::QP;

struct EpsAvail(EpsBoxChecked);

impl Availability for EpsAvail {
    fn available(&self, key: Option<&[i32]>, _label: &str) -> Option<i64> {
        key.and_then(|k| self.0.available(k).ok())
    }
}

fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .map(str::to_string)
        })
        .and_then(|l| {
            l.split_whitespace()
                .nth(1)
                .and_then(|x| x.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb / 1024)
}

fn text_or_file(s: &str) -> Result<Value, String> {
    let t = if Path::new(s).is_file() {
        std::fs::read_to_string(s).map_err(|e| e.to_string())?
    } else {
        s.to_string()
    };
    serde_json::from_str(&t).map_err(|e| format!("{s}: {e}"))
}

struct RunCtx<'a> {
    node: &'a str,
    chart: &'a [bool],
    terminals: &'a [Key],
    band: &'a [Key],
    r_max: Option<u32>,
    set: &'a rustred_campaign_core::import::ImportedSet,
    spec: &'a RegBasisSpec,
    published: Option<&'a Published>,
    avail: Option<&'a dyn Availability>,
    binding: Binding,
    out: &'a Path,
    tower: bool,
    backend: &'a str,
    given: &'a [(String, Vec<Key>)],
    closure: &'a [Key],
    /// `--tower-only`: the highest rank of the unseeded tower (no construction).
    tower_only: Option<u32>,
}

/// `--tower-only`: the unseeded tower test on the dot-free keys composed through the statement (F7).
fn tower_only(comp: &mut dyn Compose, c: &RunCtx<'_>, r_max: u32) -> Result<Value, String> {
    let cmp_fn = |a: &[i32], b: &[i32]| c.set.cmp.cmp(a, b);
    let t2 = Instant::now();
    let mut spec = c.spec.clone();
    spec.tower_seed_with_band = false;
    spec.tower_max_rank = Some(r_max);
    let tin = TowerInput {
        chart: c.chart,
        terminals: c.terminals,
        cmp: &cmp_fn,
        r_max,
        spec: &spec,
        published: c.published,
        seed: None,
        binding: Some(c.binding.clone()),
    };
    let tw = tower(comp, &tin)?;
    eprintln!(
        "TOWER-ONLY {}: {} ({:.1} s)",
        c.node,
        tw.verdict,
        t2.elapsed().as_secs_f64()
    );
    std::fs::create_dir_all(c.out).map_err(|e| e.to_string())?;
    let text = rustred_campaign_core::schema::canon::canonical_json(&tw.record, false)?;
    std::fs::write(c.out.join("tower.json"), text).map_err(|e| e.to_string())?;
    let ranks: Vec<Value> = tw
        .ranks
        .iter()
        .map(|r| {
            json!({"rank": r.rank, "keys_total": r.keys_total, "keys_composed": r.keys_composed,
                "undecided": r.undecided, "z": r.length, "increment": r.increment, "elementary": r.elementary,
                "max_pole": r.max_pole, "first_new_key": r.first_new_key, "source": r.source, "complete": r.complete})
        })
        .collect();
    Ok(
        json!({"node": c.node, "mode": "tower-only (unseeded, dot-free keys through the statement)",
        "band_keys": c.band.len(), "r_max_probed": r_max, "reference": tw.reference, "verdict": tw.verdict,
        "z_through": tw.z_through, "ranks": ranks, "seconds": t2.elapsed().as_secs_f64(), "rss_peak_mb": rss_mb()}),
    )
}

fn build_and_tower(comp: &mut dyn Compose, c: &RunCtx<'_>) -> Result<Value, String> {
    if let Some(r) = c.tower_only {
        return tower_only(comp, c, r);
    }
    let cmp_fn = |a: &[i32], b: &[i32]| c.set.cmp.cmp(a, b);
    let t1 = Instant::now();
    let inp = BuildInput {
        chart: c.chart,
        terminals: c.terminals,
        band: c.band,
        band_r_max: c.r_max,
        cmp: &cmp_fn,
        spec: c.spec,
        published: c.published,
        availability: c.avail,
        binding: Some(c.binding.clone()),
        given_sets: c.given,
        closure_extra: c.closure,
    };
    let res = construct(comp, &inp)?;
    let t_build = t1.elapsed().as_secs_f64();
    eprintln!(
        "BUILD {}: R = {:?} ({:?}); {}; q {} z {}; c1 {:?} c3 {:?} c4 {:?} c5 {:?} c2 {:?}/{:?}; {t_build:.1} s, rss {} MB",
        c.node,
        res.regular_set,
        res.sources,
        res.verdict,
        res.q,
        res.lattice.length(),
        res.cert_lattice,
        res.cert_det,
        res.cert_poles_check,
        res.cert_tadic_smith,
        res.cert_keys_with_pole,
        res.cert_keys_checked,
        rss_mb()
    );
    std::fs::create_dir_all(c.out).map_err(|e| e.to_string())?;
    std::fs::write(
        c.out.join("regular_basis.json"),
        res.record_text.clone().unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    let mut summary = json!({
        "node": c.node, "backend": c.backend, "regular_set": res.regular_set, "sources": res.sources,
        "verdict": res.verdict, "q": res.q, "z": res.lattice.length(), "keys": c.band.len(),
        "decided": res.keys_decided, "keys_with_pole_on_T": res.keys_with_pole_on_t,
        "certificates": res.record["certificate"], "needs": res.record["need"], "M_order": res.record["M_order"],
        "M_inv_order": res.record["M_inv_order"], "det_M_order": res.record["det_M_order"],
        "terminal_in_t_N": res.terminal_in_t_n, "timing": {"build_s": t_build, "rss_peak_mb": rss_mb()},
        "given_sets": res.record["given_sets"], "freedom": res.record["freedom"],
    });
    if c.tower {
        let t2 = Instant::now();
        let seed = TowerSeed::from_build(&res, c.published.is_some());
        let tin = TowerInput {
            chart: c.chart,
            terminals: c.terminals,
            cmp: &cmp_fn,
            r_max: res.r_max,
            spec: c.spec,
            published: c.published,
            seed,
            binding: Some(c.binding.clone()),
        };
        let tw = tower(comp, &tin)?;
        eprintln!(
            "TOWER {}: {} ({:.1} s)",
            c.node,
            tw.verdict,
            t2.elapsed().as_secs_f64()
        );
        let text = rustred_campaign_core::schema::canon::canonical_json(&tw.record, false)?;
        std::fs::write(c.out.join("tower.json"), text).map_err(|e| e.to_string())?;
        summary["tower"] = json!({"verdict": tw.verdict, "seconds": t2.elapsed().as_secs_f64()});
    }
    Ok(summary)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("rr_regbasis: ERROR: {e}");
        std::process::exit(1);
    }
}

/// `rr_regbasis need-from-map --map MAP.json [--d0 4] [--base 0] [--eps-box JSON|PATH] [--exact] [--out FILE]`:
/// the need of the map's rows (X = M_X L) read off M_X^-1 (lane poles' reader and exact check).
fn need_cmd(args: &[String]) -> Result<(), String> {
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    if !licensed() {
        return Err("no Symbolica licence (source WS/ops/env.sh)".into());
    }
    let map_path = get("--map").ok_or("missing --map")?;
    let map =
        rustred_campaign_algebra::valuation::basis::read_master_map(&text_or_file(&map_path)?)?;
    let d0 =
        rustred_campaign_algebra::valuation::D0::parse(&get("--d0").unwrap_or_else(|| "4".into()))
            .map_err(|e| e.to_string())?;
    let base: i64 = get("--base").and_then(|x| x.parse().ok()).unwrap_or(0);
    let avail = match get("--eps-box") {
        Some(s) => Some(EpsAvail(EpsBox::from_value(&text_or_file(&s)?)?)),
        None => None,
    };
    let (t, _chk, extra) = rustred_campaign_algebra::regbasis::need::need_from_map(
        &map,
        &d0,
        base,
        avail.as_ref().map(|a| a as &dyn Availability),
        args.iter().any(|a| a == "--exact"),
    )?;
    let v = json!({"map": map_path, "d0": d0.text(), "need": t.to_value(), "map_check": extra});
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    match get("--out") {
        Some(o) => std::fs::write(o, text).map_err(|e| e.to_string())?,
        None => println!("{text}"),
    }
    Ok(())
}

/// `rr_regbasis key --import-spec S --statement-dir D --key "a,b,.." --record REGULAR_BASIS.json [--index-dir I] [--out F]`:
/// one key, recomputed in this process from the statement: its exact composed vector on T by TWO routes (this lane's exact
/// cone composition `EpsdepthCompose::exact`, and lane epsdepth's `onto_terminals` with its exact backend), compared;
/// its coefficients on the record's R (c(K) M, exact) with their orders at d0; the statement row serving it and that
/// relation's own route-pole profile (top-sector and sub-sector terms separately).
fn key_cmd(args: &[String]) -> Result<(), String> {
    use rustred_campaign_algebra::regbasis::adapter::rf_t_to_d;
    use rustred_campaign_algebra::regbasis::exact::row_times;
    use rustred_campaign_algebra::regbasis::series::{parse_rf, rf_text, rf_valuation};
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    if !licensed() {
        return Err("no Symbolica licence (source WS/ops/env.sh)".into());
    }
    let t0 = Instant::now();
    let spec_path = get("--import-spec").ok_or("missing --import-spec")?;
    let stmt = PathBuf::from(get("--statement-dir").ok_or("missing --statement-dir")?);
    let key: Key = get("--key")
        .ok_or("missing --key")?
        .split(',')
        .map(|x| x.trim().parse::<i32>().map_err(|e| format!("--key: {e}")))
        .collect::<Result<_, _>>()?;
    let rec_path = get("--record").ok_or("missing --record")?;
    let rec: Value =
        serde_json::from_str(&std::fs::read_to_string(&rec_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let digest_ok = rustred_campaign_core::regbasis::record::verify(&rec).is_ok();
    let d0t = rec["binding"]["d0"].as_str().unwrap_or("4").to_string();
    let d0 = rustred_campaign_algebra::valuation::D0::parse(&d0t).map_err(|e| e.to_string())?;
    let ts = TableSetSpec::read(Path::new(&spec_path)).map_err(|e| e.to_string())?;
    let opts = LoadOptions {
        index_dir: get("--index-dir").map(PathBuf::from),
    };
    let set = import::load_with(&ts, Path::new("/"), &opts).map_err(|e| e.to_string())?;
    let alg = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let lau = SymbolicaLaurent::new(&d0t)?;
    let mut src = RecordSource::from_statement(&set, &stmt, alg, lau.clone(), 4096 << 20)?;
    let terminals: Vec<Key> = src.terminals().to_vec();
    let n = terminals.len();
    let idx = src
        .index_of(&key)
        .ok_or("key not in the statement's reduction")?;
    let status = format!("{:?}", src.status(idx));
    let server = src.server(idx);
    let in_start = src.box_keys().contains(&idx);
    // the statement row of the key (verbatim)
    let row = std::fs::read_to_string(stmt.join("statement.jsonl"))
        .map_err(|e| e.to_string())?
        .lines()
        .find(|l| {
            serde_json::from_str::<Value>(l)
                .ok()
                .is_some_and(|v| v["key"] == json!(key))
        })
        .map(str::to_string);
    // the serving relation's own route-pole profile at the key (every term: top and sub-sector)
    let local = src.local_profile_full(idx, 1_000_000);
    let chart: Vec<bool> = src.lines().to_vec();
    let local_v = match &local {
        Ok(p) => {
            let (mut top, mut sub) = (vec![], vec![]);
            for (k, o) in &p.worst {
                let is_sub = k.iter().zip(&chart).any(|(&x, &l)| l && x <= 0);
                if is_sub {
                    sub.push(json!({"key": k, "pole_order": o}))
                } else {
                    top.push(json!({"key": k, "pole_order": o}))
                }
            }
            json!({"pole_order_all_terms": p.pole_order, "pole_order_top_terms": p.pole_order_top,
                   "singular_terms": p.singular_terms, "terms": p.terms, "zero_terms": p.zero_terms,
                   "singular_top_terms": top.len(), "singular_subsector_terms": sub.len(),
                   "top_singular": top, "subsector_singular": sub})
        }
        Err(e) => json!({"error": e}),
    };
    // route A: this lane's exact cone composition (own code path)
    let params = ComposeParams {
        target: 1,
        ..ComposeParams::default()
    };
    let a_texts = {
        let mut comp = EpsdepthCompose::new(
            &mut src,
            lau.clone(),
            d0.value().clone(),
            n,
            params.clone(),
            1_000_000,
        );
        comp.exact(&key).map_err(|e| format!("route A: {e}"))?
    };
    // route B: lane epsdepth's composition with its exact backend
    let mut ex = SymbolicaExact::new()?;
    let b_vec =
        rustred_campaign_core::epsdepth::onto_terminals(&mut src, &mut ex, idx, n, &params)?
            .map_err(|e| format!("route B: {e}"))?;
    let b_texts: Vec<String> = b_vec
        .iter()
        .map(|c| match c {
            None => "0".to_string(),
            Some(r) => rf_text(&rf_t_to_d(r, d0.value())),
        })
        .collect();
    let a_rf: Vec<_> = a_texts
        .iter()
        .map(|t| parse_rf(t))
        .collect::<Result<_, _>>()?;
    let b_rf: Vec<_> = b_texts
        .iter()
        .map(|t| parse_rf(t))
        .collect::<Result<_, _>>()?;
    let routes_equal = a_rf
        .iter()
        .zip(&b_rf)
        .all(|(x, y)| (x - y).numerator.is_zero());
    let ord = |r: &rustred_campaign_algebra::regbasis::series::RF| rf_valuation(r, d0.value());
    // M of the record: T = M R
    let m_texts: Vec<Vec<String>> =
        serde_json::from_value(rec["M"].clone()).map_err(|e| format!("record M: {e}"))?;
    let m: Vec<Vec<_>> = m_texts
        .iter()
        .map(|r| r.iter().map(|t| parse_rf(t)).collect::<Result<Vec<_>, _>>())
        .collect::<Result<_, _>>()?;
    let on_r = row_times(&a_rf, &m);
    let out = json!({
        "key": key, "terminals": terminals, "in_statement": true, "in_start_band": in_start, "status": status,
        "server": server, "statement_row": row,
        "record": {"path": rec_path, "digest": rec["digest"], "digest_verifies": digest_ok, "regular_set": rec["regular_set"],
                   "M": m_texts, "M_order": rec["M_order"]},
        "c_on_T": {"route_A_own_cone": a_texts, "route_B_epsdepth_exact": b_texts, "routes_equal": routes_equal,
                   "orders_at_d0": a_rf.iter().map(ord).collect::<Vec<_>>()},
        "c_on_R": {"exact": on_r.iter().map(rf_text).collect::<Vec<_>>(),
                   "orders_at_d0": on_r.iter().map(ord).collect::<Vec<_>>(),
                   "convention": "K = c_T(K) . T = c_T(K) M . R (modulo sub-sectors)"},
        "serving_relation_route_poles": local_v,
        "seconds": t0.elapsed().as_secs_f64(),
    });
    let text = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    match get("--out") {
        Some(o) => std::fs::write(o, &text).map_err(|e| e.to_string())?,
        None => println!("{text}"),
    }
    Ok(())
}

/// `rr_regbasis subsector --import-spec S --statement-dir D --key "a,b,.." [--index-dir I] [--out F]`: ONE key's reduction
/// INCLUDING its sub-sector terms: over the key's top-sector cone (as the statement serves it), top-down exact weights
/// w(k) (w(key) = 1, w(child) += w(parent) * c(parent -> child)); every sub-sector term s of a served relation collects
/// W(s) += w(k) * c(k -> s) (first generation: s itself is NOT reduced further; the dispatcher's verdict at s says whether
/// the set could serve it). Check: the weights reaching the terminals equal the key's composed vector on T.
type ZPolyN = symbolica::poly::polynomial::MultivariatePolynomial<
    symbolica::domains::integer::IntegerRing,
    u16,
>;

fn subsector_cmd(args: &[String]) -> Result<(), String> {
    use rustred_campaign_algebra::epsdepth::record_source::{line_at, parse_coefficient};
    use rustred_campaign_algebra::regbasis::adapter::{rf_t_to_d, tcoeff_rf};
    use rustred_campaign_algebra::regbasis::series::{RF, ZP, rf_text, rf_valuation, t_vars};
    use rustred_campaign_core::accept::rule::RuleText;
    use rustred_campaign_core::closure::bodies::{
        RhsSlot, Sectors, TermClass, parse_rhs_pattern, rule_point,
    };
    use rustred_campaign_core::epsdepth::KeyStatus;
    use std::collections::HashMap;
    use symbolica::domains::integer::Z;
    use symbolica::domains::rational::Rational;
    use symbolica::domains::rational_polynomial::{
        FromNumeratorAndDenominator, RationalPolynomial,
    };
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    if !licensed() {
        return Err("no Symbolica licence (source WS/ops/env.sh)".into());
    }
    let t0 = Instant::now();
    let spec_path = get("--import-spec").ok_or("missing --import-spec")?;
    let stmt = PathBuf::from(get("--statement-dir").ok_or("missing --statement-dir")?);
    let key: Key = get("--key")
        .ok_or("missing --key")?
        .split(',')
        .map(|x| x.trim().parse::<i32>().map_err(|e| format!("--key: {e}")))
        .collect::<Result<_, _>>()?;
    let d0t = get("--d0").unwrap_or_else(|| "4".into());
    let d0 = rustred_campaign_algebra::valuation::D0::parse(&d0t).map_err(|e| e.to_string())?;
    let ts = TableSetSpec::read(Path::new(&spec_path)).map_err(|e| e.to_string())?;
    let opts = LoadOptions {
        index_dir: get("--index-dir").map(PathBuf::from),
    };
    let set = import::load_with(&ts, Path::new("/"), &opts).map_err(|e| e.to_string())?;
    let alg = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let alg2 = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let lau = SymbolicaLaurent::new(&d0t)?;
    let mut src = RecordSource::from_statement(&set, &stmt, alg, lau.clone(), 4096 << 20)?;
    let arity = set.arity();
    let chart: Vec<bool> = set.frame.mask().to_vec();
    let sectors = Sectors::new(chart.clone());
    let n = src.n_terminals();
    let root = src.index_of(&key).ok_or("key not in the statement")?;
    let tv = t_vars();
    let zero = || {
        RationalPolynomial::from_num_den(
            ZP::new(&Z, None, tv.clone()),
            ZP::new(&Z, None, tv.clone()).one(),
            &Z,
            false,
        )
    };
    let one = || {
        RationalPolynomial::from_num_den(
            ZP::new(&Z, None, tv.clone()).one(),
            ZP::new(&Z, None, tv.clone()).one(),
            &Z,
            false,
        )
    };
    // cone discovery through the source (top-sector terms, credits handled by the source)
    let mut top_terms: HashMap<usize, Vec<(usize, TCoeff)>> = HashMap::new();
    let mut frontier = vec![root];
    let mut seen: std::collections::HashSet<usize> = [root].into_iter().collect();
    while !frontier.is_empty() {
        let served: Vec<usize> = frontier
            .iter()
            .copied()
            .filter(|&k| matches!(src.status(k), KeyStatus::Served))
            .collect();
        for &k in &frontier {
            if let KeyStatus::Open(w) = src.status(k) {
                return Err(format!("open key in the cone: {w}"));
            }
        }
        let mut next = vec![];
        let mut err = None;
        src.relations(&served, &mut |k, r| match r {
            Ok(t) => {
                for (c, _) in &t {
                    if seen.insert(*c) {
                        next.push(*c);
                    }
                }
                top_terms.insert(k, t);
            }
            Err(e) => err = Some(e),
        })?;
        if let Some(e) = err {
            return Err(format!("relation: {e}"));
        }
        frontier = next;
    }
    // post-order (children first), then top-down = reverse
    let mut order = vec![];
    let mut state: HashMap<usize, u8> = HashMap::new();
    let mut stack = vec![(root, false)];
    while let Some((k, done)) = stack.pop() {
        if done {
            state.insert(k, 2);
            order.push(k);
            continue;
        }
        if state.contains_key(&k) {
            continue;
        }
        state.insert(k, 1);
        stack.push((k, true));
        if let Some(t) = top_terms.get(&k) {
            for (c, _) in t {
                if !state.contains_key(c) {
                    stack.push((*c, false));
                }
            }
        }
    }
    order.reverse();
    // full terms (top AND sub-sector) of each served cone key at its rule point (credits: top terms from the source)
    #[derive(Clone)]
    enum Tm {
        Top(usize, TCoeff),
        Sub(Key, TCoeff),
    }
    let mut full: HashMap<usize, Vec<Tm>> = HashMap::new();
    let (mut credits_l, mut mismatch_l) = (0usize, 0usize);
    // parsed records, cached per (table, record): domain / fixed key, term patterns, parsed coefficient polynomials
    #[allow(clippy::type_complexity)]
    let mut rcache: HashMap<
        (usize, usize),
        (
            RuleText,
            Vec<Vec<RhsSlot>>,
            Vec<Result<(ZPolyN, ZPolyN), String>>,
        ),
    > = HashMap::new();
    for &k in &order {
        let KeyStatus::Served = src.status(k) else {
            continue;
        };
        let kk = src.key(k);
        let (t, r) = src.record_of(k).ok_or("served key without a record")?;
        #[allow(clippy::map_entry)]
        if !rcache.contains_key(&(t, r)) {
            let info = set.rule(t, r);
            let (line, _) = line_at(Path::new(&info.path), info.offset)?;
            let rt = RuleText::parse(&line, &set.frame).map_err(|e| e.to_string())?;
            let pats = rt
                .rhs
                .iter()
                .map(|(p, _)| parse_rhs_pattern(p, arity))
                .collect::<Result<Vec<_>, _>>()?;
            let coefs = rt
                .rhs
                .iter()
                .map(|(_, c)| parse_coefficient(&alg2, c, arity))
                .collect();
            rcache.insert((t, r), (rt, pats, coefs));
        }
        let (rt, pats, coefs) = &rcache[&(t, r)];
        let mut pt = vec![];
        let at_key = match &rt.domain {
            Some(dm) => rule_point(dm, &kk, &mut pt),
            None => {
                let fixed: Option<Vec<i32>> = rt
                    .record
                    .fixed
                    .iter()
                    .map(|x| x.and_then(|v| i32::try_from(v).ok()))
                    .collect();
                if fixed.as_deref() == Some(kk.as_slice()) {
                    pt = kk.clone();
                    true
                } else {
                    false
                }
            }
        };
        if !at_key {
            credits_l += 1;
            full.insert(
                k,
                top_terms[&k]
                    .iter()
                    .map(|(c, x)| Tm::Top(*c, x.clone()))
                    .collect(),
            );
            continue;
        }
        let mut v = vec![];
        let mut ntop = 0usize;
        for (jj, pat) in pats.iter().enumerate() {
            let tk: Option<Vec<i32>> = pat
                .iter()
                .map(|sl| match *sl {
                    RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                    RhsSlot::Const(c) => Some(c),
                })
                .collect();
            let tk = tk.ok_or("term key out of range")?;
            let class = sectors.classify_masked(&tk, 0);
            if matches!(
                class,
                TermClass::PositiveNumerator { .. } | TermClass::ZeroSector { .. }
            ) {
                continue;
            }
            let (num, den) = coefs[jj].as_ref().map_err(|e| format!("term {jj}: {e}"))?;
            let c = lau.coeff_at_key(num, den, arity, &pt)?;
            if matches!(c, TCoeff::Zero) {
                continue;
            }
            match class {
                TermClass::Top => {
                    ntop += 1;
                    match src.index_of(&tk) {
                        Some(ci) => v.push(Tm::Top(ci, c)),
                        None => mismatch_l += 1,
                    }
                }
                _ => v.push(Tm::Sub(tk, c)),
            }
        }
        if ntop != top_terms[&k].len() {
            mismatch_l += 1;
        }
        full.insert(k, v);
    }
    if get("--mode").as_deref() != Some("exact") {
        use rustred_campaign_algebra::regbasis::series::Laurent;
        // required absolute precision per key (top-down, exact edge valuations): R(root) = 1 decides every exponent <= 0
        let topdown = &order; // parents before children
        let mut req: HashMap<usize, i64> = HashMap::new();
        req.insert(root, 1);
        for &k in topdown {
            let Some(&rk) = req.get(&k) else { continue };
            if let Some(ts) = full.get(&k) {
                for tm in ts {
                    if let Tm::Top(c, x) = tm {
                        let v = match x {
                            TCoeff::Ratio { v, .. } => *v,
                            TCoeff::Zero => continue,
                        };
                        let e = req.entry(*c).or_insert(i64::MIN);
                        *e = (*e).max(rk - v);
                    }
                }
            }
        }
        let max_req = req.values().copied().max().unwrap_or(1);
        // bottom-up (children first) truncated Laurent vectors over T and the sub-sector keys
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        enum Comp {
            T(usize),
            S(Key),
        }
        let expand = |x: &TCoeff, prec: i64| -> Laurent {
            match tcoeff_rf(x, &tv) {
                None => Laurent::zero(prec, &tv),
                Some(r) => rustred_campaign_algebra::regbasis::series::expand_t_ratio(
                    &r.numerator,
                    &r.denominator,
                    prec,
                    &tv,
                ),
            }
        };
        let mut parents_left: HashMap<usize, usize> = HashMap::new();
        for ts in full.values() {
            for tm in ts {
                if let Tm::Top(c, _) = tm {
                    *parents_left.entry(*c).or_insert(0) += 1;
                }
            }
        }
        let mut vecs: HashMap<usize, HashMap<Comp, Laurent>> = HashMap::new();
        let bottomup: Vec<usize> = order.iter().rev().copied().collect();
        let big = i64::MAX / 8;
        let mut peak = 0usize;
        for &k in &bottomup {
            let rk = *req.get(&k).unwrap_or(&1);
            let v: HashMap<Comp, Laurent> = match src.status(k) {
                KeyStatus::Terminal(j) => {
                    let mut m = HashMap::new();
                    let _one_s = rustred_campaign_algebra::regbasis::series::expand_t_ratio(
                        &ZP::new(&Z, None, tv.clone()).one(),
                        &ZP::new(&Z, None, tv.clone()).one(),
                        big,
                        &tv,
                    );
                    m.insert(
                        Comp::T(j),
                        Laurent::normalised(0, QPc::new(&QF, None, tv.clone()).one(), big),
                    );
                    m
                }
                _ => {
                    let mut m: HashMap<Comp, Laurent> = HashMap::new();
                    for tm in full.get(&k).map(|v| v.as_slice()).unwrap_or(&[]) {
                        match tm {
                            Tm::Sub(sk, x) => {
                                let e = expand(x, rk);
                                let slot = m
                                    .entry(Comp::S(sk.clone()))
                                    .or_insert_with(|| Laurent::zero(big, &tv));
                                *slot = slot.add(&e);
                            }
                            Tm::Top(c, x) => {
                                let child = vecs.get(c).ok_or("child vector missing (cycle?)")?;
                                let vlow = child
                                    .values()
                                    .map(|y| y.valuation().unwrap_or(y.prec()))
                                    .min()
                                    .unwrap_or(0);
                                let e = expand(x, rk - vlow);
                                for (comp, y) in child {
                                    let slot = m
                                        .entry(comp.clone())
                                        .or_insert_with(|| Laurent::zero(big, &tv));
                                    *slot = slot.add(&e.mul(y));
                                }
                            }
                        }
                    }
                    for tm in full.get(&k).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if let Tm::Top(c, _) = tm
                            && let Some(pl) = parents_left.get_mut(c)
                        {
                            *pl -= 1;
                            if *pl == 0 && *c != root {
                                vecs.remove(c);
                            }
                        }
                    }
                    m
                }
            };
            peak = peak.max(v.len());
            vecs.insert(k, v);
        }
        let rv = vecs.remove(&root).ok_or("root vector missing")?;
        let mut rows = vec![];
        let (mut with_pole, mut undecided, mut maxp) = (0usize, 0usize, 0i64);
        let mut tpart = vec![];
        let mut items: Vec<(Comp, Laurent)> = rv.into_iter().collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        for (comp, y) in &items {
            let decided = y.decides_pole();
            let val = y.valuation();
            match comp {
                Comp::T(j) => tpart.push(json!({"terminal": j, "valuation": val, "precision": y.prec(),
                    "leading": val.map(|v| rustred_campaign_algebra::regbasis::series::rational_text(&y.coeff(v).unwrap()))})),
                Comp::S(sk) => {
                    if val.is_none() && y.prec() >= 1 {
                        continue; // zero to a deciding precision: the coefficient of s is O(t^prec), no pole
                    }
                    let p = val.map_or(0, |v| (-v).max(0));
                    if !decided {
                        undecided += 1;
                    } else if p > 0 {
                        with_pole += 1;
                        maxp = maxp.max(p);
                    }
                    let dropped: Vec<usize> = sk.iter().zip(&chart).enumerate().filter(|(_, (x, l))| **l && **x <= 0).map(|(i, _)| i).collect();
                    let disp = match set.disp.first_match(sk) {
                        Ok(rustred_campaign_core::cover::Dispatch::Covered(_)) => "covered by the set's dispatcher",
                        Ok(rustred_campaign_core::cover::Dispatch::Uncovered) => "not served by this set",
                        Ok(_) => "blocked / undecided in this set",
                        Err(_) => "outside the dispatcher's domain",
                    };
                    rows.push(json!({"key": sk, "dropped_line_slots": dropped, "valuation": val, "precision": y.prec(),
                        "decided": decided, "pole_order": p,
                        "leading": val.map(|v| rustred_campaign_algebra::regbasis::series::rational_text(&y.coeff(v).unwrap())),
                        "set": disp}));
                }
            }
        }
        let named = [
            vec![0, 3, 3, 2, 1, 1, 1, 1, 1, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 3, 2, 0, 1, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 3, 3, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ];
        let named_rows: Vec<Value> = named
            .iter()
            .map(|nk| {
                rows.iter()
                    .find(|r| r["key"] == json!(nk))
                    .cloned()
                    .unwrap_or(json!({"key": nk, "coefficient": "O(t^1) or zero: no pole"}))
            })
            .collect();
        let out = json!({
            "key": key, "mode": "laurent (exact coefficients, tracked absolute precision; required precision from the exact edge valuations)",
            "cone_keys": order.len(), "max_required_precision": max_req, "credits_without_subsector_terms": credits_l,
            "term_mismatches": mismatch_l, "peak_vector_components": peak,
            "terminal_components": tpart,
            "subsector_first_generation": {"keys_listed": rows.len(), "with_pole": with_pole, "undecided": undecided,
                                           "max_pole_order": maxp, "named_three": named_rows, "rows": rows},
            "convention": "K = sum_T c_T T + sum_s W(s) s (+ deeper sub-sector terms of the s, NOT followed); W(s) = coefficient of the sub-sector key s in K's reduction along the statement's top-sector cone; listed: every s whose W(s) is not O(t^1) at a deciding precision",
            "seconds": t0.elapsed().as_secs_f64(),
        });
        let text = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
        match get("--out") {
            Some(o) => std::fs::write(o, &text).map_err(|e| e.to_string())?,
            None => println!("{text}"),
        }
        return Ok(());
    }
    // full terms (top AND sub-sector) of each served cone key, read from its record at its rule point
    let mut w: HashMap<usize, RF> = HashMap::new();
    w.insert(root, one());
    let mut wsub: HashMap<Key, RF> = HashMap::new();
    let mut via: HashMap<Key, usize> = HashMap::new();
    let (mut credits, mut mismatch, mut zero_sector_like) = (0usize, 0usize, 0usize);
    let d0r = Rational::zero();
    for &k in &order {
        let KeyStatus::Served = src.status(k) else {
            continue;
        };
        let Some(wk) = w.get(&k).cloned() else {
            continue;
        };
        if wk.numerator.is_zero() {
            continue;
        }
        let kk = src.key(k);
        let (t, r) = src.record_of(k).ok_or("served key without a record")?;
        let info = set.rule(t, r);
        let (line, _) = line_at(Path::new(&info.path), info.offset)?;
        let rt = RuleText::parse(&line, &set.frame).map_err(|e| e.to_string())?;
        let mut pt = vec![];
        let at_key = match &rt.domain {
            Some(dm) => rule_point(dm, &kk, &mut pt),
            None => {
                let fixed: Option<Vec<i32>> = rt
                    .record
                    .fixed
                    .iter()
                    .map(|x| x.and_then(|v| i32::try_from(v).ok()))
                    .collect();
                if fixed.as_deref() == Some(kk.as_slice()) {
                    pt = kk.clone();
                    true
                } else {
                    false
                }
            }
        };
        if !at_key {
            // a credit (served through a symmetry image): top terms from the source; its sub-sector terms are not read
            credits += 1;
            for (c, coeff) in &top_terms[&k] {
                if let Some(rc) = tcoeff_rf(coeff, &tv) {
                    let e = w.entry(*c).or_insert_with(zero);
                    *e = &*e + &(&wk * &rc);
                }
            }
            continue;
        }
        let mut top_seen = 0usize;
        for (p, ctext) in &rt.rhs {
            let pat = parse_rhs_pattern(p, arity)?;
            let tk: Option<Vec<i32>> = pat
                .iter()
                .map(|sl| match *sl {
                    RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                    RhsSlot::Const(c) => Some(c),
                })
                .collect();
            let tk = tk.ok_or("term key out of range")?;
            let class = sectors.classify_masked(&tk, 0);
            if matches!(class, TermClass::PositiveNumerator { .. }) {
                continue;
            }
            let (num, den) = parse_coefficient(&alg2, ctext, arity)?;
            let c = lau.coeff_at_key(&num, &den, arity, &pt)?;
            let Some(rc) = tcoeff_rf(&c, &tv) else {
                continue;
            };
            match class {
                TermClass::Top => {
                    top_seen += 1;
                    let Some(ci) = src.index_of(&tk) else {
                        mismatch += 1;
                        continue;
                    };
                    let e = w.entry(ci).or_insert_with(zero);
                    *e = &*e + &(&wk * &rc);
                }
                TermClass::ZeroSector { .. } => zero_sector_like += 1,
                _ => {
                    let e = wsub.entry(tk.clone()).or_insert_with(zero);
                    *e = &*e + &(&wk * &rc);
                    *via.entry(tk).or_insert(0) += 1;
                }
            }
        }
        if top_seen != top_terms[&k].len() {
            mismatch += 1;
        }
    }
    // check against the composed vector on T
    let tkeys: Vec<Key> = src.terminals().to_vec();
    let on_t: Vec<RF> = tkeys
        .iter()
        .map(|t| {
            src.index_of(t)
                .and_then(|i| w.get(&i).cloned())
                .unwrap_or_else(zero)
        })
        .collect();
    let mut comp = EpsdepthCompose::new(
        &mut src,
        lau.clone(),
        d0.value().clone(),
        n,
        ComposeParams::default(),
        1_000_000,
    );
    let a_texts = comp.exact(&key).map_err(|e| format!("route A: {e}"))?;
    let check_t = on_t.iter().zip(&a_texts).all(|(x, a)| {
        (&rf_t_to_d(x, d0.value())
            - &rustred_campaign_algebra::regbasis::series::parse_rf(a).unwrap())
            .numerator
            .is_zero()
    });
    let mut subs: Vec<(Key, RF)> = wsub
        .into_iter()
        .filter(|(_, r)| !r.numerator.is_zero())
        .collect();
    subs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut rows = vec![];
    let (mut with_pole, mut maxp) = (0usize, 0i64);
    for (s, r) in &subs {
        let v = rf_valuation(r, &d0r).unwrap_or(0);
        let p = (-v).max(0);
        if p > 0 {
            with_pole += 1;
        }
        maxp = maxp.max(p);
        let dropped: Vec<usize> = s
            .iter()
            .zip(&chart)
            .enumerate()
            .filter(|(_, (x, l))| **l && **x <= 0)
            .map(|(i, _)| i)
            .collect();
        let disp = match set.disp.first_match(s) {
            Ok(rustred_campaign_core::cover::Dispatch::Covered(_)) => {
                "covered by the set's dispatcher"
            }
            Ok(rustred_campaign_core::cover::Dispatch::Uncovered) => "not served by this set",
            Ok(_) => "blocked / undecided in this set",
            Err(_) => "outside the dispatcher's domain",
        };
        rows.push(json!({"key": s, "dropped_line_slots": dropped, "order_at_d0": v, "pole_order": p,
                         "coefficient": rf_text(&rf_t_to_d(r, d0.value())), "relations_contributing": via[s], "set": disp}));
    }
    let named: Vec<Value> = rows
        .iter()
        .filter(|r| r["relations_contributing"].as_u64().is_some())
        .take(0)
        .cloned()
        .collect();
    let _ = named;
    let out = json!({
        "key": key, "cone_keys": order.len(), "credits_without_subsector_terms": credits, "term_mismatches": mismatch,
        "zero_sector_terms": zero_sector_like,
        "check_terminal_weights_equal_composed_vector": check_t,
        "subsector_first_generation": {"keys": rows.len(), "with_pole": with_pole, "max_pole_order": maxp, "rows": rows},
        "convention": "K = sum_T c_T T + sum_s W(s) s (+ deeper sub-sector terms of the s, NOT followed): W(s) = first-generation coefficient of the sub-sector key s in K's reduction along the statement's top-sector cone",
        "seconds": t0.elapsed().as_secs_f64(),
    });
    let text = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    match get("--out") {
        Some(o) => std::fs::write(o, &text).map_err(|e| e.to_string())?,
        None => println!("{text}"),
    }
    Ok(())
}

/// Per-root result of the scalar-weight pass.
struct SubweightResult {
    value: Value,
}

/// `rr_regbasis subweights --import-spec S --statement-dir D --key "a,.." [--record R.json] [--sample N --seed X]
/// [--q 2] [--index-dir I] [--checkpoint-dir C] [--out F]`: the SUB-SECTOR coefficients W(s) of one key's reduction
/// (first generation: the sub-sector keys are not reduced further), by ONE scalar weight per top-sector cone key:
/// w(key) = 1, w(child) += w(k) c(k -> child) (top terms exactly as lane epsdepth's source serves them), W(s) += w(k)
/// c(k -> s) (sub-sector terms read from the serving record at its rule point). Truncated Laurent series with tracked
/// absolute precision; the precision each weight needs comes from the EXACT edge valuations (bottom-up: a sub-sector
/// edge needs exponents < q, a top edge the child's need minus its order, a terminal 3), so every reported order is
/// exact (decided) and a coefficient zero to its precision is reported as O(t^prec), never as a value.
fn subweights_cmd(args: &[String]) -> Result<(), String> {
    use rustred_campaign_algebra::epsdepth::record_source::{line_at, parse_coefficient};
    use rustred_campaign_algebra::regbasis::adapter::tcoeff_rf;
    use rustred_campaign_algebra::regbasis::series::{
        Laurent, expand_t_ratio, parse_rf, rational_text, t_vars,
    };
    use rustred_campaign_core::accept::rule::RuleText;
    use rustred_campaign_core::closure::bodies::{
        RhsSlot, Sectors, TermClass, parse_rhs_pattern, rule_point,
    };
    use rustred_campaign_core::epsdepth::KeyStatus;
    use std::collections::HashMap;
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    if !licensed() {
        return Err("no Symbolica licence (source WS/ops/env.sh)".into());
    }
    let t0 = Instant::now();
    let log = |m: &str| eprintln!("[{:>8.1}s] {m}", t0.elapsed().as_secs_f64());
    let spec_path = get("--import-spec").ok_or("missing --import-spec")?;
    let stmt = PathBuf::from(get("--statement-dir").ok_or("missing --statement-dir")?);
    let key0: Key = get("--key")
        .ok_or("missing --key")?
        .split(',')
        .map(|x| x.trim().parse::<i32>().map_err(|e| format!("--key: {e}")))
        .collect::<Result<_, _>>()?;
    let q: i64 = get("--q").and_then(|x| x.parse().ok()).unwrap_or(2);
    let q_t: i64 = 3;
    let ckdir = get("--checkpoint-dir").map(PathBuf::from);
    let d0t = "4".to_string();
    let ts = TableSetSpec::read(Path::new(&spec_path)).map_err(|e| e.to_string())?;
    let opts = LoadOptions {
        index_dir: get("--index-dir").map(PathBuf::from),
    };
    let set = import::load_with(&ts, Path::new("/"), &opts).map_err(|e| e.to_string())?;
    log("set loaded");
    let alg = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let alg2 = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let lau = SymbolicaLaurent::new(&d0t)?;
    let mut src = RecordSource::from_statement(&set, &stmt, alg, lau.clone(), 4096 << 20)?;
    src.threads = 4;
    let arity = set.arity();
    let chart: Vec<bool> = set.frame.mask().to_vec();
    let sectors = Sectors::new(chart.clone());
    let n = src.n_terminals();
    let tv = t_vars();
    // optional R record (cross-check of the top-sector orders on R)
    let m_rec: Option<(
        Value,
        Vec<Vec<rustred_campaign_algebra::regbasis::series::RF>>,
    )> = match get("--record") {
        None => None,
        Some(p) => {
            let rec: Value =
                serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let mt: Vec<Vec<String>> =
                serde_json::from_value(rec["M"].clone()).map_err(|e| e.to_string())?;
            let m = mt
                .iter()
                .map(|r| r.iter().map(|t| parse_rf(t)).collect::<Result<Vec<_>, _>>())
                .collect::<Result<Vec<_>, _>>()?;
            Some((
                json!({"path": p, "digest": rec["digest"], "regular_set": rec["regular_set"]}),
                m,
            ))
        }
    };
    // roots: the key, then an optional seeded sample of further top-sector closure keys
    let mut roots: Vec<Key> = vec![key0.clone()];
    if let Some(ns) = get("--sample").and_then(|x| x.parse::<usize>().ok()) {
        let seed: u64 = get("--seed").and_then(|x| x.parse().ok()).unwrap_or(0);
        let mut pool: Vec<Key> = (0..src.n_keys())
            .map(|i| src.key(i))
            .filter(|k| rustred_campaign_core::regbasis::rank_of(&chart, k).is_some() && *k != key0)
            .filter(|k| !src.terminals().contains(k))
            .collect();
        pool.sort();
        let mut st = seed;
        let mut next = || {
            st = st.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = st;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        };
        for _ in 0..ns.min(pool.len()) {
            let i = (next() % pool.len() as u64) as usize;
            roots.push(pool.swap_remove(i));
        }
    }
    // caches shared across roots
    let mut top_cache: HashMap<usize, Vec<(usize, TCoeff)>> = HashMap::new();
    #[allow(clippy::type_complexity)]
    let mut rec_cache: HashMap<(usize, usize), (RuleText, Vec<Vec<RhsSlot>>)> = HashMap::new();
    #[allow(clippy::type_complexity)]
    let mut coef_cache: HashMap<(usize, usize, usize), Result<(ZPolyN, ZPolyN), String>> =
        HashMap::new();
    let mut sub_cache: HashMap<usize, Result<Vec<(Key, TCoeff)>, String>> = HashMap::new();
    let mut results: Vec<SubweightResult> = vec![];
    let from: usize = get("--from").and_then(|x| x.parse().ok()).unwrap_or(0);
    'roots: for (ri, root_key) in roots.iter().enumerate() {
        if ri < from {
            continue;
        }
        let tr0 = Instant::now();
        let root = src.index_of(root_key).ok_or("key not in the statement")?;
        // cone (top-sector terms, exact coefficients, credits by the source)
        let mut frontier = vec![root];
        let mut seen: std::collections::HashSet<usize> = [root].into_iter().collect();
        while !frontier.is_empty() {
            let need: Vec<usize> = frontier
                .iter()
                .copied()
                .filter(|&k| {
                    matches!(src.status(k), KeyStatus::Served) && !top_cache.contains_key(&k)
                })
                .collect();
            for &k in &frontier {
                if let KeyStatus::Open(w) = src.status(k) {
                    if ri == 0 {
                        return Err(format!("open key in the cone: {w}"));
                    }
                    log(&format!(
                        "root {ri} {:?}: SKIPPED (its cone is not closed in this statement: {w})",
                        root_key
                    ));
                    results.push(SubweightResult {
                        value: json!({"key": root_key, "skipped": format!("cone not closed in this statement: {w}")}),
                    });
                    continue 'roots;
                }
            }
            let mut err = None;
            src.relations(&need, &mut |k, r| match r {
                Ok(t) => {
                    top_cache.insert(k, t);
                }
                Err(e) => err = Some(e),
            })?;
            if let Some(e) = err {
                return Err(format!("relation: {e}"));
            }
            let mut next = vec![];
            for &k in &frontier {
                if let Some(t) = top_cache.get(&k) {
                    for (c, x) in t {
                        // a term whose coefficient is exactly zero at the key contributes nothing (as in epsdepth's
                        // composition): it is not followed
                        if !matches!(x, TCoeff::Zero) && seen.insert(*c) {
                            next.push(*c);
                        }
                    }
                }
            }
            frontier = next;
        }
        // post-order (children first)
        let mut post = vec![];
        let mut state: HashMap<usize, u8> = HashMap::new();
        let mut stack = vec![(root, false)];
        while let Some((k, done)) = stack.pop() {
            if done {
                post.push(k);
                continue;
            }
            if state.contains_key(&k) {
                continue;
            }
            state.insert(k, 1);
            stack.push((k, true));
            if let Some(t) = top_cache.get(&k) {
                for (c, x) in t {
                    if !matches!(x, TCoeff::Zero) && !state.contains_key(c) {
                        stack.push((*c, false));
                    }
                }
            }
        }
        if ri == 0 {
            log(&format!("cone of the key: {} keys", post.len()));
        }
        // sub-sector terms of each served cone key (from its record at its rule point; credits: not read)
        let mut credits = 0usize;
        for &k in &post {
            if !matches!(src.status(k), KeyStatus::Served) || sub_cache.contains_key(&k) {
                continue;
            }
            let kk = src.key(k);
            let res: Result<Vec<(Key, TCoeff)>, String> = (|| {
                let (t, r) = src.record_of(k).ok_or("served key without a record")?;
                #[allow(clippy::map_entry)]
                if !rec_cache.contains_key(&(t, r)) {
                    let info = set.rule(t, r);
                    let (line, _) = line_at(Path::new(&info.path), info.offset)?;
                    let rt = RuleText::parse(&line, &set.frame).map_err(|e| e.to_string())?;
                    let pats = rt
                        .rhs
                        .iter()
                        .map(|(p, _)| parse_rhs_pattern(p, arity))
                        .collect::<Result<Vec<_>, _>>()?;
                    rec_cache.insert((t, r), (rt, pats));
                }
                let (rt, pats) = &rec_cache[&(t, r)];
                let mut pt = vec![];
                let at_key = match &rt.domain {
                    Some(dm) => rule_point(dm, &kk, &mut pt),
                    None => {
                        let fixed: Option<Vec<i32>> = rt
                            .record
                            .fixed
                            .iter()
                            .map(|x| x.and_then(|v| i32::try_from(v).ok()))
                            .collect();
                        if fixed.as_deref() == Some(kk.as_slice()) {
                            pt = kk.clone();
                            true
                        } else {
                            false
                        }
                    }
                };
                if !at_key {
                    return Err("credit".to_string());
                }
                let mut out = vec![];
                for (j, pat) in pats.iter().enumerate() {
                    let tk: Option<Vec<i32>> = pat
                        .iter()
                        .map(|sl| match *sl {
                            RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                            RhsSlot::Const(c) => Some(c),
                        })
                        .collect();
                    let tk = tk.ok_or("term key out of range")?;
                    if !matches!(sectors.classify_masked(&tk, 0), TermClass::Subsector { .. }) {
                        continue;
                    }
                    let pc = coef_cache
                        .entry((t, r, j))
                        .or_insert_with(|| parse_coefficient(&alg2, &rt.rhs[j].1, arity));
                    let (num, den) = pc.as_ref().map_err(|e| format!("term {j}: {e}"))?;
                    let c = lau.coeff_at_key(num, den, arity, &pt)?;
                    if !matches!(c, TCoeff::Zero) {
                        out.push((tk, c));
                    }
                }
                Ok(out)
            })();
            if matches!(&res, Err(e) if e == "credit") {
                credits += 1;
            }
            sub_cache.insert(k, res);
        }
        if ri == 0 {
            log(&format!(
                "sub-sector terms read; credits (sub-sector terms not read): {credits}"
            ));
        }
        let edge_v = |c: &TCoeff| -> Option<i64> {
            match c {
                TCoeff::Ratio { v, .. } => Some(*v),
                TCoeff::Zero => None,
            }
        };
        // precision plan, bottom-up from the exact edge valuations
        let mut need: HashMap<usize, i64> = HashMap::new();
        for &k in &post {
            let p = match src.status(k) {
                KeyStatus::Terminal(_) => q_t,
                _ => {
                    let mut p = i64::MIN;
                    for (c, x) in top_cache.get(&k).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if let Some(v) = edge_v(x)
                            && need.get(c).copied().unwrap_or(i64::MIN) > i64::MIN
                        {
                            p = p.max(need[c].saturating_sub(v));
                        }
                    }
                    if let Some(Ok(sv)) = sub_cache.get(&k) {
                        for (_, x) in sv {
                            if let Some(v) = edge_v(x) {
                                p = p.max(q - v);
                            }
                        }
                    }
                    p
                }
            };
            need.insert(k, p);
        }
        let max_need = need
            .values()
            .copied()
            .filter(|&x| x > i64::MIN)
            .max()
            .unwrap_or(0);
        // top-down scalar weights (checkpointed)
        let big = i64::MAX / 8;
        let one = || {
            let _o = expand_t_ratio(
                &rustred_campaign_algebra::regbasis::series::ZP::new(
                    &symbolica::domains::integer::Z,
                    None,
                    tv.clone(),
                )
                .one(),
                &rustred_campaign_algebra::regbasis::series::ZP::new(
                    &symbolica::domains::integer::Z,
                    None,
                    tv.clone(),
                )
                .one(),
                4,
                &tv,
            );
            Laurent::normalised(0, QPc::new(&QF, None, tv.clone()).one(), big)
        };
        let expand = |x: &TCoeff, prec: i64| -> Laurent {
            match tcoeff_rf(x, &tv) {
                None => Laurent::zero(prec, &tv),
                Some(r) => expand_t_ratio(&r.numerator, &r.denominator, prec, &tv),
            }
        };
        let order: Vec<usize> = post.iter().rev().copied().collect();
        let mut w: HashMap<usize, Laurent> = HashMap::new();
        let mut wsub: HashMap<Key, Laurent> = HashMap::new();
        let mut on_t: Vec<Option<Laurent>> = vec![None; n];
        let mut start_at = 0usize;
        let ckfile = ckdir.as_ref().map(|d| {
            d.join(format!(
                "ck_{}.json",
                root_key
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join("_")
            ))
        });
        if let Some(f) = &ckfile
            && let Ok(t) = std::fs::read_to_string(f)
            && let Ok(v) = serde_json::from_str::<Value>(&t)
            && v["cone"].as_u64() == Some(order.len() as u64)
        {
            let pl = |x: &Value| -> Result<Laurent, String> {
                let st: rustred_campaign_core::regbasis::SeriesText =
                    serde_json::from_value(x.clone()).map_err(|e| e.to_string())?;
                rustred_campaign_algebra::regbasis::series::parse_series(&st, &tv)
            };
            start_at = v["next"].as_u64().unwrap_or(0) as usize;
            for e in v["w"].as_array().into_iter().flatten() {
                w.insert(e[0].as_u64().unwrap_or(0) as usize, pl(&e[1])?);
            }
            for e in v["wsub"].as_array().into_iter().flatten() {
                let k: Key = serde_json::from_value(e[0].clone()).map_err(|e| e.to_string())?;
                wsub.insert(k, pl(&e[1])?);
            }
            for e in v["on_t"].as_array().into_iter().flatten() {
                let j = e[0].as_u64().unwrap_or(0) as usize;
                on_t[j] = Some(pl(&e[1])?);
            }
            log(&format!(
                "resumed from checkpoint at {start_at} of {}",
                order.len()
            ));
        } else {
            w.insert(root, one());
        }
        let mut last_ck = Instant::now();
        let save = |next: usize,
                    w: &HashMap<usize, Laurent>,
                    wsub: &HashMap<Key, Laurent>,
                    on_t: &[Option<Laurent>]|
         -> Result<(), String> {
            let Some(f) = &ckfile else { return Ok(()) };
            let v = json!({"cone": order.len(), "next": next,
                "w": w.iter().map(|(k, x)| json!([k, x.to_text()])).collect::<Vec<_>>(),
                "wsub": wsub.iter().map(|(k, x)| json!([k, x.to_text()])).collect::<Vec<_>>(),
                "on_t": on_t.iter().enumerate().filter_map(|(j, x)| x.as_ref().map(|x| json!([j, x.to_text()]))).collect::<Vec<_>>()});
            let tmp = f.with_extension("tmp");
            std::fs::write(&tmp, serde_json::to_string(&v).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, f).map_err(|e| e.to_string())
        };
        for (i, &k) in order.iter().enumerate().skip(start_at) {
            if last_ck.elapsed().as_secs() > 300 {
                save(i, &w, &wsub, &on_t)?;
                last_ck = Instant::now();
                if ri == 0 {
                    log(&format!("checkpoint at {i} of {}", order.len()));
                }
            }
            let Some(wk) = w.remove(&k) else { continue };
            if let KeyStatus::Terminal(j) = src.status(k) {
                on_t[j] = Some(match on_t[j].take() {
                    None => wk,
                    Some(o) => o.add(&wk),
                });
                continue;
            }
            let vlow = wk.valuation().unwrap_or(wk.prec());
            for (c, x) in top_cache.get(&k).map(|v| v.as_slice()).unwrap_or(&[]) {
                if matches!(x, TCoeff::Zero) || need.get(c).copied().unwrap_or(i64::MIN) == i64::MIN
                {
                    continue; // a zero coefficient, or nothing below the child needs a weight
                }
                let e = expand(x, need[c].saturating_sub(vlow));
                let contrib = wk.mul(&e);
                let slot = w.entry(*c).or_insert_with(|| Laurent::zero(big, &tv));
                *slot = slot.add(&contrib);
            }
            if let Some(Ok(sv)) = sub_cache.get(&k) {
                for (s, x) in sv {
                    let e = expand(x, q.saturating_sub(vlow));
                    let contrib = wk.mul(&e);
                    let slot = wsub
                        .entry(s.clone())
                        .or_insert_with(|| Laurent::zero(big, &tv));
                    *slot = slot.add(&contrib);
                }
            }
        }
        save(order.len(), &w, &wsub, &on_t)?;
        // results
        let lead = |y: &Laurent| y.valuation().map(|v| rational_text(&y.coeff(v).unwrap()));
        let tpart: Vec<Value> = on_t
            .iter()
            .map(|y| match y {
                None => json!({"zero": true}),
                Some(y) => {
                    json!({"valuation": y.valuation(), "precision": y.prec(), "leading": lead(y)})
                }
            })
            .collect();
        let on_r = m_rec.as_ref().map(|(info, m)| {
            let mexp: Vec<Vec<Laurent>> = m
                .iter()
                .map(|r| r.iter().map(|e| rustred_campaign_algebra::regbasis::series::expand_rf(e, &symbolica::domains::rational::Rational::from(4), q_t + 2, &tv)).collect())
                .collect();
            let v: Vec<Value> = (0..n)
                .map(|jj| {
                    let mut acc = Laurent::zero(big, &tv);
                    for (ii, y) in on_t.iter().enumerate() {
                        if let Some(y) = y {
                            acc = acc.add(&y.mul(&mexp[ii][jj]));
                        }
                    }
                    json!({"valuation": acc.valuation(), "precision": acc.prec(), "leading": lead(&acc)})
                })
                .collect();
            json!({"record": info, "coefficients": v})
        });
        let mut rows = vec![];
        let (mut n_pole, mut n_reg, mut n_vanish, mut n_undec, mut maxp) =
            (0usize, 0usize, 0usize, 0usize, 0i64);
        let mut items: Vec<(&Key, &Laurent)> = wsub.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (sk, y) in items {
            let status = match y.valuation() {
                Some(v) if v < 0 => {
                    n_pole += 1;
                    maxp = maxp.max(-v);
                    format!("pole of order {}", -v)
                }
                Some(v) if v < y.prec() => {
                    n_reg += 1;
                    format!("regular (order {v})")
                }
                _ if y.prec() >= 1 => {
                    n_vanish += 1;
                    format!(
                        "regular, O(t^{}) (zero to the precision; leading coefficient beyond it)",
                        y.prec()
                    )
                }
                _ => {
                    n_undec += 1;
                    format!("undecided (precision {})", y.prec())
                }
            };
            let dropped: Vec<usize> = sk
                .iter()
                .zip(&chart)
                .enumerate()
                .filter(|(_, (x, l))| **l && **x <= 0)
                .map(|(i, _)| i)
                .collect();
            rows.push(json!({"key": sk, "dropped_line_slots": dropped, "status": status, "valuation": y.valuation(),
                "precision": y.prec(), "leading": lead(y)}));
        }
        // first-level sub-sector terms of the root's own relation, BEFORE composition
        let first_level: Vec<Value> = match sub_cache.get(&root) {
            Some(Ok(sv)) => sv
                .iter()
                .map(|(s, x)| json!({"key": s, "edge_order": edge_v(x)}))
                .collect(),
            Some(Err(e)) => vec![json!({"error": e})],
            None => vec![],
        };
        let summary = json!({"key": root_key, "cone_keys": order.len(), "credits_subsector_not_read": credits,
            "max_required_precision": max_need, "q": q,
            "on_T": tpart, "on_R": on_r,
            "first_level_subsector_terms_of_serving_relation": first_level,
            "subsector_W": {"keys": rows.len(), "with_pole": n_pole, "max_pole_order": maxp, "regular_nonzero_at_d0_or_order_below_q": n_reg,
                            "regular_vanishing_beyond_precision": n_vanish, "undecided": n_undec, "rows": rows},
            "seconds": tr0.elapsed().as_secs_f64()});
        log(&format!(
            "root {ri} {:?}: cone {} sub-sector keys {} with pole {} (max {}) undecided {} in {:.1} s",
            root_key,
            order.len(),
            wsub.len(),
            n_pole,
            maxp,
            n_undec,
            tr0.elapsed().as_secs_f64()
        ));
        results.push(SubweightResult { value: summary });
        if let Some(o) = get("--out") {
            let v = json!({"convention": "K = sum_T c_T T + sum_s W(s) s + (deeper sub-sector terms of the s: NOT followed); W(s) = coefficient of the sub-sector key s in K's reduction along the statement's top-sector cone (first generation); orders at d0 = 4 exact (decided by tracked precision)",
                "statement": stmt.to_string_lossy(), "results": results.iter().map(|r| r.value.clone()).collect::<Vec<_>>()});
            std::fs::write(
                &o,
                serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        if ri == 0 && get("--sample").is_some() && t0.elapsed().as_secs() > 3600 {
            log("the key took more than one hour: the sample is skipped");
            break;
        }
    }
    Ok(())
}

/// `rr_regbasis dispatch --import-spec S --keys-from SUBWEIGHTS.json [--root-index 0] [--index-dir I] [--out F]`: the set's
/// first-match dispatcher verdict at every sub-sector key listed for one root of a `subweights` output (is the key served
/// by any table of the set: the boundary of what this set can reduce further), with counts per verdict and per pole order.
fn dispatch_cmd(args: &[String]) -> Result<(), String> {
    use std::collections::BTreeMap;
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let spec_path = get("--import-spec").ok_or("missing --import-spec")?;
    let sw: Value = serde_json::from_str(
        &std::fs::read_to_string(get("--keys-from").ok_or("missing --keys-from")?)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let ri: usize = get("--root-index")
        .and_then(|x| x.parse().ok())
        .unwrap_or(0);
    let rows = sw["results"][ri]["subsector_W"]["rows"]
        .as_array()
        .ok_or("no rows")?
        .clone();
    let ts = TableSetSpec::read(Path::new(&spec_path)).map_err(|e| e.to_string())?;
    let opts = LoadOptions {
        index_dir: get("--index-dir").map(PathBuf::from),
    };
    let set = import::load_with(&ts, Path::new("/"), &opts).map_err(|e| e.to_string())?;
    let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut examples: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for r in &rows {
        let k: Key = serde_json::from_value(r["key"].clone()).map_err(|e| e.to_string())?;
        let polar = r["valuation"].as_i64().is_some_and(|v| v < 0);
        let verdict = match set.disp.first_match(&k) {
            Ok(rustred_campaign_core::cover::Dispatch::Covered(rr)) => {
                let info = set.rule(rr.table, rr.record);
                *by_source
                    .entry(format!("{} / {}", info.source, info.table_name))
                    .or_insert(0) += 1;
                "covered".to_string()
            }
            Ok(rustred_campaign_core::cover::Dispatch::Uncovered) => "uncovered".to_string(),
            Ok(other) => format!("{:?}", std::mem::discriminant(&other)),
            Err(e) => format!("outside the dispatcher's domain ({e})"),
        };
        let e = counts.entry(verdict.clone()).or_insert((0, 0));
        e.0 += 1;
        if polar {
            e.1 += 1;
        }
        let ex = examples.entry(verdict).or_default();
        if ex.len() < 5 {
            ex.push(r["key"].clone());
        }
    }
    let out = json!({"root": sw["results"][ri]["key"], "subsector_keys": rows.len(),
        "verdicts": counts.iter().map(|(k, (a, b))| json!({"verdict": k, "keys": a, "with_pole": b})).collect::<Vec<_>>(),
        "covered_by_source_table": by_source, "examples": examples});
    let text = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    match get("--out") {
        Some(o) => std::fs::write(o, &text).map_err(|e| e.to_string())?,
        None => println!("{text}"),
    }
    Ok(())
}

/// One relation of the input list: target key, top-sector rhs (key, coefficient text in d), own pole order if given.
struct RelIn {
    idx: usize,
    key: Key,
    top: Vec<(Key, String)>,
    sub_terms: usize,
}

fn read_relations(path: &Path, want: &dyn Fn(usize) -> bool) -> Result<Vec<RelIn>, String> {
    use std::io::BufRead;
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = vec![];
    for (i, line) in std::io::BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() || !want(i) {
            continue;
        }
        let v: Value =
            serde_json::from_str(&line).map_err(|e| format!("relations line {}: {e}", i + 1))?;
        let key: Key = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("relations line {}: key: {e}", i + 1))?;
        let mut top = vec![];
        let mut sub_terms = 0usize;
        for t in v["rhs"].as_array().ok_or("relations: rhs")? {
            let kind = t[2].as_str().unwrap_or("top");
            if kind != "top" {
                sub_terms += 1;
                continue;
            }
            let k = rustred_campaign_core::key::parse_key(t[0].as_str().ok_or("rhs key")?)
                .map_err(|e| e.to_string())?;
            top.push((k, t[1].as_str().ok_or("rhs coefficient")?.to_string()));
        }
        out.push(RelIn {
            idx: i,
            key,
            top,
            sub_terms,
        });
    }
    Ok(out)
}

/// `rr_regbasis package-relations --package DIR --import-spec SPEC --relations REL.jsonl --record R.json --out OUT.jsonl
/// [--loci F]... [--batch 16] [--keys-only] [--outside-by-table] [--sample-check N --seed S] [--max-depth 4000]
/// [--index-dir I]`:
/// per delivered relation `K = sum_j c_j K_j (+ sub-sector terms)`, its TOP-sector right-hand side composed EXACTLY in d
/// down to the package's terminals (lane epsdepth's composition, exact backend, over the package source), then
/// multiplied by M (T = M R) of the record: the relation's coefficients ON R, their exact orders at d0, the verdict
/// spurious (no pole on R) / inherent (a pole survives on R) / undecided (why). Checkpointed per relation (OUT.jsonl:
/// relations already there are skipped). `--keys-only`: the keys-only SUPERSET cone of each batch (every term followed)
/// and its open keys, no composition.
fn package_relations_cmd(args: &[String]) -> Result<(), String> {
    use rustred_campaign_algebra::epsdepth::TRational;
    use rustred_campaign_algebra::regbasis::adapter::rf_t_to_d;
    use rustred_campaign_algebra::regbasis::exact::row_times;
    use rustred_campaign_algebra::regbasis::package::PackageSource;
    use rustred_campaign_algebra::regbasis::series::{RF, parse_rf, rf_text, rf_valuation};
    use std::collections::{HashMap, HashSet};
    use std::io::Write as _;
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let getall = |name: &str| -> Vec<String> {
        args.iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == name)
            .filter_map(|(i, _)| args.get(i + 1).cloned())
            .collect()
    };
    let flag = |name: &str| args.iter().any(|a| a == name);
    if !licensed() {
        return Err("no Symbolica licence (source WS/ops/env.sh)".into());
    }
    let t0 = Instant::now();
    let log = |m: &str| eprintln!("[{:>9.1}s] {m}", t0.elapsed().as_secs_f64());
    let pkg = PathBuf::from(get("--package").ok_or("missing --package")?);
    let spec_path = get("--import-spec").ok_or("missing --import-spec")?;
    let rel_path = PathBuf::from(get("--relations").ok_or("missing --relations")?);
    let out_path = PathBuf::from(get("--out").ok_or("missing --out")?);
    let batch: usize = get("--batch").and_then(|x| x.parse().ok()).unwrap_or(16);
    let max_depth: i64 = get("--max-depth")
        .and_then(|x| x.parse().ok())
        .unwrap_or(4000);
    let keys_only = flag("--keys-only");
    let rec: Value = serde_json::from_str(
        &std::fs::read_to_string(get("--record").ok_or("missing --record")?)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let d0t = rec["binding"]["d0"].as_str().unwrap_or("4").to_string();
    let d0 = rustred_campaign_algebra::valuation::D0::parse(&d0t).map_err(|e| e.to_string())?;
    let m_texts: Vec<Vec<String>> =
        serde_json::from_value(rec["M"].clone()).map_err(|e| format!("record M: {e}"))?;
    let m: Vec<Vec<RF>> = m_texts
        .iter()
        .map(|r| r.iter().map(|t| parse_rf(t)).collect::<Result<Vec<_>, _>>())
        .collect::<Result<_, _>>()?;
    let rec_terminals: Vec<Key> =
        serde_json::from_value(rec["terminals"].clone()).map_err(|e| e.to_string())?;
    // strict import of the package table
    let ts = TableSetSpec::read(Path::new(&spec_path)).map_err(|e| e.to_string())?;
    let opts = LoadOptions {
        index_dir: get("--index-dir").map(PathBuf::from),
    };
    let set = import::load_with(&ts, Path::new("/"), &opts).map_err(|e| e.to_string())?;
    log(&format!(
        "strict import of the package table: {} table(s), {} records",
        set.rules.len(),
        set.rules.iter().map(|t| t.len()).sum::<usize>()
    ));
    let mut segs: Vec<PathBuf> = vec![];
    for e in std::fs::read_dir(pkg.join("relations")).map_err(|e| e.to_string())? {
        let d = e.map_err(|e| e.to_string())?.path();
        if d.is_dir() {
            for f in std::fs::read_dir(&d).map_err(|e| e.to_string())? {
                let f = f.map_err(|e| e.to_string())?.path();
                if f.file_name()
                    .and_then(|x| x.to_str())
                    .is_some_and(|n| n.starts_with("seg-") && n.ends_with(".jsonl"))
                {
                    segs.push(f);
                }
            }
        }
    }
    segs.sort();
    let loci: Vec<PathBuf> = getall("--loci").into_iter().map(PathBuf::from).collect();
    let alg = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let lau = SymbolicaLaurent::new(&d0t)?;
    let mut src = PackageSource::open(
        &set,
        &pkg.join("keys.jsonl"),
        &segs,
        &loci,
        alg,
        lau.clone(),
    )?;
    if flag("--outside-by-table") {
        src.serve_outside_by_table(true);
        log(
            "keys outside the certified list: served by the table's first match where it covers them (--outside-by-table)",
        );
    }
    log(&format!(
        "package: {} certified keys, {} entries, {} segment files, terminals {:?}",
        src.stats.certified_keys,
        src.stats.entries,
        segs.len(),
        src.terminals()
    ));
    if src.terminals() != rec_terminals.as_slice() {
        return Err(format!(
            "package terminals {:?} differ from the record's {:?}",
            src.terminals(),
            rec_terminals
        ));
    }
    let n = src.terminals().len();
    for t in src.terminals().to_vec() {
        src.intern(&t);
    }
    // relations to do (checkpoint: skip those already in OUT)
    let mut done: HashSet<usize> = HashSet::new();
    if let Ok(t) = std::fs::read_to_string(&out_path) {
        for l in t.lines() {
            if let Ok(v) = serde_json::from_str::<Value>(l)
                && let Some(i) = v["idx"].as_u64()
            {
                done.insert(i as usize);
            }
        }
    }
    let only: Option<HashSet<usize>> =
        get("--only").map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    let want = |i: usize| !done.contains(&i) && only.as_ref().is_none_or(|o| o.contains(&i));
    let rels = read_relations(&rel_path, &want)?;
    log(&format!(
        "relations to do: {} (already done: {})",
        rels.len(),
        done.len()
    ));
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
        .map_err(|e| e.to_string())?;
    let params = ComposeParams {
        target: 1,
        max_depth,
        keep_coefficients: false,
        progress: flag("--progress"),
        ..ComposeParams::default()
    };
    let mut ex = SymbolicaExact::new()?;
    // resumable vector cache: one line per composed rhs key {key, vec | err}
    let vcache_path = PathBuf::from(
        get("--vec-cache").unwrap_or_else(|| format!("{}.vec-cache.jsonl", out_path.display())),
    );
    let mut vcache: HashMap<Key, Result<Vec<Option<String>>, String>> = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(&vcache_path) {
        for l in t.lines() {
            if let Ok(v) = serde_json::from_str::<Value>(l)
                && let Ok(k) = serde_json::from_value::<Key>(v["key"].clone())
            {
                let e = match v["err"].as_str() {
                    Some(er) => Err(er.to_string()),
                    None => serde_json::from_value::<Vec<Option<String>>>(v["vec"].clone())
                        .map_err(|e| e.to_string()),
                };
                vcache.insert(k, e);
            }
        }
        log(&format!("vector cache: {} rhs keys loaded", vcache.len()));
    }
    // SECOND ROUTE (--backend laurent): truncated Laurent series (lane epsdepth's SymbolicaLaurent, Symbolica Series) with
    // tracked absolute precision; the target precision is raised (x2) until every order on R is decided.
    if get("--backend").as_deref() == Some("laurent") {
        use rustred_campaign_algebra::epsdepth::QSeries;
        use rustred_campaign_algebra::regbasis::series::{Laurent, QP, expand_rf, t_vars};
        use symbolica::domains::rational::Q as QF2;
        let tv = t_vars();
        let to_laurent = |s: &QSeries| -> Laurent {
            let (lo, prec) = SymbolicaLaurent::series_order(s);
            match lo {
                None => Laurent::zero(prec, &tv),
                Some(lo) => {
                    let mut p = QP::new(&QF2, None, tv.clone());
                    for e in lo..prec {
                        if let Some(c) = SymbolicaLaurent::series_coefficient(s, e)
                            && !c.is_zero()
                        {
                            p.append_monomial(c, &[(e - lo) as u16]);
                        }
                    }
                    Laurent::normalised(lo, p, prec)
                }
            }
        };
        let mut target: i64 = get("--target").and_then(|x| x.parse().ok()).unwrap_or(8);
        let mut pending: Vec<&RelIn> = rels.iter().collect();
        let mut lau2 = lau.clone();
        while !pending.is_empty() && target <= 1024 {
            let tb = Instant::now();
            let mut uniq: Vec<Key> = pending
                .iter()
                .flat_map(|r| r.top.iter().map(|(k, _)| k.clone()))
                .collect();
            uniq.sort();
            uniq.dedup();
            let sel: Vec<usize> = uniq.iter().map(|k| src.intern(k)).collect();
            let p2 = ComposeParams {
                target,
                max_depth,
                keep_coefficients: false,
                ..ComposeParams::default()
            };
            let mut vecs: HashMap<usize, Result<Vec<Option<Laurent>>, String>> = HashMap::new();
            compose_streaming(
                &mut src,
                &mut lau2,
                &sel,
                n,
                &p2,
                Some(&mut |g: usize, r: Result<&[Option<QSeries>], &str>| {
                    vecs.insert(
                        g,
                        r.map(|v| v.iter().map(|c| c.as_ref().map(&to_laurent)).collect())
                            .map_err(|e| e.to_string()),
                    );
                }),
            )?;
            let big = i64::MAX / 8;
            let mexp: Vec<Vec<Laurent>> = m
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|e| expand_rf(e, d0.value(), target + 64, &tv))
                        .collect()
                })
                .collect();
            let mut next = vec![];
            for r in pending {
                let mut acc: Vec<Laurent> = (0..n).map(|_| Laurent::zero(big, &tv)).collect();
                let mut why = None;
                for (k, ct) in &r.top {
                    let g = src.index_of(k).expect("interned");
                    match vecs.get(&g) {
                        Some(Ok(v)) => {
                            let c = expand_rf(&parse_rf(ct)?, d0.value(), target + 64, &tv);
                            for (j, x) in v.iter().enumerate() {
                                if let Some(x) = x {
                                    acc[j] = acc[j].add(&c.mul(x));
                                }
                            }
                        }
                        Some(Err(e)) => {
                            why = Some(format!("rhs key {k:?}: {e}"));
                            break;
                        }
                        None => {
                            why = Some(format!("rhs key {k:?}: not handed over"));
                            break;
                        }
                    }
                }
                if let Some(w) = why {
                    writeln!(out, "{}", json!({"idx": r.idx, "key": r.key, "route": "laurent", "verdict": "undecided", "why": w})).map_err(|e| e.to_string())?;
                    continue;
                }
                let on_r: Vec<Laurent> = (0..n)
                    .map(|jj| {
                        let mut a = Laurent::zero(big, &tv);
                        for (ii, x) in acc.iter().enumerate() {
                            a = a.add(&x.mul(&mexp[ii][jj]));
                        }
                        a
                    })
                    .collect();
                if on_r
                    .iter()
                    .all(|x| x.decides_pole() && (x.valuation().is_some() || x.prec() >= 1))
                {
                    let ord: Vec<Value> = on_r
                        .iter()
                        .map(|x| match x.valuation() {
                            Some(v) => json!(v),
                            None => json!(format!("O(t^{})", x.prec())),
                        })
                        .collect();
                    let pole = on_r
                        .iter()
                        .filter_map(|x| x.valuation())
                        .map(|v| (-v).max(0))
                        .max()
                        .unwrap_or(0);
                    writeln!(out, "{}", json!({"idx": r.idx, "key": r.key, "route": "laurent", "target": target,
                        "verdict": if pole == 0 { "spurious" } else { "inherent" }, "pole_order_on_R": pole, "orders_on_R": ord,
                        "precision_on_R": on_r.iter().map(|x| x.prec()).collect::<Vec<_>>()})).map_err(|e| e.to_string())?;
                } else {
                    next.push(r);
                }
            }
            out.flush().map_err(|e| e.to_string())?;
            log(&format!(
                "laurent route: target {target}: {} decided, {} raised, {:.1} s",
                rels.len() - next.len(),
                next.len(),
                tb.elapsed().as_secs_f64()
            ));
            pending = next;
            target *= 2;
        }
        for r in pending {
            writeln!(out, "{}", json!({"idx": r.idx, "key": r.key, "route": "laurent", "verdict": "undecided", "why": "precision cap 1024 reached"})).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    for (bi, chunk) in rels.chunks(batch.max(1)).enumerate() {
        let tb = Instant::now();
        let mut uniq: Vec<Key> = chunk
            .iter()
            .flat_map(|r| r.top.iter().map(|(k, _)| k.clone()))
            .collect();
        uniq.sort();
        uniq.dedup();
        if keys_only {
            let (cone, open, height) = src.cone_keys_only(&uniq, usize::MAX)?;
            for r in chunk {
                let line = json!({"idx": r.idx, "key": r.key, "mode": "keys_only_superset", "batch": bi, "batch_rhs_keys": uniq.len(),
                    "batch_cone_keys": cone, "batch_height": height, "batch_open": open.len(),
                    "batch_open_examples": open.iter().take(5).map(|(k, e)| json!({"key": k, "why": e})).collect::<Vec<_>>()});
                writeln!(out, "{line}").map_err(|e| e.to_string())?;
            }
            log(&format!(
                "batch {bi}: {} relations, rhs keys {}, keys-only superset cone {cone}, height {height}, open {} ({:.1} s)",
                chunk.len(),
                uniq.len(),
                open.len(),
                tb.elapsed().as_secs_f64()
            ));
            continue;
        }
        // composed vectors of the rhs keys: from the cache (resume), else composed now and appended to the cache
        let todo: Vec<Key> = uniq
            .iter()
            .filter(|k| !vcache.contains_key(*k))
            .cloned()
            .collect();
        let sel: Vec<usize> = todo.iter().map(|k| src.intern(k)).collect();
        let gk: HashMap<usize, Key> = sel.iter().copied().zip(todo.iter().cloned()).collect();
        let key_of = |g: usize| -> Key { gk.get(&g).cloned().unwrap_or_default() };
        let mut comp_stats = None;
        if !sel.is_empty() {
            let mut vc = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&vcache_path)
                .map_err(|e| e.to_string())?;
            #[allow(clippy::type_complexity)]
            let mut newv: Vec<(usize, Result<Vec<Option<String>>, String>)> = vec![];
            let mut sink_err: Option<String> = None;
            let comp = compose_streaming(
                &mut src,
                &mut ex,
                &sel,
                n,
                &params,
                Some(&mut |g: usize, r: Result<&[Option<TRational>], &str>| {
                    let v: Result<Vec<Option<String>>, String> = r
                        .map(|v| {
                            v.iter()
                                .map(|c| c.as_ref().map(|x| rf_text(&rf_t_to_d(x, d0.value()))))
                                .collect()
                        })
                        .map_err(|e| e.to_string());
                    newv.push((g, v.clone()));
                    if let Err(e) = writeln!(
                        vc,
                        "{}",
                        json!({"key": key_of(g), "vec": v.as_ref().ok(), "err": v.as_ref().err()})
                    ) {
                        sink_err = Some(e.to_string());
                    }
                }),
            )?;
            if let Some(e) = sink_err {
                return Err(e);
            }
            comp_stats = Some(comp.stats);
            for (g, v) in newv {
                vcache.insert(key_of(g), v);
            }
        }
        for r in chunk {
            let mut acc: Vec<RF> = (0..n).map(|_| parse_rf("0").expect("zero")).collect();
            let mut why: Option<String> = None;
            for (k, ct) in &r.top {
                match vcache.get(k) {
                    Some(Ok(v)) => {
                        let c = parse_rf(ct)?;
                        for (j, comp_j) in v.iter().enumerate() {
                            if let Some(x) = comp_j {
                                let xd = parse_rf(x)?;
                                acc[j] = &acc[j] + &(&c * &xd);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        why = Some(format!("rhs key {k:?}: {e}"));
                        break;
                    }
                    None => {
                        why = Some(format!("rhs key {k:?}: not handed over by the composition"));
                        break;
                    }
                }
            }
            let line = match why {
                Some(w) => {
                    json!({"idx": r.idx, "key": r.key, "verdict": "undecided", "why": w, "top_terms": r.top.len(), "sub_terms_not_composed": r.sub_terms})
                }
                None => {
                    let on_r = row_times(&acc, &m);
                    let ord_t: Vec<Option<i64>> =
                        acc.iter().map(|x| rf_valuation(x, d0.value())).collect();
                    let ord_r: Vec<Option<i64>> =
                        on_r.iter().map(|x| rf_valuation(x, d0.value())).collect();
                    let pole = ord_r
                        .iter()
                        .filter_map(|o| *o)
                        .map(|v| (-v).max(0))
                        .max()
                        .unwrap_or(0);
                    json!({"idx": r.idx, "key": r.key, "verdict": if pole == 0 { "spurious" } else { "inherent" },
                        "pole_order_on_R": pole, "orders_on_R": ord_r, "orders_on_T": ord_t,
                        "coefficients_on_R": on_r.iter().map(rf_text).collect::<Vec<_>>(),
                        "top_terms": r.top.len(), "sub_terms_not_composed": r.sub_terms})
                }
            };
            writeln!(out, "{line}").map_err(|e| e.to_string())?;
        }
        out.flush().map_err(|e| e.to_string())?;
        // the exact open set of this batch's cones (keys outside the certified set met with a nonzero coefficient)
        let open_keys = src.take_open_keys();
        if !open_keys.is_empty() {
            let op = PathBuf::from(format!("{}.open-keys.jsonl", out_path.display()));
            let mut of = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&op)
                .map_err(|e| e.to_string())?;
            for k in &open_keys {
                writeln!(of, "{}", json!({"batch": bi, "key": k})).map_err(|e| e.to_string())?;
            }
            log(&format!(
                "batch {bi}: {} open keys (exact cone) -> {}",
                open_keys.len(),
                op.display()
            ));
        }
        log(&format!(
            "batch {bi}: {} relations, rhs keys {} ({} composed now), cone {:?} keys, max height {:?}, {:.1} s; source {}",
            chunk.len(),
            uniq.len(),
            todo.len(),
            comp_stats.as_ref().map(|s| s.cone_keys),
            comp_stats.as_ref().map(|s| s.max_height),
            tb.elapsed().as_secs_f64(),
            serde_json::to_string(&src.stats).unwrap_or_default()
        ));
    }
    Ok(())
}

/// `rr_regbasis package-closure --package DIR --import-spec SPEC --record R.json --out OUT.jsonl [--loci F]...
/// [--keys box|all] [--chunk 200000] [--index-dir I]`: certificate c2 over a PACKAGE's certified keys: every selected key
/// composed EXACTLY in d onto the package's terminals (lane epsdepth's composition, exact backend, over the package
/// source) and multiplied by M of the record: its orders on T and on R at d0. OUT.jsonl gets one line per key with a pole
/// on R (and every key's orders when --all-lines); a summary line per chunk (resumable: chunks already summarised are
/// skipped). Modulo sub-sectors (top-sector composition), like every statement of this lane.
fn package_closure_cmd(args: &[String]) -> Result<(), String> {
    use rustred_campaign_algebra::epsdepth::TRational;
    use rustred_campaign_algebra::regbasis::adapter::rf_t_to_d;
    use rustred_campaign_algebra::regbasis::exact::row_times;
    use rustred_campaign_algebra::regbasis::package::{PackageSource, Serv};
    use rustred_campaign_algebra::regbasis::series::{RF, parse_rf, rf_valuation};
    use std::collections::{BTreeMap, HashSet};
    use std::io::Write as _;
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let getall = |name: &str| -> Vec<String> {
        args.iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == name)
            .filter_map(|(i, _)| args.get(i + 1).cloned())
            .collect()
    };
    if !licensed() {
        return Err("no Symbolica licence (source WS/ops/env.sh)".into());
    }
    let t0 = Instant::now();
    let log = |m: &str| eprintln!("[{:>9.1}s] {m}", t0.elapsed().as_secs_f64());
    let pkg = PathBuf::from(get("--package").ok_or("missing --package")?);
    let spec_path = get("--import-spec").ok_or("missing --import-spec")?;
    let out_path = PathBuf::from(get("--out").ok_or("missing --out")?);
    let which = get("--keys").unwrap_or_else(|| "all".into());
    let chunk: usize = get("--chunk")
        .and_then(|x| x.parse().ok())
        .unwrap_or(200_000);
    let all_lines = args.iter().any(|a| a == "--all-lines");
    let rec: Value = serde_json::from_str(
        &std::fs::read_to_string(get("--record").ok_or("missing --record")?)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let d0t = rec["binding"]["d0"].as_str().unwrap_or("4").to_string();
    let d0 = rustred_campaign_algebra::valuation::D0::parse(&d0t).map_err(|e| e.to_string())?;
    let m_texts: Vec<Vec<String>> =
        serde_json::from_value(rec["M"].clone()).map_err(|e| format!("record M: {e}"))?;
    let m: Vec<Vec<RF>> = m_texts
        .iter()
        .map(|r| r.iter().map(|t| parse_rf(t)).collect::<Result<Vec<_>, _>>())
        .collect::<Result<_, _>>()?;
    let ts = TableSetSpec::read(Path::new(&spec_path)).map_err(|e| e.to_string())?;
    let opts = LoadOptions {
        index_dir: get("--index-dir").map(PathBuf::from),
    };
    let set = import::load_with(&ts, Path::new("/"), &opts).map_err(|e| e.to_string())?;
    let mut segs: Vec<PathBuf> = vec![];
    for e in std::fs::read_dir(pkg.join("relations")).map_err(|e| e.to_string())? {
        let d = e.map_err(|e| e.to_string())?.path();
        if d.is_dir() {
            for f in std::fs::read_dir(&d).map_err(|e| e.to_string())? {
                let f = f.map_err(|e| e.to_string())?.path();
                if f.file_name()
                    .and_then(|x| x.to_str())
                    .is_some_and(|n| n.starts_with("seg-") && n.ends_with(".jsonl"))
                {
                    segs.push(f);
                }
            }
        }
    }
    segs.sort();
    let loci: Vec<PathBuf> = getall("--loci").into_iter().map(PathBuf::from).collect();
    let alg = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let lau = SymbolicaLaurent::new(&d0t)?;
    let mut src = PackageSource::open(&set, &pkg.join("keys.jsonl"), &segs, &loci, alg, lau)?;
    if args.iter().any(|a| a == "--outside-by-table") {
        src.serve_outside_by_table(true);
    }
    let n = src.terminals().len();
    for t in src.terminals().to_vec() {
        src.intern(&t);
    }
    // selected keys: all certified (non-terminal) keys, or the in-box ones, in the file order
    let mut keys: Vec<Key> = vec![];
    {
        use std::io::BufRead;
        let f = std::fs::File::open(pkg.join("keys.jsonl")).map_err(|e| e.to_string())?;
        for line in std::io::BufReader::new(f).lines() {
            let line = line.map_err(|e| e.to_string())?;
            let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if which == "box" && v[3].as_bool() != Some(true) {
                continue;
            }
            let k = rustred_campaign_core::key::parse_key(v[0].as_str().unwrap_or(""))
                .map_err(|e| e.to_string())?;
            if !matches!(src.serv(&k), Some(Serv::Terminal(_))) {
                keys.push(k);
            }
        }
    }
    let mut done: HashSet<usize> = HashSet::new();
    if let Ok(t) = std::fs::read_to_string(&out_path) {
        for l in t.lines() {
            if let Ok(v) = serde_json::from_str::<Value>(l)
                && v["summary_chunk"].as_u64().is_some()
            {
                done.insert(v["summary_chunk"].as_u64().unwrap_or(0) as usize);
            }
        }
    }
    log(&format!(
        "{} keys selected ({which}), chunks of {chunk}, {} chunks already done",
        keys.len(),
        done.len()
    ));
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
        .map_err(|e| e.to_string())?;
    let params = ComposeParams {
        target: 1,
        max_depth: 1_000_000,
        keep_coefficients: false,
        ..ComposeParams::default()
    };
    let mut ex = SymbolicaExact::new()?;
    let (mut tot, mut tot_pole_r, mut tot_pole_t, mut tot_und) = (0usize, 0usize, 0usize, 0usize);
    for (ci, ch) in keys.chunks(chunk.max(1)).enumerate() {
        if done.contains(&ci) {
            continue;
        }
        let tc = Instant::now();
        let sel: Vec<usize> = ch.iter().map(|k| src.intern(k)).collect();
        let gk: std::collections::HashMap<usize, Key> =
            sel.iter().copied().zip(ch.iter().cloned()).collect();
        let (mut cnt, mut pole_r, mut pole_t, mut und) = (0usize, 0usize, 0usize, 0usize);
        let mut hist: BTreeMap<String, usize> = BTreeMap::new();
        let mut lines: Vec<String> = vec![];
        let comp = compose_streaming(
            &mut src,
            &mut ex,
            &sel,
            n,
            &params,
            Some(&mut |g: usize, r: Result<&[Option<TRational>], &str>| {
                cnt += 1;
                let key = gk.get(&g).cloned().unwrap_or_default();
                match r {
                    Err(e) => {
                        und += 1;
                        if lines.len() < 100_000 {
                            lines.push(json!({"key": key, "undecided": e}).to_string());
                        }
                    }
                    Ok(v) => {
                        let vt: Vec<RF> = v
                            .iter()
                            .map(|c| match c {
                                Some(x) => rf_t_to_d(x, d0.value()),
                                None => parse_rf("0").expect("zero"),
                            })
                            .collect();
                        let on_r = row_times(&vt, &m);
                        let ot: Vec<Option<i64>> =
                            vt.iter().map(|x| rf_valuation(x, d0.value())).collect();
                        let or: Vec<Option<i64>> =
                            on_r.iter().map(|x| rf_valuation(x, d0.value())).collect();
                        let pt = ot
                            .iter()
                            .filter_map(|o| *o)
                            .map(|v| (-v).max(0))
                            .max()
                            .unwrap_or(0);
                        let pr = or
                            .iter()
                            .filter_map(|o| *o)
                            .map(|v| (-v).max(0))
                            .max()
                            .unwrap_or(0);
                        if pt > 0 {
                            pole_t += 1;
                        }
                        *hist.entry(format!("T{pt}")).or_insert(0) += 1;
                        if pr > 0 {
                            pole_r += 1;
                            lines.push(json!({"key": key, "pole_on_R": pr, "orders_on_R": or, "orders_on_T": ot}).to_string());
                        } else if all_lines {
                            lines.push(
                                json!({"key": key, "orders_on_R": or, "orders_on_T": ot})
                                    .to_string(),
                            );
                        }
                    }
                }
            }),
        )?;
        for l in &lines {
            writeln!(out, "{l}").map_err(|e| e.to_string())?;
        }
        writeln!(out, "{}", json!({"summary_chunk": ci, "keys": ch.len(), "handed_over": cnt, "with_pole_on_R": pole_r,
            "with_pole_on_T": pole_t, "undecided": und, "pole_order_on_T_histogram": hist, "cone_keys": comp.stats.cone_keys,
            "max_height": comp.stats.max_height, "seconds": tc.elapsed().as_secs_f64()})).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        tot += ch.len();
        tot_pole_r += pole_r;
        tot_pole_t += pole_t;
        tot_und += und;
        log(&format!(
            "chunk {ci}: {} keys, pole on R {pole_r}, pole on T {pole_t}, undecided {und}, cone {}, height {}, {:.1} s",
            ch.len(),
            comp.stats.cone_keys,
            comp.stats.max_height,
            tc.elapsed().as_secs_f64()
        ));
    }
    log(&format!(
        "DONE this run: {tot} keys, with pole on R {tot_pole_r}, with pole on T {tot_pole_t}, undecided {tot_und}; source {}",
        serde_json::to_string(&src.stats).unwrap_or_default()
    ));
    Ok(())
}

/// `rr_regbasis package-spec --package DIR --out-dir D [--node NAME]`: from a node service package (SUMMARY.json:
/// `table`, `table_sha256`; keys.jsonl: in-box flag in column 4) write D/import-spec.json (one `engine_table` source over
/// the package table, strict policies: elided witness loci BLOCK, witness check on, tier K, evidence `imported`) and
/// D/start-box-keys.jsonl (the in-box keys: the start set). Prints both paths.
fn package_spec_cmd(args: &[String]) -> Result<(), String> {
    use std::io::{BufRead, Write as _};
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let pkg = PathBuf::from(get("--package").ok_or("missing --package")?);
    let od = PathBuf::from(get("--out-dir").ok_or("missing --out-dir")?);
    std::fs::create_dir_all(&od).map_err(|e| e.to_string())?;
    let summary: Value = serde_json::from_str(
        &std::fs::read_to_string(pkg.join("SUMMARY.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    // the table of THIS package directory (SUMMARY may name the build path of an earlier copy): the *.jsonl table whose
    // first line is a table_dump header
    let mut table: Option<PathBuf> = None;
    for e in std::fs::read_dir(&pkg).map_err(|e| e.to_string())? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.extension().is_some_and(|x| x == "jsonl") {
            let mut first = String::new();
            std::io::BufReader::new(std::fs::File::open(&p).map_err(|e| e.to_string())?)
                .read_line(&mut first)
                .map_err(|e| e.to_string())?;
            if first.contains("\"table_dump.v1\"") {
                table = Some(p);
            }
        }
    }
    let table = table.ok_or("no table_dump.v1 table in the package directory")?;
    // the table digest: SUMMARY.json `table_sha256`, else its `files.<table name>.sha256`, else computed here (streaming)
    let tname = table
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_string();
    let sha = match summary["table_sha256"]
        .as_str()
        .or_else(|| summary["files"][tname.as_str()]["sha256"].as_str())
    {
        Some(s) => s.to_string(),
        None => {
            use std::io::Read as _;
            let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
            let mut f = std::fs::File::open(&table).map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 1 << 22];
            loop {
                let k = f.read(&mut buf).map_err(|e| e.to_string())?;
                if k == 0 {
                    break;
                }
                h.update(&buf[..k]);
            }
            h.finish_hex()
        }
    };
    let node = get("--node")
        .or_else(|| summary["node"].as_str().map(str::to_string))
        .unwrap_or_else(|| "node".into());
    let start = od.join("start-box-keys.jsonl");
    let mut w = std::fs::File::create(&start).map_err(|e| e.to_string())?;
    let mut nbox = 0usize;
    for line in std::io::BufReader::new(
        std::fs::File::open(pkg.join("keys.jsonl")).map_err(|e| e.to_string())?,
    )
    .lines()
    {
        let line = line.map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if v[3].as_bool() == Some(true) {
            writeln!(w, "{}", v[0]).map_err(|e| e.to_string())?;
            nbox += 1;
        }
    }
    let spec = json!({"schema": "rustred.campaign.import-spec.v1", "node": node, "frame_from": "pkg",
        "start": {"path": start.to_string_lossy(), "format": "keys_jsonl"},
        "required_evidence": "imported",
        "policies": {"elided": "block", "witness_check": "on", "tier": "K"},
        "sources": [{"id": "pkg", "kind": "engine_table", "class": "package", "hand_name": {"template": "pkg#{label}"},
                     "path": table.to_string_lossy(), "sha256": sha, "removed": []}]});
    let sp = od.join("import-spec.json");
    std::fs::write(
        &sp,
        serde_json::to_string_pretty(&spec).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{}\n{} ({nbox} in-box keys)", sp.display(), start.display());
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("package-spec") {
        return package_spec_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("package-closure") {
        return package_closure_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("package-relations") {
        return package_relations_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("dispatch") {
        return dispatch_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("subweights") {
        return subweights_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("subsector") {
        return subsector_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("key") {
        return key_cmd(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("need-from-map") {
        return need_cmd(&args[1..]);
    }
    let get = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let flag = |name: &str| args.iter().any(|a| a == name);
    let need = |name: &str| get(name).ok_or_else(|| format!("missing {name}"));
    if !licensed() {
        return Err("no Symbolica licence (source WS/ops/env.sh)".into());
    }
    let t0 = Instant::now();
    let spec_path = need("--import-spec")?;
    let stmt = PathBuf::from(need("--statement-dir")?);
    let out = PathBuf::from(need("--out")?);
    let spec = match get("--spec") {
        None => RegBasisSpec::default(),
        Some(s) => RegBasisSpec::from_value(&text_or_file(&s)?)?,
    };
    let node = get("--node").unwrap_or_else(|| "node".into());
    let base = PathBuf::from(get("--import-base").unwrap_or_else(|| "/".into()));
    let ts = TableSetSpec::read(Path::new(&spec_path)).map_err(|e| e.to_string())?;
    let opts = LoadOptions {
        index_dir: get("--index-dir").map(PathBuf::from),
    };
    let set = import::load_with(&ts, &base, &opts).map_err(|e| e.to_string())?;
    eprintln!(
        "loaded set in {:.1} s, rss {} MB",
        t0.elapsed().as_secs_f64(),
        rss_mb()
    );
    let alg = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let lau = SymbolicaLaurent::new(&spec.d0)?;
    let cache = get("--cache-mb")
        .and_then(|x| x.parse::<usize>().ok())
        .unwrap_or(4096)
        << 20;
    let mut src = RecordSource::from_statement(&set, &stmt, alg, lau.clone(), cache)?;
    let chart: Vec<bool> = src.lines().to_vec();
    let terminals: Vec<Key> = src.terminals().to_vec();
    let band: Vec<Key> = src
        .box_keys()
        .into_iter()
        .map(|i| src.key(i))
        .filter(|k| rank_of(&chart, k).is_some())
        .collect();
    eprintln!(
        "terminals {:?}; band (top-sector start keys) {}",
        terminals,
        band.len()
    );
    let (bx_id, bx_digest, r_max) = match get("--box") {
        Some(p) => {
            let b = BoxDef::from_json(&std::fs::read_to_string(&p).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            let r = b.layers.iter().map(|l| l[1]).max();
            (b.id.clone(), b.digest(), r)
        }
        None => {
            let mut s = band.clone();
            s.sort();
            let text = serde_json::to_string(&s).map_err(|e| e.to_string())?;
            (
                "statement-start-keys".to_string(),
                blake3_hex(text.as_bytes()),
                None,
            )
        }
    };
    let stmt_digest = file_sha256(&stmt.join("statement.jsonl"))?;
    let identity = get("--identity-digest").unwrap_or_else(|| {
        let order = ts.order.as_ref().map(|o| o.to_string()).unwrap_or_default();
        blake3_hex(format!("{}|{}|{order}", set.frame.mask_string(), set.cmp.name()).as_bytes())
    });
    let binding = Binding {
        node_id: node.clone(),
        node_identity_digest: identity,
        box_id: bx_id,
        box_digest: bx_digest,
        closure_statement_digest: stmt_digest,
        d0: rustred_campaign_algebra::valuation::D0::parse(&spec.d0)
            .map_err(|e| e.to_string())?
            .text()
            .to_string(),
    };
    let published = match get("--published") {
        Some(p) => {
            let map =
                rustred_campaign_algebra::valuation::basis::read_master_map(&text_or_file(&p)?)?;
            Some(Published::from_master_map(&map, &terminals)?)
        }
        None => None,
    };
    let avail = match get("--eps-box") {
        Some(s) => Some(EpsAvail(EpsBox::from_value(&text_or_file(&s)?)?)),
        None => None,
    };
    let params = ComposeParams {
        target: 1,
        max_depth: get("--max-depth")
            .and_then(|x| x.parse().ok())
            .unwrap_or(32),
        progress: flag("--progress"),
        ..ComposeParams::default()
    };
    let exact_max = get("--exact-max-keys")
        .and_then(|x| x.parse().ok())
        .unwrap_or(200_000);
    let nt = terminals.len();
    let d0v = rustred_campaign_algebra::valuation::D0::parse(&spec.d0)
        .map_err(|e| e.to_string())?
        .value()
        .clone();
    let backend = get("--backend").unwrap_or_else(|| "exact".into());
    // --given FILE: {"label": [[key], ..], ..} candidate sets to certify
    let given: Vec<(String, Vec<Key>)> = match get("--given") {
        None => vec![],
        Some(p) => {
            let v = text_or_file(&p)?;
            let o = v
                .as_object()
                .ok_or("--given: expected an object {label: [keys]}")?;
            let mut out = vec![];
            for (label, ks) in o {
                let keys: Vec<Key> = serde_json::from_value(ks.clone())
                    .map_err(|e| format!("--given {label}: {e}"))?;
                out.push((label.clone(), keys));
            }
            out
        }
    };
    // --closure: certificate c2 also over every top-sector closure key outside the band
    let closure: Vec<Key> = if flag("--closure") {
        let bs: std::collections::HashSet<&Key> = band.iter().collect();
        (0..src.n_keys())
            .map(|i| src.key(i))
            .filter(|k| rank_of(&chart, k).is_some() && !bs.contains(k))
            .collect()
    } else {
        vec![]
    };
    eprintln!(
        "given sets {}, closure keys outside the band {}",
        given.len(),
        closure.len()
    );
    let ctx = RunCtx {
        node: &node,
        chart: &chart,
        terminals: &terminals,
        band: &band,
        r_max,
        set: &set,
        spec: &spec,
        published: published.as_ref(),
        avail: avail.as_ref().map(|a| a as &dyn Availability),
        binding,
        out: &out,
        tower: spec.tower_enabled && !flag("--no-tower"),
        backend: &backend,
        given: &given,
        closure: &closure,
        tower_only: flag("--tower-only").then(|| {
            get("--tower-r-max")
                .and_then(|x| x.parse::<u32>().ok())
                .unwrap_or_else(|| {
                    band.iter()
                        .filter_map(|k| rank_of(&chart, k))
                        .max()
                        .map_or(0, |r| r as u32 + 1)
                })
        }),
    };
    let summary = match backend.as_str() {
        "exact" => {
            let mut comp =
                EpsdepthCompose::new(&mut src, SymbolicaExact::new()?, d0v, nt, params, exact_max);
            build_and_tower(&mut comp, &ctx)?
        }
        "laurent" => {
            let mut comp = EpsdepthCompose::new(&mut src, lau, d0v, nt, params, exact_max);
            build_and_tower(&mut comp, &ctx)?
        }
        b => return Err(format!("--backend exact | laurent, got {b}")),
    };
    let mut f = std::fs::File::create(out.join("summary.json")).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
    )
    .map_err(|e| e.to_string())?;
    eprintln!(
        "DONE in {:.1} s, rss {} MB",
        t0.elapsed().as_secs_f64(),
        rss_mb()
    );
    Ok(())
}
