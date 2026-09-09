use super::super::{SpiredPreparedRowSpan, SpiredPreparedRowView, SpiredPreparedTermRole};

pub(super) const PREPARED_TAPE_ROWS: &str = "target-run prepared structural tape rows";
pub(super) const PREPARED_TAPE_TERM_ROLES: &str = "target-run prepared structural tape term roles";

/// Allocation-amortized structural tape retained across target probes.
///
/// Row headers and roles occupy two flat arenas. No row owns a `Vec`, boxed
/// role slice, or independent allocation.
#[derive(Debug, Default)]
pub(super) struct SpiredPreparedRowTape {
    rows: Vec<SpiredPreparedRowSpan>,
    term_roles: Vec<SpiredPreparedTermRole>,
}

impl SpiredPreparedRowTape {
    pub(super) const fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub(super) const fn term_role_count(&self) -> usize {
        self.term_roles.len()
    }

    pub(super) fn row(&self, ordinal: usize) -> Option<&SpiredPreparedRowSpan> {
        self.rows.get(ordinal)
    }

    pub(super) fn row_view(&self, ordinal: usize) -> Option<SpiredPreparedRowView<'_>> {
        self.rows.get(ordinal)?.try_view(&self.term_roles)
    }

    /// Check both retained-payload caps before reserving either arena.
    pub(super) fn try_reserve_new_row(
        &mut self,
        row_ordinal: usize,
        term_role_count: usize,
        max_rows: usize,
        max_term_roles: usize,
    ) -> Result<(), SpiredPreparedRowTapeError> {
        if row_ordinal != self.rows.len() {
            return Err(SpiredPreparedRowTapeError::Invariant {
                detail: "a new flat prepared row was not scheduler-contiguous",
            });
        }
        let requested_rows = self.rows.len().checked_add(1).ok_or(
            SpiredPreparedRowTapeError::ResourceCountOverflow {
                resource: PREPARED_TAPE_ROWS,
            },
        )?;
        let requested_term_roles = self.term_roles.len().checked_add(term_role_count).ok_or(
            SpiredPreparedRowTapeError::ResourceCountOverflow {
                resource: PREPARED_TAPE_TERM_ROLES,
            },
        )?;
        check_limit(PREPARED_TAPE_ROWS, requested_rows, max_rows)?;
        check_limit(
            PREPARED_TAPE_TERM_ROLES,
            requested_term_roles,
            max_term_roles,
        )?;

        self.rows
            .try_reserve(1)
            .map_err(|_| SpiredPreparedRowTapeError::AllocationFailure {
                resource: PREPARED_TAPE_ROWS,
                requested: requested_rows,
            })?;
        self.term_roles.try_reserve(term_role_count).map_err(|_| {
            SpiredPreparedRowTapeError::AllocationFailure {
                resource: PREPARED_TAPE_TERM_ROLES,
                requested: requested_term_roles,
            }
        })?;
        Ok(())
    }

    pub(super) fn term_roles_mut(&mut self) -> &mut Vec<SpiredPreparedTermRole> {
        &mut self.term_roles
    }

    /// Retain one row emitted by the paired structural preparation after the
    /// caller has reserved both flat arenas and checked its chronology.
    ///
    /// This step is deliberately infallible. Structural preparation has
    /// already committed its registry and census, so introducing a later
    /// fallible tape mutation would permit a half-advanced workspace. The
    /// producer and caller establish these private invariants before entry;
    /// debug assertions retain development-time diagnostics without creating
    /// a spurious transactional boundary.
    pub(super) fn push_prepared_row_after_preflight(
        &mut self,
        row: SpiredPreparedRowSpan,
    ) -> usize {
        let ordinal = self.rows.len();
        debug_assert_eq!(row.ordinal(), ordinal);
        debug_assert_eq!(
            row.try_view(&self.term_roles)
                .map(|view| view.term_roles().len()),
            Some(row.term_role_count())
        );
        self.rows.push(row);
        ordinal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpiredPreparedRowTapeError {
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredPreparedRowTapeError> {
    if requested > limit {
        Err(SpiredPreparedRowTapeError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
