//! `rr_campaign legacy-import` (emission contract section 12): bring an old table's records in UNCHANGED, each with the
//! class its evidence gives (`rustred_campaign_core::emission::legacy`), decided from the records' bytes and the
//! evidence files alone: no class table is read. Writes `<out>/classes.jsonl` (per rule: identity, class, the delivery
//! word, how the evidence binds) and `<out>/summary.json`; with `--write-table` also `<out>/<stem>.table.jsonl` whose
//! rule lines carry their `derivation` member (origin import, kind legacy) and the side directory (bound verdict rows,
//! proven key lists, policy) that `table-check` re-verifies. A record the policy refuses (a live record without a
//! class, a live record contradicted by a vacuous verdict at its own position) makes the import fail (exit 3): no table
//! is written then.
use rustred_campaign_core::closure::sha256::Sha256;
use rustred_campaign_core::emission::body::{
    attach_derivation, body_ids, members, sha256_hex, string_member,
};
use rustred_campaign_core::emission::class::{
    Classed, Evidence, Kind, Liveness, Origin, Policy, class_of,
};
use rustred_campaign_core::emission::legacy::{
    self, Config, Counts, Decision, KeyProofs, KeyedBy, RecordId, VerdictRow, decide, keys_text,
    legacy_member,
};
use rustred_campaign_core::emission::{
    AREA_EVID, AREA_KEYS, AREA_POLICY, Parts, derivation, side_dir, side_rel,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

fn read(p: &str) -> Result<(String, String), String> {
    let b = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
    let sha = sha256_hex(&b);
    Ok((String::from_utf8_lossy(&b).into_owned(), sha))
}

/// The `index` and `origin.dispatch_position` of a rule line.
fn index_and_dispatch(line: &str) -> (i64, Option<i64>) {
    let Ok(m) = members(line) else {
        return (-1, None);
    };
    let get = |k: &str| {
        m.iter()
            .find(|x| x.key == k)
            .map(|x| &line[x.value.clone()])
    };
    let index = get("index")
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(-1);
    let dp = get("origin")
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .and_then(|o| o["dispatch_position"].as_i64());
    (index, dp)
}

/// The evidence loaded from the configuration.
pub struct Loaded {
    pub cfg: Config,
    pub rows: Vec<VerdictRow>,
    pub proofs: BTreeMap<u64, KeyProofs>,
    pub counts: BTreeMap<u64, Counts>,
    /// reference index -> (relation, domain) for reference-keyed rows that lack either
    pub ref_ids: BTreeMap<i64, (String, String)>,
    pub sources: Vec<Value>,
}

/// Load every evidence source of `cfg` (and the reference identities the rows need).
pub fn load(cfg: Config) -> Result<Loaded, String> {
    if cfg.schema != legacy::SCHEMA {
        return Err(format!(
            "config schema {} (want {})",
            cfg.schema,
            legacy::SCHEMA
        ));
    }
    let mut sources = vec![];
    let mut pobj = BTreeMap::new();
    for po in &cfg.proof_objects {
        let (t, sha) = read(&po.path)?;
        for (k, v) in legacy::proof_objects(&t, &po.key_col, &po.exact_col) {
            pobj.insert((po.doc.clone(), k), v);
        }
        sources
            .push(json!({"kind": "proof objects", "name": po.doc, "path": po.path, "sha256": sha}));
    }
    let mut rows = vec![];
    for v in &cfg.verdicts {
        let (t, sha) = read(&v.path)?;
        let r = legacy::parse_verdicts(v, &t, &sha, &pobj)?;
        sources.push(json!({"kind": "verdicts", "name": v.name, "path": v.path, "sha256": sha, "rows": r.len()}));
        rows.extend(r);
    }
    let mut proofs = BTreeMap::new();
    for k in &cfg.key_proofs {
        let mut files: Vec<PathBuf> = vec![];
        if let Some(f) = &k.file {
            files.push(PathBuf::from(f));
        }
        if let Some(d) = &k.dir {
            // a COMPLETE listing or an error (fsx rule)
            let mut fs: Vec<PathBuf> =
                crate::fsx::list_required("emission.legacy.dir", Path::new(d))
                    .map_err(|e| format!("{d}: {e}"))?
                    .into_iter()
                    .map(|e| e.path)
                    .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                    .collect();
            fs.sort();
            files.extend(fs);
        }
        let mut h = Sha256::new();
        let mut n = 0;
        for f in &files {
            let (t, sha) = read(&f.display().to_string())?;
            h.update(format!("{} {sha}\n", f.display()).as_bytes());
            n += legacy::add_key_proofs(&k.name, &t, &mut proofs)?;
        }
        sources.push(
            json!({"kind": "key proofs", "name": k.name, "files": files.len(), "exact_lines": n,
                            "files_sha256": h.finish_hex()}),
        );
    }
    let mut counts = BTreeMap::new();
    for c in &cfg.counts {
        let (t, sha) = read(&c.path)?;
        let n = legacy::add_counts(c, &t, &sha, &mut counts)?;
        sources.push(
            json!({"kind": "counts", "name": c.name, "path": c.path, "sha256": sha, "rows": n,
                            "table_sha256": c.table_sha256}),
        );
    }
    // reference identities: only the rows that lack a hash or a domain need them
    let need: BTreeSet<i64> = rows
        .iter()
        .filter(|r| r.keyed_by == KeyedBy::Reference && (r.hash.is_none() || r.domain.is_none()))
        .map(|r| r.index)
        .collect();
    let mut ref_ids = BTreeMap::new();
    if !need.is_empty() {
        let Some(rt) = &cfg.reference_table else {
            return Err(format!(
                "{} reference-keyed rows lack a hash or domain: the config needs reference_table",
                need.len()
            ));
        };
        let f = std::fs::File::open(rt).map_err(|e| format!("{rt}: {e}"))?;
        for l in std::io::BufReader::new(f).lines() {
            let l = l.map_err(|e| format!("{rt}: {e}"))?;
            if string_member(&l, "type").as_deref() != Some("rule") {
                continue;
            }
            let (ix, _) = index_and_dispatch(&l);
            if need.contains(&ix) {
                let ids = body_ids(&l)?;
                ref_ids.insert(ix, (ids.relation_sha256, ids.domain_sha256));
            }
        }
        sources.push(json!({"kind": "reference table", "path": rt, "identities": ref_ids.len(), "needed": need.len()}));
    }
    Ok(Loaded {
        cfg,
        rows,
        proofs,
        counts,
        ref_ids,
        sources,
    })
}

/// The class `class_of` gives the decision's evidence (it must equal the decision's: table-check re-derives it).
pub fn classed(d: &Decision) -> Classed {
    let ev = Evidence {
        cone: d.cone.clone(),
        sample: d.sample.clone(),
        legacy_proof: d.proof.as_ref().map(|_| true),
        ..Default::default()
    };
    class_of(Kind::Legacy, Origin::Import, None, &ev)
}

fn liveness(d: &Decision) -> Liveness {
    match d.dead {
        Some(true) => Liveness::Dead,
        Some(false) => Liveness::Live,
        None => Liveness::Unknown,
    }
}

/// Options of one import run.
#[derive(Clone, Debug, Default)]
pub struct Args {
    pub table: PathBuf,
    pub config: PathBuf,
    pub out: PathBuf,
    pub write_table: bool,
    pub policy: Option<PathBuf>,
}

fn put_side(side: &Path, area: &str, bytes: &[u8]) -> Result<String, String> {
    let sha = sha256_hex(bytes);
    let p = side.join(side_rel(area, &sha));
    // fsx rule: written only when proven absent; an unknown answer is an error, never "absent"
    let absent = match crate::fsx::presence("emission.side.put", &p) {
        crate::fsx::Presence::Present(_) => false,
        crate::fsx::Presence::Absent => true,
        crate::fsx::Presence::Unknown(e) => return Err(format!("{}: {e}", p.display())),
    };
    if absent {
        std::fs::create_dir_all(p.parent().expect("side area"))
            .map_err(|e| format!("{}: {e}", p.display()))?;
        let tmp = p.with_extension("tmp");
        std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(sha)
}

/// Run the import; returns (summary, every record admitted).
pub fn run(a: &Args) -> Result<(Value, bool), String> {
    let cfg: Config = serde_json::from_str(&read(&a.config.display().to_string())?.0)
        .map_err(|e| format!("{}: {e}", a.config.display()))?;
    let cone_bound = cfg.cone_bound;
    let ld = load(cfg)?;
    let policy: Policy = match &a.policy {
        Some(p) => serde_json::from_str(&read(&p.display().to_string())?.0)
            .map_err(|e| format!("{}: {e}", p.display()))?,
        None => Policy {
            name: "legacy-import".into(),
            ..Policy::default()
        },
    };
    let policy_text = serde_json::to_vec(&policy).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
    let name = a
        .table
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "T.table.jsonl".into());
    let out_table = a.out.join(&name);
    let part = a.out.join(format!("{name}.part"));
    let side = side_dir(&out_table);
    let mut tw = if a.write_table {
        put_side(&side, AREA_POLICY, &policy_text)?;
        Some(std::io::BufWriter::new(
            std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?,
        ))
    } else {
        None
    };
    let policy_sha = sha256_hex(&policy_text);
    // rows by own position and by relation
    let mut by_ref: BTreeMap<i64, Vec<&VerdictRow>> = BTreeMap::new();
    let mut by_tab: BTreeMap<i64, Vec<&VerdictRow>> = BTreeMap::new();
    let mut by_rel: BTreeMap<String, Vec<&VerdictRow>> = BTreeMap::new();
    for r in &ld.rows {
        match r.keyed_by {
            KeyedBy::Reference => by_ref.entry(r.index).or_default().push(r),
            KeyedBy::Table => by_tab.entry(r.index).or_default().push(r),
        }
        let h = r
            .hash
            .clone()
            .or_else(|| ld.ref_ids.get(&r.index).map(|x| x.0.clone()));
        if let Some(h) = h {
            by_rel.entry(h).or_default().push(r);
        }
    }
    let ref_ids = |i: i64| ld.ref_ids.get(&i).cloned();
    let f = std::fs::File::open(&a.table).map_err(|e| format!("{}: {e}", a.table.display()))?;
    let mut rd = std::io::BufReader::new(f);
    let mut hasher = Sha256::new();
    let mut classes = std::io::BufWriter::new(
        std::fs::File::create(a.out.join("classes.jsonl")).map_err(|e| e.to_string())?,
    );
    let mut pos: BTreeMap<String, u64> = BTreeMap::new();
    let mut words: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_class: BTreeMap<String, u64> = BTreeMap::new();
    let mut bindings: BTreeMap<String, u64> = BTreeMap::new();
    let mut refused: Vec<Value> = vec![];
    let mut contradictions: Vec<u64> = vec![];
    let mut identical: Vec<Value> = vec![];
    // reference index -> table position (to name the identical statement's own position in this table)
    let mut pos_of_ref: BTreeMap<i64, u64> = BTreeMap::new();
    let mut deferred_identical: Vec<(u64, i64)> = vec![];
    let mut rules = 0u64;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = rd
            .read_until(b'\n', &mut buf)
            .map_err(|e| format!("{}: {e}", a.table.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf);
        let raw = std::str::from_utf8(&buf).map_err(|e| format!("table not UTF-8: {e}"))?;
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let is_rule = string_member(line, "type").as_deref() == Some("rule");
        if !is_rule {
            if let Some(w) = tw.as_mut() {
                w.write_all(&buf).map_err(|e| e.to_string())?;
            }
            continue;
        }
        rules += 1;
        let ids = body_ids(line)?;
        let p = pos.entry(ids.chart.clone()).or_insert(0);
        let position = *p;
        *p += 1;
        let (index, dp) = index_and_dispatch(line);
        let rec = RecordId {
            position,
            index,
            dispatch_position: dp,
            relation_sha256: ids.relation_sha256.clone(),
            domain_sha256: ids.domain_sha256.clone(),
            line_sha256: sha256_hex(line.as_bytes()),
            chart: ids.chart.clone(),
        };
        let mut own: Vec<&VerdictRow> = vec![];
        if let Some(dp) = dp {
            own.extend(by_ref.get(&dp).into_iter().flatten().copied());
        }
        own.extend(
            by_tab
                .get(&(position as i64))
                .into_iter()
                .flatten()
                .copied(),
        );
        let own_ptr: BTreeSet<*const VerdictRow> = own.iter().map(|r| *r as *const _).collect();
        let same: Vec<&VerdictRow> = by_rel
            .get(&rec.relation_sha256)
            .into_iter()
            .flatten()
            .copied()
            .filter(|r| !own_ptr.contains(&(*r as *const _)))
            .collect();
        if let Some(dp) = dp {
            pos_of_ref.insert(dp, position);
        }
        let counts = ld.counts.get(&position);
        let d = decide(
            &rec,
            &own,
            &same,
            &ref_ids,
            ld.proofs.get(&position),
            counts,
            cone_bound,
        );
        let c = classed(&d);
        let mut problems: Vec<String> = vec![];
        if c.class != d.class {
            problems.push(format!(
                "class_of gives '{}' but the decision '{}'",
                c.class.word(),
                d.class.word()
            ));
        }
        if d.contradiction {
            contradictions.push(position);
            problems.push("LIVE record with a vacuous / dead verdict at its own position".into());
        }
        let sample_keys = d.sample.as_ref().map_or(0, |s| s.keys);
        if let Err(e) = policy.admit(Origin::Import, liveness(&d), &c, false, sample_keys) {
            problems.push(e);
        }
        let binding = d.proof.as_ref().map(|p| p.binding.clone());
        if let Some(b) = &binding
            && b.starts_with("identical")
        {
            let p = d.proof.as_ref().expect("proof");
            if p.row.keyed_by == KeyedBy::Reference {
                deferred_identical.push((position, p.row.index));
            }
            identical.push(
                json!({"position": position, "binding": b, "source": p.row.source,
                                  "row_index": p.row.index, "word": d.word}),
            );
        }
        *words
            .entry(if d.word.starts_with("sampled") {
                "sampled N keys".into()
            } else if d.word.starts_with(legacy::IDENTICAL_WORD) {
                legacy::IDENTICAL_WORD.to_string()
            } else {
                d.word.clone()
            })
            .or_default() += 1;
        *by_class.entry(c.class.word().to_string()).or_default() += 1;
        *bindings
            .entry(match (&d.proof, &d.cone, &d.sample) {
                (Some(p), _, _) if p.binding.starts_with("own") => {
                    format!("proof object at own position ({})", p.row.source)
                }
                (Some(p), _, _) => {
                    format!("proof object of the identical statement ({})", p.row.source)
                }
                (None, Some(_), _) => "proof lines: every first-match key".into(),
                (None, None, Some(_)) => "proof lines: sampled".into(),
                _ if d.dead == Some(true) => "no class evidence; dead".into(),
                _ => "no class evidence".into(),
            })
            .or_default() += 1;
        let row = json!({"chart": rec.chart, "position": position, "index": index, "dispatch_position": dp,
            "relation_sha256": rec.relation_sha256, "domain_sha256": rec.domain_sha256, "line_sha256": rec.line_sha256,
            "class": c.class.word(), "word": d.word, "liveness": counts.and_then(|c| c.liveness.clone()),
            "proof_object": d.proof.as_ref().map(|p| json!({"source": p.row.source, "line": p.row.line, "index": p.row.index,
                "class": p.row.class, "binding": p.binding})),
            "keys_proven": d.keys.as_ref().map(|k| k.len()), "fm_keys": counts.and_then(|c| c.fm_keys),
            "own_rows": d.own_rows, "stale_rows": d.stale_rows, "notes": d.notes, "problems": problems});
        writeln!(classes, "{row}").map_err(|e| e.to_string())?;
        if !problems.is_empty() {
            refused.push(json!({"position": position, "index": index, "problems": problems}));
        }
        if let Some(w) = tw.as_mut() {
            let row_sha = match &d.proof {
                Some(p) => Some(put_side(
                    &side,
                    AREA_EVID,
                    &serde_json::to_vec(&p.row.side_json(&rec.relation_sha256, &rec.domain_sha256))
                        .map_err(|e| e.to_string())?,
                )?),
                None => None,
            };
            if let Some(k) = &d.keys {
                put_side(&side, AREA_KEYS, keys_text(k).as_bytes())?;
            }
            let parts = Parts {
                kind: Kind::Legacy,
                origin: Origin::Import,
                body_sha256: ids.body_sha256.clone(),
                producer: None,
                certificate: None,
                check: Err("legacy record: no certificate (class from bound evidence)".into()),
                span: None,
                descent: None,
                symrows: None,
                classed: c.clone(),
                cone: d.cone.clone(),
                sample: d.sample.clone(),
                ladder: None,
                legacy: Some(legacy_member(&d, row_sha.as_deref(), counts, cone_bound)),
                policy: (policy.name.clone(), policy_sha.clone()),
            };
            let out = attach_derivation(line, &derivation(&parts))?;
            w.write_all(out.as_bytes()).map_err(|e| e.to_string())?;
            w.write_all(b"\n").map_err(|e| e.to_string())?;
        }
    }
    classes.flush().map_err(|e| e.to_string())?;
    // the identical statements' own positions in this table (reference index -> position), known after the scan
    for x in identical.iter_mut() {
        if let (Some(p), Some(ix)) = (x["position"].as_u64(), x["row_index"].as_i64())
            && deferred_identical.contains(&(p, ix))
        {
            x["identical_statement_position"] = json!(pos_of_ref.get(&ix));
        }
    }
    let table_sha = hasher.finish_hex();
    // position-only count rows must have been made on THIS table
    let mut errors: Vec<String> = vec![];
    for c in &ld.cfg.counts {
        if let Some(t) = &c.table_sha256
            && *t != table_sha
        {
            errors.push(format!(
                "counts {}: made on table {t}, not on this table {table_sha}",
                c.name
            ));
        }
    }
    let ok = refused.is_empty() && errors.is_empty() && rules > 0;
    if let Some(mut w) = tw {
        w.flush().map_err(|e| e.to_string())?;
        drop(w);
        if ok {
            std::fs::rename(&part, &out_table).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(&part).map_err(|e| e.to_string())?;
        }
    }
    let summary = json!({"schema": "rustred.campaign.legacy-import.summary.v1", "table": a.table.display().to_string(),
        "table_sha256": table_sha, "config": a.config.display().to_string(), "cone_bound": cone_bound, "rules": rules,
        "by_word": words, "by_class": by_class,
        "summary_line": summary_line(&words, &identical), "by_binding": bindings, "identical_statement": identical,
        "contradictions": contradictions, "refused": refused.len(), "refusals": refused.iter().take(50).collect::<Vec<_>>(),
        "errors": errors, "sources": ld.sources, "policy": policy.name, "policy_sha256": policy_sha,
        "written_table": (a.write_table && ok).then(|| out_table.display().to_string()),
        "verdict": if ok { "IMPORTED" } else { "REFUSED" },
        "rule": "whole domain <- a whole-domain proof object bound at the record's own position (relation AND domain sha256 equal) or of the IDENTICAL statement (equal relation and domain) at another position; else every first-match key in cone <- proof lines on this line's bytes prove, shell by shell D + r <= bound, the complete first-match count; else sampled N <- N distinct proven keys; delivery word dead in cone <- the counts say dead (and no whole-domain proof object); vacuous / dead verdicts and per-key verdict text give no class; a live record with a vacuous verdict at its own position is refused"});
    std::fs::write(
        a.out.join("summary.json"),
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok((summary, ok))
}

/// The one-line summary: the identical-statement sub-class is named and counted APART from `whole domain`.
fn summary_line(words: &BTreeMap<String, u64>, identical: &[Value]) -> String {
    let g = |k: &str| words.get(k).copied().unwrap_or(0);
    let id: Vec<String> = identical
        .iter()
        .map(|x| {
            format!(
                "{} <- position {}",
                x["position"],
                x["identical_statement_position"]
                    .as_u64()
                    .map_or_else(|| format!("index {}", x["row_index"]), |p| p.to_string())
            )
        })
        .collect();
    format!(
        "whole domain {} (+ {} whole domain BY IDENTICAL STATEMENT at another position, not yet on their own bytes: {}), every first-match key in cone {}, sampled N keys {}, dead in cone {}, none {}",
        g("whole domain"),
        g(legacy::IDENTICAL_WORD),
        if id.is_empty() {
            "-".into()
        } else {
            id.join(", ")
        },
        g("every first-match key in cone"),
        g("sampled N keys"),
        g("dead in cone"),
        g("none")
    )
}

/// CLI: `legacy-import --table T --config C --out DIR [--write-table] [--policy FILE]`. Exit 0 imported, 3 refused.
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let mut a = Args::default();
    let mut i = 0;
    let val = |i: usize| {
        rest.get(i + 1)
            .cloned()
            .ok_or(format!("{} needs a value", rest[i]))
    };
    while i < rest.len() {
        match rest[i].as_str() {
            "--table" => {
                a.table = val(i)?.into();
                i += 1;
            }
            "--config" => {
                a.config = val(i)?.into();
                i += 1;
            }
            "--out" => {
                a.out = val(i)?.into();
                i += 1;
            }
            "--policy" => {
                a.policy = Some(val(i)?.into());
                i += 1;
            }
            "--write-table" => a.write_table = true,
            o => return Err(format!("unknown flag {o}")),
        }
        i += 1;
    }
    if a.table.as_os_str().is_empty()
        || a.config.as_os_str().is_empty()
        || a.out.as_os_str().is_empty()
    {
        return Err(
            "legacy-import --table T --config C --out DIR [--write-table] [--policy FILE]".into(),
        );
    }
    let (s, ok) = run(&a)?;
    let mut short = s.clone();
    if let Some(o) = short.as_object_mut() {
        o.remove("sources");
        o.remove("refusals");
    }
    Ok((short.to_string(), ok))
}
