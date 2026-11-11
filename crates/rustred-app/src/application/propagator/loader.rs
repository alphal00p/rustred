//! The fail-closed p-library loader (owner: M2-T8b rr-package).
//!
//! [`load_p_library`] rebuilds a library program from a package of
//! [`super::format`], from a directory or from embedded files, in this order;
//! the first failure is the answer, a typed [`PError`]:
//!
//! 1. `library.json`: the schema, then every `p_versions()` entry (no entry
//!    missing, none unknown), then the Symbolica save format, each a
//!    [`PError::VersionMismatch`] whose message says to regenerate with
//!    `rustred p-package`; then the whole manifest, unknown fields refused;
//! 2. the families file and every family file (length, save format, name),
//!    each presentation validated against its family (`Presentation::validate`:
//!    the slots of `library.json` are the family's denominators and shifts), and
//!    the tops against the families file (names, slots, family fingerprint);
//! 3. the DAG (read, never recomputed), the census from `census.rrbin`, equal to
//!    the one of `census.json`;
//! 4. edge maps (verified by the builder), terminals in id order, owners: the
//!    hard guard `lp_count >= 1 => residuals >= 1` is checked before a payload
//!    file is read (the stored LP is trusted: it is not re-derived at load;
//!    `rustred p-package --recount-lp` recounts it when a package is written,
//!    M3-I1); a payload must have this build's save format, the structure
//!    its record describes (charts, residual count, two-sided axes), the owner's
//!    residual terminals as its residual keys and the owner presentation's
//!    family (`custom::inspect_custom_payload`, structure only);
//! 5. the basis layer `layer/p-layer.json` when listed (M2-T9): its schema and
//!    bound versions, then its rows; set as the program's basis map;
//! 6. the rule ordering `ordering.json` when listed (p artefact v2,
//!    [`super::ordering`]): schema and version, every owner once in class
//!    order; set on the program (its tag joins the identity);
//!    the saved route verifications `routes.rrbin` when listed and the load
//!    does not verify (trusted restore on a sector's first use, else verified
//!    as before; not part of the identity);
//!    `PLibraryBuilder::build`, the identity string, the counts and the layer's
//!    terminal inventory against the program's terminals;
//! 7. with `verify_descent`, the descent certificate of M2-T8a, which also
//!    builds every owner program once (`descent::check`), then the census's
//!    strong completeness (`PCensus::check_complete`: every zero proof and every
//!    route again, so a sector held as zero without a proof is refused, never a
//!    silent zero), then the layer re-derived (`basis::derive_basis_layer`
//!    onto the masters of the scope: every row must be the one this program
//!    derives). Without it, all three are the word of the writer.
//!
//! [`read_custom_payloads`] reads the Custom payloads of a package, or of an
//! owner directory of `rustred p-generate` records ([`read_owner_records`]),
//! for `rustred p-package --reuse-custom`.
//!
//! Owner programs are otherwise built on first use through
//! `programs::lane_program`, Custom payloads only through
//! `custom::build_custom_step` behind it.
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

use rustred::algebra::CoefficientContext;
use rustred::family::IntegralFamily;
use rustred::propagator::basis::{PBasisLayer, PBasisOptions, derive_basis_layer};
use rustred::propagator::census::PCensus;
use rustred::propagator::custom::inspect_custom_payload;
use rustred::propagator::descent;
use rustred::propagator::error::PError;
use rustred::propagator::families::PFamiliesFile;
use rustred::propagator::library::{PLibrary, POwnerSpec};
use rustred::propagator::presentation::{Presentation, PresentationKind};
use rustred::propagator::programs::CustomPayloadRef;
use rustred::propagator::{
    NodeIndex, OwnerClassId, P_CUSTOM_OWNER_VERSION, P_FAMILIES_SCHEMA, P_LIBRARY_SCHEMA,
    P_OWNER_LIBRARY_SEMANTICS, PTerminalId, PresentationId, p_versions,
};

