//! lane closeloop4 (interfaces/closeloop.md 19): TERMINALS BEHIND A DECISION. A key that nothing more can serve (the
//! decided pass is final, the key is uncovered: no rule, no relation, every candidate recipe answered) is a terminal
//! PROPOSAL, never a terminal by itself. Spec `terminals.accept`:
//! - `declared_only` (default): only the terminals the import spec (or an imported statement through it) declares are
//!   terminals; proposals wait for the operator: the node has no final verdict, the run stops with exit 8 ("open:
//!   awaiting operator decision on N terminal proposals"), and `rr_campaign control CDIR resolve-terminals --accept
//!   FILE | --reject` records the decision (a ledger input) and lets the run continue to its verdict;
//! - `auto` (explicit): at most `max_auto` (default 8) proposals are accepted automatically (recorded as such); more
//!   wait for the operator as above.
//!
//! A decision covers every proposal of its round: listed keys are accepted, the others rejected (a rejected key stays
//! open: blocker `terminal_proposals_rejected`). Accepted keys become a declared terminals source of the node's
//! effective import spec (`decided-terminals`, keys inline) from the next pass on. Every terminal of a result records
//! who accepted it (`spec` | `operator` | `auto`) and its pole diagnostics (null until that check exists).

use rustred_campaign_core::import::{CheckResult, ImportedSet, KeyRow, RowStatus};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const PROPOSALS_SCHEMA: &str = "rustred.campaign.closeloop-terminal-proposals.v1";
pub const DECISION_SCHEMA: &str = "rustred.campaign.closeloop-terminal-decision.v1";
/// The source id of the decided terminals in the effective import spec.
pub const DECIDED_SOURCE: &str = "decided-terminals";

/// One recorded decision on the proposals of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalDecision {
    pub schema: String,
    /// The control request id (operator) or `auto-r<k>`.
    pub id: String,
    pub node: String,
    /// The check round whose proposals it decides.
    pub round: u32,
    /// `operator` | `auto`.
    pub accepted_by: String,
    /// Who (the control request's `by`; `spec terminals.accept = auto` for automatic decisions).
    pub by: String,
    #[serde(default)]
    pub reason: String,
    pub accept: Vec<Vec<i32>>,
    pub reject: Vec<Vec<i32>>,
}

/// The proposals record of a round (written to `final/terminal_proposals.json` and registered as a ledger input).
pub fn proposals_record(node: &str, round: u32, keys: &[Vec<i32>], rows: &[KeyRow]) -> Value {
    let by_key: BTreeMap<&Vec<i32>, &KeyRow> = rows.iter().map(|r| (&r.key, r)).collect();
    json!({"schema": PROPOSALS_SCHEMA, "node": node, "round": round, "count": keys.len(),
        "label": format!("open: awaiting operator decision on {} terminal proposals", keys.len()),
        "resolve": "rr_campaign control CDIR resolve-terminals --node NODE --accept KEYS_FILE | --reject",
        "proposals": keys.iter().map(|k| {
            let r = by_key.get(k);
            json!({"key": k, "in_start": r.map(|r| r.in_start), "open_status": r.and_then(|r| r.open_status.clone()),
                   "open_reason": r.and_then(|r| r.open_reason.clone()), "pole_diagnostics": null})
        }).collect::<Vec<_>>()})
}

/// The automatic decision of a round (`terminals.accept = auto`, at most `max_auto` proposals).
pub fn auto_decision(
    node: &str,
    round: u32,
    keys: &[Vec<i32>],
    max_auto: usize,
) -> TerminalDecision {
    TerminalDecision {
        schema: DECISION_SCHEMA.into(),
        id: format!("auto-r{round}"),
        node: node.into(),
        round,
        accepted_by: "auto".into(),
        by: format!("spec terminals.accept = auto (max_auto {max_auto})"),
        reason: String::new(),
        accept: keys.to_vec(),
        reject: vec![],
    }
}

/// The operator's decision on the proposals `proposed` (a round's record): `accept` = Some(keys) (every listed key
/// must be a proposal; the others are rejected), None = reject all. Err = a key that was not proposed.
pub fn operator_decision(
    node: &str,
    round: u32,
    proposed: &[Vec<i32>],
    accept: Option<&[Vec<i32>]>,
    id: &str,
    by: &str,
    reason: &str,
) -> Result<TerminalDecision, String> {
    let acc: Vec<Vec<i32>> = accept.map(<[Vec<i32>]>::to_vec).unwrap_or_default();
    for k in &acc {
        if !proposed.contains(k) {
            return Err(format!(
                "key {k:?} is not a terminal proposal of node {node} (round {round})"
            ));
        }
    }
    Ok(TerminalDecision {
        schema: DECISION_SCHEMA.into(),
        id: id.into(),
        node: node.into(),
        round,
        accepted_by: "operator".into(),
        by: by.into(),
        reason: reason.into(),
        reject: proposed
            .iter()
            .filter(|k| !acc.contains(k))
            .cloned()
            .collect(),
        accept: acc,
    })
}

