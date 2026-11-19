//! RELATION ENTRIES from emitted records (emission contract section 16; round 5 H0 GAP-2 + GAP-3, lane t266's request).
//!
//! GAP-2, the d = 4 regularity acceptance (section 13) against ONE KEY of an old object: the old object of a key K is
//! (a) the stored `rustred.relation-entry.v1` the engine would serve at K (the best-ranked entry of the given entry files:
//!     evidence, then pole order at d = 4, then rhs terms, then digest, as the engine ranks), its id = its entry_digest; or
//! (b) the table rule that serves K as FIRST MATCH in the old table (the engine's dispatch), restricted to K (an entry
//!     placed over the table shadows that rule at K only), its id = the rule's body sha256.
//! Same exact measure and rule as section 13: worse at K = refused; the verdict is a side file and the record's
//! derivation names it (`replaces {old_body_sha256, regularity_sha256}`).
//!
//! GAP-3, the WRITER: every record of an EMITTED table (table-check accepted: derivation, descent, check) whose key has
//! an old object NOT WORSE becomes one `rustred.relation-entry.v1` line in a relation root `<out>/<node>-<id16>/` whose
//! MANIFEST.json says `entries.precedence: over_table` (the engine reads MANIFEST.json; NODE.json is written too). The
//! entry BODY carries the emitted record line itself (`emitted_record`: derivation, regularity, descent members inside,
//! hashed into the entry_digest) and its side directory is copied beside the entry file. FAIL CLOSED: one refusal and
//! nothing is written.
//!
//! ENTRY-CHECK: an entry is refused without its emitted record, without a derivation of an EMITTED record (origin lift),
//! without its regularity verdict (`replaces`), without a LOWER descent verdict, when table-check's record check refuses
//! the record, or when the entry's key / target / rhs / pole order are not the record's.
use super::regcheck::{compare, dispatcher_of, header_of, read_line_at};
use super::tablecheck;
use rustred_campaign_core::emission::body::{attach_derivation, body_ids, sha256_hex, strip_derivation};
use rustred_campaign_core::emission::regularity::{KeySet, Relation, Verdict, keys_sha256, regularity_at};
use rustred_campaign_core::emission::validate::check_record;
use rustred_campaign_core::emission::{AREA_REGULARITY, side_dir, side_rel};
use rustred_campaign_core::schema::canon::canonical_json;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub const ENTRY_SCHEMA: &str = "rustred.relation-entry.v1";
pub const MANIFEST_SCHEMA: &str = "rustred.node-service-manifest.v1";
/// the determinism line of an emitted relation (the engine accepts `rustred.relation-import.v1/` entries)
pub const IMPORT_MAJOR: &str = "rustred.relation-import.v1/";
const ENTRY_FILE: &str = "entries-emitted.jsonl";

/// `I(k0,k1,...)`
pub fn key_text(k: &[i64]) -> String {
    format!("I({})", k.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))
}

/// The entry digest the engine checks: sha256 of the canonical JSON of the body.
pub fn entry_digest(body: &Value) -> Result<String, String> {
    Ok(sha256_hex(canonical_json(body, true)?.as_bytes()))
}

fn evidence_rank(level: &str) -> u8 {
    match level {
        "per-key" => 2,
        "imported" => 1,
        _ => 0,
    }
}

/// The engine's rank of an entry (larger first; ties: the smaller digest).
fn rank(e: &Value) -> (u8, i64, i64) {
    let ev = evidence_rank(e["evidence"]["level"].as_str().unwrap_or("none"));
    let pole = e["diagnostics"]["pole_order_at"]["4"].as_i64().unwrap_or(i64::MAX / 2);
    let terms = e["body"]["rhs"].as_array().map_or(i64::MAX / 2, |r| r.len() as i64);
    (ev, -pole, -terms)
}

/// An old stored entry at a key.
#[derive(Clone, Debug)]
pub struct OldEntry {
    pub key: Vec<i64>,
    pub digest: String,
    pub file: PathBuf,
    pub body: Value,
    pub candidates: usize,
    /// the engine's rank inputs (evidence, -pole, -terms)
    pub rank: (u8, i64, i64),
}

