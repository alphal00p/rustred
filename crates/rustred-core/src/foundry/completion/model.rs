use super::CompletionGeometryError;

/// One point in a sector's nonnegative local-coordinate lattice.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct LatticePoint {
    coordinates: Box<[u64]>,
}

impl LatticePoint {
    pub(crate) fn try_new(
        coordinates: impl IntoIterator<Item = u64>,
    ) -> Result<Self, CompletionGeometryError> {
        let mut retained = Vec::new();
        for coordinate in coordinates {
            let requested = retained.len().checked_add(1).ok_or(
                CompletionGeometryError::ResourceCountOverflow {
                    resource: "lattice-point coordinates",
                },
            )?;
            retained.try_reserve_exact(1).map_err(|_| {
                CompletionGeometryError::AllocationFailure {
                    resource: "lattice-point coordinates",
                    requested,
                }
            })?;
            retained.push(coordinate);
        }
        Self::try_from_preallocated(retained)
    }

    pub(super) fn try_from_preallocated(
        coordinates: Vec<u64>,
    ) -> Result<Self, CompletionGeometryError> {
        if coordinates.is_empty() {
            return Err(CompletionGeometryError::EmptyCoordinateSpace);
        }
        Ok(Self {
            coordinates: coordinates.into_boxed_slice(),
        })
    }

    pub(crate) fn coordinates(&self) -> &[u64] {
        &self.coordinates
    }

    pub(crate) fn arity(&self) -> usize {
        self.coordinates.len()
    }
}

/// One disjoint axis-aligned box in `N^r`.
///
/// `None` is an unbounded upper endpoint.  Every retained lower endpoint is
/// finite and every finite upper endpoint is at least its lower endpoint.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct LatticeBox {
    lower: Box<[u64]>,
    upper: Box<[Option<u64>]>,
}

impl LatticeBox {
    pub(crate) fn try_new(
        lower: impl IntoIterator<Item = u64>,
        upper: impl IntoIterator<Item = Option<u64>>,
    ) -> Result<Self, CompletionGeometryError> {
        let mut retained_lower = Vec::new();
        for coordinate in lower {
            let requested = retained_lower.len().checked_add(1).ok_or(
                CompletionGeometryError::ResourceCountOverflow {
                    resource: "lattice-box lower coordinates",
                },
            )?;
            retained_lower.try_reserve_exact(1).map_err(|_| {
                CompletionGeometryError::AllocationFailure {
                    resource: "lattice-box lower coordinates",
                    requested,
                }
            })?;
            retained_lower.push(coordinate);
        }
        let mut retained_upper = Vec::new();
        for coordinate in upper {
            let requested = retained_upper.len().checked_add(1).ok_or(
                CompletionGeometryError::ResourceCountOverflow {
                    resource: "lattice-box upper coordinates",
                },
            )?;
            retained_upper.try_reserve_exact(1).map_err(|_| {
                CompletionGeometryError::AllocationFailure {
                    resource: "lattice-box upper coordinates",
                    requested,
                }
            })?;
            retained_upper.push(coordinate);
        }
        Self::try_from_preallocated(retained_lower, retained_upper)
    }

    pub(super) fn try_full(arity: usize) -> Result<Self, CompletionGeometryError> {
        if arity == 0 {
            return Err(CompletionGeometryError::EmptyCoordinateSpace);
        }
        let mut lower = try_vec("uncovered-box lower coordinates", arity)?;
        lower.resize(arity, 0);
        let mut upper = try_vec("uncovered-box upper coordinates", arity)?;
        upper.resize(arity, None);
        Ok(Self {
            lower: lower.into_boxed_slice(),
            upper: upper.into_boxed_slice(),
        })
    }

    pub(super) fn try_clone_fallible(&self) -> Result<Self, CompletionGeometryError> {
        let mut lower = try_vec("uncovered-box lower coordinates", self.arity())?;
        lower.extend_from_slice(&self.lower);
        let mut upper = try_vec("uncovered-box upper coordinates", self.arity())?;
        upper.extend_from_slice(&self.upper);
        Ok(Self {
            lower: lower.into_boxed_slice(),
            upper: upper.into_boxed_slice(),
        })
    }

    pub(super) fn try_from_preallocated(
        lower: Vec<u64>,
        upper: Vec<Option<u64>>,
    ) -> Result<Self, CompletionGeometryError> {
        if lower.is_empty() {
            return Err(CompletionGeometryError::EmptyCoordinateSpace);
        }
        if lower.len() != upper.len() {
            return Err(CompletionGeometryError::WrongArity {
                object: "lattice-box upper endpoints",
                expected: lower.len(),
                actual: upper.len(),
            });
        }
        for (position, (&lower, &upper)) in lower.iter().zip(&upper).enumerate() {
            if let Some(upper) = upper
                && upper < lower
            {
                return Err(CompletionGeometryError::InvalidBoxBounds {
                    position,
                    lower,
                    upper,
                });
            }
        }
        Ok(Self {
            lower: lower.into_boxed_slice(),
            upper: upper.into_boxed_slice(),
        })
    }

    pub(crate) fn lower(&self) -> &[u64] {
        &self.lower
    }

    pub(crate) fn upper(&self) -> &[Option<u64>] {
        &self.upper
    }

    pub(crate) fn arity(&self) -> usize {
        self.lower.len()
    }

    pub(crate) fn free_dimension(&self) -> usize {
        self.upper.iter().filter(|upper| upper.is_none()).count()
    }

