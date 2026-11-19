//! Lane procfix7 round 5 (gate9 G9-B1, THE FILESYSTEM FACTS RULE, src/fsx.rs): a source tripwire. Spawns nothing.
//!
//! The rule is enforced by clippy (`disallowed-methods` in crates/rustred-campaign/clippy.toml: `Path::exists`,
//! `is_file`, `is_dir`, `try_exists`, `metadata`, `symlink_metadata`, `read_dir`, `std::fs::metadata`,
//! `symlink_metadata`, `read_dir`, `exists`). This tripwire keeps the rule from being dropped silently and checks
//! what clippy cannot:
//! 1. clippy.toml lists every one of those methods;
//! 2. every site label passed to an `fsx::` helper in `src/` is one of `fsx::SITES` (a fault can be injected at it);
//! 3. no file READ of production code (`fs::read(`, `fs::read_to_string(`, `fsx::read*(`) is collapsed in the same
//!    statement into "no content" (`.ok()`, `.unwrap_or_default()`, `.unwrap_or(`, `let Ok(..) = .. else`,
//!    `if let Ok(`) unless a line within the three above carries a reviewed `procfix9-ok: <reason>`;
//! 4. no `#[allow(clippy::disallowed_methods)]` in production code outside `src/fsx.rs`, `src/resources/procfs.rs`
//!    (the /proc wrapper, its own rule) and the test double `src/bin/rr_fake_child.rs`.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use std::path::{Path, PathBuf};

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut v = Vec::new();
    rs_files(&root, &mut v);
    v.sort();
    v
}

fn rel(p: &Path) -> String {
    p.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// The lines of a file before its `#[cfg(test)] mod tests` block (which ends every file here).
fn code_lines(p: &Path) -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(p).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.trim_start().starts_with("mod tests")
            && lines[i.saturating_sub(3)..i]
                .iter()
                .any(|p| p.contains("cfg(test)"))
        {
            break;
        }
        out.push((i + 1, l.to_string()));
    }
    out
}

#[test]
fn clippy_toml_forbids_the_collapsing_std_forms() {
    let t = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("clippy.toml"))
        .expect("crates/rustred-campaign/clippy.toml");
    for m in [
        "std::path::Path::exists",
        "std::path::Path::try_exists",
        "std::path::Path::is_file",
        "std::path::Path::is_dir",
        "std::path::Path::metadata",
        "std::path::Path::symlink_metadata",
        "std::path::Path::read_dir",
        "std::fs::exists",
        "std::fs::metadata",
        "std::fs::symlink_metadata",
        "std::fs::read_dir",
    ] {
        assert!(
            t.contains(&format!("\"{m}\"")),
            "clippy.toml must forbid {m}"
        );
    }
}

#[test]
fn every_fsx_site_label_is_registered() {
    let sites = rustred_campaign::fsx::SITES;
    let calls = [
        "fsx::presence(",
        "fsx::presence_lstat(",
        "fsx::list(",
        "fsx::list_required(",
        "fsx::read(",
        "fsx::read_to_string(",
        "fsx::open(",
    ];
    let mut bad = Vec::new();
    let mut used = std::collections::BTreeSet::new();
    let mut seen = 0usize;
    for p in sources() {
        if rel(&p) == "src/fsx.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        for c in calls {
            for (i, _) in text.match_indices(c) {
                // the first argument (after optional white space, also across a line break) is the site label
                let rest = text[i + c.len()..].trim_start();
                let Some(r) = rest.strip_prefix('"') else {
                    bad.push(format!("{}: a call whose site is not a literal", rel(&p)));
                    continue;
                };
                let label = r.split('"').next().unwrap_or("");
                seen += 1;
                used.insert(label.to_string());
                if !sites.contains(&label) {
                    bad.push(format!("{}: {label:?}", rel(&p)));
                }
            }
        }
    }
    assert!(seen >= 80, "only {seen} fsx calls found");
    assert!(
        bad.is_empty(),
        "fsx site labels missing from fsx::SITES:\n{}",
        bad.join("\n")
    );
    let unused: Vec<&&str> = sites.iter().filter(|s| !used.contains(**s)).collect();
    assert!(
        unused.is_empty(),
        "labels of fsx::SITES no call uses: {unused:?}"
    );
}