/// Scan entry files for `keys` (a prefix filter on the `"key"` member before parsing; lines may be MBs) and keep the
/// entry the engine would serve per key. Entries of another node identity are skipped.
pub fn find_entries(
    files: &[PathBuf],
    node_identity: &str,
    keys: &BTreeSet<Vec<i64>>,
) -> Result<BTreeMap<Vec<i64>, OldEntry>, String> {
    let wanted: BTreeMap<String, Vec<i64>> = keys
        .iter()
        .flat_map(|k| {
            let compact = serde_json::to_string(k).unwrap_or_default();
            let spaced = compact.replace(',', ", ");
            [(compact, k.clone()), (spaced, k.clone())]
        })
        .collect();
    let mut out: BTreeMap<Vec<i64>, OldEntry> = BTreeMap::new();
    for f in files {
        let fh = std::fs::File::open(f).map_err(|e| format!("{}: {e}", f.display()))?;
        let mut r = std::io::BufReader::with_capacity(1 << 22, fh);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            if r.read_until(b'\n', &mut buf).map_err(|e| format!("{}: {e}", f.display()))? == 0 {
                break;
            }
            // the envelope members come first (schema, node, node_identity, key_frame, key): look at the head only
            let head = String::from_utf8_lossy(&buf[..buf.len().min(1024)]);
            let Some(at) = head.find("\"key\":") else { continue };
            let rest = head[at + 6..].trim_start();
            let Some(end) = rest.find(']') else { continue };
            let Some(k) = wanted.get(&rest[..=end]) else { continue };
            let e: Value = match serde_json::from_slice(&buf) {
                Ok(e) => e,
                Err(_) => continue, // a torn line (the engine ignores it too)
            };
            if e["schema"] != ENTRY_SCHEMA || e["node_identity"] != node_identity || e["key"] != json!(k) {
                continue;
            }
            let digest = e["entry_digest"].as_str().unwrap_or("").to_string();
            if entry_digest(&e["body"]).ok().as_deref() != Some(digest.as_str()) {
                continue; // corrupt (the engine refuses it)
            }
            let r = rank(&e);
            let better = match out.get(k) {
                None => true,
                Some(o) => (r, std::cmp::Reverse(digest.as_str())) > (o.rank, std::cmp::Reverse(o.digest.as_str())),
            };
            let n = out.get(k).map_or(0, |o| o.candidates) + 1;
            if better {
                out.insert(k.clone(), OldEntry { key: k.clone(), digest, file: f.clone(), body: e["body"].clone(), candidates: n, rank: r });
            } else if let Some(o) = out.get_mut(k) {
                o.candidates = n;
            }
        }
    }
    Ok(out)
}

/// The relation text of an entry body for the measure (`target` + `rhs`; an import body without `target` gets I(key)).
pub fn entry_relation(body: &Value, key: &[i64]) -> String {
    let mut b = json!({"target": body["target"], "rhs": body["rhs"]});
    if b["target"].is_null() {
        b["target"] = json!(key_text(key));
    }
    b.to_string()
}

/// GAP-2: the d = 4 verdict of `pieces` against the old relation `old_rel` at ONE key. `old_kind` / `old_id` name the
/// old object (a stored entry and its digest, or a table rule at the key and its body sha256).
#[allow(clippy::too_many_arguments)]
pub fn verdict_at_key(
    old_kind: &str,
    old_id: &str,
    old_rel: &str,
    key: &[i64],
    pieces: &[String],
    header: &str,
    chart: &str,
    work: &Path,
) -> Result<Verdict, String> {
    let ks = KeySet {
        kind: old_kind.to_string(),
        cone: format!("the one key {}", key_text(key)),
        served: 1,
        compared: 0,
        sampling: None,
        keys_sha256: keys_sha256(&[key.to_vec()]),
    };
    let mut v = compare(old_rel, pieces, &[key.to_vec()], ks, header, chart, work, 1)?;
    v.old_body_sha256 = old_id.to_string();
    Ok(v)
}

/// The old object of a key: an entry of `old` (GAP-2 a), else the first-match rule of `table` (GAP-2 b).
pub enum OldAt {
    Entry(OldEntry),
    Rule { table: PathBuf, index: i64, line: String, body_sha256: String },
}

