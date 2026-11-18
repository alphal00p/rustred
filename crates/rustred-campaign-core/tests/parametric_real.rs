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
        let mut full = Table::from_table_file(id.clone(), p.display().to_string(), &tf, opts)
            .unwrap_or_else(|e| panic!("{id} ({}): {e}", p.display()));
        // BOUNDED RECIPE WITH SEAL (source class "sealed_recipe"): every record's domain is its face INTERSECTED with
        // D + r <= bound_dr (a linear bound added to the domain); the seal (every key of that region solved, descent ok) is
        // an input bound by sha256 (refused on mismatch); recorded in the loader notes
        if let Some(bdr) = src["bound_dr"].as_i64() {
            let seal = &src["seal"];
            let sp = seal["path"]
                .as_str()
                .unwrap_or_else(|| panic!("{id}: sealed_recipe without seal.path"));
            let want = seal["sha256"]
                .as_str()
                .unwrap_or_else(|| panic!("{id}: sealed_recipe without seal.sha256"));
            let got = sha256_file(Path::new(sp));
            assert_eq!(
                got, want,
                "{id}: seal {sp} hashes to {got}, the source names {want}"
            );
            let fr = NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap();
            let n = fr.arity();
            // bound_dr + #lines - sum_lines n_i + sum_numerators n_j >= 0
            let nl = (0..n).filter(|&i| fr.is_line(i)).count() as i128;
            let mut b = rustred_campaign_core::poly::Poly::constant(i128::from(bdr) + nl);
            for i in 0..n {
                let v = rustred_campaign_core::poly::Poly::var(i);
                b = if fr.is_line(i) {
                    b.sub(&v).unwrap()
                } else {
                    b.add(&v).unwrap()
                };
            }
            for d in &mut full.domains {
                d.bounds.push(b.clone());
            }
            notes.push(format!(
                "{id}: sealed bounded recipe: D + r <= {bdr}, seal {sp} sha256 {got}"
            ));
        }
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
    let bank_loci = std::env::var("PARAMETRIC_BANK_LOCI")
        .ok()
        .map(|f| restore_bank_loci(&mut ld, &f));
    if let Some(b) = &bank_loci {
        eprintln!("bank witness loci: {b}");
    }
    // PARAMETRIC_ENGINE_ORDER=1: certify the ENGINE's two-phase order (ordinary records, then slot aliases; TRICKS 43)
    let engine_order_info = (std::env::var("PARAMETRIC_ENGINE_ORDER").as_deref() == Ok("1"))
        .then(|| engine_order(&mut ld));
    if let Some(e) = &engine_order_info {
        eprintln!("engine order: {e}");
    }
    // PARAMETRIC_NOTCARRIED=engine: records whose source carries NO witness loci (status NotCarried: T162's converted
    // bank tables) are treated as the ENGINE treats them (its matcher never tests witness loci; it writes their
    // vanishing sets into the exceptions): their witness check is disabled (Assurance::CheckDisabled, labelled in the
    // evidence). Default: the core's fail-closed block (every key where such a record is selected is open).
    let mut notcarried_disabled = 0usize;
    let elided_engine = std::env::var("PARAMETRIC_ELIDED").as_deref() == Ok("engine");
    if std::env::var("PARAMETRIC_NOTCARRIED").as_deref() == Ok("engine") || elided_engine {
        // PARAMETRIC_ELIDED=engine: records whose witness loci are ELIDED are applied as the engine applies them
        // (its matcher never tests witness loci); labelled in the evidence (notcarried_witness_check_disabled counts both)
        use rustred_campaign_core::cover::{Assurance, WitnessStatus};
        let nc = std::env::var("PARAMETRIC_NOTCARRIED").as_deref() == Ok("engine");
        let mut tables = ld.disp.tables().to_vec();
        for t in &mut tables {
            for (m, d) in t.meta.iter_mut().zip(t.domains.iter_mut()) {
                if (nc && m.witness == WitnessStatus::NotCarried)
                    || (elided_engine && m.witness == WitnessStatus::Elided)
                {
                    m.witness_basis = Assurance::CheckDisabled;
                    d.witness_elided = false;
                    notcarried_disabled += 1;
                }
            }
        }
        let d = Dispatcher::new(tables, ld.disp.priority().to_vec()).unwrap();
        ld.disp = match ld.disp.chart() {
            Some(c) => d.with_chart(c.to_vec()).unwrap(),
            None => d,
        };
        eprintln!(
            "NotCarried records with the witness check disabled (engine semantics): {notcarried_disabled}"
        );
    }
    // decision G5 fall-through: "1" = every pole locus moved into the exceptions; "reduced" = loci proved disjoint
    // from their record's selected set dropped, the others moved (cone::fall_through_poles_reduced; same dispatch)
    // "core+g5": the partition runs on the CORE dispatch (a pole = stop set, blocked sub-partitions; equal to G5 on
    // every locus proved disjoint from its record's selected set, a superset of the G5-uncovered set on the others),
    // every residual key is re-dispatched with the exact G5 dispatcher (reduced fall-through)
    let ft_mode = std::env::var("PARAMETRIC_FALLTHROUGH").unwrap_or_default();
    let fall_through = ft_mode == "1" || ft_mode == "reduced";
    let fell;
    let mut ft_stats = serde_json::Value::Null;
    // PARAMETRIC_REDISPATCH_SPEC: the residual keys are judged by ANOTHER dispatcher (sources list / import spec; same
    // restored loci, NotCarried mode and reduced G5 fall-through), e.g. the catalogue + services on top of a cheaper
    // partition; recorded in the evidence
    let redispatch_spec = std::env::var("PARAMETRIC_REDISPATCH_SPEC").ok();
    let g5: Option<Dispatcher> = if let Some(rs) = &redispatch_spec {
        let mut l2 = load_any(Path::new(rs), None);
        if let Ok(f) = std::env::var("PARAMETRIC_SUPPLIED_LOCI") {
            restore_loci(&mut l2, &f);
        }
        if let Ok(f) = std::env::var("PARAMETRIC_BANK_LOCI") {
            eprintln!(
                "re-dispatcher bank witness loci: {}",
                restore_bank_loci(&mut l2, &f)
            );
        }
        if std::env::var("PARAMETRIC_NOTCARRIED").as_deref() == Ok("engine") {
            use rustred_campaign_core::cover::{Assurance, WitnessStatus};
            let mut tables = l2.disp.tables().to_vec();
            for t in &mut tables {
                for m in &mut t.meta {
                    if m.witness == WitnessStatus::NotCarried {
                        m.witness_basis = Assurance::CheckDisabled;
                    }
                }
            }
            let d = Dispatcher::new(tables, l2.disp.priority().to_vec()).unwrap();
            l2.disp = match l2.disp.chart() {
                Some(c) => d.with_chart(c.to_vec()).unwrap(),
                None => d,
            };
        }
        let (d, st) = rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
            &l2.disp,
            &l2.frame,
            &ProofBudget::default(),
        )
        .unwrap();
        eprintln!(
            "re-dispatcher {rs}: {} records ({}); reduced fall-through {}/{} redundant",
            l2.jobs.len(),
            l2.loader,
            st.redundant,
            st.poles
        );
        Some(d)
    } else if ft_mode == "core+g5" {
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
    // ENGINE MODEL keyed by engine id (recorded in every certificate): "line7d" (default: frozen engine-line7d, two-prime zero
    // test, its misreads modelled as modular-zero-risk stop pieces) or "4895ee68" (rustred slot-alias 4895ee68: exact zero
    // test via da5d13da, slot aliases in table order: no modular stop pieces)
    let engine_id = std::env::var("PARAMETRIC_ENGINE").unwrap_or_else(|_| "line7d".into());
    let engine_model = match engine_id.as_str() {
        "line7d" => {
            json!({"id": "frozen engine-line7d (bin/frozen/20261115T0652Z-engine-line7d)", "zero_test": "two primes 2^61-1, 2^61-31 (misreads modelled as modular-zero-risk stop pieces)", "alias_order": "table order (no alias records)"})
        }
        "4895ee68" => {
            opts.engine_exact_zero = true;
            json!({"id": "rustred slot-alias 4895ee68 (2746b4c5 alias order, da5d13da exact zero test)", "zero_test": "exact: two primes, then big rationals when both residues vanish (no modular stop piece)", "alias_order": "slot-permuted alias records in table order"})
        }
        other => panic!("PARAMETRIC_ENGINE {other}: unknown engine id (line7d | 4895ee68)"),
    };
    // affine pieces: the loci resolver makes the pieces' equations explicit (one parameter fewer per equation)
    opts.reduce_equalities = std::env::var("PARAMETRIC_PIECE_EQS").is_ok();
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
    // PARAMETRIC_TERMINALS=<file>: the TERMINAL SET as an explicit input of every certificate (TRICKS 34): a JSON list of
    // keys or JSONL rows {"key": [...]}; written into each certificate (keys + sha256) and every cone reports the open
    // keys outside it (the statement is "open set = terminal set inside the cone")
    let terminals: Option<(BTreeSet<Vec<i32>>, serde_json::Value)> =
        std::env::var("PARAMETRIC_TERMINALS").ok().map(|p| {
            let txt = std::fs::read_to_string(&p).unwrap();
            let parsed = serde_json::from_str::<serde_json::Value>(&txt);
            // a terminal-set document {"keys": [...]} (rustred.campaign.terminal-set.v1), a JSON list, or JSONL rows
            let parsed = match parsed {
                Ok(serde_json::Value::Object(o)) if o.get("keys").is_some_and(|k| k.is_array()) => {
                    Ok(o["keys"].clone())
                }
                other => other,
            };
            let keys: Vec<Vec<i32>> = match parsed {
                Ok(serde_json::Value::Array(a)) => a
                    .iter()
                    .map(|k| {
                        serde_json::from_value(k.get("key").cloned().unwrap_or(k.clone())).unwrap()
                    })
                    .collect(),
                _ => txt
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| {
                        let v: serde_json::Value = serde_json::from_str(l).unwrap();
                        serde_json::from_value(v.get("key").cloned().unwrap_or(v)).unwrap()
                    })
                    .collect(),
            };
            let meta = json!({"path": p, "sha256": sha256_file(Path::new(&p)), "keys": keys});
            (keys.into_iter().collect(), meta)
        });
    let mut queue: VecDeque<Vec<(usize, bool)>> = VecDeque::new();
    // PARAMETRIC_PIECE_LIST="1,2,8;2,6;0,5,9;...": explicit FACES instead of the split (each piece = the listed slots
    // beyond their boundary value: line >= 2 / numerator <= -1, EVERY other slot at its boundary value: line = 1 /
    // numerator = 0); "none" = the corner
    if let Ok(list) = std::env::var("PARAMETRIC_PIECE_LIST") {
        for face in list.split(';').filter(|x| !x.trim().is_empty()) {
            let beyond: BTreeSet<usize> = face
                .split(',')
                .filter(|x| !x.trim().is_empty() && x.trim() != "none")
                .map(|x| x.trim().parse().unwrap())
                .collect();
            queue.push_back(
                (0..frame.arity())
                    .map(|s| (s, beyond.contains(&s)))
                    .collect(),
            );
        }
    } else {
        for code in 0..(1usize << depth) {
            queue.push_back(
                (0..depth)
                    .map(|j| (slots[j], (code >> (depth - 1 - j)) & 1 == 1))
                    .collect(),
            );
        }
    }
    let mut n_run = 0usize;
    while let Some(choice) = queue.pop_front() {
        let mut piece = face_piece(&frame, &choice).expect("piece");
        // PARAMETRIC_CAP_BOX=R: the piece is also capped to the box |k_s - boundary| <= R per slot (lines <= R + 1,
        // numerators >= -R), which contains the cone D + r <= R: keys outside it are served by the cap records, so
        // the partition (and every blocked sub-partition) only describes the box (a radius-R statement)
        if let Ok(rb) = std::env::var("PARAMETRIC_CAP_BOX") {
            let rb: i64 = rb.parse().unwrap();
            for s in 0..frame.arity() {
                let mut a = vec![0i64; frame.arity()];
                if frame.is_line(s) {
                    a[s] = 1;
                    piece.constraints.push((a, rb + 1));
                } else {
                    a[s] = -1;
                    piece.constraints.push((a, rb));
                }
            }
            piece.label = format!("{} [box {rb}]", piece.label);
        }
        // PARAMETRIC_PIECE_EQS="a_0,...,a_{n-1}=b;...": every piece is also cut to the AFFINE SUBSPACE sum_s a_s k_s = b
        // (two opposite constraints per equation): an exact partition of a lattice line / plane of the face, e.g. a
        // common-zero family of a fragile rule's exception branch found by elimination (lane parametric, 2026-11-17)
        if let Ok(eqs) = std::env::var("PARAMETRIC_PIECE_EQS") {
            for eq in eqs.split(';').filter(|x| !x.trim().is_empty()) {
                let (lhs, rhs) = eq.split_once('=').expect("PARAMETRIC_PIECE_EQS: a,..=b");
                let a: Vec<i64> = lhs.split(',').map(|x| x.trim().parse().unwrap()).collect();
                assert_eq!(a.len(), frame.arity(), "PARAMETRIC_PIECE_EQS: arity");
                let b: i64 = rhs.trim().parse().unwrap();
                piece.constraints.push((a.clone(), b));
                piece.constraints.push((a.iter().map(|x| -x).collect(), -b));
            }
            // short label (file names): the equations are hashed; the full text is in the run's log line below
            let h = eqs.bytes().fold(0xcbf2_9ce4_8422_2325u64, |a, b| {
                (a ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
            eprintln!("affine piece eqs {h:016x}: {eqs}");
            piece.label = format!("{} [affine {h:016x}]", piece.label);
        }
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
        if std::env::var("PARAMETRIC_ELEMENT_PROBE").as_deref() == Ok("1") {
            element_probe(&cp, &capped, g5.as_ref(), &frame, &dir, &file_id(&id));
        }
        let mut cone_lines = Vec::new();
        for (tag, cone0) in &cones {
            let cone = cone0.clone().and(&piece);
            let cov = coverage_from_partition(&cp, &capped, &frame, &cone, &opts).unwrap();
            let cert = certificate(&cov, &capped, &frame);
            let chk = check_cone_certificate(&cert, &capped, &frame, None);
            let fid = file_id(&id);
            let mut cert_v = serde_json::to_value(&cert).unwrap();
            if let Some((_, meta)) = &terminals {
                cert_v["terminals"] = meta.clone();
            }
            // the engine model and the completeness rule: a certificate over an incomplete partition (work-budget
            // exhaustion, a skipped blocked sub-partition, any flag) is REFUSED
            cert_v["engine"] = engine_model.clone();
            let status = if cp.flags.is_empty() {
                "complete".to_string()
            } else {
                format!("REFUSED: incomplete partition: {}", cp.flags.join("; "))
            };
            cert_v["certificate_status"] = json!(status);
            std::fs::write(
                dir.join(format!("{fid}.{tag}.certificate.json")),
                serde_json::to_string(&cert_v).unwrap(),
            )
            .unwrap();
            let mut open_not_terminal: Vec<Vec<i32>> = Vec::new();
            let mut open_terminals = 0u64;
            let (mut res_keys, mut unsized_cells, mut open_keys) = (0u64, 0usize, 0u64);
            let mut reasons: BTreeMap<String, u64> = BTreeMap::new();
            let mut faces: BTreeMap<String, u64> = BTreeMap::new();
            let mut keys_out: Vec<String> = Vec::new();
            // a key can lie in several listed elements (main and blocked sub-partition elements overlap): every count is in
            // DISTINCT keys (TRICKS 39); duplicate rows are counted apart
            let mut seen_keys: std::collections::HashSet<Vec<i32>> =
                std::collections::HashSet::new();
            let mut duplicate_rows = 0u64;
            for l in &cov.listed {
                let i = l["element"].as_u64().unwrap() as usize;
                let el = &cp.elements[i];
                if el.kind == "unexplored" || res_keys >= residual_max {
                    unsized_cells += 1;
                    continue;
                }
                let mut keys_here = Vec::new();
                let mut push = |k: &[i32]| keys_here.push(k.to_vec());
                let sized = if el.cell.is_some() {
                    cell_keys_each(&el.region(), &cone, &frame, 1_000_000, &mut push)
                } else {
                    rustred_campaign_core::parametric::cone::region_keys_each(
                        &el.region(),
                        &cone,
                        1_000_000,
                        &mut push,
                    )
                };
                match sized {
                    Some(()) => {
                        for k in keys_here {
                            if !seen_keys.insert(k.clone()) {
                                duplicate_rows += 1;
                                continue;
                            }
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
                            if open && let Some((tset, _)) = &terminals {
                                if tset.contains(&k) {
                                    open_terminals += 1;
                                } else if open_not_terminal.len() < 1000 {
                                    open_not_terminal.push(k.clone());
                                }
                            }
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
                "cone": tag, "certificate_status": status, "engine": engine_model["id"], "verdict": cov.verdict, "elements": cov.elements, "outside_cone": cov.outside,
                "uncovered": cov.uncovered, "undecided": cov.undecided,
                "residual": {"cell_keys_in_cone": res_keys, "duplicate_rows_skipped": duplicate_rows, "open": open_keys, "cells_not_sized": unsized_cells, "by_dispatch": reasons, "open_by_face": faces},
                "terminals": terminals.as_ref().map(|(t, m)| json!({"terminal_set_sha256": m["sha256"], "terminal_keys": t.len(),
                    "open_terminal_keys": open_terminals, "open_keys_not_terminal": open_not_terminal.len(), "open_not_terminal_first": open_not_terminal.iter().take(20).collect::<Vec<_>>(),
                    "open_set_equals_terminals_in_cone": open_not_terminal.is_empty() && unsized_cells == 0 && cov.undecided == 0})),
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
        "notcarried_witness_check_disabled": notcarried_disabled, "elided_engine": elided_engine, "residual_redispatch_spec": redispatch_spec,
        "bank_witness_loci": bank_loci, "engine_order": engine_order_info,
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

/// Exact KEY-LEVEL re-dispatch: every key of PARAMETRIC_KEYS (a `*.cellkeys.jsonl`, rows `{"key", "open"}`; only open
/// rows unless PARAMETRIC_KEYS_ALL=1) through the dispatcher of PARAMETRIC_SPEC (sources list or import spec), with
/// PARAMETRIC_SUPPLIED_LOCI, PARAMETRIC_NOTCARRIED=engine and the decision-G5 reduced fall-through applied as in
/// `b2_face_by_face`. Output PARAMETRIC_OUT (JSON): counts by dispatch reason and by face of the keys left open.
#[test]
#[ignore]
fn redispatch_keys() {
    use rustred_campaign_core::cover::Dispatch;
    use rustred_campaign_core::partition::face_of;
    use std::io::BufRead;
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let keys_p = PathBuf::from(std::env::var("PARAMETRIC_KEYS").expect("PARAMETRIC_KEYS"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let all = std::env::var("PARAMETRIC_KEYS_ALL").as_deref() == Ok("1");
    let mut ld = load_any(
        &spec,
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
    let bank_loci = std::env::var("PARAMETRIC_BANK_LOCI")
        .ok()
        .map(|f| restore_bank_loci(&mut ld, &f));
    if let Some(b) = &bank_loci {
        eprintln!("bank witness loci: {b}");
    }
    if std::env::var("PARAMETRIC_ENGINE_ORDER").as_deref() == Ok("1") {
        eprintln!("engine order: {}", engine_order(&mut ld));
    }
    let mut notcarried = 0usize;
    if std::env::var("PARAMETRIC_NOTCARRIED").as_deref() == Ok("engine") {
        use rustred_campaign_core::cover::{Assurance, WitnessStatus};
        let mut tables = ld.disp.tables().to_vec();
        for t in &mut tables {
            for m in &mut t.meta {
                if m.witness == WitnessStatus::NotCarried {
                    m.witness_basis = Assurance::CheckDisabled;
                    notcarried += 1;
                }
            }
        }
        let d = Dispatcher::new(tables, ld.disp.priority().to_vec()).unwrap();
        ld.disp = match ld.disp.chart() {
            Some(c) => d.with_chart(c.to_vec()).unwrap(),
            None => d,
        };
    }
    let (g5, st) = rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
        &ld.disp,
        &ld.frame,
        &ProofBudget::default(),
    )
    .unwrap();
    let lines: Vec<bool> = (0..ld.frame.arity()).map(|i| ld.frame.is_line(i)).collect();
    // BINARY key lists (lane arrivals' RRKEYS01: magic, u32 LE header length, header JSON, then per key arity x i8 + u8 depth
    // + u8 senders): streamed in blocks over PARAMETRIC_THREADS threads; counts only (no per-key file)
    if keys_p.extension().is_some_and(|e| e == "bin") {
        use std::io::Read;
        let mut f =
            std::io::BufReader::with_capacity(1 << 24, std::fs::File::open(&keys_p).unwrap());
        let mut magic = [0u8; 8];
        f.read_exact(&mut magic).unwrap();
        assert_eq!(&magic, b"RRKEYS01", "not an RRKEYS01 file");
        let mut hl = [0u8; 4];
        f.read_exact(&mut hl).unwrap();
        let mut hdr = vec![0u8; u32::from_le_bytes(hl) as usize];
        f.read_exact(&mut hdr).unwrap();
        let header: serde_json::Value = serde_json::from_slice(&hdr).unwrap();
        let n = ld.frame.arity();
        let rec = n + 2;
        let threads: usize = std::env::var("PARAMETRIC_THREADS")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(16);
        let block = 1usize << 20;
        let mut served: BTreeMap<String, u64> = BTreeMap::new();
        let mut reasons: BTreeMap<String, u64> = BTreeMap::new();
        let mut open_faces: BTreeMap<String, u64> = BTreeMap::new();
        let mut open_first: Vec<serde_json::Value> = Vec::new();
        let (mut total, mut open) = (0u64, 0u64);
        // PARAMETRIC_DUMP_SERVED (JSON array of served names "table#label") + PARAMETRIC_DUMP_OUT (JSONL): every key whose first
        // match is one of these records is written with its ordinal in the key file, its chain depth and the serving record.
        let dump_set: std::collections::BTreeSet<String> = std::env::var("PARAMETRIC_DUMP_SERVED")
            .ok()
            .map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap())
            .unwrap_or_default();
        let mut dump_w = std::env::var("PARAMETRIC_DUMP_OUT")
            .ok()
            .map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
        let mut dumped = 0u64;
        let t0 = std::time::Instant::now();
        let mut buf = vec![0u8; block * rec];
        loop {
            let mut got = 0usize;
            while got < buf.len() {
                let k = f.read(&mut buf[got..]).unwrap();
                if k == 0 {
                    break;
                }
                got += k;
            }
            if got == 0 {
                break;
            }
            assert_eq!(got % rec, 0, "truncated record");
            let nk = got / rec;
            let chunk = nk.div_ceil(threads);
            type Part = (
                BTreeMap<String, u64>,
                BTreeMap<String, u64>,
                BTreeMap<String, u64>,
                Vec<serde_json::Value>,
                u64,
                Vec<serde_json::Value>,
            );
            let base = total;
            let parts: Vec<Part> = std::thread::scope(|sc| {
                let hs: Vec<_> = (0..threads)
                    .map(|t| {
                        let lo = (t * chunk).min(nk);
                        let hi = ((t + 1) * chunk).min(nk);
                        let data = &buf[lo * rec..hi * rec];
                        let g5 = &g5;
                        let lines = &lines;
                        let dump_set = &dump_set;
                        sc.spawn(move || {
                            let (mut sv, mut rs, mut of, mut ofirst, mut op, mut dp) = (
                                BTreeMap::new(),
                                BTreeMap::new(),
                                BTreeMap::new(),
                                Vec::new(),
                                0u64,
                                Vec::new(),
                            );
                            for (i, r) in data.chunks_exact(rec).enumerate() {
                                let k: Vec<i32> =
                                    r[..n].iter().map(|&b| i32::from(b as i8)).collect();
                                match g5.first_match(&k) {
                                    Ok(Dispatch::Covered(rr)) => {
                                        let name = format!(
                                            "{}#{}",
                                            g5.tables()[rr.table].name,
                                            g5.tables()[rr.table].labels[rr.record]
                                        );
                                        if dump_set.contains(&name) {
                                            dp.push(json!({"ordinal": base + (lo + i) as u64, "key": k, "depth": r[n], "served": name}));
                                        }
                                        *sv.entry(name).or_insert(0u64) += 1;
                                    }
                                    other => {
                                        op += 1;
                                        let s = format!("{other:?}");
                                        let why = match s.find("reason: ") {
                                            Some(p) => format!(
                                                    "blocked:{}",
                                                    s[p + 8..]
                                                        .chars()
                                                        .take_while(
                                                            |c| c.is_alphanumeric() || *c == '_'
                                                        )
                                                        .collect::<String>()
                                                ),
                                            None => s.chars().take(40).collect(),
                                        };
                                        *rs.entry(why.clone()).or_insert(0u64) += 1;
                                        *of.entry(format!("{:?}", face_of(lines, &k)))
                                            .or_insert(0u64) += 1;
                                        if ofirst.len() < 200 {
                                            ofirst.push(json!({"key": k, "why": why}));
                                        }
                                    }
                                }
                            }
                            (sv, rs, of, ofirst, op, dp)
                        })
                    })
                    .collect();
                hs.into_iter().map(|h| h.join().unwrap()).collect()
            });
            for (sv, rs, of, ofirst, op, dp) in parts {
                if let Some(w) = dump_w.as_mut() {
                    use std::io::Write;
                    for x in &dp {
                        writeln!(w, "{x}").unwrap();
                    }
                    dumped += dp.len() as u64;
                }
                for (k, v) in sv {
                    *served.entry(k).or_default() += v;
                }
                for (k, v) in rs {
                    *reasons.entry(k).or_default() += v;
                }
                for (k, v) in of {
                    *open_faces.entry(k).or_default() += v;
                }
                for x in ofirst {
                    if open_first.len() < 2000 {
                        open_first.push(x);
                    }
                }
                open += op;
            }
            total += nk as u64;
            if total % (16 * block as u64) < block as u64 {
                eprintln!(
                    "  {total} keys, {open} open, {:.0} s",
                    t0.elapsed().as_secs_f64()
                );
            }
        }
        let res = json!({
            "spec": {"path": spec.display().to_string(), "sha256": sha256_file(&spec)}, "loader": ld.loader,
            "keys": {"path": keys_p.display().to_string(), "format": "RRKEYS01", "header": header},
            "restored_loci": restored, "bank_witness_loci": bank_loci, "fall_through": "reduced (decision G5)",
            "keys_checked": total, "open": open, "open_by_dispatch": reasons, "open_by_face": open_faces,
            "open_keys_first_2000": open_first, "covered_by_record": served, "seconds": t0.elapsed().as_secs_f64(),
            "threads": threads, "dump": {"served": dump_set, "out": std::env::var("PARAMETRIC_DUMP_OUT").ok(), "keys": dumped},
        });
        if let Some(mut w) = dump_w {
            use std::io::Write;
            w.flush().unwrap();
        }
        std::fs::write(&out, serde_json::to_string_pretty(&res).unwrap()).unwrap();
        eprintln!("redispatch (binary): {total} keys, {open} open");
        return;
    }
    let f = std::io::BufReader::new(std::fs::File::open(&keys_p).unwrap());
    let (mut n, mut open) = (0u64, 0u64);
    let mut reasons: BTreeMap<String, u64> = BTreeMap::new();
    let mut faces: BTreeMap<String, u64> = BTreeMap::new();
    let mut served_by_table: BTreeMap<String, u64> = BTreeMap::new();
    let mut served_by_record: BTreeMap<String, u64> = BTreeMap::new();
    let mut open_keys: Vec<serde_json::Value> = Vec::new();
    // per key: the serving record (table#label) or the open reason, in input order
    let mut per_key =
        std::io::BufWriter::new(std::fs::File::create(format!("{out}.keys.jsonl")).unwrap());
    for line in f.lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if !all && v["open"] != true {
            continue;
        }
        let k: Vec<i32> = v["key"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_i64().unwrap() as i32)
            .collect();
        n += 1;
        let d = g5.first_match(&k);
        let why = match &d {
            Ok(Dispatch::Covered(r)) => {
                *served_by_table
                    .entry(g5.tables()[r.table].name.clone())
                    .or_default() += 1;
                *served_by_record
                    .entry(format!(
                        "{}#{}",
                        g5.tables()[r.table].name,
                        g5.tables()[r.table].labels[r.record]
                    ))
                    .or_default() += 1;
                "covered".to_string()
            }
            other => {
                let s = format!("{other:?}");
                match s.find("reason: ") {
                    Some(p) => format!(
                        "blocked:{}",
                        s[p + 8..]
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect::<String>()
                    ),
                    None => s.chars().take(40).collect(),
                }
            }
        };
        {
            use std::io::Write;
            let rec = |r: &rustred_campaign_core::cover::RuleRef| {
                json!(format!(
                    "{}#{}",
                    g5.tables()[r.table].name,
                    g5.tables()[r.table].labels[r.record]
                ))
            };
            let (served, at) = match &d {
                Ok(Dispatch::Covered(r)) => (rec(r), serde_json::Value::Null),
                Ok(Dispatch::Blocked { at, .. }) | Ok(Dispatch::Undecided { at, .. }) => {
                    (serde_json::Value::Null, rec(at))
                }
                _ => (serde_json::Value::Null, serde_json::Value::Null),
            };
            writeln!(
                per_key,
                "{}",
                json!({"key": k, "served": served, "why": why, "at": at})
            )
            .unwrap();
        }
        if why != "covered" {
            open += 1;
            if open_keys.len() < 2000 {
                open_keys.push(json!({"key": k, "why": why}));
            }
            *faces
                .entry(format!("{:?}", face_of(&lines, &k)))
                .or_default() += 1;
        }
        *reasons.entry(why).or_default() += 1;
    }
    let res = json!({
        "spec": {"path": spec.display().to_string(), "sha256": sha256_file(&spec)}, "loader": ld.loader,
        "keys": {"path": keys_p.display().to_string(), "sha256": sha256_file(&keys_p), "only_open_rows": !all},
        "restored_loci": restored, "notcarried_witness_check_disabled": notcarried, "fall_through": "reduced (decision G5)",
        "fall_through_stats": {"poles": st.poles, "redundant": st.redundant, "moved_unknown": st.moved_unknown, "moved_nonempty": st.moved_nonempty},
        "keys_checked": n, "open": open, "by_dispatch": reasons, "open_by_face": faces, "covered_by_table": served_by_table,
        "covered_by_record": served_by_record, "open_keys_first_2000": open_keys, "bank_witness_loci": bank_loci,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&res).unwrap()).unwrap();
    eprintln!("redispatch: {n} keys, {open} open");
}

/// The element probe (ANSWER-UNBOXED section 5 / T178-UNBOXED item A): one JSONL line per gap element of the partition:
/// face (dotted lines / nonzero numerators / free slots), unbounded directions, minimal D + r, keys inside the probe
/// cone (PARAMETRIC_PROBE_CONE, default simplex:33), for a blocked element its blocker (table, label), stop piece and
/// whether the blocker APPLIES on the whole element (`box_inside_domain`, exact), the first record of the judging
/// dispatcher (the re-dispatcher if given, else the partitioned one) that applies on the whole element
/// (PARAMETRIC_PROBE_CONTAIN=1), and the judging dispatch of sample keys beyond the cone (D + r = 34, 45, 60, 100 and
/// every unbounded slot pushed by 40).
fn element_probe(
    cp: &rustred_campaign_core::parametric::cone::ConePartition,
    disp: &Dispatcher,
    judge: Option<&Dispatcher>,
    frame: &NodeFrame,
    dir: &Path,
    fid: &str,
) {
    use rustred_campaign_core::cover::Dispatch;
    use rustred_campaign_core::parametric::cone::{
        box_inside_domain, cell_keys_each, region_keys_each, sample_keys_beyond, slot_box,
    };
    use rustred_campaign_core::partition::{DispatcherSource, DomainSource};
    let judge = judge.unwrap_or(disp);
    let n = frame.arity();
    let cone = parse_cone(
        frame,
        &std::env::var("PARAMETRIC_PROBE_CONE").unwrap_or_else(|_| "simplex:33".into()),
    );
    let contain = std::env::var("PARAMETRIC_PROBE_CONTAIN").as_deref() == Ok("1");
    let budget = ProofBudget {
        max_nodes: 5_000,
        ..ProofBudget::default()
    };
    let src = DispatcherSource::new(disp);
    let jsrc = DispatcherSource::new(judge);
    let describe = |d: &Result<Dispatch, _>, j: &Dispatcher| -> String {
        match d {
            Ok(Dispatch::Covered(r)) => format!("covered:{}#{}", j.tables()[r.table].name, r.label),
            other => {
                let s = format!("{other:?}");
                match s.find("reason: ") {
                    Some(p) => format!(
                        "blocked:{}",
                        s[p + 8..]
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect::<String>()
                    ),
                    None => s.chars().take(30).collect(),
                }
            }
        }
    };
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(dir.join(format!("{fid}.elements.jsonl"))).unwrap(),
    );
    let t0 = std::time::Instant::now();
    for (i, el) in cp.elements.iter().enumerate() {
        let bx = slot_box(el);
        let (mut dotted, mut nonzero, mut free, mut unbounded) = (vec![], vec![], vec![], vec![]);
        let mut min_excess: Option<i64> = Some(0);
        if let Some(b) = &bx {
            for (s, (l, h)) in b.iter().enumerate() {
                if frame.is_line(s) {
                    let l1 = l.unwrap_or(1).max(1);
                    if l1 >= 2 {
                        dotted.push(s);
                    } else if *h != Some(1) {
                        free.push(s);
                    }
                    if h.is_none() {
                        unbounded.push(s);
                    }
                    min_excess = min_excess.map(|m| m + (l1 - 1));
                } else {
                    let h0 = h.unwrap_or(0).min(0);
                    if h0 <= -1 {
                        nonzero.push(s);
                    } else if *l != Some(0) {
                        free.push(s);
                    }
                    if l.is_none() {
                        unbounded.push(s);
                    }
                    min_excess = min_excess.map(|m| m - h0);
                }
            }
        } else {
            min_excess = None;
        }
        let mut nk = 0u64;
        let sized = if el.cell.is_some() {
            cell_keys_each(&el.region(), &cone, frame, 2_000_000, &mut |_| nk += 1)
        } else {
            region_keys_each(&el.region(), &cone, 2_000_000, &mut |_| nk += 1)
        };
        let later = std::env::var("PARAMETRIC_PROBE_LATER").as_deref() == Ok("1");
        let blocker = el.blocked_by.map(|e| {
            let r = src.rule_ref(e);
            let applies = bx
                .as_ref()
                .and_then(|b| box_inside_domain(b, src.domain(e), frame, &budget));
            let exact_lits = bx.as_ref().and_then(|b| {
                rustred_campaign_core::parametric::cone::literals_exact_on_box(
                    src.domain(e),
                    b,
                    1 << 20,
                )
            });
            // the first LATER record (dispatch order) that applies on the whole element, with its own literal status
            let mut later_rec = serde_json::Value::Null;
            if later
                && applies != Some(true)
                && let Some(b) = &bx
            {
                for j in e + 1..src.len() {
                    if box_inside_domain(b, src.domain(j), frame, &budget) == Some(true) {
                        let rr = src.rule_ref(j);
                        let ex = rustred_campaign_core::parametric::cone::literals_exact_on_box(
                            src.domain(j),
                            b,
                            1 << 20,
                        );
                        later_rec = json!({"source": j, "table": disp.tables()[rr.table].name,
                            "label": rr.label, "literals_exact_on_element": ex});
                        break;
                    }
                }
            }
            json!({"source": e, "table": disp.tables()[r.table].name, "label": r.label,
                   "applies_on_whole_element": applies, "literals_exact_on_element": exact_lits,
                   "later_container": later_rec})
        });
        let mut contained = serde_json::Value::Null;
        let mut contain_undecided = 0usize;
        let mut contain_undecided_list: Vec<serde_json::Value> = Vec::new();
        if contain && let Some(b) = &bx {
            for j in 0..jsrc.len() {
                match box_inside_domain(b, jsrc.domain(j), frame, &budget) {
                    Some(true) => {
                        let r = jsrc.rule_ref(j);
                        let d = jsrc.domain(j);
                        contained = json!({"table": judge.tables()[r.table].name, "label": r.label,
                            "point_rule": d.fixed.iter().all(|x| x.is_some())});
                        break;
                    }
                    None => {
                        contain_undecided += 1;
                        if contain_undecided_list.len() < 6 {
                            let r = jsrc.rule_ref(j);
                            let d = jsrc.domain(j);
                            contain_undecided_list.push(json!({"table": judge.tables()[r.table].name, "label": r.label,
                                "fixed": d.fixed,
                                "equations": d.equations.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                                "bounds": d.bounds.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                                "exceptions": d.exceptions.iter().map(|br| br.iter().map(|p| p.to_string()).collect::<Vec<_>>()).collect::<Vec<_>>()}));
                        }
                    }
                    Some(false) => {}
                }
            }
        }
        let samples: Vec<serde_json::Value> =
            sample_keys_beyond(el, frame, &[34, 45, 60, 100], 2, 40)
                .into_iter()
                .map(|(lab, k)| {
                    let d = judge.first_match(&k);
                    json!({"at": lab, "key": k, "judge": describe(&d, judge)})
                })
                .collect();
        writeln!(
            f,
            "{}",
            json!({"element": i, "kind": el.kind, "blocked_by": blocker, "stop": el.stop,
                "box": bx.as_ref().map(|b| b.iter().map(|(l, h)| json!([l, h])).collect::<Vec<_>>()),
                "dotted": dotted, "nonzero": nonzero, "free": free, "unbounded": unbounded, "min_excess": min_excess,
                "keys_in_probe_cone": if sized.is_some() { json!(nk) } else { json!(null) },
                "contained_by": contained, "contain_undecided_records": contain_undecided, "contain_undecided_list": contain_undecided_list, "samples": samples,
                "arity": n})
        )
        .unwrap();
        if i % 500 == 0 {
            eprintln!(
                "  probe {i}/{} ({:.0} s)",
                cp.elements.len(),
                t0.elapsed().as_secs_f64()
            );
        }
    }
    f.flush().unwrap();
}

/// T178-UNBOXED item A': the stated radius R. For every record of the dispatch (PARAMETRIC_SPEC, PARAMETRIC_APPENDED,
/// PARAMETRIC_FALLTHROUGH = 1 | reduced) its engine modular-zero risk (`partition::nf::modular_risk`): Far radii
/// `(v, r)` (a misread needs |pt[v]| >= r), Points, Anywhere. A key with D + r <= R has every line <= R + 1 and every
/// |numerator| <= R, so |pt[v]| <= R + 1 + |o_v| (o_v = the target offset of parameter v; an unmapped parameter keeps
/// its fixed value). R_record = min over its Far entries of (r - |o_v| - 2); R = min over records. Points / Anywhere
/// risks are listed (a Points risk inside the cone is a stop AT that point). Output PARAMETRIC_OUT (JSON).
#[test]
#[ignore]
fn modular_radii() {
    use rustred_campaign_core::domain::{TargetMap, TargetSlot};
    use rustred_campaign_core::partition::nf::{
        ModularRisk, modular_risk, modular_risk_value_bound,
    };
    // PARAMETRIC_RADIUS_CRITERION=value: the plain value bound (no factor criterion; VERIFIER_SPEC item 4)
    let value_bound = std::env::var("PARAMETRIC_RADIUS_CRITERION").as_deref() == Ok("value");
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let mut ld = load_any(&spec, None);
    // the same restored witness loci as the coverage runs (moved loci become exception members: literals the engine
    // evaluates)
    if let Ok(f) = std::env::var("PARAMETRIC_SUPPLIED_LOCI") {
        eprintln!("restored loci: {}", restore_loci(&mut ld, &f));
    }
    if let Ok(f) = std::env::var("PARAMETRIC_BANK_LOCI") {
        eprintln!("bank witness loci: {}", restore_bank_loci(&mut ld, &f));
    }
    let mode = std::env::var("PARAMETRIC_FALLTHROUGH").unwrap_or_default();
    let disp = match mode.as_str() {
        "1" => rustred_campaign_core::parametric::cone::fall_through_poles(&ld.disp).unwrap(),
        "reduced" => {
            let (d, st) = rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
                &ld.disp,
                &ld.frame,
                &ProofBudget::default(),
            )
            .unwrap();
            eprintln!(
                "reduced fall-through: {}",
                serde_json::to_string(&json!({"poles": st.poles, "redundant": st.redundant,
                    "moved_nonempty": st.moved_nonempty, "moved_unknown": st.moved_unknown}))
                .unwrap()
            );
            d
        }
        _ => {
            let d =
                Dispatcher::new(ld.disp.tables().to_vec(), ld.disp.priority().to_vec()).unwrap();
            match ld.disp.chart() {
                Some(c) => d.with_chart(c.to_vec()).unwrap(),
                None => d,
            }
        }
    };
    let mut recs = Vec::new();
    let mut r_global: Option<(i128, String, i64)> = None;
    let (mut n_far, mut n_points, mut n_any) = (0, 0, 0);
    for &ti in disp.priority() {
        let t = &disp.tables()[ti];
        for (ri, d) in t.domains.iter().enumerate() {
            let risk = if value_bound {
                modular_risk_value_bound(d)
            } else {
                modular_risk(d)
            };
            let Some(risk) = risk else {
                continue;
            };
            let label = t.labels.get(ri).copied().unwrap_or(-1);
            let off = |v: usize| -> Option<i128> {
                match &d.target {
                    TargetMap::Canonical => Some(0),
                    TargetMap::Pattern(slots) => slots.iter().find_map(|s| match *s {
                        TargetSlot::Param { param, offset } if param == v => {
                            Some(i128::from(offset).abs())
                        }
                        _ => None,
                    }),
                }
            };
            match &risk {
                ModularRisk::Far(list) => {
                    n_far += 1;
                    let mut rr: Option<i128> = None;
                    for &(v, r) in list {
                        // unmapped parameter: fixed value (never reaches r if |fixed| < r)
                        let Some(o) = off(v) else { continue };
                        let b = r.abs() - o - 2;
                        rr = Some(rr.map_or(b, |x| x.min(b)));
                    }
                    if let Some(b) = rr
                        && r_global.as_ref().is_none_or(|g| b < g.0)
                    {
                        r_global = Some((b, t.name.clone(), label));
                    }
                    recs.push(json!({"table": t.name, "label": label, "kind": "far", "far": list, "r_record": rr}));
                }
                ModularRisk::Points(p) => {
                    n_points += 1;
                    recs.push(
                        json!({"table": t.name, "label": label, "kind": "points", "points": p}),
                    );
                }
                ModularRisk::Anywhere(w) => {
                    n_any += 1;
                    recs.push(
                        json!({"table": t.name, "label": label, "kind": "anywhere", "why": w}),
                    );
                }
            }
        }
    }
    let res = json!({
        "spec": {"path": spec.display().to_string(), "sha256": sha256_file(&spec)}, "fall_through": mode,
        "records": disp.tables().iter().map(|t| t.domains.len()).sum::<usize>(),
        "records_with_far_risk": n_far, "records_with_point_risk": n_points, "records_with_anywhere_risk": n_any,
        "R": r_global.as_ref().map(|g| g.0), "R_binding_record": r_global.as_ref().map(|g| json!({"table": g.1, "label": g.2})),
        "criterion": if value_bound { "value bound (no factor criterion)" } else { "factor criterion (dispatcher)" }, "rule": "R = min over records and Far entries (v, r) of r - |o_v| - 2: every key with D + r <= R has |pt[v]| <= R + 1 + |o_v| < r",
        "risks": recs,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&res).unwrap()).unwrap();
    eprintln!(
        "modular radii: far {n_far}, points {n_points}, anywhere {n_any}; R = {:?}",
        r_global
    );
}

/// T178-UNBOXED A': exact containment of given BOXES (PARAMETRIC_BOXES: JSON list of per-slot [lo|null, hi|null]) in
/// the records of a dispatch (PARAMETRIC_SPEC, PARAMETRIC_APPENDED, PARAMETRIC_FALLTHROUGH = 1 | reduced): for each box
/// every record that APPLIES on the whole box (`box_inside_domain`, budget PARAMETRIC_BOX_NODES, default 200,000) and the
/// undecided ones with their exception branches; output PARAMETRIC_OUT (JSON).
#[test]
#[ignore]
fn box_containment() {
    use rustred_campaign_core::parametric::cone::{box_inside_domain, literals_exact_on_box};
    use rustred_campaign_core::partition::{DispatcherSource, DomainSource};
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let boxes: Vec<Vec<(Option<i64>, Option<i64>)>> = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("PARAMETRIC_BOXES").expect("PARAMETRIC_BOXES"))
            .unwrap(),
    )
    .unwrap();
    let nodes: u64 = std::env::var("PARAMETRIC_BOX_NODES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);
    let ld = load_any(&spec, None);
    let disp = match std::env::var("PARAMETRIC_FALLTHROUGH").as_deref() {
        Ok("reduced") => {
            rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
                &ld.disp,
                &ld.frame,
                &ProofBudget::default(),
            )
            .unwrap()
            .0
        }
        _ => rustred_campaign_core::parametric::cone::fall_through_poles(&ld.disp).unwrap(),
    };
    let src = DispatcherSource::new(&disp);
    let budget = ProofBudget {
        max_nodes: nodes,
        ..ProofBudget::default()
    };
    let mut res = Vec::new();
    for bx in &boxes {
        let (mut apply, mut undecided) = (Vec::new(), Vec::new());
        for j in 0..src.len() {
            let d = src.domain(j);
            match box_inside_domain(bx, d, &ld.frame, &budget) {
                Some(true) => {
                    let r = src.rule_ref(j);
                    apply.push(
                        json!({"source": j, "table": disp.tables()[r.table].name, "label": r.label,
                        "literals_exact_on_box": literals_exact_on_box(d, bx, 1 << 20)}),
                    );
                }
                None => {
                    let r = src.rule_ref(j);
                    undecided.push(json!({"source": j, "table": disp.tables()[r.table].name, "label": r.label,
                        "fixed": d.fixed, "bounds": d.bounds.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                        "exceptions": d.exceptions.iter().map(|b| b.iter().map(|p| p.to_string()).collect::<Vec<_>>()).collect::<Vec<_>>()}));
                }
                Some(false) => {}
            }
        }
        eprintln!(
            "box: {} records apply on the whole box, {} undecided",
            apply.len(),
            undecided.len()
        );
        res.push(json!({"box": bx, "apply_on_whole_box": apply, "undecided": undecided}));
    }
    std::fs::write(&out, serde_json::to_string_pretty(&res).unwrap()).unwrap();
}

/// T178-UNBOXED item D: the core side of the engine drill. Keys from PARAMETRIC_KEYS (one JSON array per line) are
/// dispatched by the dispatcher of PARAMETRIC_SPEC (sources list; PARAMETRIC_FALLTHROUGH = reduced applies the G5
/// transform, which on a table whose loci are already exception branches drops every pole); one line per key in
/// PARAMETRIC_OUT: {"key", "label" (record label or null), "dispatch"}. Compared with `rr_node first-match` outside.
#[test]
#[ignore]
fn drill_core_first_match() {
    use rustred_campaign_core::cover::Dispatch;
    use std::io::BufRead;
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let keys = std::env::var("PARAMETRIC_KEYS").expect("PARAMETRIC_KEYS");
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let mut ld = load_any(&spec, None);
    let el = std::env::var("PARAMETRIC_ELIDED").as_deref() == Ok("engine");
    let nc = std::env::var("PARAMETRIC_NOTCARRIED").as_deref() == Ok("engine");
    if el || nc {
        use rustred_campaign_core::cover::{Assurance, WitnessStatus};
        let mut tables = ld.disp.tables().to_vec();
        let mut n = 0;
        for t in &mut tables {
            for (m, d) in t.meta.iter_mut().zip(t.domains.iter_mut()) {
                if (el && m.witness == WitnessStatus::Elided)
                    || (nc && m.witness == WitnessStatus::NotCarried)
                {
                    m.witness_basis = Assurance::CheckDisabled;
                    d.witness_elided = false;
                    n += 1;
                }
            }
        }
        eprintln!("elided / not-carried records applied as the engine does: {n}");
        let d = Dispatcher::new(tables, ld.disp.priority().to_vec()).unwrap();
        ld.disp = match ld.disp.chart() {
            Some(c) => d.with_chart(c.to_vec()).unwrap(),
            None => d,
        };
    }
    let (disp, st) = if std::env::var("PARAMETRIC_FALLTHROUGH").as_deref() == Ok("reduced") {
        let (d, st) = rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
            &ld.disp,
            &ld.frame,
            &ProofBudget::default(),
        )
        .unwrap();
        (d, Some(st))
    } else {
        let d = Dispatcher::new(ld.disp.tables().to_vec(), ld.disp.priority().to_vec()).unwrap();
        (
            match ld.disp.chart() {
                Some(c) => d.with_chart(c.to_vec()).unwrap(),
                None => d,
            },
            None,
        )
    };
    if let Some(st) = &st {
        eprintln!(
            "reduced fall-through: poles {} redundant {} moved {}",
            st.poles,
            st.redundant,
            st.moved_unknown + st.moved_nonempty
        );
    }
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    let mut n = 0u64;
    for line in std::io::BufReader::new(std::fs::File::open(&keys).unwrap()).lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let k: Vec<i32> = serde_json::from_str(&line).unwrap();
        let d = disp.first_match(&k);
        // the serving TABLE (source id) beside the label (2026-11-17, final certification: a recipe source's labels
        // repeat table labels, so the label alone does not name the record)
        let (label, table, why) = match &d {
            Ok(Dispatch::Covered(r)) => (
                json!(r.label),
                json!(disp.tables()[r.table].name),
                "covered".to_string(),
            ),
            other => (
                json!(null),
                json!(null),
                format!("{other:?}").chars().take(600).collect(),
            ),
        };
        writeln!(
            f,
            "{}",
            json!({"key": k, "label": label, "table": table, "dispatch": why})
        )
        .unwrap();
        n += 1;
    }
    f.flush().unwrap();
    eprintln!("drill: {n} keys dispatched");
}

/// T162 bank witness loci (reports/T162-WITNESS-LOCI.md; witness-loci-bank.jsonl, one line per bank record): RESTORED /
/// RESTORED_WITH_HIDDEN_BRANCHES records get their witness loci as pole loci (status Complete) and the hidden exception
/// branches; NOT_ESTABLISHED records are left as they are (no witness loci: they BLOCK under the strict reading). Matched
/// by the raw line's sha256 (import jobs: `record_sha256`, bank-conv tables) or, for catalogue tables built by lane
/// parametric, by `origin.source == "bank#<index>"`.
fn restore_bank_loci(ld: &mut Loaded, file: &str) -> serde_json::Value {
    use rustred_campaign_core::domain::{PoleKind, PoleLocus};
    use std::io::{Read, Seek, SeekFrom};
    let mut by_sha: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut by_index: BTreeMap<i64, serde_json::Value> = BTreeMap::new();
    for l in std::fs::read_to_string(file)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        by_sha.insert(v["record_sha256"].as_str().unwrap().to_string(), v.clone());
        by_index.insert(v["index"].as_i64().unwrap(), v);
    }
    let n = ld.frame.arity();
    let mut tables: Vec<Table> = ld.disp.tables().to_vec();
    let (mut restored, mut hidden, mut not_est, mut matched) = (0usize, 0usize, 0usize, 0usize);
    let mut elided_kept = 0usize;
    let mut unproven: Vec<serde_json::Value> = Vec::new();
    let unproven_engine = std::env::var("PARAMETRIC_BANK_UNPROVEN").as_deref() == Ok("engine");
    let mut seen: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
    for j in &mut ld.jobs {
        let mut entry = by_sha.get(&j.record_sha256).cloned();
        if entry.is_none() {
            let mut f = std::fs::File::open(&j.path).unwrap();
            f.seek(SeekFrom::Start(j.offset)).unwrap();
            let mut b = vec![0u8; j.bytes as usize];
            f.read_exact(&mut b).unwrap();
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&b)
                && let Some(s) = v["origin"]["source"].as_str()
                && let Some(ix) = s.strip_prefix("bank#").and_then(|x| x.parse::<i64>().ok())
            {
                entry = by_index.get(&ix).cloned();
            }
        }
        let Some(e) = entry else { continue };
        matched += 1;
        seen.insert(e["index"].as_i64().unwrap());
        let status = e["status"].as_str().unwrap_or("");
        let ix = e["index"].as_i64().unwrap();
        let t = &mut tables[j.table];
        let d = &mut t.domains[j.record];
        // hidden exception branches (the re-lift's rule has them): added where not already present; this only
        // removes domain
        if let Some(hb) = e["hidden_branches"].as_array() {
            for br in hb {
                let members: Vec<rustred_campaign_core::poly::Poly> = match br {
                    serde_json::Value::Array(a) => a
                        .iter()
                        .map(|m| {
                            rustred_campaign_core::poly::Poly::parse_d_free_exact(
                                m.as_str().unwrap(),
                                n,
                            )
                            .unwrap()
                        })
                        .collect(),
                    serde_json::Value::String(s) => {
                        vec![rustred_campaign_core::poly::Poly::parse_d_free_exact(s, n).unwrap()]
                    }
                    _ => continue,
                };
                if !d.exceptions.contains(&members) {
                    d.exceptions.push(members);
                    hidden += 1;
                }
            }
        }
        let loci: Vec<String> = e["witness_loci"]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect())
            .unwrap_or_default();
        let elided = loci.iter().any(|x| x.trim() == "elided");
        if status == "NOT_ESTABLISHED" || elided {
            // strict: unusable wherever selected (NotCarried / Elided); PARAMETRIC_BANK_UNPROVEN=engine applies them as
            // the engine does (to measure their cost by difference)
            if status == "NOT_ESTABLISHED" {
                not_est += 1;
                unproven.push(json!({"bank": ix, "status": status, "why": "not established"}));
            } else {
                elided_kept += 1;
                d.witness_elided = true;
                t.meta[j.record].witness = rustred_campaign_core::cover::WitnessStatus::Elided;
                unproven.push(json!({"bank": ix, "status": status, "why": "restored loci are the literal elided (ruling F1)"}));
            }
            if unproven_engine {
                d.witness_elided = false;
                t.meta[j.record].witness_basis =
                    rustred_campaign_core::cover::Assurance::CheckDisabled;
            }
            j.domain = d.clone();
            continue;
        }
        d.poles.retain(|p| p.kind != PoleKind::Witness);
        for s in &loci {
            let p = rustred_campaign_core::poly::Poly::parse_exact(s, n, true).unwrap();
            d.poles.push(PoleLocus::new(PoleKind::Witness, &p));
        }
        d.witness_elided = false;
        t.meta[j.record].witness = rustred_campaign_core::cover::WitnessStatus::Complete;
        t.meta[j.record].witness_basis = rustred_campaign_core::cover::Assurance::Record;
        j.domain = d.clone();
        restored += 1;
    }
    let still_notcarried = tables
        .iter()
        .flat_map(|t| t.meta.iter())
        .filter(|m| m.witness == rustred_campaign_core::cover::WitnessStatus::NotCarried)
        .count();
    let still_elided = tables
        .iter()
        .flat_map(|t| t.domains.iter())
        .filter(|d| d.witness_elided)
        .count();
    let pr = ld.disp.priority().to_vec();
    ld.disp = Dispatcher::new(tables, pr)
        .unwrap()
        .with_chart(ld.frame.mask().to_vec())
        .unwrap();
    json!({"file": file, "sha256": sha256_file(Path::new(file)), "lines": by_sha.len(), "records_matched": matched,
           "distinct_bank_indices_matched": seen.len(), "restored": restored, "hidden_branches_added": hidden,
           "not_established": not_est, "elided_loci": elided_kept, "unproven": unproven,
           "unproven_applied_as_engine": unproven_engine,
           "records_still_notcarried": still_notcarried, "records_still_elided": still_elided})
}

