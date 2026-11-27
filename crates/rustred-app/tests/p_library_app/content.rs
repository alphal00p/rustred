//! The content digest of a package (`p-content.json`,
//! `rustred_app::propagator::content`): written with every package, the same
//! for equal content, different for different tables, checked by the loader
//! only when asked, refreshed by the in-place writers; and the
//! `rustred p-content` command (digest, verification, manifest, content keys).
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rustred::propagator::error::PError;
use rustred::propagator::library::{PContentDigest, PLibrary};
use rustred_app::propagator::content::{
    P_CONTENT_FILE, P_CONTENT_SCHEMA, PContentCheck, PContentFile, PContentManifest,
    canonical_library_json, compute_p_content, p_content_differences, read_p_content,
};
use rustred_app::propagator::format::LIBRARY_FILE;
use rustred_app::propagator::package::p_library_files;
use rustred_app::propagator::{
    AssembledLibrary, PLibraryOptions, PLibrarySource, load_p_library, load_p_library_checked,
    write_p_library,
};

use super::package::{Scratch, scope_two, tree};

const FAST: PLibraryOptions = PLibraryOptions {
    verify_descent: false,
};

fn directory_of(path: &Path) -> PLibrarySource {
    PLibrarySource::Directory(path.to_path_buf())
}

/// The package of `assembled` written into a scratch directory.
fn written(assembled: &AssembledLibrary, label: &str) -> (Scratch, PathBuf) {
    let scratch = Scratch::new(label);
    let directory = scratch.join("p-library");
    write_p_library(assembled, &directory).unwrap();
    (scratch, directory)
}

fn load_with(directory: &Path, check: PContentCheck) -> Result<std::sync::Arc<PLibrary>, PError> {
    load_p_library_checked(directory_of(directory), &FAST, check)
}

fn computed(directory: &Path) -> PContentManifest {
    compute_p_content(&directory_of(directory)).unwrap()
}

