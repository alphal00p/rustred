//! Opt-in package editor for Insertion/Carpet outer-map choices.
//!
//! p_outer_maps BASE NEW_OUT REPORT.json EDGE_IDS CONTROL_IDS
//!
//! Copies BASE without changing payloads, basis, ordering or saved census routes.
//! Only strictly cheaper allowlisted EdgeMaps change (including the required
//! physical line_slots/merged_slot permutation). Controls are scored but must
//! remain unchanged. No runtime default or schema/version tag is changed.
//!
//! MEASUREMENT ONLY: PLibrary identity does not digest EdgeMaps. Both comparison
//! runs must disable persistent step caches and bind source + manifest + report.
//! Production use requires a map-choice cache-identity design.
use std::collections::BTreeSet;
use std::path::Path;

use rustred::propagator::route_choice::outer;
use rustred_app::propagator::format::{EdgeMapRecord, LoopMapRecord};
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};
use serde_json::{Value, json};

fn copy_tree(source: &Path, target: &Path) -> std::io::Result<usize> {
    std::fs::create_dir(target)?;
    let mut files = 0;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            files += copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), target.join(entry.file_name()))?;
            files += 1;
        } else {
            return Err(std::io::Error::other(
                "package contains a symlink or special file",
            ));
        }
    }
    Ok(files)
}

fn ids(text: &str) -> BTreeSet<usize> {
    text.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().expect("integer edge ID"))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        6,
        "usage: p_outer_maps BASE NEW_OUT REPORT.json EDGE_IDS CONTROL_IDS"
    );
    let source = std::fs::canonicalize(&args[1])?;
    let out = Path::new(&args[2]);
    let report = Path::new(&args[3]);
    assert!(!out.exists(), "NEW_OUT must not exist");
    assert!(!report.exists(), "report must not exist");
    let allowed = ids(&args[4]);
    let controls = ids(&args[5]);
    assert!(allowed.is_disjoint(&controls), "controls cannot be changed");
    let library = load_p_library(
        PLibrarySource::Directory(source.clone()),
        &PLibraryOptions {
            verify_descent: false,
        },
    )?;
    let before: Value =
        serde_json::from_str(&std::fs::read_to_string(source.join("library.json"))?)?;
    let mut manifest = before.clone();
    let (mut rows, mut changed) = (Vec::new(), Vec::new());
    for &index in allowed.union(&controls) {
        let edge = library.edge_maps().get(index).expect("edge ID");
        let owners: Vec<_> = library
            .owners()
            .filter(|o| o.spec.edge_maps.contains(&index))
            .collect();
        assert!(!owners.is_empty(), "edge is not used by an owner");
        let roles = owners[0].spec.roles;
        assert!(
            owners.iter().all(|o| o.spec.roles == roles),
            "ambiguous roles on edge"
        );
        let parent = &library.presentations()[edge.parent.0 as usize];
        let child = &library.presentations()[edge.child.0 as usize];
        let choice = outer::choose_static(parent, child, edge, roles)?;
        let is_changed = choice.edge != *edge;
        assert!(
            !is_changed || allowed.contains(&index),
            "identity control changed"
        );
        if is_changed {
            assert!(choice.score_after < choice.score_before);
            manifest["edge_maps"][index] = serde_json::to_value(EdgeMapRecord::of(&choice.edge))?;
            changed.push(index);
        }
        let candidates: Vec<_> = choice
            .candidates
            .iter()
            .map(|c| {
                json!({
                    "automorphism": LoopMapRecord::of(&c.automorphism),
                    "edge": EdgeMapRecord::of(&c.edge),
                    "physical_permutation": c.physical_permutation,
                    "score": c.score,
                    "factor_terms": c.factor_terms,
                    "selected": c.edge == choice.edge,
                })
            })
            .collect();
        rows.push(json!({
            "edge_id": index, "parent": edge.parent.0, "child": edge.child.0,
            "control": controls.contains(&index), "changed": is_changed,
            "before": EdgeMapRecord::of(edge), "after": EdgeMapRecord::of(&choice.edge),
            "score_before": choice.score_before, "score_after": choice.score_after,
            "candidates": candidates,
        }));
    }
    let mut restored = manifest.clone();
    for &index in &changed {
        restored["edge_maps"][index] = before["edge_maps"][index].clone();
    }
    assert_eq!(restored, before, "non-allowlisted semantic field changed");
    let copied_files = copy_tree(&source, out)?;
    std::fs::write(
        out.join("library.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let after = load_p_library(
        PLibrarySource::Directory(out.to_path_buf()),
        &PLibraryOptions {
            verify_descent: false,
        },
    )?;
    assert_eq!(
        library.identity(),
        after.identity(),
        "no fabricated identity/version tag"
    );
    assert_eq!(library.census(), after.census(), "census changed");
    assert_eq!(library.edge_maps().len(), after.edge_maps().len());
    for (i, (old, new)) in library
        .edge_maps()
        .iter()
        .zip(after.edge_maps())
        .enumerate()
    {
        assert!(old == new || changed.contains(&i), "unbound edge changed");
    }
    let result = json!({
        "status": "DERIVED_MEASUREMENT_ONLY",
        "source_commit": option_env!("ASTRA_SOURCE_COMMIT").unwrap_or("unknown"),
        "source_package": source, "output_package": out,
        "library_identity": after.identity(), "cache_contract": "persistent step caches must be None",
        "source_manifest_identity_only": true,
        "allowed_edges": allowed, "control_edges": controls, "changed_edges": changed,
        "copied_files": copied_files, "nonedge_semantics_equal": true, "census_equal": true,
        "ordering_and_saved_routes": "copied unchanged; no census or route regeneration",
        "score": "post-integration elementary factors: sum(n+n*(n+1)/2), sum(n*n); strict improvements only",
        "edges": rows,
    });
    std::fs::write(report, serde_json::to_vec_pretty(&result)?)?;
    println!(
        "{}",
        json!({"status":"DERIVED_MEASUREMENT_ONLY", "changed_edges":changed, "report":report})
    );
    Ok(())
}
