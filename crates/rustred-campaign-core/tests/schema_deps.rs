//! Plan 4.0: `rustred-campaign-core` must not depend (also transitively) on symbolica, graphica or numerica. Decided:
//! implemented by walking the dependency graph recorded in the workspace `Cargo.lock` (no network, no cargo call). The
//! lock lists, per package, every dependency any feature activates anywhere in the workspace, so the walk is a SUPERSET
//! of the real tree: a pass is conclusive; dev-dependencies are included (stricter than needed).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn lock() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// name -> dependency names (version qualifiers dropped: a superset when several versions exist).
fn graph(text: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut g: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut name: Option<String> = None;
    let mut in_deps = false;
    for line in text.lines() {
        let l = line.trim();
        if l == "[[package]]" {
            name = None;
            in_deps = false;
        } else if let Some(v) = l.strip_prefix("name = ") {
            let n = v.trim_matches('"').to_string();
            g.entry(n.clone()).or_default();
            name = Some(n);
        } else if l.starts_with("dependencies = [") {
            in_deps = !l.ends_with(']');
            for d in l.trim_start_matches("dependencies = [").trim_end_matches(']').split(',') {
                let d = d.trim().trim_matches('"');
                if let (Some(n), Some(dep)) = (&name, d.split_whitespace().next()) {
                    g.get_mut(n).unwrap().insert(dep.to_string());
                }
            }
        } else if in_deps {
            if l == "]" {
                in_deps = false;
            } else if let (Some(n), Some(dep)) = (&name, l.trim_matches(|c| c == '"' || c == ',').split_whitespace().next()) {
                g.get_mut(n).unwrap().insert(dep.to_string());
            }
        }
    }
    g
}

fn closure(g: &BTreeMap<String, BTreeSet<String>>, root: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![root.to_string()];
    while let Some(n) = stack.pop() {
        if seen.insert(n.clone()) {
            stack.extend(g.get(&n).into_iter().flatten().cloned());
        }
    }
    seen
}

#[test]
fn light_crates_have_no_engine_math_dependency() {
    let g = graph(&lock());
    assert!(g.contains_key("symbolica"), "the lock parser found no symbolica entry: parser broken");
    assert!(closure(&g, "rustred").contains("symbolica"), "control: rustred depends on symbolica");
    for root in ["rustred-campaign-core", "rustred-campaign-contract", "rustred-campaign"] {
        let deps = closure(&g, root);
        let bad: Vec<&str> = ["symbolica", "graphica", "numerica", "rustred", "rustred-app"]
            .into_iter()
            .filter(|b| deps.contains(*b))
            .collect();
        assert!(bad.is_empty(), "{root} depends on {bad:?} (plan 4.0 forbids it)");
        println!("{root}: {} packages in the dependency superset, none of symbolica/graphica/numerica", deps.len());
    }
}