use super::format::{
    CENSUS_FILE, CENSUS_JSON_FILE, CountsRecord, FAMILIES_FILE, LAYER_FILE, LIBRARY_FILE,
    LaneRecord, LibraryManifest, OwnerRecord, SYMBOLICA_SAVE_FORMAT, TOPOLOGY_FILE, TerminalRecord,
    check_save_format, decode_census, decode_family_file, decode_topology, family_file, malformed,
    owner_file,
};
use super::generate::CustomOwnerPayload;
use super::ordering::{ORBITS_FILE, ORDERING_FILE, ROUTES_FILE, read_ordering};
use super::package::payload_save_format;
use super::{PLibraryOptions, PLibrarySource};
use rustred::propagator::saved_orbits::PSavedOrbits;
use rustred::propagator::saved_routes::PSavedRoutes;

/// The files of one package.
struct Files<'a>(&'a PLibrarySource);

impl Files<'_> {
    /// `name` is relative, `/`-separated, of plain components.
    fn check_name(name: &str) -> Result<(), PError> {
        let plain = |part: &str| {
            !part.is_empty()
                && !part.starts_with('.')
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        };
        if name.split('/').all(plain) {
            Ok(())
        } else {
            Err(malformed(format!(
                "the file name {name:?} leaves the package"
            )))
        }
    }

    fn read(&self, name: &str) -> Result<Cow<'static, [u8]>, PError> {
        Self::check_name(name)?;
        match self.0 {
            PLibrarySource::Directory(directory) => {
                let path = directory.join(name);
                std::fs::read(&path).map(Cow::Owned).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::NotFound {
                        malformed(format!("missing file {name} in {}", directory.display()))
                    } else {
                        malformed(format!("reading {}: {error}", path.display()))
                    }
                })
            }
            PLibrarySource::Memory(files) => files
                .iter()
                .find(|(file, _)| *file == name)
                .map(|(_, bytes)| Cow::Borrowed(*bytes))
                .ok_or_else(|| malformed(format!("missing file {name} in the embedded p library"))),
        }
    }

    /// `name` with exactly `bytes` bytes.
    fn read_sized(&self, name: &str, bytes: u64) -> Result<Cow<'static, [u8]>, PError> {
        let content = self.read(name)?;
        if content.len() as u64 != bytes {
            return Err(malformed(format!(
                "{name} has {} bytes, library.json records {bytes}",
                content.len()
            )));
        }
        Ok(content)
    }
}

fn version_mismatch(name: &str, found: impl Into<String>, expected: impl Into<String>) -> PError {
    PError::VersionMismatch {
        name: name.to_owned(),
        found: found.into(),
        expected: expected.into(),
    }
}

const ABSENT: &str = "(absent)";

/// The schema, every version and the save format of a manifest, before its
/// structure is read: a package of another build is refused by name.
fn check_manifest_versions(value: &serde_json::Value) -> Result<(), PError> {
    let schema = value.get("schema").and_then(serde_json::Value::as_str);
    if schema != Some(P_LIBRARY_SCHEMA) {
        return Err(version_mismatch(
            "P_LIBRARY_SCHEMA",
            schema.unwrap_or(ABSENT),
            P_LIBRARY_SCHEMA,
        ));
    }
    let versions = value
        .get("versions")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| malformed(format!("{LIBRARY_FILE}: no versions")))?;
    let current = p_versions();
    for (name, expected) in &current {
        match versions.get(*name).map(|found| found.as_str()) {
            Some(Some(found)) if found == expected => {}
            Some(Some(found)) => return Err(version_mismatch(name, found, expected.as_str())),
            Some(None) => return Err(version_mismatch(name, "(not a string)", expected.as_str())),
            None => return Err(version_mismatch(name, ABSENT, expected.as_str())),
        }
    }
    if let Some((name, found)) = versions
        .iter()
        .find(|(name, _)| !current.iter().any(|(known, _)| known == name))
    {
        let found = found
            .as_str()
            .map_or_else(|| found.to_string(), str::to_owned);
        return Err(version_mismatch(name, found, ABSENT));
    }
    let format = value
        .get("symbolica_save_format")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            version_mismatch(
                SYMBOLICA_SAVE_FORMAT,
                ABSENT,
                "the save format of this build",
            )
        })?;
    let format = u16::try_from(format)
        .map_err(|_| version_mismatch(SYMBOLICA_SAVE_FORMAT, format.to_string(), "a u16"))?;
    check_save_format(format, LIBRARY_FILE)
}

