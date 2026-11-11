//! M5 maps: generation-time route-map choice for a p library package.
//!
//! Every routed census sector maps a source sector onto its owner through a
//! stored `SignedAffineLoopMap` T. Composing T with an automorphism M of the
//! owner sector gives another valid route `T M`; the routed numerators expand
//! into a different number of owner terms. This tool takes the chosen maps
//! (computed outside the engine, `SCRATCHPAD/m5-maps/tools/choose_maps.py`),
//! verifies each one exactly with the census's own admission
//! (`class::verify_route`: unit Jacobian, monomial line actions, equal shifts,
//! the same owner support) and writes a new `census.json` and `census.rrbin`.
//! The engine then follows the stored maps strictly, as it always does.
//!
//! usage: p_route_maps <library dir> <choices.json> <out dir>
//!   choices.json: [{"presentation": u32, "support": u64, "map": {loops_in, loops_out,
//!   loop_linear, loop_external, external_sign}}, ...]
//! Writes <out dir>/census.json, <out dir>/census.rrbin and prints a summary;
//! any choice that fails verification aborts (nothing written).
use std::path::PathBuf;

use rustred::propagator::PresentationId;
use rustred::propagator::census::{PCensus, PSectorFate, SignedAffineLoopMap};
use rustred::propagator::class::{ClassLimits, classify, verify_route};
use rustred_app::propagator::format::encode_census;
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: p_route_maps <library dir> <choices.json> <out dir>"
    );
    let (dir, choices, out) = (PathBuf::from(&args[1]), &args[2], PathBuf::from(&args[3]));
    let library = load_p_library(
        PLibrarySource::Directory(dir.clone()),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .expect("library");
    let presentations = library.presentations();
    let mut census = PCensus::from_json(&std::fs::read_to_string(dir.join("census.json")).unwrap())
        .expect("census.json");
    assert!(
        &census == library.census(),
        "census.json is not the loaded census"
    );
    let choices: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(choices).unwrap()).unwrap();
    let limits = ClassLimits::default();
    let (mut changed, mut same) = (0_usize, 0_usize);
    for choice in &choices {
        let presentation = PresentationId(choice["presentation"].as_u64().unwrap() as u32);
        let support = choice["support"].as_u64().unwrap();
        let m = &choice["map"];
        let ints = |key: &str| -> Box<[i64]> {
            m[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect()
        };
        let candidate = SignedAffineLoopMap {
            loops_in: m["loops_in"].as_u64().unwrap() as u8,
            loops_out: m["loops_out"].as_u64().unwrap() as u8,
            loop_linear: ints("loop_linear"),
            loop_external: ints("loop_external"),
            external_sign: m["external_sign"].as_i64().unwrap() as i8,
        };
        let Some(PSectorFate::Owner { class, map }) = census.sectors.get(&(presentation, support))
        else {
            panic!("sector ({}, {support:#x}) is not routed", presentation.0);
        };
        let class = *class;
        if *map == candidate {
            same += 1;
            continue;
        }
        let record = &census.classes[&class];
        let (owner_id, owner_support) = record.owner;
        assert!(
            (presentation, support) != record.owner,
            "the owner sector keeps the identity"
        );
        let source_p = &presentations[presentation.0 as usize];
        let owner_p = &presentations[owner_id.0 as usize];
        let raw =
            |p: &rustred::propagator::presentation::Presentation, s: u64| s & !p.forced_axes();
        let source = classify(source_p, raw(source_p, support), &limits)
            .expect("classify source")
            .expect("source class");
        let owner = classify(owner_p, raw(owner_p, owner_support), &limits)
            .expect("classify owner")
            .expect("owner class");
        let verified = verify_route(&source, &owner, &candidate, &limits).expect("verify");
        assert!(
            verified.as_ref() == Some(&candidate),
            "choice for sector ({}, {support:#x}) is not an admissible route",
            presentation.0
        );
        census.sectors.insert(
            (presentation, support),
            PSectorFate::Owner {
                class,
                map: candidate,
            },
        );
        changed += 1;
    }
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join("census.json"),
        census.to_json(presentations).unwrap(),
    )
    .unwrap();
    std::fs::write(out.join("census.rrbin"), encode_census(&census).unwrap()).unwrap();
    println!(
        "{{\"choices\": {}, \"changed\": {changed}, \"unchanged\": {same}}}",
        choices.len()
    );
}
