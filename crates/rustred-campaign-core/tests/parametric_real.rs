//! B1 / B2 on REAL table sets (ignored; run by hand). The set is an import spec (`rustred.campaign.import-spec.v1`).
//!
//!   PARAMETRIC_SPEC=<spec.json> PARAMETRIC_OUT=<out.jsonl> [PARAMETRIC_ZEROS=<a.jsonl:b.jsonl>] [PARAMETRIC_THREADS=4]
//!   [PARAMETRIC_INDEX_DIR=<dir>] [PARAMETRIC_LIMIT=<n records>]  cargo test --release --test parametric_real -- --ignored b1
//!
//! Output: JSONL (header with the table sha256s and the zero-proof files, one line per dispatched record, summary)
//! and `<out>.summary.json`.

use std::io::Write;
use std::path::{Path, PathBuf};

use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::import::{LoadOptions, TableSetSpec, load_with};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::loci::contain::ProofBudget;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::parametric::descent::{
    RecordJob, ZeroIndex, jobs_of, run_jobs, summarize,
};
use rustred_campaign_core::parametric::{PARAMETRIC_DESCENT_SCHEMA, PARAMETRIC_DESCENT_VERSION};
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};
use serde_json::json;
use std::collections::BTreeMap;

fn sha256_file(p: &Path) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(&std::fs::read(p).unwrap());
    h.finish_hex()
}

