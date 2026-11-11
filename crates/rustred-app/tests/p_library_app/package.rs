//! p-library format, writer, loader and equivalence (owner: M2-T8b rr-package).
//!
//! The scope 1-2 library program written as a package and loaded back from the
//! directory and from embedded bytes: the same program (structural
//! equivalence), the same reductions on random keys, the same step-cache
//! binding; byte-identical renderings; and one test per way a package can be
//! wrong, each refused with a typed error before anything is reduced.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::algebra::Coefficient;
use rustred::family::IntegralKey;
use rustred::propagator::basis::{PBasisLayer, PBasisOptions, derive_basis_layer};
use rustred::propagator::census::{PCensus, PSectorFate};
use rustred::propagator::descent;
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::lane::{LaneRoles, OwnerLane};
use rustred::propagator::library::{
    PBasis, PLibrary, POwnerSpec, PReduction, PReductionRequest, PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{Presentation, Slot};
use rustred::propagator::programs::CustomPayloadRef;
use rustred::propagator::{P_LIBRARY_SCHEMA, PTerminalId, PresentationId};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedStepCache, OrderedWeightArithmetic,
    PolynomialOptions, ReductionLimits, SampledOptions,
};
use rustred_app::propagator::assemble::{SplitMix, ValidateOptions, validation_keys};
use rustred_app::propagator::format::{
    CENSUS_FILE, CENSUS_JSON_FILE, FAMILIES_FILE, LAYER_FILE, LIBRARY_FILE, LibraryManifest,
    TOPOLOGY_FILE, decode_census, encode_census, symbolica_save_format,
};
use rustred_app::propagator::loader::{read_custom_payloads, read_p_library_manifest};
use rustred_app::propagator::package::{
    assemble_p_package, p_library_differences, p_library_files,
};
use rustred_app::propagator::{
    AssembleOptions, AssembledLibrary, CustomOwnerOptions, CustomPayloads, PLibraryOptions,
    PLibrarySource, equivalent_p_libraries, generate_custom_owner, load_p_library, write_p_library,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn families() -> PFamiliesFile {
    serde_json::from_str(P_FAMILIES_V1_JSON).unwrap()
}

/// The program `rustred p-package` writes: the assembly with its basis layer.
pub(crate) fn assemble(max_loops: u8) -> AssembledLibrary {
    assemble_p_package(
        &families(),
        max_loops,
        CustomPayloads::Generate(CustomOwnerOptions::default()),
        &AssembleOptions {
            threads: 1,
            verify_descent: true,
        },
    )
    .unwrap()
}

/// The scope 1-2 library program, assembled once per test process.
pub(crate) fn scope_two() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| assemble(2))
}

/// A fresh directory under the system temporary directory, removed on drop.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-p-package-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(crate) fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Every file under `directory`, by relative `/`-separated name.
pub(crate) fn tree(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let name = path.strip_prefix(root).unwrap().to_string_lossy();
                out.insert(name.replace('\\', "/"), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(directory, directory, &mut out);
    out
}

/// The package of `assembled` written into a scratch directory.
fn written(assembled: &AssembledLibrary, label: &str) -> (Scratch, PathBuf) {
    let scratch = Scratch::new(label);
    let directory = scratch.join("p-library");
    write_p_library(assembled, &directory).unwrap();
    (scratch, directory)
}

/// Embedded files, as vakint holds them.
fn memory(files: &BTreeMap<String, Vec<u8>>) -> PLibrarySource {
    let leaked: Vec<(&'static str, &'static [u8])> = files
        .iter()
        .map(|(name, bytes)| {
            let name: &'static str = Box::leak(name.clone().into_boxed_str());
            let bytes: &'static [u8] = Box::leak(bytes.clone().into_boxed_slice());
            (name, bytes)
        })
        .collect();
    PLibrarySource::Memory(Box::leak(leaked.into_boxed_slice()))
}

const VERIFY: PLibraryOptions = PLibraryOptions {
    verify_descent: true,
};

fn load(directory: &Path) -> Result<Arc<PLibrary>, PError> {
    load_p_library(PLibrarySource::Directory(directory.to_path_buf()), &VERIFY)
}

fn manifest_of(directory: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(directory.join(LIBRARY_FILE)).unwrap()).unwrap()
}