fn read_manifest(files: &Files<'_>) -> Result<LibraryManifest, PError> {
    let bytes = files.read(LIBRARY_FILE)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    check_manifest_versions(&value)?;
    let manifest: LibraryManifest = serde_json::from_value(value)
        .map_err(|error| malformed(format!("{LIBRARY_FILE}: {error}")))?;
    let [first, last] = manifest.scope;
    if first == 0 || first > last {
        return Err(malformed(format!("{LIBRARY_FILE}: scope {first}-{last}")));
    }
    Ok(manifest)
}

/// The manifest of a package, after the version checks of [`load_p_library`].
pub fn read_p_library_manifest(source: &PLibrarySource) -> Result<LibraryManifest, PError> {
    read_manifest(&Files(source))
}

/// The hard false knock-out guard on an owner record, before its payload is read.
fn check_owner_record(owner: &OwnerRecord) -> Result<(), PError> {
    if let Some(custom) = &owner.custom
        && custom.lp_count >= 1
        && custom.residuals == 0
    {
        return Err(malformed(format!(
            "false knock-out guard: the Custom owner {} records LP = {} and 0 residuals",
            owner.class, custom.lp_count
        )));
    }
    Ok(())
}

/// The payload bytes of `owner`, if it has one: its file, length and save format.
fn owner_payload(files: &Files<'_>, owner: &OwnerRecord) -> Result<Option<Arc<[u8]>>, PError> {
    check_owner_record(owner)?;
    let Some(payload) = &owner.payload else {
        return Ok(None);
    };
    let expected = owner_file(&owner.class)?;
    if payload.file != expected {
        return Err(malformed(format!(
            "the payload of {} is {}, not {expected}",
            owner.class, payload.file
        )));
    }
    let bytes = files.read_sized(&payload.file, payload.bytes)?;
    check_save_format(payload_save_format(&bytes, &payload.file)?, &payload.file)?;
    Ok(Some(Arc::from(bytes.as_ref())))
}

/// A payload is the one its record describes: its structure (charts,
/// residual count, two-sided axes) gives the record's meta, its residual keys
/// are the owner's residual terminals, and, when `fingerprint` is given, it is
/// bound to that family. Structure only: nothing is imported into Symbolica.
fn check_payload(
    owner: &OwnerRecord,
    bytes: &[u8],
    terminals: &[TerminalRecord],
    fingerprint: Option<&str>,
) -> Result<(), PError> {
    let summary = inspect_custom_payload(bytes)?;
    let custom = owner
        .custom
        .as_ref()
        .ok_or_else(|| malformed(format!("the payload of {} has no meta", owner.class)))?;
    let mut recorded = custom.meta();
    recorded.charts.sort_unstable();
    let held = summary.meta(custom.lp_count)?;
    if held != recorded {
        return Err(malformed(format!(
            "the payload of {} holds {held:?}, {LIBRARY_FILE} records {recorded:?}",
            owner.class
        )));
    }
    if let Some(fingerprint) = fingerprint
        && summary.family_fingerprint != fingerprint
    {
        return Err(malformed(format!(
            "the payload of {} is bound to another family than presentation {}",
            owner.class, owner.presentation
        )));
    }
    let mut named: Vec<Vec<i64>> = (owner.terminals.iter())
        .filter_map(|&id| match terminals.get(id as usize) {
            Some(TerminalRecord::Residual { owner: class, key }) if *class == owner.class => {
                Some(key.clone())
            }
            _ => None,
        })
        .collect();
    let mut residuals = summary.residual_keys();
    named.sort_unstable();
    residuals.sort_unstable();
    if named != residuals {
        return Err(malformed(format!(
            "the payload of {} has the residuals {residuals:?}, its terminals are {named:?}",
            owner.class
        )));
    }
    Ok(())
}

