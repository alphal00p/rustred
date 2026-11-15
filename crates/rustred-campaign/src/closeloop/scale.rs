//! Lane t266run (interfaces/closeloop.md 17): big-node helpers of the pass. The keys-only pass writes its rows to a
//! file (`check_streamed`); the facts the exact tier needs are read back from that file one row at a time (bounded
//! memory: only the missing (record, key) pairs are kept, never the rows).

use rustred_campaign_core::import::{Evidence, ImportedSet, WitnessKind};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;

/// Every served rule row of the keys-only rows file whose winning record has no usable regularity fact (memory or
/// store) and was not asked before goes into `missing` ((table, record) -> keys); keys of records with ELIDED witness
/// loci are counted in `elided` instead (rr_proof instantiate refuses those records as a whole).
pub fn missing_facts_streamed(
    set: &ImportedSet,
    ev: &Evidence,
    rows: &Path,
    issued: &BTreeSet<(String, Vec<i32>)>,
    missing: &mut BTreeMap<(usize, usize), BTreeSet<Vec<i32>>>,
    elided: &mut u64,
) -> Result<(), String> {
    let f = std::fs::File::open(rows).map_err(|e| format!("{}: {e}", rows.display()))?;
    let rd = std::io::BufReader::with_capacity(1 << 22, f);
    for (i, l) in rd.lines().enumerate() {
        let l = l.map_err(|e| format!("{}: {e}", rows.display()))?;
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l)
            .map_err(|e| format!("{} line {}: {e}", rows.display(), i + 1))?;
        if v["status"] != "served" || v["server"]["kind"] != "rule" {
            continue;
        }
        let Some(sha) = v["server"]["record_sha256"].as_str() else {
            continue;
        };
        let Some((t, r)) = set.find_sha(sha) else {
            return Err(format!(
                "{} line {}: served by a record the set does not hold ({sha})",
                rows.display(),
                i + 1
            ));
        };
        let key: Vec<i32> = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("{} line {}: key: {e}", rows.display(), i + 1))?;
        if set.rules[t][r].witness == WitnessKind::Elided {
            *elided += 1;
            continue;
        }
        if !ev.has_lift(set, t, r, &key) && !issued.contains(&(sha.to_string(), key.clone())) {
            missing.entry((t, r)).or_default().insert(key);
        }
    }
    Ok(())
}
