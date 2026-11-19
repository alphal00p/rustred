//! Lane wireflow (g), interfaces/repairflow.md 23, POLES_OBJECTIVE.md 2 (B) / 3.5: the REGULAR TERMINAL SET R as a
//! phase-D DIAGNOSTIC at P5 (policy `regbasis`; default off). When the closing loop's final verdict is `closed`, the
//! repair flow issues ONE job of kind `regular-basis` (`rr_regbasis`, legacy-wrapped, adapters/regbasis.rs) on the
//! node's effective import spec (`closeloop/<n>/import-spec.json`) and its final statement directory
//! (`final/statement/`: statement.jsonl, terminals.json, SHA256SUMS pinned). Row 37 (the export object sealed and bound
//! to the pinned statement): R + the exact map and the tower record are written NEXT TO THE STATEMENT
//! (`final/regbasis/regular_basis.json`, `tower.json`) and the record `repairflow:<n>:regbasis` says so; any other
//! end (rows 38 / 39, refusals, a node that is not closed) records why there is no R. Never changes a closure, a
//! statement or a verdict, never attention (a diagnostic never holds a node).

use super::ids::node_dir;
use crate::adapters::regbasis::{RegBasisJobConfig, job_request, records_of_settled};
use crate::ledger::{Event, ItemEvent};
use crate::state::{CampaignState, ItemPhase, Rung};
use crate::supervisor::worksource::{AcceptedRecord, JobRequest, Routing, Settled};
use rustred_campaign_contract::fake::FakeOp;
use rustred_campaign_core::repairflow::policy::RegBasisStep;
use serde_json::{Value, json};
use std::path::Path;

/// Item kind of the diagnostic job.
pub const KIND_REGBASIS: &str = "regular-basis";
pub const ROLE_REGBASIS: &str = "repairflow_regbasis";
pub const REGBASIS_SCHEMA: &str = "rustred.campaign.repairflow-regbasis.v1";

pub fn input_regbasis(n: &str) -> String {
    format!("repairflow:{n}:regbasis")
}

pub fn item_of(n: &str) -> String {
    format!("{n}.rf.rb")
}