#[test]
fn no_file_read_is_collapsed_into_no_content() {
    let reads = [
        "fs::read(",
        "fs::read_to_string(",
        "fsx::read(",
        "fsx::read_to_string(",
    ];
    let collapse = [".ok()", ".unwrap_or_default()", ".unwrap_or(", "if let Ok("];
    let mut bad = Vec::new();
    for p in sources() {
        let r = rel(&p);
        if r.starts_with("src/bin/") || r == "src/fsx.rs" {
            continue;
        }
        let lines = code_lines(&p);
        // statements: from a line with a read to the line that ends it (`;` or `{` at the end), at most 8 lines
        for (k, (n, l)) in lines.iter().enumerate() {
            let t = l.trim_start();
            if t.starts_with("//") || !reads.iter().any(|x| t.contains(x)) {
                continue;
            }
            let mut stmt = String::new();
            for (_, x) in lines[k..].iter().take(8) {
                stmt.push_str(x.trim());
                stmt.push(' ');
                let e = x.trim_end();
                if e.ends_with(';') || e.ends_with('{') || e.ends_with(',') {
                    break;
                }
            }
            // the statement before the read on the same line (`let Ok(x) = fs::read(..) else`)
            let let_ok_else = t.starts_with("let Ok(") && stmt.contains(" else");
            if !(collapse.iter().any(|c| stmt.contains(c)) || let_ok_else) {
                continue;
            }
            // a typed match on fsx::Read is not a collapse
            if stmt.contains("fsx::Read::") {
                continue;
            }
            let reviewed = lines[k.saturating_sub(3)..k]
                .iter()
                .any(|(_, x)| x.contains("procfix9-ok:"));
            if !reviewed {
                bad.push(format!("{r}:{n}: {}", stmt.trim()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "a file read collapsed into \"no content\" (absent and unreadable made one) without `procfix9-ok: <reason>` above:\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_rule_is_not_switched_off_in_production_code() {
    let allowed = [
        "src/fsx.rs",
        "src/resources/procfs.rs",
        "src/bin/rr_fake_child.rs",
    ];
    let mut bad = Vec::new();
    for p in sources() {
        let r = rel(&p);
        if allowed.contains(&r.as_str()) {
            continue;
        }
        for (n, l) in code_lines(&p) {
            if l.contains("clippy::disallowed_methods") && !l.trim_start().starts_with("//") {
                // the allow placed directly above a `#[cfg(test)]` module is test code
                bad.push(format!("{r}:{n}: {}", l.trim()));
            }
        }
    }
    // test modules carry the allow on the line directly above `#[cfg(test)]`: those lines are part of code_lines (the
    // module starts after them); accept exactly that placement
    let bad: Vec<String> = bad
        .into_iter()
        .filter(|b| !b.contains("test code: the fsx rule"))
        .collect();
    assert!(
        bad.is_empty(),
        "the fsx rule (clippy disallowed_methods) switched off in production code:\n{}",
        bad.join("\n")
    );
}

/// procfix7 round 6 (gate10 m10-4: clippy's disallowed-methods is a PROCESS gate, and some error-collapsing forms
/// (`File::open(p).is_ok()`, `canonicalize(..)` fallbacks, `read_link(..).is_ok()`, a DirEntry's `file_type()` mapped to
/// a default) have no clippy rule): the suite itself runs the mechanical candidate list (audit/fs_sites.py) and fails
/// when a PRODUCTION candidate has no review (audit/fs_sites_review.tsv) and no verdict the code shows by itself.
#[test]
fn every_production_candidate_of_the_mechanical_list_is_reviewed() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("audit/fs_sites.py");
    let out = match std::process::Command::new("python3")
        .arg("-I")
        .arg(&script)
        .arg("--check")
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("SKIPPED: python3 unavailable ({e})");
            return;
        }
    };
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && text.contains("production candidates UNREVIEWED: 0"),
        "audit/fs_sites.py --check failed (a new production site needs a review in audit/fs_sites_review.tsv):\n{text}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// procfix7 round 6 (gate10 m10-4): the mechanical list catches EVERY known evasion form. audit/evasion_forms.rs holds
/// gate10's 23 clippy-evasion probes (scripts/gate10/clippytest) and further forms, one per line marked EXPECT; the
/// script, run with `--scan` (the file as production code, no review), must report an UNREVIEWED candidate on each.
/// A form that escapes the list fails here (the rule is enforced by the suite, not only by clippy before a commit).
#[test]
fn the_mechanical_list_catches_every_known_evasion_form() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = dir.join("audit/evasion_forms.rs");
    let out = match std::process::Command::new("python3")
        .arg("-I")
        .arg(dir.join("audit/fs_sites.py"))
        .arg("--scan")
        .arg(&fixture)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("SKIPPED: python3 unavailable ({e})");
            return;
        }
    };
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let flagged: std::collections::BTreeSet<usize> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() >= 3 && f[2] == "UNREVIEWED").then(|| f[1].parse().ok())?
        })
        .collect();
    let text = std::fs::read_to_string(&fixture).unwrap();
    let expected: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("// EXPECT") && !l.trim_start().starts_with("//"))
        .map(|(i, l)| (i + 1, l))
        .collect();
    assert!(expected.len() >= 30, "the fixture lost its forms");
    let missed: Vec<String> = expected
        .iter()
        .filter(|(n, _)| !flagged.contains(n))
        .map(|(n, l)| format!("line {n}: {l}"))
        .collect();
    assert!(
        missed.is_empty(),
        "evasion forms the mechanical list does not catch:\n{}",
        missed.join("\n")
    );
}

