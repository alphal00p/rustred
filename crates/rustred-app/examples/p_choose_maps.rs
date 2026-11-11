//! M5 maps (p5l-maps-rs): static-score route-map choice for a p library
//! package, in Rust and in parallel (port of `tools/m5-maps/choose_maps.py
//! static`; same choices, see `rustred::propagator::route_choice`).
//!
//! usage: p_choose_maps <library dir> <out dir> [--threads N] [--no-verify] [--write-census]
//!
//! For every routed census sector (not an owner sector), every composition
//! `T M` with `M` in the owner sector's automorphism group is scored; the
//! first strict minimum replaces the stored map when it beats it strictly.
//! Every changed map is verified exactly with `class::verify_route` (unit
//! Jacobian, monomial line actions, equal shifts, the same owner support)
//! unless `--no-verify`; one failure aborts before anything is written.
//! Writes `<out>/choices.json` (the format of choose_maps.py and of
//! `p_route_maps`), `<out>/summary.json`, and with `--write-census` the
//! rewritten `<out>/census.json` and `<out>/census.rrbin`.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;
use rustred::propagator::PresentationId;
use rustred::propagator::census::{PCensus, PSectorFate, SignedAffineLoopMap};
use rustred::propagator::class::{ClassLimits, classify, verify_route};
use rustred::propagator::presentation::Presentation;
use rustred::propagator::route_choice::{
    FormBasis, RouteDecision, choose_static, sector_automorphisms,
};
use rustred_app::propagator::format::encode_census;
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};
use serde_json::json;