fn write_manifest(directory: &Path, value: &serde_json::Value) {
    std::fs::write(
        directory.join(LIBRARY_FILE),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
}

/// A fresh copy of the scope 1-2 package, damaged by `damage`, loaded.
fn damaged(label: &str, damage: impl FnOnce(&Path)) -> Result<Arc<PLibrary>, PError> {
    let (_scratch, directory) = written(scope_two(), label);
    damage(&directory);
    load(&directory)
}

/// The inputs of one reduction request.
type Inputs = Vec<(PresentationId, IntegralKey, Coefficient)>;

fn request<'a>(inputs: Inputs, onto_masters: bool) -> PReductionRequest<'a> {
    PReductionRequest {
        inputs,
        arithmetic: OrderedWeightArithmetic::Exact,
        scheduler: OrderedSchedulerOptions::default(),
        step_cache: None,
        uncovered: PUncoveredPolicy::Reject,
        onto_masters,
        limits: ReductionLimits::default(),
        inspector: None,
    }
}

/// p-validate's key bounds (dots 0-3, rank 0-8 in total) with `points` keys.
fn keys(points: usize) -> ValidateOptions {
    ValidateOptions {
        points,
        ..ValidateOptions::default()
    }
}

/// Requests of M2-T8a's random keys (p-validate's, random subsectors), with
/// the bounds `options(L)` per loop count: each key alone, and sums of five
/// with small integer weights.
fn random_requests(
    assembled: &AssembledLibrary,
    options: impl Fn(u8) -> ValidateOptions,
) -> Vec<Inputs> {
    let context = assembled.library.coefficient_context();
    let mut rng = SplitMix::new(0x0bad_cafe);
    let mut requests = Vec::new();
    for loops in 1..=assembled.max_loops {
        let (drawn, _) = validation_keys(assembled, loops, &options(loops));
        let inputs: Vec<_> = drawn
            .into_iter()
            .map(|(id, powers)| (id, IntegralKey::try_new(powers).unwrap()))
            .collect();
        for (id, key) in &inputs {
            requests.push(vec![(*id, key.clone(), context.one())]);
        }
        for chunk in inputs.chunks(5) {
            requests.push(
                chunk
                    .iter()
                    .map(|(id, key)| {
                        let weight = rng.below(7) as i64 - 3;
                        (*id, key.clone(), context.integer(weight))
                    })
                    .collect(),
            );
        }
    }
    requests
}

fn reductions(library: &PLibrary, requests: &[Inputs], onto_masters: bool) -> Vec<PReduction> {
    requests
        .iter()
        .map(|inputs| {
            library
                .reduce(request(inputs.clone(), onto_masters))
                .unwrap()
        })
        .collect()
}

/// `reductions` under `arithmetic`.
fn reductions_with(
    library: &PLibrary,
    requests: &[Inputs],
    onto_masters: bool,
    arithmetic: OrderedWeightArithmetic,
) -> Vec<PReduction> {
    requests
        .iter()
        .map(|inputs| {
            let mut run = request(inputs.clone(), onto_masters);
            run.arithmetic = arithmetic;
            library.reduce(run).unwrap()
        })
        .collect()
}

/// The two modular arithmetics of the engine on one thread.
fn modular_arithmetics() -> [(&'static str, OrderedWeightArithmetic); 2] {
    let common = ModularCommon {
        threads: 1,
        ..ModularCommon::default()
    };
    [
        (
            "ModularSampled",
            OrderedWeightArithmetic::ModularSampled(common, SampledOptions::default()),
        ),
        (
            "ModularPolynomial",
            OrderedWeightArithmetic::ModularPolynomial(common, PolynomialOptions::default()),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Round trip
// ---------------------------------------------------------------------------

/// Written and loaded back, from the directory and from embedded bytes, the
/// scope 1-2 program is the same: structurally equivalent, with the same
/// identity and step-cache binding, and the same reductions (terms, conditions
/// and work counters) on random keys, onto the terminals and onto the masters.
#[test]
fn scope_one_to_two_round_trips_through_a_directory_and_embedded_bytes() {
    let assembled = scope_two();
    let (_scratch, directory) = written(assembled, "round-trip");
    let from_directory = load(&directory).unwrap();
    let from_memory = load_p_library(memory(&tree(&directory)), &VERIFY).unwrap();
    for loaded in [&from_directory, &from_memory] {
        assert_eq!(
            p_library_differences(&assembled.library, loaded).unwrap(),
            Vec::<String>::new()
        );
        assert!(equivalent_p_libraries(&assembled.library, loaded).unwrap());
        assert_eq!(loaded.identity(), assembled.library.identity());
        assert_eq!(
            loaded.step_cache_binding().unwrap(),
            assembled.library.step_cache_binding().unwrap()
        );
    }
    assert!(equivalent_p_libraries(&from_directory, &from_memory).unwrap());

    let requests = random_requests(assembled, |_| keys(150));
    let start = std::time::Instant::now();
    let expected = reductions(&assembled.library, &requests, false);
    let seconds = start.elapsed().as_secs_f64();
    assert_eq!(reductions(&from_directory, &requests, false), expected);
    assert_eq!(reductions(&from_memory, &requests, false), expected);
    let onto_masters = &requests[..40];
    let expected_masters = reductions(&assembled.library, onto_masters, true);
    assert_eq!(
        reductions(&from_directory, onto_masters, true),
        expected_masters
    );
    let nonzero = expected.iter().filter(|r| !r.terms.is_empty()).count();
    println!(
        "round trip: {} requests ({nonzero} nonzero), identical from the directory and from memory; assembled reductions {seconds:.2} s",
        requests.len()
    );
    assert!(nonzero >= 100, "{nonzero}");
}

/// A step cache filled through the assembled program serves the loaded one:
/// the same binding, the same input replayed without a miss, an overlapping
/// input with step hits, results equal to uncached reductions.
#[test]
fn a_step_cache_of_the_assembled_program_serves_the_loaded_one() {
    let assembled = scope_two();
    let (_scratch, directory) = written(assembled, "step-cache");
    let loaded = load(&directory).unwrap();
    let cache = OrderedStepCache::in_memory(
        assembled.library.step_cache_binding().unwrap(),
        Default::default(),
    );
    let context = assembled.library.coefficient_context();
    let top = assembled.tops_of(2)[0];
    let arity = assembled.library.presentation(top).slots.len();
    let input = |first: i64| {
        let mut powers = vec![1_i64; arity];
        powers[0] = first;
        vec![(top, IntegralKey::try_new(powers).unwrap(), context.one())]
    };
    let cached = |library: &PLibrary, inputs| {
        let mut run = request(inputs, false);
        run.step_cache = Some(Arc::clone(&cache));
        library.reduce(run).unwrap()
    };
    let first = cached(&assembled.library, input(1));
    let again = cached(&loaded, input(1));
    assert_eq!(again.terms, first.terms);
    assert_eq!(again.statistics.progress.step_cache_misses, 0);
    let raised = cached(&loaded, input(2));
    let uncached = assembled.library.reduce(request(input(2), false)).unwrap();
    assert_eq!(raised.terms, uncached.terms);
    let progress = raised.statistics.progress;
    println!(
        "loaded program on the assembled program's cache: same input {} hits {} misses, raised input {} hits {} misses (assembled first run {} misses)",
        again.statistics.progress.step_cache_hits,
        again.statistics.progress.step_cache_misses,
        progress.step_cache_hits,
        progress.step_cache_misses,
        first.statistics.progress.step_cache_misses
    );
    assert!(progress.step_cache_hits > 0, "{progress:?}");
}

/// Two renderings in one process, and of two assemblies, are byte-identical
/// (two `rustred p-package` processes: `cli::`), and the package has the
/// expected files. Prints the sizes (plan M2 risk 14).
#[test]
fn renderings_are_byte_identical_and_the_sizes_are_recorded() {
    let assembled = scope_two();
    let files = p_library_files(assembled).unwrap();
    assert_eq!(p_library_files(assembled).unwrap(), files);
    assert_eq!(p_library_files(&assemble(2)).unwrap(), files);
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    for name in [
        LIBRARY_FILE,
        FAMILIES_FILE,
        TOPOLOGY_FILE,
        CENSUS_FILE,
        CENSUS_JSON_FILE,
        LAYER_FILE,
    ] {
        assert!(names.contains(&name), "{names:?}");
    }
    // P1L, P2L and one family per node presentation (6 at scope 1-2).
    let family_files = names
        .iter()
        .filter(|name| name.starts_with("families/") && name.ends_with(".rrbin"))
        .count();
    assert_eq!(family_files, 2 + 6, "{names:?}");
    assert!(!names.iter().any(|name| name.starts_with("owners/")));
    let total: usize = files.iter().map(|(_, bytes)| bytes.len()).sum();
    for (name, bytes) in &files {
        println!("{:>9}  {name}", bytes.len());
    }
    println!("{total:>9}  total, {} files (scope 1-2)", files.len());
    let manifest = read_p_library_manifest(&memory(&files.into_iter().collect())).unwrap();
    assert_eq!(manifest.schema, P_LIBRARY_SCHEMA);
    assert_eq!(manifest.identity, assembled.library.identity());
    assert_eq!(
        manifest.symbolica_save_format,
        symbolica_save_format().unwrap()
    );
    assert_eq!(manifest.scope, [1, 2]);
}

/// The structural comparison sees another program.
#[test]
fn structural_equivalence_tells_programs_apart() {
    let one = assemble(1);
    let two = &scope_two().library;
    assert!(equivalent_p_libraries(two, two).unwrap());
    let differences = p_library_differences(&one.library, two).unwrap();
    assert!(
        differences.iter().any(|d| d.starts_with("identity")),
        "{differences:?}"
    );
    assert!(
        differences.iter().any(|d| d.starts_with("presentations")),
        "{differences:?}"
    );
    assert!(!equivalent_p_libraries(two, &one.library).unwrap());
}

/// The writer publishes a whole directory or nothing; it never writes into a
/// non-empty one.
#[test]
fn the_writer_refuses_a_non_empty_directory() {
    let scratch = Scratch::new("non-empty");
    let directory = scratch.join("p-library");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("stale"), b"x").unwrap();
    assert!(matches!(
        write_p_library(scope_two(), &directory),
        Err(PError::Unsupported(m)) if m.contains("non-empty")
    ));
    std::fs::remove_file(directory.join("stale")).unwrap();
    write_p_library(scope_two(), &directory).unwrap();
    assert!(load(&directory).is_ok());
}

// ---------------------------------------------------------------------------
// Fail closed
// ---------------------------------------------------------------------------

fn version_mismatch(error: PError, name: &str, found: &str) {
    let text = error.to_string();
    match error {
        PError::VersionMismatch {
            name: n, found: f, ..
        } => {
            assert_eq!((n.as_str(), f.as_str()), (name, found), "{text}");
            assert!(text.contains("regenerate with rustred p-package"), "{text}");
        }
        other => panic!("expected a version mismatch of {name}, got {other:?}"),
    }
}

fn malformed(result: Result<Arc<PLibrary>, PError>, needle: &str) {
    match result {
        Err(PError::MalformedLibrary(message)) => {
            assert!(message.contains(needle), "{message:?} lacks {needle:?}")
        }
        Err(other) => panic!("expected a malformed library ({needle}), got {other:?}"),
        Ok(_) => panic!("a damaged package ({needle}) loaded"),
    }
}

/// Another value of any version, a missing or unknown entry, another schema or
/// another Symbolica save format: refused by name, with the regenerate hint.
#[test]
fn another_version_is_refused_with_the_regenerate_hint() {
    let edit = |label: &str, edit: &dyn Fn(&mut serde_json::Value)| {
        damaged(label, |directory| {
            let mut value = manifest_of(directory);
            edit(&mut value);
            write_manifest(directory, &value);
        })
        .unwrap_err()
    };
    version_mismatch(
        edit("census-version", &|v| {
            v["versions"]["P_CENSUS_VERSION"] = "0".into()
        }),
        "P_CENSUS_VERSION",
        "0",
    );
    version_mismatch(
        edit("semantics", &|v| {
            v["versions"]["P_OWNER_LIBRARY_SEMANTICS"] = "rustred-p-owner-library-1999-01-01".into()
        }),
        "P_OWNER_LIBRARY_SEMANTICS",
        "rustred-p-owner-library-1999-01-01",
    );
    version_mismatch(
        edit("missing-version", &|v| {
            v["versions"]
                .as_object_mut()
                .unwrap()
                .remove("P_TOPOLOGY_VERSION");
        }),
        "P_TOPOLOGY_VERSION",
        "(absent)",
    );
    version_mismatch(
        edit("unknown-version", &|v| {
            v["versions"]["P_FUTURE_VERSION"] = "1".into()
        }),
        "P_FUTURE_VERSION",
        "1",
    );
    version_mismatch(
        edit("schema", &|v| {
            v["schema"] = "rustred.p-owner-library.v2".into()
        }),
        "P_LIBRARY_SCHEMA",
        "rustred.p-owner-library.v2",
    );
    let format = symbolica_save_format().unwrap();
    version_mismatch(
        edit("save-format", &|v| v["symbolica_save_format"] = 11.into()),
        &format!("SYMBOLICA_SAVE_FORMAT ({LIBRARY_FILE})"),
        "11",
    );
    assert_ne!(format, 11);
    // The same refusal for the payload reader of --reuse-custom.
    let (_scratch, directory) = written(scope_two(), "reuse-version");
    let mut value = manifest_of(&directory);
    value["versions"]["P_CUSTOM_OWNER_VERSION"] = "0".into();
    write_manifest(&directory, &value);
    version_mismatch(
        read_custom_payloads(&PLibrarySource::Directory(directory)).unwrap_err(),
        "P_CUSTOM_OWNER_VERSION",
        "0",
    );
}

/// A missing file is named; nothing is guessed.
#[test]
fn a_missing_file_is_refused() {
    let family = |directory: &Path| {
        let manifest: LibraryManifest = serde_json::from_value(manifest_of(directory)).unwrap();
        manifest.files.family_files[0].file.clone()
    };
    for (label, file) in [
        ("missing-census-json", CENSUS_JSON_FILE.to_owned()),
        ("missing-census", CENSUS_FILE.to_owned()),
        ("missing-topology", TOPOLOGY_FILE.to_owned()),
        ("missing-families", FAMILIES_FILE.to_owned()),
        ("missing-manifest", LIBRARY_FILE.to_owned()),
    ] {
        malformed(
            damaged(label, |directory| {
                std::fs::remove_file(directory.join(&file)).unwrap()
            }),
            &format!("missing file {file}"),
        );
    }
    malformed(
        damaged("missing-family", |directory| {
            std::fs::remove_file(directory.join(family(directory))).unwrap()
        }),
        "missing file families/",
    );
    let mut files = tree(&written(scope_two(), "missing-memory").1);
    files.remove(CENSUS_FILE);
    malformed(
        load_p_library(memory(&files), &VERIFY),
        "missing file census.rrbin in the embedded p library",
    );
}

/// A family file that is not the family of its presentations, slots that are
/// not the family's, and tops that are not the families file's are refused.
#[test]
fn a_family_mismatch_is_refused() {
    // Another family under the name of the first: same length recorded, so the
    // decoded family itself is compared.
    malformed(
        damaged("family-swap", |directory| {
            let mut value = manifest_of(directory);
            let files = value["files"]["family_files"].as_array().unwrap().clone();
            let (first, second) = (&files[0], &files[1]);
            let bytes = std::fs::read(directory.join(second["file"].as_str().unwrap())).unwrap();
            std::fs::write(directory.join(first["file"].as_str().unwrap()), &bytes).unwrap();
            value["files"]["family_files"][0]["bytes"] = bytes.len().into();
            write_manifest(directory, &value);
        }),
        "holds the family",
    );
    // A truncated family file.
    malformed(
        damaged("family-truncated", |directory| {
            let value = manifest_of(directory);
            let file = directory.join(value["files"]["family_files"][0]["file"].as_str().unwrap());
            let bytes = std::fs::read(&file).unwrap();
            std::fs::write(&file, &bytes[..bytes.len() - 1]).unwrap();
        }),
        "bytes, library.json records",
    );
    // A slot momentum of library.json that its family does not have.
    malformed(
        damaged("slot", |directory| {
            let mut value = manifest_of(directory);
            let slot = &mut value["presentations"][1]["slots"][0]["line"]["momentum"][0];
            *slot = (slot.as_i64().unwrap() + 1).into();
            write_manifest(directory, &value);
        }),
        "does not describe its slots",
    );
    // A top that is not the family of the families file.
    malformed(
        damaged("families-file", |directory| {
            let path = directory.join(FAMILIES_FILE);
            let mut file: PFamiliesFile =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            file.families.swap(0, 1);
            std::fs::write(&path, serde_json::to_vec_pretty(&file).unwrap()).unwrap();
        }),
        "differs from the family",
    );
}

/// The census with one route replaced by another unimodular map: the builder
/// accepts it (shape and unimodularity), the descent certificate refuses it,
/// and the package equality check counts it as a difference (its tolerance of
/// alternative route maps needs both maps verified). `census.rrbin` alone damaged: the two census files disagree.
#[test]
fn a_corrupted_route_fails_the_descent_certificate() {
    fn corrupt(census: &mut PCensus) {
        let (sector, map) = census
            .sectors
            .iter_mut()
            .find_map(|(sector, fate)| match fate {
                PSectorFate::Owner { class, map } => {
                    let owner = census_owner(*class);
                    (owner != *sector).then_some((*sector, map))
                }
                PSectorFate::Zero => None,
            })
            .unwrap();
        map.loop_external[0] += 1;
        println!("corrupted the route of sector {sector:?}: {map:?}");
    }
    fn census_owner(class: rustred::propagator::OwnerClassId) -> (PresentationId, u64) {
        scope_two().library.census().classes[&class].owner
    }
    let rewrite = |directory: &Path, json: bool| {
        let mut census =
            decode_census(&std::fs::read(directory.join(CENSUS_FILE)).unwrap()).unwrap();
        corrupt(&mut census);
        std::fs::write(directory.join(CENSUS_FILE), encode_census(&census).unwrap()).unwrap();
        if json {
            let presentations = scope_two().library.presentations();
            std::fs::write(
                directory.join(CENSUS_JSON_FILE),
                census.to_json(presentations).unwrap(),
            )
            .unwrap();
        }
    };
    let (_scratch, directory) = written(scope_two(), "route");
    rewrite(&directory, true);
    let unchecked = load_p_library(
        PLibrarySource::Directory(directory.clone()),
        &PLibraryOptions {
            verify_descent: false,
        },
    );
    assert!(unchecked.is_ok(), "{unchecked:?}");
    // The equality check tolerates another stored route map only when both
    // maps are admissible routes (`class::verify_route`, M5 combined): this
    // one is not, so it is a difference.
    assert_eq!(
        p_library_differences(&scope_two().library, unchecked.as_ref().unwrap()).unwrap(),
        vec!["census sectors: 1 differ".to_owned()]
    );
    malformed(load(&directory), "the route of sector");

    malformed(
        damaged("census-disagree", |directory| rewrite(directory, false)),
        "census.rrbin and census.json disagree",
    );
}

/// A routed sector of a top held as zero in both census files: the files agree
/// and the builder accepts them, the census's completeness check under
/// `verify_descent` refuses the zero without a proof.
#[test]
fn a_zero_without_a_proof_is_refused() {
    let (_scratch, directory) = written(scope_two(), "zero");
    let mut census = decode_census(&std::fs::read(directory.join(CENSUS_FILE)).unwrap()).unwrap();
    let owners: Vec<_> = census.classes.values().map(|record| record.owner).collect();
    let (sector, fate) = census
        .sectors
        .iter_mut()
        .find(|(sector, fate)| {
            sector.0 == PresentationId(1)
                && matches!(fate, PSectorFate::Owner { .. })
                && !owners.contains(sector)
        })
        .unwrap();
    println!("held as zero: sector {sector:?}");
    *fate = PSectorFate::Zero;
    std::fs::write(directory.join(CENSUS_FILE), encode_census(&census).unwrap()).unwrap();
    std::fs::write(
        directory.join(CENSUS_JSON_FILE),
        census.to_json(scope_two().library.presentations()).unwrap(),
    )
    .unwrap();
    let mut manifest = manifest_of(&directory);
    let zeros = manifest["counts"]["zero_sectors"].as_u64().unwrap();
    manifest["counts"]["zero_sectors"] = (zeros + 1).into();
    write_manifest(&directory, &manifest);
    let unchecked = load_p_library(
        PLibrarySource::Directory(directory.clone()),
        &PLibraryOptions {
            verify_descent: false,
        },
    );
    assert!(unchecked.is_ok(), "{unchecked:?}");
    malformed(load(&directory), "held as zero, without a proof");
}

/// An owner record with LP >= 1 and no residual is refused at load, before
/// its payload is read (the payload file does not even exist here).
#[test]
fn the_false_knock_out_guard_holds_at_load() {
    let guard = |directory: &Path| {
        let mut value = manifest_of(directory);
        let owner = &mut value["owners"][0];
        let label = owner["class"].as_str().unwrap().to_owned();
        owner["lane"] = "custom".into();
        owner["roles"] = "none".into();
        owner["edge_maps"] = serde_json::json!([]);
        owner["terminals"] = serde_json::json!([]);
        owner["payload"] = serde_json::json!({"file": format!("owners/{label}.rrbin"), "bytes": 4});
        owner["custom"] =
            serde_json::json!({"lp_count": 1, "residuals": 0, "two_sided_axes": 0, "charts": [1]});
        write_manifest(directory, &value);
    };
    malformed(damaged("guard", guard), "false knock-out guard");
    let (_scratch, directory) = written(scope_two(), "guard-reuse");
    guard(&directory);
    assert!(matches!(
        read_custom_payloads(&PLibrarySource::Directory(directory)),
        Err(PError::MalformedLibrary(m)) if m.contains("false knock-out guard")
    ));
}

/// Edits that keep the versions: an identity that is not the program's,
/// counts that are not the files', an unknown field, a terminal or edge map
/// out of place, a file name leaving the package.
#[test]
fn an_inconsistent_manifest_is_refused() {
    let edit = |label: &str, edit: &dyn Fn(&mut serde_json::Value)| {
        damaged(label, |directory| {
            let mut value = manifest_of(directory);
            edit(&mut value);
            write_manifest(directory, &value);
        })
    };
    malformed(
        edit("identity", &|v| {
            v["identity"] = v["identity"]
                .as_str()
                .unwrap()
                .replace("scope=1-2", "scope=1-3")
                .into()
        }),
        "records the identity",
    );
    malformed(
        edit("counts", &|v| v["counts"]["sectors"] = 1.into()),
        "the counts disagree",
    );
    malformed(
        edit("unknown-field", &|v| v["extra"] = true.into()),
        "unknown field",
    );
    malformed(
        edit("terminal", &|v| {
            v["terminals"].as_array_mut().unwrap().push("unit".into())
        }),
        "terminal 1 repeats",
    );
    malformed(
        edit("escape", &|v| {
            v["files"]["families"] = "../p-families.json".into()
        }),
        "families file",
    );
    malformed(
        edit("escape-family", &|v| {
            v["presentations"][0]["family"] = "../../etc/passwd".into()
        }),
        "does not list",
    );
    malformed(
        edit("node", &|v| v["nodes"][0]["presentation"] = 3.into()),
        "node record 0",
    );
}

/// `layer/p-layer.json` of a package, edited as JSON.
fn edit_layer(directory: &Path, edit: &dyn Fn(&mut serde_json::Value)) {
    let path = directory.join(LAYER_FILE);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    edit(&mut value);
    std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

/// The basis layer travels as `layer/p-layer.json` (M2-T9's format): read back
/// it is the assembled program's layer and the loaded program's map. Another
/// layer schema or bound version is refused by name; a row that is no valid
/// layer row, a terminal inventory that is not the program's, a missing or
/// renamed layer file are refused; a package without its layer is another
/// program (no basis map).
#[test]
fn the_basis_layer_travels_and_fails_closed() {
    let assembled = scope_two();
    let layer = PBasisLayer::from_library(&assembled.library)
        .unwrap()
        .expect("the packaged program has a basis layer");
    assert_eq!(layer.rows().len(), assembled.library.terminals().len());
    let (_scratch, directory) = written(assembled, "layer");
    let text = std::fs::read_to_string(directory.join(LAYER_FILE)).unwrap();
    assert_eq!(text, layer.to_json());
    let loaded = load(&directory).unwrap();
    assert_eq!(PBasisLayer::from_library(&loaded).unwrap(), Some(layer));
    assert_eq!(
        manifest_of(&directory)["files"]["layer"].as_str(),
        Some(LAYER_FILE)
    );

    let layer_edit = |label: &str, edit: &dyn Fn(&mut serde_json::Value)| {
        damaged(label, |directory| edit_layer(directory, edit))
    };
    version_mismatch(
        layer_edit("layer-schema", &|v| {
            v["schema"] = "rustred.p-basis-layer.v0".into()
        })
        .unwrap_err(),
        "P_BASIS_LAYER_SCHEMA",
        "rustred.p-basis-layer.v0",
    );
    version_mismatch(
        layer_edit("layer-census", &|v| {
            v["base"]["P_CENSUS_VERSION"] = "0".into()
        })
        .unwrap_err(),
        "P_CENSUS_VERSION",
        "0",
    );
    malformed(
        layer_edit("layer-unit", &|v| {
            v["rows"][0]["terms"][0]["coefficient"] = "2".into()
        }),
        "Unit row",
    );
    malformed(
        layer_edit("layer-node", &|v| {
            let node = v["rows"][0]["terminal"]["node"].as_u64().unwrap();
            v["rows"][0]["terminal"]["node"] = (node + 1).into();
        }),
        "basis layer row 0 names",
    );
    malformed(
        layer_edit("layer-terminals", &|v| v["base"]["terminals"] = 2.into()),
        "terminals",
    );
    malformed(
        damaged("layer-missing", |directory| {
            std::fs::remove_file(directory.join(LAYER_FILE)).unwrap()
        }),
        "missing file layer/p-layer.json",
    );
    malformed(
        damaged("layer-renamed", |directory| {
            let mut value = manifest_of(directory);
            value["files"]["layer"] = "layer/other.json".into();
            write_manifest(directory, &value);
        }),
        "layer file",
    );
    // Without its layer, the package is a program without a basis map.
    let unmapped = damaged("layer-dropped", |directory| {
        let mut value = manifest_of(directory);
        value["files"]["layer"] = serde_json::Value::Null;
        write_manifest(directory, &value);
        std::fs::remove_file(directory.join(LAYER_FILE)).unwrap();
    })
    .unwrap();
    assert!(unmapped.basis_map().is_none());
    assert_eq!(
        p_library_differences(&assembled.library, &unmapped).unwrap(),
        vec!["basis map on one side only".to_owned()]
    );
}

// ---------------------------------------------------------------------------
// Custom owners
// ---------------------------------------------------------------------------

/// Scope 1-2 with the 2L sunset owner (a SeriesParallel owner on three lines,
/// LP = 1) replaced by a generated Custom owner on its presentation: payload,
/// meta and residual terminal, its edge map dropped. Built with the library's
/// builder, without a basis map (the layer of M2-T9 has no master for this
/// residual); it passes the descent certificate on load.
fn with_custom_sunset() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let assembled = scope_two();
        let library = &assembled.library;
        let census = library.census();
        let target = library
            .owners()
            .map(|owner| &owner.spec)
            .find(|spec| {
                let record = &census.classes[&spec.class];
                spec.lane == OwnerLane::SeriesParallel && record.key.loops == 2 && record.t == 3
            })
            .unwrap()
            .clone();
        let label = census.classes[&target.class].key.label();
        let payload = generate_custom_owner(
            library.presentation(target.presentation),
            &CustomOwnerOptions::default(),
        )
        .unwrap();
        assert_eq!((payload.meta.lp_count, payload.meta.residuals), (1, 1));

        let mut builder = PLibrary::builder(library.coefficient_context().clone(), library.scope());
        for presentation in library.presentations() {
            builder
                .add_presentation(Presentation::clone(presentation))
                .unwrap();
        }
        builder
            .set_topology(Arc::clone(library.topology().unwrap()))
            .unwrap();
        let mut renumbered = BTreeMap::new();
        for (index, map) in library.edge_maps().iter().enumerate() {
            if !target.edge_maps.contains(&index) {
                renumbered.insert(index, builder.add_edge_map(map.clone()).unwrap());
            }
        }
        for terminal in library.terminals() {
            builder.add_terminal(terminal.clone()).unwrap();
        }
        let summary = rustred::propagator::custom::inspect_custom_payload(&payload.bytes).unwrap();
        let residuals = summary
            .residual_keys()
            .into_iter()
            .map(|key| {
                builder
                    .add_terminal(PTerminal::Residual {
                        owner: target.class,
                        key: IntegralKey::try_new(key).unwrap(),
                    })
                    .unwrap()
            })
            .collect::<Vec<PTerminalId>>();
        builder.set_census(census.clone()).unwrap();
        for owner in library.owners() {
            let mut spec = owner.spec.clone();
            if spec.class == target.class {
                spec = POwnerSpec {
                    lane: OwnerLane::Custom,
                    roles: LaneRoles::None,
                    edge_maps: Vec::new(),
                    payload: Some(CustomPayloadRef::Bytes(Arc::clone(&payload.bytes))),
                    custom: Some(payload.meta.clone()),
                    terminals: residuals.clone(),
                    ..spec
                };
            } else {
                spec.edge_maps = spec.edge_maps.iter().map(|e| renumbered[e]).collect();
            }
            builder.add_owner(spec).unwrap();
        }
        let mut nodes = assembled.nodes.clone();
        let node = nodes
            .iter_mut()
            .find(|node| node.presentation == Some(target.presentation))
            .unwrap();
        node.decision.lane = OwnerLane::Custom;
        node.decision.roles = LaneRoles::None;
        AssembledLibrary {
            library: Arc::new(builder.build().unwrap()),
            families: assembled.families.clone(),
            max_loops: assembled.max_loops,
            dag: Arc::clone(&assembled.dag),
            nodes,
            custom: BTreeMap::from([(label, payload)]),
            descent: None,
            timings: Vec::new(),
        }
    })
}

const UNCHECKED: PLibraryOptions = PLibraryOptions {
    verify_descent: false,
};

/// A Custom owner travels as its candidate bundle: `owners/<label>.rrbin`
/// verbatim, its meta and residual terminal in `library.json`; the loaded
/// program passes the descent certificate (the Custom witness boxes run the
/// payload's step), is equivalent (payloads compared through their canonical
/// dumps) and reduces random keys as the assembled one, through the Custom
/// owner onto its residual; `read_custom_payloads` returns the payload and
/// meta, and a second rendering is byte-identical.
#[test]
fn a_custom_owner_travels_with_its_payload() {
    let assembled = with_custom_sunset();
    let (label, payload) = assembled.custom.first_key_value().unwrap();
    let (_scratch, directory) = written(assembled, "custom");
    let files = tree(&directory);
    let file = format!("owners/{label}.rrbin");
    assert_eq!(files[&file], payload.bytes.as_ref());
    assert!(!files.contains_key(LAYER_FILE));
    assert_eq!(
        p_library_files(assembled).unwrap(),
        files.clone().into_iter().collect::<Vec<_>>()
    );
    let loaded = load(&directory).unwrap();
    assert_eq!(
        p_library_differences(&assembled.library, &loaded).unwrap(),
        Vec::<String>::new()
    );
    let from_memory = load_p_library(memory(&files), &VERIFY).unwrap();
    assert!(equivalent_p_libraries(&loaded, &from_memory).unwrap());

    // Reductions through the Custom owner: the residual terminal is reached.
    let residual = (assembled.library.terminals().iter())
        .position(|terminal| matches!(terminal, PTerminal::Residual { .. }))
        .map(|index| PTerminalId(index as u32))
        .unwrap();
    let requests = random_requests(assembled, |_| keys(60));
    let expected = reductions(&assembled.library, &requests, false);
    assert_eq!(reductions(&loaded, &requests, false), expected);
    assert_eq!(reductions(&from_memory, &requests, false), expected);
    let through_custom = (expected.iter())
        .filter(|reduction| {
            (reduction.terms.iter()).any(|term| term.basis == PBasis::Residual(residual))
        })
        .count();
    println!(
        "Custom sunset: {} requests identical, {through_custom} reach its residual terminal {}",
        requests.len(),
        residual.0
    );
    assert!(through_custom >= 5, "{through_custom}");
    assert_eq!(
        read_custom_payloads(&PLibrarySource::Directory(directory)).unwrap(),
        assembled.custom
    );
    // Another payload under the same label is another program.
    let manifest: LibraryManifest = serde_json::from_slice(&files[LIBRARY_FILE]).unwrap();
    let owner = manifest.owners.iter().find(|o| &o.class == label).unwrap();
    println!(
        "Custom sunset {label}: payload {} bytes, meta {:?}, terminals {:?}",
        payload.bytes.len(),
        owner.custom,
        owner.terminals
    );
    assert!(!equivalent_p_libraries(&scope_two().library, &loaded).unwrap());
}

/// A payload that its record does not describe is refused at load, before any
/// program is built: another LP/residual record (the guard), other charts,
/// another residual terminal, a payload of another family, a payload of
/// another Symbolica save format.
#[test]
fn a_payload_its_record_does_not_describe_is_refused() {
    let edit_owner = |label: &str, edit: &dyn Fn(&mut serde_json::Value, &Path)| {
        let (_scratch, directory) = written(with_custom_sunset(), label);
        let mut value = manifest_of(&directory);
        let owner = value["owners"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|owner| owner["lane"] == "custom")
            .unwrap();
        edit(owner, &directory);
        write_manifest(&directory, &value);
        load_p_library(PLibrarySource::Directory(directory), &UNCHECKED)
    };
    malformed(
        edit_owner("custom-guard", &|owner, _| {
            owner["custom"]["residuals"] = 0.into()
        }),
        "false knock-out guard",
    );
    malformed(
        edit_owner("custom-charts", &|owner, _| {
            owner["custom"]["charts"] = serde_json::json!([1])
        }),
        "records",
    );
    malformed(
        edit_owner("custom-two-sided", &|owner, _| {
            owner["custom"]["two_sided_axes"] = 1.into()
        }),
        "records",
    );
    malformed(
        edit_owner("custom-length", &|owner, _| {
            owner["payload"]["bytes"] = (owner["payload"]["bytes"].as_u64().unwrap() + 1).into()
        }),
        "bytes, library.json records",
    );
    malformed(
        edit_owner("custom-junk", &|owner, directory| {
            let file = directory.join(owner["payload"]["file"].as_str().unwrap());
            let length = std::fs::read(&file).unwrap().len();
            std::fs::write(&file, vec![0_u8; length]).unwrap();
        }),
        "owners/",
    );
    // Another family: the payload moved onto another presentation of three
    // lines (the record follows, so only the binding is wrong).
    let other = scope_two()
        .library
        .presentations()
        .iter()
        .find(|p| {
            let sunset = with_custom_sunset()
                .library
                .owners()
                .find(|o| o.spec.lane == OwnerLane::Custom)
                .unwrap()
                .spec
                .presentation;
            p.id != sunset
                && p.slots.len()
                    == with_custom_sunset()
                        .library
                        .presentation(sunset)
                        .slots
                        .len()
        })
        .map(|p| p.id.0)
        .expect("another presentation of three lines");
    malformed(
        edit_owner("custom-family", &|owner, _| {
            owner["presentation"] = other.into()
        }),
        "bound to another family",
    );
    // The residual terminal dropped from the owner.
    let result = edit_owner("custom-residual", &|owner, _| {
        owner["terminals"] = serde_json::json!([])
    });
    malformed(result, "residuals");
}

// ---------------------------------------------------------------------------
// Scope 1-3: the built-in library (slow: its assembly generates the Custom
// owners NO and t1star05)
// ---------------------------------------------------------------------------

/// The scope 1-3 program of `rustred p-package --max-loops 3`, assembled once
/// per test process.
pub(crate) fn scope_three() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let start = Instant::now();
        let assembled = assemble(3);
        println!(
            "scope 1-3 assembled with its basis layer in {:.1} s",
            start.elapsed().as_secs_f64()
        );
        assembled
    })
}

