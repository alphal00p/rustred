//! Lane import5 (interfaces/import.md 18.1 / 18.2): ONE closed list of line kinds (`record::TABLE_LINE_KINDS`) for the
//! import and the assembly; the `sym` record parser; the assembly's handling of `composite_terminal_ledger` (input
//! terminals, regenerated as residual records) and of the composite closing markers.

mod import5_common;
use import5_common::*;

use std::collections::BTreeSet;

use rustred_campaign_core::accept::NodeTarget;
use rustred_campaign_core::assemble::{
    AssemblyInput, COPIED_ONCE, END_LIKE, FullRetirements, InputTable, NOT_COPIED, assemble,
};
use rustred_campaign_core::record::{TABLE_LINE_KINDS, parse_sym_line};
use serde_json::json;

#[test]
fn one_closed_list_of_line_kinds() {
    let all: BTreeSet<&str> = TABLE_LINE_KINDS.iter().copied().collect();
    assert_eq!(all.len(), TABLE_LINE_KINDS.len());
    let mut handled: BTreeSet<&str> = ["header", "rule", "sym", "chart_error"]
        .into_iter()
        .collect();
    handled.extend(COPIED_ONCE);
    handled.extend(NOT_COPIED);
    handled.extend(END_LIKE);
    assert_eq!(handled, all, "assembly handles exactly the closed list");
    assert_eq!(
        rustred_campaign_core::import::lines::KNOWN_KINDS,
        TABLE_LINE_KINDS
    );
    assert!(!all.contains("sym_zero"));
}

#[test]
fn sym_parser_is_strict() {
    let ok = sym_record(4, &[Some(2), Some(1), Some(1), Some(0), Some(0)], "+").to_string();
    let r = parse_sym_line(ok.as_bytes(), 9).unwrap();
    assert_eq!((r.index, r.line_no, r.sign.as_str()), (4, 9, "+"));
    assert_eq!(r.action[0], Some((1, false)));
    let rr = r.as_rule_record(true);
    assert!(rr.exceptions.is_empty());
    assert_eq!(rr.rhs_keys, Some(vec![r.image.clone()]));
    assert_eq!(rr.witness_loci, Some(vec![]));
    for (tag, m) in [
        ("rhs", json!({"rhs": [["I(1,1,1,0,0)", "1"]]})),
        ("exc", json!({"exceptions": []})),
        ("action", json!({"action": [[1, false]]})),
        ("noindex", json!({"index": null})),
    ] {
        let mut v = sym_record(4, &[Some(2), Some(1), Some(1), Some(0), Some(0)], "+");
        for (k, x) in m.as_object().unwrap() {
            v[k] = x.clone();
        }
        assert!(
            parse_sym_line(v.to_string().as_bytes(), 1).is_err(),
            "{tag}"
        );
    }
}

fn input(path: &std::path::Path, terminals: Vec<Vec<i32>>) -> AssemblyInput {
    AssemblyInput {
        node: "toy".into(),
        target: NodeTarget::Full,
        inputs: vec![InputTable {
            id: "in".into(),
            path: path.to_path_buf(),
            priority: 0,
            sha256: None,
        }],
        accepted: vec![],
        terminals,
        credits: vec![],
        retirements: vec![],
        removals: vec![],
        full_retirements: FullRetirements::Refuse,
        keep_skipped: false,
        used: None,
        renumber_index: true,
        recipes: 0,
    }
}

/// ASSEMBLY RULE (18.2): the ledger record is not copied; its keys are input terminals (must be declared); every
/// declared terminal is regenerated as a residual record. A composite closing marker closes an input table.
#[test]
fn assembly_reads_ledger_keys_as_input_terminals() {
    let d = scratch("assemble-ledger");
    let t1 = vec![1, 1, 1, 0, 0];
    let t2 = vec![2, 1, 1, 0, 0];
    let tail = vec![
        json!({"type": "composite_terminal_ledger", "keys": [kt(&t1), kt(&t2)], "count": 2}).to_string(),
        json!({"type": "composite_assembly_complete", "complete": true, "rules": 1, "residuals": 2, "expected_LP": 2})
            .to_string(),
    ];
    let t = write_table_with(
        &d,
        "in.jsonl",
        &header_value(),
        &[point_rule(0, &[2, 2, 1, 0, 0], &[&t1, &t2])],
        Some(&tail),
    );
    let out = assemble(&input(&t, vec![t1.clone(), t2.clone()]), &d.join("out")).unwrap();
    assert!(out.report.undeclared_input_terminals.is_empty());
    let text = std::fs::read_to_string(&out.table).unwrap();
    assert!(!text.contains("composite_terminal_ledger"));
    assert_eq!(text.matches("\"residual\"").count(), 2, "{text}");
    let out = assemble(&input(&t, vec![]), &d.join("out2")).unwrap();
    assert_eq!(out.report.undeclared_input_terminals.len(), 2);
    assert!(!out.report.export_ready);
    // a sym record in an input is refused by the assembly
    let s = write_table(
        &d,
        "sym.jsonl",
        &[sym_record(0, &[Some(2), Some(1), Some(1), Some(0), Some(0)], "+").to_string()],
    );
    assert!(assemble(&input(&s, vec![]), &d.join("out3")).is_err());
}
