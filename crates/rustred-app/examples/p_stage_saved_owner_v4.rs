//! Strict v4 native owner staging; all package version guards stay authoritative.
//! It never reads or relabels a historical <owner>.json generation record.
//! Usage: p_stage_saved_owner_v4 BASE PAYLOAD OUT TAG RULES PAYLOAD_SOURCE OWNER EXPECTED_BLAKE3 [--allow-programmed-order]
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Component, Path, PathBuf};

use rustred::propagator::custom::{CustomPayloadSummary, inspect_custom_payload};
use rustred::propagator::library::{OwnerProgram, PLibrary};
use rustred::propagator::programs::lane_program;
use rustred::propagator::step::{PKeyView, POrderKey, PStepContext};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::{OwnerClassId, P_ORDERING_VERSION};
use rustred::reduction::ReductionLimits;
use rustred::sector::OrderingPolicy;
use rustred_app::propagator::{
    PLibraryOptions, PLibrarySource,
    format::{CustomRecord, LIBRARY_FILE, LaneRecord, LibraryManifest},
    load_p_library,
    loader::read_p_library_manifest,
    ordering::{OrderingFile, compute_ordering, ordered_identity},
};

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const ASTRA_ROOT: &str = "/shared/vhirschi/t3code/astra_5L";
const STOCK_ORDER: &str = "rustred.spired-uncut-sector-order.v1";
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn require(ok: bool, message: &str) -> Result<()> {
    if !ok {
        return Err(message.to_owned().into());
    }
    Ok(())
}

fn arguments(mut args: Vec<String>) -> Result<(Vec<String>, bool)> {
    let allow = args.last().is_some_and(|a| a == "--allow-programmed-order");
    if allow {
        args.pop();
    }
    require(
        args.len() == 9 && !args.iter().skip(1).any(|a| a.starts_with("--")),
        "usage: p_stage_saved_owner_v4 BASE PAYLOAD OUT TAG RULES PAYLOAD_SOURCE OWNER EXPECTED_BLAKE3 [--allow-programmed-order]",
    )?;
    Ok((args, allow))
}

fn check_ordering(old: &str, new: &str, arity: usize, allow: bool) -> Result<()> {
    require(
        old == STOCK_ORDER,
        "BASE must retain its stock SpiredUncut owner order",
    )?;
    if new == STOCK_ORDER {
        return Ok(());
    }
    require(
        allow,
        "non-stock candidate order requires explicit --allow-programmed-order",
    )?;
    let policy = OrderingPolicy::try_from_stable_id(new)?;
    let program = policy
        .program()
        .ok_or("candidate is not a validated programmed order")?;
    require(
        program.arity() == arity,
        "programmed order arity differs from owner",
    )?;
    Ok(())
}

