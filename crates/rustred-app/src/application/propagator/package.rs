//! Writing a p library and structural equivalence (owner: M2-T8b rr-package).
//!
//! [`assemble_p_package`] assembles the program of a package (the assembly
//! with its basis layer, [`attach_basis_layer`]); [`p_library_files`] renders
//! an assembled library program as the files of the format of
//! [`super::format`], in memory and in name order; [`write_p_library`]
//! publishes them as one directory. Rendering is
//! deterministic: JSON from ordered maps, binary records in library order.
//! The Symbolica state inside a `.rrbin` file lists the symbols and variable
//! maps known to the writing process, so the same command in a fresh process
//! writes the same bytes, and custom payloads are copied verbatim.
//!
//! [`equivalent_p_libraries`] compares two programs structurally:
//! presentations (kind, loops, slots, node, family name and fingerprint), the
//! DAG, the census, edge maps, terminals, owners with their Custom payloads as
//! canonical rule dumps, and the basis map row by row; never Symbolica wire
//! ids or payload bytes.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustred::family::IntegralFamily;
use rustred::persistence::{BinaryIoLimits, SectionTag, inspect_program};
use rustred::propagator::basis::{PBasisLayer, PBasisOptions, with_derived_basis_layer};
use rustred::propagator::custom::custom_payload_dump;
use rustred::propagator::error::PError;
use rustred::propagator::families::PFamiliesFile;
use rustred::propagator::library::{PLibrary, PTerm, PTerminal};
use rustred::propagator::programs::CustomPayloadRef;
use rustred::propagator::tag::GammaTag;
use rustred::propagator::{OwnerClassId, P_LIBRARY_SCHEMA, PTerminalId, p_versions};

use super::CustomPayloads;
use super::assemble::{
    AssembleOptions, AssembledLibrary, LpRecount, RecountProvidedLp, assemble_library,
    assemble_library_recounted,
};
use super::format::{
    CENSUS_FILE, CENSUS_JSON_FILE, CustomRecord, EdgeMapRecord, FAMILIES_FILE, FamilyFileRecord,
    FilesRecord, LAYER_FILE, LIBRARY_FILE, LaneRecord, LibraryManifest, NodeRecord, OwnerRecord,
    PayloadRecord, PresentationRecord, ProducerRecord, RolesRecord, TOPOLOGY_FILE, TerminalRecord,
    check_save_format, encode_census, encode_family_file, encode_topology, family_file, malformed,
    owner_file, pretty_json, state_save_format, symbolica_save_format,
};
use super::loader::counts_of;
use crate::application::producer::ProducerOutputV1;

/// `assembled` with a basis map: when assembly attached none, the library's
/// own layer onto Forcer's masters is derived and attached (M2-T9,
/// `basis::with_derived_basis_layer`: same programs, same identity, the map
/// acts after the engine). At scope 1-2 the layer is the `Unit` row alone.
pub fn attach_basis_layer(mut assembled: AssembledLibrary) -> Result<AssembledLibrary, PError> {
    if assembled.library.basis_map().is_none() {
        let (library, _) = with_derived_basis_layer(&assembled.library, &PBasisOptions::default())?;
        assembled.library = Arc::new(library);
    }
    Ok(assembled)
}

/// The program `rustred p-package` writes: [`assemble_library`] then
/// [`attach_basis_layer`]. A regeneration check compares a loaded package
/// with this (`equivalent_p_libraries`), not with the bare assembly, which has
/// no basis map.
pub fn assemble_p_package(
    families: &PFamiliesFile,
    max_loops: u8,
    custom: CustomPayloads,
    options: &AssembleOptions,
) -> Result<AssembledLibrary, PError> {
    attach_basis_layer(assemble_library(families, max_loops, custom, options)?)
}

/// [`assemble_p_package`] with the LP recount of the `Provided` payloads
/// that `recount` takes (`assemble_library_recounted`, M3-I1): `rustred
/// p-package --reuse-custom <dir> --recount-lp`.
pub fn assemble_p_package_recounted(
    families: &PFamiliesFile,
    max_loops: u8,
    custom: CustomPayloads,
    options: &AssembleOptions,
    recount: &RecountProvidedLp,
) -> Result<(AssembledLibrary, Vec<LpRecount>), PError> {
    let (assembled, recounts) =
        assemble_library_recounted(families, max_loops, custom, options, recount)?;
    Ok((attach_basis_layer(assembled)?, recounts))
}

