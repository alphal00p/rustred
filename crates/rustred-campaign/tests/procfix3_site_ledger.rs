//! The shared host CPU ledger (`WS/ops/CPU_LEDGER.md`) and lock directory are APPEND-ONLY / FLOCK-ONLY for this crate
//! (directing session, after the host file was truncated by some lane at 2026-11-14T07:04:41Z; SUBAGENT_RULES L13).
//! (a) runtime: a private temp ledger pre-filled with other lanes' lines keeps its earlier bytes as an unchanged PREFIX
//!     (same inode, never truncated, rewritten or renamed over) through acquire / release / drop / checked release /
//!     the abandoned-acquire helper / the restart reconciliation of "missing release lines";
//! (b) source: in every src file that touches the site config's `ledger_file` / `lock_dir`, no `File::create`,
//!     `fs::write`, `.truncate(true)`, `rename(`, `remove_file`, `set_len`, and every `OpenOptions` chain that opens for
//!     writing says `.append(true)` (ledger) or `.truncate(false)` (lock files: create without truncate only);
//! (c) no test of the crate can reach the real WS/ops paths: the only sources naming them are the documented example
//!     (parsed, never opened) and the schema example; every `SitePolicy::new` on that example first replaces
//!     `lock_dir` and `ledger_file` by a temp path.
//! Never touches the real WS/ops (rule L13): everything below lives in a private temp directory.

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::cpu_policy::{Acquire, CpuPolicy, SITE_SCHEMA, SiteConfig, SitePolicy};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn cpus4() -> Vec<usize> {
    rustred_campaign::resources::allowed_cpus()
        .unwrap()
        .into_iter()
        .filter(|c| !(168..=175).contains(c) && !(288..=319).contains(c))
        .take(4)
        .collect()
}