/// The Custom payloads of a package by class label, with their meta, for
/// `rustred p-package --reuse-custom` and regeneration tests
/// (`CustomPayloads::Provided`). Versions and the guard are checked as by
/// [`load_p_library`]. An existing directory without `library.json` and
/// without the package's `census.rrbin`, `census.json` or `owners/` is read as
/// an owner directory of `rustred p-generate` ([`read_owner_records`]); a
/// missing directory, or a package that lost its `library.json`, is refused as
/// a package (missing `library.json`).
pub fn read_custom_payloads(
    source: &PLibrarySource,
) -> Result<BTreeMap<String, CustomOwnerPayload>, PError> {
    if let PLibrarySource::Directory(directory) = source
        && directory.is_dir()
        && !directory.join(LIBRARY_FILE).exists()
        && ![CENSUS_FILE, CENSUS_JSON_FILE, "owners"]
            .iter()
            .any(|name| directory.join(name).exists())
    {
        return read_owner_records(directory);
    }
    let files = Files(source);
    let manifest = read_manifest(&files)?;
    let mut payloads = BTreeMap::new();
    for owner in &manifest.owners {
        let Some(bytes) = owner_payload(&files, owner)? else {
            continue;
        };
        check_payload(owner, &bytes, &manifest.terminals, None)?;
        let meta = owner
            .custom
            .as_ref()
            .ok_or_else(|| malformed(format!("the payload of {} has no meta", owner.class)))?
            .meta();
        payloads.insert(owner.class.clone(), CustomOwnerPayload { bytes, meta });
    }
    Ok(payloads)
}

/// Schema of the owner record `<label>.json` of `rustred p-generate`.
const OWNER_RECORD_SCHEMA: &str = "rustred.p-custom-owner.v1";

/// A slot mask written as one `0` or `1` per slot, slot 0 first, as
/// `rustred p-generate` writes it.
fn mask_text_value(text: &str, arity: usize) -> Option<u64> {
    if text.len() != arity || arity > 64 {
        return None;
    }
    (text.bytes().enumerate()).try_fold(0_u64, |mask, (slot, bit)| match bit {
        b'0' => Some(mask),
        b'1' => Some(mask | 1 << slot),
        _ => None,
    })
}

/// One owner record of [`read_owner_records`]: `<label>.json` describes the
/// payload `<label>.rrbin`, whose LP count it stores.
fn read_owner_record(
    directory: &std::path::Path,
    label: &str,
) -> Result<CustomOwnerPayload, PError> {
    let read = |name: String| {
        let path = directory.join(&name);
        std::fs::read(&path)
            .map_err(|error| malformed(format!("reading {}: {error}", path.display())))
    };
    let what = format!("{label}.json");
    let record: serde_json::Value = serde_json::from_slice(&read(what.clone())?)
        .map_err(|error| malformed(format!("{what}: {error}")))?;
    let schema = record.get("schema").and_then(serde_json::Value::as_str);
    if schema != Some(OWNER_RECORD_SCHEMA) {
        return Err(version_mismatch(
            "the owner record schema",
            schema.unwrap_or(ABSENT),
            OWNER_RECORD_SCHEMA,
        ));
    }
    let versions = record.get("versions");
    let version = |name: &str| {
        let value = versions.and_then(|versions| versions.get(name));
        value.map_or_else(
            || ABSENT.to_owned(),
            |value| {
                value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_owned)
            },
        )
    };
    for (name, expected) in [
        ("P_CUSTOM_OWNER_VERSION", P_CUSTOM_OWNER_VERSION.to_string()),
        (
            "P_OWNER_LIBRARY_SEMANTICS",
            P_OWNER_LIBRARY_SEMANTICS.to_owned(),
        ),
    ] {
        let found = version(name);
        if found != expected {
            return Err(version_mismatch(name, found, expected));
        }
    }
    let field = |name: &str| {
        record
            .get(name)
            .ok_or_else(|| malformed(format!("{what}: no {name}")))
    };
    let number = |name: &str| {
        field(name)?
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| malformed(format!("{what}: {name} is no count")))
    };
    if field("label")?.as_str() != Some(label) {
        return Err(malformed(format!("{what} records another label")));
    }
    let payload_file = format!("{label}.rrbin");
    let bytes: Arc<[u8]> = Arc::from(read(payload_file.clone())?);
    check_save_format(payload_save_format(&bytes, &payload_file)?, &payload_file)?;
    let summary = inspect_custom_payload(&bytes)?;
    let meta = summary.meta(number("lp_count")?)?;
    let arity = summary.arity;
    let mask = |name: &str, text: Option<&str>| {
        text.and_then(|text| mask_text_value(text, arity))
            .ok_or_else(|| malformed(format!("{what}: {name} is no mask of {arity} slots")))
    };
    let mut charts = (field("charts")?.as_array())
        .ok_or_else(|| malformed(format!("{what}: charts is no list")))?
        .iter()
        .map(|chart| {
            mask(
                "a chart",
                chart.get("chart").and_then(serde_json::Value::as_str),
            )
        })
        .collect::<Result<Vec<u64>, PError>>()?;
    charts.sort_unstable();
    let recorded = (
        number("arity")? as usize,
        mask("support", field("support")?.as_str())?,
        number("residuals")?,
        mask("two_sided_axes", field("two_sided_axes")?.as_str())?,
        charts,
    );
    let held = (
        arity,
        summary.support,
        meta.residuals,
        meta.two_sided_axes,
        meta.charts.clone(),
    );
    if recorded != held {
        return Err(malformed(format!(
            "{payload_file} holds (arity, support, residuals, two-sided axes, charts) {held:?}, \
             {what} records {recorded:?}"
        )));
    }
    if meta.lp_count >= 1 && meta.residuals == 0 {
        return Err(malformed(format!(
            "false knock-out guard: the Custom owner {label} records LP = {} and 0 residuals",
            meta.lp_count
        )));
    }
    Ok(CustomOwnerPayload { bytes, meta })
}

