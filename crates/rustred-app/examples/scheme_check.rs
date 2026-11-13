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
    println!(
        "schemes (name, enabled at assembly): {:?}",
        scheme_names().unwrap()
    );
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
        // M5 no6 coverage census: SCHEME_COVER=<dots>:<rank> enumerates every Apply key with at most
        // <dots> total dots and <rank> total numerator rank, follows the dispatch (permutation images)
        // and lists the keys no rule takes, in presentation and in Forcer slot order.
        if let Ok(cover) = std::env::var("SCHEME_COVER") {
            let (dots, rank) = cover.split_once(':').expect("SCHEME_COVER=<dots>:<rank>");
            let (dots, rank): (i64, i64) = (dots.parse().unwrap(), rank.parse().unwrap());
            let presentation = library.presentation(owner.spec.presentation);
            let lines = (presentation.slots.iter())
                .filter(|slot| matches!(slot, rustred::propagator::presentation::Slot::Line(_)))
                .count();
            let arity = presentation.slots.len();
            let perm = template.forcer_slots().to_vec();
            let mut keys: Vec<Vec<i64>> = vec![Vec::new()];
            for slot in 0..arity {
                let mut next = Vec::new();
                for key in &keys {
                    let used_dots: i64 = key.iter().take(lines).map(|p| p - 1).sum();
                    let used_rank: i64 = key.iter().skip(lines).map(|p| -p).sum();
                    if slot < lines {
                        for p in 1..=1 + dots - used_dots {
                            let mut k = key.clone();
                            k.push(p);
                            next.push(k);
                        }
                    } else {
                        for r in 0..=rank - used_rank {
                            let mut k = key.clone();
                            k.push(-r);
                            next.push(k);
                        }
                    }
                }
                keys = next;
            }
            let (mut covered, mut uncovered, mut via) = (0usize, Vec::new(), 0usize);
            for key in &keys {
                let mut at = key.clone();
                let mut hops = 0;
                loop {
                    match step.dispatch_rule(&at).unwrap() {
                        Some((_, Some(image))) if hops < 32 => {
                            at = image;
                            hops += 1;
                        }
                        Some(_) => {
                            covered += 1;
                            via += usize::from(hops > 0);
                            break;
                        }
                        None => {
                            uncovered.push((key.clone(), at.clone()));
                            break;
                        }
                    }
                }
            }
            println!(
                "cover {}: {} keys (dots <= {dots}, rank <= {rank}): {covered} covered ({via} through a permutation image), {} uncovered",
                template.name(),
                keys.len(),
                uncovered.len()
            );
            for (key, at) in &uncovered {
                let forcer: Vec<i64> = perm.iter().map(|&slot| at[slot]).collect();
                println!("  uncovered {key:?} -> {at:?} Forcer Z{forcer:?}");
            }
        }
        // scheme-certs: SCHEME_KEYS=<file> of `class<TAB>powers (comma list)<TAB>count` lines (stepped
        // Apply keys of a census, e.g. p_path_census with P_CENSUS_KEYS=1): the rule the dispatch
        // takes at each key of this owner, with the values every slot takes under each rule.
        let mut dispatched = serde_json::Map::new();
        if let Ok(path) = std::env::var("SCHEME_KEYS") {
            let class = format!("{:016x}", owner.spec.class.0);
            let arity = library.presentation(owner.spec.presentation).slots.len();
            let perm = template.forcer_slots().to_vec();
            let mut per_rule: std::collections::BTreeMap<
                String,
                (u64, u64, Vec<std::collections::BTreeSet<i64>>),
            > = std::collections::BTreeMap::new();
            for line in std::fs::read_to_string(&path).unwrap().lines() {
                let mut fields = line.split('\t');
                if fields.next() != Some(class.as_str()) {
                    continue;
                }
                let powers: Vec<i64> = fields
                    .next()
                    .unwrap()
                    .split(',')
                    .take(arity)
                    .map(|x| x.parse().unwrap())
                    .collect();
                let count: u64 = fields.next().map_or(1, |c| c.parse().unwrap());
                let name = match step.dispatch_rule(&powers).unwrap() {
                    Some((name, _)) => name,
                    None => "(none: fallback)".to_owned(),
                };
                let entry = per_rule
                    .entry(name)
                    .or_insert_with(|| (0, 0, vec![Default::default(); arity]));
                entry.0 += count;
                entry.1 += 1;
                // Forcer slot order (1-based in the report).
                for (a, &slot) in perm.iter().enumerate() {
                    entry.2[a].insert(powers[slot]);
                }
            }
            println!(
                "keys {}: {} rules dispatched",
                template.name(),
                per_rule.len()
            );
            for (name, (hits, keys, values)) in &per_rule {
                println!("  dispatched {name:<22} hits {hits} keys {keys}");
                dispatched.insert(
                    name.clone(),
                    serde_json::json!({"hits": hits, "keys": keys, "values": values}),
                );
            }
        }
        report.push(serde_json::json!({
            "scheme": template.name(), "class": format!("{:016x}", owner.spec.class.0),
            "dispatched": dispatched,
            "descent_witness_shifts": witness,
            "rules": rules.iter().map(|r| serde_json::json!({"name": r.name, "verification": format!("{:?}", r.verification), "meta": r.meta})).collect::<Vec<_>>(),
        }));
    }
    if let Some(out) = out {
        std::fs::write(out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
}
