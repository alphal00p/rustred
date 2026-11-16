//! Lane wireflow (c), interfaces/repairflow.md 22, TRICKS 2: the composite rounds' REGION FITS SHIPPED AS CASE DOMAINS at
//! P5 (policy `ship_regions`; default off). Every composite round carries lane regions' fit (the tool's sealed
//! `regions.json`: per face, regions = (condition of <= 2 atoms, one choice = element / image rule) fitted on the
//! per-key choices) and the keys each face's fit was made on (`regions.keys.json`, from the sealed `choices.jsonl`).
//! A face's regions are SHIPPED (export `regions/`) iff
//! - the fit is EXACT: no residual key, no unservable key, and every one of the face's keys satisfies one region's
//!   condition (re-evaluated here atom by atom);
//! - EVERY key of the face is certified in the final statement (status served);
//! - the closing loop's SHIP GATE passed (`final/ship-gate.json`, `passed: true`; `require_gate: false` = tests only);
//! - the face record is the one the round's routed record pins (regionfit digest; the routed input is BLAKE3-pinned).
//!
//! A region is a generalisation, never a proof: each shipped case domain says `verify per key at use` (whoever serves a
//! key with it verifies that key's relation like any other). Never changes a closure, a statement or a verdict.
//! Ledger: input `repairflow:<n>:regions` (role repairflow_regions) = the record (every face: shipped or why not).

use super::ids::node_dir;
use crate::ledger::Event;
use crate::state::CampaignState;
use crate::supervisor::worksource::{AcceptedRecord, Routing};
use rustred_campaign_core::regionfit::atoms::Atom;
use rustred_campaign_core::repairflow::policy::ShipRegionsStep;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const ROLE_REGIONS: &str = "repairflow_regions";
pub const REGIONS_SCHEMA: &str = "rustred.campaign.repairflow-regions-ship.v1";
pub const CASE_DOMAINS_SCHEMA: &str = "rustred.campaign.case-domains.v1";

pub fn input_regions(n: &str) -> String {
    format!("repairflow:{n}:regions")
}

/// The ship decision of one face's fit (pure): `Ok(case domains)` or `Err(why not)`.
pub fn ship_face(
    fit: &Value,
    keys: &[Vec<i32>],
    certified: &BTreeSet<Vec<i32>>,
    gate: Option<bool>,
    require_gate: bool,
) -> Result<Vec<Value>, String> {
    if fit.get("error").is_some() {
        return Err(format!("fit error: {}", fit["error"]));
    }
    let residual = fit["residual"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(usize::MAX);
    let unservable = fit["unservable"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(usize::MAX);
    if residual != 0 || unservable != 0 {
        return Err(format!(
            "not exact: residual {residual}, unservable {unservable}"
        ));
    }
    if fit["keys"].as_u64() != Some(keys.len() as u64) || keys.is_empty() {
        return Err(format!(
            "the fit's key count {} differs from the face's {} keys",
            fit["keys"],
            keys.len()
        ));
    }
    let mut regions: Vec<(Vec<Atom>, &Value)> = vec![];
    for r in fit["regions"].as_array().into_iter().flatten() {
        let atoms: Option<Vec<Atom>> = r["condition"]["atoms"]
            .as_array()
            .map(|a| a.iter().map(Atom::from_json).collect())
            .unwrap_or(Some(vec![]));
        let atoms = atoms.ok_or_else(|| format!("unreadable condition {}", r["condition"]))?;
        regions.push((atoms, r));
    }
    let uncovered = keys
        .iter()
        .filter(|k| !regions.iter().any(|(a, _)| a.iter().all(|x| x.eval(k))))
        .count();
    if uncovered > 0 {
        return Err(format!("{uncovered} face keys satisfy no region condition"));
    }
    let not_cert = keys.iter().filter(|k| !certified.contains(*k)).count();
    if not_cert > 0 {
        return Err(format!("{not_cert} of the face's keys are not certified"));
    }
    match (require_gate, gate) {
        (false, _) | (true, Some(true)) => {}
        (true, Some(false)) => return Err("the ship gate did not pass".into()),
        (true, None) => return Err("the ship gate did not run (no Tier C candidate close)".into()),
    }
    Ok(regions
        .iter()
        .map(|(_, r)| {
            json!({"condition": r["condition"], "choice": r["choice"], "keys": r["keys"],
                   "image_rules": r["image_rules"], "use": "verify per key at use (a generalisation, never a proof)"})
        })
        .collect())
}

fn read_json(p: &Path) -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?)
        .map_err(|e| format!("{}: {e}", p.display()))
}

