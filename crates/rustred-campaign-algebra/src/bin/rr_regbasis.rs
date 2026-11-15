//! rr_regbasis: construct and certify a REGULAR terminal set R of a closed node on its band (lane regbasis;
//! interfaces/regbasis.md), write the export object and the tower record. Nothing about any node is known here: the
//! node, its closure statement, its box, its published-master map and the user's eps box are INPUT DATA.
//!
//! Usage:
//!   rr_regbasis --import-spec SPEC.json --statement-dir DIR --out DIR
//!               [--import-base /] [--index-dir DIR] [--cache-mb 4096] [--spec JSON|PATH] [--node NAME]
//!               [--box BOXDEF.json] [--published MAP.json] [--eps-box JSON|PATH] [--identity-digest TEXT]
//!               [--max-depth 32] [--exact-max-keys 200000] [--no-tower]
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
use rustred_campaign_core::epsdepth::{ComposeParams, EpsBox, EpsBoxChecked, ReductionSource};
use rustred_campaign_core::import::{self, LoadOptions, TableSetSpec};
use rustred_campaign_core::key::Key;
use rustred_campaign_core::regbasis::record::Binding;
use rustred_campaign_core::regbasis::{Availability, Compose, RegBasisSpec, rank_of};
use serde_json::{Value, json};

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
}

fn build_and_tower(comp: &mut dyn Compose, c: &RunCtx<'_>) -> Result<Value, String> {
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
    let get = |name: &str| -> Option<String> { args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned()) };
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
    let rec: Value = serde_json::from_str(&std::fs::read_to_string(&rec_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let digest_ok = rustred_campaign_core::regbasis::record::verify(&rec).is_ok();
    let d0t = rec["binding"]["d0"].as_str().unwrap_or("4").to_string();
    let d0 = rustred_campaign_algebra::valuation::D0::parse(&d0t).map_err(|e| e.to_string())?;
    let ts = TableSetSpec::read(Path::new(&spec_path)).map_err(|e| e.to_string())?;
    let opts = LoadOptions { index_dir: get("--index-dir").map(PathBuf::from) };
    let set = import::load_with(&ts, Path::new("/"), &opts).map_err(|e| e.to_string())?;
    let alg = SymbolicaAlgebra::new().map_err(|e| e.to_string())?;
    let lau = SymbolicaLaurent::new(&d0t)?;
    let mut src = RecordSource::from_statement(&set, &stmt, alg, lau.clone(), 4096 << 20)?;
    let terminals: Vec<Key> = src.terminals().to_vec();
    let n = terminals.len();
    let idx = src.index_of(&key).ok_or("key not in the statement's reduction")?;
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
                if is_sub { sub.push(json!({"key": k, "pole_order": o})) } else { top.push(json!({"key": k, "pole_order": o})) }
            }
            json!({"pole_order_all_terms": p.pole_order, "pole_order_top_terms": p.pole_order_top,
                   "singular_terms": p.singular_terms, "terms": p.terms, "zero_terms": p.zero_terms,
                   "singular_top_terms": top.len(), "singular_subsector_terms": sub.len(),
                   "top_singular": top, "subsector_singular": sub})
        }
        Err(e) => json!({"error": e}),
    };
    // route A: this lane's exact cone composition (own code path)
    let params = ComposeParams { target: 1, ..ComposeParams::default() };
    let a_texts = {
        let mut comp = EpsdepthCompose::new(&mut src, lau.clone(), d0.value().clone(), n, params.clone(), 1_000_000);
        comp.exact(&key).map_err(|e| format!("route A: {e}"))?
    };
    // route B: lane epsdepth's composition with its exact backend
    let mut ex = SymbolicaExact::new()?;
    let b_vec = rustred_campaign_core::epsdepth::onto_terminals(&mut src, &mut ex, idx, n, &params)?
        .map_err(|e| format!("route B: {e}"))?;
    let b_texts: Vec<String> = b_vec
        .iter()
        .map(|c| match c {
            None => "0".to_string(),
            Some(r) => rf_text(&rf_t_to_d(r, d0.value())),
        })
        .collect();
    let a_rf: Vec<_> = a_texts.iter().map(|t| parse_rf(t)).collect::<Result<_, _>>()?;
    let b_rf: Vec<_> = b_texts.iter().map(|t| parse_rf(t)).collect::<Result<_, _>>()?;
    let routes_equal = a_rf.iter().zip(&b_rf).all(|(x, y)| (x - y).numerator.is_zero());
    let ord = |r: &rustred_campaign_algebra::regbasis::series::RF| rf_valuation(r, d0.value());
    // M of the record: T = M R
    let m_texts: Vec<Vec<String>> = serde_json::from_value(rec["M"].clone()).map_err(|e| format!("record M: {e}"))?;
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
type ZPolyN = symbolica::poly::polynomial::MultivariatePolynomial<symbolica::domains::integer::IntegerRing, u16>;

