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

use rustred_campaign_algebra::epsdepth::SymbolicaLaurent;
use rustred_campaign_algebra::epsdepth::record_source::{RecordSource, file_sha256};
use rustred_campaign_algebra::regbasis::adapter::EpsdepthCompose;
use rustred_campaign_algebra::regbasis::build::{BuildInput, Published, construct};
use rustred_campaign_algebra::regbasis::tower::{TowerInput, TowerSeed, tower};
use rustred_campaign_algebra::{SymbolicaAlgebra, licensed};
use rustred_campaign_core::boxdef::{BoxDef, blake3_hex};
use rustred_campaign_core::epsdepth::{ComposeParams, EpsBox, EpsBoxChecked, ReductionSource};
use rustred_campaign_core::import::{self, LoadOptions, TableSetSpec};
use rustred_campaign_core::key::Key;
use rustred_campaign_core::regbasis::record::Binding;
use rustred_campaign_core::regbasis::{Availability, RegBasisSpec, rank_of};
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

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        max_keys: None,
        max_seconds: None,
    };
    let exact_max = get("--exact-max-keys")
        .and_then(|x| x.parse().ok())
        .unwrap_or(200_000);
    let nt = terminals.len();
    let cmp_fn = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let mut comp = EpsdepthCompose::new(&mut src, lau, nt, params, exact_max);
    let t1 = Instant::now();
    let inp = BuildInput {
        chart: &chart,
        terminals: &terminals,
        band: &band,
        band_r_max: r_max,
        cmp: &cmp_fn,
        spec: &spec,
        published: published.as_ref(),
        availability: avail.as_ref().map(|a| a as &dyn Availability),
        binding: Some(binding.clone()),
    };
    let res = construct(&mut comp, &inp)?;
    let t_build = t1.elapsed().as_secs_f64();
    eprintln!(
        "BUILD {node}: R = {:?} ({:?}); {}; q {} z {}; c1 {:?} c3 {:?} c4 {:?} c2 {:?}/{:?}; {t_build:.1} s, rss {} MB",
        res.regular_set,
        res.sources,
        res.verdict,
        res.q,
        res.lattice.length(),
        res.cert_lattice,
        res.cert_det,
        res.cert_poles_check,
        res.cert_keys_with_pole,
        res.cert_keys_checked,
        rss_mb()
    );
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    std::fs::write(
        out.join("regular_basis.json"),
        res.record_text.clone().unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    let mut summary = json!({
        "node": node, "regular_set": res.regular_set, "sources": res.sources, "verdict": res.verdict,
        "q": res.q, "z": res.lattice.length(), "keys": band.len(), "decided": res.keys_decided,
        "keys_with_pole_on_T": res.keys_with_pole_on_t, "certificates": res.record["certificate"],
        "needs": res.record["need"], "M_order": res.record["M_order"], "M_inv_order": res.record["M_inv_order"],
        "det_M_order": res.record["det_M_order"], "terminal_in_t_N": res.terminal_in_t_n,
        "timing": {"build_s": t_build, "rss_peak_mb": rss_mb()},
    });
    if spec.tower_enabled && !flag("--no-tower") {
        let t2 = Instant::now();
        let seed = TowerSeed::from_build(&res, published.is_some());
        let tin = TowerInput {
            chart: &chart,
            terminals: &terminals,
            cmp: &cmp_fn,
            r_max: res.r_max,
            spec: &spec,
            published: published.as_ref(),
            seed,
            binding: Some(binding),
        };
        let tw = tower(&mut comp, &tin)?;
        eprintln!(
            "TOWER {node}: {} ({:.1} s)",
            tw.verdict,
            t2.elapsed().as_secs_f64()
        );
        let text = rustred_campaign_core::schema::canon::canonical_json(&tw.record, false)?;
        std::fs::write(out.join("tower.json"), text).map_err(|e| e.to_string())?;
        summary["tower"] = json!({"verdict": tw.verdict, "seconds": t2.elapsed().as_secs_f64()});
    }
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