/// Round-trip keys at scope 1-3: p-validate's bounds and 100 keys at 1L and
/// 2L; at 3L 60 keys of rank <= 4 and dots <= 2 (3L reductions at rank 8 are
/// slow in debug); every top's corner (lines at 1, ISPs at 0), which reaches
/// the full sectors of the Custom owners.
fn scope_three_requests(assembled: &AssembledLibrary) -> Vec<Inputs> {
    let mut requests = random_requests(assembled, |loops| match loops {
        3 => ValidateOptions {
            points: 60,
            max_dots: 2,
            max_rank: 4,
            ..ValidateOptions::default()
        },
        _ => keys(100),
    });
    requests.extend(corners(assembled).into_iter().map(|(_, inputs)| inputs));
    requests
}

/// Every top's corner (lines at 1, ISPs at 0) as a request, with its loop count.
fn corners(assembled: &AssembledLibrary) -> Vec<(u8, Inputs)> {
    let library = &assembled.library;
    let mut out = Vec::new();
    for loops in 1..=assembled.max_loops {
        for top in assembled.tops_of(loops) {
            let corner = (library.presentation(top).slots.iter())
                .map(|slot| i64::from(matches!(slot, Slot::Line(_))))
                .collect::<Vec<_>>();
            let key = IntegralKey::try_new(corner).unwrap();
            out.push((loops, vec![(top, key, library.coefficient_context().one())]));
        }
    }
    out
}