/// Rewrite `library.json` of `directory` through `edit`.
fn edit_library(directory: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let path = directory.join(LIBRARY_FILE);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    edit(&mut value);
    std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

#[test]
fn the_canonical_library_json_drops_producer_and_revision_and_sorts_keys() {
    let a = br#"{"producer": {"name": "x"}, "identity": "p-library:scope=1-2:V=1,P_PACKAGE_REVISION=r7", "b": [1, {"z": 1, "a": "t\n"}], "a": null}"#;
    let b =
        br#"{ "a": null, "b": [1, {"a": "t\n", "z": 1}], "identity": "p-library:scope=1-2:V=1" }"#;
    let canonical = canonical_library_json(a).unwrap();
    assert_eq!(canonical, canonical_library_json(b).unwrap());
    assert_eq!(
        String::from_utf8(canonical).unwrap(),
        r#"{"a":null,"b":[1,{"a":"t\n","z":1}],"identity":"p-library:scope=1-2:V=1"}"#
    );
    assert!(canonical_library_json(b"[1]").is_err());
}

#[test]
fn a_manifest_is_its_own_check() {
    let file = |path: &str, bytes: u64| PContentFile {
        path: path.to_owned(),
        bytes,
        blake3: "00".repeat(32),
    };
    let manifest = PContentManifest::of(vec![file("z", 1), file("a/b", 2)]).unwrap();
    assert_eq!(manifest.schema, P_CONTENT_SCHEMA);
    assert_eq!(manifest.files[0].path, "a/b");
    assert!(manifest.digest.starts_with("blake3:") && manifest.digest.len() == 7 + 64);
    let parsed = PContentManifest::parse(&manifest.to_json().unwrap()).unwrap();
    assert_eq!(parsed, manifest);
    let mut forged = manifest.clone();
    forged.files[1].bytes = 3;
    assert!(PContentManifest::parse(&serde_json::to_vec(&forged).unwrap()).is_err());
    let mut other_schema = manifest.clone();
    other_schema.schema = "rustred.p-content.v0".to_owned();
    assert!(matches!(
        PContentManifest::parse(&serde_json::to_vec(&other_schema).unwrap()),
        Err(PError::VersionMismatch { .. })
    ));
    assert!(PContentManifest::of(vec![file("a", 1), file("a", 1)]).is_err());
    for bad in ["/a", "a/../b", "a\tb", "", "a//b", "./a"] {
        assert!(PContentManifest::of(vec![file(bad, 1)]).is_err(), "{bad:?}");
    }
}

/// Every written package carries `p-content.json`: the digest of the files
/// beside it, the same from the directory, from embedded bytes and from the
/// rendering, recorded by the loader (verified when the check is on).
#[test]
fn a_written_package_carries_its_content_digest() {
    let assembled = scope_two();
    let (_scratch, directory) = written(assembled, "content-written");
    let recorded = read_p_content(&directory_of(&directory)).unwrap().unwrap();
    let from_files = computed(&directory);
    assert_eq!(recorded, from_files);
    assert!(p_content_differences(&recorded, &from_files).is_empty());
    let rendered: BTreeMap<String, Vec<u8>> =
        p_library_files(assembled).unwrap().into_iter().collect();
    assert_eq!(
        PContentManifest::parse(&rendered[P_CONTENT_FILE]).unwrap(),
        recorded
    );
    // Covered: library.json and every file it names; not covered: the
    // manifest itself and census.json.
    let covered: Vec<&str> = recorded.files.iter().map(|f| f.path.as_str()).collect();
    assert!(covered.contains(&LIBRARY_FILE));
    assert!(!covered.contains(&P_CONTENT_FILE));
    assert!(!covered.contains(&"census.json"));
    assert_eq!(covered.len(), rendered.len() - 2, "{covered:?}");
    // From embedded bytes (vakint's built-in packages): the same digest.
    let leaked: Vec<(&'static str, &'static [u8])> = tree(&directory)
        .into_iter()
        .map(|(name, bytes)| {
            let name: &'static str = Box::leak(name.into_boxed_str());
            let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
            (name, bytes)
        })
        .collect();
    let memory = PLibrarySource::Memory(Box::leak(leaked.into_boxed_slice()));
    assert_eq!(compute_p_content(&memory).unwrap(), recorded);
    // The loader records it: unverified with the check off, verified with it.
    let off = load_with(&directory, PContentCheck::Off).unwrap();
    assert_eq!(
        off.content_digest(),
        Some(&PContentDigest {
            digest: recorded.digest.clone(),
            verified: false
        })
    );
    let checked = load_with(&directory, PContentCheck::WhenPresent).unwrap();
    assert_eq!(
        checked.content_digest(),
        Some(&PContentDigest {
            digest: recorded.digest.clone(),
            verified: true
        })
    );
    assert_eq!(checked.identity(), off.identity());
    // Rendering twice gives the same manifest (deterministic writer).
    assert_eq!(
        p_library_files(assembled).unwrap(),
        rendered.into_iter().collect::<Vec<_>>()
    );
}

/// The producer record and the revision label are no content; a changed
/// table, family, relation or open-nodes sidecar is. The loader refuses a
/// package whose files differ from its manifest only when the check is on.
#[test]
fn the_digest_sees_every_table_and_ignores_producer_and_revision() {
    let (_scratch, directory) = written(scope_two(), "content-change");
    let base = computed(&directory);

    // Producer and revision: same digest, the check passes.
    edit_library(&directory, |value| {
        value["producer"]["symbolica_version"] = "another-build".into();
        let identity = value["identity"].as_str().unwrap().to_owned();
        value["identity"] = format!("{identity},P_PACKAGE_REVISION=sk-test").into();
    });
    assert_eq!(computed(&directory).digest, base.digest);
    let revised = load_with(&directory, PContentCheck::WhenPresent).unwrap();
    assert_eq!(revised.revision(), Some("sk-test"));
    assert!(revised.content_digest().unwrap().verified);

    // A shipped relation (explicit key expressions): another digest, named.
    let relations = directory.join("relations/P2L-0000000000000000");
    std::fs::create_dir_all(&relations).unwrap();
    std::fs::write(relations.join("NODE.json"), br#"{"node": "P2L"}"#).unwrap();
    let with_relation = computed(&directory);
    assert_ne!(with_relation.digest, base.digest);
    let differences = p_content_differences(&base, &with_relation);
    assert_eq!(
        differences,
        ["relations/P2L-0000000000000000/NODE.json: covered by the package, not in the manifest"]
    );
    // Off: loads, records the manifest's (stale) digest unverified.
    let off = load_with(&directory, PContentCheck::Off).unwrap();
    assert_eq!(off.content_digest().unwrap().digest, base.digest);
    assert!(!off.content_digest().unwrap().verified);
    match load_with(&directory, PContentCheck::WhenPresent) {
        Err(PError::MalformedLibrary(message)) => {
            assert!(
                message.contains("relations/P2L-0000000000000000/NODE.json"),
                "{message}"
            )
        }
        other => panic!("a package with an unrecorded relation loaded under the check: {other:?}"),
    }
    std::fs::remove_dir_all(directory.join("relations")).unwrap();

    // A family file: one byte appended, another digest (the loader itself
    // would refuse the family too; the digest names it first).
    let family = base
        .files
        .iter()
        .find(|f| f.path.starts_with("families/") && f.path.ends_with(".rrbin"))
        .unwrap()
        .path
        .clone();
    let path = directory.join(&family);
    let original = std::fs::read(&path).unwrap();
    let mut changed = original.clone();
    changed.push(0);
    std::fs::write(&path, &changed).unwrap();
    let differences = p_content_differences(&base, &computed(&directory));
    assert_eq!(differences.len(), 1, "{differences:?}");
    assert!(
        differences[0].starts_with(&format!("{family}: ")),
        "{differences:?}"
    );
    std::fs::write(&path, &original).unwrap();

    // A presentation's slot in library.json: another digest.
    edit_library(&directory, |value| {
        value["presentations"][1]["slots"][0]["line"]["eps_units"] = 1.into();
    });
    let differences = p_content_differences(&base, &computed(&directory));
    assert_eq!(differences.len(), 1, "{differences:?}");
    assert!(
        differences[0].starts_with("library.json: "),
        "{differences:?}"
    );
}

/// A package without `p-content.json` (every package written before it)
/// loads exactly as one with it; `Required` refuses it.
#[test]
fn a_package_without_a_manifest_loads_as_before() {
    let (_scratch, directory) = written(scope_two(), "content-absent");
    let with = load_with(&directory, PContentCheck::Off).unwrap();
    std::fs::remove_file(directory.join(P_CONTENT_FILE)).unwrap();
    assert_eq!(read_p_content(&directory_of(&directory)).unwrap(), None);
    for check in [PContentCheck::Off, PContentCheck::WhenPresent] {
        let without = load_with(&directory, check).unwrap();
        assert_eq!(without.content_digest(), None);
        assert_eq!(without.identity(), with.identity());
        assert_eq!(
            rustred_app::propagator::package::p_library_differences(&with, &without).unwrap(),
            Vec::<String>::new()
        );
    }
    // the default entry point (RUSTRED_P_CHECK_CONTENT unset in the tests)
    assert_eq!(
        load_p_library(directory_of(&directory), &FAST)
            .unwrap()
            .content_digest(),
        None
    );
    assert!(matches!(
        load_with(&directory, PContentCheck::Required),
        Err(PError::MalformedLibrary(message)) if message.contains(P_CONTENT_FILE)
    ));
    // The digest of a package without a manifest is computed all the same.
    assert_eq!(
        computed(&directory),
        PContentManifest::parse(
            &p_library_files(scope_two())
                .unwrap()
                .into_iter()
                .collect::<BTreeMap<_, _>>()[P_CONTENT_FILE]
        )
        .unwrap()
    );
}

/// `write_routes` / `write_orbits` / `write_ordering` rewrite library.json in
/// place: they refresh a package's manifest, and add none to a package
/// without one.
#[test]
fn the_in_place_writers_refresh_the_manifest() {
    use rustred_app::propagator::ordering::{write_orbits, write_ordering, write_routes};
    let (_scratch, directory) = written(scope_two(), "content-refresh");
    let before = computed(&directory);
    write_routes(&directory).unwrap();
    write_orbits(&directory).unwrap();
    write_ordering(&directory, "generation").unwrap();
    let after = computed(&directory);
    assert_ne!(after.digest, before.digest);
    assert_eq!(
        read_p_content(&directory_of(&directory)).unwrap(),
        Some(after.clone())
    );
    for file in ["routes.rrbin", "orbits.rrbin", "ordering.json"] {
        assert!(after.files.iter().any(|f| f.path == file), "{file}");
    }
    assert!(load_with(&directory, PContentCheck::WhenPresent).is_ok());

    let (_scratch, bare) = written(scope_two(), "content-refresh-bare");
    std::fs::remove_file(bare.join(P_CONTENT_FILE)).unwrap();
    write_routes(&bare).unwrap();
    assert!(!bare.join(P_CONTENT_FILE).exists());
}

// ---------------------------------------------------------------------------
// rustred p-content
// ---------------------------------------------------------------------------

fn p_content(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rustred"))
        .arg("p-content")
        .args(arguments)
        .output()
        .unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn p_content_digests_verifies_and_writes() {
    for help in ["--help", "-h"] {
        let output = p_content(&[help]);
        assert_eq!(output.status.code(), Some(0));
        assert!(text(&output).contains("EXIT STATUS:"));
    }
    for arguments in [
        &[][..],
        &["--package"],
        &["--package", "p", "--threads", "0"],
        &["--x"],
    ] {
        assert_eq!(p_content(arguments).status.code(), Some(2), "{arguments:?}");
    }
    let scratch = Scratch::new("content-cli");
    assert_eq!(
        p_content(&["--package", scratch.join("none").to_str().unwrap()])
            .status
            .code(),
        Some(3)
    );
    let (_package_scratch, directory) = written(scope_two(), "content-cli-package");
    let package = directory.to_str().unwrap();
    let recorded = read_p_content(&directory_of(&directory)).unwrap().unwrap();

    let output = p_content(&["--package", package, "--verify", "--threads", "2"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(text(&output).contains(&format!("content {}", recorded.digest)));
    assert!(text(&output).contains("agrees"));

    // A difference: exit 1, named.
    std::fs::create_dir_all(directory.join("relations/x")).unwrap();
    std::fs::write(directory.join("relations/x/a.json"), b"{}").unwrap();
    let output = p_content(&["--package", package, "--verify"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(text(&output).contains("relations/x/a.json"));
    // --write refuses the existing manifest, --force replaces it.
    assert_eq!(
        p_content(&["--package", package, "--write"]).status.code(),
        Some(4)
    );
    let output = p_content(&["--package", package, "--write", "--force"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(
        p_content(&["--package", package, "--verify"]).status.code(),
        Some(0)
    );
    // No manifest: --verify is exit 1, the digest is still printed; --out.
    std::fs::remove_file(directory.join(P_CONTENT_FILE)).unwrap();
    let out = scratch.join("manifest.json");
    let output = p_content(&[
        "--package",
        package,
        "--verify",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(text(&output).contains("no p-content.json"));
    assert_eq!(
        PContentManifest::parse(&std::fs::read(&out).unwrap()).unwrap(),
        computed(&directory)
    );
    // --keys: one row per presentation, the loaded library's content keys.
    let keys = scratch.join("keys.tsv");
    let output = p_content(&["--package", package, "--keys", keys.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let rows = std::fs::read_to_string(&keys).unwrap();
    let library = &scope_two().library;
    assert_eq!(rows.lines().count(), 1 + library.presentations().len());
    for (row, presentation) in rows.lines().skip(1).zip(library.presentations()) {
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields[0], presentation.id.0.to_string());
        assert_eq!(fields[5], presentation.content_key().digest());
    }
}