/// The Symbolica save format of a Custom payload (a candidate bundle): the
/// header of its state section.
pub fn payload_save_format(bytes: &[u8], what: &str) -> Result<u16, PError> {
    let envelope = inspect_program(bytes, BinaryIoLimits::default())
        .map_err(|error| malformed(format!("{what}: {error}")))?;
    let state = envelope
        .section(SectionTag::SYMBOLICA_STATE)
        .ok_or_else(|| malformed(format!("{what}: no Symbolica state section")))?;
    state_save_format(state).ok_or_else(|| malformed(format!("{what}: no Symbolica state header")))
}

/// Every file of the package of `assembled`, `(relative name, bytes)` in name
/// order; `layer/p-layer.json` when the library has a basis map. A library
/// without a topology, or with a basis map whose rows are no valid layer
/// (`PBasisLayer::new`), is refused.
pub fn p_library_files(assembled: &AssembledLibrary) -> Result<Vec<(String, Vec<u8>)>, PError> {
    let library = &assembled.library;
    let mut files = BTreeMap::new();
    let save_format = symbolica_save_format()?;

    // Families: one file per family name; a name names one family.
    let mut families: BTreeMap<String, Arc<IntegralFamily>> = BTreeMap::new();
    let mut presentations = Vec::with_capacity(library.presentations().len());
    for presentation in library.presentations() {
        let family = &presentation.family;
        let name = family.name().to_owned();
        let file = family_file(&name)?;
        if let Some(known) = families.get(&name) {
            if known.fingerprint() != family.fingerprint() {
                return Err(malformed(format!(
                    "two different families are named {name}"
                )));
            }
        } else {
            families.insert(name, Arc::clone(family));
        }
        presentations.push(PresentationRecord::of(presentation, file));
    }
    let mut family_files = Vec::with_capacity(families.len());
    for (name, family) in &families {
        let file = family_file(name)?;
        let bytes = encode_family_file(family)?;
        family_files.push(FamilyFileRecord {
            name: name.clone(),
            file: file.clone(),
            bytes: bytes.len() as u64,
        });
        files.insert(file, bytes);
    }
    files.insert(
        FAMILIES_FILE.to_owned(),
        pretty_json(&assembled.families, FAMILIES_FILE)?,
    );

    // Topology and census.
    let dag = library
        .topology()
        .ok_or_else(|| malformed("a library without a topology cannot be packaged"))?;
    files.insert(TOPOLOGY_FILE.to_owned(), encode_topology(dag)?);
    let census = library.census();
    files.insert(CENSUS_FILE.to_owned(), encode_census(census)?);
    files.insert(
        CENSUS_JSON_FILE.to_owned(),
        census.to_json(library.presentations())?.into_bytes(),
    );

    // Owners, their payloads and the terminals.
    let labels: BTreeMap<OwnerClassId, String> = (census.classes.iter())
        .map(|(id, record)| (*id, record.key.label()))
        .collect();
    let mut owners = Vec::with_capacity(library.owners().len());
    for owner in library.owners() {
        let spec = &owner.spec;
        let label = labels
            .get(&spec.class)
            .cloned()
            .ok_or_else(|| malformed(format!("owner {:016x} outside the census", spec.class.0)))?;
        let payload = match &spec.payload {
            None => None,
            Some(payload) => {
                let file = owner_file(&label)?;
                let bytes = payload.bytes()?;
                check_save_format(payload_save_format(&bytes, &file)?, &file)?;
                let record = PayloadRecord {
                    file: file.clone(),
                    bytes: bytes.len() as u64,
                };
                files.insert(file, bytes.to_vec());
                Some(record)
            }
        };
        owners.push(OwnerRecord {
            class: label,
            lane: LaneRecord::of(spec.lane),
            presentation: spec.presentation.0,
            node: spec.node.map(|node| node.0),
            roles: RolesRecord::of(&spec.roles),
            edge_maps: spec.edge_maps.clone(),
            terminals: spec.terminals.iter().map(|terminal| terminal.0).collect(),
            payload,
            custom: spec.custom.as_ref().map(CustomRecord::of),
        });
    }
    let terminals = (library.terminals().iter())
        .map(|terminal| TerminalRecord::of(terminal, &labels))
        .collect::<Result<Vec<_>, PError>>()?;
    // The basis layer, read back through the library's map (any map whose rows
    // form a valid layer).
    let layer = match PBasisLayer::from_library(library)? {
        Some(layer) => {
            files.insert(LAYER_FILE.to_owned(), layer.to_json().into_bytes());
            Some(LAYER_FILE.to_owned())
        }
        None => None,
    };

    let producer = ProducerOutputV1::current();
    let manifest = LibraryManifest {
        schema: P_LIBRARY_SCHEMA.to_owned(),
        identity: library.identity(),
        scope: [*library.scope().start(), *library.scope().end()],
        versions: p_versions()
            .into_iter()
            .map(|(n, v)| (n.to_owned(), v))
            .collect(),
        symbolica_save_format: save_format,
        producer: ProducerRecord {
            name: producer.name.to_owned(),
            rustred_version: producer.rustred_version.to_owned(),
            symbolica_version: producer.symbolica_version.to_owned(),
        },
        files: FilesRecord {
            families: FAMILIES_FILE.to_owned(),
            family_files,
            topology: TOPOLOGY_FILE.to_owned(),
            census: CENSUS_FILE.to_owned(),
            census_json: CENSUS_JSON_FILE.to_owned(),
            layer,
        },
        presentations,
        nodes: (assembled.nodes.iter())
            .map(|node| NodeRecord {
                index: node.index.0,
                lane: LaneRecord::of(node.decision.lane),
                trivial: node.decision.trivial,
                presentation: node.presentation.map(|id| id.0),
                fallback: node.fallback.clone(),
            })
            .collect(),
        edge_maps: library.edge_maps().iter().map(EdgeMapRecord::of).collect(),
        terminals,
        owners,
        counts: counts_of(library, assembled.families.families.len()),
    };
    files.insert(
        LIBRARY_FILE.to_owned(),
        pretty_json(&manifest, LIBRARY_FILE)?,
    );
    Ok(files.into_iter().collect())
}

