//! Shared test-directory lifetime, independent of the optional CLI parser.
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct Scratch(pub PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os("RUSTRED_E2E_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
pub(super) fn scratch(label: &str) -> Scratch {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "rustred-verify-closure-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    Scratch(path)
}
