//! Scheme lane check (M5 A1 schemes): load a p package, rebuild every Scheme owner's template (each rule's
//! certificate against the generator's own rows, the static descent witness) and print per rule how it is
//! verified (certificate, replay, none) with its stored metadata.
//! usage: scheme_check <p-library dir> [out.json]
use std::path::PathBuf;
use std::time::Instant;

use rustred::propagator::lane::OwnerLane;
use rustred::propagator::scheme::{SchemeStep, Verification, scheme_names};
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("library dir"));
    let out = std::env::args().nth(2);
    println!("schemes (name, enabled at assembly): {:?}", scheme_names().unwrap());
    let library = load_p_library(
        PLibrarySource::Directory(dir),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap();
    let mut report = Vec::new();
    for owner in library.owners() {
        if owner.spec.lane != OwnerLane::Scheme {
            continue;
        }
        let started = Instant::now();
        let step = SchemeStep::from_owner(&library, &owner.spec).unwrap();
        let template = step.template();
        let witness = template.descent_witness().unwrap();
        let rules = template.rules();
        let count = |v: Verification| rules.iter().filter(|r| r.verification == v).count();
        println!(
            "scheme {} owner {:016x} presentation {}: {} rules ({} certificate, {} replay, {} symmetry, {} unverified, {} master), descent witness {} shifts, {:.2} s",
            template.name(),
            owner.spec.class.0,
            owner.spec.presentation.0,
            rules.len(),
            count(Verification::Certificate),
            count(Verification::Replay),
            count(Verification::Symmetry),
            count(Verification::None),
            count(Verification::Master),
            witness,
            started.elapsed().as_secs_f64()
        );
        for rule in rules {
            println!("  {:<22} {:?}", rule.name, rule.verification);
        }
        report.push(serde_json::json!({
            "scheme": template.name(), "class": format!("{:016x}", owner.spec.class.0),
            "descent_witness_shifts": witness,
            "rules": rules.iter().map(|r| serde_json::json!({"name": r.name, "verification": format!("{:?}", r.verification), "meta": r.meta})).collect::<Vec<_>>(),
        }));
    }
    if let Some(out) = out {
        std::fs::write(out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
}