fn io_failure(path: &Path, error: std::io::Error) -> PError {
    PError::Generation(format!("writing the p library {}: {error}", path.display()))
}

/// Write the package of `library` as the directory `directory`, which must not
/// exist or be empty: the files go to a sibling temporary directory first,
/// which is then renamed, so a reader never sees half a package.
pub fn write_p_library(library: &AssembledLibrary, directory: &Path) -> Result<(), PError> {
    write_p_library_files(&p_library_files(library)?, directory)
}

/// [`write_p_library`] of files already rendered by [`p_library_files`].
pub fn write_p_library_files(files: &[(String, Vec<u8>)], directory: &Path) -> Result<(), PError> {
    if directory.exists() {
        let mut entries = fs::read_dir(directory).map_err(|error| io_failure(directory, error))?;
        if entries.next().is_some() {
            return Err(PError::Unsupported(format!(
                "refusing to write the p library into the non-empty directory {}",
                directory.display()
            )));
        }
    }
    let name = directory.file_name().ok_or_else(|| {
        PError::Unsupported(format!("no directory name in {}", directory.display()))
    })?;
    let parent = match directory.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    fs::create_dir_all(&parent).map_err(|error| io_failure(&parent, error))?;
    let partial = parent.join(format!(
        ".{}.partial-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    if partial.exists() {
        fs::remove_dir_all(&partial).map_err(|error| io_failure(&partial, error))?;
    }
    let result = (|| {
        for (file, bytes) in files {
            let path = partial.join(file);
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).map_err(|error| io_failure(dir, error))?;
            }
            fs::write(&path, bytes).map_err(|error| io_failure(&path, error))?;
        }
        if directory.exists() {
            fs::remove_dir(directory).map_err(|error| io_failure(directory, error))?;
        }
        fs::rename(&partial, directory).map_err(|error| io_failure(directory, error))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&partial);
    }
    result
}

// ---------------------------------------------------------------------------
// Structural equivalence
// ---------------------------------------------------------------------------

/// The canonical text of a Custom payload: its rules and coefficients
/// (M2-T7c's dump), independent of the Symbolica symbol table.
fn payload_text(payload: &CustomPayloadRef) -> Result<String, PError> {
    custom_payload_dump(&payload.bytes()?)
}

/// One basis row: the terms of `terminal` at tag one.
fn basis_row(library: &PLibrary, terminal: PTerminalId) -> Option<Result<Vec<PTerm>, PError>> {
    library
        .basis_map()
        .map(|map| map.map(terminal, &GammaTag::one()))
}

