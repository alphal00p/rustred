//! Black-box validation of the generic AnalyzeSectors-style inventory.
//!
//! Concrete one- and two-loop families are validation fixtures only.  The
//! compiler receives no loop count, topology tag, recurrence, or master list.

use rustred::{
    AffineDenominator, Coefficient, CoefficientContext, CutConstraint,
    FamilySectorInventoryCompiler, FamilySectorInventoryError, FamilySectorInventoryLimits,
    FamilySectorInventoryStatus, IntegralFamily, IntegralOrderingPolicy, PowerShiftPolicy,
    SectorMask, SectorPattern, SectorRestrictions, ZeroSectorError,
};

fn affine(
    constant: Coefficient,
    coefficients: impl IntoIterator<Item = Coefficient>,
) -> AffineDenominator {
    AffineDenominator::new(constant, coefficients.into_iter().collect())
}

fn mask(bits: &str) -> SectorMask {
    SectorMask::try_from_bit_string(bits).unwrap()
}

fn massive_tadpole(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(context.parse("-m2").unwrap(), [context.one()])],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn massive_sunset(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    let mass = context.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(
                mass.clone(),
                [context.one(), context.zero(), context.zero()],
            ),
            affine(
                mass.clone(),
                [context.zero(), context.zero(), context.one()],
            ),
            affine(mass, [context.one(), context.integer(-2), context.one()]),
        ],
        Vec::new(),
        vec![context.zero(), context.zero(), context.zero()],
    )
    .unwrap()
}

fn shifted_off_shell_bubble(name: &str, shift: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "s", "nu"]);
    let s = context.parameter("s").unwrap();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(context.zero(), [context.one(), context.zero()]),
            affine(s.clone(), [context.one(), context.integer(2)]),
        ],
        vec![vec![s]],
        vec![context.zero(), context.parse(shift).unwrap()],
    )
    .unwrap()
}

