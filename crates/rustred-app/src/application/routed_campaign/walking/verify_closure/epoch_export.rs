//! Reader of the epoch engine's S2 final export (`epoch-export.json`,
//! walk semantics 3) for the closure oracle. Written independently of the
//! engine code: it reads the raw files (length and blake3 of every file
//! against the manifest), decodes the canonical domain images through the
//! shared compact encoding, expands the edge run log into pairs and decodes
//! ledger6 with its own reader. Its extra checks (`EpochChecks`) re-derive
//! the ledger6 rules from the raw sections: valid tags and per-tag counts,
//! the F8 seal rule on the saved flags, aliases forward in the same (phase,
//! owner) with exactly their one-edge run, one run per merged native with
//! strictly increasing targets below W, merge epochs <= k, the records and
//! edge digests recomputed from the runs (the out-degree check), and the
//! InitialDBand anchor rules. Everything else (records, F8 on records,
//! closure, F10) is the verifier's ordinary path.
use super::super::checkpoint::RawCheckpoint;
use super::super::queue::{CompactDomain, Domain, Phase};
use rustred::solver::DomainPowerBounds;
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) const MANIFEST: &str = "epoch-export.json";

/// One anchor record as the verifier reads it (layout v2): node, record
/// kind, dispatch version, `(anchor, lent scope code, stamp)` per anchor,
/// and the raw scope bytes.
pub(super) type AnchorRow = (u32, u8, u64, Vec<(u32, u8, u64)>, Vec<u8>);

/// The ledger6 view the verifier re-derives (independent decoder).
pub(super) struct EpochSections {
    pub ledger: Vec<u64>,
    pub runs: Vec<(u32, Vec<u32>)>,
    pub anchors: Vec<AnchorRow>,
    pub manifest: Value,
}

fn blake3_file(path: &Path) -> Result<(u64, String), String> {
    let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok((bytes, hasher.finalize().to_hex().to_string()))
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("length overflow")?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated epoch section")?;
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("2")))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8")))
    }
    fn header(&mut self, magic: &[u8; 8], arity: usize) -> Result<u64, String> {
        if self.take(8)? != magic {
            return Err(format!(
                "epoch section magic {:?}",
                String::from_utf8_lossy(magic)
            ));
        }
        if self.u32()? != 1 || self.u32()? as usize != arity {
            return Err("epoch section version or arity".into());
        }
        self.u64()
    }
    fn done(&self) -> Result<(), String> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err("trailing bytes in an epoch section".into())
        }
    }
}

fn read_image<const N: usize>(
    c: &mut Cursor<'_>,
    wire_arity: usize,
) -> Result<CompactDomain<N>, String> {
    let phase = match c.u8()? {
        0 => Phase::Apply,
        1 => Phase::Route,
        _ => return Err("epoch domain phase".into()),
    };
    let owner = c.u32()?;
    if N < 32 && owner >> N != 0 {
        return Err("epoch domain owner uses an omitted axis".into());
    }
    let rank = if c.u8()? == 1 {
        Some(c.u32()?)
    } else {
        c.u32()?;
        None
    };
    let lower = (0..wire_arity)
        .map(|_| c.u16().map(u64::from))
        .collect::<Result<Vec<_>, _>>()?;
    let upper = (0..wire_arity)
        .map(|_| c.u16().map(|v| (v != u16::MAX).then_some(u64::from(v))))
        .collect::<Result<Vec<_>, _>>()?;
    let option = |c: &mut Cursor<'_>| -> Result<Option<u64>, String> {
        let present = c.u8()?;
        let value = c.u64()?;
        Ok((present == 1).then_some(value))
    };
    let positive = option(c)?;
    let least = option(c)?.map(|v| v as i64);
    let most = option(c)?.map(|v| v as i64);
    let domain = Domain {
        phase,
        owner: std::array::from_fn(|axis| owner >> axis & 1 == 1),
        lower,
        upper,
        rank,
        powers: DomainPowerBounds {
            max_positive_power: positive,
            min_power_difference: least,
            max_power_difference: most,
        },
    };
    CompactDomain::restore(&domain)
}

fn usize_counter(value: &Value, key: &str) -> Result<usize, String> {
    value[key]
        .as_u64()
        .map(|v| v as usize)
        .ok_or_else(|| format!("epoch export counter {key} missing"))
}