/// A record whose rhs carries a symbol at another slot (`n<i>` at slot j != i): the engine treats it as a SLOT ALIAS and
/// applies it AFTER every ordinary rule (TRICKS 43).
fn is_slot_alias_raw(v: &serde_json::Value) -> bool {
    let Some(rhs) = v["rhs"].as_array() else {
        return false;
    };
    rhs.iter().any(|t| {
        let Some(term) = t.get(0).and_then(|x| x.as_str()) else {
            return false;
        };
        let (Some(a), Some(b)) = (term.find('('), term.rfind(')')) else {
            return false;
        };
        term[a + 1..b].split(',').enumerate().any(|(j, e)| {
            let e = e.trim();
            e.strip_prefix('n')
                .map(|r| {
                    r.chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                })
                .and_then(|d| d.parse::<usize>().ok())
                .is_some_and(|i| i != j)
        })
    })
}

/// Per (table, record) of the dispatcher: is it a slot alias (read from the record's raw line).
fn alias_flags(ld: &Loaded) -> std::collections::BTreeSet<(usize, usize)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut out = std::collections::BTreeSet::new();
    for j in &ld.jobs {
        let mut f = std::fs::File::open(&j.path).unwrap();
        f.seek(SeekFrom::Start(j.offset)).unwrap();
        let mut b = vec![0u8; j.bytes as usize];
        f.read_exact(&mut b).unwrap();
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&b)
            && is_slot_alias_raw(&v)
        {
            out.insert((j.table, j.record));
        }
    }
    out
}