impl OldAt {
    pub fn kind(&self) -> String {
        match self {
            OldAt::Entry(_) => "one key of a stored relation entry".into(),
            OldAt::Rule { index, .. } => format!("one key of table rule {index} (first match)"),
        }
    }
    pub fn id(&self) -> String {
        match self {
            OldAt::Entry(e) => e.digest.clone(),
            OldAt::Rule { body_sha256, .. } => body_sha256.clone(),
        }
    }
    pub fn relation(&self, key: &[i64]) -> Result<String, String> {
        match self {
            OldAt::Entry(e) => Ok(entry_relation(&e.body, key)),
            OldAt::Rule { line, .. } => Ok(strip_derivation(line)?.0),
        }
    }
    pub fn describe(&self) -> Value {
        match self {
            OldAt::Entry(e) => json!({"kind": "stored relation entry", "entry_digest": e.digest, "file": e.file.display().to_string(),
                                      "rhs_terms": e.body["rhs"].as_array().map(Vec::len), "entries_at_key": e.candidates}),
            OldAt::Rule { table, index, body_sha256, .. } => json!({"kind": "table rule at one key", "table": table.display().to_string(),
                                      "rule": index, "body_sha256": body_sha256}),
        }
    }
}

/// The first-match rule of `table` at `key` (the engine's dispatch).
pub fn rule_at_key(table: &Path, disp: &rustred_campaign_core::cover::Dispatcher, key: &[i64]) -> Result<Option<OldAt>, String> {
    let k32: Vec<i32> = key.iter().map(|&x| x as i32).collect();
    let Some(w) = disp.first_match(&k32).ok().and_then(|d| d.winner()) else {
        return Ok(None);
    };
    let index = w.label as i64;
    let line = read_line_at(table, index, None)?;
    let (b, _) = strip_derivation(&line)?;
    Ok(Some(OldAt::Rule { table: table.to_path_buf(), index, body_sha256: body_ids(&b)?.body_sha256, line }))
}

/// "top" when the daughter has the target's positive lines, else "sub" (the engine record's third rhs member).
fn top_or_sub(chart: &str, target: &[i64], daughter: &str) -> String {
    let d: Vec<i64> = daughter
        .trim_start_matches("I(")
        .trim_end_matches(')')
        .split(',')
        .map(|x| x.trim().parse().unwrap_or(0))
        .collect();
    let same = chart
        .bytes()
        .enumerate()
        .filter(|(_, c)| *c == b'1')
        .all(|(i, _)| (target.get(i).copied().unwrap_or(0) > 0) == (d.get(i).copied().unwrap_or(0) > 0));
    if same { "top".into() } else { "sub".into() }
}

/// Arguments of [`write_entries`].
#[derive(Clone, Debug, Default)]
pub struct WriteArgs {
    pub emitted: PathBuf,
    pub node: String,
    pub node_identity: String,
    pub out: PathBuf,
    pub revision: String,
    pub old_entries: Vec<PathBuf>,
    pub old_table: Option<PathBuf>,
    pub work: PathBuf,
    pub descent_recheck: Option<PathBuf>,
}

/// The node directory of a relation root.
pub fn node_dir(out: &Path, node: &str, node_identity: &str) -> PathBuf {
    let safe: String = node.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    out.join(format!("{safe}-{}", node_identity.get(..16).unwrap_or(node_identity)))
}

