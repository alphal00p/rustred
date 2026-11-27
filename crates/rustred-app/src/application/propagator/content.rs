//! The content digest of a p-library package: `p-content.json` beside
//! `library.json` (schema [`P_CONTENT_SCHEMA`]).
//!
//! The library identity (`PLibrary::identity`) names versions, the scope and
//! an optional free-form revision label, so two packages with different
//! tables can carry the same identity. The content digest names the content:
//! two packages have the same digest exactly when the files below are
//! byte-identical (`library.json` in its canonical form).
//!
//! # Definition (version 1)
//!
//! The covered files, by `/`-separated path relative to the package directory:
//! - `library.json` in its canonical form: the JSON value without the object
//!   `producer` and with a trailing `,P_PACKAGE_REVISION=<label>` removed from
//!   `identity`, written compactly with every object's keys sorted by their
//!   bytes ([`canonical_library_json`]);
//! - the families file (`files.families`) and every family file
//!   (`files.family_files[].file`): the tops and the families of every
//!   presentation (its slots, ids and order are in `library.json`);
//! - the topology (`files.topology`, the DAG) and the census
//!   (`files.census`: sectors, classes, zero proofs);
//! - `files.layer`, `files.ordering`, `files.routes`, `files.orbits` when
//!   listed (basis layer, rule orderings, saved routes, saved orbits);
//! - every owner payload (`owners[].payload.file`), as stored (its encoding is
//!   recorded in `library.json`): the owner tables;
//! - `open-nodes.json` when present (read by the loader);
//! - every regular file under `relations/` (the explicit key expressions a
//!   package ships; vakint hands them to RustRed's relation lookup).
//!
//! Each covered file `f` gives the line `<path>\t<bytes>\t<blake3 hex of its
//! bytes>`; the lines are sorted by path (bytes) and the digest is
//! `blake3:` + the blake3 hex of `rustred.p-content.v1\n` followed by every
//! line with a newline. `p-content.json` lists the lines as records, so a
//! verification names the files that differ.
//!
//! Not covered: the producer record (which program and Symbolica build wrote
//! the package: equal content written by two builds has one digest); the
//! revision label (a name chosen at assembly, `RUSTRED_P_PACKAGE_REVISION`);
//! `census.json` (a JSON rendering of the census for people, never read by
//! the loader; absent from published packages); `p-master-catalogue.json`
//! and `masters/` (vakint's master values, written by the release step and
//! neither read nor written by RustRed: consumers bind them separately);
//! `p-content.json` itself, `SHA256SUMS`, READMEs and every other file.
//! A package re-encoded (other payload encoding, other Symbolica state
//! encoding of a family file) has another digest although it holds the same
//! tables: the digest errs on the side of "different".
//!
//! # Where it is written and checked
//!
//! The package writer (`write_p_library`, `rustred p-package`) adds
//! `p-content.json`; `ordering::{write_ordering, write_routes, write_orbits}`
//! refresh it when the package has one. `rustred p-content` computes the
//! digest of any package (one without a manifest too), writes the manifest
//! and verifies a package against it. The loader checks the manifest only
//! when asked ([`PContentCheck`], `RUSTRED_P_CHECK_CONTENT`; off by default:
//! a package without the manifest, and every package with the check off,
//! loads exactly as before) and records the digest for
//! `PLibrary::content_digest`.
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rustred::propagator::error::PError;
use rustred::propagator::library::{P_PACKAGE_REVISION_KEY, PContentDigest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::PLibrarySource;
use super::format::{LIBRARY_FILE, malformed};
use super::loader::OPEN_NODES_FILE;

/// File name of the content manifest inside a package (beside `library.json`).
pub const P_CONTENT_FILE: &str = "p-content.json";
/// Schema of [`PContentManifest`], also the first line of the digested text.
pub const P_CONTENT_SCHEMA: &str = "rustred.p-content.v1";
/// The loader's content check ([`PContentCheck::from_env`]): unset, empty,
/// `0` or `off` = off (the default); `1`, `on` or `present` = check the
/// manifest when the package has one; `require` = also refuse a package
/// without one.
pub const P_CONTENT_CHECK_ENV: &str = "RUSTRED_P_CHECK_CONTENT";
/// The directory of a package's explicit key expressions.
pub const RELATIONS_DIRECTORY: &str = "relations";

/// One covered file: its path, length and blake3 digest (hex).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PContentFile {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

/// `p-content.json`: the digest and the covered files in canonical order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PContentManifest {
    pub schema: String,
    pub digest: String,
    pub files: Vec<PContentFile>,
}

