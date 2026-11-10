//! A1 spike: load a p package, rebuild every Scheme owner's template and print which rules the
//! generator's own rows verify. usage: scheme_check <p-library dir>
use std::path::PathBuf;
use std::time::Instant;

use rustred::propagator::lane::OwnerLane;
use rustred::propagator::scheme::SchemeStep;
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("library dir"));
    let library = load_p_library(
        PLibrarySource::Directory(dir),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap();
    for owner in library.owners() {
        if owner.spec.lane != OwnerLane::Scheme {
            continue;
        }
        let started = Instant::now();
        let step = SchemeStep::from_owner(&library, &owner.spec).unwrap();
        let rules = step.template().rules();
        let verified: Vec<&str> = rules.iter().filter(|r| r.verified && !r.master).map(|r| r.name.as_str()).collect();
        let open: Vec<&str> = rules.iter().filter(|r| !r.verified).map(|r| r.name.as_str()).collect();
        println!(
            "scheme owner {:016x} presentation {}: {} rules, {} verified by the rows ({:.2} s), unverified {:?}",
            owner.spec.class.0,
            owner.spec.presentation.0,
            rules.len(),
            verified.len(),
            started.elapsed().as_secs_f64(),
            open
        );
        println!("verified: {verified:?}");
    }
}