fn unrestricted(
    family: &IntegralFamily,
    limits: FamilySectorInventoryLimits,
) -> Result<rustred::FamilySectorInventoryCertificate, FamilySectorInventoryError> {
    FamilySectorInventoryCompiler::compile(
        family,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
}

#[test]
fn one_loop_inventory_retains_proof_and_unresolved_witness_without_master_inference() {
    let family = massive_tadpole("inventory-massive-tadpole");
    let certificate = unrestricted(&family, FamilySectorInventoryLimits::default()).unwrap();
    assert_eq!(certificate.entries().len(), 2);
    assert_eq!(certificate.stats().proved_zero(), 1);
    assert_eq!(certificate.stats().unresolved(), 1);
    assert_eq!(certificate.stats().excluded(), 0);
    assert_eq!(certificate.stats().resource_limited(), 0);
    assert_eq!(certificate.stats().failed(), 0);
    assert!(certificate.monotone_zero_closure_verified());

    let FamilySectorInventoryStatus::ProvedZero(zero) = certificate.status(&mask("0")).unwrap()
    else {
        panic!("the empty tadpole face must carry an analytic zero proof")
    };
    zero.replay(&family).unwrap();

    let unresolved = certificate.status(&mask("1")).unwrap();
    assert!(unresolved.is_unresolved_after_zero_test());
    let FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(witness) = unresolved else {
        unreachable!()
    };
    assert_eq!(witness.raw_sector(), &mask("1"));
    assert_eq!(witness.effective_sector(), &mask("1"));

    assert_eq!(
        certificate
            .unresolved_solve_order()
            .iter()
            .map(|entry| entry.sector().to_bit_string())
            .collect::<Vec<_>>(),
        ["1"]
    );
    certificate.replay(&family).unwrap();

    let foreign = massive_tadpole("inventory-foreign-tadpole");
    assert!(matches!(
        certificate.replay(&foreign),
        Err(FamilySectorInventoryError::ForeignFamily)
    ));
}

#[test]
fn two_loop_sunset_order_places_every_included_subsector_before_its_supersector() {
    let family = massive_sunset("inventory-massive-sunset");
    let certificate = unrestricted(&family, FamilySectorInventoryLimits::default()).unwrap();
    assert_eq!(certificate.entries().len(), 8);
    assert_eq!(certificate.stats().proved_zero(), 4);
    assert_eq!(certificate.stats().unresolved(), 4);
    assert_eq!(certificate.stats().dependency_checks(), 6);
    assert_eq!(certificate.stats().distinct_effective_masks(), 8);

    for entry in certificate.entries() {
        if entry.sector().active_count() <= 1 {
            assert!(entry.status().is_proved_zero(), "{}", entry.sector());
        } else {
            assert!(
                entry.status().is_unresolved_after_zero_test(),
                "{}",
                entry.sector()
            );
        }
    }
    let order = certificate.unresolved_solve_order();
    assert_eq!(
        order
            .iter()
            .map(|entry| entry.sector().to_bit_string())
            .collect::<Vec<_>>(),
        ["011", "101", "110", "111"]
    );
    assert!(
        order
            .windows(2)
            .all(|pair| pair[0].corner_key() < pair[1].corner_key())
    );

    // Explicitly audit every included proper-subsector dependency, not only
    // immediate edges of this concrete lattice.
    for (subsector_position, subsector) in order.iter().enumerate() {
        for (supersector_position, supersector) in order.iter().enumerate() {
            if subsector
                .sector()
                .is_strict_subsector_of(supersector.sector())
                .unwrap()
            {
                assert!(
                    subsector_position < supersector_position,
                    "{} must precede {}",
                    subsector.sector(),
                    supersector.sector(),
                );
            }
        }
    }
    certificate.replay(&family).unwrap();
}

#[test]
fn cuts_and_patterns_remain_owned_exclusions_not_zero_or_nonzero_claims() {
    let family = massive_sunset("inventory-restricted-sunset");
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(3, [0]).unwrap(),
        SectorPattern::try_from_string("*0*").unwrap(),
    )
    .unwrap();
    let certificate = FamilySectorInventoryCompiler::compile(
        &family,
        restrictions.clone(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        FamilySectorInventoryLimits::default(),
    )
    .unwrap();
    assert_eq!(certificate.restrictions(), &restrictions);
    assert_eq!(certificate.stats().excluded(), 6);
    assert_eq!(certificate.stats().proved_zero(), 1);
    assert_eq!(certificate.stats().unresolved(), 1);
    assert!(matches!(
        certificate.status(&mask("001")),
        Some(FamilySectorInventoryStatus::Excluded(exclusion))
            if exclusion.violates_cut()
    ));
    assert!(matches!(
        certificate.status(&mask("110")),
        Some(FamilySectorInventoryStatus::Excluded(exclusion))
            if exclusion.violates_pattern()
    ));
    assert!(matches!(
        certificate.status(&mask("100")),
        Some(FamilySectorInventoryStatus::ProvedZero(_))
    ));
    assert!(matches!(
        certificate.status(&mask("101")),
        Some(FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(_))
    ));
    assert_eq!(
        certificate
            .unresolved_solve_order()
            .iter()
            .map(|entry| entry.sector().to_bit_string())
            .collect::<Vec<_>>(),
        ["101"]
    );
    certificate.replay(&family).unwrap();
}

#[test]
fn formal_power_support_is_retained_and_unsupported_shift_or_cut_fails_typed() {
    let family = shifted_off_shell_bubble("inventory-symbolic-shift", "nu");
    let certificate = unrestricted(&family, FamilySectorInventoryLimits::default()).unwrap();
    assert_eq!(certificate.power_support(), &mask("01"));
    assert!(matches!(
        certificate.status(&mask("00")),
        Some(FamilySectorInventoryStatus::ProvedZero(certificate))
            if certificate.effective_sector() == &mask("01")
    ));
    assert!(matches!(
        certificate.status(&mask("10")),
        Some(FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(witness))
            if witness.effective_sector() == &mask("11")
    ));
    assert_eq!(
        certificate
            .unresolved_solve_order()
            .iter()
            .map(|entry| entry.sector().to_bit_string())
            .collect::<Vec<_>>(),
        ["10", "11"]
    );
    assert!(!certificate.zero_sector_domain().conditions().is_empty());
    certificate.replay(&family).unwrap();

    let integer = shifted_off_shell_bubble("inventory-integer-shift", "1");
    assert!(matches!(
        unrestricted(&integer, FamilySectorInventoryLimits::default()),
        Err(FamilySectorInventoryError::ZeroSector(
            ZeroSectorError::UnsupportedNonzeroIntegerPowerShift { denominator: 1 }
        ))
    ));

    let shifted_cut = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(2, [1]).unwrap(),
        SectorPattern::any(2).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        FamilySectorInventoryCompiler::compile(
            &family,
            shifted_cut,
            PowerShiftPolicy::FormalGeneric,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            FamilySectorInventoryLimits::default(),
        ),
        Err(FamilySectorInventoryError::ZeroSector(
            ZeroSectorError::UnsupportedShiftedCut { denominator: 1 }
        ))
    ));
}