/// The string literal right after each occurrence of `pat` in `line` (the literal starts where `pat` ends).
fn literals_after(line: &str, pat: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = line[from..].find(pat) {
        let start = from + i + pat.len();
        match line[start..].find('"') {
            Some(j) => {
                out.push((line[start..start + j].to_string(), start + j + 1));
                from = start + j + 1;
            }
            None => break,
        }
    }
    out
}

/// procfix7 round 20 (gate R-C; the pf7land gate's finding: `ship-gate.json` and `shipecon/proposal.json` registered without
/// a sentinel): every file a closing-loop node writes under a FIXED name in its `final/` directory is a sentinel of the final
/// walk (`closeloop::pass::FINAL_SENTINELS`), so a listing cut short cannot drop it from the registry unnoticed. Scanned
/// forms (production code of src/closeloop): `final_dir.join("<f>")`, `final_dir.join("<d>").join("<f>")`,
/// `sdir.join("<f>")` / `statement_dir.join("<f>")` (= `statement/<f>`), and the literals `"final/<p>"` and
/// `"{..}/final/<p>"` without a placeholder in `<p>`. Directories and the names the walk does not register are skipped
/// (NOT_REGISTERED). A path built otherwise (the ship economy's proposal: `<statement>/../shipecon/proposal.json`) is covered
/// by procfix10's control, which checks a real run's registry against the same list.
#[test]
fn every_fixed_final_output_is_a_listing_sentinel() {
    const NOT_REGISTERED: &[&str] = &["result.json", "terminal_proposals.json"];
    let sentinels = rustred_campaign::closeloop::pass::FINAL_SENTINELS;
    let is_file = |n: &str| n.contains('.') || n.ends_with("SHA256SUMS");
    let mut seen = std::collections::BTreeSet::new();
    let mut bad = Vec::new();
    for p in sources() {
        let r = rel(&p);
        if !r.starts_with("src/closeloop/") {
            continue;
        }
        for (n, l) in code_lines(&p) {
            let t = l.trim_start();
            if t.starts_with("//") {
                continue;
            }
            let mut names = Vec::new();
            for (lit, end) in literals_after(&l, "final_dir.join(\"") {
                match l[end..].strip_prefix(").join(\"") {
                    Some(rest) => {
                        if let Some(j) = rest.find('"') {
                            names.push(format!("{lit}/{}", &rest[..j]));
                        }
                    }
                    None => names.push(lit),
                }
            }
            for pat in ["sdir.join(\"", "statement_dir.join(\""] {
                for (lit, _) in literals_after(&l, pat) {
                    names.push(format!("statement/{lit}"));
                }
            }
            for pat in ["\"final/", "}/final/"] {
                for (lit, _) in literals_after(&l, pat) {
                    names.push(lit);
                }
            }
            for name in names {
                if name.contains('{') || !is_file(&name) || NOT_REGISTERED.contains(&name.as_str())
                {
                    continue;
                }
                seen.insert(name.clone());
                if !sentinels.contains(&name.as_str()) {
                    bad.push(format!("{r}:{n}: final/{name}"));
                }
            }
        }
    }
    eprintln!("fixed-name final outputs found in the sources: {seen:?}");
    assert!(seen.len() >= 10, "the scan found too few outputs: {seen:?}");
    assert!(
        bad.is_empty(),
        "fixed-name outputs of final/ without a sentinel in closeloop::pass::FINAL_SENTINELS (a truncated listing could drop them unnoticed):\n{}",
        bad.join("\n")
    );
}
