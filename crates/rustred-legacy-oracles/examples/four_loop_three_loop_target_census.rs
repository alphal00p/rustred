use std::collections::{BTreeMap, BTreeSet};

use rustred_legacy_oracles::{
    FourLoopComponentScalarBranchKind, FourLoopComponentTransport,
    FourLoopComponentTransportConfig, FourLoopNextInventory, FourLoopNextInventoryConfig,
    MassiveVacuumMaster,
};

fn degrees(powers: &[i32]) -> (u64, u64) {
    let dots = powers
        .iter()
        .map(|power| u64::from(power.saturating_sub(1).max(0) as u32))
        .sum();
    let numerators = powers
        .iter()
        .map(|power| u64::from(power.saturating_neg().max(0) as u32))
        .sum();
    (dots, numerators)
}

fn main() {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default()).unwrap();
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())
            .unwrap();

    let mut plan_counts = BTreeMap::<MassiveVacuumMaster, usize>::new();
    let mut branch_counts = BTreeMap::<MassiveVacuumMaster, usize>::new();
    let mut call_counts = BTreeMap::<MassiveVacuumMaster, usize>::new();
    let mut targets = BTreeSet::<(MassiveVacuumMaster, Vec<i32>)>::new();
    let mut degree_counts = BTreeMap::<(MassiveVacuumMaster, u64, u64), usize>::new();
    let mut maxima = BTreeMap::<MassiveVacuumMaster, (u64, u64)>::new();
    let mut components = 0usize;
    let mut local_slots = 0usize;
    let mut n0 = 0usize;
    let mut n1 = 0usize;
    let mut branch_kinds = BTreeMap::<&'static str, usize>::new();

    for plan in transport.plans() {
        let genuine = plan
            .components()
            .iter()
            .find(|component| component.master().loops() == 3)
            .map(|component| component.master());
        let Some(genuine) = genuine else {
            continue;
        };
        components += plan.components().len();
        local_slots += plan
            .components()
            .iter()
            .map(|component| component.local_powers().len())
            .sum::<usize>();
        if plan.affine_image().is_some() {
            n1 += 1;
        } else {
            n0 += 1;
        }
        *plan_counts.entry(genuine).or_default() += 1;
        for branch in plan.scalar_branches() {
            *branch_counts.entry(genuine).or_default() += 1;
            let mut branch_targets = plan
                .components()
                .iter()
                .map(|component| (component.master(), component.local_powers().to_vec()))
                .collect::<Vec<_>>();
            match branch.kind() {
                FourLoopComponentScalarBranchKind::Base
                | FourLoopComponentScalarBranchKind::Constant => {
                    assert!(branch.lowered_component_powers().is_none());
                    let key = match branch.kind() {
                        FourLoopComponentScalarBranchKind::Base => "base",
                        FourLoopComponentScalarBranchKind::Constant => "constant",
                        _ => unreachable!(),
                    };
                    *branch_kinds.entry(key).or_default() += 1;
                }
                FourLoopComponentScalarBranchKind::Local {
                    component_index, ..
                } => {
                    let key = match plan.components()[component_index].master() {
                        MassiveVacuumMaster::T1 => "local-T1",
                        MassiveVacuumMaster::B4 => "local-B4",
                        MassiveVacuumMaster::F5 => "local-F5",
                        MassiveVacuumMaster::M6 => "local-M6",
                        MassiveVacuumMaster::S2 => "local-S2",
                    };
                    *branch_kinds.entry(key).or_default() += 1;
                    branch_targets[component_index].1 =
                        branch.lowered_component_powers().unwrap().to_vec();
                }
            }
            for (master, powers) in branch_targets {
                *call_counts.entry(master).or_default() += 1;
                targets.insert((master, powers));
            }
        }
    }

    for (master, powers) in &targets {
        let (dots, numerators) = degrees(powers);
        *degree_counts
            .entry((*master, dots, numerators))
            .or_default() += 1;
        maxima
            .entry(*master)
            .and_modify(|current| {
                current.0 = current.0.max(dots);
                current.1 = current.1.max(numerators);
            })
            .or_insert((dots, numerators));
    }

    println!("plan_counts={plan_counts:?}");
    println!("branch_counts={branch_counts:?}");
    println!("call_counts={call_counts:?}");
    println!("components={components} local_slots={local_slots} n0={n0} n1={n1}");
    println!("branch_kinds={branch_kinds:?}");
    println!("unique_targets={}", targets.len());
    println!("degree_counts={degree_counts:?}");
    println!("maxima={maxima:?}");
    println!("manifest_checksum=fnv1a64:9bb3c1a6d4ea7bdd");
}
