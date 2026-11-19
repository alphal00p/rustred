//! Lane procfix7 round 19 (the coordinator's landing condition): the NON-TEST callers of `RealLauncher::new` are
//! FAIL-CLOSED when the launcher cannot hold its (ad-hoc) campaign id. Here the closing loop's DIRECT termmin solve
//! (`closeloop::termmin::direct::solve_recipe_once`, the caller of this branch): with this process's ad-hoc id lock held
//! by ANOTHER open file description, the solve returns `Err("launcher refused: ... in use ...")` and starts no job (no
//! attempt directory, no child); with the lock free the same call gets past the launcher (any error then is not the
//! launcher's). Every other caller's handling is pinned by procfix7_tripwire `every_launcher_constructor_call_is_fail_closed`.
//!
//! Builds launchers: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh`, rule L10). ONE test function: the
//! id lock of the process is process-wide.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::termmin::direct::{DirectConfig, solve_recipe_once};
use rustred_campaign::launch::{adhoc_campaign_id, default_lock_dir};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_core::termmin::TermminSpec;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

fn sampler() -> Arc<Sampler> {
    Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(200),
            collect_foreign_pinning: false,
            foreign_pin_max_cpus: usize::MAX,
        })
        .unwrap(),
    )
}

/// This process's ad-hoc id lock, held by a separate open file description (every `RealLauncher::new` of this process
/// is then refused with `IdLock(InUse)`).
fn hold_own_adhoc_lock() -> std::fs::File {
    let dir = default_lock_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(dir.join(format!("{}.lock", adhoc_campaign_id())))
        .unwrap();
    f.try_lock().unwrap();
    f
}

fn unique_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "rr-pf19-callers-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn config(dir: &Path) -> DirectConfig {
    let family = dir.join("family.txt");
    std::fs::write(&family, b"family pf19\n").unwrap();
    DirectConfig {
        node: "pf19".into(),
        import_spec: dir.join("import.json"),
        import_base: dir.to_path_buf(),
        extra_solved: vec![],
        terminals: None,
        spec: TermminSpec::parse("{}").unwrap(),
        program: Program::from_file(
            "rr_fake_child",
            Path::new(env!("CARGO_BIN_EXE_rr_fake_child")),
            Invocation::Contract,
        )
        .unwrap(),
        family,
        order: "x".into(),
        cpus: vec![],
        work_root: dir.join("work"),
        out_dir: dir.join("out"),
        check_threads: 1,
        licence_env: None,
        wall_cap_s: 30,
        verbose: false,
    }
}

#[test]
fn termmin_direct_refuses_when_its_launcher_cannot_hold_its_id() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: builds launchers: only inside a private pid namespace (rule L10)");
        return;
    }
    let dir = unique_dir();
    let cfg = config(&dir);
    let recipe = dir.join("recipe.json");
    std::fs::write(&recipe, b"{}\n").unwrap();
    let held = hold_own_adhoc_lock();
    let r = solve_recipe_once(&cfg, &recipe, &[1, 1, 1], "pf19", 0, sampler());
    eprintln!("PF19C termmin direct, lock held: {r:?}");
    let e = r.expect_err("a refused launcher must fail the solve");
    assert!(
        e.contains("launcher refused") && e.contains("in use"),
        "{e}"
    );
    // no job was started: the solve's work directory holds no attempt
    let work = cfg.work_root.join("pf19");
    let entries: Vec<_> = std::fs::read_dir(&work)
        .map(|rd| rd.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(entries.is_empty(), "a job directory was made: {entries:?}");
    drop(held);
    // the lock free: the same call gets past the launcher (it may fail later, never as the launcher's refusal)
    let r = solve_recipe_once(&cfg, &recipe, &[1, 1, 1], "pf19b", 0, sampler());
    eprintln!("PF19C termmin direct, lock free: {r:?}");
    if let Err(e) = &r {
        assert!(!e.contains("launcher refused"), "{e}");
    }
}