/// The terminals and masters `reductions` reach, with how many reductions
/// reach each.
fn reached(reductions: &[PReduction]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for reduction in reductions {
        let mut names: Vec<String> = (reduction.terms.iter())
            .map(|term| match &term.basis {
                PBasis::Unit => "unit".to_owned(),
                PBasis::Master(name) => format!("master {name}"),
                PBasis::Residual(id) => format!("residual {}", id.0),
            })
            .collect();
        names.sort();
        names.dedup();
        for name in names {
            *out.entry(name).or_default() += 1;
        }
    }
    out
}

/// The built-in scope 1-3 program written and loaded back, from the directory
/// (without the certificate, whose parts are then timed) and from embedded
/// bytes (with it): structurally
/// equivalent (payloads through their canonical dumps, layer rows), the same
/// identity and step-cache binding, the same reductions on random keys and top
/// corners onto the terminals (both Custom residuals reached) and onto the
/// masters (`no` and `t105` reached, no residual left). A second rendering is
/// byte-identical. Prints the sizes (target <= 5 MB, plan M2 risk 14) and the
/// load times.
#[test]
#[ignore = "slow: scope 1-3 assembly generates the Custom owners NO and t1star05"]
fn scope_one_to_three_round_trips_through_a_directory_and_embedded_bytes() {
    let assembled = scope_three();
    let library = &assembled.library;
    let custom: Vec<_> = (library.owners())
        .filter(|owner| owner.spec.lane == OwnerLane::Custom)
        .map(|owner| library.census().classes[&owner.spec.class].key.label())
        .collect();
    assert_eq!(custom.len(), 2, "{custom:?}");
    let layer = PBasisLayer::from_library(library).unwrap().unwrap();
    assert_eq!(layer.rows().len(), 3);
    assert_eq!(layer.masters(), ["no", "t105"]);

    let start = Instant::now();
    let files = p_library_files(assembled).unwrap();
    let written_layer = (files.iter())
        .find(|(name, _)| name == LAYER_FILE)
        .map(|(_, bytes)| String::from_utf8(bytes.clone()).unwrap())
        .unwrap();
    assert_eq!(written_layer, layer.to_json());
    let rows: serde_json::Value = serde_json::from_str(&written_layer).unwrap();
    assert_eq!(rows["rows"].as_array().unwrap().len(), 3);
    let rendered = start.elapsed().as_secs_f64();
    assert_eq!(p_library_files(assembled).unwrap(), files);
    let mut groups: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (name, bytes) in &files {
        println!("{:>9}  {name}", bytes.len());
        let group = match name.as_str() {
            n if n.starts_with("families/") && n.ends_with(".rrbin") => "families/*.rrbin",
            n if n.starts_with("owners/") => "owners/*.rrbin",
            n => n,
        };
        let entry = groups.entry(group).or_default();
        entry.0 += 1;
        entry.1 += bytes.len();
    }
    let total: usize = files.iter().map(|(_, bytes)| bytes.len()).sum();
    for (group, (count, bytes)) in &groups {
        println!("group {group}: {count} files, {bytes} bytes");
    }
    println!(
        "{total:>9}  total, {} files (scope 1-3), rendered in {rendered:.2} s",
        files.len()
    );
    assert!(groups.contains_key(LAYER_FILE));
    for label in &custom {
        assert!(
            files
                .iter()
                .any(|(name, _)| *name == format!("owners/{label}.rrbin")),
            "{label}"
        );
    }

    let (_scratch, directory) = written(assembled, "scope-three");
    let on_disk = tree(&directory);
    assert_eq!(on_disk, files.iter().cloned().collect::<BTreeMap<_, _>>());
    // From the directory without the certificate, then its three parts timed
    // one by one; from memory with the certificate (the loader's own order).
    let start = Instant::now();
    let from_directory =
        load_p_library(PLibrarySource::Directory(directory.clone()), &UNCHECKED).unwrap();
    let unchecked_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    descent::check(&from_directory).unwrap();
    let descent_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    (from_directory.census())
        .check_complete(
            from_directory.presentations(),
            from_directory.topology().map(Arc::as_ref),
        )
        .unwrap();
    let census_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let derived = derive_basis_layer(&from_directory, &PBasisOptions::default()).unwrap();
    assert_eq!(
        Some(derived.layer),
        PBasisLayer::from_library(&from_directory).unwrap()
    );
    let layer_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let from_memory = load_p_library(memory(&on_disk), &VERIFY).unwrap();
    let memory_seconds = start.elapsed().as_secs_f64();
    println!(
        "loaded from the directory without the certificate in {unchecked_seconds:.2} s; certificate parts: descent {descent_seconds:.2} s, census completeness {census_seconds:.2} s, layer re-derivation {layer_seconds:.2} s; from memory with the certificate {memory_seconds:.2} s"
    );
    for loaded in [&from_directory, &from_memory] {
        assert_eq!(
            p_library_differences(library, loaded).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            PBasisLayer::from_library(loaded).unwrap().as_ref(),
            Some(&layer)
        );
        assert_eq!(loaded.identity(), library.identity());
        assert_eq!(
            loaded.step_cache_binding().unwrap(),
            library.step_cache_binding().unwrap()
        );
    }

    let requests = scope_three_requests(assembled);
    let start = Instant::now();
    let expected = reductions(library, &requests, false);
    let seconds = start.elapsed().as_secs_f64();
    assert_eq!(reductions(&from_directory, &requests, false), expected);
    assert_eq!(reductions(&from_memory, &requests, false), expected);
    let terminals = reached(&expected);
    let expected_masters = reductions(library, &requests, true);
    assert_eq!(
        reductions(&from_directory, &requests, true),
        expected_masters
    );
    assert_eq!(reductions(&from_memory, &requests, true), expected_masters);
    let masters = reached(&expected_masters);
    println!(
        "round trip scope 1-3: {} requests identical onto the terminals {terminals:?} and onto the masters {masters:?}; assembled reductions {seconds:.1} s",
        requests.len()
    );
    assert!(terminals.contains_key("residual 1") && terminals.contains_key("residual 2"));
    assert!(masters.contains_key("master no") && masters.contains_key("master t105"));
    assert!(
        !masters.keys().any(|name| name.starts_with("residual")),
        "{masters:?}"
    );

    // The modular arithmetics on one thread, on every top's corner (the 3L
    // corners reach NO and t1star05) and on two random keys per loop count
    // with at least two terms onto the masters: the same reductions from the
    // three programs, the exact terms.
    let mut modular_requests: Vec<_> = corners(assembled)
        .into_iter()
        .map(|(_, inputs)| inputs)
        .collect();
    for loops in 1..=assembled.max_loops {
        modular_requests.extend(
            (requests.iter().zip(&expected_masters))
                .filter(|(inputs, reduction)| {
                    inputs.len() == 1
                        && library.presentation(inputs[0].0).loops == loops
                        && reduction.terms.len() >= 2
                })
                .take(2)
                .map(|(inputs, _)| inputs.clone()),
        );
    }
    let mut per_loop: BTreeMap<u8, usize> = BTreeMap::new();
    for inputs in &modular_requests {
        *per_loop
            .entry(library.presentation(inputs[0].0).loops)
            .or_default() += 1;
    }
    assert!(per_loop.len() == 3 && per_loop[&3] >= 5, "{per_loop:?}");
    let exact = reductions(library, &modular_requests, true);
    for (name, arithmetic) in modular_arithmetics() {
        let start = Instant::now();
        let modular = reductions_with(library, &modular_requests, true, arithmetic);
        let seconds = start.elapsed().as_secs_f64();
        for loaded in [&from_directory, &from_memory] {
            assert_eq!(
                reductions_with(loaded, &modular_requests, true, arithmetic),
                modular,
                "{name}"
            );
        }
        for (modular, exact) in modular.iter().zip(&exact) {
            assert_eq!(modular.terms, exact.terms, "{name}");
        }
        println!(
            "{name} (1 thread): {} requests (per loop count {per_loop:?}: corners and random keys) identical from the three programs and equal to Exact onto {:?}; assembled {seconds:.1} s",
            modular_requests.len(),
            reached(&modular)
        );
    }
}

