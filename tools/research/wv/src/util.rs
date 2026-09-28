//! Options, checkpoint loading and small statistics helpers.
use crate::ckpt::{self, Ledger, Manifest};
use crate::geom::{self, Dom};
use crate::recs::{self, K_NATIVE, Rec};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub struct Opts(pub HashMap<String, String>);
impl Opts {
    pub fn parse(a: &[String]) -> Self {
        let mut m = HashMap::new();
        let mut i = 0;
        while i < a.len() {
            let k = a[i].trim_start_matches("--").to_string();
            let v = a.get(i + 1).cloned().unwrap_or_default();
            m.insert(k, v);
            i += 2;
        }
        Opts(m)
    }
    pub fn get(&self, k: &str) -> Option<&str> {
        self.0.get(k).map(|s| s.as_str())
    }
    pub fn num<T: std::str::FromStr>(&self, k: &str, d: T) -> T {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
}

pub struct Ckpt {
    pub m: Manifest,
    pub n: usize,
    pub doms: Vec<Dom>,
    pub recs: Vec<Rec>,
    pub nodes: Vec<u8>,
    pub ledger: Option<Ledger>,
    /// ID -> index into `recs` of its native record (u32::MAX if none).
    pub native_of: Vec<u32>,
}

pub fn load(dir: &Path, opts: &Opts, with_records: bool) -> Ckpt {
    let t = std::time::Instant::now();
    if dir.is_file() {
        return load_result(dir, opts.num("arity", 15usize));
    }
    let m = ckpt::manifest(dir, opts.get("manifest"));
    let n = m.arity;
    let doms = ckpt::domains(&m);
    eprintln!("domains {} ({:.1} s)", doms.len(), t.elapsed().as_secs_f64());
    let nodes = ckpt::nodes(&m);
    let ledger = ckpt::ledger(&m);
    eprintln!("nodes/ledger ({:.1} s)", t.elapsed().as_secs_f64());
    let recs = if with_records { recs::records(&m) } else { Vec::new() };
    eprintln!("records {} ({:.1} s)", recs.len(), t.elapsed().as_secs_f64());
    let mut native_of = vec![u32::MAX; doms.len()];
    let mut dup = 0usize;
    for (i, r) in recs.iter().enumerate() {
        if r.kind == K_NATIVE || r.kind == recs::K_PARTIAL {
            if native_of[r.id as usize] != u32::MAX {
                dup += 1;
            }
            native_of[r.id as usize] = i as u32;
        }
    }
    if dup > 0 {
        eprintln!("warning: {dup} duplicate native records (last one kept)");
    }
    Ckpt { m, n, doms, recs, nodes, ledger, native_of }
}

/// Generation of the domain segment that holds an ID.
pub fn id_gen(m: &Manifest, id: usize) -> u64 {
    m.domains.iter().rev().find(|s| s.first <= id).map_or(0, |s| s.generation)
}

/// Least-squares fit y = a + b x; returns (b, a, r2, n).
pub fn ols(xy: &[(f64, f64)]) -> (f64, f64, f64, usize) {
    let n = xy.len() as f64;
    if xy.len() < 3 {
        return (f64::NAN, f64::NAN, f64::NAN, xy.len());
    }
    let (sx, sy) = xy.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    let (mx, my) = (sx / n, sy / n);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for &(x, y) in xy {
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
        syy += (y - my) * (y - my);
    }
    let b = sxy / sxx;
    let a = my - b * mx;
    let r2 = if syy > 0.0 { sxy * sxy / (sxx * syy) } else { f64::NAN };
    (b, a, r2, xy.len())
}

/// Per-domain summary used by several subcommands.
#[derive(Clone, Copy, Default, Debug)]
pub struct DomSum {
    /// Lattice points (f64; NaN if infinite).
    pub points: f64,
    /// [Amin, Amax, Rmin, Rmax, Pmin, Pmax, Dmin, Dmax]; all zero if empty,
    /// i32::MAX where infinite.
    pub ext: [i32; 8],
    pub empty: bool,
    pub infinite: bool,
}

pub fn dom_sum(d: &Dom, n: usize) -> DomSum {
    match geom::levels(d, n) {
        None => {
            // Infinite: report the finite bounds that exist.
            let c = geom::caps(d);
            let amax = c.acap.map_or(i32::MAX, |a| a + c.t);
            let rmax = c.rcap.unwrap_or(i32::MAX);
            DomSum {
                points: f64::NAN,
                ext: [c.t, amax, 0, rmax, c.t, amax.saturating_add(rmax), i32::MIN, i32::MAX],
                empty: false,
                infinite: true,
            }
        }
        Some(l) => match l.extrema() {
            None => DomSum { points: 0.0, ext: [0; 8], empty: true, infinite: false },
            Some(e) => DomSum { points: l.total() as f64, ext: e, empty: false, infinite: false },
        },
    }
}

pub fn dom_sums(doms: &[Dom], n: usize) -> Vec<DomSum> {
    doms.par_iter().map(|d| dom_sum(d, n)).collect()
}

pub fn quantiles(v: &mut [f64], qs: &[f64]) -> Vec<f64> {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    qs.iter()
        .map(|q| if v.is_empty() { f64::NAN } else { v[((v.len() - 1) as f64 * q).round() as usize] })
        .collect()
}

pub fn sha256_of(path: &Path) -> String {
    let out = std::process::Command::new("sha256sum").arg(path).output();
    out.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.split_whitespace().next().map(|x| x.to_string()))
        .unwrap_or_default()
}

/// A drained run's pretty `result.json` (legacy CP4 pilots have no CP5
/// sections): domains from the records' own geometry, records in array order
/// (commit order for the Ordered policy), no ledger and no node flags.
pub fn load_result(path: &Path, n: usize) -> Ckpt {
    let t = std::time::Instant::now();
    let rows = recs::result_json(path, n);
    let maxid = rows.iter().map(|r| r.0.id as usize).max().map_or(0, |x| x + 1);
    let mut doms = vec![Dom { phase: 255, ..Default::default() }; maxid];
    let mut recs_v = Vec::with_capacity(rows.len());
    for (r, d) in rows {
        doms[r.id as usize] = d;
        recs_v.push(r);
    }
    eprintln!("result.json: {} records, {} ids ({:.1} s)", recs_v.len(), maxid, t.elapsed().as_secs_f64());
    let mut native_of = vec![u32::MAX; doms.len()];
    for (i, r) in recs_v.iter().enumerate() {
        if recs::is_native(r.kind) {
            native_of[r.id as usize] = i as u32;
        }
    }
    Ckpt { m: ckpt::Manifest::dummy(path, n), n, doms, recs: recs_v, nodes: Vec::new(), ledger: None, native_of }
}