// One small runtime-ABI qualification, only for an explicitly changed order.
// It is not a certificate or whole-owner descent audit.
fn qualify_order(
    library: &PLibrary,
    class: &str,
    summary: &CustomPayloadSummary,
) -> Result<serde_json::Value> {
    let policy = OrderingPolicy::try_from_stable_id(&summary.ordering)?;
    let program = policy
        .program()
        .ok_or("qualification requires a programmed order")?;
    let corner = summary
        .residual_keys()
        .into_iter()
        .next()
        .ok_or("owner has no residual sample")?;
    let active: Vec<_> = (0..summary.arity)
        .filter(|i| summary.support >> i & 1 == 1)
        .collect();
    let inactive: Vec<_> = (0..summary.arity)
        .filter(|i| summary.support >> i & 1 == 0)
        .collect();
    require(
        active.len() >= 2 && inactive.len() >= 2,
        "no distinguishing source-index pair",
    )?;
    let mut left = corner.clone();
    let mut right = corner.clone();
    left[active[0]] += 1;
    left[inactive[1]] -= 1;
    right[active[1]] += 1;
    right[inactive[0]] -= 1;
    let old = OrderingPolicy::SpiredUncutV1;
    if policy.compare(&left, &right)? == old.compare(&left, &right)? {
        // A metadata-derived inversion, not an endpoint grid.
        let priority = &program.descriptor().coordinate_priority;
        let rank = |axis| priority.iter().position(|&a| a == axis).unwrap();
        let inversion = inactive
            .iter()
            .enumerate()
            .find_map(|(i, &a)| {
                inactive[i + 1..]
                    .iter()
                    .copied()
                    .find(|&b| rank(a) > rank(b))
                    .map(|b| (a, b))
            })
            .ok_or("no distinguishing inactive-priority inversion")?;
        left = corner.clone();
        right = corner.clone();
        left[active[0]] += 1;
        right[active[0]] += 1;
        left[inversion.0] -= 1;
        right[inversion.1] -= 1;
    }
    let expected = policy.compare(&left, &right)?;
    require(
        expected != old.compare(&left, &right)?,
        "sample pair does not distinguish old/new order",
    )?;
    let class = OwnerClassId(u64::from_str_radix(
        class.rsplit('-').next().ok_or("class")?,
        16,
    )?);
    let owner = library.owner(class).ok_or("loaded owner")?;
    let OwnerProgram::Lane(step) = lane_program(library, &owner.spec)? else {
        return Err("loaded owner lane".into());
    };
    let limits = ReductionLimits::default();
    let cx = PStepContext {
        library,
        owner,
        limits: &limits,
    };
    let lk = step.order_key(&cx, &left)?;
    let rk = step.order_key(&cx, &right)?;
    require(
        lk == POrderKey::Complexity(policy.complexity_key(&left)?)
            && rk == POrderKey::Complexity(policy.complexity_key(&right)?)
            && lk.cmp(&rk) == expected,
        "loaded numeric order does not retain descriptor",
    )?;
    let output = step.step(
        &cx,
        &PKeyView {
            presentation: owner.spec.presentation,
            powers: &left,
            tag: &GammaTag::one(),
        },
    )?;
    let mut checked = 0;
    let mut foreign = 0;
    for child in &output.children {
        if child.coefficient.is_zero() {
            continue;
        }
        if child.presentation != owner.spec.presentation {
            foreign += 1;
            continue;
        }
        policy.prove_strict_descent(&left, &child.powers)?;
        checked += 1;
    }
    Ok(
        serde_json::json!({"scope":"one descriptor-distinguishing valid index pair and one actual native step; not a full owner audit",
        "left":left,"right":right,"native_comparison":format!("{expected:?}"),"old_comparison":format!("{:?}",old.compare(&left,&right)?),
        "retained_numeric_descriptor":true,"nonzero_same_frame_children_strictly_lower":checked,"different_frame_exits":foreign}),
    )
}

fn relative(root: &Path, name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    require(
        !name.is_empty() && path.components().all(|c| matches!(c, Component::Normal(_))),
        "native package filename is not a safe relative path",
    )?;
    Ok(root.join(path))
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}

fn residuals(summary: &CustomPayloadSummary) -> BTreeMap<u64, BTreeSet<Vec<i64>>> {
    summary
        .charts
        .iter()
        .map(|c| (c.chart, c.residuals.iter().cloned().collect()))
        .collect()
}

// The complete engine-referenced native package, including artifact-v2 files.
// Publication-only receipts/checksum lists are not candidate-native inputs.
fn native_files(m: &LibraryManifest) -> BTreeSet<String> {
    let f = &m.files;
    let mut names = BTreeSet::from([
        LIBRARY_FILE.to_owned(),
        f.families.clone(),
        f.topology.clone(),
        f.census.clone(),
        f.census_json.clone(),
    ]);
    names.extend(f.family_files.iter().map(|r| r.file.clone()));
    names.extend(
        [
            f.layer.as_ref(),
            f.ordering.as_ref(),
            f.routes.as_ref(),
            f.orbits.as_ref(),
        ]
        .into_iter()
        .flatten()
        .cloned(),
    );
    names.extend(
        m.owners
            .iter()
            .filter_map(|o| o.payload.as_ref().map(|p| p.file.clone())),
    );
    names
}

fn check_hash(bytes: &[u8], expected: &str) -> Result<()> {
    require(
        expected.len() == 64 && expected.bytes().all(|b| b.is_ascii_hexdigit()),
        "EXPECTED_BLAKE3 must be one complete native payload digest",
    )?;
    require(
        blake3::hash(bytes).to_hex().as_str() == expected,
        "payload hash mismatch",
    )
}

