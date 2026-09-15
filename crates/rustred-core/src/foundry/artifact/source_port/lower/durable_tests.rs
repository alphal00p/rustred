use crate::family::IntegralKey;
use crate::foundry::artifact::{ArtifactLoadLimits, ArtifactPersistenceError, ClosedArtifact};
use crate::reduction::Reducer;

use super::super::super::tests::{solved_tadpole, tadpole};

pub(in crate::foundry::artifact) fn installed_k1() -> ClosedArtifact {
    let (audit, solution) = solved_tadpole();
    audit
        .install_complete(tadpole(), [([true], None, solution)])
        .unwrap()
}

#[test]
fn original_domain_durable_k1_roundtrip_uses_existing_reducer_and_no_anchor() {
    let artifact = installed_k1();
    let bytes = artifact.encode_durable().unwrap();
    let independent = installed_k1();
    assert_eq!(bytes, independent.encode_durable().unwrap());
    let loaded = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert_eq!(bytes, loaded.encode_durable().unwrap());
    assert_eq!(loaded.masters(), artifact.masters());
    assert_eq!(loaded.source_relations(), artifact.source_relations());
    let rule = loaded.rule_cells()[0].rule();
    assert!(rule.anchor().is_none());
    assert!(rule.pivot_guard().is_none());
    assert!(rule.concrete_replay().is_none());
    let mut direct = Reducer::new(&artifact).unwrap();
    let mut cold = Reducer::new(&loaded).unwrap();
    for n in [-2, 0, 1, 2, 7] {
        let target = IntegralKey::try_new([n]).unwrap();
        assert_eq!(
            direct.reduce_unit_mass(&target).unwrap().terms(),
            cold.reduce_unit_mass(&target).unwrap().terms()
        );
    }
}

#[test]
fn original_domain_cold_load_honors_final_admission_and_cover_limits() {
    let artifact = installed_k1();
    let bytes = artifact.encode_durable().unwrap();
    let mut cover = ArtifactLoadLimits::default();
    cover.cover_replay.max_requested_boxes = 1; // Rule cell plus explicit master require two.
    assert_eq!(
        ClosedArtifact::decode_durable_with_limits(&bytes, cover).unwrap_err(),
        ArtifactPersistenceError::ResourceLimit {
            resource: "combined cover boxes",
            requested: 2,
            limit: 1
        }
    );
    let mut admission = ArtifactLoadLimits::default();
    admission.rule_derivation.max_ordering_key_coordinate_cells = 6;
    ClosedArtifact::decode_durable_with_limits(&bytes, admission).unwrap();
    admission.rule_derivation.max_ordering_key_coordinate_cells = 0;
    assert!(
        matches!(ClosedArtifact::decode_durable_with_limits(&bytes, admission),
        Err(ArtifactPersistenceError::OriginalDomainReplay { detail }) if detail.contains("ordering") || detail.contains("coordinate"))
    );
    let mut endpoints = ArtifactLoadLimits::default();
    endpoints.rule_derivation.max_domain_bound_endpoint_cells = 14;
    ClosedArtifact::decode_durable_with_limits(&bytes, endpoints).unwrap();
    endpoints.rule_derivation.max_domain_bound_endpoint_cells = 13;
    assert!(
        matches!(ClosedArtifact::decode_durable_with_limits(&bytes, endpoints),
        Err(ArtifactPersistenceError::OriginalDomainReplay { detail }) if detail.contains("combined domain bound endpoint cells"))
    );
    let mut origins = ArtifactLoadLimits::default();
    let guards = artifact.rule_cells()[0].rule().nonzero_guards();
    assert_eq!(
        guards.len(),
        1,
        "the origin budget must not be a distinct-guard test"
    );
    assert!(guards[0].origins().len() > 1);
    origins.rule_derivation.max_guard_origins = 1;
    assert!(
        matches!(ClosedArtifact::decode_durable_with_limits(&bytes, origins),
        Err(ArtifactPersistenceError::OriginalDomainReplay { detail }) if detail.contains("combined guard origins"))
    );
}