fn rec(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

/// P5 step of node `n` (true = done or off: the report may be written; false = the record was just registered).
pub fn poll(
    st: &CampaignState,
    cdir: &Path,
    n: &str,
    step: &ShipRegionsStep,
    rt: &mut Routing,
) -> Result<bool, String> {
    if !step.enabled || st.inputs.contains_key(&input_regions(n)) {
        return Ok(true);
    }
    let nd = cdir.join(node_dir(n));
    // the gate (absent = did not run) and the certified keys of the final statement
    let gate = read_json(&nd.join("final/ship-gate.json"))
        .ok()
        .and_then(|g| g["passed"].as_bool());
    let mut certified: BTreeSet<Vec<i32>> = BTreeSet::new();
    let stmt = nd.join("final/statement/statement.jsonl");
    for l in std::fs::read_to_string(&stmt)
        .map_err(|e| format!("{}: {e}", stmt.display()))?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        if let Ok(v) = serde_json::from_str::<Value>(l)
            && v["status"] == "served"
            && let Ok(k) = serde_json::from_value::<Vec<i32>>(v["key"].clone())
        {
            certified.insert(k);
        }
    }
    let mut faces_v: Vec<Value> = vec![];
    let mut files: Vec<(String, Vec<u8>)> = vec![];
    let rounds: BTreeSet<u32> = st
        .inputs
        .keys()
        .filter_map(|id| {
            id.strip_prefix(&format!("repairflow:{n}:composite:routed:r"))
                .and_then(|k| k.parse().ok())
        })
        .collect();
    for k in rounds {
        let routed: Value = serde_json::from_slice(&super::wire_composite::read_input(
            st,
            &super::wire_composite::input_routed(n, k),
        )?)
        .map_err(|e| format!("routed r{k}: {e}"))?;
        let rd = nd.join(format!("repairflow/composite/r{k}"));
        let (Ok(fits), Ok(keys)) = (
            read_json(&rd.join("regions.json")),
            read_json(&rd.join("regions.keys.json")),
        ) else {
            continue;
        };
        let fits = fits.get("faces").cloned().unwrap_or(Value::Null);
        for (face, fit) in fits.as_object().into_iter().flatten() {
            let pinned = routed["regions"][face]["digest"].clone();
            let ks: Vec<Vec<i32>> =
                serde_json::from_value(keys["faces"][face].clone()).unwrap_or_default();
            let d = if fit["digest"] != pinned {
                Err("the face record is not the one the round pins".to_string())
            } else {
                ship_face(fit, &ks, &certified, gate, step.require_gate)
            };
            let mut line = json!({"round": k, "face": face, "keys": ks.len(), "fit_digest": fit["digest"],
                "regions": fit["regions"].as_array().map(Vec::len)});
            match d {
                Ok(cases) => {
                    let name = format!("r{k}.{}.json", super::ids::safe(face));
                    let doc = json!({"schema": CASE_DOMAINS_SCHEMA, "node": n, "round": k, "face": face,
                        "source": format!("rf:symmetry-composite:r{k}"), "keys": ks.len(),
                        "key_list_digest": fit["key_list_digest"], "fit_digest": fit["digest"],
                        "policy_digest": fit["policy_digest"], "method": fit["fit"]["method"],
                        "case_domains": cases,
                        "verified_on": format!("exact on the face's {} certified keys; per key at use", ks.len()),
                        "gate": gate});
                    let b = serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())?;
                    line["shipped"] = json!(true);
                    line["file"] = json!(format!("regions/{name}"));
                    files.push((format!("{}/repairflow/regions/{name}", node_dir(n)), b));
                }
                Err(why) => {
                    line["shipped"] = json!(false);
                    line["why"] = json!(why);
                }
            }
            faces_v.push(line);
        }
    }
    let shipped = faces_v.iter().filter(|f| f["shipped"] == true).count();
    let record = json!({"schema": REGIONS_SCHEMA, "node": n, "require_gate": step.require_gate, "gate": gate,
        "faces": faces_v, "shipped": shipped});
    for (p, b) in files {
        rt.accepts.push(rec(p, b));
    }
    let r = rec(
        format!("{}/repairflow/regions/regions.json", node_dir(n)),
        serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
    );
    let f = r.file_ref();
    rt.events.push(Event::InputRegistered {
        input: input_regions(n),
        role: ROLE_REGIONS.into(),
        path: cdir.join(&r.path).display().to_string(),
        bytes: f.bytes,
        blake3: f.blake3,
    });
    rt.accepts.push(r);
    Ok(false)
}

