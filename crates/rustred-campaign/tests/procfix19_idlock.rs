//! Lane procfix7 round 19: `RealLauncher::new` (the process's own AD-HOC campaign id; one-off jobs of the termmin direct
//! path, and on the int line coreseed / seedmin / shipecon / symcert / polereport) used to run WITHOUT the host-wide id lock
//! when the lock could not be taken, silently (`with_config`'s fallback: the same ad-hoc id again, `acquire(..).ok()`).
//! Now FAIL-CLOSED: a typed `LauncherError`, nothing started. Both outcomes:
//! - refused: the lock directory unusable (group / other writable) -> `IdLock(Unusable)`; the lock file held by another
//!   open file description -> `IdLock(InUse)`; an ad-hoc id with an unread part (`adhoc-0-..`, `..-0`) -> `AdhocId`;
//! - accepted: the launcher holds `<default lock dir>/<its ad-hoc id>.lock`.
//!
//! Builds launchers (child subreaper, reaper thread): ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh`,
//! rule L10). ONE test function: the id-lock registry is process-wide.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::launch::{
    CapPreference, IdLockError, LauncherError, RealConfig, RealLauncher, adhoc_campaign_id,
    adhoc_id_proven_unique, default_lock_dir,
};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
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

fn what(r: &Result<RealLauncher, LauncherError>) -> String {
    match r {
        Ok(l) => format!("Ok({} lock {:?})", l.config().campaign_id, l.id_lock_path()),
        Err(e) => format!("Err({e})"),
    }
}

#[test]
fn an_adhoc_launcher_that_cannot_hold_its_id_is_refused_never_run_without_its_lock() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: builds launchers: only inside a private pid namespace (rule L10)");
        return;
    }
    // the ad-hoc id names this process only when every part was read
    let id = adhoc_campaign_id();
    assert!(adhoc_id_proven_unique(&id).is_ok(), "{id}");
    for bad in [
        "adhoc-0-6-0",
        "adhoc-0-6-308326982",
        "adhoc-4026538754-6-0",
        "adhoc-4026538754-0-1",
        "adhoc-x-6-1",
        "adhoc-1-2",
    ] {
        assert!(adhoc_id_proven_unique(bad).is_err(), "{bad}");
    }
    assert!(adhoc_id_proven_unique("rbcamp").is_ok());
    let r = RealLauncher::try_with_config(
        sampler(),
        RealConfig {
            campaign_id: "adhoc-0-6-0".into(),
            cap_preference: CapPreference::NoSystemd,
            ..RealConfig::default()
        },
    );
    eprintln!("PF19 unread-part id: {}", what(&r));
    assert!(matches!(r, Err(LauncherError::AdhocId(_))), "{}", what(&r));

    // REFUSED (a): the lock directory is unusable
    let dir = default_lock_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    let r = RealLauncher::new(sampler());
    eprintln!("PF19 unusable dir: {}", what(&r));
    assert!(
        matches!(&r, Err(LauncherError::IdLock(IdLockError::Unusable { .. }))),
        "{}",
        what(&r)
    );
    drop(r);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();

    // REFUSED (b): another holder of this id's lock (a separate open file description)
    let path = dir.join(format!("{id}.lock"));
    let holder = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .unwrap();
    holder.try_lock().unwrap();
    let r = RealLauncher::new(sampler());
    eprintln!("PF19 lock held: {}", what(&r));
    assert!(
        matches!(&r, Err(LauncherError::IdLock(IdLockError::InUse { .. }))),
        "{}",
        what(&r)
    );
    drop(r);
    holder.unlock().unwrap();
    drop(holder);

    // ACCEPTED: the launcher holds its own lock file
    let r = RealLauncher::new(sampler());
    eprintln!("PF19 accepted: {}", what(&r));
    let l = r.expect("the ad-hoc launcher");
    assert_eq!(l.config().campaign_id, id);
    assert_eq!(l.id_lock_path(), Some(path.as_path()));
    assert_eq!(l.setup().id_lock.as_deref(), Some(path.as_path()));
    // while it lives, a second holder of the same file cannot lock it (the launcher really holds it)
    let probe = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    assert!(probe.try_lock().is_err(), "the launcher holds {path:?}");
}
