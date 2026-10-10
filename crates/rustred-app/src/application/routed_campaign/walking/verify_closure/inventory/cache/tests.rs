use super::*;
use crate::application::routed_campaign::walking::verify_closure::test_scratch::scratch;

fn identity() -> Identity {
    Identity {
        schema: SCHEMA.into(),
        classifier: "matcher".into(),
        family: "family".into(),
        physical_arity: 3,
        owners: vec!["first".into(), "second".into()],
        scope_extensions: vec![],
        generation: 1,
        domains: 2,
        segments: vec![Segment {
            generation: 1,
            first: 0,
            count: 2,
            bytes: 42,
            file: "records-00000000000000000001.bin".into(),
            blake3: "segment".into(),
        }],
        counts: Counts {
            apply: 2,
            ..Counts::default()
        },
    }
}

fn payload() -> Payload {
    Payload {
        identity: identity(),
        rules: vec![(
            RuleIdentity {
                owner: vec![true; 3],
                batch: 0,
                rule: 2,
            },
            7,
        )],
        terminals: vec![vec![1, 1, 1]],
        selected_rule_events: 7,
        terminal_events: 3,
        zero_events: 11,
    }
}

#[test]
fn lineage_requires_exact_immutable_prefix_and_classifier_identity() {
    let old = identity();
    let mut current = old.clone();
    current.generation = 2;
    current.domains = 3;
    current.segments.push(Segment {
        generation: 2,
        first: 2,
        count: 1,
        bytes: 14,
        file: "records-00000000000000000002.bin".into(),
        blake3: "new".into(),
    });
    current.scope_extensions.push("required-extension".into());
    assert!(compatible(&old, &current).is_ok());
    for changed in 0..11 {
        let mut bad = current.clone();
        match changed {
            0 => bad.schema = "future".into(),
            1 => bad.classifier.push('!'),
            2 => bad.family.push('!'),
            3 => bad.physical_arity += 1,
            4 => bad.owners.reverse(),
            5 => bad.segments[0].blake3.push('!'),
            6 => bad.segments[0].bytes += 1,
            7 => bad.segments[0].first += 1,
            8 => bad.segments[0].count += 1,
            9 => bad.domains = 1,
            _ => bad.generation = 0,
        }
        // A future current schema cannot occur through identity(); testing
        // the cached schema directly also protects the decoder boundary.
        if changed == 0 {
            assert!(compatible(&bad, &current).is_err());
        } else {
            assert!(compatible(&old, &bad).is_err(), "mutation {changed}");
        }
    }
    let mut bad = old.clone();
    bad.domains += 1;
    assert!(compatible(&bad, &current).is_err());
    bad = old.clone();
    bad.counts.apply = usize::MAX;
    bad.counts.route = 1;
    assert!(compatible(&bad, &current).is_err());
    bad = old.clone();
    bad.scope_extensions.push("different".into());
    assert!(compatible(&bad, &current).is_err());
}

#[test]
fn cache_is_bounded_digest_bound_and_rejects_extra_bytes() {
    let dir = scratch("inventory-cache-codec");
    let path = dir.0.join("census.bin");
    let mut bytes = bincode::serde::encode_to_vec(payload(), bincode::config::standard()).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let source = OwnerDomainWalkCensusSource {
        path,
        expected_blake3: blake3::hash(&bytes).to_hex().to_string(),
        bytes: bytes.len() as u64,
    };
    assert!(read(&source).is_ok());
    let mut bad = source.clone();
    bad.bytes += 1;
    assert!(read(&bad).is_err());
    bad = source.clone();
    bad.expected_blake3 = blake3::hash(b"other").to_hex().to_string();
    assert!(read(&bad).is_err());
    bad.bytes = MAX_BYTES as u64 + 1;
    assert!(read(&bad).is_err());
    bytes.push(0);
    std::fs::write(&source.path, &bytes).unwrap();
    bad = source;
    bad.bytes = bytes.len() as u64;
    bad.expected_blake3 = blake3::hash(&bytes).to_hex().to_string();
    assert_eq!(read(&bad).err(), Some("cache_trailing_bytes"));
}

#[test]
fn cached_aggregate_shape_and_limits_are_checked_before_seeding() {
    let mut options = OwnerDomainWalkInventoryOptions::new("unused");
    let collector = Collector::new(&options);
    let (loaded, _) = state(payload(), &collector).unwrap();
    assert_eq!(loaded.selected_rule_events, 7);
    assert_eq!(loaded.terminal_events, 3);
    assert_eq!(loaded.zero_events, 11);
    assert_eq!(loaded.terminals.len(), 1);
    for mutation in 0..5 {
        let mut bad = payload();
        match mutation {
            0 => bad.rules.push(bad.rules[0].clone()),
            1 => bad.terminals.push(bad.terminals[0].clone()),
            2 => bad.rules[0].0.owner.pop().map(|_| ()).unwrap(),
            3 => bad.selected_rule_events += 1,
            _ => bad.terminal_events = 0,
        }
        assert!(state(bad, &collector).is_err());
    }
    options.max_unique_rules = 0;
    assert!(state(payload(), &Collector::new(&options)).is_err());
    assert!(collector.state.lock().unwrap().rules.is_empty());
}

#[cfg(feature = "cli")]
#[test]
fn cache_output_requires_every_domain_published_without_failures_or_abandonment() {
    let run = super::super::cache_tests::fixture();
    let options = OwnerDomainWalkVerifyOptions::new(run.dir.0.join("checkpoint"));
    let mut violations = super::super::super::Violations::new(100);
    let mut loaded = super::super::super::load::<3>(&options, false, &mut violations).unwrap();
    let cancel = AtomicBool::new(false);
    assert!(violations.is_empty());
    assert!(fully_published(&loaded, &cancel));
    let native = loaded.nodes.iter().position(|node| node.native()).unwrap();
    let saved = loaded.nodes[native];
    for mutation in 0..4 {
        loaded.nodes[native] = saved;
        match mutation {
            0 => loaded.nodes[native].kind = Kind::Missing,
            1 => loaded.nodes[native].abandoned = true,
            2 => loaded.nodes[native].error = true,
            _ => loaded.nodes[native].finished = false,
        }
        assert!(!fully_published(&loaded, &cancel));
    }
    loaded.nodes[native] = saved;
    loaded.raw.records[0].1 += 1;
    assert!(!fully_published(&loaded, &cancel));
    loaded.raw.records[0].1 -= 1;
    cancel.store(true, Ordering::Relaxed);
    assert!(!fully_published(&loaded, &cancel));
}