/// Zero proofs for a run: `PARAMETRIC_ZEROS` files, VERIFIED by default (`PARAMETRIC_ZERO_SUMMARIES` = `summary@sha256`
/// entries separated by `;`, one per group of files that summary verified; every file must be listed by one of them).
/// `PARAMETRIC_ZEROS_UNVERIFIED=1` loads them without a pinned summary (labelled in the evidence).
fn load_zeros() -> (Option<ZeroIndex>, serde_json::Value) {
    let paths: Vec<PathBuf> = std::env::var("PARAMETRIC_ZEROS")
        .ok()
        .map(|s| {
            s.split(':')
                .filter(|x| !x.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    if paths.is_empty() {
        return (None, json!(null));
    }
    let refs: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
    if std::env::var("PARAMETRIC_ZEROS_UNVERIFIED").as_deref() == Ok("1") {
        let z = ZeroIndex::load(&refs).unwrap();
        return (Some(z), json!("UNVERIFIED (no pinned verify summary)"));
    }
    let sums = std::env::var("PARAMETRIC_ZERO_SUMMARIES")
        .expect("PARAMETRIC_ZERO_SUMMARIES (or PARAMETRIC_ZEROS_UNVERIFIED=1)");
    let mut total = ZeroIndex::new();
    let mut pins = Vec::new();
    let mut done: Vec<String> = Vec::new();
    for entry in sums.split(';').filter(|x| !x.is_empty()) {
        let (sp, sha) = entry.rsplit_once('@').expect("summary@sha256");
        let sv: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(sp).unwrap()).unwrap();
        let listed: Vec<String> = sv["proofs_sha256"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let mine: Vec<&Path> = refs
            .iter()
            .copied()
            .filter(|p| {
                listed
                    .iter()
                    .any(|l| l.trim_start_matches("sha256:") == sha256_file(p))
            })
            .collect();
        let z = ZeroIndex::load_verified(&mine, Path::new(sp), sha).unwrap();
        for (p, _) in &z.files {
            done.push(p.clone());
        }
        let v = serde_json::to_value(z.files.clone()).unwrap();
        pins.push(json!({"summary": sp, "sha256": sha, "files": v}));
        total.merge(z);
    }
    for p in &refs {
        assert!(
            done.contains(&p.display().to_string()),
            "{}: not verified by any pinned summary",
            p.display()
        );
    }
    (Some(total), json!({"verified": pins}))
}

#[test]
#[ignore]
fn b1_descent_on_a_real_set() {
    let spec_path = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = PathBuf::from(std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT"));
    let threads: usize = std::env::var("PARAMETRIC_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let limit: Option<usize> = std::env::var("PARAMETRIC_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok());
    let t0 = std::time::Instant::now();
    let ld = load_any(
        &spec_path,
        std::env::var("PARAMETRIC_INDEX_DIR")
            .ok()
            .map(PathBuf::from),
    );
    eprintln!(
        "loaded in {:.1} s: {} records ({})",
        t0.elapsed().as_secs_f64(),
        ld.jobs.len(),
        ld.loader
    );
    let (zeros, zero_basis) = load_zeros();
    if let Some(z) = &zeros {
        eprintln!(
            "zero proofs: {} proved lines, {} other",
            z.proved_lines, z.other_lines
        );
    }
    let mut jobs = ld.jobs.clone();
    if let Some(l) = limit {
        jobs.truncate(l);
    }
    let budget = ProofBudget {
        wall: Some(std::time::Duration::from_secs(20)),
        ..ProofBudget::default()
    };
    let t1 = std::time::Instant::now();
    let results = run_jobs(&jobs, &ld.frame, &ld.cmp, zeros.as_ref(), &budget, threads);
    let secs = t1.elapsed().as_secs_f64();
    let summary = summarize(&results, 200);
    let header = json!({
        "schema": PARAMETRIC_DESCENT_SCHEMA, "version": PARAMETRIC_DESCENT_VERSION, "kind": "header",
        "spec": {"path": spec_path.display().to_string(), "sha256": sha256_file(&spec_path)},
        "node": ld.node, "frame": ld.frame.mask_string(), "comparator": ld.cmp.name(), "loader": ld.loader,
        "tables": ld.tables,
        "zero_proofs": zeros.as_ref().map(|z| z.files.iter().map(|(p, s)| json!({"path": p, "sha256": s})).collect::<Vec<_>>()),
        "zero_proof_basis": zero_basis,
        "records": jobs.len(), "limit": limit, "budget_wall_seconds_per_emptiness": 20, "threads": threads,
    });
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    writeln!(f, "{header}").unwrap();
    for (line, _) in &results {
        writeln!(f, "{line}").unwrap();
    }
    let s = json!({"kind": "summary", "seconds": secs, "summary": summary});
    writeln!(f, "{s}").unwrap();
    f.flush().unwrap();
    let mut sp = out.clone().into_os_string();
    sp.push(".summary.json");
    std::fs::write(
        &sp,
        serde_json::to_string_pretty(
            &json!({"header": header, "seconds": secs, "summary": summary}),
        )
        .unwrap(),
    )
    .unwrap();
    eprintln!(
        "{} records in {secs:.1} s; {}",
        jobs.len(),
        summary["records_by_worst"]
    );
}

/// What a run needs, from the import (preferred) or, when the import refuses the set, from the CORE READER
/// (`read_table_file` + `Table::from_table_file` with the import's own `DomainOptions`), honouring the spec's
/// `engine_table` sources in spec order with their `removed` labels and `records` subsets. Supplied relations,
/// recipes and terminals (per-key services) are not part of either. The loader used is written into the evidence.
pub struct Loaded {
    pub jobs: Vec<RecordJob>,
    pub disp: Dispatcher,
    pub frame: NodeFrame,
    pub cmp: NodeComparator,
    pub tables: BTreeMap<String, String>,
    pub loader: String,
    pub node: Option<String>,
}

fn line_offsets(p: &Path) -> Vec<(u64, u64)> {
    use std::io::BufRead;
    let f = std::fs::File::open(p).unwrap();
    let mut r = std::io::BufReader::with_capacity(1 << 22, f);
    let (mut out, mut off, mut buf) = (Vec::new(), 0u64, Vec::new());
    loop {
        buf.clear();
        let n = r.read_until(b'\n', &mut buf).unwrap() as u64;
        if n == 0 {
            return out;
        }
        let mut len = n;
        while len > 0 && (buf[(len - 1) as usize] == b'\n' || buf[(len - 1) as usize] == b'\r') {
            len -= 1;
        }
        out.push((off, len));
        off += n;
    }
}

pub fn load_any(spec_path: &Path, index_dir: Option<PathBuf>) -> Loaded {
    if spec_path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(|f| f.ends_with("sources.json"))
    {
        return load_sources(spec_path);
    }
    let spec_text = std::fs::read_to_string(spec_path).unwrap();
    let spec = TableSetSpec::from_json(&spec_text).unwrap();
    let base = spec_path.parent().unwrap_or(Path::new("."));
    match load_with(&spec, base, &LoadOptions { index_dir }) {
        Ok(set) => {
            let jobs = jobs_of(&set);
            let node = set.report().node.clone();
            Loaded {
                jobs,
                disp: {
                    // the same tables, priority and chart (integer frame: no eps axes)
                    assert!(!set.frame.has_eps_axes(), "eps frame: not supported here");
                    let d =
                        Dispatcher::new(set.disp.tables().to_vec(), set.disp.priority().to_vec())
                            .unwrap();
                    match set.disp.chart() {
                        Some(c) => d.with_chart(c.to_vec()).unwrap(),
                        None => d,
                    }
                },
                frame: set.frame.clone(),
                cmp: set.cmp.clone(),
                tables: set.table_digests.clone(),
                loader: "import (strict)".into(),
                node,
            }
        }
        Err(e) => {
            eprintln!("import refused: {e:?}; core reader fallback");
            let v: serde_json::Value = serde_json::from_str(&spec_text).unwrap();
            let mut tables = Vec::new();
            let mut jobs = Vec::new();
            let mut digests = BTreeMap::new();
            let mut frame: Option<NodeFrame> = None;
            let mut cmp: Option<NodeComparator> = None;
            let mut order0: Option<serde_json::Value> = None;
            for src in v["sources"].as_array().unwrap() {
                if src["kind"] != "engine_table" {
                    continue;
                }
                let id = src["id"].as_str().unwrap().to_string();
                let path = PathBuf::from(src["path"].as_str().unwrap());
                let removed: Vec<i64> = src["removed"].as_array().map_or(vec![], |a| {
                    a.iter().filter_map(|r| r["label"].as_i64()).collect()
                });
                let subset: Option<Vec<i64>> = src["records"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|x| x.as_i64()).collect());
                let tf = read_table_file(&path, ReadOptions { rhs: RhsMode::Skip }).unwrap();
                let hline = std::io::BufRead::lines(std::io::BufReader::new(
                    std::fs::File::open(&path).unwrap(),
                ))
                .next()
                .unwrap()
                .unwrap();
                let hv: serde_json::Value = serde_json::from_str(&hline).unwrap();
                if frame.is_none() {
                    frame = Some(NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap());
                    cmp = Some(NodeComparator::from_header(&hv).unwrap());
                    order0 = Some(hv["node_order"].clone());
                } else {
                    assert_eq!(
                        Some(&hv["node_order"]),
                        order0.as_ref(),
                        "{}: another node order",
                        path.display()
                    );
                }
                let full = Table::from_table_file(
                    id.clone(),
                    path.display().to_string(),
                    &tf,
                    DomainOptions::default(),
                )
                .unwrap();
                let offs = line_offsets(&path);
                digests.insert(id.clone(), format!("sha256:{}", sha256_file(&path)));
                let ti = tables.len();
                let mut t = Table::new(id.clone(), full.arity());
                for (r, d) in full.domains.iter().enumerate() {
                    let label = full.labels[r];
                    if removed.contains(&label)
                        || subset.as_ref().is_some_and(|s| !s.contains(&label))
                    {
                        continue;
                    }
                    let meta = full.meta[r];
                    let (off, len) = offs[meta.line_no - 1];
                    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
                    {
                        use std::io::{Read, Seek, SeekFrom};
                        let mut f = std::fs::File::open(&path).unwrap();
                        f.seek(SeekFrom::Start(off)).unwrap();
                        let mut b = vec![0u8; len as usize];
                        f.read_exact(&mut b).unwrap();
                        h.update(&b);
                    }
                    let sha = h.finish_hex();
                    jobs.push(RecordJob {
                        table: ti,
                        record: t.domains.len(),
                        domain: d.clone(),
                        path: path.display().to_string(),
                        offset: off,
                        bytes: len,
                        record_sha256: sha.clone(),
                        meta: json!({"table": ti, "record": t.domains.len(), "source": id, "class": src["class"],
                            "label": label, "path": path.display().to_string(), "offset": off, "record_sha256": sha}),
                    });
                    t.push_with_meta(label, d.clone(), meta).unwrap();
                }
                tables.push(t);
            }
            let frame = frame.unwrap();
            let n = tables.len();
            let disp = Dispatcher::new(tables, (0..n).collect())
                .unwrap()
                .with_chart(frame.mask().to_vec())
                .unwrap();
            Loaded {
                jobs,
                disp,
                frame,
                cmp: cmp.unwrap(),
                tables: digests,
                loader: format!("core reader fallback (the import refused the set: {e:?})"),
                node: v["node"].as_str().map(str::to_string),
            }
        }
    }
}

/// B2 on a real set: ONE dispatch-mode partition of the set's rule tables, then every cone of
/// `PARAMETRIC_CONES` (`orthant;simplex:16;box:9:-8`), each with its certificate re-checked by the independent checker.
///   PARAMETRIC_SPEC=<spec> PARAMETRIC_OUT=<prefix> [PARAMETRIC_CONES=..] [PARAMETRIC_WALL=<s>] [PARAMETRIC_UNITS=<n>]
#[test]
#[ignore]
fn b2_cone_coverage_on_a_real_set() {
    use rustred_campaign_core::parametric::check::check_cone_certificate;
    use rustred_campaign_core::parametric::cone::{
        Cone, ConeOptions, certificate, coverage_from_partition, partition_for_cones,
    };
    let spec_path = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let cones_s = std::env::var("PARAMETRIC_CONES").unwrap_or_else(|_| "orthant".into());
    let wall: u64 = std::env::var("PARAMETRIC_WALL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5400);
    let units: u64 = std::env::var("PARAMETRIC_UNITS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(400_000_000);
    let t0 = std::time::Instant::now();
    let mut ld = load_any(
        &spec_path,
        std::env::var("PARAMETRIC_INDEX_DIR")
            .ok()
            .map(PathBuf::from),
    );
    if let Ok(x) = std::env::var("PARAMETRIC_EXTRA_SOURCES") {
        ld = append_sources(ld, Path::new(&x));
    }
    let restored = std::env::var("PARAMETRIC_SUPPLIED_LOCI")
        .ok()
        .map(|f| restore_loci(&mut ld, &f));
    eprintln!(
        "loaded in {:.1} s: {} records ({}); restored loci {}",
        t0.elapsed().as_secs_f64(),
        ld.jobs.len(),
        ld.loader,
        restored
            .as_ref()
            .map_or("none".to_string(), |r| r.to_string())
    );
    let mut opts = ConeOptions::default();
    opts.budget.max_nodes = Some(units);
    opts.budget.wall = Some(std::time::Duration::from_secs(wall));
    if let Some(m) = std::env::var("PARAMETRIC_MEMO")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        opts.budget.max_memo = Some(m);
    }
    // optional CAPS (one-slot, e.g. "box:6:-3"): the partition describes the capped region only (capped_dispatcher)
    use rustred_campaign_core::parametric::cone::{capped_dispatcher, cone_inside_caps};
    let caps: Option<Cone> = std::env::var("PARAMETRIC_CAPS")
        .ok()
        .filter(|c| !c.is_empty())
        .map(|c| {
            let v: Vec<&str> = c.split(':').collect();
            assert_eq!(v[0], "box", "caps: box:<line max>:<numerator min>");
            Cone::box_caps(&ld.frame, v[1].parse().unwrap(), v[2].parse().unwrap())
        });
    // decision G5 (BLOCK = no match on a witness locus, the key falls through): PARAMETRIC_FALLTHROUGH=1
    let fell;
    let base: &Dispatcher = if std::env::var("PARAMETRIC_FALLTHROUGH").as_deref() == Ok("1") {
        fell = rustred_campaign_core::parametric::cone::fall_through_poles(&ld.disp).unwrap();
        &fell
    } else {
        &ld.disp
    };
    let capped;
    let disp = match &caps {
        Some(c) => {
            capped = capped_dispatcher(base, &ld.frame, c).unwrap();
            &capped
        }
        None => base,
    };
    let cp = partition_for_cones(disp, &ld.frame, &opts).unwrap();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for e in &cp.elements {
        *kinds
            .entry(format!(
                "{}{}",
                e.kind,
                if e.blocked_by.is_some() {
                    "/blocked"
                } else {
                    ""
                }
            ))
            .or_default() += 1;
    }
    eprintln!(
        "partition in {:.1} s: {} elements {:?}; flags {:?}",
        cp.millis as f64 / 1e3,
        cp.elements.len(),
        kinds,
        cp.flags
    );
    let mut all = Vec::new();
    for c in cones_s.split(';').filter(|x| !x.is_empty()) {
        let cone = match c.split(':').collect::<Vec<_>>().as_slice() {
            ["orthant"] => Cone::orthant(),
            ["simplex", s] => Cone::excess_simplex(&ld.frame, s.parse().unwrap()),
            ["box", l, m] => Cone::box_caps(&ld.frame, l.parse().unwrap(), m.parse().unwrap()),
            ["drbox", d, r, l] => Cone::dots_rank_box(
                &ld.frame,
                d.parse().unwrap(),
                r.parse().unwrap(),
                l.parse().unwrap(),
            ),
            other => panic!("cone {other:?}"),
        };
        if let Some(c) = &caps
            && !cone_inside_caps(&cone, c, &ld.frame)
        {
            eprintln!(
                "cone {} is not proved inside the caps {}: skipped",
                cone.label, c.label
            );
            all.push(
                json!({"cone": cone, "verdict": "not_run", "reason": "not proved inside the caps"}),
            );
            continue;
        }
        let cov = coverage_from_partition(&cp, disp, &ld.frame, &cone, &opts).unwrap();
        let cert = certificate(&cov, disp, &ld.frame);
        let chk = check_cone_certificate(&cert, disp, &ld.frame, None);
        let tag = c.replace(':', "_");
        std::fs::write(
            format!("{out}.{tag}.certificate.json"),
            serde_json::to_string(&cert).unwrap(),
        )
        .unwrap();
        let mut by_kind: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
        for l in &cov.listed {
            *by_kind
                .entry(l["kind"].as_str().unwrap_or("?").to_string())
                .or_default()
                .entry(l["v"]["verdict"].as_str().unwrap_or("?").to_string())
                .or_default() += 1;
        }
        // the residual finite part: keys of the uncovered / undecided CELLS inside the cone (an upper bound of the open
        // keys: a cell minus covered sub-loci); keys of cells up to 1e6 points are written, the others counted as unsized
        use rustred_campaign_core::parametric::cone::cell_keys_each;
        let (mut res_keys, mut unsized_cells, mut open_keys) = (0u64, 0usize, 0u64);
        let mut reasons: BTreeMap<String, u64> = BTreeMap::new();
        let residual_max: u64 = std::env::var("PARAMETRIC_RESIDUAL_MAX")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5_000_000);
        let mut kf = std::io::BufWriter::new(
            std::fs::File::create(format!("{out}.{tag}.cellkeys.jsonl")).unwrap(),
        );
        for l in &cov.listed {
            let i = l["element"].as_u64().unwrap() as usize;
            let el = &cp.elements[i];
            if el.kind != "generic" {
                unsized_cells += 1;
                continue;
            }
            let mut keys_here = Vec::new();
            if res_keys >= residual_max {
                unsized_cells += 1;
                continue;
            }
            match cell_keys_each(&el.region(), &cone, &ld.frame, 1_000_000, &mut |k| {
                keys_here.push(k.to_vec())
            }) {
                Some(()) => {
                    for k in keys_here {
                        res_keys += 1;
                        let d = format!("{:?}", disp.first_match(&k));
                        let why = if d.starts_with("Ok(Covered") {
                            "covered".to_string()
                        } else if let Some(p) = d.find("reason: ") {
                            let r: String = d[p + 8..]
                                .chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect();
                            format!("blocked:{r}")
                        } else {
                            d.chars().take(40).collect()
                        };
                        let open = why != "covered";
                        open_keys += u64::from(open);
                        *reasons.entry(why.clone()).or_default() += 1;
                        writeln!(
                            kf,
                            "{}",
                            json!({"key": k, "element": i, "open": open, "dispatch": why})
                        )
                        .unwrap();
                    }
                }
                None => unsized_cells += 1,
            }
        }
        kf.flush().unwrap();
        let summary = json!({
            "residual": {"cell_keys_in_cone": res_keys, "of_them_open_by_first_match": open_keys, "cells_not_sized": unsized_cells, "by_dispatch": reasons},
            "cone": cov.cone, "verdict": cov.verdict, "elements": cov.elements, "outside_cone": cov.outside,
            "uncovered": cov.uncovered, "undecided": cov.undecided, "listed_by_kind": by_kind,
            "partition": cov.partition, "flags": cov.partition_flags, "probes": cov.probes.len(),
            "probes_open_unexplained": cov.probes_open_unexplained,
            "checker": {"ok": chk.ok(), "problems": chk.problems.iter().take(50).collect::<Vec<_>>(), "n_problems": chk.problems.len(),
                        "witnesses_checked": chk.witnesses_checked, "probes_checked": chk.probes_checked},
        });
        eprintln!(
            "cone {c}: {} uncovered {} undecided {} (checker ok {}, {} probes)",
            cov.verdict,
            cov.uncovered,
            cov.undecided,
            chk.ok(),
            chk.probes_checked
        );
        all.push(summary);
    }
    let head = json!({
        "schema": rustred_campaign_core::parametric::CONE_COVERAGE_SCHEMA, "version": rustred_campaign_core::parametric::CONE_COVERAGE_VERSION,
        "spec": {"path": spec_path.display().to_string(), "sha256": sha256_file(&spec_path)},
        "node": ld.node, "loader": ld.loader, "tables": ld.tables, "records": ld.jobs.len(),
        "partition_seconds": cp.millis as f64 / 1e3, "elements_by_kind": kinds, "partition_flags": cp.flags,
        "budget": {"units": units, "wall_seconds": wall, "max_memo": opts.budget.max_memo}, "caps": caps,
        "restored_loci": restored, "fall_through": std::env::var("PARAMETRIC_FALLTHROUGH").as_deref() == Ok("1"),
    });
    std::fs::write(
        format!("{out}.summary.json"),
        serde_json::to_string_pretty(&json!({"header": head, "cones": all})).unwrap(),
    )
    .unwrap();
}

/// Records named by catalogue entries (`{"entries": [{"table", "offset", ...}]}`) whose table path starts with a prefix,
/// read with the core reader; one comparator / frame (asserted equal across the tables' headers).
fn load_catalogue(cat: &Path, prefix: &str) -> Loaded {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cat).unwrap()).unwrap();
    let mut by_table: BTreeMap<String, Vec<(u64, serde_json::Value)>> = BTreeMap::new();
    let mut refused: Vec<serde_json::Value> = Vec::new();
    for e in v["entries"].as_array().unwrap() {
        let t = e["table"].as_str().unwrap();
        if t.starts_with(prefix) {
            by_table
                .entry(t.to_string())
                .or_default()
                .push((e["offset"].as_u64().unwrap(), e.clone()));
        }
    }
    let mut tables = Vec::new();
    let mut jobs = Vec::new();
    let mut digests = BTreeMap::new();
    let (mut frame, mut cmp, mut order0): (
        Option<NodeFrame>,
        Option<NodeComparator>,
        Option<serde_json::Value>,
    ) = (None, None, None);
    for (path_s, entries) in &by_table {
        let path = PathBuf::from(path_s);
        let tf = match read_table_file(&path, ReadOptions { rhs: RhsMode::Skip }) {
            Ok(tf) => tf,
            Err(e) => {
                eprintln!("REFUSED by the reader: {path_s}: {e}");
                refused.push(
                    json!({"table": path_s, "entries": entries.len(), "error": e.to_string()}),
                );
                continue;
            }
        };
        let hline =
            std::io::BufRead::lines(std::io::BufReader::new(std::fs::File::open(&path).unwrap()))
                .next()
                .unwrap()
                .unwrap();
        let hv: serde_json::Value = serde_json::from_str(&hline).unwrap();
        if let Some(fr) = frame.as_ref() {
            assert_eq!(
                Some(&hv["node_order"]),
                order0.as_ref(),
                "{}: another node order",
                path.display()
            );
            assert_eq!(
                NodeFrame::from_header(tf.header.as_ref().unwrap())
                    .unwrap()
                    .mask_string(),
                fr.mask_string()
            );
        } else {
            frame = Some(NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap());
            cmp = Some(NodeComparator::from_header(&hv).unwrap());
            order0 = Some(hv["node_order"].clone());
        }
        let full = match Table::from_table_file(
            path_s.clone(),
            path_s.clone(),
            &tf,
            DomainOptions::default(),
        ) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("REFUSED by the strict reader: {path_s}: {e}");
                refused.push(
                    json!({"table": path_s, "entries": entries.len(), "error": e.to_string()}),
                );
                continue;
            }
        };
        let offs = line_offsets(&path);
        digests.insert(path_s.clone(), format!("sha256:{}", sha256_file(&path)));
        let ti = tables.len();
        let mut t = Table::new(path_s.clone(), full.arity());
        for (off, e) in entries {
            let Some(r) =
                (0..full.domains.len()).find(|&r| offs[full.meta[r].line_no - 1].0 == *off)
            else {
                panic!(
                    "{path_s}: no dispatched record at offset {off} (entry {})",
                    e["name"]
                );
            };
            let (o, len) = offs[full.meta[r].line_no - 1];
            let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
            {
                use std::io::{Read, Seek, SeekFrom};
                let mut f = std::fs::File::open(&path).unwrap();
                f.seek(SeekFrom::Start(o)).unwrap();
                let mut b = vec![0u8; len as usize];
                f.read_exact(&mut b).unwrap();
                h.update(&b);
            }
            let sha = h.finish_hex();
            jobs.push(RecordJob {
                table: ti,
                record: t.domains.len(),
                domain: full.domains[r].clone(),
                path: path_s.clone(),
                offset: o,
                bytes: len,
                record_sha256: sha.clone(),
                meta: json!({"table": ti, "record": t.domains.len(), "source": e["name"], "class": e["kind"],
                    "label": full.labels[r], "path": path_s, "offset": o, "record_sha256": sha}),
            });
            t.push_with_meta(full.labels[r], full.domains[r].clone(), full.meta[r])
                .unwrap();
        }
        tables.push(t);
    }
    let frame = frame.unwrap();
    let n = tables.len();
    let disp = Dispatcher::new(tables, (0..n).collect())
        .unwrap()
        .with_chart(frame.mask().to_vec())
        .unwrap();
    Loaded {
        jobs,
        disp,
        frame,
        cmp: cmp.unwrap(),
        tables: digests,
        loader: format!(
            "catalogue entries of {} with table prefix {prefix} (core reader); refused tables: {}",
            cat.display(),
            serde_json::to_string(&refused).unwrap()
        ),
        node: None,
    }
}

