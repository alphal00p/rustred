//! Digest this crate's sources for the Ordered step-cache bindings it computes.
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Collect every Rust source below `directory` as (`label`-relative name, path).
fn collect_sources(directory: &Path, label: &str, out: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("source directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| panic!("non-UTF-8 source path {}", path.display()));
        let child = format!("{label}/{name}");
        if path.is_dir() {
            collect_sources(&path, &child, out);
        } else if name.ends_with(".rs") {
            out.push((child, path));
        }
    }
}

/// BLAKE3 digest of every Rust source of this crate. Its loaders and route
/// preparation turn saved files into the programs whose steps an Ordered step
/// cache records, so the program digests this crate computes include it.
fn main() {
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest.join("src");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect_sources(&root, "rustred-app/src", &mut files);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"rustred-app.ordered-step-sources.v1\n");
    for (name, path) in files {
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        hasher.update(&(name.len() as u64).to_le_bytes());
        hasher.update(name.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    println!(
        "cargo:rustc-env=RUSTRED_APP_SOURCES={}",
        hasher.finalize().to_hex()
    );
}
