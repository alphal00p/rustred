//! Coordinator decision C (COORDINATOR-ANSWERS-20261116-1.md), implemented by lane parametric: a rule record's `domain`
//! member (bounds on the case variables) is BINDING for dispatch; `source_rows`, `verification` and `case_note` are
//! informational. Contradictions and unknown forms refuse the record; unknown members still refuse.

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
fn domain_is_binding_and_informational_members_are_read() {
    let dir = scratch("bind");
    // n3 <= -1 (numerator) and n0 >= 2 (line); provenance members alongside
    let r = rule(
        0,
        "[null,1,null,null]",
        "I(n0,1,n2,n3)",
        r#","domain":{"n3":"<= -1","n0":">= 2"},"source_rows":[["I(n0,1,n2,n3)","symmetry"]],"verification":"x","case_note":"y""#,
    );
    let d = load(&table(&dir, "a.jsonl", &[r])).unwrap();
    assert!(served(&d, &[2, 1, 0, -1]));
    assert!(served(&d, &[5, 1, -3, -4]));
    assert!(!served(&d, &[2, 1, 0, 0]), "n3 = 0 is outside the domain");
    assert!(!served(&d, &[1, 1, 0, -1]), "n0 = 1 is outside the domain");
    // without the member the record serves those keys: the member is what restricts
    let r2 = rule(0, "[null,1,null,null]", "I(n0,1,n2,n3)", "");
    let d2 = load(&table(&dir, "b.jsonl", &[r2])).unwrap();
    assert!(served(&d2, &[2, 1, 0, 0]) && served(&d2, &[1, 1, 0, -1]));
    // it merges with an existing domain_bounds (intersection)
    let r3 = rule(
        0,
        "[null,1,null,null]",
        "I(n0,1,n2,n3)",
        r#","domain_bounds":{"min":[3,null,null,null],"max":null},"domain":{"n3":"<= -1"}"#,
    );
    let d3 = load(&table(&dir, "c.jsonl", &[r3])).unwrap();
    assert!(
        !served(&d3, &[2, 1, 0, -1]) && served(&d3, &[3, 1, 0, -1]) && !served(&d3, &[3, 1, 0, 0])
    );
}

#[test]
fn contradictions_and_unknown_forms_refuse() {
    let dir = scratch("refuse");
    let cases = [
        // fixed slot n1 = 1 contradicts n1 >= 2
        (r#","domain":{"n1":">= 2"}"#, "fixed"),
        // a line bounded <= 0
        (r#","domain":{"n0":"<= 0"}"#, "chart"),
        // a numerator bounded >= 1
        (r#","domain":{"n2":">= 1"}"#, "chart"),
        // empty with domain_bounds
        (
            r#","domain_bounds":{"min":[3,null,null,null]},"domain":{"n0":"<= 2"}"#,
            "empty",
        ),
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
        assert!(e.contains(why) || e.contains("domain"), "case {i}: {e}");
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