fn site(root: &Path, cpus: &[usize]) -> SiteConfig {
    SiteConfig {
        schema: SITE_SCHEMA.into(),
        tag: "rr-campaign".into(),
        lock_dir: root.join("locks"),
        lock_name: "cpu.{n}".into(),
        ledger_file: root.join("CPU_LEDGER.md"),
        ledger_scan_lines: 1500,
        candidates: Some(
            cpus.iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        exclude: vec![],
        held_files: vec![],
        pause_files: vec![],
        idle_min: 0.0,
        max_cores: 4,
        respect_foreign_pinning: false,
        ignore_own_tag_claims: false,
    }
}

/// The ledger's identity and bytes now.
fn snap(p: &Path) -> (u64, u64, Vec<u8>) {
    let md = fs::metadata(p).unwrap();
    (md.dev(), md.ino(), fs::read(p).unwrap())
}

/// The earlier bytes are an unchanged prefix of the file, which is the same inode.
fn assert_prefix(p: &Path, before: &(u64, u64, Vec<u8>), step: &str) -> (u64, u64, Vec<u8>) {
    let now = snap(p);
    assert_eq!(
        (now.0, now.1),
        (before.0, before.1),
        "{step}: the ledger was replaced (inode)"
    );
    assert!(
        now.2.len() >= before.2.len() && now.2[..before.2.len()] == before.2[..],
        "{step}: the earlier ledger bytes are not an unchanged prefix any more"
    );
    now
}

/// (a) every write path of SitePolicy only APPENDS to the ledger.
#[test]
fn site_ledger_writes_only_append_earlier_bytes_stay_a_prefix() {
    let _g = serial();
    let d = TempDir::new("site-append");
    let root = d.path().to_path_buf();
    let cpus = cpus4();
    assert!(cpus.len() >= 4, "{cpus:?}");
    fs::create_dir_all(root.join("locks")).unwrap();
    let ledger = root.join("CPU_LEDGER.md");
    // Other lanes' lines, a stale own claim with no RELEASED line (its holder died: the flock is free), and a last
    // line WITHOUT newline (a writer that died mid-line): all of it must survive byte for byte.
    let other = format!(
        "# CPU ledger (shared; other lanes)\n\
         2026-11-14T06:00:00Z (t162-cert): + CPU 9999 (flock) -> worker. Released at its end.\n\
         2026-11-14T06:00:01Z (other-lane): + CPUs 9998,9997 (flock) -> long job. Released at its end.\n\
         2026-11-14T06:00:02Z (rr-campaign): + CPUs {} (flock) -> crashed job a1. Released at its end.\n\
         2026-11-14T06:00:03Z (t162-cert): RELEASED 9999 (worker ended)\n\
         2026-11-14T06:00:04Z (5l-pilot): + CPU 9996 (flock) -> torn",
        cpus[3]
    );
    fs::write(&ledger, other.as_bytes()).unwrap();
    let mut s = snap(&ledger);
    let mut p = SitePolicy::new(site(&root, &cpus)).unwrap();
    s = assert_prefix(&ledger, &s, "new");
    // the restart reconciliation of "missing release lines"
    let r = p.reconcile_own_claims().unwrap();
    assert_eq!(r.released, vec![cpus[3]], "{r:?}");
    s = assert_prefix(&ledger, &s, "reconcile_own_claims");
    // acquire + explicit release
    let lease = match p.acquire(1, "job one").unwrap() {
        Acquire::Granted(l) => l,
        Acquire::NotNow(w) => panic!("not granted: {w}"),
    };
    s = assert_prefix(&ledger, &s, "acquire");
    lease.release().unwrap();
    s = assert_prefix(&ledger, &s, "release");
    // acquire + Drop
    match p.acquire(2, "job two").unwrap() {
        Acquire::Granted(l) => drop(l),
        Acquire::NotNow(w) => panic!("not granted: {w}"),
    }
    std::thread::sleep(Duration::from_millis(300));
    s = assert_prefix(&ledger, &s, "drop");
    // checked release while another open file description holds one CPU's flock (its claim stays, still append-only)
    let lease = match p.acquire(1, "job three").unwrap() {
        Acquire::Granted(l) => l,
        Acquire::NotNow(w) => panic!("not granted: {w}"),
    };
    let held = root.join("locks").join(format!("cpu.{}", lease.cpus()[0]));
    let (r, after) = {
        let holder = fs::OpenOptions::new().read(true).open(&held).unwrap();
        let (r, after) = std::thread::scope(|sc| {
            sc.spawn(|| {
                // take the flock as soon as the lease drops its own
                let t0 = std::time::Instant::now();
                while holder.try_lock().is_err() && t0.elapsed() < Duration::from_secs(5) {
                    std::thread::sleep(Duration::from_micros(200));
                }
            });
            lease.release_checked()
        });
        drop(holder);
        (r, after)
    };
    let _ = (r, after);
    s = assert_prefix(&ledger, &s, "release_checked");
    // the abandoned acquire: an I/O bound so small that the helper finishes after the caller gave up; its late
    // claim is released by the helper (both lines appended)
    p.set_io_timeout(Duration::from_nanos(1));
    let _ = p.acquire(1, "job four");
    std::thread::sleep(Duration::from_secs(1));
    s = assert_prefix(&ledger, &s, "abandoned acquire");
    p.set_io_timeout(Duration::from_secs(10));
    // a second policy instance (a restart) reconciling again: nothing rewritten
    drop(p);
    let mut q = SitePolicy::new(site(&root, &cpus)).unwrap();
    let _ = q.reconcile_own_claims().unwrap();
    s = assert_prefix(&ledger, &s, "second reconcile");
    // the original bytes are still the head of the file
    assert!(s.2.starts_with(other.as_bytes()));
    // no other file appeared next to the ledger (no temp file renamed over it), lock files only in locks/
    let names: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        vec!["CPU_LEDGER.md".to_string(), "locks".to_string()],
        "{names:?}"
    );
    for e in fs::read_dir(root.join("locks")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        assert!(n.starts_with("cpu."), "unexpected file in locks/: {n}");
        assert_eq!(
            e.metadata().unwrap().len(),
            0,
            "lock files are never written: {n}"
        );
    }
}

/// The non-test part of a source file (up to a `#[cfg(test)]` that opens a `mod`).
fn code_of(text: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("#[cfg(test)]")
            && lines[i + 1..]
                .iter()
                .find(|l| !l.trim().is_empty())
                .is_some_and(|l| l.trim_start().starts_with("mod "))
        {
            break;
        }
        out.push((i + 1, line.split("//").next().unwrap_or("").to_string()));
    }
    out
}