/// The Custom payloads of an owner directory written by `rustred p-generate`
/// (M3-T15 stages the scope 1-4 owners so), by class label: every
/// `<label>.json` with a payload `<label>.rrbin` beside it is an owner record;
/// other files are not read. Each record has this build's schema,
/// `P_CUSTOM_OWNER_VERSION` and `P_OWNER_LIBRARY_SEMANTICS` (else
/// [`PError::VersionMismatch`]), names its own label, and describes its payload
/// (save format, arity, support, residuals, two-sided axes, charts); its LP
/// count is the stored one, taken as recorded (plan M3 D-M3-3: counted once, at
/// generation), under the hard guard. A directory without a record is refused.
pub fn read_owner_records(
    directory: &std::path::Path,
) -> Result<BTreeMap<String, CustomOwnerPayload>, PError> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| malformed(format!("reading {}: {error}", directory.display())))?;
    let mut labels = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| malformed(format!("{}: {error}", directory.display())))?;
        let name = entry.file_name();
        let Some(label) = name.to_str().and_then(|name| name.strip_suffix(".json")) else {
            continue;
        };
        if directory.join(format!("{label}.rrbin")).is_file() {
            owner_file(label)?;
            labels.push(label.to_owned());
        }
    }
    if labels.is_empty() {
        return Err(malformed(format!(
            "{} holds neither {LIBRARY_FILE} nor an owner record <label>.json with <label>.rrbin",
            directory.display()
        )));
    }
    labels.sort_unstable();
    let mut payloads = BTreeMap::new();
    for label in labels {
        let payload = read_owner_record(directory, &label)?;
        payloads.insert(label, payload);
    }
    Ok(payloads)
}

/// The counts `library.json` records for `library`.
pub(super) fn counts_of(library: &PLibrary, tops: usize) -> CountsRecord {
    let census = library.census();
    let mut owners: BTreeMap<LaneRecord, usize> = BTreeMap::new();
    for owner in library.owners() {
        *owners.entry(LaneRecord::of(owner.spec.lane)).or_default() += 1;
    }
    let dag = library.topology();
    CountsRecord {
        presentations: library.presentations().len(),
        tops,
        dag_nodes: dag.map_or(0, |dag| dag.nodes.len()),
        dag_edges: dag.map_or(0, |dag| dag.edges.len()),
        classes: census.classes.len(),
        sectors: census.sectors.len(),
        zero_sectors: (census.sectors.values())
            .filter(|fate| matches!(fate, rustred::propagator::census::PSectorFate::Zero))
            .count(),
        edge_maps: library.edge_maps().len(),
        terminals: library.terminals().len(),
        owners,
    }
}

/// The families of a package: every family file of the manifest, decoded once.
fn load_families(
    files: &Files<'_>,
    manifest: &LibraryManifest,
) -> Result<BTreeMap<String, Arc<IntegralFamily>>, PError> {
    let mut families = BTreeMap::new();
    for record in &manifest.files.family_files {
        let expected = family_file(&record.name)?;
        if record.file != expected {
            return Err(malformed(format!(
                "the family {} is in {}, not {expected}",
                record.name, record.file
            )));
        }
        let bytes = files.read_sized(&record.file, record.bytes)?;
        let family = decode_family_file(&bytes, &record.file)?;
        if family.name() != record.name {
            return Err(malformed(format!(
                "{} holds the family {}, not {}",
                record.file,
                family.name(),
                record.name
            )));
        }
        if families
            .insert(record.file.clone(), Arc::new(family))
            .is_some()
        {
            return Err(malformed(format!("{} is listed twice", record.file)));
        }
    }
    Ok(families)
}

