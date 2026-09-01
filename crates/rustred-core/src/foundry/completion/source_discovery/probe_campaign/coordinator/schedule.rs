use crate::foundry::completion::UncoveredPartition;

use super::super::super::boundary_simplex::BoundarySimplexSamplingProfile;
use super::{
    ProbeCoordinatorClass, ProbeCoordinatorClassSchedule, ProbeCoordinatorConfig,
    ProbeCoordinatorFailure,
};

const DIMENSION_SLOTS: &str = "present free-dimension slots";
const PRESENT_DIMENSIONS: &str = "present free dimensions";
const CLASSES: &str = "canonical boundary classes";

pub(crate) fn try_build_class_schedule(
    partition: &UncoveredPartition,
    arity: usize,
    config: &ProbeCoordinatorConfig,
) -> Result<ProbeCoordinatorClassSchedule, ProbeCoordinatorFailure> {
    if partition.is_empty() {
        return Err(ProbeCoordinatorFailure::EmptyUncoveredPartition);
    }
    for (box_ordinal, lattice_box) in partition.boxes().iter().enumerate() {
        if lattice_box.arity() != arity {
            return Err(ProbeCoordinatorFailure::WrongPartitionBoxArity {
                box_ordinal,
                expected: arity,
                actual: lattice_box.arity(),
            });
        }
    }

    let limits = config.limits();
    let slot_count =
        arity
            .checked_add(1)
            .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow {
                resource: DIMENSION_SLOTS,
            })?;
    check_limit(DIMENSION_SLOTS, slot_count, limits.max_present_dimensions)?;
    let mut present = Vec::new();
    present.try_reserve_exact(slot_count).map_err(|_| {
        ProbeCoordinatorFailure::AllocationFailure {
            resource: DIMENSION_SLOTS,
            requested: slot_count,
        }
    })?;
    present.resize(slot_count, false);
    for lattice_box in partition.boxes() {
        present[lattice_box.free_dimension()] = true;
    }
    let present_count = present.iter().filter(|&&is_present| is_present).count();
    check_limit(
        PRESENT_DIMENSIONS,
        present_count,
        limits.max_present_dimensions,
    )?;
    let mut dimensions = Vec::new();
    dimensions.try_reserve_exact(present_count).map_err(|_| {
        ProbeCoordinatorFailure::AllocationFailure {
            resource: PRESENT_DIMENSIONS,
            requested: present_count,
        }
    })?;
    dimensions.extend(
        present
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(dimension, &is_present)| is_present.then_some(dimension)),
    );
    drop(present);

    let mut class_count = 0usize;
    for &parent_dimension in &dimensions {
        class_count = class_count
            .checked_add(parent_dimension + 1)
            .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow { resource: CLASSES })?;
    }
    check_limit(CLASSES, class_count, limits.max_classes_per_epoch)?;
    let mut classes = Vec::new();
    classes.try_reserve_exact(class_count).map_err(|_| {
        ProbeCoordinatorFailure::AllocationFailure {
            resource: CLASSES,
            requested: class_count,
        }
    })?;
    let maximal_effective_dimension = dimensions[0];
    for effective_dimension in (0..=maximal_effective_dimension).rev() {
        for &parent_dimension in &dimensions {
            if parent_dimension < effective_dimension {
                continue;
            }
            let profile = if effective_dimension == 0 {
                BoundarySimplexSamplingProfile::Vertex
            } else {
                BoundarySimplexSamplingProfile::Simplex {
                    interior_margin: config.interior_margin(),
                    polynomial_degree_ceiling: config.polynomial_degree_ceiling(),
                }
            };
            classes.push(ProbeCoordinatorClass::new(
                classes.len(),
                effective_dimension,
                parent_dimension,
                profile,
            ));
        }
    }
    if classes.len() != class_count {
        return Err(ProbeCoordinatorFailure::Invariant {
            detail: "canonical class count differed from its exact preflight",
        });
    }
    Ok(ProbeCoordinatorClassSchedule::new(dimensions, classes))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ProbeCoordinatorFailure> {
    if requested > limit {
        Err(ProbeCoordinatorFailure::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
