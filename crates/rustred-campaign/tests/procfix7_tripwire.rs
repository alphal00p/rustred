//! Lane procfix7 (gate7 B1, the CLASS): a source tripwire in the style of tests/procfix3_proc.rs. Spawns nothing.
//!
//! The principle: in the process layer and the supervisor, a failed read may only make the supervisor MORE
//! conservative (never fewer hunted processes, never exit 0). Hence:
//! 1. every `let _ =` (an ignored result) in src/supervisor, src/launch and src/resources (outside `mod tests`) carries,
//!    on the line above, `#[allow(clippy::let_underscore_must_use)] // ignored-ok (procfix7): <reason>`; the per-file
//!    counts are pinned below (the reviewed list of WS/notes/implementer.procfix7.md): a new site must be reviewed and
//!    the count raised here;
//! 2. the three module roots deny `clippy::let_underscore_must_use` and `unused_must_use`;
//! 3. no ledger pass (`for_each_record`) has its result dropped.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut v = Vec::new();
    for d in ["supervisor", "launch", "resources"] {
        for e in std::fs::read_dir(root.join(d)).unwrap().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "rs") {
                v.push(p);
            }
        }
    }
    v.sort();
    v
}

/// The lines of a source file outside its `#[cfg(test)] mod tests` block (which always ends the file here).
fn code_lines(p: &Path) -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(p).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.trim_start().starts_with("mod tests")
            && lines[i.saturating_sub(2)..i]
                .iter()
                .any(|p| p.contains("cfg(test)"))
        {
            break;
        }
        out.push((i + 1, l.to_string()));
    }
    out
}

fn rel(p: &Path) -> String {
    p.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// The reviewed `let _ =` sites per file (procfix7, gate7 B1). Raise a count only after reviewing the new site.
const REVIEWED: &[(&str, usize)] = &[
    ("src/launch/cgroup.rs", 8),
    ("src/launch/guard.rs", 7),
    ("src/launch/hunt.rs", 4),
    ("src/launch/idlock.rs", 6),
    ("src/launch/real.rs", 11),
    ("src/launch/sweep.rs", 1),
    ("src/resources/children.rs", 1),
    ("src/resources/scan.rs", 1),
    ("src/supervisor/failpoint.rs", 1),
    ("src/supervisor/mod.rs", 16),
    ("src/supervisor/workdir.rs", 3),
];

#[test]
fn every_ignored_result_is_a_reviewed_exception() {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut bad = Vec::new();
    for p in sources() {
        let lines = code_lines(&p);
        for (k, (n, l)) in lines.iter().enumerate() {
            let t = l.trim_start();
            if t.starts_with("//") || !t.contains("let _ =") {
                continue;
            }
            *counts.entry(rel(&p)).or_default() += 1;
            // rustfmt puts the reason comment on its own line between the attribute and the `let`
            let above: Vec<&str> = lines[k.saturating_sub(3)..k]
                .iter()
                .map(|(_, x)| x.trim())
                .collect();
            let has_allow = above
                .iter()
                .any(|x| x.starts_with("#[allow(clippy::let_underscore_must_use)]"));
            let reason = above
                .iter()
                .find_map(|x| x.split_once("ignored-ok (procfix7):"))
                .map(|(_, r)| r.trim())
                .unwrap_or("");
            if !has_allow || reason.len() < 8 {
                bad.push(format!("{}:{n}: {}", rel(&p), t));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "ignored results without a reviewed exception (line above: `#[allow(clippy::let_underscore_must_use)] // ignored-ok (procfix7): <reason>`):\n{}",
        bad.join("\n")
    );
    let want: BTreeMap<String, usize> = REVIEWED.iter().map(|(f, n)| (f.to_string(), *n)).collect();
    assert_eq!(
        counts, want,
        "the reviewed `let _ =` sites changed: review the new site (WS/notes/implementer.procfix7.md) and update REVIEWED"
    );
}

#[test]
fn the_module_roots_deny_ignored_results() {
    for f in [
        "src/supervisor/mod.rs",
        "src/launch/mod.rs",
        "src/resources/mod.rs",
    ] {
        let t = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(f)).unwrap();
        assert!(
            t.contains("#![deny(clippy::let_underscore_must_use, unused_must_use)]"),
            "{f} must deny ignored results"
        );
    }
}

#[test]
fn no_ledger_pass_drops_its_result() {
    let mut bad = Vec::new();
    for p in sources() {
        for (n, l) in code_lines(&p) {
            let t = l.trim_start();
            if t.contains("for_each_record(") && (t.starts_with("let _") || t.contains(".ok()")) {
                bad.push(format!("{}:{n}: {t}", rel(&p)));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "ledger passes whose result is dropped:\n{}",
        bad.join("\n")
    );
}
