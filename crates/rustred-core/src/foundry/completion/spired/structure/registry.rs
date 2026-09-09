use std::collections::HashMap;

use crate::foundry::completion::stratum::{
    ProspectiveClassificationCost, ProspectiveColumnClassifier, ProspectiveColumnKind,
};

use super::{SpiredPreparedTermRole, SpiredStructuralPreparationError};

const CACHED_SHIFTS: &str = "cached prospective shifts";
const CACHED_SHIFT_COORDINATES: &str = "cached prospective-shift coordinate cells";
const PHYSICAL_COLUMNS: &str = "prospective physical columns";
const FORBIDDEN_COLUMNS: &str = "prospective forbidden columns";
const COLUMN_COORDINATE_CELLS: &str = "prospective physical-column coordinate cells";
const TARGET_SECTOR_CELLS: &str = "prospective target-sector cells";
const OWNER_PROBES: &str = "prospective immutable-owner probes";
const RETAINED_OWNER_WITNESSES: &str = "prospective retained owner witnesses";

#[derive(Debug)]
pub(super) struct StructuralRoleRegistry {
    classifier: ProspectiveColumnClassifier,
    roles: HashMap<Vec<i64>, SpiredPreparedTermRole>,
    coordinate_cells: usize,
    forbidden_columns: usize,
    target_sector_cells: usize,
    owner_probes: usize,
    retained_owner_witnesses: usize,
    max_shifts: usize,
    max_coordinate_cells: usize,
}

impl StructuralRoleRegistry {
    pub(super) fn new(
        classifier: ProspectiveColumnClassifier,
        max_shifts: usize,
        max_coordinate_cells: usize,
    ) -> Self {
        Self {
            classifier,
            roles: HashMap::new(),
            coordinate_cells: 0,
            forbidden_columns: 0,
            target_sector_cells: 0,
            owner_probes: 0,
            retained_owner_witnesses: 0,
            max_shifts,
            max_coordinate_cells,
        }
    }

    pub(super) fn try_classify(
        &mut self,
        shift: &[i64],
    ) -> Result<SpiredPreparedTermRole, SpiredStructuralPreparationError> {
        if let Some(role) = self.roles.get(shift) {
            return Ok(*role);
        }
        let next_count = checked_add(CACHED_SHIFTS, self.roles.len(), 1)?;
        check_limit(CACHED_SHIFTS, next_count, self.max_shifts)?;
        check_limit(
            PHYSICAL_COLUMNS,
            next_count,
            self.classifier.limits().max_physical_columns,
        )?;
        let next_cells = checked_add(CACHED_SHIFT_COORDINATES, self.coordinate_cells, shift.len())?;
        check_limit(
            CACHED_SHIFT_COORDINATES,
            next_cells,
            self.max_coordinate_cells,
        )?;
        check_limit(
            COLUMN_COORDINATE_CELLS,
            next_cells,
            self.classifier.limits().max_column_coordinate_cells,
        )?;

        let classification = self.classifier.try_classify_shift_with_cost(shift)?;
        let (target_sector_cells, owner_probes, retained_owner_witnesses) =
            self.try_preflight_cost(classification.cost())?;
        let mut forbidden_columns = self.forbidden_columns;
        let role = match classification.kind() {
            ProspectiveColumnKind::Target => SpiredPreparedTermRole::Target,
            ProspectiveColumnKind::Allowed => SpiredPreparedTermRole::Allowed,
            ProspectiveColumnKind::Forbidden => {
                let id = u32::try_from(self.forbidden_columns).map_err(|_| {
                    SpiredStructuralPreparationError::ForbiddenColumnIdNotRepresentable {
                        forbidden_columns: self.forbidden_columns,
                    }
                })?;
                forbidden_columns = checked_add(FORBIDDEN_COLUMNS, self.forbidden_columns, 1)?;
                SpiredPreparedTermRole::Forbidden(id)
            }
        };

        let mut owned = Vec::new();
        owned.try_reserve_exact(shift.len()).map_err(|_| {
            SpiredStructuralPreparationError::AllocationFailure {
                resource: CACHED_SHIFT_COORDINATES,
                requested: shift.len(),
            }
        })?;
        owned.extend_from_slice(shift);
        self.roles.try_reserve(1).map_err(|_| {
            SpiredStructuralPreparationError::AllocationFailure {
                resource: CACHED_SHIFTS,
                requested: next_count,
            }
        })?;
        if self.roles.insert(owned, role).is_some() {
            return Err(SpiredStructuralPreparationError::Invariant {
                detail: "a missing prospective shift replaced an existing structural role",
            });
        }
        self.coordinate_cells = next_cells;
        self.forbidden_columns = forbidden_columns;
        self.target_sector_cells = target_sector_cells;
        self.owner_probes = owner_probes;
        self.retained_owner_witnesses = retained_owner_witnesses;
        Ok(role)
    }

    pub(super) fn len(&self) -> usize {
        self.roles.len()
    }
    pub(super) const fn coordinate_cells(&self) -> usize {
        self.coordinate_cells
    }
    pub(super) const fn forbidden_columns(&self) -> usize {
        self.forbidden_columns
    }
    pub(super) const fn target_sector_cells(&self) -> usize {
        self.target_sector_cells
    }
    pub(super) const fn owner_probes(&self) -> usize {
        self.owner_probes
    }
    pub(super) const fn retained_owner_witnesses(&self) -> usize {
        self.retained_owner_witnesses
    }

    fn try_preflight_cost(
        &self,
        cost: ProspectiveClassificationCost,
    ) -> Result<(usize, usize, usize), SpiredStructuralPreparationError> {
        let target_sector_cells = checked_add(
            TARGET_SECTOR_CELLS,
            self.target_sector_cells,
            cost.target_sector_cells(),
        )?;
        check_limit(
            TARGET_SECTOR_CELLS,
            target_sector_cells,
            self.classifier.limits().max_target_sector_cells,
        )?;
        let owner_probes = checked_add(OWNER_PROBES, self.owner_probes, cost.owner_probes())?;
        check_limit(
            OWNER_PROBES,
            owner_probes,
            self.classifier.limits().max_owner_probes,
        )?;
        let retained = checked_add(
            RETAINED_OWNER_WITNESSES,
            self.retained_owner_witnesses,
            cost.retained_owner_witnesses(),
        )?;
        check_limit(
            RETAINED_OWNER_WITNESSES,
            retained,
            self.classifier.limits().max_retained_owner_witnesses,
        )?;
        Ok((target_sector_cells, owner_probes, retained))
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredStructuralPreparationError> {
    left.checked_add(right)
        .ok_or(SpiredStructuralPreparationError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredStructuralPreparationError> {
    if requested > limit {
        Err(SpiredStructuralPreparationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