fn check_candidate(
    old: &CustomPayloadSummary,
    new: &CustomPayloadSummary,
    expected: usize,
    allow: bool,
) -> Result<()> {
    require(
        new.rules() == expected,
        "candidate rule count differs from declared variant",
    )?;
    require(
        old.family_fingerprint == new.family_fingerprint
            && old.arity == new.arity
            && old.support == new.support
            && old.two_sided_axes == new.two_sided_axes,
        "candidate changes the selected owner family/basis/support/two-sided contract",
    )?;
    check_ordering(&old.ordering, &new.ordering, new.arity, allow)?;
    require(
        residuals(old) == residuals(new),
        "candidate changes residual terminals or their charts",
    )
}

fn current_v4(source: &PLibrarySource, library: &PLibrary) -> Result<LibraryManifest> {
    let manifest = read_p_library_manifest(source)?;
    require(
        manifest
            .versions
            .get("P_SCHEME_TEMPLATE_VERSION")
            .map(String::as_str)
            == Some("4"),
        "this stager requires the genuine current v4 package",
    )?;
    require(
        manifest.files.orbits.as_deref() == Some("orbits.rrbin"),
        "current v4 package must retain its saved orbits",
    )?;
    require(
        library.saved_orbits().is_some()
            && library.orbit_canonicalization()
            && library.route_canonicalization()
            && library.orbit_prefer_high(),
        "saved orbit restore/ON/route/greatest-image contract differs",
    )?;
    Ok(manifest)
}

fn utility(args: &[String]) -> Result<bool> {
    if args.get(1).is_some_and(|s| s == "inspect") {
        require(args.len() == 3, "inspect PAYLOAD")?;
        let bytes = fs::read(&args[2])?;
        let s = inspect_custom_payload(&bytes)?;
        println!(
            "{}",
            serde_json::json!({
                "status":"NATIVE_PAYLOAD_INSPECTED_NOT_VALUE_ACCEPTED",
                "bytes":bytes.len(),"blake3":blake3::hash(&bytes).to_hex().to_string(),
                "rules":s.rules(),"family_fingerprint":s.family_fingerprint,
                "arity":s.arity,"support":s.support,"ordering":s.ordering,
                "residuals":residuals(&s),
            })
        );
        return Ok(true);
    }
    if args.get(1).is_some_and(|s| s == "self-test") {
        require(
            args.len() == 6,
            "self-test BASE PAYLOAD RULES EXPECTED_BLAKE3",
        )?;
        let base = PathBuf::from(&args[2]);
        let source = PLibrarySource::Directory(base.clone());
        let library = load_p_library(
            source.clone(),
            &PLibraryOptions {
                verify_descent: false,
            },
        )?;
        let manifest = current_v4(&source, &library)?;
        let record = manifest
            .owners
            .iter()
            .find(|o| o.class == "L4T08-d261f9c2d25e2dac")
            .ok_or("base Bebe owner missing")?;
        let old = inspect_custom_payload(&fs::read(relative(
            &base,
            &record.payload.as_ref().ok_or("base payload")?.file,
        )?)?)?;
        let bytes = fs::read(&args[3])?;
        let new = inspect_custom_payload(&bytes)?;
        let rules: usize = args[4].parse()?;
        check_hash(&bytes, &args[5])?;
        check_candidate(&old, &new, rules, true)?;
        require(
            check_hash(&bytes, &"0".repeat(64)).is_err(),
            "hash mutation accepted",
        )?;
        require(
            check_candidate(&old, &new, rules + 1, true).is_err(),
            "metadata mutation accepted",
        )?;
        let cross = manifest
            .owners
            .iter()
            .find(|o| o.class == "L4T08-cd9ded89aad8feea")
            .ok_or("frame control owner missing")?;
        let cross_bytes = fs::read(relative(
            &base,
            &cross.payload.as_ref().ok_or("cross payload")?.file,
        )?)?;
        let wrong = inspect_custom_payload(&cross_bytes)?;
        require(
            check_candidate(&old, &wrong, wrong.rules(), true).is_err(),
            "wrong family/frame accepted",
        )?;
        let orbit_bytes = fs::read(relative(
            &base,
            manifest.files.orbits.as_deref().ok_or("orbits")?,
        )?)?;
        require(!orbit_bytes.is_empty(), "empty orbit control")?;
        require(
            rustred::propagator::saved_orbits::PSavedOrbits::decode(
                &orbit_bytes[..orbit_bytes.len() - 1],
            )
            .is_err(),
            "truncated orbit control accepted",
        )?;
        require(
            native_files(&manifest).contains("orbits.rrbin"),
            "orbit enumeration omitted",
        )?;
        println!(
            "{}",
            serde_json::json!({"status":"PASS_NATIVE_STAGING_CONTROLS",
            "valid_candidate":true,"bad_hash_rejected":true,"bad_metadata_rejected":true,
            "bad_frame_rejected":true,"bad_orbits_rejected":true,"orbits_enumerated":true})
        );
        return Ok(true);
    }
    Ok(false)
}

