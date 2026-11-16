//! Lane procfix7 round 4 (gate8 G8-B1, the /proc CLASS): a source tripwire. Spawns nothing.
//!
//! The rule (src/resources/procfs.rs): `resources::procfs` is the ONLY code of this crate that opens `/proc`, and every
//! read there returns `ProcResult<T>` (Gone only for ENOENT / ESRCH, everything else Unreadable). Hence:
//! 1. no string literal that STARTS with `/proc` (a path) anywhere in `src/` outside `src/resources/procfs.rs`, the
//!    test helper program `src/bin/rr_fake_child.rs` and `#[cfg(test)] mod tests` blocks (a message that mentions /proc
//!    in the middle of a sentence is not a path);
//! 2. every `pub fn` of `impl ProcRoot` that reads returns `ProcResult<..>` (no `Option` from a /proc read);
//! 3. no read result of the wrapper is collapsed on the same line (`.ok()`, `.unwrap_or*`, `.is_ok()`, `.is_err()`
//!    after a `procfs::` call, `read_stat(` or `boot_id()`), unless the line above carries a reviewed
//!    `procfix8-ok: <reason>`.

use std::path::{Path, PathBuf};

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
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

/// The lines of a file outside its `#[cfg(test)] mod tests` block (which ends every file here).
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

fn exempt(r: &str) -> bool {
    r == "src/resources/procfs.rs" || r.starts_with("src/bin/")
}

#[test]
fn no_proc_path_outside_the_wrapper() {
    let mut bad = Vec::new();
    for p in sources() {
        let r = rel(&p);
        if exempt(&r) {
            continue;
        }
        for (n, l) in code_lines(&p) {
            let t = l.trim_start();
            if t.starts_with("//") {
                continue;
            }
            if t.contains("\"/proc") {
                bad.push(format!("{r}:{n}: {t}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "a /proc path outside src/resources/procfs.rs (every /proc read goes through the wrapper):\n{}",
        bad.join("\n")
    );
}

#[test]
fn every_reader_of_the_wrapper_returns_a_proc_result() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/resources/procfs.rs");
    let lines = code_lines(&p);
    let not_readers = ["system", "at", "with_faults", "dir", "is_system"];
    let mut in_impl = false;
    let mut bad = Vec::new();
    let mut checked = 0usize;
    for (k, (n, l)) in lines.iter().enumerate() {
        if l.starts_with("impl ProcRoot") {
            in_impl = true;
            continue;
        }
        if in_impl && l.starts_with('}') {
            in_impl = false;
        }
        let t = l.trim_start();
        if !in_impl || !t.starts_with("pub fn ") {
            continue;
        }
        let name = t["pub fn ".len()..]
            .split(['(', '<'])
            .next()
            .unwrap_or("")
            .to_string();
        if not_readers.contains(&name.as_str()) {
            continue;
        }
        // the signature may span lines: up to the opening brace
        let sig: String = lines[k..]
            .iter()
            .take(8)
            .map(|(_, x)| x.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let sig = sig.split('{').next().unwrap_or("");
        checked += 1;
        if !sig.contains("-> ProcResult<") {
            bad.push(format!("procfs.rs:{n}: {name}"));
        }
    }
    assert!(checked >= 20, "only {checked} readers found");
    assert!(
        bad.is_empty(),
        "readers of the wrapper that do not return ProcResult:\n{}",
        bad.join("\n")
    );
}

#[test]
fn no_wrapper_result_is_collapsed_without_a_reviewed_reason() {
    let calls = ["procfs::", "read_stat(", "boot_id()"];
    let collapse = [
        ".ok()",
        ".unwrap_or(",
        ".unwrap_or_default()",
        ".unwrap_or_else(",
        ".is_ok()",
        ".is_err()",
    ];
    let mut bad = Vec::new();
    for p in sources() {
        let r = rel(&p);
        if exempt(&r) {
            continue;
        }
        let lines = code_lines(&p);
        for (k, (n, l)) in lines.iter().enumerate() {
            let t = l.trim_start();
            if t.starts_with("//") {
                continue;
            }
            if !calls.iter().any(|c| t.contains(c)) || !collapse.iter().any(|c| t.contains(c)) {
                continue;
            }
            let reviewed = lines[k.saturating_sub(3)..k]
                .iter()
                .any(|(_, x)| x.contains("procfix8-ok:"));
            if !reviewed {
                bad.push(format!("{r}:{n}: {t}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "a /proc read result collapsed (Gone and Unreadable made one) without `procfix8-ok: <reason>` above:\n{}",
        bad.join("\n")
    );
}