/// The ENGINE's effective first-match order (TRICKS 43): every ordinary record in table order, THEN the slot aliases.
/// The dispatcher is rebuilt so: one ordinary copy per table (same priority), then one alias copy per table holding
/// aliases. Labels are kept; jobs are re-pointed.
fn engine_order(ld: &mut Loaded) -> serde_json::Value {
    let aliases = alias_flags(ld);
    if aliases.is_empty() {
        return json!({"slot_aliases": 0});
    }
    let tables = ld.disp.tables().to_vec();
    let prio = ld.disp.priority().to_vec();
    let mut new_tables: Vec<Table> = Vec::new();
    let mut remap: BTreeMap<(usize, usize), (usize, usize)> = BTreeMap::new();
    let mut alias_parts: Vec<Table> = Vec::new();
    let mut alias_src: Vec<Vec<(usize, usize)>> = Vec::new();
    for &ti in &prio {
        let t = &tables[ti];
        let mut ord = t.clone();
        let mut ali = t.clone();
        let (mut od, mut ol, mut om) = (vec![], vec![], vec![]);
        let (mut ad, mut al, mut am) = (vec![], vec![], vec![]);
        let mut asrc = vec![];
        for r in 0..t.domains.len() {
            if aliases.contains(&(ti, r)) {
                ad.push(t.domains[r].clone());
                al.push(t.labels[r]);
                am.push(t.meta[r]);
                asrc.push((ti, r));
            } else {
                remap.insert((ti, r), (new_tables.len(), od.len()));
                od.push(t.domains[r].clone());
                ol.push(t.labels[r]);
                om.push(t.meta[r]);
            }
        }
        ord.domains = od;
        ord.labels = ol;
        ord.meta = om;
        new_tables.push(ord);
        if !ad.is_empty() {
            ali.name = format!("{} [slot aliases, engine phase 2]", t.name);
            ali.domains = ad;
            ali.labels = al;
            ali.meta = am;
            alias_parts.push(ali);
            alias_src.push(asrc);
        }
    }
    for (k, t) in alias_parts.into_iter().enumerate() {
        let ni = new_tables.len();
        for (r, &src) in alias_src[k].iter().enumerate() {
            remap.insert(src, (ni, r));
        }
        new_tables.push(t);
    }
    for j in &mut ld.jobs {
        let (t, r) = remap[&(j.table, j.record)];
        j.table = t;
        j.record = r;
    }
    let m = new_tables.len();
    ld.disp = Dispatcher::new(new_tables, (0..m).collect())
        .unwrap()
        .with_chart(ld.frame.mask().to_vec())
        .unwrap();
    json!({"slot_aliases": aliases.len(), "order": "ordinary records in table order, then slot aliases (engine two-phase dispatch)"})
}