/// The presentations of the manifest over their families, each validated: the
/// slots of `library.json` are exactly the family's denominators and shifts.
fn load_presentations(
    manifest: &LibraryManifest,
    families: &BTreeMap<String, Arc<IntegralFamily>>,
) -> Result<Vec<Presentation>, PError> {
    let mut presentations = Vec::with_capacity(manifest.presentations.len());
    for (position, record) in manifest.presentations.iter().enumerate() {
        if record.index as usize != position {
            return Err(malformed(format!(
                "presentation {} at position {position}",
                record.index
            )));
        }
        let family = families.get(&record.family).ok_or_else(|| {
            malformed(format!(
                "presentation {position} names the family file {}, which library.json does not list",
                record.family
            ))
        })?;
        let presentation = record.presentation(Arc::clone(family))?;
        presentation.validate().map_err(|error| {
            malformed(format!(
                "presentation {position}: the family file {} does not describe its slots in {LIBRARY_FILE}: {error}",
                record.family
            ))
        })?;
        if let PresentationKind::Top { name } = &presentation.kind
            && name != family.name()
        {
            return Err(malformed(format!(
                "the top presentation {position} is named {name}, its family {}",
                family.name()
            )));
        }
        presentations.push(presentation);
    }
    Ok(presentations)
}

/// The tops are presentations `0..n`, the families of the families file in
/// order, with its slots and family.
fn check_tops(
    presentations: &[Presentation],
    families: &PFamiliesFile,
    context: &CoefficientContext,
) -> Result<(), PError> {
    if families.schema != P_FAMILIES_SCHEMA {
        return Err(version_mismatch(
            "P_FAMILIES_SCHEMA",
            families.schema.as_str(),
            P_FAMILIES_SCHEMA,
        ));
    }
    let tops = presentations
        .iter()
        .filter(|p| matches!(p.kind, PresentationKind::Top { .. }))
        .count();
    if tops != families.families.len() {
        return Err(malformed(format!(
            "{tops} top presentations for {} families in {FAMILIES_FILE}",
            families.families.len()
        )));
    }
    for (index, entry) in families.families.iter().enumerate() {
        let id = PresentationId(index as u32);
        let expected = entry.presentation(id, context)?;
        let found = presentations
            .get(index)
            .ok_or_else(|| malformed(format!("no presentation {index}")))?;
        let same = found.kind == expected.kind
            && found.loops == expected.loops
            && found.slots == expected.slots
            && found.family.fingerprint() == expected.family.fingerprint();
        if !same {
            return Err(malformed(format!(
                "the top presentation {index} differs from the family {} of {FAMILIES_FILE}",
                entry.name
            )));
        }
    }
    Ok(())
}

/// The census of `census.rrbin`, which `census.json` must repeat.
fn load_census(
    files: &Files<'_>,
    manifest: &LibraryManifest,
    verify: bool,
) -> Result<PCensus, PError> {
    if (
        manifest.files.census.as_str(),
        manifest.files.census_json.as_str(),
    ) != (CENSUS_FILE, CENSUS_JSON_FILE)
    {
        return Err(malformed(format!(
            "{LIBRARY_FILE}: census files {} and {}",
            manifest.files.census, manifest.files.census_json
        )));
    }
    let census = decode_census(&files.read(CENSUS_FILE)?)?;
    if !verify {
        // census.json is the human-readable mirror of census.rrbin; parsing
        // and comparing it (22 MB for scope 1-4) is part of the verified
        // load only (M5 1L-3L: constant cost of every cold request).
        return Ok(census);
    }
    let json = files.read(CENSUS_JSON_FILE)?;
    let json = std::str::from_utf8(&json)
        .map_err(|error| malformed(format!("{CENSUS_JSON_FILE}: {error}")))?;
    if PCensus::from_json(json)? != census {
        return Err(malformed(format!(
            "{CENSUS_FILE} and {CENSUS_JSON_FILE} disagree"
        )));
    }
    Ok(census)
}