/// A disk step cache filled through the assembled scope 1-3 program, saved and
/// reopened, serves the loaded program: the 3L corner input that reaches the
/// Custom owner NO (dotted and plain) replays without a miss, with the terms
/// of the first run. Prints the hit and miss counts.
#[test]
#[ignore = "slow: scope 1-3 assembly generates the Custom owners NO and t1star05"]
fn a_disk_step_cache_of_the_assembled_scope_three_program_serves_the_loaded_one() {
    let assembled = scope_three();
    let library = &assembled.library;
    let context = library.coefficient_context();
    let (scratch, directory) = written(assembled, "disk-cache");
    let loaded = load(&directory).unwrap();
    let binding = library.step_cache_binding().unwrap();
    assert_eq!(loaded.step_cache_binding().unwrap(), binding);

    // The 3L top whose corner reaches NO's residual, its corner and a dotted
    // corner in one input.
    let no = (library.terminals().iter())
        .position(|terminal| match terminal {
            PTerminal::Residual { owner, .. } => {
                library.census().classes[owner].key.loops == 3 && {
                    let spec = &library
                        .owners()
                        .find(|o| o.spec.class == *owner)
                        .unwrap()
                        .spec;
                    spec.custom
                        .as_ref()
                        .is_some_and(|meta| meta.two_sided_axes == 0)
                }
            }
            PTerminal::Unit => false,
        })
        .map(|index| PTerminalId(index as u32))
        .unwrap();
    let (top, corner) = (corners(assembled).into_iter())
        .filter(|(loops, _)| *loops == 3)
        .map(|(_, inputs)| inputs[0].clone())
        .map(|(top, key, _)| (top, key.powers().to_vec()))
        .find(|(top, corner)| {
            let reduction = (library.reduce(request(
                vec![(
                    *top,
                    IntegralKey::try_new(corner.clone()).unwrap(),
                    context.one(),
                )],
                false,
            )))
            .unwrap();
            (reduction.terms.iter()).any(|term| term.basis == PBasis::Residual(no))
        })
        .unwrap();
    let mut dotted = corner.clone();
    dotted[0] += 1;
    let input = vec![
        (top, IntegralKey::try_new(corner).unwrap(), context.one()),
        (
            top,
            IntegralKey::try_new(dotted).unwrap(),
            context.integer(2),
        ),
    ];
    let cached = |program: &PLibrary, cache: &Arc<OrderedStepCache>| {
        let mut run = request(input.clone(), false);
        run.step_cache = Some(Arc::clone(cache));
        program.reduce(run).unwrap()
    };

    let cache_directory = scratch.join("step-cache");
    let cache =
        OrderedStepCache::open(&cache_directory, binding.clone(), Default::default()).unwrap();
    let first = cached(library, &cache);
    let saved = cache.save().unwrap();
    let filled = cache.statistics();
    drop(cache);
    assert!(
        (first.terms.iter()).any(|term| term.basis == PBasis::Residual(no)),
        "{:?}",
        first.terms
    );

    let reopened = OrderedStepCache::open(
        &cache_directory,
        loaded.step_cache_binding().unwrap(),
        Default::default(),
    )
    .unwrap();
    let replay = cached(&loaded, &reopened);
    let progress = replay.statistics.progress;
    let statistics = reopened.statistics();
    println!(
        "disk step cache, scope 1-3, {top:?} corner + dotted corner (reaches NO): assembled run {} hits {} misses, \
         {} records saved ({saved:?}); loaded program on the reopened cache {} hits {} misses (cache: {} hits, {} \
         misses, {} loaded shards, {} file bytes; tail {:?})",
        first.statistics.progress.step_cache_hits,
        first.statistics.progress.step_cache_misses,
        filled.records,
        progress.step_cache_hits,
        progress.step_cache_misses,
        statistics.hits,
        statistics.misses,
        statistics.loaded_shards,
        statistics.file_bytes,
        reopened.tail_statistics()
    );
    assert!(first.statistics.progress.step_cache_misses > 0);
    assert_eq!(replay.terms, first.terms);
    assert_eq!(replay.nonzero_conditions, first.nonzero_conditions);
    assert_eq!(progress.step_cache_misses, 0, "{progress:?}");
    assert!(progress.step_cache_hits > 0, "{progress:?}");
}

