//! Add a rule ordering (p artefact v2) to a written p library, in place, then
//! load it back and print its identity and a per-owner summary.
//!
//!   p_order_package <package dir> <heuristic: generation | fewest-terms | none> [routes] [orbits]
//!
//! `routes` also records the route verifications (`routes.rrbin`); `orbits`
//! the verified automorphism groups of orbit canonicalisation (`orbits.rrbin`,
//! M5 batch 2).
//!
//! Start from a copy of an unordered package (`cp -a`); SHA256SUMS, if the
//! package has one, must be rewritten afterwards.
use std::path::PathBuf;

use rustred_app::propagator::ordering::{write_orbits, write_ordering, write_routes};
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let extras = args.get(3..).unwrap_or_default();
    assert!(
        args.len() >= 3 && extras.iter().all(|a| a == "routes" || a == "orbits"),
        "usage: p_order_package <package dir> <heuristic|none> [routes] [orbits]"
    );
    let directory = PathBuf::from(&args[1]);
    if extras.iter().any(|a| a == "orbits") {
        let started = std::time::Instant::now();
        let (saved, nontrivial) = write_orbits(&directory).unwrap();
        println!(
            "orbits: {saved} sectors saved ({nontrivial} with more than +-1), {:.1} s",
            started.elapsed().as_secs_f64()
        );
    }
    if extras.iter().any(|a| a == "routes") {
        let started = std::time::Instant::now();
        let (saved, skipped) = write_routes(&directory).unwrap();
        println!(
            "routes: {saved} saved, {skipped} left to run-time verification, {:.1} s",
            started.elapsed().as_secs_f64()
        );
    }
    let ordering = if args[2] == "none" {
        None
    } else {
        Some(write_ordering(&directory, &args[2]).unwrap())
    };
    let library = load_p_library(
        PLibrarySource::Directory(directory),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .unwrap();
    println!("identity {}", library.identity());
    println!(
        "saved routes {}",
        library.saved_routes().map_or(0, |routes| routes.len())
    );
    println!(
        "saved orbits {}",
        library.saved_orbits().map_or(0, |orbits| orbits.len())
    );
    let Some(ordering) = ordering else {
        return;
    };
    let (mut custom, mut rules, mut moved) = (0, 0, 0);
    for owner in &ordering.owners {
        if owner.charts.is_empty() {
            continue;
        }
        custom += 1;
        rules += owner.rules.len();
        for chart in &owner.charts {
            moved += (chart.order.iter().enumerate())
                .filter(|&(position, &index)| position != index as usize)
                .count();
        }
    }
    println!(
        "owners {} (custom {custom}), rules {rules}, rules off their generation position {moved}",
        ordering.owners.len()
    );
}