/// The basis layer of the package, if `library.json` lists one: its schema
/// and bound versions (`PError::VersionMismatch`), its rows (M2-T9's
/// `PBasisLayer::from_json`); bound to the program after the build
/// (`PBasisLayer::check_library`).
fn load_layer(
    files: &Files<'_>,
    manifest: &LibraryManifest,
    context: &CoefficientContext,
) -> Result<Option<PBasisLayer>, PError> {
    let Some(file) = &manifest.files.layer else {
        return Ok(None);
    };
    if file != LAYER_FILE {
        return Err(malformed(format!("{LIBRARY_FILE}: layer file {file}")));
    }
    let bytes = files.read(LAYER_FILE)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|error| malformed(format!("{LAYER_FILE}: {error}")))?;
    PBasisLayer::from_json(text, context).map(Some)
}

/// The rows of a package's layer are the ones this library derives (M2-T9's
/// `derive_basis_layer`, with its fresh-`d` checks, onto the masters of the
/// library's scope, `PBasisOptions::for_library`): a row mapping a residual
/// onto the wrong master, tag or coefficient is refused, never a wrong value.
fn check_layer_derivation(library: &PLibrary, layer: &PBasisLayer) -> Result<(), PError> {
    let derived = derive_basis_layer(library, &PBasisOptions::for_library(library)?)?.layer;
    if let Some(index) = (layer.rows().iter().zip(derived.rows()))
        .position(|(held, own)| held != own)
        .or_else(|| (layer.rows().len() != derived.rows().len()).then_some(derived.rows().len()))
    {
        return Err(malformed(format!(
            "{LAYER_FILE}: row {index} is not the row this library derives (rustred p-basis)"
        )));
    }
    Ok(())
}