fn run() -> Result<()> {
    let raw: Vec<String> = std::env::args().collect();
    if utility(&raw)? {
        return Ok(());
    }
    let (args, allow_programmed_order) = arguments(raw)?;
    let base = PathBuf::from(&args[1]).canonicalize()?;
    let payload_path = PathBuf::from(&args[2]).canonicalize()?;
    let requested = PathBuf::from(&args[3]);
    let tag = &args[4];
    require(
        !tag.is_empty()
            && tag
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
        "UNIQUE_TAG must use ASCII letters/digits/dash/underscore/dot",
    )?;
    let expected: usize = args[5].parse()?;
    let source_commit = &args[6];
    let owner_class = args[7].as_str();
    require(
        owner_class == "L4T08-d261f9c2d25e2dac",
        "this measured v4 staging scope admits Bebe only",
    )?;
    let parent = requested
        .parent()
        .ok_or("OUT needs an existing parent")?
        .canonicalize()?;
    let output = parent.join(requested.file_name().ok_or("OUT needs a filename")?);
    require(
        parent.starts_with(Path::new(ASTRA_ROOT).canonicalize()?),
        "OUT must be physically within ASTRA_HOME",
    )?;
    require(
        !output.exists() && !output.starts_with(&base),
        "OUT must be new and outside BASE",
    )?;

    let options = PLibraryOptions {
        verify_descent: false,
    };
    let source = PLibrarySource::Directory(base.clone());
    let original_library = load_p_library(source.clone(), &options)?;
    let mut manifest = current_v4(&source, &original_library)?;
    let original_manifest = manifest.clone();
    let orbit_count = original_library
        .saved_orbits()
        .ok_or("BASE orbits missing")?
        .len();
    let index = manifest
        .owners
        .iter()
        .position(|o| o.class == owner_class)
        .ok_or("BASE has no selected owner")?;
    let record = &manifest.owners[index];
    require(
        record.lane == LaneRecord::Custom,
        "selected owner is not the expected Custom owner",
    )?;
    let owner_payload = record
        .payload
        .clone()
        .ok_or("selected owner has no payload")?;
    let owner_meta = record
        .custom
        .clone()
        .ok_or("selected owner has no Custom metadata")?;
    let ordering_name = manifest
        .files
        .ordering
        .clone()
        .ok_or("gated BASE must carry ordering")?;
    let _routes_name = manifest
        .files
        .routes
        .as_ref()
        .ok_or("gated BASE must carry saved routes")?;
    let route_count = original_library
        .saved_routes()
        .ok_or("BASE saved routes did not load")?
        .len();

    let old_bytes = fs::read(relative(&base, &owner_payload.file)?)?;
    let bytes = fs::read(&payload_path)?;
    check_hash(&bytes, &args[8])?;
    let old = inspect_custom_payload(&old_bytes)?;
    let new = inspect_custom_payload(&bytes)?;
    check_candidate(&old, &new, expected, allow_programmed_order)?;
    let meta = new.meta(owner_meta.lp_count)?;
    require(
        meta.charts == owner_meta.charts && meta.residuals == owner_meta.residuals,
        "candidate changes the terminal/chart contract",
    )?;

    let original_ordering: OrderingFile =
        serde_json::from_slice(&fs::read(relative(&base, &ordering_name)?)?)?;
    let suffix = format!(
        ",P_ORDERING_VERSION={P_ORDERING_VERSION}:{}",
        original_ordering.heuristic
    );
    let base_identity = manifest
        .identity
        .strip_suffix(&suffix)
        .ok_or("BASE identity does not end with its native ordering tag")?
        .to_owned();
    let heuristic = format!("{}.astra-saved-owner-{tag}", original_ordering.heuristic);
    let mut ordering = original_ordering.clone();
    let files = native_files(&manifest);
    fs::create_dir(&output)?;
    for name in &files {
        if name == LIBRARY_FILE || name == &owner_payload.file || name == &ordering_name {
            continue;
        }
        let destination = relative(&output, name)?;
        fs::create_dir_all(destination.parent().ok_or("missing destination parent")?)?;
        fs::copy(relative(&base, name)?, destination)?;
    }

    // Only this native payload, native Custom metadata and its byte length change.
    let destination = relative(&output, &owner_payload.file)?;
    fs::create_dir_all(destination.parent().ok_or("missing owner directory")?)?;
    fs::write(destination, &bytes)?;
    manifest.owners[index].custom = Some(CustomRecord::of(&meta));
    manifest.owners[index].payload.as_mut().unwrap().bytes = u64::try_from(bytes.len())?;
    write_json(&output.join(LIBRARY_FILE), &manifest)?;

    // This public function reads payloads, not the old order file. Never strip
    // ordering/routes or rebuild the library to make a temporary load pass.
    let generated = compute_ordering(&output, "generation")?;
    let mut replacement = generated
        .owners
        .into_iter()
        .find(|o| o.class == owner_class)
        .ok_or("native ordering computation omitted selected owner")?;
    replacement
        .provenance
        .parameters
        .insert("candidate_source_commit".into(), source_commit.clone());
    replacement.provenance.parameters.insert(
        "candidate_payload".into(),
        payload_path.display().to_string(),
    );
    replacement
        .provenance
        .parameters
        .insert("overlay_variant".into(), tag.clone());
    if allow_programmed_order {
        replacement
            .provenance
            .parameters
            .insert("programmed_order_opt_in".into(), "true".into());
        replacement
            .provenance
            .parameters
            .insert("old_integral_order".into(), old.ordering.clone());
        replacement
            .provenance
            .parameters
            .insert("integral_order".into(), new.ordering.clone());
    }
    replacement.provenance.census = None; // no measurement of this new package yet
    *ordering
        .owners
        .iter_mut()
        .find(|o| o.class == owner_class)
        .ok_or("BASE ordering omitted selected owner")? = replacement;
    ordering.heuristic = heuristic.clone();
    manifest.identity = ordered_identity(&base_identity, &heuristic);
    for (before, after) in original_manifest.owners.iter().zip(&manifest.owners) {
        if before.class != owner_class {
            require(before == after, "a non-selected owner changed")?;
        }
    }
    for (before, after) in original_ordering.owners.iter().zip(&ordering.owners) {
        if before.class != owner_class {
            require(before == after, "a non-selected ordering changed")?;
        }
    }
    write_json(&relative(&output, &ordering_name)?, &ordering)?;
    write_json(&output.join(LIBRARY_FILE), &manifest)?;
    let loaded = load_p_library(PLibrarySource::Directory(output.clone()), &options)?;
    require(
        loaded.identity() == manifest.identity && loaded.identity() != original_library.identity(),
        "overlay did not receive its distinct native cache identity",
    )?;
    require(
        loaded.terminals() == original_library.terminals(),
        "terminal vector changed",
    )?;
    require(
        loaded.saved_routes().map(|r| r.len()) == Some(route_count),
        "saved routes were lost",
    )?;
    require(
        manifest.versions == original_manifest.versions
            && manifest.symbolica_save_format == original_manifest.symbolica_save_format,
        "version/save-format metadata changed",
    )?;
    require(
        loaded.saved_orbits().map(|o| o.len()) == Some(orbit_count)
            && loaded.orbit_canonicalization()
            && loaded.route_canonicalization()
            && loaded.orbit_prefer_high(),
        "saved orbit context was lost",
    )?;
    require(
        fs::read(relative(&base, "orbits.rrbin")?)?
            == fs::read(relative(&output, "orbits.rrbin")?)?,
        "saved orbit bytes changed",
    )?;
    let qualification = if allow_programmed_order && new.ordering != STOCK_ORDER {
        Some(qualify_order(&loaded, owner_class, &new)?)
    } else {
        None
    };
    println!(
        "{}",
        serde_json::json!({
            "status": "READY_V4_NOT_VALUE_ACCEPTED",
            "stager_source_commit":option_env!("ASTRA_SOURCE_COMMIT").unwrap_or("unknown"),
            "linked_native_source":option_env!("ASTRA_LINKED_NATIVE_SOURCE").unwrap_or("unknown"),
            "payload_blake3":blake3::hash(&bytes).to_hex().to_string(),
            "scheme_template_version":4,"saved_orbits":orbit_count,
            "orbit_canonicalization":loaded.orbit_canonicalization(),
            "route_canonicalization":loaded.route_canonicalization(),
            "orbit_prefer_high":loaded.orbit_prefer_high(),
            "base": base, "payload": payload_path, "output": output, "owner": owner_class,
            "rules": new.rules(), "source_commit": source_commit,
            "programmed_order_opt_in":allow_programmed_order,"old_integral_order":old.ordering,
            "integral_order":new.ordering,"numeric_order_qualification":qualification,
            "base_identity": original_library.identity(), "identity": loaded.identity(),
            "terminals": loaded.terminals().len(), "saved_routes": route_count,
            "native_files": files.len(), "other_owner_records_unchanged": true,
            "other_ordering_entries_unchanged": true, "ordering": heuristic,
            "basis_and_maps": "copied native files; no assembly or basis derivation",
            "publication_receipts": "remain with BASE; not relabelled as overlay receipts"
        })
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("p_stage_saved_owner_v4: {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opt_in_is_explicit_and_unknown_or_duplicate_flags_refuse() {
        let args = vec![
            "tool",
            "base",
            "payload",
            "out",
            "tag",
            "2",
            "commit",
            "class",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        assert!(!arguments(args.clone()).unwrap().1);
        let mut yes = args.clone();
        yes.push("--allow-programmed-order".into());
        assert!(arguments(yes.clone()).unwrap().1);
        yes.push("--allow-programmed-order".into());
        assert!(arguments(yes).is_err());
        let mut bad = args;
        bad.push("--unknown".into());
        assert!(arguments(bad).is_err());
    }
    #[test]
    fn parsed_program_arity_and_default_stock_guard_are_retained() {
        use rustred::order::{
            CompiledOrder, CoordinateGroups, DegreeRow, Direction, Limits, OrderDescriptor,
        };
        let descriptor = OrderDescriptor {
            pre_support_degree_rows: vec![],
            support_weights: vec![0; 2],
            support_priority: vec![0, 1],
            degree_rows: vec![DegreeRow {
                active: vec![1; 2],
                inactive: vec![1; 2],
            }],
            coordinate_priority: vec![0, 1],
            coordinate_groups: CoordinateGroups::InactiveFirst,
            active_direction: Direction::Descending,
            inactive_direction: Direction::Descending,
            two_sided: vec![],
        };
        let policy = OrderingPolicy::try_programmed(
            CompiledOrder::compile(descriptor, Limits::default()).unwrap(),
        )
        .unwrap();
        let id = policy.stable_id().to_string();
        assert!(check_ordering(STOCK_ORDER, STOCK_ORDER, 2, false).is_ok());
        assert!(check_ordering(STOCK_ORDER, &id, 2, false).is_err());
        assert!(check_ordering(STOCK_ORDER, &id, 2, true).is_ok());
        assert!(check_ordering(STOCK_ORDER, &id, 3, true).is_err());
        assert!(check_ordering(STOCK_ORDER, "not-an-order", 2, true).is_err());
    }
}