/// DISPATCH-ORDER CHECK (TRICKS 43, builder check): for every slot alias A of PARAMETRIC_SPEC (sources list / import spec,
/// certified order) and every ORDINARY record R placed after it, is A's selected set disjoint from R's (fixed slots,
/// equations, bounds; R's exception branches are ignored: a superset)? A non-empty intersection means the engine (aliases
/// applied after every ordinary rule) may serve those keys with R instead of A. Output PARAMETRIC_OUT (JSON).
#[test]
#[ignore]
fn dispatch_order_check() {
    use rustred_campaign_core::parametric::descent::{EmptyState, emptiness};
    use rustred_campaign_core::poly::Poly;
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let ld = load_any(&spec, None);
    let aliases = alias_flags(&ld);
    let budget = ProofBudget::default();
    let order: Vec<(usize, usize)> = ld
        .disp
        .priority()
        .iter()
        .flat_map(|&ti| (0..ld.disp.tables()[ti].domains.len()).map(move |r| (ti, r)))
        .collect();
    let mut found = Vec::new();
    let (mut checked, mut overlaps) = (0usize, 0usize);
    for (p, &(ta, ra)) in order.iter().enumerate() {
        if !aliases.contains(&(ta, ra)) {
            continue;
        }
        let a = &ld.disp.tables()[ta].domains[ra];
        for &(tr, rr) in &order[p + 1..] {
            if aliases.contains(&(tr, rr)) {
                continue;
            }
            let r = &ld.disp.tables()[tr].domains[rr];
            let mut eqs: Vec<Poly> = r.equations.clone();
            let mut conflict = false;
            for (s, fx) in r.fixed.iter().enumerate() {
                if let Some(v) = fx {
                    if let Some(av) = a.fixed[s] {
                        if av != *v {
                            conflict = true;
                            break;
                        }
                        continue;
                    }
                    eqs.push(Poly::var(s).sub(&Poly::constant(i128::from(*v))).unwrap());
                }
            }
            checked += 1;
            if conflict {
                continue;
            }
            let (st, w, why) = emptiness(a, &ld.frame, &budget, &eqs, &r.bounds);
            if st != EmptyState::Empty {
                overlaps += 1;
                found.push(json!({"alias": {"table": ld.disp.tables()[ta].name, "label": ld.disp.tables()[ta].labels[ra], "position": p},
                    "ordinary_after": {"table": ld.disp.tables()[tr].name, "label": ld.disp.tables()[tr].labels[rr]},
                    "intersection": format!("{st:?}"), "witness": w, "why": why}));
            }
        }
    }
    let res = json!({"spec": {"path": spec.display().to_string(), "sha256": sha256_file(&spec)}, "slot_aliases": aliases.len(),
        "pairs_checked": checked, "overlaps": overlaps, "found": found,
        "rule": "no ordinary record after a slot alias may overlap it (the engine applies aliases after every ordinary rule)"});
    std::fs::write(&out, serde_json::to_string_pretty(&res).unwrap()).unwrap();
    eprintln!(
        "dispatch order check: {} aliases, {checked} pairs, {overlaps} overlaps",
        aliases.len()
    );
}