/// GAP-3: write the entries of an emitted table (see the module text). Returns (summary, written).
pub fn write_entries(a: &WriteArgs) -> Result<(Value, bool), String> {
    let t0 = std::time::Instant::now();
    // 1. the emitted table must pass table-check (record level; + the descent re-run when asked)
    let (tc, ok) = tablecheck::table_check(&tablecheck::Args {
        table: a.emitted.clone(),
        every: 1,
        work: Some(a.work.join("table-check")),
        descent_recheck: a.descent_recheck.clone(),
        ..Default::default()
    })?;
    if !ok {
        return Ok((json!({"verdict": "REFUSED (nothing written)", "reason": "the emitted table does not pass table-check", "table_check": tc}), false));
    }
    let header = header_of(&a.emitted)?;
    let text = std::fs::read_to_string(&a.emitted).map_err(|e| format!("{}: {e}", a.emitted.display()))?;
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| rustred_campaign_core::emission::body::string_member(l, "type").as_deref() == Some("rule"))
        .collect();
    // 2. the keys (point records only)
    let mut recs = vec![];
    let mut refusals: Vec<Value> = vec![];
    for l in &lines {
        let (b, d) = strip_derivation(l)?;
        let v: Value = serde_json::from_str(&b).map_err(|e| e.to_string())?;
        let key: Option<Vec<i64>> = v["fixed"].as_array().and_then(|a| a.iter().map(Value::as_i64).collect());
        match key {
            Some(k) => recs.push((l.to_string(), b, d.unwrap_or_default(), v, k)),
            None => refusals.push(json!({"index": v["index"], "reason": "not a point record (a relation entry serves one key)"})),
        }
    }
    let keys: BTreeSet<Vec<i64>> = recs.iter().map(|r| r.4.clone()).collect();
    if keys.len() != recs.len() {
        refusals.push(json!({"reason": "two records at the same key"}));
    }
    // 3. the old objects
    let olds = if a.old_entries.is_empty() { BTreeMap::new() } else { find_entries(&a.old_entries, &a.node_identity, &keys)? };
    let disp = match &a.old_table {
        Some(t) => Some(dispatcher_of(t)?.0),
        None => None,
    };
    let side = side_dir(&a.emitted);
    let mut entries = vec![];
    let mut verdicts = vec![];
    for (i, (_line, body, der, v, key)) in recs.iter().enumerate() {
        let old = match olds.get(key) {
            Some(e) => Some(OldAt::Entry(e.clone())),
            None => match (&a.old_table, &disp) {
                (Some(t), Some(d)) => rule_at_key(t, d, key)?,
                _ => None,
            },
        };
        let Some(old) = old else {
            refusals.push(json!({"index": v["index"], "key": key, "reason": "no old object at the key (no stored entry, no first-match rule): nothing to compare (contract section 16)"}));
            continue;
        };
        let chart = v["chart"].as_str().unwrap_or("").to_string();
        let ver = verdict_at_key(&old.kind(), &old.id(), &old.relation(key)?, key, &[body.clone()], &header, &chart, &a.work.join(format!("reg-{i}")))?;
        let mut vj = serde_json::to_value(&ver).map_err(|e| e.to_string())?;
        vj["old"] = old.describe();
        let vb = serde_json::to_vec(&vj).map_err(|e| e.to_string())?;
        let vsha = sha256_hex(&vb);
        let vp = side.join(side_rel(AREA_REGULARITY, &vsha));
        std::fs::create_dir_all(vp.parent().expect("area")).map_err(|e| e.to_string())?;
        std::fs::write(&vp, &vb).map_err(|e| format!("{}: {e}", vp.display()))?;
        let pole_old = regularity_at(&Relation::parse(&old.relation(key)?)?, key)?.max_pole;
        let pole_new = regularity_at(&Relation::parse(body)?, key)?.max_pole;
        verdicts.push(json!({"key": key, "old": old.describe(), "old_pole": pole_old, "new_pole": pole_new, "not_worse": ver.not_worse,
                             "regularity_sha256": vsha}));
        if !ver.not_worse || ver.keys.compared != 1 {
            refusals.push(json!({"index": v["index"], "key": key, "reason": format!("regularity_d4_worse at the key (old pole {pole_old}, new {pole_new})")}));
            continue;
        }
        // the record's derivation names the verdict (it is a replacement of the old object at the key)
        let mut dv: Value = serde_json::from_str(der).map_err(|e| format!("derivation: {e}"))?;
        if dv["descent"]["verdict"] != "LOWER" {
            refusals.push(json!({"index": v["index"], "key": key, "reason": "the record's descent verdict is not LOWER"}));
            continue;
        }
        dv["replaces"] = json!({"old_body_sha256": old.id(), "regularity_sha256": vsha});
        let stamped = attach_derivation(body, &dv)?;
        let kind = v["origin"]["kind"].as_str().unwrap_or("point").to_string();
        let target = v["target"].as_str().unwrap_or("").to_string();
        let rhs: Vec<Value> = v["rhs"]
            .as_array()
            .ok_or("record without rhs")?
            .iter()
            .map(|t| json!([t[0], t[1], top_or_sub(&chart, key, t[0].as_str().unwrap_or(""))]))
            .collect();
        let rec_sha = body_ids(body)?.body_sha256;
        let ebody = json!({"type": "relation", "key": key, "target": target, "verdict": "SOLVED", "all_lower": true,
            "proof": {"ok": true, "kind": format!("emitted/{}", dv["check"]["mode"].as_str().unwrap_or("weight-cert")),
                      "tool_sha256": dv["check"]["tool_sha256"], "output_sha256": dv["check"]["output_sha256"],
                      "certificate_sha256": dv["certificate"]["sha256"], "descent_output_sha256": dv["descent"]["output_sha256"]},
            "relation_digest": Value::Null,
            "determinism_id": format!("{IMPORT_MAJOR}kind=emitted-{kind}/statement={rec_sha}"),
            "rhs": rhs, "rhs_count": v["rhs"].as_array().map(Vec::len),
            "emitted_record": stamped});
        let digest = entry_digest(&ebody)?;
        let rid = format!("{}/emitted/{}", a.node, &rec_sha[..16]);
        entries.push(json!({"schema": ENTRY_SCHEMA, "node": a.node, "node_identity": a.node_identity, "key_frame": "table",
            "key": key, "entry_digest": digest, "body": ebody,
            "recipe": {"id": rid, "file_sha256": Value::Null, "engine_digest": Value::Null},
            "bound_to": {"determinism_id": ebody["determinism_id"], "node_identity": a.node_identity,
                         "relation_rule": format!("emitted-{kind}"), "package_revision": a.revision},
            "evidence": {"level": "per-key", "proof": {"tool": "rustred-campaign emission contract (independent checker)",
                         "tool_digest": dv["check"]["tool_sha256"], "record_sha256": digest, "verdict": "SOLVED"},
                         "fact": rec_sha},
            "diagnostics": {"pole_order_at": {"4": pole_new}, "regular_alternative": "not_tried", "recipe": rid,
                            "outside_certified_set": false, "replaces": old.describe(), "old_pole_order_at_4": pole_old},
            "written": {"by": "rustred-campaign emit-entries"}}));
    }
    let summary = |written: Option<&Path>| {
        json!({"schema": "rustred.campaign.emit-entries.v1", "emitted": a.emitted.display().to_string(), "records": lines.len(),
               "entries": entries.len(), "refused": refusals.len(), "refusals": refusals.iter().take(50).collect::<Vec<_>>(),
               "regularity": verdicts, "root": written.map(|p| p.display().to_string()),
               "seconds": t0.elapsed().as_secs_f64(),
               "verdict": if written.is_some() { "WRITTEN" } else { "REFUSED (nothing written)" }})
    };
    if !refusals.is_empty() || entries.is_empty() {
        return Ok((summary(None), false));
    }
    // 4. the relation root (fail closed: never over a directory that holds entries)
    let dir = node_dir(&a.out, &a.node, &a.node_identity);
    if dir.is_dir()
        && std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
    {
        return Err(format!("{} already holds entries", dir.display()));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut merged = Vec::new();
    let mut index = String::new();
    for e in &entries {
        let line = serde_json::to_string(e).map_err(|x| x.to_string())?;
        // the engine's rank (evidence, -pole, -terms): read_index defaults a missing rank to (0, 0, 0), and merge_layer lets
        // a LATER layer with higher evidence replace an earlier one at the key (a per-key r2 entry would displace ours)
        let r = rank(e);
        index.push_str(&format!("{}\n", json!({"key": e["key"], "file": ENTRY_FILE, "offset": merged.len(), "len": line.len(),
                                                  "entry_digest": e["entry_digest"], "rank": [r.0, r.1, r.2]})));
        merged.extend_from_slice(line.as_bytes());
        merged.push(b'\n');
    }
    let ef = dir.join(ENTRY_FILE);
    std::fs::write(&ef, &merged).map_err(|e| format!("{}: {e}", ef.display()))?;
    std::fs::write(dir.join("index.jsonl"), &index).map_err(|e| e.to_string())?;
    super::carry_side(&side, &side_dir(&ef))?;
    let regular = entries.iter().filter(|e| e["diagnostics"]["pole_order_at"]["4"] == 0).count();
    let manifest = json!({"schema": MANIFEST_SCHEMA, "node": a.node, "node_identity": a.node_identity, "key_frame": "table",
        "revision": a.revision,
        "entries": {"count": entries.len(), "bytes": merged.len(), "index": "index.jsonl",
                    "files": [{"file": ENTRY_FILE, "sha256": sha256_hex(&merged), "entries": entries.len()}],
                    "precedence": "over_table", "over_table_keys": entries.iter().map(|e| e["key"].clone()).collect::<Vec<_>>(),
                    "kinds": {"recipe_solve": 0, "import": entries.len()}},
        "verification": {"entries_checked": entries.len(), "d4": {"entry_regular": regular, "entry_pole": entries.len() - regular},
                         "tool": "rustred-campaign emit-entries (emission contract sections 13-16)",
                         "note": "every entry carries its emitted record (derivation: check, class, descent LOWER, d = 4 verdict \
                                  NOT WORSE against the old object at its key); entry-check re-verifies them"},
        "layer_order": "list this root BEFORE the root of the entries it replaces: at a key held by two layers of equal \
                        evidence the engine keeps the FIRST layer (relation_entries.rs merge_layer); over_table puts it before the table"});
    std::fs::write(dir.join("MANIFEST.json"), serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("NODE.json"),
        json!({"node": a.node, "node_identity": a.node_identity, "note": "written by rustred-campaign emit-entries; precedence in MANIFEST.json"})
            .to_string(),
    )
    .map_err(|e| e.to_string())?;
    // 5. self-check
    let (ec, ok) = entry_check(&dir)?;
    let mut s = summary(Some(&dir));
    s["entry_check"] = ec;
    if !ok {
        s["verdict"] = json!("WRITTEN BUT ENTRY-CHECK REFUSED (do not ship)");
    }
    let mut sums = String::new();
    for f in [ENTRY_FILE, "index.jsonl", "MANIFEST.json", "NODE.json"] {
        let b = std::fs::read(dir.join(f)).map_err(|e| e.to_string())?;
        sums.push_str(&format!("{}  {f}\n", sha256_hex(&b)));
    }
    std::fs::write(dir.join("SHA256SUMS"), sums).map_err(|e| e.to_string())?;
    Ok((s, ok))
}