/// B1 on catalogue entries: PARAMETRIC_CATALOGUE=<cat.json> PARAMETRIC_PREFIX=<table path prefix> PARAMETRIC_OUT=..
/// [PARAMETRIC_ZEROS=..]
#[test]
#[ignore]
fn b1_descent_on_catalogue_entries() {
    let cat = PathBuf::from(std::env::var("PARAMETRIC_CATALOGUE").expect("PARAMETRIC_CATALOGUE"));
    let prefix = std::env::var("PARAMETRIC_PREFIX").expect("PARAMETRIC_PREFIX");
    let out = PathBuf::from(std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT"));
    let ld = load_catalogue(&cat, &prefix);
    let (zeros, zero_basis) = load_zeros();
    let budget = ProofBudget {
        wall: Some(std::time::Duration::from_secs(20)),
        ..ProofBudget::default()
    };
    let results = run_jobs(&ld.jobs, &ld.frame, &ld.cmp, zeros.as_ref(), &budget, 4);
    let summary = summarize(&results, 200);
    let header = json!({"schema": PARAMETRIC_DESCENT_SCHEMA, "version": PARAMETRIC_DESCENT_VERSION, "kind": "header",
        "catalogue": {"path": cat.display().to_string(), "sha256": sha256_file(&cat)}, "prefix": prefix, "loader": ld.loader,
        "frame": ld.frame.mask_string(), "comparator": ld.cmp.name(), "tables": ld.tables, "records": ld.jobs.len(),
        "zero_proofs": zeros.as_ref().map(|z| z.files.iter().map(|(p, s)| json!({"path": p, "sha256": s})).collect::<Vec<_>>())});
    let mut header = header;
    header["zero_proof_basis"] = zero_basis;
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    writeln!(f, "{header}").unwrap();
    for (line, _) in &results {
        writeln!(f, "{line}").unwrap();
    }
    writeln!(f, "{}", json!({"kind": "summary", "summary": summary})).unwrap();
    f.flush().unwrap();
    let mut sp = out.clone().into_os_string();
    sp.push(".summary.json");
    std::fs::write(
        &sp,
        serde_json::to_string_pretty(&json!({"header": header, "summary": summary})).unwrap(),
    )
    .unwrap();
    eprintln!("{} records; {}", ld.jobs.len(), summary["records_by_worst"]);
}

/// Brute-force CROSS-CHECK of B1 on real records (not a proof): seeded sample of records, keys of a small box around
/// each domain's floor where the record applies, every term instantiated with its own pattern parser and compared by the
/// concrete comparator; a positive-numerator term must carry a zero proof on a locus through the key.
///   PARAMETRIC_SPEC=.. [PARAMETRIC_ZEROS=..] [PARAMETRIC_SAMPLE=300] [PARAMETRIC_KEYS=64]
#[test]
#[ignore]
fn b1_crosscheck_by_brute_force_on_real_records() {
    use rustred_campaign_core::import::zero_proofs::Locus;
    use rustred_campaign_core::parametric::descent::read_record;
    let spec_path = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let sample: usize = std::env::var("PARAMETRIC_SAMPLE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let per: usize = std::env::var("PARAMETRIC_KEYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(64);
    let ld = load_any(
        &spec_path,
        std::env::var("PARAMETRIC_INDEX_DIR")
            .ok()
            .map(PathBuf::from),
    );
    let (zeros, _) = load_zeros();
    let zeros = zeros.unwrap_or_default();
    let n = ld.frame.arity();
    let mut seed: u64 = 0x5eed_b1c0;
    let mut next = move || {
        seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let (mut keys, mut terms, mut posnum, mut handdown, mut viol) =
        (0u64, 0u64, 0u64, 0u64, Vec::new());
    for _ in 0..sample {
        let job = &ld.jobs[(next() % ld.jobs.len() as u64) as usize];
        let (_, rhs, sha) = read_record(Path::new(&job.path), job.offset, job.bytes).unwrap();
        // own parser: slot s = constant or n_s + o
        let pats: Vec<Vec<Result<i32, i32>>> = rhs
            .iter()
            .map(|t| {
                t.trim()
                    .trim_start_matches("I(")
                    .trim_end_matches(')')
                    .split(',')
                    .enumerate()
                    .map(|(s, p)| {
                        let p = p.trim();
                        if let Some(rest) = p.strip_prefix(&format!("n{s}")) {
                            Err(if rest.is_empty() {
                                0
                            } else {
                                rest.trim_start_matches('+').parse().unwrap()
                            })
                        } else {
                            Ok(p.parse().unwrap())
                        }
                    })
                    .collect()
            })
            .collect();
        let d = &job.domain;
        for _ in 0..per {
            let key: Vec<i32> = (0..n)
                .map(|s| match d.fixed[s] {
                    Some(v) => v,
                    None => {
                        if ld.frame.is_line(s) {
                            1 + (next() % 4) as i32
                        } else {
                            -((next() % 4) as i32)
                        }
                    }
                })
                .collect();
            if !d.applies(&key).applies() {
                continue;
            }
            keys += 1;
            for (j, p) in pats.iter().enumerate() {
                terms += 1;
                let tk: Vec<i32> = p
                    .iter()
                    .enumerate()
                    .map(|(s, x)| match x {
                        Ok(c) => *c,
                        Err(o) => key[s] + o,
                    })
                    .collect();
                let pos: Vec<usize> = (0..n)
                    .filter(|&s| !ld.frame.is_line(s) && tk[s] >= 1)
                    .collect();
                if !pos.is_empty() {
                    posnum += 1;
                    let ok = zeros.proof(&sha, j as u32, Locus::Domain).is_some()
                        || pos.iter().any(|&s| {
                            zeros
                                .proof(
                                    &sha,
                                    j as u32,
                                    Locus::Plane {
                                        param: s as u32,
                                        value: i64::from(key[s]),
                                    },
                                )
                                .is_some()
                        });
                    if !ok && viol.len() < 20 {
                        viol.push(format!(
                            "{}: term {j} positive numerator at {key:?} without a zero proof",
                            job.meta["source"]
                        ));
                    }
                    continue;
                }
                if (0..n).any(|s| ld.frame.is_line(s) && tk[s] <= 0) {
                    handdown += 1;
                }
                if ld.cmp.cmp(&tk, &key) != std::cmp::Ordering::Less && viol.len() < 20 {
                    viol.push(format!(
                        "{}: term {j} not lower at {key:?}",
                        job.meta["source"]
                    ));
                }
            }
        }
    }
    eprintln!(
        "crosscheck: {sample} records sampled, {keys} keys where the record applies, {terms} term instances ({posnum} positive-numerator, {handdown} hand-down): {} violation(s) {:?}",
        viol.len(),
        viol
    );
    assert!(viol.is_empty());
}

/// Every dispatched record of the given table files (core reader, all records), one comparator / frame.
fn load_tables(paths: &[PathBuf]) -> Loaded {
    let mut tables = Vec::new();
    let mut jobs = Vec::new();
    let mut digests = BTreeMap::new();
    let (mut frame, mut cmp, mut order0): (
        Option<NodeFrame>,
        Option<NodeComparator>,
        Option<serde_json::Value>,
    ) = (None, None, None);
    for path in paths {
        let path_s = path.display().to_string();
        let tf = read_table_file(path, ReadOptions { rhs: RhsMode::Skip }).unwrap();
        let hline =
            std::io::BufRead::lines(std::io::BufReader::new(std::fs::File::open(path).unwrap()))
                .next()
                .unwrap()
                .unwrap();
        let hv: serde_json::Value = serde_json::from_str(&hline).unwrap();
        if let Some(fr) = frame.as_ref() {
            assert_eq!(
                Some(&hv["node_order"]),
                order0.as_ref(),
                "{path_s}: another node order"
            );
            assert_eq!(
                NodeFrame::from_header(tf.header.as_ref().unwrap())
                    .unwrap()
                    .mask_string(),
                fr.mask_string()
            );
        } else {
            frame = Some(NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap());
            cmp = Some(NodeComparator::from_header(&hv).unwrap());
            order0 = Some(hv["node_order"].clone());
        }
        let full = Table::from_table_file(
            path_s.clone(),
            path_s.clone(),
            &tf,
            DomainOptions::default(),
        )
        .unwrap();
        let offs = line_offsets(path);
        digests.insert(path_s.clone(), format!("sha256:{}", sha256_file(path)));
        let ti = tables.len();
        for r in 0..full.domains.len() {
            let (o, len) = offs[full.meta[r].line_no - 1];
            let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
            {
                use std::io::{Read, Seek, SeekFrom};
                let mut f = std::fs::File::open(path).unwrap();
                f.seek(SeekFrom::Start(o)).unwrap();
                let mut b = vec![0u8; len as usize];
                f.read_exact(&mut b).unwrap();
                h.update(&b);
            }
            let sha = h.finish_hex();
            jobs.push(RecordJob {
                table: ti,
                record: r,
                domain: full.domains[r].clone(),
                path: path_s.clone(),
                offset: o,
                bytes: len,
                record_sha256: sha.clone(),
                meta: json!({"table": ti, "record": r, "label": full.labels[r], "line": full.meta[r].line_no,
                    "path": path_s, "offset": o, "record_sha256": sha}),
            });
        }
        tables.push(full);
    }
    let frame = frame.unwrap();
    let n = tables.len();
    let disp = Dispatcher::new(tables, (0..n).collect())
        .unwrap()
        .with_chart(frame.mask().to_vec())
        .unwrap();
    Loaded {
        jobs,
        disp,
        frame,
        cmp: cmp.unwrap(),
        tables: digests,
        loader: "table files, every dispatched record (core reader)".into(),
        node: None,
    }
}

/// B1 on table files: PARAMETRIC_TABLES=<a:b> PARAMETRIC_OUT=.. [PARAMETRIC_ZEROS=.. + summaries]
#[test]
#[ignore]
fn b1_descent_on_tables() {
    let paths: Vec<PathBuf> = std::env::var("PARAMETRIC_TABLES")
        .expect("PARAMETRIC_TABLES")
        .split(':')
        .filter(|x| !x.is_empty())
        .map(PathBuf::from)
        .collect();
    let out = PathBuf::from(std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT"));
    let ld = load_tables(&paths);
    let (zeros, zero_basis) = load_zeros();
    let budget = ProofBudget {
        wall: Some(std::time::Duration::from_secs(20)),
        ..ProofBudget::default()
    };
    let results = run_jobs(&ld.jobs, &ld.frame, &ld.cmp, zeros.as_ref(), &budget, 4);
    let summary = summarize(&results, 200);
    let header = json!({"schema": PARAMETRIC_DESCENT_SCHEMA, "version": PARAMETRIC_DESCENT_VERSION, "kind": "header",
        "loader": ld.loader, "frame": ld.frame.mask_string(), "comparator": ld.cmp.name(), "tables": ld.tables,
        "records": ld.jobs.len(), "zero_proof_basis": zero_basis});
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    writeln!(f, "{header}").unwrap();
    for (line, _) in &results {
        writeln!(f, "{line}").unwrap();
    }
    writeln!(f, "{}", json!({"kind": "summary", "summary": summary})).unwrap();
    f.flush().unwrap();
    let mut sp = out.clone().into_os_string();
    sp.push(".summary.json");
    std::fs::write(
        &sp,
        serde_json::to_string_pretty(&json!({"header": header, "summary": summary})).unwrap(),
    )
    .unwrap();
    eprintln!("{} records; {}", ld.jobs.len(), summary["records_by_worst"]);
}

/// A SOURCES list (lane parametric's dispatch builder: `{"node", "sources": [{"id", "class", "path", "records": [labels] |
/// null, "witness": true | false | "APPENDED"}]}`), read with the core reader in list order. `witness: false` loads the
/// table with the witness check off (the statement's semantics for its elided lane records); "APPENDED" follows
/// `PARAMETRIC_APPENDED` = `apply` (witness off: the helper's APPEND realisation) or `block` (default: the core blocks).
/// A table the core reader refuses stops the load (named); records the reader skips (not dispatched) are counted.
fn load_sources(path: &Path) -> Loaded {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let appended_apply = std::env::var("PARAMETRIC_APPENDED").as_deref() == Ok("apply");
    let mut tables = Vec::new();
    let mut jobs = Vec::new();
    let mut digests = BTreeMap::new();
    let (mut frame, mut cmp): (Option<NodeFrame>, Option<NodeComparator>) = (None, None);
    let mut notes = Vec::new();
    for src in v["sources"].as_array().unwrap() {
        let id = src["id"].as_str().unwrap().to_string();
        let p = PathBuf::from(src["path"].as_str().unwrap());
        let witness = match &src["witness"] {
            serde_json::Value::Bool(b) => *b,
            serde_json::Value::String(s) if s == "APPENDED" => !appended_apply,
            _ => true,
        };
        let subset: Option<Vec<i64>> = src["records"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_i64()).collect());
        let tf = read_table_file(&p, ReadOptions { rhs: RhsMode::Skip })
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        if frame.is_none() {
            let hline =
                std::io::BufRead::lines(std::io::BufReader::new(std::fs::File::open(&p).unwrap()))
                    .next()
                    .unwrap()
                    .unwrap();
            let hv: serde_json::Value = serde_json::from_str(&hline).unwrap();
            frame = Some(NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap());
            cmp = Some(NodeComparator::from_header(&hv).unwrap());
        }
        let opts = DomainOptions {
            witness,
            ..DomainOptions::default()
        };
        let full = Table::from_table_file(id.clone(), p.display().to_string(), &tf, opts)
            .unwrap_or_else(|e| panic!("{id} ({}): {e}", p.display()));
        let offs = line_offsets(&p);
        digests.insert(id.clone(), format!("sha256:{}", sha256_file(&p)));
        let ti = tables.len();
        let mut t = Table::new(id.clone(), full.arity());
        let order: Vec<usize> = match &subset {
            None => (0..full.domains.len()).collect(),
            Some(s) => s
                .iter()
                .map(|l| {
                    full.labels
                        .iter()
                        .position(|x| x == l)
                        .unwrap_or_else(|| panic!("{id}: no dispatched record {l}"))
                })
                .collect(),
        };
        for r in order {
            let (off, len) = offs[full.meta[r].line_no - 1];
            let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
            {
                use std::io::{Read, Seek, SeekFrom};
                let mut f = std::fs::File::open(&p).unwrap();
                f.seek(SeekFrom::Start(off)).unwrap();
                let mut b = vec![0u8; len as usize];
                f.read_exact(&mut b).unwrap();
                h.update(&b);
            }
            let sha = h.finish_hex();
            jobs.push(RecordJob {
                table: ti,
                record: t.domains.len(),
                domain: full.domains[r].clone(),
                path: p.display().to_string(),
                offset: off,
                bytes: len,
                record_sha256: sha.clone(),
                meta: json!({"table": ti, "record": t.domains.len(), "source": id, "class": src["class"], "label": full.labels[r],
                    "path": p.display().to_string(), "offset": off, "record_sha256": sha, "witness_check": witness}),
            });
            t.push_with_meta(full.labels[r], full.domains[r].clone(), full.meta[r])
                .unwrap();
        }
        if full.skipped_count() > 0 {
            notes.push(format!(
                "{id}: {} record(s) not dispatched (skipped by the reader)",
                full.skipped_count()
            ));
        }
        tables.push(t);
    }
    let frame = frame.unwrap();
    let n = tables.len();
    let disp = Dispatcher::new(tables, (0..n).collect())
        .unwrap()
        .with_chart(frame.mask().to_vec())
        .unwrap();
    Loaded {
        jobs,
        disp,
        frame,
        cmp: cmp.unwrap(),
        tables: digests,
        loader: format!(
            "sources list {} (core reader; appended records: {}); {}",
            path.display(),
            if appended_apply { "apply" } else { "block" },
            notes.join("; ")
        ),
        node: v["node"].as_str().map(str::to_string),
    }
}

/// `PARAMETRIC_EXTRA_SOURCES`: a sources list appended AFTER the loaded dispatch (same frame and comparator, checked).
fn append_sources(ld: Loaded, extra: &Path) -> Loaded {
    let ex = load_sources(extra);
    assert_eq!(
        ex.frame.mask_string(),
        ld.frame.mask_string(),
        "extra sources: another frame"
    );
    assert_eq!(
        ex.cmp.name(),
        ld.cmp.name(),
        "extra sources: another comparator"
    );
    let mut tables: Vec<Table> = ld
        .disp
        .priority()
        .iter()
        .map(|&t| ld.disp.tables()[t].clone())
        .collect();
    // job table indices follow the priority order of the rebuilt dispatcher
    let remap: BTreeMap<usize, usize> = ld
        .disp
        .priority()
        .iter()
        .enumerate()
        .map(|(i, &t)| (t, i))
        .collect();
    let mut jobs: Vec<RecordJob> = ld
        .jobs
        .into_iter()
        .map(|mut j| {
            j.table = remap[&j.table];
            j.meta["table"] = json!(j.table);
            j
        })
        .collect();
    let base = tables.len();
    for &t in ex.disp.priority() {
        tables.push(ex.disp.tables()[t].clone());
    }
    for mut j in ex.jobs {
        j.table += base;
        j.meta["table"] = json!(j.table);
        jobs.push(j);
    }
    let n = tables.len();
    let disp = Dispatcher::new(tables, (0..n).collect())
        .unwrap()
        .with_chart(ld.frame.mask().to_vec())
        .unwrap();
    let mut digests = ld.tables;
    digests.extend(ex.tables);
    Loaded {
        jobs,
        disp,
        frame: ld.frame,
        cmp: ld.cmp,
        tables: digests,
        loader: format!("{} + {}", ld.loader, ex.loader),
        node: ld.node,
    }
}

/// `PARAMETRIC_SUPPLIED_LOCI` (`a.jsonl:b.jsonl`, lines `{"name", "witness_loci": [..]}`): RESTORED witness loci (coordinator
/// decision A for T162): the record whose name (import hand name, or `origin.source` of a derived record) is listed gets
/// exactly these loci as its witness pole loci and is no longer elided. Returns the evidence (file sha256s, matched names).
fn restore_loci(ld: &mut Loaded, files: &str) -> serde_json::Value {
    use rustred_campaign_core::domain::{PoleKind, PoleLocus};
    let mut loci: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut fev = Vec::new();
    for f in files.split(':').filter(|x| !x.is_empty()) {
        for l in std::fs::read_to_string(f)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
        {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            let w: Vec<String> = v["witness_loci"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect();
            loci.insert(v["name"].as_str().unwrap().to_string(), w);
        }
        fev.push(json!({"path": f, "sha256": sha256_file(Path::new(f))}));
    }
    let n = ld.frame.arity();
    let mut tables: Vec<Table> = ld.disp.tables().to_vec();
    let mut matched = Vec::new();
    for j in &mut ld.jobs {
        let mut name = j.meta["hand_name"].as_str().map(str::to_string);
        if (name.is_none() || !loci.contains_key(name.as_deref().unwrap_or("")))
            && let Ok((_, _, _)) = rustred_campaign_core::parametric::descent::read_record(
                Path::new(&j.path),
                j.offset,
                j.bytes,
            )
        {
            use std::io::{Read, Seek, SeekFrom};
            let mut f = std::fs::File::open(&j.path).unwrap();
            f.seek(SeekFrom::Start(j.offset)).unwrap();
            let mut b = vec![0u8; j.bytes as usize];
            f.read_exact(&mut b).unwrap();
            let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
            if let Some(s) = v["origin"]["source"].as_str() {
                name = Some(s.to_string());
            }
        }
        let Some(name) = name else { continue };
        let Some(w) = loci.get(&name) else { continue };
        let t = &mut tables[j.table];
        let d = &mut t.domains[j.record];
        d.poles.retain(|p| p.kind != PoleKind::Witness);
        for s in w {
            let p = rustred_campaign_core::poly::Poly::parse_exact(s, n, true).unwrap();
            d.poles.push(PoleLocus::new(PoleKind::Witness, &p));
        }
        d.witness_elided = false;
        t.meta[j.record].witness = rustred_campaign_core::cover::WitnessStatus::Complete;
        j.domain = d.clone();
        matched.push(name);
    }
    let all: Vec<&String> = loci.keys().collect();
    let missing: Vec<&String> = all
        .iter()
        .copied()
        .filter(|k| !matched.contains(k))
        .collect();
    let pr = ld.disp.priority().to_vec();
    ld.disp = Dispatcher::new(tables, pr)
        .unwrap()
        .with_chart(ld.frame.mask().to_vec())
        .unwrap();
    json!({"files": fev, "listed": loci.len(), "matched": matched.len(), "not_in_dispatch": missing})
}

/// A cone from its text form (`orthant`, `simplex:S`, `box:L:M`, `drbox:D:R:L`).
fn parse_cone(frame: &NodeFrame, c: &str) -> rustred_campaign_core::parametric::cone::Cone {
    use rustred_campaign_core::parametric::cone::Cone;
    match c.split(':').collect::<Vec<_>>().as_slice() {
        ["orthant"] => Cone::orthant(),
        ["simplex", s] => Cone::excess_simplex(frame, s.parse().unwrap()),
        ["box", l, m] => Cone::box_caps(frame, l.parse().unwrap(), m.parse().unwrap()),
        ["drbox", d, r, l] => Cone::dots_rank_box(
            frame,
            d.parse().unwrap(),
            r.parse().unwrap(),
            l.parse().unwrap(),
        ),
        other => panic!("cone {other:?}"),
    }
}

/// Resident memory of this process (bytes; 0 if unreadable).
fn rss_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb * 1024)
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

/// Give freed heap back to the system between pieces (glibc), so the next piece's memory guard reads its own use.
fn trim_heap() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: glibc's malloc_trim only releases free heap pages; no pointer is touched.
    unsafe {
        malloc_trim(0);
    }
}

/// Payload of the memory guard's unwind (a piece over its memory limit is refined, never a test failure).
struct PieceOverMemory(u64);

/// B2 FACE BY FACE (coordinator task 2 of PARAMETRIC-ANSWER-2): ONE capped partition per face piece
/// (`parametric::cone::face_piece`: per split slot the boundary value or the rest of the chart; the pieces are disjoint
/// and cover the chart orthant), each piece run on `capped_dispatcher(base, piece)` (dispatch unchanged inside the
/// piece) and each cone evaluated on `cone and piece`. STREAMED: one line per finished or refined piece in
/// `<out>.pieces.jsonl` (a rerun resumes: finished pieces are skipped, refined ones replaced by their children);
/// certificates / residual keys under `<out>.pieces/`. A piece is REFINED on the next slot of `PARAMETRIC_PIECE_SLOTS`
/// when its partition is not complete or its resident memory passes `PARAMETRIC_PIECE_RSS_GB` (memory guard: the
/// partition's progress callback unwinds, the piece's memory is freed, nothing of it is used).
///
/// Env: as `b2_cone_coverage_on_a_real_set` (PARAMETRIC_SPEC, PARAMETRIC_INDEX_DIR, PARAMETRIC_EXTRA_SOURCES,
/// PARAMETRIC_SUPPLIED_LOCI, PARAMETRIC_FALLTHROUGH, PARAMETRIC_CONES, PARAMETRIC_MEMO, PARAMETRIC_OUT) plus
/// PARAMETRIC_PIECE_SLOTS (comma list, refinement order), PARAMETRIC_PIECE_DEPTH (initial split, default 0),
/// PARAMETRIC_PIECE_RSS_GB (default 30), PARAMETRIC_PIECE_UNITS (per piece, default 200 M), PARAMETRIC_PIECE_WALL
/// (seconds per piece, default 1800), PARAMETRIC_RESIDUAL_MAX (per piece and cone, default 2 M).
#[test]
#[ignore]
fn b2_face_by_face() {
    use rustred_campaign_core::parametric::check::check_cone_certificate;
    use rustred_campaign_core::parametric::cone::{
        ConeOptions, capped_dispatcher, cell_keys_each, certificate, coverage_from_partition,
        face_piece, partition_for_cones_traced,
    };
    use rustred_campaign_core::partition::face_of;
    use std::cell::Cell;
    use std::collections::{BTreeSet, VecDeque};
    let env_u64 = |k: &str, d: u64| -> u64 {
        std::env::var(k)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(d)
    };
    let spec_path = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let cones_s = std::env::var("PARAMETRIC_CONES").unwrap_or_else(|_| "orthant".into());
    let slots: Vec<usize> = std::env::var("PARAMETRIC_PIECE_SLOTS")
        .expect("PARAMETRIC_PIECE_SLOTS")
        .split(',')
        .filter(|x| !x.is_empty())
        .map(|x| x.trim().parse().unwrap())
        .collect();
    let depth = env_u64("PARAMETRIC_PIECE_DEPTH", 0) as usize;
    assert!(depth <= slots.len());
    let rss_max = env_u64("PARAMETRIC_PIECE_RSS_GB", 30) << 30;
    let units = env_u64("PARAMETRIC_PIECE_UNITS", 200_000_000);
    let wall = env_u64("PARAMETRIC_PIECE_WALL", 1800);
    let residual_max = env_u64("PARAMETRIC_RESIDUAL_MAX", 2_000_000);
    let t0 = std::time::Instant::now();
    let mut ld = load_any(
        &spec_path,
        std::env::var("PARAMETRIC_INDEX_DIR")
            .ok()
            .map(PathBuf::from),
    );
    if let Ok(x) = std::env::var("PARAMETRIC_EXTRA_SOURCES") {
        ld = append_sources(ld, Path::new(&x));
    }
    let restored = std::env::var("PARAMETRIC_SUPPLIED_LOCI")
        .ok()
        .map(|f| restore_loci(&mut ld, &f));
    // decision G5 fall-through: "1" = every pole locus moved into the exceptions; "reduced" = loci proved disjoint
    // from their record's selected set dropped, the others moved (cone::fall_through_poles_reduced; same dispatch)
    // "core+g5": the partition runs on the CORE dispatch (a pole = stop set, blocked sub-partitions; equal to G5 on
    // every locus proved disjoint from its record's selected set, a superset of the G5-uncovered set on the others),
    // every residual key is re-dispatched with the exact G5 dispatcher (reduced fall-through)
    let ft_mode = std::env::var("PARAMETRIC_FALLTHROUGH").unwrap_or_default();
    let fall_through = ft_mode == "1" || ft_mode == "reduced";
    let fell;
    let mut ft_stats = serde_json::Value::Null;
    let g5: Option<Dispatcher> = if ft_mode == "core+g5" {
        let (d, st) = rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
            &ld.disp,
            &ld.frame,
            &ProofBudget::default(),
        )
        .unwrap();
        eprintln!(
            "G5 re-dispatcher (reduced fall-through): {}",
            serde_json::to_string(&st).unwrap()
        );
        ft_stats = serde_json::to_value(&st).unwrap();
        Some(d)
    } else {
        None
    };
    let base: &Dispatcher = if ft_mode == "reduced" {
        let tf = std::time::Instant::now();
        let (d, st) = rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
            &ld.disp,
            &ld.frame,
            &ProofBudget::default(),
        )
        .unwrap();
        eprintln!(
            "fall-through (reduced) in {:.1} s: {}",
            tf.elapsed().as_secs_f64(),
            serde_json::to_string(&st).unwrap()
        );
        ft_stats = serde_json::to_value(&st).unwrap();
        fell = d;
        &fell
    } else if fall_through {
        fell = rustred_campaign_core::parametric::cone::fall_through_poles(&ld.disp).unwrap();
        &fell
    } else {
        &ld.disp
    };
    eprintln!(
        "loaded in {:.1} s: {} records ({}); restored loci {}; fall-through {ft_mode:?}; rss {:.1} GB",
        t0.elapsed().as_secs_f64(),
        ld.jobs.len(),
        ld.loader,
        restored
            .as_ref()
            .map_or("none".to_string(), |r| r.to_string()),
        rss_bytes() as f64 / 1e9
    );
    if std::env::var("PARAMETRIC_LOAD_ONLY").as_deref() == Ok("1") {
        std::fs::write(
            format!("{out}.fallthrough.json"),
            serde_json::to_string_pretty(&ft_stats).unwrap(),
        )
        .unwrap();
        return;
    }
    let frame = ld.frame.clone();
    let lines: Vec<bool> = (0..frame.arity()).map(|i| frame.is_line(i)).collect();
    let cones: Vec<(String, rustred_campaign_core::parametric::cone::Cone)> = cones_s
        .split(';')
        .filter(|x| !x.is_empty())
        .map(|c| (c.replace(':', "_"), parse_cone(&frame, c)))
        .collect();
    let mut opts = ConeOptions::default();
    opts.budget.max_nodes = Some(units);
    opts.budget.wall = Some(std::time::Duration::from_secs(wall));
    if let Some(m) = std::env::var("PARAMETRIC_MEMO")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        opts.budget.max_memo = Some(m);
    }
    opts.factor_exceptions = std::env::var("PARAMETRIC_FACTOR_EXCEPTIONS").as_deref() == Ok("1");
    opts.describe = std::env::var("PARAMETRIC_DESCRIBE").as_deref() == Ok("1");
    opts.max_blocked_classes = std::env::var("PARAMETRIC_MAX_BLOCKED_CLASSES")
        .ok()
        .and_then(|s| s.parse().ok());
    let dir = PathBuf::from(format!("{out}.pieces"));
    std::fs::create_dir_all(&dir).unwrap();
    let lines_path = format!("{out}.pieces.jsonl");
    // resume
    let mut status: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(t) = std::fs::read_to_string(&lines_path) {
        for l in t.lines().filter(|l| !l.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            status.insert(
                v["piece"].as_str().unwrap().to_string(),
                v["status"].as_str().unwrap().to_string(),
            );
        }
    }
    let mut pf = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&lines_path)
            .unwrap(),
    );
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.payload().downcast_ref::<PieceOverMemory>().is_none() {
            prev_hook(info);
        }
    }));
    let file_id = |label: &str| -> String {
        label
            .trim_start_matches("piece ")
            .replace("<=-1", "m")
            .replace(">=2", "p")
            .replace("=0", "z")
            .replace("=1", "o")
            .replace(", ", "_")
            .replace(' ', "_")
    };
    let mut queue: VecDeque<Vec<(usize, bool)>> = VecDeque::new();
    for code in 0..(1usize << depth) {
        queue.push_back(
            (0..depth)
                .map(|j| (slots[j], (code >> (depth - 1 - j)) & 1 == 1))
                .collect(),
        );
    }
    let mut n_run = 0usize;
    while let Some(choice) = queue.pop_front() {
        let piece = face_piece(&frame, &choice).expect("piece");
        let id = piece.label.clone();
        let children = |q: &mut VecDeque<Vec<(usize, bool)>>| {
            let s = slots[choice.len()];
            for b in [false, true] {
                let mut c = choice.clone();
                c.push((s, b));
                q.push_back(c);
            }
        };
        match status.get(&id).map(String::as_str) {
            Some("done") => continue,
            Some("refined") => {
                children(&mut queue);
                continue;
            }
            _ => {}
        }
        let can_refine = choice.len() < slots.len();
        let tp = std::time::Instant::now();
        let capped = capped_dispatcher(base, &frame, &piece).unwrap();
        trim_heap();
        let rss0 = rss_bytes();
        let peak = Cell::new(rss0);
        let last = Cell::new(std::time::Instant::now());
        let cb = |p: &rustred_campaign_core::partition::Progress| {
            let r = rss_bytes();
            if r > peak.get() {
                peak.set(r);
            }
            if last.get().elapsed().as_secs() >= 60 {
                last.set(std::time::Instant::now());
                eprintln!(
                    "  [{id}] nodes {} memo {} depth {} rss {:.1} GB",
                    p.nodes,
                    p.memo_size,
                    p.depth,
                    r as f64 / 1e9
                );
            }
            if r > rss_max {
                std::panic::panic_any(PieceOverMemory(r));
            }
        };
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            partition_for_cones_traced(&capped, &frame, &opts, Some(&cb), &|m: &str| {
                eprintln!("  [{id}] {m}; rss {:.1} GB", rss_bytes() as f64 / 1e9)
            })
        }));
        let cp = match res {
            Err(payload) => {
                let Some(PieceOverMemory(r)) = payload.downcast_ref::<PieceOverMemory>() else {
                    std::panic::resume_unwind(payload);
                };
                let r = *r;
                drop(payload);
                trim_heap();
                eprintln!(
                    "piece [{id}]: over memory ({:.1} GB > {} GB) after {:.0} s: {}",
                    r as f64 / 1e9,
                    rss_max >> 30,
                    tp.elapsed().as_secs_f64(),
                    if can_refine {
                        "refined"
                    } else {
                        "FAILED (no slot left)"
                    }
                );
                let st = if can_refine { "refined" } else { "failed" };
                writeln!(pf, "{}", json!({"piece": id, "choice": choice, "status": st, "reason": format!("resident memory {} > {} GB", r, rss_max >> 30), "seconds": tp.elapsed().as_secs_f64()})).unwrap();
                pf.flush().unwrap();
                if can_refine {
                    children(&mut queue);
                }
                continue;
            }
            Ok(Err(e)) => panic!("piece [{id}]: {e}"),
            Ok(Ok(cp)) => cp,
        };
        let incomplete = cp.flags.iter().any(|f| f.starts_with("not a complete run"));
        if incomplete && can_refine {
            eprintln!(
                "piece [{id}]: partition not complete after {:.0} s ({:?}): refined",
                tp.elapsed().as_secs_f64(),
                cp.flags
            );
            writeln!(pf, "{}", json!({"piece": id, "choice": choice, "status": "refined", "reason": cp.flags, "seconds": tp.elapsed().as_secs_f64(), "elements": cp.elements.len()})).unwrap();
            pf.flush().unwrap();
            drop(cp);
            trim_heap();
            children(&mut queue);
            continue;
        }
        n_run += 1;
        let mut cone_lines = Vec::new();
        for (tag, cone0) in &cones {
            let cone = cone0.clone().and(&piece);
            let cov = coverage_from_partition(&cp, &capped, &frame, &cone, &opts).unwrap();
            let cert = certificate(&cov, &capped, &frame);
            let chk = check_cone_certificate(&cert, &capped, &frame, None);
            let fid = file_id(&id);
            std::fs::write(
                dir.join(format!("{fid}.{tag}.certificate.json")),
                serde_json::to_string(&cert).unwrap(),
            )
            .unwrap();
            let (mut res_keys, mut unsized_cells, mut open_keys) = (0u64, 0usize, 0u64);
            let mut reasons: BTreeMap<String, u64> = BTreeMap::new();
            let mut faces: BTreeMap<String, u64> = BTreeMap::new();
            let mut keys_out: Vec<String> = Vec::new();
            for l in &cov.listed {
                let i = l["element"].as_u64().unwrap() as usize;
                let el = &cp.elements[i];
                if el.kind != "generic" || res_keys >= residual_max {
                    unsized_cells += 1;
                    continue;
                }
                let mut keys_here = Vec::new();
                match cell_keys_each(&el.region(), &cone, &frame, 1_000_000, &mut |k| {
                    keys_here.push(k.to_vec())
                }) {
                    Some(()) => {
                        for k in keys_here {
                            res_keys += 1;
                            let d = match &g5 {
                                Some(g) => format!("{:?}", g.first_match(&k)),
                                None => format!("{:?}", capped.first_match(&k)),
                            };
                            let why = if d.starts_with("Ok(Covered") {
                                "covered".to_string()
                            } else if let Some(p) = d.find("reason: ") {
                                let r: String = d[p + 8..]
                                    .chars()
                                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                                    .collect();
                                format!("blocked:{r}")
                            } else {
                                d.chars().take(40).collect()
                            };
                            let open = why != "covered";
                            if open {
                                open_keys += 1;
                                *faces
                                    .entry(format!("{:?}", face_of(&lines, &k)))
                                    .or_default() += 1;
                            }
                            *reasons.entry(why.clone()).or_default() += 1;
                            keys_out.push(
                                json!({"key": k, "element": i, "open": open, "dispatch": why})
                                    .to_string(),
                            );
                        }
                    }
                    None => unsized_cells += 1,
                }
            }
            if !keys_out.is_empty() {
                std::fs::write(
                    dir.join(format!("{fid}.{tag}.cellkeys.jsonl")),
                    keys_out.join("\n") + "\n",
                )
                .unwrap();
            }
            cone_lines.push(json!({
                "cone": tag, "verdict": cov.verdict, "elements": cov.elements, "outside_cone": cov.outside,
                "uncovered": cov.uncovered, "undecided": cov.undecided,
                "residual": {"cell_keys_in_cone": res_keys, "open": open_keys, "cells_not_sized": unsized_cells, "by_dispatch": reasons, "open_by_face": faces},
                "probes": cov.probes.len(), "probes_open_unexplained": cov.probes_open_unexplained,
                "checker": {"ok": chk.ok(), "n_problems": chk.problems.len(), "problems": chk.problems.iter().take(10).collect::<Vec<_>>(), "witnesses_checked": chk.witnesses_checked, "probes_checked": chk.probes_checked},
            }));
        }
        let pk = peak.get();
        eprintln!(
            "piece [{id}] done in {:.0} s: {} elements, peak rss {:.1} GB, flags {:?}; {}",
            tp.elapsed().as_secs_f64(),
            cp.elements.len(),
            pk as f64 / 1e9,
            cp.flags,
            cone_lines
                .iter()
                .map(|c| format!(
                    "{} {} {}/{}",
                    c["cone"].as_str().unwrap(),
                    c["verdict"].as_str().unwrap_or("?"),
                    c["uncovered"],
                    c["undecided"]
                ))
                .collect::<Vec<_>>()
                .join("; ")
        );
        writeln!(pf, "{}", json!({"piece": id, "choice": choice, "status": "done", "seconds": tp.elapsed().as_secs_f64(),
            "partition_seconds": cp.millis as f64 / 1e3, "elements": cp.elements.len(), "flags": cp.flags,
            "rss_start": rss0, "rss_peak": pk, "cones": cone_lines})).unwrap();
        pf.flush().unwrap();
        drop(cp);
        drop(capped);
        trim_heap();
    }
    drop(pf);
    // aggregate every finished piece of the file (this run and earlier ones)
    let text = std::fs::read_to_string(&lines_path).unwrap();
    let mut last_of: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        last_of.insert(v["piece"].as_str().unwrap().to_string(), v);
    }
    let done: Vec<&serde_json::Value> =
        last_of.values().filter(|v| v["status"] == "done").collect();
    let failed: Vec<&serde_json::Value> = last_of
        .values()
        .filter(|v| v["status"] == "failed")
        .collect();
    let refined = last_of
        .values()
        .filter(|v| v["status"] == "refined")
        .count();
    let mut per_cone = Vec::new();
    for (tag, cone) in &cones {
        let (mut unc, mut und, mut keys, mut open) = (0u64, 0u64, 0u64, 0u64);
        let (mut cov_pieces, mut unc_pieces, mut und_pieces, mut chk_bad) = (0, 0, 0, 0);
        let mut reasons: BTreeMap<String, u64> = BTreeMap::new();
        let mut faces: BTreeMap<String, u64> = BTreeMap::new();
        let mut incomplete_pieces = BTreeSet::new();
        for v in &done {
            let Some(c) = v["cones"]
                .as_array()
                .and_then(|a| a.iter().find(|c| c["cone"] == tag.as_str()))
            else {
                continue;
            };
            unc += c["uncovered"].as_u64().unwrap_or(0);
            und += c["undecided"].as_u64().unwrap_or(0);
            keys += c["residual"]["cell_keys_in_cone"].as_u64().unwrap_or(0);
            open += c["residual"]["open"].as_u64().unwrap_or(0);
            match c["verdict"].as_str() {
                Some("covered") => cov_pieces += 1,
                Some("uncovered") => unc_pieces += 1,
                _ => und_pieces += 1,
            }
            if c["checker"]["ok"] != true {
                chk_bad += 1;
            }
            if v["flags"].as_array().is_some_and(|a| !a.is_empty()) {
                incomplete_pieces.insert(v["piece"].as_str().unwrap().to_string());
            }
            for (k, x) in c["residual"]["by_dispatch"]
                .as_object()
                .into_iter()
                .flatten()
            {
                *reasons.entry(k.clone()).or_default() += x.as_u64().unwrap_or(0);
            }
            for (k, x) in c["residual"]["open_by_face"]
                .as_object()
                .into_iter()
                .flatten()
            {
                *faces.entry(k.clone()).or_default() += x.as_u64().unwrap_or(0);
            }
        }
        per_cone.push(json!({"cone": tag, "label": cone.label, "pieces_covered": cov_pieces, "pieces_uncovered": unc_pieces,
            "pieces_undecided": und_pieces, "uncovered_elements": unc, "undecided_elements": und,
            "residual_cell_keys": keys, "residual_open_keys": open, "by_dispatch": reasons, "open_by_face": faces,
            "pieces_with_flags": incomplete_pieces, "checker_not_ok": chk_bad,
            "verdict": if !failed.is_empty() { "undecided (failed pieces)" } else if und_pieces > 0 { "undecided" } else if unc_pieces > 0 { "uncovered" } else { "covered" }}));
    }
    let head = json!({
        "schema": rustred_campaign_core::parametric::CONE_COVERAGE_SCHEMA, "version": rustred_campaign_core::parametric::CONE_COVERAGE_VERSION,
        "mode": "face by face (capped partition per face piece)", "piece_slots": slots, "initial_depth": depth,
        "spec": {"path": spec_path.display().to_string(), "sha256": sha256_file(&spec_path)},
        "node": ld.node, "loader": ld.loader, "tables": ld.tables, "records": ld.jobs.len(),
        "restored_loci": restored, "fall_through": ft_mode, "fall_through_stats": ft_stats, "factor_exceptions": opts.factor_exceptions,
        "budget_per_piece": {"units": units, "wall_seconds": wall, "max_memo": opts.budget.max_memo, "rss_gb": rss_max >> 30},
        "pieces": {"done": done.len(), "refined": refined, "failed": failed.iter().map(|v| v["piece"].clone()).collect::<Vec<_>>(), "run_now": n_run},
    });
    std::fs::write(
        format!("{out}.summary.json"),
        serde_json::to_string_pretty(&json!({"header": head, "cones": per_cone})).unwrap(),
    )
    .unwrap();
    eprintln!(
        "face by face: {} pieces done ({} now), {} refined, {} failed; {}",
        done.len(),
        n_run,
        refined,
        failed.len(),
        per_cone
            .iter()
            .map(|c| format!(
                "{}: {} ({} uncovered / {} undecided elements, {} open residual keys)",
                c["cone"].as_str().unwrap(),
                c["verdict"].as_str().unwrap(),
                c["uncovered_elements"],
                c["undecided_elements"],
                c["residual_open_keys"]
            ))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

/// Decision A, T178 APPEND variant (COORDINATOR-ANSWERS-20261116-1.md): the elided records appended AFTER the dispatch
/// change no statement key. Loads a sources list (PARAMETRIC_SPEC = `*sources.json`, run with PARAMETRIC_APPENDED=apply),
/// builds the dispatch WITHOUT the appended sources (table names ending in "(appended elided)") and WITH them, both
/// under the decision-G5 fall-through (`cone::fall_through_poles`), and dispatches every key of PARAMETRIC_KEYS (rows
/// `["I(..)", kind, rule, in_box, [D, r]]`; only `in_box` rows unless PARAMETRIC_KEYS_ALL=1). A key CHANGES when the two
/// first matches differ. Output: PARAMETRIC_OUT (JSON).
#[test]
#[ignore]
fn append_variant_changes_no_statement_key() {
    use rustred_campaign_core::cover::Dispatch;
    use std::io::BufRead;
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let keys_p = PathBuf::from(std::env::var("PARAMETRIC_KEYS").expect("PARAMETRIC_KEYS"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let all = std::env::var("PARAMETRIC_KEYS_ALL").as_deref() == Ok("1");
    assert_eq!(
        std::env::var("PARAMETRIC_APPENDED").as_deref(),
        Ok("apply"),
        "run with PARAMETRIC_APPENDED=apply"
    );
    let ld = load_any(&spec, None);
    let with = rustred_campaign_core::parametric::cone::fall_through_poles(&ld.disp).unwrap();
    let appended: Vec<usize> = ld
        .disp
        .tables()
        .iter()
        .enumerate()
        .filter(|(_, t)| t.name.ends_with("(appended elided)"))
        .map(|(i, _)| i)
        .collect();
    let n_app: usize = appended
        .iter()
        .map(|&i| ld.disp.tables()[i].domains.len())
        .sum();
    // the same tables in the same priority, the appended ones removed (Dispatcher::new wants a permutation)
    let kept: Vec<usize> = ld
        .disp
        .priority()
        .iter()
        .copied()
        .filter(|i| !appended.contains(i))
        .collect();
    let tables: Vec<Table> = kept.iter().map(|&i| ld.disp.tables()[i].clone()).collect();
    let base = Dispatcher::new(tables, (0..kept.len()).collect()).unwrap();
    let base = match ld.disp.chart() {
        Some(c) => base.with_chart(c.to_vec()).unwrap(),
        None => base,
    };
    let without = rustred_campaign_core::parametric::cone::fall_through_poles(&base).unwrap();
    let f = std::io::BufReader::with_capacity(1 << 24, std::fs::File::open(&keys_p).unwrap());
    let (mut n, mut same, mut changed, mut open_both) = (0u64, 0u64, 0u64, 0u64);
    let mut by_kind: BTreeMap<String, u64> = BTreeMap::new();
    let mut examples = Vec::new();
    for line in f.lines() {
        let line = line.unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if !all && v[3] != true {
            continue;
        }
        let s = v[0].as_str().unwrap();
        let k: Vec<i32> = s
            .trim_start_matches("I(")
            .trim_end_matches(')')
            .split(',')
            .map(|x| x.trim().parse().unwrap())
            .collect();
        n += 1;
        let a = without.first_match(&k);
        let b = with.first_match(&k);
        let (fa, fb) = (format!("{a:?}"), format!("{b:?}"));
        if fa == fb {
            same += 1;
            if !matches!(a, Ok(Dispatch::Covered(_))) {
                open_both += 1;
                *by_kind
                    .entry(format!(
                        "open in both: statement kind {}",
                        v[1].as_str().unwrap_or("?")
                    ))
                    .or_default() += 1;
            }
        } else {
            changed += 1;
            if examples.len() < 50 {
                examples.push(json!({"key": s, "without": fa, "with": fb, "statement": v}));
            }
        }
    }
    let res = json!({
        "spec": {"path": spec.display().to_string(), "sha256": sha256_file(&spec)},
        "keys": {"path": keys_p.display().to_string(), "only_in_box": !all},
        "appended_tables": appended.len(), "appended_records": n_app, "fall_through": "fall_through_poles (decision G5)",
        "keys_checked": n, "same_first_match": same, "changed": changed, "open_in_both": open_both, "open_by_kind": by_kind,
        "examples": examples,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&res).unwrap()).unwrap();
    eprintln!(
        "append check: {n} keys, {changed} changed, {open_both} open in both; appended {} tables / {n_app} records",
        appended.len()
    );
}