/// The export as the verifier's raw checkpoint plus the epoch sections.
pub(super) fn read_raw<const N: usize>(
    directory: &Path,
) -> Result<(RawCheckpoint<N>, EpochSections), String> {
    let started = std::time::Instant::now();
    let text = fs::read(directory.join(MANIFEST)).map_err(|e| format!("{MANIFEST}: {e}"))?;
    let manifest: Value = serde_json::from_slice(&text).map_err(|e| format!("{MANIFEST}: {e}"))?;
    if manifest["format"] != "RUSTRED-EPOCH-EXPORT"
        || manifest["schema"] != 2
        || manifest["walk_semantics_version"] != 4
        || manifest["record_schema"] != 1
        || manifest["publication_policy"] != "epoch"
        || !manifest["arity"].as_u64().is_some_and(|arity| {
            crate::application::routed_campaign::storage::compatible_width(arity as usize, N)
        })
    {
        return Err("not an epoch export of this arity".into());
    }
    let wire_arity = manifest["arity"].as_u64().ok_or("epoch wire arity")? as usize;
    let read = |key: &str| -> Result<Vec<u8>, String> {
        let entry = &manifest["files"][key];
        let file = entry["file"]
            .as_str()
            .ok_or_else(|| format!("export file {key}"))?;
        let path = directory.join(file);
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if Some(bytes.len() as u64) != entry["meta"]["bytes"].as_u64()
            || blake3::hash(&bytes).to_hex().as_str()
                != entry["meta"]["blake3"].as_str().unwrap_or("")
        {
            return Err(format!("epoch export checksum or length mismatch: {file}"));
        }
        Ok(bytes)
    };
    let domain_bytes = read("domains")?;
    let mut c = Cursor {
        bytes: &domain_bytes,
        at: 0,
    };
    let count = c.header(b"EPDOMS01", wire_arity)? as usize;
    let mut domains = Vec::with_capacity(count);
    for _ in 0..count {
        domains.push(read_image::<N>(&mut c, wire_arity)?);
    }
    c.done()?;
    let node_bytes = read("nodes")?;
    let mut c = Cursor {
        bytes: &node_bytes,
        at: 0,
    };
    if c.header(b"EPNODE01", wire_arity)? as usize != count {
        return Err("epoch nodes count".into());
    }
    let flags = c.take(count)?.to_vec();
    c.done()?;
    let ledger_bytes = read("ledger6")?;
    let mut c = Cursor {
        bytes: &ledger_bytes,
        at: 0,
    };
    if c.header(b"EPLED601", wire_arity)? as usize != count {
        return Err("epoch ledger6 count".into());
    }
    let ledger = (0..count).map(|_| c.u64()).collect::<Result<Vec<_>, _>>()?;
    c.done()?;
    let edge_bytes = read("edges")?;
    let mut c = Cursor {
        bytes: &edge_bytes,
        at: 0,
    };
    let run_count = c.header(b"EPEDGE01", wire_arity)?;
    let mut runs = Vec::new();
    let mut edges = Vec::new();
    for _ in 0..run_count {
        let source = c.u32()?;
        let n = c.u32()? as usize;
        let targets = (0..n).map(|_| c.u32()).collect::<Result<Vec<_>, _>>()?;
        edges.extend(targets.iter().map(|&t| (source, t)));
        runs.push((source, targets));
    }
    c.done()?;
    let anchor_bytes = read("anchors")?;
    let mut c = Cursor {
        bytes: &anchor_bytes,
        at: 0,
    };
    // Layout v2 (the S2 note's amended §11.7): u32 counts, a lent scope
    // code per anchor, zero reserved bytes.
    if c.u16()? != 2 {
        return Err("epoch anchors version".into());
    }
    let mut anchors = Vec::new();
    for _ in 0..c.u64()? {
        let node = c.u32()?;
        let kind = c.u8()?;
        if c.u8()? != 0 || c.u16()? != 0 {
            return Err("epoch anchors reserved bytes".into());
        }
        let n = c.u32()? as usize;
        let scope_len = c.u32()? as usize;
        let dispatch = c.u64()?;
        let mut list = Vec::new();
        for _ in 0..n {
            let anchor = c.u32()?;
            let lent = c.u8()?;
            if c.take(3)? != [0, 0, 0] {
                return Err("epoch anchors padding".into());
            }
            list.push((anchor, lent, c.u64()?));
        }
        let scope = c.take(scope_len)?.to_vec();
        anchors.push((node, kind, dispatch, list, scope));
    }
    c.done()?;
    let mut records = Vec::new();
    for segment in manifest["records"]
        .as_array()
        .ok_or("epoch export records")?
    {
        let file = segment["file"].as_str().ok_or("epoch record file")?;
        let path: PathBuf = directory.join(file);
        let (bytes, digest) = blake3_file(&path)?;
        if Some(bytes) != segment["bytes"].as_u64()
            || Some(digest.as_str()) != segment["blake3"].as_str()
        {
            return Err(format!("epoch export checksum or length mismatch: {file}"));
        }
        records.push((
            path,
            segment["count"].as_u64().ok_or("epoch record count")? as usize,
        ));
    }
    let counters = &manifest["counters"];
    let raw = RawCheckpoint {
        generation: manifest["generation"].as_u64().ok_or("epoch generation")?,
        request: manifest["request"]
            .as_str()
            .ok_or("epoch request")?
            .to_owned(),
        publication_policy: "epoch".into(),
        walk_semantics_version: manifest["walk_semantics_version"]
            .as_u64()
            .ok_or("epoch semantics")? as u32,
        executable: manifest["executable"].as_str().unwrap_or("").to_owned(),
        owners: serde_json::from_value(manifest["owners"].clone()).map_err(|e| e.to_string())?,
        counters: [
            usize_counter(counters, "events")?,
            usize_counter(counters, "successors")?,
            usize_counter(counters, "conditional")?,
            usize_counter(counters, "job_local_reuse_hits")?,
            usize_counter(counters, "pre_admitted_orthant_hits")?,
            usize_counter(counters, "frontiers")?,
            usize_counter(counters, "completed")?,
            usize_counter(counters, "natives")?,
            usize_counter(counters, "routed")?,
            usize_counter(counters, "route_masks")?,
            usize_counter(counters, "initial")?,
            usize_counter(counters, "initial_inspected")?,
        ],
        closure: manifest["closure"].clone(),
        inputs: serde_json::from_value(manifest["inputs"].clone()).map_err(|e| e.to_string())?,
        input_frontiers: serde_json::from_value(manifest["input_frontiers"].clone())
            .map_err(|e| e.to_string())?,
        uncommitted: Vec::new(),
        flags,
        edges,
        domains,
        records,
        verify_seconds: started.elapsed().as_secs_f64(),
        // The S2 export cannot contain amended or activated state: epoch
        // admission rejects those combinations before loading any input.
        amendments: Vec::new(),
        pending_frontiers: Vec::new(),
        g2_activation: None,
    };
    Ok((
        raw,
        EpochSections {
            ledger,
            runs,
            anchors,
            manifest,
        },
    ))
}