/// ENTRY-CHECK of a relation node directory (every `*.jsonl` entry file but the index; side directory beside each).
pub fn entry_check(dir: &Path) -> Result<(Value, bool), String> {
    let mut n = 0u64;
    let mut refused: Vec<Value> = vec![];
    let mut n_refused = 0u64;
    let manifest: Value = std::fs::read(dir.join("MANIFEST.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|x| x == "jsonl")
                && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("index"))
        })
        .collect();
    files.sort();
    for f in &files {
        let side = side_dir(f);
        let lookup = |area: &str, sha: &str| std::fs::read(side.join(side_rel(area, sha))).ok();
        let fh = std::fs::File::open(f).map_err(|e| format!("{}: {e}", f.display()))?;
        for (ln, l) in std::io::BufReader::new(fh).lines().enumerate() {
            let l = l.map_err(|e| format!("{}: {e}", f.display()))?;
            if l.trim().is_empty() {
                continue;
            }
            n += 1;
            let problems = check_entry(&l, &manifest, &lookup);
            if !problems.is_empty() {
                n_refused += 1;
                if refused.len() < 50 {
                    refused.push(json!({"file": f.display().to_string(), "line": ln + 1, "problems": problems}));
                }
            }
        }
    }
    let ok = n > 0 && n_refused == 0;
    Ok((
        json!({"schema": "rustred.campaign.entry-check.v1", "dir": dir.display().to_string(), "entries": n, "refused": n_refused,
               "refusals": refused, "verdict": if ok { "ACCEPTED" } else { "REFUSED" }}),
        ok,
    ))
}