impl PContentManifest {
    /// The manifest of `files` (any order; sorted by path here).
    pub fn of(mut files: Vec<PContentFile>) -> Result<Self, PError> {
        files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
        if let Some(pair) = files.windows(2).find(|pair| pair[0].path == pair[1].path) {
            return Err(malformed(format!(
                "{P_CONTENT_FILE}: {} is listed twice",
                pair[0].path
            )));
        }
        for file in &files {
            check_path(&file.path)?;
        }
        Ok(Self {
            schema: P_CONTENT_SCHEMA.to_owned(),
            digest: digest_of(&files),
            files,
        })
    }

    /// The manifest file's bytes (pretty JSON and a newline).
    pub fn to_json(&self) -> Result<Vec<u8>, PError> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| malformed(format!("{P_CONTENT_FILE}: {error}")))?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Parse a manifest file: the schema, and the digest of its own records.
    pub fn parse(bytes: &[u8]) -> Result<Self, PError> {
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|error| malformed(format!("{P_CONTENT_FILE}: {error}")))?;
        if manifest.schema != P_CONTENT_SCHEMA {
            return Err(PError::VersionMismatch {
                name: "P_CONTENT_SCHEMA".to_owned(),
                found: manifest.schema,
                expected: P_CONTENT_SCHEMA.to_owned(),
            });
        }
        let rebuilt = Self::of(manifest.files.clone())?;
        if rebuilt.files != manifest.files || rebuilt.digest != manifest.digest {
            return Err(malformed(format!(
                "{P_CONTENT_FILE}: the digest {} is not the digest of its own records ({})",
                manifest.digest, rebuilt.digest
            )));
        }
        Ok(manifest)
    }

    /// The covered bytes.
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }
}

/// The digest of the canonical records `files` (sorted, distinct paths).
fn digest_of(files: &[PContentFile]) -> String {
    let mut text = String::with_capacity(64 + 128 * files.len());
    text.push_str(P_CONTENT_SCHEMA);
    text.push('\n');
    for file in files {
        let _ = writeln!(text, "{}\t{}\t{}", file.path, file.bytes, file.blake3);
    }
    format!("blake3:{}", blake3::hash(text.as_bytes()).to_hex())
}

