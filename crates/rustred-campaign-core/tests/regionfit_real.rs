//! regionfit on REAL per-key choice data (test data only; nothing of it is in the library):
//!
//! - T162: the helper's per-key element records of the enlarged-box faces (sense i = image lower, sense ii = the
//!   COMPOSED relation lower than the original key, TRICKS 1), converted to FaceInput by
//!   implementer-scratch/regions/tools/extract.py into $RUSTRED_CAMPAIGN_FIXTURES/regionfit (sha256 manifest);
//! - T178: the repair flow's route-0b per-key element sizes (route0b_smallest3.json).
//!
//! Every record is checked for soundness and partition by the independent evaluator; the coordinator's / helper's hand
//! forms are COMPARED (printed), and the one the notes name (n0 <= n2 for element 2 on T162 face S0-2-8) is asserted.
//! Skips (with a message) when the fixture tree is absent.

mod regionfit_common;

use regionfit_common::{check_record, summary};
use rustred_campaign_core::regionfit::{FaceInput, FaceKey, RegionPolicy, fit_regions};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn root() -> Option<PathBuf> {
    let base = std::env::var("RUSTRED_CAMPAIGN_FIXTURES").unwrap_or_else(|_| {
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures".into()
    });
    let r = PathBuf::from(base).join("regionfit");
    if !r.join("MANIFEST.sha256").exists() {
        eprintln!("SKIP: no regionfit fixtures under {}", r.display());
        return None;
    }
    // manifest check
    let man = std::fs::read_to_string(r.join("MANIFEST.sha256")).unwrap();
    for line in man.lines() {
        let (h, p) = line.split_once("  ").unwrap();
        let bytes = std::fs::read(r.join(p)).unwrap();
        let got = rustred_campaign_core::closure::sha256::sha256_tag(&bytes);
        assert_eq!(got, format!("sha256:{h}"), "fixture {p} changed");
    }
    Some(r)
}

fn load(p: &std::path::Path) -> FaceInput {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn files(r: &std::path::Path, sub: &str, suffix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(r.join(sub))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(suffix))
        .collect();
    v.sort();
    v
}

/// Per-element view (the helper's question "where does element e pass"): e where it passes, "other" elsewhere.
fn per_element(f: &FaceInput, e: &str) -> FaceInput {
    FaceInput {
        signature: format!("{}#{e}", f.signature),
        keys: f
            .keys
            .iter()
            .map(|k| FaceKey {
                key: k.key.clone(),
                acceptable: [if k.acceptable.contains(e) { e } else { "other" }.to_string()]
                    .into_iter()
                    .collect(),
                image_rules: Default::default(),
            })
            .collect(),
    }
}

fn report_dir() -> Option<PathBuf> {
    std::env::var("REGIONFIT_REPORT_DIR")
        .ok()
        .map(PathBuf::from)
}

fn write_record(name: &str, text: &str) {
    if let Some(d) = report_dir() {
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(format!("{name}.json")), text).unwrap();
    }
}