/// TERMINAL / STUB CHECK of a delivered table (TRICKS 51; builder check): PARAMETRIC_SPEC (sources list / import spec),
/// PARAMETRIC_TERMINALS (terminal-set document): REFUSED when any record's domain contains a declared terminal key
/// (`cone::records_containing`) or when any record is a coverage STUB (empty rhs: an intermediate device, never in a delivered
/// table). Output PARAMETRIC_OUT (JSON).
#[test]
#[ignore]
fn terminal_check() {
    use rustred_campaign_core::parametric::cone::records_containing;
    use std::io::{Read, Seek, SeekFrom};
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let tp = std::env::var("PARAMETRIC_TERMINALS").expect("PARAMETRIC_TERMINALS");
    let tdoc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&tp).unwrap()).unwrap();
    let terms: Vec<Vec<i32>> = serde_json::from_value(tdoc["keys"].clone()).unwrap();
    let ld = load_any(&spec, None);
    let mut contained = Vec::new();
    for k in &terms {
        for (t, r) in records_containing(&ld.disp, k) {
            contained.push(json!({"terminal": k, "table": ld.disp.tables()[t].name, "label": ld.disp.tables()[t].labels[r]}));
        }
    }
    let mut stubs = Vec::new();
    // a BOUNDED RECIPE (source class sealed_recipe, its seal verified by sha256 at load) is its own artifact kind: its
    // coverage record has no rhs by construction and is NOT a coverage stub (listed separately)
    let mut recipes = Vec::new();
    for j in &ld.jobs {
        if j.meta["class"].as_str() == Some("sealed_recipe") {
            recipes.push(json!({"table": ld.disp.tables()[j.table].name, "label": ld.disp.tables()[j.table].labels[j.record]}));
            continue;
        }
        let mut f = std::fs::File::open(&j.path).unwrap();
        f.seek(SeekFrom::Start(j.offset)).unwrap();
        let mut b = vec![0u8; j.bytes as usize];
        f.read_exact(&mut b).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        if v["rhs"].as_array().is_some_and(|a| a.is_empty()) {
            stubs.push(json!({"table": ld.disp.tables()[j.table].name, "label": ld.disp.tables()[j.table].labels[j.record]}));
        }
    }
    let refused = !contained.is_empty() || !stubs.is_empty();
    let res = json!({"spec": {"path": spec.display().to_string(), "sha256": sha256_file(&spec)},
        "terminals": {"path": tp, "sha256": sha256_file(Path::new(&tp)), "keys": terms},
        "records_containing_a_terminal": contained, "coverage_stubs": stubs.len(), "stubs_first": stubs.iter().take(100).collect::<Vec<_>>(),
        "bounded_recipes_not_stubs": recipes,
        "verdict": if refused { "REFUSED" } else { "ok" },
        "rule": "a declared terminal is never served by a record; a delivered table carries no coverage stub"});
    std::fs::write(&out, serde_json::to_string_pretty(&res).unwrap()).unwrap();
    eprintln!(
        "terminal check: {} containments, {} stubs: {}",
        contained.len(),
        stubs.len(),
        if refused { "REFUSED" } else { "ok" }
    );
}