fn map_json(m: &SignedAffineLoopMap) -> serde_json::Value {
    json!({
        "loops_in": m.loops_in, "loops_out": m.loops_out,
        "loop_linear": m.loop_linear.to_vec(), "loop_external": m.loop_external.to_vec(),
        "external_sign": m.external_sign,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage =
        "usage: p_choose_maps <library dir> <out dir> [--threads N] [--no-verify] [--write-census]";
    assert!(args.len() >= 3, "{usage}");
    let (dir, out) = (PathBuf::from(&args[1]), PathBuf::from(&args[2]));
    let mut threads = 1_usize;
    let (mut verify, mut write_census) = (true, false);
    let mut rest = args[3..].iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--threads" => threads = rest.next().expect(usage).parse().expect("threads"),
            "--no-verify" => verify = false,
            "--write-census" => write_census = true,
            other => panic!("unknown argument {other}; {usage}"),
        }
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .stack_size(32 * 1024 * 1024)
        .build()
        .expect("thread pool");
    let t_load = Instant::now();
    let library = load_p_library(
        PLibrarySource::Directory(dir.clone()),
        &PLibraryOptions {
            verify_descent: false,
        },
    )
    .expect("library");
    let presentations: Vec<Arc<Presentation>> = library.presentations().to_vec();
    let mut census: PCensus = library.census().clone();
    let load_s = t_load.elapsed().as_secs_f64();

    let t_choose = Instant::now();
    // routed sectors in (presentation, support) order, owners excluded
    let routed: Vec<((PresentationId, u64), _, SignedAffineLoopMap)> = census
        .sectors
        .iter()
        .filter_map(|(&key, fate)| match fate {
            PSectorFate::Owner { class, map } => {
                let owner = census.classes[class].owner;
                (key != owner).then(|| (key, owner, map.clone()))
            }
            PSectorFate::Zero => None,
        })
        .collect();
    let mut owners: Vec<(PresentationId, u64)> = routed.iter().map(|(_, o, _)| *o).collect();
    owners.sort();
    owners.dedup();
    let groups: BTreeMap<(PresentationId, u64), Vec<SignedAffineLoopMap>> = pool.install(|| {
        owners
            .par_iter()
            .map(|&(p, s)| {
                let g = sector_automorphisms(&presentations[p.0 as usize], s).expect("group");
                ((p, s), g)
            })
            .collect()
    });
    let mut owner_presentations: Vec<PresentationId> = owners.iter().map(|o| o.0).collect();
    owner_presentations.dedup();
    let bases: BTreeMap<PresentationId, FormBasis> = pool.install(|| {
        owner_presentations
            .par_iter()
            .map(|&p| {
                let b = FormBasis::of(&presentations[p.0 as usize])
                    .expect("basis")
                    .unwrap_or_else(|| panic!("presentation {} slots are no basis", p.0));
                (p, b)
            })
            .collect()
    });
    let groups_s = t_choose.elapsed().as_secs_f64();
    let decisions: Vec<RouteDecision> = pool.install(|| {
        routed
            .par_iter()
            .map(|((p, s), owner, t)| {
                choose_static(
                    &presentations[p.0 as usize],
                    *s,
                    t,
                    &groups[owner],
                    &bases[&owner.0],
                )
                .expect("score")
            })
            .collect()
    });
    let choose_s = t_choose.elapsed().as_secs_f64();

    let (mut trivial, mut kept, mut changed) = (0_usize, 0_usize, 0_usize);
    let mut choices = Vec::new();
    let mut to_verify = Vec::new();
    for (((p, s), owner, _), d) in routed.iter().zip(&decisions) {
        match d {
            RouteDecision::TrivialGroup => trivial += 1,
            RouteDecision::Kept => kept += 1,
            RouteDecision::Changed(c) => {
                changed += 1;
                choices.push(json!({
                    "presentation": p.0, "support": s, "map": map_json(&c.map),
                    "score_before": [c.score_before.0, c.score_before.1],
                    "score_after": [c.score_after.0, c.score_after.1],
                }));
                to_verify.push(((*p, *s), *owner, c.map.clone()));
            }
        }
    }
    let t_verify = Instant::now();
    if verify {
        let limits = ClassLimits::default();
        let raw = |p: &Presentation, s: u64| s & !p.forced_axes();
        let failures: Vec<String> = pool.install(|| {
            to_verify
                .par_iter()
                .filter_map(|((p, s), (op, os), candidate)| {
                    let source_p = &presentations[p.0 as usize];
                    let owner_p = &presentations[op.0 as usize];
                    let source = classify(source_p, raw(source_p, *s), &limits)
                        .expect("classify source")
                        .expect("source class");
                    let owner = classify(owner_p, raw(owner_p, *os), &limits)
                        .expect("classify owner")
                        .expect("owner class");
                    let verified =
                        verify_route(&source, &owner, candidate, &limits).expect("verify");
                    (verified.as_ref() != Some(candidate)).then(|| format!("({}, {s:#x})", p.0))
                })
                .collect()
        });
        assert!(
            failures.is_empty(),
            "{} choices are not admissible routes, e.g. {:?}",
            failures.len(),
            &failures[..failures.len().min(5)]
        );
    }
    let verify_s = t_verify.elapsed().as_secs_f64();
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join("choices.json"),
        serde_json::to_string(&choices).unwrap(),
    )
    .unwrap();
    if write_census {
        for ((p, s), _, map) in &to_verify {
            let Some(PSectorFate::Owner { class, .. }) = census.sectors.get(&(*p, *s)) else {
                unreachable!()
            };
            let class = *class;
            census.sectors.insert(
                (*p, *s),
                PSectorFate::Owner {
                    class,
                    map: map.clone(),
                },
            );
        }
        std::fs::write(
            out.join("census.json"),
            census.to_json(library.presentations()).unwrap(),
        )
        .unwrap();
        std::fs::write(out.join("census.rrbin"), encode_census(&census).unwrap()).unwrap();
    }
    let summary = json!({
        "library": dir.display().to_string(), "threads": threads,
        "routed": routed.len(), "owners": owners.len(),
        "trivial_group": trivial, "kept": kept, "changed": changed,
        "verified": verify, "census_written": write_census,
        "seconds": {"load": load_s, "groups_and_bases": groups_s, "choose": choose_s, "verify": verify_s},
    });
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("{summary}");
}