const TAG_SHIFT: u32 = 61;

/// ledger6 decoded by the verifier: (tag, payload).
fn decode(word: u64) -> (u64, u64) {
    (word >> TAG_SHIFT, word & ((1 << TAG_SHIFT) - 1))
}

fn bucket<const N: usize>(domain: &CompactDomain<N>) -> (Phase, [bool; N]) {
    (domain.phase(), domain.owner())
}

/// The epoch-specific re-derivations (see the module doc). `records` gives,
/// per ID, the verifier's record reading: (is native, frontiers, error,
/// alias representative).
pub(super) fn check<const N: usize>(
    sections: &EpochSections,
    domains: &[CompactDomain<N>],
    flags: &[u8],
    record_of: &dyn Fn(usize) -> Option<(bool, u32, bool, Option<usize>, bool)>,
    add: &mut dyn FnMut(&'static str, String),
) {
    let total = domains.len();
    let manifest = &sections.manifest;
    let k = manifest["k"].as_u64().unwrap_or(0);
    let p0 = manifest["p0"].as_u64().unwrap_or(0) as usize;
    if p0 > total || sections.ledger.len() != total {
        add(
            "epoch_ledger",
            format!(
                "P0 {p0} or {} ledger6 entries against {total} domains",
                sections.ledger.len()
            ),
        );
        return;
    }
    let mut counts = [0u64; 8];
    let mut alias_to = vec![None; total];
    for (id, &word) in sections.ledger.iter().enumerate() {
        let (tag, payload) = decode(word);
        if tag > 7 {
            add(
                "epoch_ledger",
                format!("id {id}: invalid ledger6 tag {tag}"),
            );
            continue;
        }
        counts[tag as usize] += 1;
        let sealed = flags.get(id).is_some_and(|f| f & 1 != 0);
        if sealed != (tag == 2 || tag == 5) {
            add(
                "epoch_seal_rule",
                format!("id {id}: ledger6 tag {tag} vs seal flag {sealed}"),
            );
        }
        if matches!(tag, 2..=4 | 7) && (payload & ((1 << 48) - 1)) > k {
            add("epoch_ledger", format!("id {id}: merge epoch beyond k {k}"));
        }
        let record = record_of(id);
        match tag {
            2..=4 => match record {
                Some((true, frontiers, error, _, false)) => {
                    let expected = if error {
                        4
                    } else if frontiers > 0 {
                        3
                    } else {
                        2
                    };
                    if expected != tag {
                        add(
                            "epoch_ledger",
                            format!("id {id}: ledger6 tag {tag} but the record implies {expected}"),
                        );
                    }
                }
                _ => add(
                    "epoch_ledger",
                    format!("id {id}: merged native without a native record"),
                ),
            },
            5 => {
                let to = payload as usize;
                alias_to[id] = Some(to);
                if to <= id
                    || to >= total
                    || bucket(&domains[to]) != bucket(&domains[id])
                    || id < p0
                {
                    add(
                        "epoch_alias",
                        format!("alias {id} -> {to}: not forward in the same bucket"),
                    );
                }
                if record.and_then(|r| r.3) != Some(to) {
                    add(
                        "epoch_alias",
                        format!("alias {id}: record representative differs from ledger6"),
                    );
                }
            }
            7 => {
                if record != Some((true, 1, false, None, true))
                    || flags[id] & 7 != 0
                    || payload == 0
                    || payload >> 48 != 0
                {
                    add(
                        "epoch_abandoned",
                        format!("id {id}: abandonment record, flags or epoch differs"),
                    );
                }
            }
            _ => {
                if record.is_some() {
                    add(
                        "epoch_ledger",
                        format!("id {id}: unmerged ledger6 tag {tag} but a record exists"),
                    );
                }
            }
        }
    }
    let names = [
        "pending",
        "reserved",
        "native",
        "native_frontier",
        "native_error",
        "alias",
        "exhausted",
        "abandoned",
    ];
    for (index, name) in names.iter().enumerate() {
        if manifest["ledger6_counts"][name].as_u64() != Some(counts[index]) {
            add(
                "epoch_ledger",
                format!("ledger6 count {name} differs from the manifest"),
            );
        }
    }
    // Runs: one per merged native, one of length 1 per alias (its target),
    // strictly increasing targets below W; digests recomputed.
    let mut seen = vec![false; total];
    let mut edge_digest = blake3::Hasher::new();
    let mut records_digest = blake3::Hasher::new();
    for (source, targets) in &sections.runs {
        let source = *source as usize;
        if source >= total || std::mem::replace(&mut seen[source], true) {
            add(
                "epoch_runs",
                format!("run source {source} out of range or repeated"),
            );
            continue;
        }
        if targets.windows(2).any(|w| w[0] >= w[1]) || targets.iter().any(|&t| t as usize >= total)
        {
            add(
                "epoch_runs",
                format!("run of {source}: targets not strictly increasing below W"),
            );
        }
        edge_digest.update(&(source as u32).to_le_bytes());
        edge_digest.update(&(targets.len() as u32).to_le_bytes());
        for target in targets {
            edge_digest.update(&target.to_le_bytes());
        }
        let (tag, _) = decode(sections.ledger[source]);
        match tag {
            5 => {
                if alias_to[source] != Some(targets.first().copied().unwrap_or(u32::MAX) as usize)
                    || targets.len() != 1
                {
                    add(
                        "epoch_runs",
                        format!("alias {source}: run is not exactly its representative"),
                    );
                }
            }
            2..=4 | 7 => {
                if tag == 7 && !targets.is_empty() {
                    add(
                        "epoch_abandoned",
                        format!("abandoned {source} has dependencies"),
                    );
                }
                records_digest.update(&(source as u32).to_le_bytes());
                records_digest.update(&[tag as u8]);
                records_digest.update(&(targets.len() as u32).to_le_bytes());
            }
            _ => add(
                "epoch_runs",
                format!("run from unmerged id {source} (tag {tag})"),
            ),
        }
    }
    for (id, &word) in sections.ledger.iter().enumerate() {
        if matches!(decode(word).0, 2..=5 | 7) && !seen[id] {
            add("epoch_runs", format!("merged id {id} has no edge run"));
        }
    }
    if manifest["edge_digest"].as_str() != Some(edge_digest.finalize().to_hex().as_str()) {
        add("epoch_digest", "edge digest differs from the runs".into());
    }
    if manifest["records_digest"].as_str() != Some(records_digest.finalize().to_hex().as_str()) {
        add(
            "epoch_digest",
            "records digest (id, tag, out-degree) differs from the runs".into(),
        );
    }
    // InitialDBand anchors: exactly one anchor, anchor < P0 <= node, same
    // bucket, no stamp, it lends its full domain (code 0), the anchor edge
    // present, the node's ledger6 native carries the D-band flag and not the
    // G2' residual flag. G2' kinds are refused (S2 produces none; the G2'
    // verifier rules are W4).
    let run_of: std::collections::HashMap<usize, &Vec<u32>> = sections
        .runs
        .iter()
        .map(|(s, t)| (*s as usize, t))
        .collect();
    let anchor_of: std::collections::BTreeMap<u32, &AnchorRow> =
        sections.anchors.iter().map(|r| (r.0, r)).collect();
    for (id, &word) in sections.ledger.iter().enumerate() {
        let (tag, payload) = decode(word);
        let anchor = anchor_of.get(&(id as u32));
        let residual = anchor.is_some_and(|a| a.1 != 0);
        if flags
            .get(id)
            .is_none_or(|f| (f & 8 != 0) != anchor.is_some() || (f & 16 != 0) != residual)
            || tag == 2
                && ((payload & (1 << 48) != 0) != residual
                    || (payload & (1 << 49) != 0) != anchor.is_some_and(|a| a.1 == 0))
        {
            add(
                "epoch_anchor",
                format!("node {id}: raw anchor, node and ledger flags differ"),
            );
        }
    }
    for (node, kind, dispatch, list, scope) in &sections.anchors {
        let node = *node as usize;
        if node >= total || node < p0 {
            add(
                "epoch_anchor",
                format!("anchor record on node {node} outside [P0, W)"),
            );
            continue;
        }
        if *kind != 0 {
            let (tag, payload) = decode(sections.ledger[node]);
            let epoch = payload & ((1 << 48) - 1);
            if !matches!(kind, 1 | 2)
                || !matches!(tag, 2..=4)
                || *dispatch >= epoch
                || domains[node].phase() != Phase::Apply
                || list.is_empty()
            {
                add(
                    "epoch_anchor",
                    format!("node {node}: G2 source kind, scope or dispatch epoch"),
                );
            }
            let mut distinct = std::collections::BTreeSet::new();
            for &(anchor, lent, stamp) in list {
                let a = anchor as usize;
                if a >= total {
                    add(
                        "epoch_anchor",
                        format!("node {node}: G2 anchor {a} outside arena"),
                    );
                    continue;
                }
                let (tag, payload) = decode(sections.ledger[a]);
                let prior = anchor_of.get(&anchor);
                let expected_lent = u8::from(prior.is_some_and(|r| r.1 == 0));
                let empty_residual =
                    prior.is_some_and(|r| r.1 != 0 && r.4.get(..4) == Some(&[0, 0, 0, 0]));
                if !distinct.insert(anchor)
                    || tag != 2
                    || stamp != payload & ((1 << 48) - 1)
                    || stamp > *dispatch
                    || stamp == 0
                    || lent != expected_lent
                    || empty_residual
                    || *kind == 1 && prior.is_some()
                    || bucket(&domains[a]) != bucket(&domains[node])
                    || !run_of
                        .get(&node)
                        .is_some_and(|t| t.binary_search(&anchor).is_ok())
                {
                    add(
                        "epoch_anchor",
                        format!(
                            "node {node}: G2 lender {a} violates scope, eligibility, edge or prior-cut stamp"
                        ),
                    );
                }
            }
            continue;
        }
        if list.len() != 1 || scope.len() != 8 {
            add(
                "epoch_anchor",
                format!("node {node}: InitialDBand record needs one anchor and an 8-byte cut"),
            );
        }
        for &(anchor, lent, stamp) in list {
            let anchor = anchor as usize;
            let ok = anchor < p0
                && lent == 0
                && stamp == u64::MAX
                && !anchor_of.contains_key(&(anchor as u32))
                && bucket(&domains[anchor]) == bucket(&domains[node])
                && run_of
                    .get(&node)
                    .is_some_and(|t| t.binary_search(&(anchor as u32)).is_ok());
            if !ok {
                add(
                    "epoch_anchor",
                    format!("node {node}: InitialDBand anchor {anchor} violates R1/R2"),
                );
            }
        }
        let (tag, payload) = decode(sections.ledger[node]);
        if matches!(tag, 2) && (payload & (1 << 49) == 0 || payload & (1 << 48) != 0) {
            add(
                "epoch_anchor",
                format!(
                    "node {node}: anchored native without the D-band flag (or with the G2' flag)"
                ),
            );
        }
    }
}
