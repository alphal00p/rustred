use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::campaign::{
    CampaignDependencyInsertion, CampaignPlan, CampaignPlanError, CampaignPlanLimits,
    CampaignRootId, CampaignRootInsertion, CampaignRootSpec,
};
use rustred::{
    AffineDenominator, CoefficientContext, IntegralFamily, IntegralOrderingPolicy, SectorMask,
};

fn family(name: &str) -> Arc<IntegralFamily> {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap(),
    )
}

fn one_loop_one_external_family(name: &str) -> Arc<IntegralFamily> {
    let coefficients = CoefficientContext::new(["d", "s"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k".into()],
            vec!["p".into()],
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(zero.clone(), vec![one.clone(), zero.clone()]),
                AffineDenominator::new(
                    coefficients.parameter("s").unwrap(),
                    vec![one, coefficients.integer(2)],
                ),
            ],
            vec![vec![coefficients.parameter("s").unwrap()]],
            vec![zero.clone(), zero],
        )
        .unwrap(),
    )
}

fn root(id: &str, family: &Arc<IntegralFamily>, sector: &str) -> CampaignRootSpec {
    CampaignRootSpec::try_new(
        id,
        Arc::clone(family),
        SectorMask::try_from_bit_string(sector).unwrap(),
    )
    .unwrap()
}

fn plan(roots: Vec<CampaignRootSpec>) -> CampaignPlan {
    CampaignPlan::compile(
        roots,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CampaignPlanLimits::default(),
    )
    .unwrap()
}

#[test]
fn exact_representations_deduplicate_without_erasing_ingress_roots() {
    let first_family = family("campaign-exact-family");
    let independently_rebuilt = family("campaign-exact-family");
    let left = plan(vec![
        root("b", &independently_rebuilt, "111"),
        root("a", &first_family, "111"),
        root("a", &first_family, "111"),
    ]);
    let right = plan(vec![
        root("a", &independently_rebuilt, "111"),
        root("b", &first_family, "111"),
    ]);
    assert_eq!(left, right);
    assert_eq!(left.stats().roots(), 2);
    assert_eq!(left.stats().families(), 1);
    assert_eq!(left.stats().jobs(), 1);
    assert_eq!(left.intrinsic_jobs().count(), 1);
    left.verify().unwrap();

    let renamed = family("campaign-exact-family-renamed");
    let distinct = plan(vec![
        root("original", &first_family, "111"),
        root("renamed", &renamed, "111"),
    ]);
    assert_eq!(distinct.stats().families(), 2);
    assert_eq!(distinct.stats().jobs(), 2);
}

#[test]
fn repeated_root_is_idempotent_and_conflict_is_transactional() {
    let family = family("campaign-root-conflict");
    let mut campaign = plan(vec![root("same", &family, "110")]);
    let already = campaign
        .try_insert_root(root("same", &family, "110"))
        .unwrap();
    assert!(matches!(
        already,
        CampaignRootInsertion::AlreadyPresent { .. }
    ));
    let before = campaign.clone();
    assert!(matches!(
        campaign.try_insert_root(root("same", &family, "101")),
        Err(CampaignPlanError::RootConflict { .. })
    ));
    assert_eq!(campaign, before);
    assert_eq!(campaign.stats(), before.stats());
}

fn shared_child_plan(
    reverse_edges: bool,
) -> (CampaignPlan, Vec<rustred::campaign::CampaignJobKey>) {
    let family = family("campaign-shared-child");
    let mut campaign = plan(vec![
        root("left", &family, "110"),
        root("right", &family, "101"),
    ]);
    let left = campaign
        .root(&CampaignRootId::try_new("left").unwrap())
        .unwrap()
        .job()
        .clone();
    let right = campaign
        .root(&CampaignRootId::try_new("right").unwrap())
        .unwrap()
        .job()
        .clone();
    let parents = if reverse_edges {
        vec![right.clone(), left.clone()]
    } else {
        vec![left.clone(), right.clone()]
    };
    let mut child = None;
    for parent in parents {
        let insertion = campaign
            .try_add_strict_subsector_dependency(
                &parent,
                SectorMask::try_from_bit_string("100").unwrap(),
            )
            .unwrap();
        child = Some(insertion.child().clone());
    }
    (campaign, vec![left, right, child.unwrap()])
}