/// The problems of one entry line (empty = accepted).
pub fn check_entry(l: &str, manifest: &Value, lookup: &dyn Fn(&str, &str) -> Option<Vec<u8>>) -> Vec<String> {
    let mut p = vec![];
    let e: Value = match serde_json::from_str(l) {
        Ok(e) => e,
        Err(x) => return vec![format!("not JSON: {x}")],
    };
    if e["schema"] != ENTRY_SCHEMA || e["key_frame"] != "table" {
        p.push(format!("schema {} / key frame {}", e["schema"], e["key_frame"]));
    }
    if !manifest.is_null() && e["node_identity"] != manifest["node_identity"] {
        p.push("another node identity than the manifest's".into());
    }
    let body = &e["body"];
    match entry_digest(body) {
        Ok(d) if e["entry_digest"].as_str() == Some(d.as_str()) => {}
        _ => p.push("entry_digest is not the digest of the body".into()),
    }
    if body["key"] != e["key"] || body["verdict"] != "SOLVED" || body["all_lower"] != json!(true) || body["proof"]["ok"] != json!(true) {
        p.push("the body is not a proved, lower relation of the entry's key".into());
    }
    let Some(rec) = body["emitted_record"].as_str() else {
        p.push("an entry without its emitted record (derivation, regularity, descent: contract section 16)".into());
        return p;
    };
    let (rb, der) = match strip_derivation(rec) {
        Ok(x) => x,
        Err(x) => {
            p.push(format!("emitted record: {x}"));
            return p;
        }
    };
    let Some(der) = der.and_then(|d| serde_json::from_str::<Value>(&d).ok()) else {
        p.push("the emitted record has no derivation member".into());
        return p;
    };
    if der["origin"] != "lift" {
        p.push("the record was not emitted (origin is not lift)".into());
    }
    if der["replaces"]["regularity_sha256"].as_str().is_none() {
        p.push("an entry without its d = 4 regularity verdict against the old object at its key (contract sections 13, 16)".into());
    }
    if der["descent"]["verdict"] != "LOWER" {
        p.push("an entry without a LOWER descent verdict (contract section 14)".into());
    }
    let rv: Value = serde_json::from_str(&rb).unwrap_or(Value::Null);
    let index = rv["index"].as_i64().unwrap_or(-1);
    let verdict = check_record(rec, index.max(0) as u64, lookup, &|_| None);
    for x in verdict.problems {
        p.push(format!("record: {x}"));
    }
    if rv["fixed"] != e["key"] || rv["target"] != body["target"] {
        p.push("the entry's key / target is not the record's".into());
    }
    let pairs = |a: &Value| -> Vec<(Value, Value)> {
        a.as_array().map(|v| v.iter().map(|t| (t[0].clone(), t[1].clone())).collect()).unwrap_or_default()
    };
    if pairs(&rv["rhs"]) != pairs(&body["rhs"]) {
        p.push("the entry's rhs is not the record's".into());
    }
    let key: Option<Vec<i64>> = e["key"].as_array().and_then(|a| a.iter().map(Value::as_i64).collect());
    if let Some(k) = key {
        match Relation::parse(&rb).and_then(|r| regularity_at(&r, &k)) {
            Ok(r) if e["diagnostics"]["pole_order_at"]["4"].as_u64() == Some(r.max_pole as u64) => {}
            Ok(r) => p.push(format!("stated pole order {} but the relation has {} at its key", e["diagnostics"]["pole_order_at"]["4"], r.max_pole)),
            Err(x) => p.push(format!("measure: {x}")),
        }
    }
    p
}