/// CHAIN WALK of one key through a dispatch (acceptance case, TRICKS 52): breadth-first over the TOP-SECTOR keys reached from
/// PARAMETRIC_KEY (comma list) by first match + rhs instantiation (rhs read from the record line, or from its origin source
/// line when the dispatch record is a coverage form), the strict loci / G5 reduced dispatch as in redispatch_keys. Stubs
/// (empty rhs) and open keys are NOT expanded and are listed; terminals (PARAMETRIC_TERMINALS) stop the walk. Output
/// PARAMETRIC_OUT (JSON): records used, keys, max depth, stub steps (by stub record), open keys.
#[test]
#[ignore]
fn chain_walk() {
    use rustred_campaign_core::cover::Dispatch;
    use std::io::{Read, Seek, SeekFrom};
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let start: Vec<i32> = std::env::var("PARAMETRIC_KEY")
        .expect("PARAMETRIC_KEY")
        .split(',')
        .map(|x| x.trim().parse().unwrap())
        .collect();
    let max_keys: usize = std::env::var("PARAMETRIC_MAX_KEYS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(5_000_000);
    let terms: std::collections::BTreeSet<Vec<i32>> = std::env::var("PARAMETRIC_TERMINALS")
        .ok()
        .map(|p| {
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
            serde_json::from_value::<Vec<Vec<i32>>>(v["keys"].clone())
                .unwrap()
                .into_iter()
                .collect()
        })
        .unwrap_or_default();
    let mut ld = load_any(
        &spec,
        std::env::var("PARAMETRIC_INDEX_DIR")
            .ok()
            .map(PathBuf::from),
    );
    if let Ok(f) = std::env::var("PARAMETRIC_SUPPLIED_LOCI") {
        restore_loci(&mut ld, &f);
    }
    if let Ok(f) = std::env::var("PARAMETRIC_BANK_LOCI") {
        restore_bank_loci(&mut ld, &f);
    }
    let (g5, _) = rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
        &ld.disp,
        &ld.frame,
        &ProofBudget::default(),
    )
    .unwrap();
    let n = ld.frame.arity();
    let lines: Vec<bool> = (0..n).map(|i| ld.frame.is_line(i)).collect();
    let job_of: BTreeMap<(usize, usize), usize> = ld
        .jobs
        .iter()
        .enumerate()
        .map(|(i, j)| ((j.table, j.record), i))
        .collect();
    let read_line = |p: &str, off: u64, len: Option<u64>| -> serde_json::Value {
        let mut f = std::fs::File::open(p).unwrap();
        f.seek(SeekFrom::Start(off)).unwrap();
        let b = match len {
            Some(l) => {
                let mut b = vec![0u8; l as usize];
                f.read_exact(&mut b).unwrap();
                b
            }
            None => {
                let mut r = std::io::BufReader::new(f);
                let mut s = String::new();
                std::io::BufRead::read_line(&mut r, &mut s).unwrap();
                s.into_bytes()
            }
        };
        serde_json::from_slice(&b).unwrap()
    };
    // per (table, record): (target slots, rhs term slots) or None for a stub
    type Slots = Option<(Vec<String>, Vec<Vec<String>>)>;
    let mut rules: BTreeMap<(usize, usize), Slots> = BTreeMap::new();
    let parse_slots = |t: &str| -> Vec<String> {
        t[t.find('(').unwrap() + 1..t.rfind(')').unwrap()]
            .split(',')
            .map(|x| x.trim().to_string())
            .collect()
    };
    // PARAMETRIC_CHAIN_POINTS (optional): a table-dump file of POINT relations (concrete target and rhs, e.g. the sealed
    // relations of a bounded recipe at the chain's keys): a key served by a record without rhs (stub) follows the point
    // relation of that key when the file has one.
    let points: BTreeMap<Vec<i32>, Vec<Vec<i32>>> = std::env::var("PARAMETRIC_CHAIN_POINTS")
        .ok()
        .map(|p| {
            let mut m = BTreeMap::new();
            for l in std::fs::read_to_string(p).unwrap().lines() {
                let v: serde_json::Value = match serde_json::from_str(l) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if v["type"] != "rule" {
                    continue;
                }
                let ints = |t: &str| -> Option<Vec<i32>> {
                    t[t.find('(')? + 1..t.rfind(')')?]
                        .split(',')
                        .map(|x| x.trim().parse().ok())
                        .collect()
                };
                let Some(tg) = v["target"].as_str().and_then(ints) else {
                    continue;
                };
                let rhs: Vec<Vec<i32>> = v["rhs"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t[0].as_str().and_then(ints))
                            .collect()
                    })
                    .unwrap_or_default();
                m.insert(tg, rhs);
            }
            m
        })
        .unwrap_or_default();
    // PARAMETRIC_CHAIN_KEYS_OUT (optional, JSONL): every key of the walk with its level, its first match and its kind
    // (rule / point / stub / terminal / open); the keys of the top sector reached from the start key.
    let mut keys_w = std::env::var("PARAMETRIC_CHAIN_KEYS_OUT")
        .ok()
        .map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut put = |k: &Vec<i32>, level: usize, served: Option<&str>, kind: &str| {
        if let Some(w) = keys_w.as_mut() {
            use std::io::Write;
            writeln!(
                w,
                "{}",
                json!({"key": k, "level": level, "served": served, "kind": kind})
            )
            .unwrap();
        }
    };
    let mut point_steps = 0u64;
    let mut seen: std::collections::BTreeSet<Vec<i32>> = std::collections::BTreeSet::new();
    let mut frontier = vec![start.clone()];
    seen.insert(start.clone());
    // the keys served by a record without rhs (coverage stub / bounded recipe): listed (the engine needs their relations)
    let mut stub_keys: BTreeMap<String, Vec<Vec<i32>>> = BTreeMap::new();
    let (mut depth, mut open, mut stub_steps, mut used, mut terminal_hits, mut lower) = (
        0usize,
        Vec::new(),
        BTreeMap::<String, u64>::new(),
        BTreeMap::<String, u64>::new(),
        0u64,
        0u64,
    );
    while !frontier.is_empty() && seen.len() < max_keys {
        let mut next = Vec::new();
        for k in &frontier {
            if terms.contains(k) {
                terminal_hits += 1;
                put(k, depth, None, "terminal");
                continue;
            }
            match g5.first_match(k) {
                Ok(Dispatch::Covered(r)) => {
                    let tname = g5.tables()[r.table].name.clone();
                    let lab = g5.tables()[r.table].labels[r.record];
                    let ent = rules.entry((r.table, r.record)).or_insert_with(|| {
                        let j = &ld.jobs[job_of[&(r.table, r.record)]];
                        let v = read_line(&j.path, j.offset, Some(j.bytes));
                        let v = if v["rhs"].as_array().is_some_and(|a| a.is_empty())
                            && v["origin"]["path"].is_string()
                            && (v["origin"]["offset"].is_u64() || v["origin"]["offset"].is_string())
                        {
                            let off: u64 = v["origin"]["offset"]
                                .as_str()
                                .map(|s| s.parse().unwrap())
                                .or(v["origin"]["offset"].as_u64())
                                .unwrap();
                            read_line(v["origin"]["path"].as_str().unwrap(), off, None)
                        } else {
                            v
                        };
                        let rhs = v["rhs"].as_array().cloned().unwrap_or_default();
                        if rhs.is_empty() {
                            return None;
                        }
                        Some((
                            parse_slots(v["target"].as_str().unwrap()),
                            rhs.iter()
                                .map(|t| parse_slots(t[0].as_str().unwrap()))
                                .collect(),
                        ))
                    });
                    let served = format!("{tname}#{lab}");
                    match ent {
                        None => {
                            *stub_steps.entry(served.clone()).or_default() += 1;
                            stub_keys.entry(served.clone()).or_default().push(k.clone());
                            match points.get(k) {
                                Some(rhs) => {
                                    put(k, depth, Some(&served), "point");
                                    point_steps += 1;
                                    for c in rhs {
                                        if (0..n).any(|i| lines[i] && c[i] < 1) {
                                            lower += 1;
                                            continue;
                                        }
                                        if seen.insert(c.clone()) {
                                            next.push(c.clone());
                                        }
                                    }
                                }
                                None => put(k, depth, Some(&served), "stub"),
                            }
                        }
                        Some((tg, pats)) => {
                            put(k, depth, Some(&served), "rule");
                            *used.entry(served.clone()).or_default() += 1;
                            // parameter values from the target pattern: slot s = n_j + o  =>  n_j = k[s] - o
                            let mut val: BTreeMap<usize, i64> = BTreeMap::new();
                            let sym = |e: &str| -> Option<(usize, i64)> {
                                let e = e.trim();
                                let rest = e.strip_prefix('n')?;
                                let d: String =
                                    rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                                let o = rest[d.len()..].trim();
                                Some((
                                    d.parse().ok()?,
                                    if o.is_empty() {
                                        0
                                    } else {
                                        o.replace('+', "").parse().ok()?
                                    },
                                ))
                            };
                            for (s, e) in tg.iter().enumerate() {
                                if let Some((j, o)) = sym(e) {
                                    val.insert(j, i64::from(k[s]) - o);
                                }
                            }
                            for p in pats.iter() {
                                let c: Vec<i32> = p
                                    .iter()
                                    .map(|e| match sym(e) {
                                        Some((j, o)) => (val[&j] + o) as i32,
                                        None => e.parse().unwrap(),
                                    })
                                    .collect();
                                if (0..n).any(|i| lines[i] && c[i] < 1) {
                                    lower += 1; // leaves the top sector (another node)
                                    continue;
                                }
                                if seen.insert(c.clone()) {
                                    next.push(c);
                                }
                            }
                        }
                    }
                }
                other => {
                    put(k, depth, None, "open");
                    if open.len() < 200 {
                        open.push(json!({"key": k, "dispatch": format!("{other:?}").chars().take(120).collect::<String>()}));
                    }
                }
            }
        }
        frontier = next;
        depth += 1;
        eprintln!(
            "  depth {depth}: {} keys seen, frontier {}",
            seen.len(),
            frontier.len()
        );
    }
    let mut used_top: Vec<(&String, &u64)> = used.iter().collect();
    used_top.sort_by(|a, b| b.1.cmp(a.1));
    used_top.truncate(40);
    let res = json!({"start": start, "spec": {"path": spec.display().to_string(), "sha256": sha256_file(&spec)},
        "keys_reached_in_top_sector": seen.len(), "depth": depth, "complete": frontier.is_empty(), "terminal_hits": terminal_hits,
        "terms_leaving_the_sector": lower, "records_used": used.len(), "records_used_top": used_top,
        "stub_steps": stub_steps, "stub_keys": stub_keys, "records_used_all": used, "open_keys": open,
        "point_relations": points.len(), "point_steps": point_steps, "keys_out": std::env::var("PARAMETRIC_CHAIN_KEYS_OUT").ok(),
        "unvisited_frontier": frontier.len()});
    // `put`'s borrow of keys_w ends at its last use (no drop needed: a closure has no Drop)
    if let Some(mut w) = keys_w {
        use std::io::Write;
        w.flush().unwrap();
    }
    std::fs::write(&out, serde_json::to_string_pretty(&res).unwrap()).unwrap();
    eprintln!(
        "chain walk: {} keys, depth {depth}, {} records, stub steps {:?}, open {}",
        seen.len(),
        used.len(),
        stub_steps,
        open.len()
    );
}