#[test]
fn t162_faces_fit_sound_and_reported() {
    let Some(r) = root() else { return };
    let p = RegionPolicy::default();
    for variant in [".sense_ii.json", ".sense_ii_rule.json", ".sense_i.json"] {
        for path in files(&r, "T162", variant) {
            let f = load(&path);
            let fit = fit_regions(&f, None, &p).unwrap();
            let rec = fit.record();
            check_record(&f, &rec);
            let name = path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .replace(".json", "");
            write_record(&name, &fit.record_text());
            println!(
                "FIT {name}: keys {} | {} regions, {} per key, {} unservable | method {} | candidates {} | {:?}",
                fit.keys,
                fit.regions.len(),
                fit.residual.len(),
                fit.unservable.len(),
                fit.method.text(),
                fit.candidates,
                summary(&rec)
                    .iter()
                    .zip(&fit.regions)
                    .map(|((t, c), r)| format!("{t} -> {c} ({} keys)", r.keys))
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn t162_per_element_forms_compared_with_the_helper() {
    let Some(r) = root() else { return };
    let p = RegionPolicy {
        max_atoms: 3,
        ..RegionPolicy::default()
    };
    let mut asserted = false;
    for path in files(&r, "T162", ".sense_i.json") {
        let face = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(".sense_i.json", "");
        let helper: Option<serde_json::Value> = std::fs::read_to_string(
            r.join("T162")
                .join(format!("{face}.helper_conditions.json")),
        )
        .ok()
        .map(|t| serde_json::from_str(&t).unwrap());
        for (sense, suffix, hfield) in [
            ("i", ".sense_i.json", "sense_i_condition"),
            ("ii", ".sense_ii.json", "sense_ii_condition"),
        ] {
            let f = load(&r.join("T162").join(format!("{face}{suffix}")));
            if f.keys.len() > 600 {
                continue; // max_atoms 3 on the largest faces is a long run: the 2-atom fit above covers them
            }
            let elements: BTreeSet<String> = f
                .keys
                .iter()
                .flat_map(|k| k.acceptable.iter().cloned())
                .collect();
            for e in &elements {
                let fe = per_element(&f, e);
                let fit = fit_regions(&fe, None, &p).unwrap();
                check_record(&fe, &fit.record());
                let ours: Vec<String> = fit
                    .regions
                    .iter()
                    .filter(|x| &x.choice == e)
                    .map(|x| format!("{} ({} keys)", x.text, x.verified))
                    .collect();
                let per_key = fit.residual.iter().filter(|(_, c)| c == e).count();
                let theirs = helper
                    .as_ref()
                    .map(|h| h["elements"][e.trim_start_matches('g')][hfield]["terms"].clone());
                println!(
                    "CMP {face} sense {sense} {e}: ours {ours:?} + {per_key} per key | helper {}",
                    theirs.map(|t| t.to_string()).unwrap_or_default()
                );
                if face == "T162-S0-2-8-Nnone" && sense == "i" && e == "g2" {
                    assert_eq!(ours, vec!["n0<=n2 (161 keys)".to_string()]);
                    assert_eq!(per_key, 0);
                    asserted = true;
                }
            }
        }
    }
    assert!(asserted, "the named hand form was not compared");
}

#[test]
fn t162_layer_extension() {
    // fit on the lower layers of a face, test the form on its top layer (dots + numerator rank, from the key)
    let Some(r) = root() else { return };
    let p = RegionPolicy::default();
    let layer = |k: &[i32]| -> i32 { k.iter().map(|&x| if x > 0 { x - 1 } else { -x }).sum() };
    for path in files(&r, "T162", ".sense_ii.json") {
        let f = load(&path);
        let top = f.keys.iter().map(|k| layer(&k.key)).max().unwrap_or(0);
        let (low, high): (Vec<FaceKey>, Vec<FaceKey>) =
            f.keys.iter().cloned().partition(|k| layer(&k.key) < top);
        if low.len() < 4 || high.is_empty() {
            continue;
        }
        let fl = FaceInput {
            signature: format!("{}<L{top}", f.signature),
            keys: low,
        };
        let fit = fit_regions(&fl, Some(&high), &p).unwrap();
        check_record(&fl, &fit.record());
        println!(
            "EXT {} (fit on {} keys below layer {top}, deeper {}): {:?}",
            f.signature,
            fl.keys.len(),
            high.len(),
            fit.regions
                .iter()
                .map(|x| {
                    let e = x.extends.unwrap();
                    format!("{} -> {}: extends {} of {}", x.text, x.choice, e.n, e.m)
                })
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn t178_route0b_faces() {
    let Some(r) = root() else { return };
    for path in files(&r, "T178", ".route0b.json") {
        let f = load(&path);
        let fit = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
        let rec = fit.record();
        check_record(&f, &rec);
        println!(
            "FIT {}: keys {} | {:?} | per key {}",
            f.signature,
            fit.keys,
            summary(&rec),
            fit.residual.len()
        );
        // every key of this record accepts the flow's best element: one region on the whole face
        assert_eq!(fit.regions.len(), 1);
        assert_eq!(fit.regions[0].text, "true");
    }
}