/// The real Custom payloads and the scope 1-3 layer fail closed: the NO and
/// t1star05 payload files swapped (their recorded lengths following) are
/// refused as payloads their records do not describe; layer rows naming each
/// other's terminals fail the inventory check; layer rows with each other's
/// terms (a consistent inventory, wrong masters) load only without the
/// certificate, which re-derives the layer and refuses them.
#[test]
#[ignore = "slow: scope 1-3 assembly generates the Custom owners NO and t1star05"]
fn scope_one_to_three_payloads_and_layer_fail_closed() {
    let assembled = scope_three();
    let fresh = |label: &str| written(assembled, label);

    let (_scratch, directory) = fresh("swap-payloads");
    let mut value = manifest_of(&directory);
    let payloads: Vec<(usize, String, u64)> = (value["owners"].as_array().unwrap().iter())
        .enumerate()
        .filter(|(_, owner)| !owner["payload"].is_null())
        .map(|(index, owner)| {
            let payload = &owner["payload"];
            (
                index,
                payload["file"].as_str().unwrap().to_owned(),
                payload["bytes"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(payloads.len(), 2);
    let (a, b) = (&payloads[0], &payloads[1]);
    let bytes_a = std::fs::read(directory.join(&a.1)).unwrap();
    let bytes_b = std::fs::read(directory.join(&b.1)).unwrap();
    std::fs::write(directory.join(&a.1), &bytes_b).unwrap();
    std::fs::write(directory.join(&b.1), &bytes_a).unwrap();
    value["owners"][a.0]["payload"]["bytes"] = b.2.into();
    value["owners"][b.0]["payload"]["bytes"] = a.2.into();
    write_manifest(&directory, &value);
    malformed(
        load_p_library(PLibrarySource::Directory(directory), &UNCHECKED),
        "the payload of",
    );

    let swap_rows = |label: &str, part: &str| {
        let (scratch, directory) = fresh(label);
        edit_layer(&directory, &|v| {
            let rows = v["rows"].as_array_mut().unwrap();
            let first = rows[1][part].clone();
            rows[1][part] = rows[2][part].clone();
            rows[2][part] = first;
        });
        (scratch, directory)
    };
    let (_scratch, directory) = swap_rows("swap-layer-terminals", "terminal");
    malformed(
        load_p_library(PLibrarySource::Directory(directory), &UNCHECKED),
        "basis layer row 1",
    );
    let (_scratch, directory) = swap_rows("swap-layer-terms", "terms");
    let unchecked =
        load_p_library(PLibrarySource::Directory(directory.clone()), &UNCHECKED).unwrap();
    let differences = p_library_differences(&assembled.library, &unchecked).unwrap();
    assert!(
        differences.len() == 2
            && differences[0].starts_with("basis row of terminal 1 (residual")
            && differences[1].starts_with("basis row of terminal 2 (residual"),
        "{differences:?}"
    );
    malformed(
        load(&directory),
        "row 1 is not the row this library derives",
    );
}

/// M5 batch 2 (m5-a2-canon): the saved automorphism groups (`orbits.rrbin`)
/// restore exactly the groups the run-time search authenticates: with orbit
/// canonicalisation on, a library with them reduces every request to the same
/// result (terms, conditions and path statistics) as without them, and to the
/// same terms as with canonicalisation off. They are not in the identity; a
/// verified load ignores them.
#[test]
fn saved_orbits_restore_the_searched_groups_and_change_no_value() {
    use rustred_app::propagator::ordering::{write_orbits, write_routes};
    let assembled = scope_two();
    let requests = random_requests(assembled, |_| keys(60));
    let fast = PLibraryOptions {
        verify_descent: false,
    };
    let (_scratch, directory) = written(assembled, "saved-orbits");
    write_routes(&directory).unwrap();
    let plain = load_p_library(PLibrarySource::Directory(directory.clone()), &fast).unwrap();
    assert!(plain.saved_orbits().is_none());
    let (saved, nontrivial) = write_orbits(&directory).unwrap();
    assert!(
        saved > 0 && nontrivial > 0,
        "{saved} saved, {nontrivial} nontrivial"
    );
    let loaded = load_p_library(PLibrarySource::Directory(directory.clone()), &fast).unwrap();
    assert_eq!(
        loaded.saved_orbits().map(|orbits| orbits.len()),
        Some(saved)
    );
    assert_eq!(loaded.identity(), plain.identity());
    // Saved groups switch canonicalisation on by default (M5 batch 2); a
    // library without them keeps the old default and step identity.
    assert_eq!(
        plain.orbit_canonicalization(),
        rustred::propagator::P_ORBIT_CANON_DEFAULT
    );
    assert_eq!(
        loaded.orbit_canonicalization(),
        rustred::propagator::P_ORBIT_CANON_WITH_SAVED_ORBITS
    );
    loaded.set_orbit_canonicalization(plain.orbit_canonicalization());
    assert_eq!(loaded.step_identity(), plain.step_identity());
    let off = reductions(&plain, &requests, false);
    for library in [&plain, &loaded] {
        library.set_orbit_canonicalization(true);
    }
    let searched = reductions(&plain, &requests, false);
    let restored = reductions(&loaded, &requests, false);
    assert_eq!(restored, searched);
    for (on, off) in restored.iter().zip(&off) {
        assert_eq!(on.terms, off.terms);
    }
    assert!(
        restored
            .iter()
            .zip(&off)
            .any(|(on, off)| on.statistics != off.statistics),
        "canonicalisation changed no path: the test exercises no group"
    );
    let verified = load(&directory).unwrap();
    assert!(verified.saved_orbits().is_none());
    assert_eq!(
        verified.orbit_canonicalization(),
        rustred::propagator::P_ORBIT_CANON_DEFAULT
    );
}

/// p artefact v2 (M5 m5-1l3l, notes/artifact-ordering.SPEC.md): an ordering
/// under every heuristic, and the saved route verifications, give exactly the
/// reductions of the unordered program; the identity carries the ordering
/// tag; a verified load ignores the saved routes and keeps the ordering; a
/// damaged rule order is refused when its owner is built.
#[test]
fn an_ordering_and_saved_routes_change_no_value() {
    use rustred_app::propagator::ordering::{ORDERING_FILE, write_ordering, write_routes};
    // Scope 1-3: the first scope with Custom owners (NO, t1star05), so the
    // ordering has rule orders to apply (scope 1-2 has none).
    let assembled = scope_three();
    let requests = scope_three_requests(assembled);
    let expected = reductions(&assembled.library, &requests, false);
    let fast = PLibraryOptions {
        verify_descent: false,
    };
    for heuristic in ["generation", "fewest-terms"] {
        let (_scratch, directory) = written(assembled, heuristic);
        let (saved, _) = write_routes(&directory).unwrap();
        assert!(saved > 0);
        let ordering = write_ordering(&directory, heuristic).unwrap();
        assert!(ordering.owners.iter().any(|owner| !owner.charts.is_empty()));
        let loaded = load_p_library(PLibrarySource::Directory(directory.clone()), &fast).unwrap();
        assert_eq!(
            loaded.identity(),
            format!(
                "{},P_ORDERING_VERSION=1:{heuristic}",
                assembled.library.identity()
            )
        );
        assert_eq!(
            loaded.saved_routes().map(|routes| routes.len()),
            Some(saved)
        );
        assert_eq!(reductions(&loaded, &requests, false), expected);
        let verified = load(&directory).unwrap();
        assert!(verified.saved_routes().is_none());
        assert_eq!(verified.identity(), loaded.identity());
        assert_eq!(
            reductions(&verified, &requests[..20], false),
            expected[..20]
        );
    }
    let (_scratch, directory) = written(assembled, "damaged-order");
    write_ordering(&directory, "generation").unwrap();
    let mut file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join(ORDERING_FILE)).unwrap()).unwrap();
    let owner = (file["owners"].as_array_mut().unwrap().iter_mut())
        .find(|owner| {
            owner["charts"][0]["order"]
                .as_array()
                .is_some_and(|o| o.len() >= 2)
        })
        .unwrap();
    let order = owner["charts"][0]["order"].as_array_mut().unwrap();
    order[0] = order[1].clone();
    std::fs::write(
        directory.join(ORDERING_FILE),
        serde_json::to_vec(&file).unwrap(),
    )
    .unwrap();
    let damaged = load_p_library(PLibrarySource::Directory(directory), &fast).unwrap();
    let refused = requests.iter().any(|inputs| {
        matches!(
            damaged.reduce(request(inputs.clone(), false)),
            Err(PError::MalformedLibrary(message)) if message.contains("rule order")
        )
    });
    assert!(refused, "no request reached the damaged owner");
}