/// PER-RECORD LIVENESS in a cone (delivery note / dead-record pruning): record r is LIVE iff some key of D + r <= R is first-
/// matched by it. Exact: the dispatcher capped to r's face (one-slot caps from its fixed slots and one-slot bounds) is
/// partitioned with ONLY the records before r (rule subset); every uncovered element inside the cone is enumerated (cap per
/// element) and r tested at its keys: a key where r applies = live (witness); none with complete enumeration = dead; else
/// undecided. Env: PARAMETRIC_SPEC (+ loci / bank loci / FALLTHROUGH reduced, ENGINE), PARAMETRIC_LIVENESS_DR (R),
/// PARAMETRIC_LIVENESS_RANGE "a:b" (records by global dispatch position), PARAMETRIC_OUT (JSONL, one line per record).
#[test]
#[ignore]
fn record_liveness() {
    use rustred_campaign_core::parametric::cone::ConeOptions;
    let spec = PathBuf::from(std::env::var("PARAMETRIC_SPEC").expect("PARAMETRIC_SPEC"));
    let out = std::env::var("PARAMETRIC_OUT").expect("PARAMETRIC_OUT");
    let rdr: i64 = std::env::var("PARAMETRIC_LIVENESS_DR")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(39);
    let mut ld = load_any(
        &spec,
        std::env::var("PARAMETRIC_INDEX_DIR")
            .ok()
            .map(PathBuf::from),
    );
    if let Ok(f) = std::env::var("PARAMETRIC_SUPPLIED_LOCI") {
        restore_loci(&mut ld, &f);
    }
    if let Ok(f) = std::env::var("PARAMETRIC_BANK_LOCI") {
        restore_bank_loci(&mut ld, &f);
    }
    let (g5, _) = rustred_campaign_core::parametric::cone::fall_through_poles_reduced(
        &ld.disp,
        &ld.frame,
        &ProofBudget::default(),
    )
    .unwrap();
    let frame = ld.frame.clone();
    let n = frame.arity();
    let order: Vec<(usize, usize)> = g5
        .priority()
        .iter()
        .flat_map(|&ti| (0..g5.tables()[ti].domains.len()).map(move |r| (ti, r)))
        .collect();
    let (a, b) = std::env::var("PARAMETRIC_LIVENESS_RANGE")
        .ok()
        .and_then(|x| {
            x.split_once(':')
                .map(|(p, q)| (p.parse().unwrap(), q.parse().unwrap()))
        })
        .unwrap_or((0usize, order.len()));
    let mut base_opts = ConeOptions::default();
    // per-record budget (a record that exceeds it is UNDECIDED, never live / dead)
    base_opts.budget.max_nodes = Some(
        std::env::var("PARAMETRIC_LIVENESS_UNITS")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(50_000_000),
    );
    base_opts.budget.wall = Some(std::time::Duration::from_secs(
        std::env::var("PARAMETRIC_LIVENESS_WALL")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(120),
    ));
    base_opts.engine_exact_zero = std::env::var("PARAMETRIC_ENGINE").as_deref() == Ok("4895ee68");
    // the cone D + r <= R
    let mut cone_a = vec![0i64; n];
    let mut cone_b = rdr;
    for (s, c) in cone_a.iter_mut().enumerate() {
        if frame.is_line(s) {
            *c = 1;
            cone_b += 1;
        } else {
            *c = -1;
        }
    }
    let region = parse_cone(
        &frame,
        &std::env::var("PARAMETRIC_LIVENESS_CONE").unwrap_or_else(|_| format!("simplex:{rdr}")),
    );
    let enum_cap: u64 = std::env::var("PARAMETRIC_LIVENESS_ENUM")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(20_000);
    let _ = (&cone_a, cone_b);
    // final-certification tool step (2026-11-17): PARAMETRIC_LIVENESS_POSITIONS = a file or a list of positions (JSON array
    // or "a,b,c:d") restricting the range (second pass on undecided records); PARAMETRIC_LIVENESS_RESUME=1 keeps the complete
    // lines of an existing PARAMETRIC_OUT, cuts a torn tail and decides only the positions not yet written
    use rustred_campaign_core::parametric::prune::{
        decided_positions, parse_positions, positions_to_decide,
    };
    let selected = std::env::var("PARAMETRIC_LIVENESS_POSITIONS")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| {
            let txt = if Path::new(&s).is_file() {
                std::fs::read_to_string(&s).unwrap()
            } else {
                s
            };
            parse_positions(&txt).unwrap()
        });
    let resume = std::env::var("PARAMETRIC_LIVENESS_RESUME").as_deref() == Ok("1");
    let (done, keep) = if resume {
        std::fs::read_to_string(&out)
            .map(|t| decided_positions(&t))
            .unwrap_or_default()
    } else {
        Default::default()
    };
    let file = if resume {
        use std::io::{Seek, SeekFrom};
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&out)
            .unwrap();
        f.set_len(keep as u64).unwrap();
        f.seek(SeekFrom::End(0)).unwrap();
        f
    } else {
        std::fs::File::create(&out).unwrap()
    };
    let todo = positions_to_decide((a, b), order.len(), selected.as_ref(), &done);
    eprintln!(
        "record_liveness: {} position(s) to decide in [{a}, {b}) of {} ({} already written; selection {})",
        todo.len(),
        order.len(),
        done.len(),
        selected
            .as_ref()
            .map_or("none".to_string(), |s| s.len().to_string())
    );
    let count = std::env::var("PARAMETRIC_LIVENESS_COUNTS").as_deref() == Ok("1");
    let count_cap: u64 = std::env::var("PARAMETRIC_COUNT_ENUM")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(10_000_000);
    let mut w = std::io::BufWriter::new(file);
    for pos in todo {
        let (ti, ri) = order[pos];
        let label = g5.tables()[ti].labels[ri];
        let t0 = std::time::Instant::now();
        // the library's exact first-match liveness (rustred_campaign_core::parametric::prune, commit 16e65d06)
        let v = rustred_campaign_core::parametric::prune::record_liveness(
            &g5, &frame, &region, pos, &base_opts, enum_cap,
        );
        let (live, witness, why) = match v {
            rustred_campaign_core::parametric::prune::Liveness::Live(k) => ("live", Some(k), None),
            rustred_campaign_core::parametric::prune::Liveness::Dead => ("dead", None, None),
            rustred_campaign_core::parametric::prune::Liveness::Undecided(y) => {
                ("undecided", None, Some(y))
            }
        };
        // PARAMETRIC_LIVENESS_COUNTS=1 (2026-11-17, final certification: the per-key proof lane needs them): for a LIVE
        // record its first-match distinct keys in the region per shell D + r (enumeration cap PARAMETRIC_COUNT_ENUM per element)
        let mut line = json!({"position": pos, "table": g5.tables()[ti].name, "label": label, "live": live, "witness": witness,
            "why": why, "region": region.label, "seconds": t0.elapsed().as_secs_f64()});
        if count && live == "live" {
            let t1 = std::time::Instant::now();
            let shell = |k: &[i32]| -> i64 {
                (0..n)
                    .map(|s| {
                        if frame.is_line(s) {
                            i64::from(k[s]) - 1
                        } else {
                            -i64::from(k[s])
                        }
                    })
                    .sum()
            };
            match rustred_campaign_core::parametric::prune::record_first_match_counts(
                &g5, &frame, &region, pos, &base_opts, count_cap, &shell,
            ) {
                Ok((c, complete)) => {
                    line["fm_keys"] = json!(c.values().sum::<u64>());
                    line["fm_keys_by_shell"] = json!(
                        c.iter()
                            .map(|(k, v)| (k.to_string(), *v))
                            .collect::<std::collections::BTreeMap<_, _>>()
                    );
                    line["fm_count_complete"] = json!(complete);
                }
                Err(e) => line["fm_count_error"] = json!(e),
            }
            line["count_seconds"] = json!(t1.elapsed().as_secs_f64());
        }
        writeln!(w, "{line}").unwrap();
        w.flush().unwrap();
    }
}