#[test]
fn shared_child_and_ready_antichains_are_insertion_order_independent() {
    let (mut campaign, keys) = shared_child_plan(false);
    let (reordered, _) = shared_child_plan(true);
    assert_eq!(campaign, reordered);
    assert_eq!(campaign.stats().jobs(), 3);
    assert_eq!(campaign.stats().dependency_edges(), 2);
    assert_eq!(campaign.stats().dependency_witness_positions(), 2);

    let repeated = campaign
        .try_add_strict_subsector_dependency(
            &keys[0],
            SectorMask::try_from_bit_string("100").unwrap(),
        )
        .unwrap();
    assert!(matches!(
        repeated,
        CampaignDependencyInsertion::AlreadyPresent { .. }
    ));
    assert_eq!(campaign, reordered);

    let child = keys[2].clone();
    assert_eq!(
        campaign.try_ready_job_antichain(&BTreeSet::new()).unwrap(),
        BTreeSet::from([child.clone()])
    );
    let completed_child = BTreeSet::from([child]);
    assert_eq!(
        campaign.try_ready_job_antichain(&completed_child).unwrap(),
        BTreeSet::from([keys[0].clone(), keys[1].clone()])
    );
    assert!(matches!(
        campaign.try_ready_job_antichain(&BTreeSet::from([keys[0].clone()])),
        Err(CampaignPlanError::CompletionPrefixNotClosed { .. })
    ));
    campaign.verify().unwrap();
}

#[test]
fn every_non_strict_dependency_rejects_without_mutation() {
    let (mut campaign, keys) = shared_child_plan(false);
    let parent = keys[0].clone();
    for invalid in ["110", "111", "011"] {
        let before = campaign.clone();
        assert!(matches!(
            campaign.try_add_strict_subsector_dependency(
                &parent,
                SectorMask::try_from_bit_string(invalid).unwrap()
            ),
            Err(CampaignPlanError::NonDescendingDependency { .. })
        ));
        assert_eq!(campaign, before);
        assert_eq!(campaign.stats(), before.stats());
    }
    let before = campaign.clone();
    assert!(matches!(
        campaign.try_add_strict_subsector_dependency(
            &parent,
            SectorMask::try_from_bit_string("10").unwrap()
        ),
        Err(CampaignPlanError::WrongDependencyArity { .. })
    ));
    assert_eq!(campaign, before);

    let foreign_family = family("campaign-foreign-parent");
    let foreign = plan(vec![root("foreign", &foreign_family, "111")]);
    let foreign_key = foreign.intrinsic_jobs().next().unwrap();
    assert!(matches!(
        campaign.try_add_strict_subsector_dependency(
            foreign_key,
            SectorMask::try_from_bit_string("110").unwrap()
        ),
        Err(CampaignPlanError::UnknownJob { .. })
    ));
}