    pub(crate) fn varying_dimension(&self) -> usize {
        self.lower
            .iter()
            .zip(&self.upper)
            .filter(|&(&lower, &upper)| upper.is_none_or(|upper| lower < upper))
            .count()
    }

    pub(crate) fn contains(&self, point: &LatticePoint) -> bool {
        point.arity() == self.arity()
            && self
                .lower
                .iter()
                .zip(&self.upper)
                .zip(point.coordinates())
                .all(|((&lower, &upper), &coordinate)| {
                    coordinate >= lower && upper.is_none_or(|upper| coordinate <= upper)
                })
    }

    pub(super) fn intersects_orthant(&self, origin: &LatticePoint) -> bool {
        self.arity() == origin.arity()
            && self
                .upper
                .iter()
                .zip(origin.coordinates())
                .all(|(&upper, &coordinate)| upper.is_none_or(|upper| upper >= coordinate))
    }

    pub(super) fn intersects_box(&self, other: &Self) -> bool {
        self.arity() == other.arity()
            && self
                .lower
                .iter()
                .zip(&self.upper)
                .zip(other.lower.iter().zip(&other.upper))
                .all(
                    |((&left_lower, &left_upper), (&right_lower, &right_upper))| {
                        upper_contains(left_upper, right_lower)
                            && upper_contains(right_upper, left_lower)
                    },
                )
    }

    pub(super) fn contains_box(&self, other: &Self) -> bool {
        self.arity() == other.arity()
            && self
                .lower
                .iter()
                .zip(&self.upper)
                .zip(other.lower.iter().zip(&other.upper))
                .all(
                    |((&outer_lower, &outer_upper), (&inner_lower, &inner_upper))| {
                        outer_lower <= inner_lower && upper_contains_upper(outer_upper, inner_upper)
                    },
                )
    }

    pub(super) fn is_inside_orthant(&self, origin: &LatticePoint) -> bool {
        self.arity() == origin.arity()
            && self
                .lower
                .iter()
                .zip(origin.coordinates())
                .all(|(&lower, &coordinate)| lower >= coordinate)
    }

    pub(super) fn set_upper(&mut self, position: usize, upper: u64) {
        self.upper[position] = Some(self.upper[position].map_or(upper, |old| old.min(upper)));
    }

    pub(super) fn raise_lower(&mut self, position: usize, lower: u64) {
        self.lower[position] = self.lower[position].max(lower);
    }
}

fn upper_contains(upper: Option<u64>, coordinate: u64) -> bool {
    upper.is_none_or(|upper| coordinate <= upper)
}

fn upper_contains_upper(outer: Option<u64>, inner: Option<u64>) -> bool {
    match (outer, inner) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(outer), Some(inner)) => inner <= outer,
    }
}

/// Exact disjoint complement of a finite leading ideal or structural box union.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UncoveredPartition {
    boxes: Box<[LatticeBox]>,
    split_operations: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LatticeCardinality {
    Finite(usize),
    Infinite,
}

impl UncoveredPartition {
    pub(super) fn new(boxes: Vec<LatticeBox>, split_operations: usize) -> Self {
        Self {
            boxes: boxes.into_boxed_slice(),
            split_operations,
        }
    }

    pub(crate) fn boxes(&self) -> &[LatticeBox] {
        &self.boxes
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    pub(crate) fn is_finite(&self) -> bool {
        self.boxes.iter().all(|cell| cell.free_dimension() == 0)
    }

    pub(crate) fn split_operations(&self) -> usize {
        self.split_operations
    }

    pub(crate) fn containing_box(&self, point: &LatticePoint) -> Option<&LatticeBox> {
        self.boxes.iter().find(|cell| cell.contains(point))
    }

    pub(crate) fn try_cardinality(
        &self,
        max_points: usize,
    ) -> Result<LatticeCardinality, CompletionGeometryError> {
        if !self.is_finite() {
            return Ok(LatticeCardinality::Infinite);
        }
        let mut total = 0usize;
        for cell in &self.boxes {
            let mut cell_points = 1usize;
            for (&lower, &upper) in cell.lower().iter().zip(cell.upper()) {
                let upper = upper.ok_or(CompletionGeometryError::Invariant {
                    detail: "a finite uncovered partition retained an unbounded box",
                })?;
                let width = upper
                    .checked_sub(lower)
                    .and_then(|width| width.checked_add(1))
                    .and_then(|width| usize::try_from(width).ok())
                    .ok_or(CompletionGeometryError::ResourceCountOverflow {
                        resource: "finite uncovered lattice points",
                    })?;
                cell_points = cell_points.checked_mul(width).ok_or(
                    CompletionGeometryError::ResourceCountOverflow {
                        resource: "finite uncovered lattice points",
                    },
                )?;
                if cell_points > max_points {
                    return Err(CompletionGeometryError::ResourceLimit {
                        resource: "finite uncovered lattice points",
                        requested: cell_points,
                        limit: max_points,
                    });
                }
            }
            total = total.checked_add(cell_points).ok_or(
                CompletionGeometryError::ResourceCountOverflow {
                    resource: "finite uncovered lattice points",
                },
            )?;
            if total > max_points {
                return Err(CompletionGeometryError::ResourceLimit {
                    resource: "finite uncovered lattice points",
                    requested: total,
                    limit: max_points,
                });
            }
        }
        Ok(LatticeCardinality::Finite(total))
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, CompletionGeometryError> {
    let mut retained = Vec::new();
    retained.try_reserve_exact(capacity).map_err(|_| {
        CompletionGeometryError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(retained)
}
