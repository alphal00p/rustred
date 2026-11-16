//! Coordinator decision C as answered by the engine lane (COORDINATOR-ANSWERS-20261116-1.md), implemented by lane
//! parametric: a rule record's `domain` member is INFORMATIONAL for dispatch (the engine dispatches by case +
//! domain_bounds + exceptions) but must be IMPLIED by those members, else the record is refused; `source_rows`,
//! `verification` and `case_note` are informational; unknown members still refuse.

use std::path::{Path, PathBuf};

use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::record::{ReadOptions, read_table_file};

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pdm_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

const MASK: &str = "1100";

fn table(dir: &Path, name: &str, rules: &[String]) -> PathBuf {
    let mut lines = vec![format!(
        r#"{{"type":"header","schema":"table_dump.v1","family":"pdm","denominators":4,"mask":"{MASK}"}}"#
    )];
    lines.push(format!(
        r#"{{"type":"chart","chart":"{MASK}","guards":["n>=1","n>=1","n<=0","n<=0"]}}"#
    ));
    lines.extend(rules.iter().cloned());
    lines.push(format!(
        r#"{{"type":"end","charts":1,"rules":{},"residuals":0}}"#,
        rules.len()
    ));
    let p = dir.join(name);
    std::fs::write(&p, lines.join("\n") + "\n").unwrap();
    p
}

fn rule(index: usize, fixed: &str, target: &str, extra: &str) -> String {
    format!(
        r#"{{"type":"rule","chart":"{MASK}","index":{index},"fixed":{fixed},"equations":[],"exceptions":[],"target":"{target}","rhs":[],"witness_loci":[],"dispatch":"Partition"{extra}}}"#
    )
}

fn load(p: &Path) -> Result<Dispatcher, String> {
    let tf = read_table_file(p, ReadOptions::default()).map_err(|e| e.to_string())?;
    let frame = NodeFrame::from_header(tf.header.as_ref().unwrap()).map_err(|e| e.to_string())?;
    let t = Table::from_table_file("t", "pdm", &tf, DomainOptions::default())
        .map_err(|e| e.to_string())?;
    Dispatcher::new(vec![t], vec![0])
        .map_err(|e| format!("{e:?}"))?
        .with_chart(frame.mask().to_vec())
        .map_err(|e| format!("{e:?}"))
}

fn served(d: &Dispatcher, k: &[i32]) -> bool {
    matches!(d.first_match(k), Ok(Dispatch::Covered(_)))
}

#[test]
fn an_implied_domain_is_accepted_and_changes_nothing() {
    let dir = scratch("implied");
    // n3 <= -1 implied by the exception branch n3 (= 0 excluded); n0 >= 2 implied by domain_bounds min 2
    let r = rule(
        0,
        "[null,1,null,null]",
        "I(n0,1,n2,n3)",
        r#","exceptions":[["n3"]],"domain_bounds":{"min":[2,null,null,null]},"domain":{"n3":"<= -1","n0":">= 2"},"source_rows":[["I(n0,1,n2,n3)","symmetry"]],"verification":"x","case_note":"y""#,
    );
    let r = r.replacen(r#""exceptions":[],"#, "", 1);
    let d = load(&table(&dir, "a.jsonl", &[r])).unwrap();
    assert!(served(&d, &[2, 1, 0, -1]) && served(&d, &[5, 1, -3, -4]));
    assert!(!served(&d, &[2, 1, 0, 0]) && !served(&d, &[1, 1, 0, -1]));
    // a domain on a fixed slot it agrees with
    let r2 = rule(
        0,
        "[null,1,null,null]",
        "I(n0,1,n2,n3)",
        r#","domain":{"n1":"<= 1"}"#,
    );
    assert!(load(&table(&dir, "b.jsonl", &[r2])).is_ok());
}

#[test]
fn a_disagreeing_domain_and_unknown_forms_refuse() {
    let dir = scratch("refuse");
    let cases = [
        // the record serves n3 = 0, the member says n3 <= -1 (the facegen case)
        (r#","domain":{"n3":"<= -1"}"#, "not implied"),
        (r#","domain":{"n0":">= 2"}"#, "not implied"),
        // fixed slot n1 = 1 against n1 >= 2
        (r#","domain":{"n1":">= 2"}"#, "not implied"),
        // other forms, variables, values
        (r#","domain":{"n3":"< -1"}"#, "neither"),
        (r#","domain":{"n3":"== -1"}"#, "neither"),
        (r#","domain":{"n9":"<= -1"}"#, "case variable"),
        (r#","domain":{"x3":"<= -1"}"#, "case variable"),
        (r#","domain":{"n3":"<= a"}"#, "integer"),
        (r#","domain":["n3"]"#, "object"),
    ];
    for (i, (extra, why)) in cases.iter().enumerate() {
        let r = rule(0, "[null,1,null,null]", "I(n0,1,n2,n3)", extra);
        let e = load(&table(&dir, &format!("r{i}.jsonl"), &[r]))
            .err()
            .expect("refused");
        assert!(e.contains(why), "case {i}: {e}");
    }
    // an unknown member still refuses
    let r = rule(
        0,
        "[null,1,null,null]",
        "I(n0,1,n2,n3)",
        r#","domain_note":"z""#,
    );
    assert!(load(&table(&dir, "u.jsonl", &[r])).is_err());
    // null = absent
    let r = rule(
        0,
        "[null,1,null,null]",
        "I(n0,1,n2,n3)",
        r#","domain":null"#,
    );
    assert!(served(
        &load(&table(&dir, "n.jsonl", &[r])).unwrap(),
        &[1, 1, 0, 0]
    ));
}