#[test]
fn original_domain_durable_k3_retains_all_four_actual_terminals() {
    let artifact = super::tests::generated::<3>(
        crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap(),
    );
    let bytes = artifact.encode_durable().unwrap();
    let loaded = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert_eq!(bytes, loaded.encode_durable().unwrap());
    assert_eq!(loaded.masters(), artifact.masters());
    assert_eq!(loaded.masters().len(), 4);
    let mut direct = Reducer::new(&artifact).unwrap();
    let mut cold = Reducer::new(&loaded).unwrap();
    for powers in [
        [1, 1, 1],
        [2, 1, 1],
        [0, 1, 1],
        [1, 0, 1],
        [1, 1, 0],
        [-1, 2, 1],
    ] {
        let target = IntegralKey::try_new(powers).unwrap();
        assert_eq!(
            direct.reduce_unit_mass(&target).unwrap().terms(),
            cold.reduce_unit_mass(&target).unwrap().terms()
        );
    }
}

#[test]
#[ignore = "complete canonical K6 durable original-source replay and existing reducer"]
fn original_domain_durable_k6_exact_keys_mass_and_cache() {
    let artifact = super::tests::generated::<6>(
        crate::foundry::artifact::three_loop::canonical_family().unwrap(),
    );
    let bytes = artifact.encode_durable().unwrap();
    let loaded = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert_eq!(bytes, loaded.encode_durable().unwrap());
    assert_eq!(loaded.masters(), artifact.masters());
    assert_eq!(loaded.masters().len(), 38);
    assert!(
        loaded
            .masters()
            .iter()
            .all(|key| key.powers().iter().all(|value| matches!(value, 0 | 1)))
    );
    let mut direct = Reducer::new(&artifact).unwrap();
    let mut cold = Reducer::new(&loaded).unwrap();
    for master in loaded.masters() {
        assert_eq!(
            cold.reduce_unit_mass(master).unwrap().terms().get(master),
            Some(&loaded.coefficient_context().one())
        );
    }
    let target = IntegralKey::try_new([2, 1, 1, 1, 1, 1]).unwrap();
    let unit = cold.reduce_unit_mass(&target).unwrap();
    assert_eq!(
        unit.terms(),
        direct.reduce_unit_mass(&target).unwrap().terms()
    );
    assert!(
        unit.terms().len() > 1,
        "mass probe must genuinely have multiple masters"
    );
    let before = cold.statistics().cache_hits();
    assert_eq!(
        unit.terms(),
        cold.reduce_unit_mass(&target).unwrap().terms()
    );
    assert!(cold.statistics().cache_hits() > before);
    let context = loaded.coefficient_context();
    let massive = cold
        .reduce_with_common_mass_squared(&target, &context.integer(3))
        .unwrap();
    let homogeneous = cold.reduce_with_common_mass_homogeneity(&target).unwrap();
    for (master, coefficient) in unit.terms() {
        let exponent = master
            .powers()
            .iter()
            .map(|value| i128::from(*value))
            .sum::<i128>()
            - 7;
        assert_eq!(
            homogeneous
                .coefficient(master)
                .unwrap()
                .common_mass_squared_power(),
            exponent
        );
        let denominator = match exponent {
            -4 => 81,
            -3 => 27,
            -2 => 9,
            -1 => 3,
            _ => panic!("unexpected finite corner exponent {exponent}"),
        };
        let expected = context
            .try_div(
                coefficient,
                &context.integer(denominator),
                Default::default(),
            )
            .unwrap();
        assert_eq!(massive.coefficient(master), Some(&expected));
    }
    eprintln!(
        "durable canonical K6: bytes={}, cells={}, typed_terminals={}",
        bytes.len(),
        loaded.rule_cells().len(),
        loaded.masters().len()
    );
}
