//! int6 (integration of lane import5 x lane closeloop4): what a node's TERMINALS are after both lanes, on the toy sunset
//! of lane import5 (tests/import5_common of the core crate; nothing five-loop specific).
//!
//! - import5 (interfaces/import.md 18.2): the keys of a table's `composite_terminal_ledger` are DECLARED terminals of
//!   the set by default (`table_terminals: ledger`); residual records only under `ledger_and_residuals`; `none` declares
//!   nothing from the table.
//! - closeloop4 (interfaces/closeloop.md 19, `terminals.accept = declared_only`, the default): only DECLARED terminals
//!   are terminals; an uncovered key nothing more can serve is a PROPOSAL waiting for the operator.
//!
//! Together: a ledger key is a terminal with no proposal and no decision (`accepted_by: spec`, named with its table
//! provenance); the same key with `table_terminals: none` (or a residual record under the default policy) is uncovered
//! and therefore a proposal; an operator decision makes it a declared terminal of the effective spec
//! (`decided-terminals`, `accepted_by: operator`).

#[path = "../../rustred-campaign-core/tests/import5_common/mod.rs"]
mod import5_common;
use import5_common::*;

use rustred_campaign::closeloop::terminals::{
    DECIDED_SOURCE, accepted_terminals, effective_spec, operator_decision,
};
use rustred_campaign_core::import::{CheckResult, KeyRow, RowStatus};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const TOP: [i32; 5] = [2, 2, 1, 0, 0];
const T1: [i32; 5] = [1, 1, 1, 0, 0];
const T2: [i32; 5] = [2, 1, 1, 0, 0];

fn ledger(keys: &[&[i32]]) -> String {
    json!({"type": "composite_terminal_ledger", "schema": "test.ledger.v1",
        "keys": keys.iter().map(|k| kt(k)).collect::<Vec<_>>(), "count": keys.len(),
        "label": "terminal ledger (test)"})
    .to_string()
}

/// The closing loop's proposal predicate (closeloop/pass.rs: an open row whose status is `uncovered`).
fn proposal(r: &KeyRow) -> bool {
    r.status == RowStatus::Open && r.open_status.as_deref() == Some("uncovered")
}

fn proposals(r: &CheckResult) -> Vec<Vec<i32>> {
    r.rows
        .iter()
        .filter(|x| proposal(x))
        .map(|x| x.key.clone())
        .collect()
}

fn accepted_of<'a>(acc: &'a [Value], k: &[i32]) -> &'a Value {
    acc.iter()
        .find(|v| v["key"] == json!(k))
        .unwrap_or_else(|| panic!("{k:?} not among the accepted terminals: {acc:?}"))
}

#[test]
fn ledger_keys_are_declared_terminals_under_declared_only_and_residuals_are_proposals() {
    let d = scratch("int6-table-terminals");
    // TOP -> T1 + T2; T1 in the ledger; T2 a residual record (declared only under `ledger_and_residuals`)
    let mut lines = vec![point_rule(0, &TOP, &[&T1, &T2])];
    lines.push(json!({"type": "residual", "chart": MASK, "key": kt(&T2)}).to_string());
    let tail = vec![ledger(&[&T1])];
    let t = write_table_with(&d, "t.jsonl", &header_value(), &lines, Some(&tail));
    let none = BTreeSet::new();

    // default `table_terminals: ledger`: T1 is a declared terminal (no proposal), T2 is uncovered -> a proposal
    let sp = spec(&d, &t, &[&TOP], json!({}), &[]);
    let (set, _ev, r) = run(&sp, &d);
    assert_eq!(row(&r, &T1).status, RowStatus::Terminal);
    assert!(!proposal(row(&r, &T1)));
    assert_eq!(proposals(&r), vec![T2.to_vec()]);
    let acc = accepted_terminals(&set, &r, &[], &none, &[]);
    let a1 = accepted_of(&acc, &T1);
    assert_eq!(a1["accepted_by"], "spec", "{a1}");
    assert_eq!(a1["provenance"], "composite_terminal_ledger", "{a1}");
    assert_eq!(a1["decision"], Value::Null);

    // the operator accepts the proposal T2: a declared terminal of the effective spec from the next pass on
    let dec = operator_decision(
        "toy-sunset",
        1,
        &[T2.to_vec()],
        Some(&[T2.to_vec()]),
        "c-1",
        "op",
        "test",
    )
    .unwrap();
    let eff: Value =
        serde_json::from_str(&effective_spec(&sp.to_string(), std::slice::from_ref(&dec)).unwrap())
            .unwrap();
    assert!(
        eff["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == DECIDED_SOURCE)
    );
    let (set2, _ev, r2) = run(&eff, &d);
    assert_eq!(row(&r2, &T1).status, RowStatus::Terminal);
    assert_eq!(row(&r2, &T2).status, RowStatus::Terminal);
    assert!(proposals(&r2).is_empty());
    let acc2 = accepted_terminals(&set2, &r2, std::slice::from_ref(&dec), &none, &[]);
    assert_eq!(accepted_of(&acc2, &T1)["accepted_by"], "spec");
    assert_eq!(accepted_of(&acc2, &T2)["accepted_by"], "operator");

    // `ledger_and_residuals`: both declared by the table, no proposal at all
    let sp = spec(
        &d,
        &t,
        &[&TOP],
        json!({"table_terminals": "ledger_and_residuals"}),
        &[],
    );
    let (set3, _ev, r3) = run(&sp, &d);
    assert!(proposals(&r3).is_empty());
    let acc3 = accepted_terminals(&set3, &r3, &[], &none, &[]);
    assert_eq!(accepted_of(&acc3, &T2)["provenance"], "residual");

    // `none`: nothing from the table: the ledger key is uncovered too -> both are proposals (declared_only waits)
    let sp = spec(&d, &t, &[&TOP], json!({"table_terminals": "none"}), &[]);
    let (_set4, _ev, r4) = run(&sp, &d);
    let mut p4 = proposals(&r4);
    p4.sort();
    assert_eq!(p4, vec![T1.to_vec(), T2.to_vec()]);
    let _ = std::fs::remove_dir_all(&d);
}