/// CLI `emit-entries --emitted T --node N --node-identity ID --out ROOT --revision R [--old-entries F]... [--old-table T]
/// [--work DIR] [--descent-node BIN]`: exit 0 written, 3 refused (nothing written).
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let mut a = WriteArgs::default();
    let mut i = 0;
    let val = |i: usize| rest.get(i + 1).cloned().ok_or(format!("{} needs a value", rest[i]));
    while i < rest.len() {
        match rest[i].as_str() {
            "--emitted" => a.emitted = val(i)?.into(),
            "--node" => a.node = val(i)?,
            "--node-identity" => a.node_identity = val(i)?,
            "--out" => a.out = val(i)?.into(),
            "--revision" => a.revision = val(i)?,
            "--old-entries" => a.old_entries.push(val(i)?.into()),
            "--old-table" => a.old_table = Some(val(i)?.into()),
            "--work" => a.work = val(i)?.into(),
            "--descent-node" => a.descent_recheck = Some(val(i)?.into()),
            x => return Err(format!("unknown flag {x}")),
        }
        i += 2;
    }
    if a.emitted.as_os_str().is_empty() || a.node.is_empty() || a.node_identity.is_empty() || a.out.as_os_str().is_empty() {
        return Err("emit-entries --emitted T --node N --node-identity ID --out ROOT [--revision R] [--old-entries F]... [--old-table T] [--work DIR] [--descent-node BIN]".into());
    }
    if a.work.as_os_str().is_empty() {
        a.work = std::env::temp_dir().join(format!("emit-entries-{}", std::process::id()));
    }
    if a.revision.is_empty() {
        a.revision = "emitted".into();
    }
    let (s, ok) = write_entries(&a)?;
    Ok((s.to_string(), ok))
}

