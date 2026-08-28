use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR"),
    );
    let symbolica_manifest_path = manifest_dir.join("vendor/symbolica/Cargo.toml");
    println!(
        "cargo:rerun-if-changed={}",
        symbolica_manifest_path.display()
    );

    let source = fs::read_to_string(&symbolica_manifest_path).unwrap_or_else(|error| {
        panic!(
            "cannot read vendored Symbolica manifest {}: {error}",
            symbolica_manifest_path.display()
        )
    });
    let manifest: toml::Value = toml::from_str(&source).unwrap_or_else(|error| {
        panic!(
            "cannot parse vendored Symbolica manifest {}: {error}",
            symbolica_manifest_path.display()
        )
    });
    let version = manifest
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "vendored Symbolica manifest {} has no string package.version",
                symbolica_manifest_path.display()
            )
        });
    println!("cargo:rustc-env=RUSTRED_SYMBOLICA_PACKAGE_VERSION={version}");
}