/// Load a written p library, failing closed: see the module docs.
pub fn load_p_library(
    source: PLibrarySource,
    options: &PLibraryOptions,
) -> Result<Arc<PLibrary>, PError> {
    let files = Files(&source);
    let manifest = read_manifest(&files)?;
    let context = CoefficientContext::try_new(["d"]).map_err(|_| PError::ContextMismatch)?;

    // Families and presentations.
    if manifest.files.families != FAMILIES_FILE {
        return Err(malformed(format!(
            "{LIBRARY_FILE}: families file {}",
            manifest.files.families
        )));
    }
    let families_file: PFamiliesFile = serde_json::from_slice(&files.read(FAMILIES_FILE)?)
        .map_err(|error| malformed(format!("{FAMILIES_FILE}: {error}")))?;
    let families = load_families(&files, &manifest)?;
    let presentations = load_presentations(&manifest, &families)?;
    check_tops(&presentations, &families_file, &context)?;

    // DAG and census.
    if manifest.files.topology != TOPOLOGY_FILE {
        return Err(malformed(format!(
            "{LIBRARY_FILE}: topology file {}",
            manifest.files.topology
        )));
    }
    let dag = decode_topology(&files.read(TOPOLOGY_FILE)?)?;
    let census = load_census(&files, &manifest, options.verify_descent)?;
    if manifest.nodes.len() != dag.nodes.len() {
        return Err(malformed(format!(
            "{} node records for {} DAG nodes",
            manifest.nodes.len(),
            dag.nodes.len()
        )));
    }
    for (position, node) in manifest.nodes.iter().enumerate() {
        let presented = node.presentation.map(|id| {
            (presentations.get(id as usize))
                .and_then(|p| p.node.as_ref())
                .map(|reference| reference.index)
        });
        if node.index as usize != position
            || presented.is_some_and(|index| index != Some(NodeIndex(node.index)))
        {
            return Err(malformed(format!(
                "the node record {position} names presentation {:?} of another node",
                node.presentation
            )));
        }
    }
    let layer = load_layer(&files, &manifest, &context)?;

    // The program.
    let [first, last] = manifest.scope;
    let fingerprints: Vec<String> = (presentations.iter())
        .map(|p| p.family.fingerprint().to_owned())
        .collect();
    let mut builder = PLibrary::builder(context, first..=last);
    for presentation in presentations {
        builder.add_presentation(presentation)?;
    }
    builder.set_topology(Arc::new(dag))?;
    for (position, record) in manifest.edge_maps.iter().enumerate() {
        if builder.add_edge_map(record.edge_map())? != position {
            return Err(malformed(format!("edge map {position} is out of order")));
        }
    }
    let classes: BTreeMap<String, OwnerClassId> = (census.classes.iter())
        .map(|(id, record)| (record.key.label(), *id))
        .collect();
    for (position, record) in manifest.terminals.iter().enumerate() {
        if builder.add_terminal(record.terminal(&classes)?)?.0 as usize != position {
            return Err(malformed(format!(
                "terminal {position} repeats an earlier one"
            )));
        }
    }
    let mut owners = Vec::with_capacity(manifest.owners.len());
    for record in &manifest.owners {
        let class = *classes.get(&record.class).ok_or_else(|| {
            malformed(format!(
                "the owner {} is no class of the census",
                record.class
            ))
        })?;
        let payload = owner_payload(&files, record)?;
        if let Some(bytes) = &payload {
            let fingerprint = fingerprints
                .get(record.presentation as usize)
                .ok_or_else(|| {
                    malformed(format!(
                        "the owner {} is on the unknown presentation {}",
                        record.class, record.presentation
                    ))
                })?;
            check_payload(record, bytes, &manifest.terminals, Some(fingerprint))?;
        }
        owners.push(POwnerSpec {
            class,
            lane: record.lane.lane(),
            presentation: PresentationId(record.presentation),
            node: record.node.map(NodeIndex),
            roles: record.roles.roles(),
            edge_maps: record.edge_maps.clone(),
            payload: payload.map(CustomPayloadRef::Bytes),
            custom: record.custom.as_ref().map(|custom| custom.meta()),
            terminals: record.terminals.iter().map(|&id| PTerminalId(id)).collect(),
        });
    }
    builder.set_census(census)?;
    for owner in owners {
        builder.add_owner(owner)?;
    }
    if let Some(layer) = &layer {
        builder.set_basis_map(Arc::new(layer.clone()))?;
    }
    if let Some(file) = &manifest.files.ordering {
        if file != ORDERING_FILE {
            return Err(malformed(format!("{LIBRARY_FILE}: ordering file {file}")));
        }
        let (heuristic, orders) = read_ordering(&files.read(ORDERING_FILE)?, &manifest, &classes)?;
        builder.set_ordering(&heuristic, orders)?;
    }
    if let Some(file) = &manifest.files.routes {
        if file != ROUTES_FILE {
            return Err(malformed(format!("{LIBRARY_FILE}: routes file {file}")));
        }
        // The verified load proves every route itself (descent certificate).
        if !options.verify_descent {
            builder.set_saved_routes(PSavedRoutes::decode(&files.read(ROUTES_FILE)?)?);
        }
    }
    if let Some(file) = &manifest.files.orbits {
        if file != ORBITS_FILE {
            return Err(malformed(format!("{LIBRARY_FILE}: orbits file {file}")));
        }
        // The verified load searches and authenticates every group itself.
        if !options.verify_descent {
            builder.set_saved_orbits(PSavedOrbits::decode(&files.read(ORBITS_FILE)?)?);
        }
    }
    let library = builder.build()?;
    // A package that carries orbits.rrbin runs with orbit canonicalisation on
    // whatever the load (M5 batch 2): the verified load searches the same
    // groups at run time instead of trusting the file, so both loads have one
    // step identity and share step caches (the saved groups are the searched
    // ones: package::saved_orbits_restore_the_searched_groups_and_change_no_value).
    if manifest.files.orbits.is_some() && options.verify_descent {
        library.set_orbit_canonicalization(rustred::propagator::P_ORBIT_CANON_WITH_SAVED_ORBITS);
    }

    if library.identity() != manifest.identity {
        return Err(malformed(format!(
            "{LIBRARY_FILE} records the identity {}, the library is {}",
            manifest.identity,
            library.identity()
        )));
    }
    if counts_of(&library, families_file.families.len()) != manifest.counts {
        return Err(malformed(format!(
            "{LIBRARY_FILE}: the counts disagree with the files"
        )));
    }
    if let Some(layer) = &layer {
        layer.check_library(&library)?;
    }
    if options.verify_descent {
        descent::check(&library)?;
        library
            .census()
            .check_complete(library.presentations(), library.topology().map(Arc::as_ref))?;
        if let Some(layer) = &layer {
            check_layer_derivation(&library, layer)?;
        }
    }
    Ok(Arc::new(library))
}