/// A covered path: relative, `/`-separated, no `.`/`..` part, no tab or newline.
fn check_path(path: &str) -> Result<(), PError> {
    let bad = path.is_empty()
        || path.starts_with('/')
        || path.contains(['\t', '\n', '\r', '\\'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if bad {
        return Err(malformed(format!(
            "{P_CONTENT_FILE}: {path:?} is no package-relative path"
        )));
    }
    Ok(())
}

/// The canonical form of `library.json` (module docs): without `producer`,
/// without the revision label, compact, keys sorted.
pub fn canonical_library_json(bytes: &[u8]) -> Result<Vec<u8>, PError> {
    let mut value: Value = serde_json::from_slice(bytes)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| malformed(format!("{LIBRARY_FILE} is not an object")))?;
    object.remove("producer");
    if let Some(Value::String(identity)) = object.get_mut("identity")
        && let Some(at) = identity.rfind(&format!(",{P_PACKAGE_REVISION_KEY}="))
    {
        identity.truncate(at);
    }
    let mut out = Vec::with_capacity(bytes.len());
    write_canonical(&value, &mut out);
    Ok(out)
}

/// Compact JSON with object keys sorted by their bytes, whatever the map
/// order of `serde_json` in this build (`preserve_order` may be on in a
/// dependent workspace).
fn write_canonical(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            out.push(b'{');
            for (position, (key, item)) in entries.into_iter().enumerate() {
                if position > 0 {
                    out.push(b',');
                }
                write_canonical(&Value::String(key.clone()), out);
                out.push(b':');
                write_canonical(item, out);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        // Scalars: serde_json's own compact form (strings escaped, numbers
        // as parsed).
        scalar => out.extend_from_slice(scalar.to_string().as_bytes()),
    }
}

/// The files `library.json` names (module docs), without `library.json`.
fn listed_files(library: &[u8]) -> Result<Vec<String>, PError> {
    let value: Value = serde_json::from_slice(library)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    let text = |value: &Value, what: &str| -> Result<String, PError> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| malformed(format!("{LIBRARY_FILE}: {what} is not a file name")))
    };
    let files = value
        .get("files")
        .ok_or_else(|| malformed(format!("{LIBRARY_FILE}: no files")))?;
    let mut out = Vec::new();
    for what in ["families", "topology", "census"] {
        out.push(text(&files[what], what)?);
    }
    for what in ["layer", "ordering", "routes", "orbits"] {
        match files.get(what) {
            None | Some(Value::Null) => {}
            Some(file) => out.push(text(file, what)?),
        }
    }
    let family_files = files["family_files"]
        .as_array()
        .ok_or_else(|| malformed(format!("{LIBRARY_FILE}: no family_files")))?;
    for record in family_files {
        out.push(text(&record["file"], "a family file")?);
    }
    let owners = value["owners"]
        .as_array()
        .ok_or_else(|| malformed(format!("{LIBRARY_FILE}: no owners")))?;
    for owner in owners {
        match owner.get("payload") {
            None | Some(Value::Null) => {}
            Some(payload) => out.push(text(&payload["file"], "an owner payload")?),
        }
    }
    Ok(out)
}

/// The manifest of a package whose `library.json` has the bytes `library`:
/// `names` are the files the package holds (for `open-nodes.json` and
/// `relations/`), `hash` gives a covered file's length and digest.
fn manifest_from(
    library: &[u8],
    names: &[String],
    hash: impl Fn(&str) -> Result<(u64, String), PError> + Sync,
) -> Result<PContentManifest, PError> {
    let mut paths = listed_files(library)?;
    let relations = format!("{RELATIONS_DIRECTORY}/");
    paths.extend(
        names
            .iter()
            .filter(|name| *name == OPEN_NODES_FILE || name.starts_with(&relations))
            .cloned(),
    );
    paths.sort();
    paths.dedup();
    paths.retain(|path| path != LIBRARY_FILE);
    let mut files: Vec<PContentFile> = paths
        .par_iter()
        .map(|path| {
            check_path(path)?;
            let (bytes, blake3) = hash(path)?;
            Ok(PContentFile {
                path: path.clone(),
                bytes,
                blake3,
            })
        })
        .collect::<Result<_, PError>>()?;
    let canonical = canonical_library_json(library)?;
    files.push(PContentFile {
        path: LIBRARY_FILE.to_owned(),
        bytes: canonical.len() as u64,
        blake3: blake3::hash(&canonical).to_hex().to_string(),
    });
    PContentManifest::of(files)
}

/// The manifest of the rendered files of a package (the writer, before the
/// files are written): `stored` gives the length and digest of the payload
/// files written separately (`write_p_library`).
pub(crate) fn manifest_of_rendered(
    files: &BTreeMap<String, Vec<u8>>,
    stored: &BTreeMap<String, (u64, String)>,
) -> Result<PContentManifest, PError> {
    let library = files
        .get(LIBRARY_FILE)
        .ok_or_else(|| malformed(format!("a package without {LIBRARY_FILE}")))?;
    let names: Vec<String> = files.keys().chain(stored.keys()).cloned().collect();
    manifest_from(library, &names, |path| match files.get(path) {
        Some(bytes) => Ok((bytes.len() as u64, blake3::hash(bytes).to_hex().to_string())),
        None => stored
            .get(path)
            .cloned()
            .ok_or_else(|| malformed(format!("{LIBRARY_FILE} names {path}, which is not written"))),
    })
}

/// Length and blake3 digest (hex) of `bytes`.
pub(crate) fn hash_bytes(bytes: &[u8]) -> (u64, String) {
    (bytes.len() as u64, blake3::hash(bytes).to_hex().to_string())
}