/// The export lines of the shipped faces (report): (manifest summary, files (export-relative name, bytes)).
pub fn export(
    st: &CampaignState,
    cdir: &Path,
    n: &str,
) -> Result<Option<(Value, Vec<(String, Vec<u8>)>)>, String> {
    if !st.inputs.contains_key(&input_regions(n)) {
        return Ok(None);
    }
    let v: Value =
        serde_json::from_slice(&super::wire_composite::read_input(st, &input_regions(n))?)
            .map_err(|e| format!("regions record: {e}"))?;
    let mut files = vec![];
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for f in v["faces"].as_array().into_iter().flatten() {
        if let Some(rel) = f["file"].as_str() {
            let p = cdir.join(node_dir(n)).join("repairflow").join(rel);
            let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            files.push((rel.to_string(), b));
            *by.entry("shipped".into()).or_default() += 1;
        } else {
            *by.entry("not_shipped".into()).or_default() += 1;
        }
    }
    Ok(Some((json!({"faces": by, "record": v}), files)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fit(residual: usize) -> Value {
        json!({"keys": 3, "residual": vec![json!({}); residual], "unservable": [],
               "regions": [{"condition": {"atoms": [{"kind": "ge_const", "i": 1, "c": 2}], "text": "n1>=2"},
                            "choice": "e1", "keys": 3}]})
    }

    #[test]
    fn exact_certified_and_gated_fits_ship_the_rest_says_why_not() {
        let keys = vec![vec![1, 2, 0], vec![1, 3, 0], vec![1, 4, 0]];
        let cert: BTreeSet<Vec<i32>> = keys.iter().cloned().collect();
        let ok = ship_face(&fit(0), &keys, &cert, Some(true), true).unwrap();
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0]["choice"], "e1");
        assert!(ok[0]["use"].as_str().unwrap().contains("per key at use"));
        assert!(
            ship_face(&fit(1), &keys, &cert, Some(true), true)
                .unwrap_err()
                .contains("not exact")
        );
        assert!(
            ship_face(&fit(0), &keys, &cert, Some(false), true)
                .unwrap_err()
                .contains("did not pass")
        );
        assert!(
            ship_face(&fit(0), &keys, &cert, None, true)
                .unwrap_err()
                .contains("did not run")
        );
        assert!(ship_face(&fit(0), &keys, &cert, None, false).is_ok());
        let mut partial = cert.clone();
        partial.remove(&vec![1, 4, 0]);
        assert!(
            ship_face(&fit(0), &keys, &partial, Some(true), true)
                .unwrap_err()
                .contains("not certified")
        );
        // a key outside every condition: not exact on the face's keys
        let keys2 = vec![vec![1, 2, 0], vec![1, 3, 0], vec![1, 1, 0]];
        let cert2: BTreeSet<Vec<i32>> = keys2.iter().cloned().collect();
        assert!(
            ship_face(&fit(0), &keys2, &cert2, Some(true), true)
                .unwrap_err()
                .contains("no region")
        );
        assert!(
            ship_face(&fit(0), &keys[..2], &cert, Some(true), true)
                .unwrap_err()
                .contains("key count")
        );
    }

    #[test]
    fn atoms_round_trip_through_their_json() {
        for a in [
            Atom::LeRel { i: 0, j: 2 },
            Atom::LtRel { i: 1, j: 3 },
            Atom::EqRel { i: 0, j: 1 },
            Atom::EqConst { i: 2, c: -1 },
            Atom::LeConst { i: 4, c: 3 },
            Atom::GeConst { i: 1, c: 2 },
            Atom::Parity { i: 0, odd: true },
            Atom::SumLe { i: 0, j: 1, c: 5 },
        ] {
            assert_eq!(Atom::from_json(&a.json()), Some(a));
        }
        assert_eq!(Atom::from_json(&json!({"kind": "xor", "i": 0})), None);
    }
}