fn rec(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

fn register(cdir: &Path, n: &str, v: &Value, rt: &mut Routing) {
    let r = rec(
        format!("{}/repairflow/regbasis.json", node_dir(n)),
        serde_json::to_vec_pretty(v).unwrap_or_default(),
    );
    let f = r.file_ref();
    rt.events.push(Event::InputRegistered {
        input: input_regbasis(n),
        role: ROLE_REGBASIS.into(),
        path: cdir.join(&r.path).display().to_string(),
        bytes: f.bytes,
        blake3: f.blake3,
    });
    rt.accepts.push(r);
}

/// procfix7 round 19 (landing, int8b-gate review (ii) wire_regbasis.rs:55): a final result that is not there is no
/// verdict; one that cannot be READ is an error (it was "no verdict": the P5 step of a closed node registered "skipped").
fn final_verdict(cdir: &Path, n: &str) -> Result<Option<String>, String> {
    let p = cdir.join(node_dir(n)).join("final/result.json");
    let b = match crate::fsx::read("closeloop.output", &p) {
        crate::fsx::Read::Content(b) => b,
        crate::fsx::Read::Absent => return Ok(None),
        crate::fsx::Read::Failed(e) => return Err(format!("{}: {e}", p.display())),
    };
    Ok(serde_json::from_slice::<Value>(&b)
        .ok()
        .and_then(|v| v["verdict"].as_str().map(String::from)))
}

/// P5 step of node `n` (true = done or off: the report may be written).
pub fn poll(
    st: &CampaignState,
    cdir: &Path,
    n: &str,
    node: &str,
    step: &RegBasisStep,
    rt: &mut Routing,
) -> Result<bool, String> {
    if !step.enabled || st.inputs.contains_key(&input_regbasis(n)) {
        return Ok(true);
    }
    let verdict = final_verdict(cdir, n)?;
    if verdict.as_deref() != Some("closed") {
        register(
            cdir,
            n,
            &json!({"schema": REGBASIS_SCHEMA, "node": node, "status": "skipped",
                    "why": format!("the closing loop's verdict is {verdict:?}, not closed")}),
            rt,
        );
        return Ok(false);
    }
    let item = item_of(n);
    match st.items.get(&item) {
        None => {
            rt.events.push(Event::ItemCreated {
                item,
                node: node.to_string(),
                kind: KIND_REGBASIS.into(),
                rung: Rung(0),
                parent: None,
                inputs: vec![],
                label: "regular basis (diagnostic)".into(),
                weight: 1,
            });
            Ok(false)
        }
        // the route registers the record at row 37; an item that ended otherwise records why there is no R
        Some(it)
            if !matches!(
                it.phase,
                ItemPhase::Open | ItemPhase::InProgress | ItemPhase::Attention(_)
            ) =>
        {
            register(
                cdir,
                n,
                &json!({"schema": REGBASIS_SCHEMA, "node": node, "status": "no_regular_basis",
                        "why": format!("the diagnostic job ended without a record ({:?})", it.phase)}),
                rt,
            );
            Ok(false)
        }
        Some(_) => Ok(false),
    }
}

/// The job of node `n`'s open diagnostic item (None = nothing to issue).
pub fn ready(
    st: &CampaignState,
    cdir: &Path,
    n: &str,
    step: &RegBasisStep,
    fake: Option<&[FakeOp]>,
) -> Option<Result<JobRequest, String>> {
    if !step.enabled || st.inputs.contains_key(&input_regbasis(n)) {
        return None;
    }
    let item = item_of(n);
    let it = st.items.get(&item)?;
    let job = match it.phase {
        ItemPhase::Open => format!("{item}-t{}", it.jobs.len()),
        ItemPhase::InProgress => match it.jobs.last().and_then(|l| st.jobs.get(l)).map(|r| r.state)
        {
            Some(crate::state::AttemptState::Queued) => it.jobs.last()?.clone(),
            _ => return None,
        },
        _ => return None,
    };
    let nd = cdir.join(node_dir(n));
    let cfg = RegBasisJobConfig {
        program: if fake.is_some() {
            "rr_fake_child".into()
        } else {
            step.program.clone()
        },
        args: step.args.clone(),
        cap_bytes: step.cap_bytes,
        expected_peak_bytes: step.expected_peak_bytes,
        heartbeat_s: step.heartbeat_s(),
        priority: 1_500_000_000,
        fake: fake.map(super::wire_composite::script),
    };
    Some(job_request(
        &item,
        &job,
        &nd.join("import-spec.json"),
        &nd.join("final/statement"),
        &cfg,
        vec![],
    ))
}

/// ROUTE: the settled diagnostic attempt.
pub fn route(
    st: &CampaignState,
    cdir: &Path,
    n: &str,
    node: &str,
    step: &RegBasisStep,
    done: &Settled,
) -> Result<Routing, String> {
    let mut rt = Routing::default();
    let cl = &done.classification;
    let end = |rt: &mut Routing, reason: String| {
        rt.events.push(Event::Item {
            item: done.item.clone(),
            ev: ItemEvent::Escalate {
                to: Rung(1),
                reason,
            },
        });
    };
    match records_of_settled(done) {
        Ok(Some((record, tower))) => {
            let dir = format!("{}/final/regbasis", node_dir(n));
            let rb = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
            let mut files = vec![json!({"file": "final/regbasis/regular_basis.json",
                "blake3": blake3::hash(&rb).to_hex().to_string()})];
            rt.accepts
                .push(rec(format!("{dir}/regular_basis.json"), rb));
            if let Some(t) = &tower {
                let tb = serde_json::to_vec_pretty(t).map_err(|e| e.to_string())?;
                files.push(json!({"file": "final/regbasis/tower.json",
                    "blake3": blake3::hash(&tb).to_hex().to_string()}));
                rt.accepts.push(rec(format!("{dir}/tower.json"), tb));
            }
            register(
                cdir,
                n,
                &json!({"schema": REGBASIS_SCHEMA, "node": node, "status": "regular_basis",
                        "job": done.job, "attempt": done.attempt, "row": cl.row,
                        "regular_set_size": record["regular_set"].as_array().map(Vec::len),
                        "closure_statement_digest": record["closure_statement_digest"],
                        "files": files}),
                &mut rt,
            );
            // a diagnostic accepts no record into the ledger's items: the item ends here
            end(
                &mut rt,
                format!(
                    "regular basis recorded next to the statement (row {})",
                    cl.row
                ),
            );
        }
        Ok(None) | Err(_) => {
            let attempts = st
                .items
                .get(&done.item)
                .map(|i| i.jobs.len() as u32)
                .unwrap_or(1);
            if !cl.attention && attempts < step.max_attempts.max(1) {
                rt.events.push(Event::Item {
                    item: done.item.clone(),
                    ev: ItemEvent::Requeue {
                        reason: format!("regular-basis row {}{}: {}", cl.row, cl.sub, cl.why),
                    },
                });
            } else {
                end(
                    &mut rt,
                    format!(
                        "no regular basis (row {}{}: {}): a diagnostic never holds the node",
                        cl.row, cl.sub, cl.why
                    ),
                );
            }
        }
    }
    Ok(rt)
}