/// Length and blake3 digest (hex) of the file at `path`, streamed.
fn hash_file(path: &Path) -> Result<(u64, String), PError> {
    let io = |error: std::io::Error| malformed(format!("{}: {error}", path.display()));
    let mut file = std::fs::File::open(path).map_err(io)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    Ok((total, hasher.finalize().to_hex().to_string()))
}

/// Every regular file under `directory/relations`, by package-relative path.
fn relations_files(directory: &Path) -> Result<Vec<String>, PError> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<String>) -> Result<(), PError> {
        let io = |error: std::io::Error| malformed(format!("{}: {error}", at.display()));
        for entry in std::fs::read_dir(at).map_err(io)? {
            let path = entry.map_err(io)?.path();
            let metadata = std::fs::metadata(&path).map_err(io)?;
            if metadata.is_dir() {
                walk(root, &path, out)?;
            } else if metadata.is_file() {
                let relative = path.strip_prefix(root).map_err(|_| {
                    malformed(format!("{} is outside {}", path.display(), root.display()))
                })?;
                let parts: Option<Vec<&str>> = relative
                    .components()
                    .map(|part| part.as_os_str().to_str())
                    .collect();
                let parts = parts.ok_or_else(|| {
                    malformed(format!("{}: a file name that is not UTF-8", path.display()))
                })?;
                out.push(parts.join("/"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    let relations = directory.join(RELATIONS_DIRECTORY);
    if relations.is_dir() {
        walk(directory, &relations, &mut out)?;
    }
    Ok(out)
}

/// The content manifest of the package `source` computed from its files
/// (whether or not it has a `p-content.json`). Files are hashed in parallel
/// on the current rayon pool.
pub fn compute_p_content(source: &PLibrarySource) -> Result<PContentManifest, PError> {
    match source {
        PLibrarySource::Directory(directory) => {
            let path = directory.join(LIBRARY_FILE);
            let library = std::fs::read(&path)
                .map_err(|error| malformed(format!("{}: {error}", path.display())))?;
            let mut names = relations_files(directory)?;
            if directory.join(OPEN_NODES_FILE).is_file() {
                names.push(OPEN_NODES_FILE.to_owned());
            }
            manifest_from(&library, &names, |path| hash_file(&directory.join(path)))
        }
        PLibrarySource::Memory(files) => {
            let map: BTreeMap<&str, &[u8]> = files.iter().copied().collect();
            let library = map
                .get(LIBRARY_FILE)
                .ok_or_else(|| malformed(format!("no {LIBRARY_FILE} in memory")))?;
            let names: Vec<String> = map.keys().map(|name| (*name).to_owned()).collect();
            manifest_from(library, &names, |path| {
                map.get(path)
                    .map(|bytes| hash_bytes(bytes))
                    .ok_or_else(|| malformed(format!("{LIBRARY_FILE} names {path}, not in memory")))
            })
        }
    }
}

/// The package's `p-content.json`, `None` when it has none.
pub fn read_p_content(source: &PLibrarySource) -> Result<Option<PContentManifest>, PError> {
    let bytes = match source {
        PLibrarySource::Directory(directory) => {
            let path = directory.join(P_CONTENT_FILE);
            match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(malformed(format!("{}: {error}", path.display()))),
            }
        }
        PLibrarySource::Memory(files) => {
            match files.iter().find(|(name, _)| *name == P_CONTENT_FILE) {
                Some((_, bytes)) => bytes.to_vec(),
                None => return Ok(None),
            }
        }
    };
    PContentManifest::parse(&bytes).map(Some)
}

/// Where `computed` (from the files) differs from `recorded` (the manifest),
/// one line per file; empty when they are equal.
pub fn p_content_differences(
    recorded: &PContentManifest,
    computed: &PContentManifest,
) -> Vec<String> {
    let by_path = |manifest: &PContentManifest| -> BTreeMap<String, PContentFile> {
        (manifest.files.iter())
            .map(|file| (file.path.clone(), file.clone()))
            .collect()
    };
    let (recorded_files, computed_files) = (by_path(recorded), by_path(computed));
    let mut out = Vec::new();
    for (path, file) in &recorded_files {
        match computed_files.get(path) {
            None => out.push(format!(
                "{path}: in the manifest, not covered by the package"
            )),
            Some(found) if found != file => out.push(format!(
                "{path}: {} bytes {} in the manifest, {} bytes {} in the package",
                file.bytes, file.blake3, found.bytes, found.blake3
            )),
            Some(_) => {}
        }
    }
    for path in computed_files.keys() {
        if !recorded_files.contains_key(path) {
            out.push(format!(
                "{path}: covered by the package, not in the manifest"
            ));
        }
    }
    if out.is_empty() && recorded.digest != computed.digest {
        out.push(format!(
            "digest {} in the manifest, {} computed",
            recorded.digest, computed.digest
        ));
    }
    out
}

/// The loader's content check ([`super::loader::load_p_library_checked`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PContentCheck {
    /// No check (the default): the digest a `p-content.json` records is
    /// noted for `PLibrary::content_digest` (unverified) when it parses.
    #[default]
    Off,
    /// When the package has a `p-content.json`, recompute the digest from the
    /// files and refuse the package unless it is the recorded one.
    WhenPresent,
    /// [`Self::WhenPresent`], and refuse a package without `p-content.json`.
    Required,
}

impl PContentCheck {
    /// The check [`P_CONTENT_CHECK_ENV`] asks for (off when unset).
    pub fn from_env() -> Result<Self, PError> {
        match std::env::var(P_CONTENT_CHECK_ENV) {
            Err(std::env::VarError::NotPresent) => Ok(Self::Off),
            Ok(value) => match value.trim() {
                "" | "0" | "off" => Ok(Self::Off),
                "1" | "on" | "present" => Ok(Self::WhenPresent),
                "require" => Ok(Self::Required),
                other => Err(PError::Unsupported(format!(
                    "{P_CONTENT_CHECK_ENV}={other}: off, on (or present) or require"
                ))),
            },
            Err(error) => Err(PError::Unsupported(format!(
                "{P_CONTENT_CHECK_ENV}: {error}"
            ))),
        }
    }

    /// The content digest of `source` under this check: `None` when there is
    /// nothing to record; an error when the check refuses the package.
    pub(crate) fn record(self, source: &PLibrarySource) -> Result<Option<PContentDigest>, PError> {
        match self {
            // Off: never fails a load; a manifest that does not parse is
            // simply not recorded.
            Self::Off => Ok(read_p_content(source)
                .ok()
                .flatten()
                .map(|manifest| PContentDigest {
                    digest: manifest.digest,
                    verified: false,
                })),
            Self::WhenPresent | Self::Required => {
                let Some(recorded) = read_p_content(source)? else {
                    return if self == Self::Required {
                        Err(malformed(format!(
                            "the package has no {P_CONTENT_FILE} and {P_CONTENT_CHECK_ENV}=require"
                        )))
                    } else {
                        Ok(None)
                    };
                };
                let computed = compute_p_content(source)?;
                let differences = p_content_differences(&recorded, &computed);
                if !differences.is_empty() {
                    let shown: Vec<&str> = differences.iter().take(8).map(String::as_str).collect();
                    return Err(malformed(format!(
                        "the package's files do not have the content {P_CONTENT_FILE} records \
                         ({} differences): {}",
                        differences.len(),
                        shown.join("; ")
                    )));
                }
                Ok(Some(PContentDigest {
                    digest: computed.digest,
                    verified: true,
                }))
            }
        }
    }
}

/// Rewrite `p-content.json` of the package in `directory` from its files when
/// it has one (a package rewritten in place: ordering, routes, orbits); a
/// package without one is left without one. `Some(manifest)` when rewritten.
pub fn refresh_p_content_if_present(directory: &Path) -> Result<Option<PContentManifest>, PError> {
    let path: PathBuf = directory.join(P_CONTENT_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let manifest = compute_p_content(&PLibrarySource::Directory(directory.to_owned()))?;
    std::fs::write(&path, manifest.to_json()?)
        .map_err(|error| malformed(format!("{}: {error}", path.display())))?;
    Ok(Some(manifest))
}
