//! Shared helpers of the `adapters_*` tests (lane adapters): a real launcher (own session, PDEATHSIG, marker, verified
//! kill, hunt) with a sampler, temporary roots, fake scripts of kind solve-recipe. Every test that launches anything
//! runs under `LIGHT_PIDNS=1` (private pid namespace, rule L10).
#![allow(dead_code)]

use rustred_campaign::adapters::job::{InputFile, Invocation, Program};
use rustred_campaign::launch::RealLauncher;
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::classify::recipe::Verdict;
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RecipeQuirk, RssPlan};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");

/// One launching test at a time in a test binary.
pub fn serial() -> MutexGuard<'static, ()> {
    static M: Mutex<()> = Mutex::new(());
    M.lock().unwrap_or_else(|p| p.into_inner())
}

/// Refuse to launch outside a private pid namespace (L10): the tests are meant for `LIGHT_PIDNS=1`.
pub fn require_namespace() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run under LIGHT_PIDNS=1 (private pid namespace, rule L10)"
    );
}

static N: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary root (removed on drop unless RR_KEEP is set).
pub struct Root(pub PathBuf);

impl Root {
    pub fn new(tag: &str) -> Root {
        let base = std::env::var("RR_ADAPTERS_OUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let p = base.join(format!(
            "rr-adapters-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Root(p)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
    /// Write a file under the root and return it as an input.
    pub fn file(&self, name: &str, bytes: &[u8]) -> InputFile {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        InputFile::from_file(&p).unwrap()
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        if std::env::var_os("RR_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

pub fn launcher() -> RealLauncher {
    let s = Sampler::spawn(SamplerConfig {
        interval: Duration::from_millis(200),
        collect_foreign_pinning: false,
        foreign_pin_max_cpus: usize::MAX,
    })
    .unwrap();
    RealLauncher::new(Arc::new(s)).expect("launcher (procfix7 round 19: fail-closed)")
}

/// The fake child as a contract program.
pub fn fake_program() -> Program {
    let mut p = Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract).unwrap();
    p.features_required = vec!["fake".into()];
    p
}

/// A fake script of kind solve-recipe: prepare, one `relation` op per verdict (SOLVED gets one rhs key one lower in
/// slot 0), end; plus quirks.
pub fn recipe_script(verdicts: &[Verdict], key: &[i64], quirks: &[RecipeQuirk]) -> FakeScript {
    let mut ops = vec![FakeOp::Prepare { ms: 20 }];
    for q in quirks {
        ops.push(FakeOp::RecipeQuirk { quirk: *q });
    }
    for v in verdicts {
        let mut lower = key.to_vec();
        lower[0] -= 1;
        ops.push(FakeOp::Relation {
            ms: 30,
            verdict: *v,
            rhs: if *v == Verdict::Solved {
                vec![lower]
            } else {
                vec![]
            },
        });
    }
    ops.push(FakeOp::End {
        stop: None,
        verdict: None,
    });
    FakeScript {
        script: ops,
        on_sigterm: OnSigterm::Graceful,
        slow_ms: 0,
        start_delay_ms: 0,
        rss: RssPlan::default(),
        helpers: vec![],
        faults: vec![],
        time_scale: 1.0,
        volatile_jitter: true,
    }
}