#[test]
fn outer_inventory_and_dependency_budgets_fail_closed() {
    let family = massive_sunset("inventory-outer-limits");

    let mut limits = FamilySectorInventoryLimits::default();
    limits.max_inventory_entries = 7;
    assert!(matches!(
        unrestricted(&family, limits),
        Err(FamilySectorInventoryError::ResourceLimit {
            resource: "family sector inventory entries",
            requested: 8,
            limit: 7,
        })
    ));

    limits = FamilySectorInventoryLimits::default();
    limits.max_unresolved_solve_entries = 3;
    assert!(matches!(
        unrestricted(&family, limits),
        Err(FamilySectorInventoryError::ResourceLimit {
            resource: "unresolved sector solve entries",
            requested: 4,
            limit: 3,
        })
    ));

    limits = FamilySectorInventoryLimits::default();
    limits.max_dependency_checks = 5;
    assert!(matches!(
        unrestricted(&family, limits),
        Err(FamilySectorInventoryError::ResourceLimit {
            resource: "unresolved sector dependency checks",
            requested: 6,
            limit: 5,
        })
    ));

    limits = FamilySectorInventoryLimits::default();
    limits.max_binding_bytes = 0;
    assert!(matches!(
        unrestricted(&family, limits),
        Err(FamilySectorInventoryError::ResourceLimit {
            resource: "family sector inventory binding bytes",
            limit: 0,
            ..
        })
    ));

    limits = FamilySectorInventoryLimits::default();
    limits.zero_sectors.max_sectors = 7;
    assert!(matches!(
        unrestricted(&family, limits),
        Err(FamilySectorInventoryError::ZeroSector(
            ZeroSectorError::ResourceLimit {
                resource: "raw sectors",
                requested: 8,
                limit: 7,
            }
        ))
    ));
}

#[test]
fn per_sector_resource_statuses_and_exact_limits_survive_replay() {
    let family = massive_sunset("inventory-retained-resource-status");
    let mut limits = FamilySectorInventoryLimits::default();
    limits.zero_sectors.max_rank_rows = 0;
    let certificate = unrestricted(&family, limits).unwrap();
    assert_eq!(certificate.limits(), limits);
    assert!(certificate.stats().resource_limited() > 0);
    assert_eq!(certificate.stats().failed(), 0);
    assert_eq!(certificate.stats().unresolved_solve_entries(), 0);
    assert!(certificate.entries().iter().any(|entry| {
        matches!(
            entry.status(),
            FamilySectorInventoryStatus::ResourceLimited(resource)
                if resource.resource() == "rank matrix rows" && resource.limit() == 0
        )
    }));
    assert!(
        certificate.unresolved_solve_order().is_empty(),
        "resource-limited sectors must never be promoted into the solve queue"
    );
    certificate.replay(&family).unwrap();
}

#[test]
fn restriction_arity_is_checked_before_analysis() {
    let family = massive_tadpole("inventory-wrong-restriction-arity");
    assert!(matches!(
        FamilySectorInventoryCompiler::compile(
            &family,
            SectorRestrictions::unrestricted(2).unwrap(),
            PowerShiftPolicy::FormalGeneric,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            FamilySectorInventoryLimits::default(),
        ),
        Err(FamilySectorInventoryError::WrongRestrictionsArity {
            expected: 1,
            actual: 2,
        })
    ));
}