fn rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    files
}

/// (b) source-level: the site config's ledger and lock paths are reached only by O_APPEND (ledger) or
/// create-without-truncate (lock files) opens.
#[test]
fn site_ledger_and_lock_paths_are_opened_append_or_create_without_truncate_only() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut touching = Vec::new();
    let mut hits = Vec::new();
    for f in rs_files(&src) {
        let rel = f.strip_prefix(&src).unwrap().display().to_string();
        let text = fs::read_to_string(&f).unwrap();
        let code = code_of(&text);
        // files that use the SITE config's paths (field accesses; the campaign ledger's own `ledger_dir` and the id
        // lock's `id_lock_dir` / `default_lock_dir` are other things)
        let uses = code
            .iter()
            .any(|(_, c)| c.contains(".ledger_file") || c.contains(".lock_dir"));
        if !uses {
            continue;
        }
        touching.push(rel.clone());
        for (n, c) in &code {
            for pat in [
                "File::create",
                "fs::write(",
                ".truncate(true)",
                "rename(",
                "remove_file",
                "set_len(",
            ] {
                if c.contains(pat) {
                    hits.push(format!("{rel}:{n}: [{pat}] {}", c.trim()));
                }
            }
        }
        // every OpenOptions chain (up to its `.open(`) that writes: append, or truncate(false)
        let joined: Vec<(usize, &str)> = code.iter().map(|(n, c)| (*n, c.as_str())).collect();
        for (k, (n, c)) in joined.iter().enumerate() {
            if !c.contains("OpenOptions::new()") {
                continue;
            }
            let mut chain = String::new();
            for (_, l) in joined[k..].iter().take(12) {
                chain.push_str(l);
                if l.contains(".open(") {
                    break;
                }
            }
            if chain.contains(".write(true)")
                && !chain.contains(".append(true)")
                && !chain.contains(".truncate(false)")
            {
                hits.push(format!(
                    "{rel}:{n}: OpenOptions for writing without append or truncate(false): {}",
                    chain.split_whitespace().collect::<Vec<_>>().join(" ")
                ));
            }
        }
    }
    assert_eq!(
        touching,
        vec!["cpu_policy.rs".to_string()],
        "the site ledger / lock paths are used in cpu_policy.rs only"
    );
    assert!(
        hits.is_empty(),
        "writes that could rewrite the shared ledger / locks: {hits:#?}"
    );
}

/// (c) no test can reach the real WS/ops paths: the only sources (src test modules and tests/) naming them are the
/// documented site example in cpu_policy.rs and the schema example (both parsed, never opened); in cpu_policy.rs every
/// `SitePolicy::new` of a test replaces `lock_dir` and `ledger_file` first.
#[test]
fn no_test_can_reach_the_real_ws_ops_ledger_or_locks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut naming = Vec::new();
    for dir in ["src", "tests"] {
        for f in rs_files(&root.join(dir)) {
            let text = fs::read_to_string(&f).unwrap();
            if text.contains("/ops/CPU_LEDGER") || text.contains("/ops/locks") {
                naming.push(f.strip_prefix(root).unwrap().display().to_string());
            }
        }
    }
    naming.sort();
    // this very file names them in its documentation only
    naming.retain(|n| n != "tests/procfix3_site_ledger.rs");
    assert_eq!(naming, vec!["src/cpu_policy.rs".to_string()], "{naming:?}");
    let text = fs::read_to_string(root.join("src/cpu_policy.rs")).unwrap();
    let tests = &text[text.find("#[cfg(test)]").unwrap()..];
    for (k, _) in tests.match_indices("SitePolicy::new(") {
        let before = &tests[..k];
        let fn_start = before.rfind("fn ").unwrap();
        let body = &before[fn_start..];
        assert!(
            !body.contains("SITE_EXAMPLE")
                || (body.contains(".lock_dir = ") && body.contains(".ledger_file = ")),
            "a test builds a SitePolicy on the real example paths: {}",
            &body[..body.len().min(200)]
        );
    }
}
