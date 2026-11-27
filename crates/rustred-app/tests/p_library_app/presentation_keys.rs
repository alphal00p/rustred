//! The content key of a presentation (`Presentation::content_key`): the
//! content-derived name beside its numeric id, equal for equal content in
//! two packages whose ids differ, different for other slots or families.
use std::collections::BTreeMap;

use rustred::propagator::library::PLibrary;
use rustred::propagator::presentation::{PresentationKind, Slot};

use super::package::{scope_three, scope_two};

/// Content keys: equal content gives equal keys across packages whose ids
/// differ (the 1-2 nodes inside the 1-3 package); different slots give
/// different keys; a top has no graph key.
#[test]
fn content_keys_name_presentations_across_packages() {
    let two = &scope_two().library;
    let three = &scope_three().library;
    let by_graph = |library: &PLibrary| -> BTreeMap<Vec<u8>, (u32, String)> {
        (library.presentations().iter())
            .filter_map(|p| {
                let key = p.content_key();
                (!key.graph_key.is_empty()).then(|| (key.graph_key.clone(), (p.id.0, key.digest())))
            })
            .collect()
    };
    let (in_two, in_three) = (by_graph(two), by_graph(three));
    assert!(!in_two.is_empty());
    let mut moved = 0;
    for (graph, (id, key)) in &in_two {
        let (other_id, other_key) = &in_three[graph];
        assert_eq!(key, other_key, "node {graph:02x?}");
        moved += usize::from(id != other_id);
    }
    assert!(
        moved > 0,
        "the ids of the 1-2 nodes are the same in the 1-3 package"
    );
    // Tops: no node, keyed by their slots and family.
    for top in two
        .presentations()
        .iter()
        .filter(|p| matches!(p.kind, PresentationKind::Top { .. }))
    {
        let key = top.content_key();
        assert!(key.graph_key.is_empty());
        let in_three = three
            .presentations()
            .iter()
            .find(|p| p.family.name() == top.family.name())
            .unwrap();
        assert_eq!(in_three.content_key(), key);
    }
    // Another slot (momentum or eps units) is another key.
    let presentation = two
        .presentations()
        .iter()
        .find(|p| p.node.is_some())
        .unwrap();
    let key = presentation.content_key();
    let mut eps = key.clone();
    if let Some(Slot::Line(line)) = eps.slots.first_mut() {
        line.eps_units += 1;
    }
    eps.line_eps_units[0] += 1;
    assert_ne!(eps.digest(), key.digest());
    let mut routed = key.clone();
    if let Some(Slot::Line(line)) = routed.slots.first_mut() {
        line.momentum = line.momentum.iter().map(|c| -c).collect();
    }
    assert_ne!(routed.digest(), key.digest());
    let mut family = key.clone();
    family.family_fingerprint[0] ^= 1;
    assert_ne!(family.digest(), key.digest());
    assert_eq!(key.to_string(), key.digest());
}