fn subsector_cmd(args: &[String]) -> Result<(), String> {
    use rustred_campaign_algebra::epsdepth::record_source::{line_at, parse_coefficient};
    use rustred_campaign_algebra::regbasis::adapter::{rf_t_to_d, tcoeff_rf};
    use rustred_campaign_algebra::regbasis::series::{RF, ZP, rf_text, rf_valuation, t_vars};
    use rustred_campaign_core::accept::rule::RuleText;
    use rustred_campaign_core::closure::bodies::{RhsSlot, Sectors, TermClass, parse_rhs_pattern, rule_point};
    use rustred_campaign_core::epsdepth::KeyStatus;
    use std::collections::HashMap;
    use symbolica::domains::integer::Z;
    use symbolica::domains::rational::Rational;
    use symbolica::domains::rational_polynomial::{FromNumeratorAndDenominator, RationalPolynomial};
    let get = |name: &str| -> Option<String> { args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned()) };
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
    let opts = LoadOptions { index_dir: get("--index-dir").map(PathBuf::from) };
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
    let zero = || RationalPolynomial::from_num_den(ZP::new(&Z, None, tv.clone()), ZP::new(&Z, None, tv.clone()).one(), &Z, false);
    let one = || RationalPolynomial::from_num_den(ZP::new(&Z, None, tv.clone()).one(), ZP::new(&Z, None, tv.clone()).one(), &Z, false);
    // cone discovery through the source (top-sector terms, credits handled by the source)
    let mut top_terms: HashMap<usize, Vec<(usize, TCoeff)>> = HashMap::new();
    let mut frontier = vec![root];
    let mut seen: std::collections::HashSet<usize> = [root].into_iter().collect();
    while !frontier.is_empty() {
        let served: Vec<usize> = frontier.iter().copied().filter(|&k| matches!(src.status(k), KeyStatus::Served)).collect();
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
        if state.get(&k).is_some() {
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
    let mut rcache: HashMap<(usize, usize), (RuleText, Vec<Vec<RhsSlot>>, Vec<Result<(ZPolyN, ZPolyN), String>>)> = HashMap::new();
    for &k in &order {
        let KeyStatus::Served = src.status(k) else { continue };
        let kk = src.key(k);
        let (t, r) = src.record_of(k).ok_or("served key without a record")?;
        if !rcache.contains_key(&(t, r)) {
            let info = set.rule(t, r);
            let (line, _) = line_at(Path::new(&info.path), info.offset)?;
            let rt = RuleText::parse(&line, &set.frame).map_err(|e| e.to_string())?;
            let pats = rt.rhs.iter().map(|(p, _)| parse_rhs_pattern(p, arity)).collect::<Result<Vec<_>, _>>()?;
            let coefs = rt.rhs.iter().map(|(_, c)| parse_coefficient(&alg2, c, arity)).collect();
            rcache.insert((t, r), (rt, pats, coefs));
        }
        let (rt, pats, coefs) = &rcache[&(t, r)];
        let mut pt = vec![];
        let at_key = match &rt.domain {
            Some(dm) => rule_point(dm, &kk, &mut pt),
            None => {
                let fixed: Option<Vec<i32>> = rt.record.fixed.iter().map(|x| x.and_then(|v| i32::try_from(v).ok())).collect();
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
            full.insert(k, top_terms[&k].iter().map(|(c, x)| Tm::Top(*c, x.clone())).collect());
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
            if matches!(class, TermClass::PositiveNumerator { .. } | TermClass::ZeroSector { .. }) {
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
                Some(r) => rustred_campaign_algebra::regbasis::series::expand_t_ratio(&r.numerator, &r.denominator, prec, &tv),
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
                    let one_s = rustred_campaign_algebra::regbasis::series::expand_t_ratio(
                        &ZP::new(&Z, None, tv.clone()).one(),
                        &ZP::new(&Z, None, tv.clone()).one(),
                        big,
                        &tv,
                    );
                    m.insert(Comp::T(j), Laurent::normalised(0, one_s.poly.clone(), big));
                    m
                }
                _ => {
                    let mut m: HashMap<Comp, Laurent> = HashMap::new();
                    for tm in full.get(&k).map(|v| v.as_slice()).unwrap_or(&[]) {
                        match tm {
                            Tm::Sub(sk, x) => {
                                let e = expand(x, rk);
                                let slot = m.entry(Comp::S(sk.clone())).or_insert_with(|| Laurent::zero(big, &tv));
                                *slot = slot.add(&e);
                            }
                            Tm::Top(c, x) => {
                                let child = vecs.get(c).ok_or("child vector missing (cycle?)")?;
                                let vlow = child.values().map(|y| y.valuation().unwrap_or(y.prec)).min().unwrap_or(0);
                                let e = expand(x, rk - vlow);
                                for (comp, y) in child {
                                    let slot = m.entry(comp.clone()).or_insert_with(|| Laurent::zero(big, &tv));
                                    *slot = slot.add(&e.mul(y));
                                }
                            }
                        }
                    }
                    for tm in full.get(&k).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if let Tm::Top(c, _) = tm {
                            if let Some(pl) = parents_left.get_mut(c) {
                                *pl -= 1;
                                if *pl == 0 && *c != root {
                                    vecs.remove(c);
                                }
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
                Comp::T(j) => tpart.push(json!({"terminal": j, "valuation": val, "precision": y.prec,
                    "leading": val.map(|v| rustred_campaign_algebra::regbasis::series::rational_text(&y.coeff(v).unwrap()))})),
                Comp::S(sk) => {
                    if val.is_none() && y.prec >= 1 {
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
                    rows.push(json!({"key": sk, "dropped_line_slots": dropped, "valuation": val, "precision": y.prec,
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
            .map(|nk| rows.iter().find(|r| r["key"] == json!(nk)).cloned().unwrap_or(json!({"key": nk, "coefficient": "O(t^1) or zero: no pole"})))
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
        let KeyStatus::Served = src.status(k) else { continue };
        let Some(wk) = w.get(&k).cloned() else { continue };
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
                let fixed: Option<Vec<i32>> = rt.record.fixed.iter().map(|x| x.and_then(|v| i32::try_from(v).ok())).collect();
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
            let Some(rc) = tcoeff_rf(&c, &tv) else { continue };
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
        .map(|t| src.index_of(t).and_then(|i| w.get(&i).cloned()).unwrap_or_else(zero))
        .collect();
    let mut comp = EpsdepthCompose::new(&mut src, lau.clone(), d0.value().clone(), n, ComposeParams::default(), 1_000_000);
    let a_texts = comp.exact(&key).map_err(|e| format!("route A: {e}"))?;
    let check_t = on_t
        .iter()
        .zip(&a_texts)
        .all(|(x, a)| (&rf_t_to_d(x, d0.value()) - &rustred_campaign_algebra::regbasis::series::parse_rf(a).unwrap()).numerator.is_zero());
    let mut subs: Vec<(Key, RF)> = wsub.into_iter().filter(|(_, r)| !r.numerator.is_zero()).collect();
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
        let dropped: Vec<usize> = s.iter().zip(&chart).enumerate().filter(|(_, (x, l))| **l && **x <= 0).map(|(i, _)| i).collect();
        let disp = match set.disp.first_match(s) {
            Ok(rustred_campaign_core::cover::Dispatch::Covered(_)) => "covered by the set's dispatcher",
            Ok(rustred_campaign_core::cover::Dispatch::Uncovered) => "not served by this set",
            Ok(_) => "blocked / undecided in this set",
            Err(_) => "outside the dispatcher's domain",
        };
        rows.push(json!({"key": s, "dropped_line_slots": dropped, "order_at_d0": v, "pole_order": p,
                         "coefficient": rf_text(&rf_t_to_d(r, d0.value())), "relations_contributing": via[s], "set": disp}));
    }
    let named: Vec<Value> = rows.iter().filter(|r| r["relations_contributing"].as_u64().is_some()).take(0).cloned().collect();
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

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
