//! Exit keys (hand-downs) and the coordinator's exit-dispatch answer (interfaces/coord-exit-dispatch.md; R36, plan
//! 2.4.16). The closure EMITS the exit key sets per child sector (dropped-line mask, key-set files with digests); this
//! module READS an exit-routing answer file and checks that every emitted child sector has an engine-routed provider.
//!
//! Format read: schema `*.exit-routing.v1` (example: the t162-exits lane's routing file): a header with
//! `<node>_presentation.slot_map_table_to_presentation` (table slot -> presentation slot) and `children[]`, each with
//! `presentation_line_dropped` (one-line exits today), `fate` ("routed" = engine-routed), `class`, `owner {lane,
//! dag_node, presentation}`, `route_chosen` (map; digested here). The field names of the presentation block vary with
//! the node; the reader takes the first header field ending in `_presentation` that carries the slot map.
//!
//! NOT YET AVAILABLE from the engine (marked in the check result, never passed silently): multi-line exits (the file
//! format has one dropped line per child), the provider-coverage result per mapped key (needs the engine's exit
//! dispatch with a Collect policy, EC-16), and a boxed provider's box (box-containment check). The check therefore
//! reports `routed` / `exit_undispatched` per child sector and `provider_coverage: not_available`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::sha256::sha256_tag;

/// One child (exit sector) of an exit-routing answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitChild {
    /// Dropped line slots in the TABLE frame (the closure's frame).
    pub dropped_table_slots: Vec<usize>,
    pub fate: String,
    pub class: Option<String>,
    pub lane: Option<String>,
    pub owner_dag_node: Option<i64>,
    pub owner_presentation: Option<i64>,
    /// sha256 of the canonical JSON of `route_chosen`.
    pub route_digest: Option<String>,
}

/// An exit-routing answer file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitRouting {
    pub schema: String,
    pub node: String,
    pub children: Vec<ExitChild>,
    pub file_digest: String,
}

/// Per child sector of the closure: the exit-dispatch status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitStatus {
    /// Dropped-line mask (table frame).
    pub dropped: u64,
    pub terms: u64,
    pub distinct_keys: u64,
    pub key_set_digest: Option<String>,
    /// `routed` | `exit_undispatched` | `not_routed:<fate>`
    pub status: String,
    pub class: Option<String>,
    pub lane: Option<String>,
    pub route_digest: Option<String>,
    /// Always `not_available` until the engine's exit dispatch reports coverage per mapped key (EC-16).
    pub provider_coverage: String,
}

impl ExitRouting {
    /// Parse an exit-routing answer (see the module documentation).
    pub fn parse(text: &str) -> Result<ExitRouting, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let schema = v["schema"].as_str().unwrap_or("").to_string();
        if !schema.ends_with(".exit-routing.v1") {
            return Err(format!("schema {schema:?} is not *.exit-routing.v1"));
        }
        let obj = v.as_object().ok_or("not an object")?;
        let slot_map: Vec<usize> = obj
            .iter()
            .filter(|(k, _)| k.ends_with("_presentation"))
            .find_map(|(_, p)| p["slot_map_table_to_presentation"].as_array().cloned())
            .ok_or("no *_presentation.slot_map_table_to_presentation")?
            .iter()
            .map(|x| x.as_u64().map(|u| u as usize).ok_or("bad slot map entry"))
            .collect::<Result<_, _>>()?;
        let mut children = vec![];
        for c in v["children"].as_array().ok_or("no children")? {
            let dropped_pres: Vec<usize> = match &c["presentation_line_dropped"] {
                serde_json::Value::Array(a) => a.iter().filter_map(|x| x.as_u64().map(|u| u as usize)).collect(),
                x => x.as_u64().map(|u| vec![u as usize]).ok_or("child without presentation_line_dropped")?,
            };
            let mut dropped_table_slots: Vec<usize> = dropped_pres
                .iter()
                .map(|&p| slot_map.iter().position(|&m| m == p).ok_or(format!("presentation slot {p} not mapped")))
                .collect::<Result<_, _>>()?;
            dropped_table_slots.sort_unstable();
            let route_digest = (!c["route_chosen"].is_null()).then(|| sha256_tag(c["route_chosen"].to_string().as_bytes()));
            children.push(ExitChild {
                dropped_table_slots,
                fate: c["fate"].as_str().unwrap_or("").to_string(),
                class: c["class"].as_str().map(str::to_string),
                lane: c["owner"]["lane"].as_str().map(str::to_string),
                owner_dag_node: c["owner"]["dag_node"].as_i64(),
                owner_presentation: c["owner"]["presentation"].as_i64(),
                route_digest,
            });
        }
        Ok(ExitRouting {
            schema,
            node: v["node"].as_str().unwrap_or("").to_string(),
            children,
            file_digest: sha256_tag(text.as_bytes()),
        })
    }

    /// The child record for a dropped-line mask, if any.
    pub fn child(&self, dropped: u64) -> Option<&ExitChild> {
        self.children
            .iter()
            .find(|c| c.dropped_table_slots.iter().fold(0u64, |a, &s| a | (1 << s)) == dropped)
    }
}

/// Exit-dispatch status of every child sector the closure handed down to.
pub fn check_exits(cl: &super::Closure, routing: Option<&ExitRouting>) -> Vec<ExitStatus> {
    let s = cl.summary();
    let mut out = vec![];
    let sets = cl.hand_down_sets();
    let mut masks: BTreeMap<u64, u64> = s.hand_downs.iter().map(|(&m, v)| (m, v[0])).collect();
    for &m in sets.keys() {
        masks.entry(m).or_default();
    }
    for (m, terms) in masks {
        let ks = sets.get(&m);
        let (status, class, lane, rd) = match routing.and_then(|r| r.child(m)) {
            None => ("exit_undispatched".to_string(), None, None, None),
            Some(c) if c.fate == "routed" => ("routed".to_string(), c.class.clone(), c.lane.clone(), c.route_digest.clone()),
            Some(c) => (format!("not_routed:{}", c.fate), c.class.clone(), c.lane.clone(), c.route_digest.clone()),
        };
        out.push(ExitStatus {
            dropped: m,
            terms,
            distinct_keys: ks.map(|k| k.len() as u64).unwrap_or(0),
            key_set_digest: ks.map(|k| k.digest()),
            status,
            class,
            lane,
            route_digest: rd,
            provider_coverage: "not_available".into(),
        });
    }
    out
}