/// CLI `entry-check --dir NODE_DIR`: exit 0 accepted, 2 refused.
pub fn check_cmd(rest: &[String]) -> Result<(String, bool), String> {
    let dir = match rest {
        [f, d] if f == "--dir" => PathBuf::from(d),
        _ => return Err("entry-check --dir NODE_DIR".into()),
    };
    let (s, ok) = entry_check(&dir)?;
    Ok((s.to_string(), ok))
}

/// CLI (GAP-2 standalone) `regularity-at-key --key JSON --pieces P.jsonl --header-table T (--old-entries F... --node-identity ID
/// | --old-table T) --out DIR`: exit 0 not worse, 3 worse.
pub fn at_key_cmd(rest: &[String]) -> Result<(String, bool), String> {
    let (mut key, mut pieces, mut out, mut old_table, mut header_table, mut ident) = (None, None, None, None, None, None);
    let mut olds: Vec<PathBuf> = vec![];
    let mut i = 0;
    let val = |i: usize| rest.get(i + 1).cloned().ok_or(format!("{} needs a value", rest[i]));
    while i < rest.len() {
        match rest[i].as_str() {
            "--key" => key = Some(serde_json::from_str::<Vec<i64>>(&val(i)?).map_err(|e| format!("--key: {e}"))?),
            "--pieces" => pieces = Some(PathBuf::from(val(i)?)),
            "--out" => out = Some(PathBuf::from(val(i)?)),
            "--old-table" => old_table = Some(PathBuf::from(val(i)?)),
            "--header-table" => header_table = Some(PathBuf::from(val(i)?)),
            "--old-entries" => olds.push(val(i)?.into()),
            "--node-identity" => ident = Some(val(i)?),
            x => return Err(format!("unknown flag {x}")),
        }
        i += 2;
    }
    let (Some(key), Some(pieces), Some(out)) = (key, pieces, out) else {
        return Err("regularity-at-key --key JSON --pieces P.jsonl --out DIR (--old-entries F... --node-identity ID | --old-table T) [--header-table T]".into());
    };
    let lines: Vec<String> = std::fs::read_to_string(&pieces)
        .map_err(|e| format!("{}: {e}", pieces.display()))?
        .lines()
        .filter(|l| rustred_campaign_core::emission::body::string_member(l, "type").as_deref() == Some("rule"))
        .map(str::to_string)
        .collect();
    let old = if !olds.is_empty() {
        let id = ident.ok_or("--old-entries needs --node-identity")?;
        let set: BTreeSet<Vec<i64>> = [key.clone()].into();
        find_entries(&olds, &id, &set)?.remove(&key).map(OldAt::Entry)
    } else if let Some(t) = &old_table {
        let (d, _) = dispatcher_of(t)?;
        rule_at_key(t, &d, &key)?
    } else {
        return Err("give --old-entries or --old-table".into());
    };
    let old = old.ok_or("no old object at the key")?;
    let ht = header_table.or(old_table).ok_or("--header-table T (the table frame of the pieces)")?;
    let header = header_of(&ht)?;
    let (b0, _) = strip_derivation(lines.first().ok_or("no piece")?)?;
    let chart = serde_json::from_str::<Value>(&b0).ok().and_then(|v| v["chart"].as_str().map(str::to_string)).unwrap_or_default();
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let v = verdict_at_key(&old.kind(), &old.id(), &old.relation(&key)?, &key, &lines, &header, &chart, &out.join("work"))?;
    let mut j = serde_json::to_value(&v).map_err(|e| e.to_string())?;
    j["old"] = old.describe();
    let mut f = std::fs::File::create(out.join("regularity.json")).map_err(|e| e.to_string())?;
    f.write_all(serde_json::to_string_pretty(&j).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    Ok((json!({"not_worse": v.not_worse, "counts": v.counts, "first_regressions": v.first_regressions, "old": j["old"]}).to_string(), v.not_worse))
}