/// Where `a` and `b` differ structurally, one line per difference (empty:
/// equivalent). Payloads are compared through their canonical dumps; errors
/// only when a payload cannot be read.
pub fn p_library_differences(a: &PLibrary, b: &PLibrary) -> Result<Vec<String>, PError> {
    let mut out = Vec::new();
    let mut differ = |what: String| out.push(what);
    if a.identity() != b.identity() {
        differ(format!("identity: {} vs {}", a.identity(), b.identity()));
    }
    if a.scope() != b.scope() {
        differ(format!("scope: {:?} vs {:?}", a.scope(), b.scope()));
    }
    if a.coefficient_context().parameter_names() != b.coefficient_context().parameter_names() {
        differ("coefficient context".to_owned());
    }

    // Presentations.
    if a.presentations().len() != b.presentations().len() {
        differ(format!(
            "presentations: {} vs {}",
            a.presentations().len(),
            b.presentations().len()
        ));
    }
    for (x, y) in a.presentations().iter().zip(b.presentations()) {
        let same = x.id == y.id
            && x.kind == y.kind
            && x.loops == y.loops
            && x.slots == y.slots
            && x.node == y.node
            && x.family.name() == y.family.name()
            && x.family.fingerprint() == y.family.fingerprint();
        if !same {
            differ(format!("presentation {}", x.id.0));
        }
    }

    // DAG, census, edge maps, terminals.
    if a.topology().map(Arc::as_ref) != b.topology().map(Arc::as_ref) {
        differ("topology".to_owned());
    }
    let (census_a, census_b) = (a.census(), b.census());
    if census_a.classes != census_b.classes {
        differ("census classes".to_owned());
    }
    if census_a.sectors != census_b.sectors {
        let count = (census_a.sectors.iter())
            .filter(|(sector, fate)| census_b.sectors.get(sector) != Some(fate))
            .count()
            + (census_b.sectors.keys())
                .filter(|sector| !census_a.sectors.contains_key(sector))
                .count();
        differ(format!("census sectors: {count} differ"));
    }
    if a.edge_maps() != b.edge_maps() {
        let count = (a.edge_maps().iter().zip(b.edge_maps()))
            .filter(|(x, y)| x != y)
            .count();
        differ(format!(
            "edge maps: {} vs {}, {count} of the common ones differ",
            a.edge_maps().len(),
            b.edge_maps().len()
        ));
    }
    if a.terminals() != b.terminals() {
        differ(format!(
            "terminals: {} vs {}",
            a.terminals().len(),
            b.terminals().len()
        ));
    }

    // Owners.
    let owners_b: BTreeMap<_, _> = b.owners().map(|owner| (owner.spec.class, owner)).collect();
    if a.owners().len() != b.owners().len() {
        differ(format!(
            "owners: {} vs {}",
            a.owners().len(),
            b.owners().len()
        ));
    }
    for owner in a.owners() {
        let x = &owner.spec;
        let Some(other) = owners_b.get(&x.class) else {
            differ(format!("owner {:016x} only in the first", x.class.0));
            continue;
        };
        let y = &other.spec;
        let same = x.lane == y.lane
            && x.presentation == y.presentation
            && x.node == y.node
            && x.roles == y.roles
            && x.edge_maps == y.edge_maps
            && x.custom == y.custom
            && x.terminals == y.terminals;
        if !same {
            differ(format!("owner {:016x}", x.class.0));
        }
        match (&x.payload, &y.payload) {
            (None, None) => {}
            (Some(p), Some(q)) => {
                if payload_text(p)? != payload_text(q)? {
                    differ(format!("owner {:016x}: payload rules", x.class.0));
                }
            }
            _ => differ(format!(
                "owner {:016x}: payload on one side only",
                x.class.0
            )),
        }
    }

    // Basis map, row by row.
    match (a.basis_map().is_some(), b.basis_map().is_some()) {
        (false, false) => {}
        (true, true) => {
            for (index, terminal) in a.terminals().iter().enumerate() {
                let id = PTerminalId(index as u32);
                if basis_row(a, id) != basis_row(b, id) {
                    let what = match terminal {
                        PTerminal::Unit => "unit".to_owned(),
                        PTerminal::Residual { owner, key } => {
                            format!("residual {:016x} {:?}", owner.0, key.powers())
                        }
                    };
                    differ(format!("basis row of terminal {index} ({what})"));
                }
            }
        }
        _ => differ("basis map on one side only".to_owned()),
    }
    Ok(out)
}

/// `a` and `b` are the same library program, see [`p_library_differences`].
pub fn equivalent_p_libraries(a: &PLibrary, b: &PLibrary) -> Result<bool, PError> {
    Ok(p_library_differences(a, b)?.is_empty())
}