#[test]
fn planning_is_target_driven_and_does_not_enumerate_all_sectors() {
    let family = family("campaign-no-eager-sectors");
    let campaign = plan(vec![root("top", &family, "111")]);
    assert_eq!(campaign.stats().roots(), 1);
    assert_eq!(campaign.stats().jobs(), 1);
    assert_eq!(campaign.stats().dependency_edges(), 0);
    assert_eq!(
        campaign
            .try_ready_job_antichain(&BTreeSet::new())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn configurable_root_id_and_exact_limits_are_enforced() {
    let family = family("campaign-limits");
    let id = CampaignRootId::try_new_with_limit("12345", 5).unwrap();
    let spec = CampaignRootSpec::new(
        id,
        Arc::clone(&family),
        SectorMask::try_from_bit_string("111").unwrap(),
    );
    let mut limits = CampaignPlanLimits::default();
    limits.max_root_id_bytes = 5;
    limits.max_total_root_id_bytes = 5;
    let mut campaign = CampaignPlan::compile(
        vec![spec],
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
    .unwrap();
    let before = campaign.clone();
    assert!(matches!(
        campaign.try_insert_root(root("x", &family, "110")),
        Err(CampaignPlanError::ResourceLimit {
            resource: "campaign root identifier bytes",
            requested: 6,
            limit: 5,
        })
    ));
    assert_eq!(campaign, before);
    assert_eq!(campaign.stats(), before.stats());
}

#[test]
fn equal_family_identity_with_different_retained_limits_is_rejected_transactionally() {
    let original = family("campaign-family-policy");
    let mut different_limits = original.limits();
    different_limits.max_scalar_products += 1;
    let rebuilt = Arc::new(
        IntegralFamily::new_with_limits(
            original.name(),
            original.loop_momenta().to_vec(),
            original.external_momenta().to_vec(),
            original.coefficient_context().clone(),
            original.dimension().clone(),
            original.denominators().to_vec(),
            original.external_gram().to_vec(),
            original.power_shifts().to_vec(),
            different_limits,
        )
        .unwrap(),
    );
    assert_eq!(original.fingerprint_ref(), rebuilt.fingerprint_ref());

    let mut campaign = plan(vec![root("first", &original, "111")]);
    let before = campaign.clone();
    assert!(matches!(
        campaign.try_insert_root(root("second", &rebuilt, "111")),
        Err(CampaignPlanError::FamilyResourcePolicyConflict { .. })
    ));
    assert_eq!(campaign, before);
    assert_eq!(campaign.stats(), before.stats());
}

#[test]
fn dependency_witness_limit_rejects_before_plan_mutation() {
    let family = family("campaign-witness-limit");
    let mut limits = CampaignPlanLimits::default();
    limits.max_dependency_witness_positions = 0;
    let mut campaign = CampaignPlan::compile(
        vec![root("parent", &family, "111")],
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
    .unwrap();
    let parent = campaign.intrinsic_jobs().next().unwrap().clone();
    let before = campaign.clone();
    assert!(matches!(
        campaign.try_add_strict_subsector_dependency(
            &parent,
            SectorMask::try_from_bit_string("110").unwrap(),
        ),
        Err(CampaignPlanError::ResourceLimit {
            resource: "campaign dependency witness positions",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(campaign, before);
    assert_eq!(campaign.stats(), before.stats());
}

#[test]
fn nonvacuum_family_and_foreign_equal_key_use_the_generic_canonical_path() {
    let external = one_loop_one_external_family("campaign-one-loop-external");
    let mut external_plan = plan(vec![root("external", &external, "11")]);
    let external_parent = external_plan.intrinsic_jobs().next().unwrap().clone();
    external_plan
        .try_add_strict_subsector_dependency(
            &external_parent,
            SectorMask::try_from_bit_string("10").unwrap(),
        )
        .unwrap();
    assert_eq!(external_plan.stats().jobs(), 2);
    external_plan.verify().unwrap();

    let first = family("campaign-foreign-equal-key");
    let rebuilt = family("campaign-foreign-equal-key");
    let mut destination = plan(vec![root("destination", &first, "110")]);
    let source = plan(vec![root("source", &rebuilt, "110")]);
    let foreign_but_equal_parent = source.intrinsic_jobs().next().unwrap();
    destination
        .try_add_strict_subsector_dependency(
            foreign_but_equal_parent,
            SectorMask::try_from_bit_string("100").unwrap(),
        )
        .unwrap();
    destination.verify().unwrap();
}