/// The effective import spec text: the base spec plus one declared terminals source with every accepted key (keys
/// inline; none = the base text unchanged).
pub fn effective_spec(base: &str, decisions: &[TerminalDecision]) -> Result<String, String> {
    let mut keys: Vec<Vec<i32>> = vec![];
    for d in decisions {
        for k in &d.accept {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
    }
    if keys.is_empty() {
        return Ok(base.to_string());
    }
    let mut v: Value = serde_json::from_str(base).map_err(|e| format!("import spec: {e}"))?;
    let srcs = v
        .get_mut("sources")
        .and_then(Value::as_array_mut)
        .ok_or("import spec: no sources")?;
    srcs.push(
        json!({"id": DECIDED_SOURCE, "kind": "terminals", "class": "terminal", "keys": keys}),
    );
    Ok(serde_json::to_string_pretty(&v).expect("json") + "\n")
}

/// Who accepted each terminal row of a check: `operator` / `auto` (a decision; the latest wins), `auto` for the keys an
/// escalation accepted under its rule (`escalated`), `spec` for pre-approved keys and every other declared terminal.
/// int6 (import5 x closeloop4): a terminal declared by a TABLE (`composite_terminal_ledger` keys; residual keys under
/// `table_terminals: ledger_and_residuals`) is a declared terminal (`spec`), named with its table provenance.
pub fn accepted_terminals(
    set: &ImportedSet,
    res: &CheckResult,
    decisions: &[TerminalDecision],
    escalated: &std::collections::BTreeSet<Vec<i32>>,
    approved: &[Vec<i32>],
) -> Vec<Value> {
    let mut by: BTreeMap<&Vec<i32>, &TerminalDecision> = BTreeMap::new();
    for d in decisions {
        for k in &d.accept {
            by.insert(k, d);
        }
    }
    res.rows
        .iter()
        .filter(|r| r.status == RowStatus::Terminal)
        .map(|r| match by.get(&r.key) {
            Some(d) => json!({"key": r.key, "accepted_by": d.accepted_by, "by": d.by, "decision": d.id,
                              "round": d.round, "pole_diagnostics": null}),
            None if approved.contains(&r.key) => json!({"key": r.key, "accepted_by": "spec",
                "by": "escalation.terminals.approved (pre-approved)", "decision": null, "pole_diagnostics": null}),
            None if escalated.contains(&r.key) => json!({"key": r.key, "accepted_by": "auto",
                "by": "escalation terminal rule (terminals.max_auto)", "decision": null, "pole_diagnostics": null}),
            None if set.terminal_provenance.contains_key(&r.key) => {
                let t = &set.terminal_provenance[&r.key];
                json!({"key": r.key, "accepted_by": "spec",
                       "by": format!("import spec: table {} ({}, line {})", t.source, t.provenance, t.line),
                       "provenance": t.provenance, "decision": null, "pole_diagnostics": null})
            }
            None => json!({"key": r.key, "accepted_by": "spec", "by": "import spec (declared terminals)",
                           "decision": null, "pole_diagnostics": null}),
        })
        .collect()
}

/// Parse a keys file: one key per line, a JSON array (`[1,2,0]`) or the `I(1,2,0)` text.
pub fn read_keys(text: &str) -> Result<Vec<Vec<i32>>, String> {
    let mut out = vec![];
    for (i, l) in text.lines().enumerate() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let k: Vec<i32> =
            if let Some(inner) = t.strip_prefix("I(").and_then(|x| x.strip_suffix(')')) {
                inner
                    .split(',')
                    .map(|x| x.trim().parse::<i32>())
                    .collect::<Result<_, _>>()
                    .map_err(|e| format!("line {}: {e}", i + 1))?
            } else {
                serde_json::from_str(t).map_err(|e| format!("line {}: {e}", i + 1))?
            };
        out.push(k);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_and_effective_spec() {
        let p = vec![vec![1, 1, 0], vec![2, 1, 0]];
        let d = operator_decision("n", 3, &p, Some(&[vec![2, 1, 0]]), "c-1", "me", "ok").unwrap();
        assert_eq!(d.accept, vec![vec![2, 1, 0]]);
        assert_eq!(d.reject, vec![vec![1, 1, 0]]);
        assert!(operator_decision("n", 3, &p, Some(&[vec![9, 9, 9]]), "c", "me", "").is_err());
        let r = operator_decision("n", 3, &p, None, "c-2", "me", "").unwrap();
        assert!(r.accept.is_empty() && r.reject.len() == 2);
        let base = r#"{"sources":[{"id":"A"}]}"#;
        assert_eq!(effective_spec(base, &[r]).unwrap(), base);
        let e: Value = serde_json::from_str(&effective_spec(base, &[d]).unwrap()).unwrap();
        assert_eq!(e["sources"][1]["id"], DECIDED_SOURCE);
        assert_eq!(e["sources"][1]["keys"], json!([[2, 1, 0]]));
        let a = auto_decision("n", 2, &p, 8);
        assert_eq!(a.accepted_by, "auto");
        assert_eq!(
            read_keys("[1,2,0]\nI(3, 1,0)\n# c\n").unwrap(),
            vec![vec![1, 2, 0], vec![3, 1, 0]]
        );
    }
}
