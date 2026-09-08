use crate::identity::{IntegralShift, TranslatedSourceRequest};

use super::SpiredFoundationError;

const ARITY: &str = "signed-L1 index-space arity";
const SOURCE_ROWS: &str = "signed-L1 source rows";
const DEPTH: &str = "signed-L1 depth";
const OFFSETS: &str = "signed-L1 offsets per depth shell";
const REQUESTS: &str = "signed-L1 source requests per depth shell";
const ENUMERATION_WORKSPACE: &str = "signed-L1 enumeration workspace entries";
const CHUNK_REQUESTS: &str = "signed-L1 source requests per chunk";
const CHUNK_OFFSET_COORDINATES: &str = "signed-L1 offset coordinate cells per chunk";

/// Complete work and peak-memory envelope for signed-L1 scheduling.
///
/// Global shell limits bound eventual work but cause no shell-sized
/// allocation. Chunk limits bound each materialized request window. A shell
/// is never truncated: any failed preflight returns no shell or no chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SignedL1ScheduleLimits {
    pub(crate) max_arity: usize,
    pub(crate) max_source_rows: usize,
    pub(crate) max_depth: usize,
    pub(crate) max_offsets_per_shell: usize,
    pub(crate) max_requests_per_shell: usize,
    pub(crate) max_enumeration_workspace_entries: usize,
    pub(crate) max_requests_per_chunk: usize,
    pub(crate) max_offset_coordinate_cells_per_chunk: usize,
}

impl Default for SignedL1ScheduleLimits {
    fn default() -> Self {
        Self {
            max_arity: 4_096,
            max_source_rows: 65_536,
            max_depth: 1_048_576,
            max_offsets_per_shell: 4_194_304,
            max_requests_per_shell: 67_108_864,
            max_enumeration_workspace_entries: 12_288,
            max_requests_per_chunk: 65_536,
            max_offset_coordinate_cells_per_chunk: 4_194_304,
        }
    }
}

/// Immutable configuration for deterministic depth-wise source scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SignedL1ShellScheduler {
    arity: usize,
    source_row_count: usize,
    limits: SignedL1ScheduleLimits,
}

impl SignedL1ShellScheduler {
    pub(crate) fn try_new(
        arity: usize,
        source_row_count: usize,
        limits: SignedL1ScheduleLimits,
    ) -> Result<Self, SpiredFoundationError> {
        if arity == 0 {
            return Err(SpiredFoundationError::EmptyIndexSpace);
        }
        if source_row_count == 0 {
            return Err(SpiredFoundationError::EmptySourceRows);
        }
        check_limit(ARITY, arity, limits.max_arity)?;
        check_limit(SOURCE_ROWS, source_row_count, limits.max_source_rows)?;
        let workspace_entries = checked_mul(ENUMERATION_WORKSPACE, arity, 3)?;
        check_limit(
            ENUMERATION_WORKSPACE,
            workspace_entries,
            limits.max_enumeration_workspace_entries,
        )?;
        Ok(Self {
            arity,
            source_row_count,
            limits,
        })
    }

    pub(crate) const fn arity(self) -> usize {
        self.arity
    }

    pub(crate) const fn source_row_count(self) -> usize {
        self.source_row_count
    }

    /// Open the complete shell `sum_i |s_i| = depth` as an O(arity) cursor.
    ///
    /// Exact offset and request counts are checked before cursor allocation.
    /// No offset or source-request Cartesian product is retained here.
    pub(crate) fn try_depth_shell(
        self,
        depth: usize,
    ) -> Result<SignedL1DepthShell, SpiredFoundationError> {
        let signed_depth = i64::try_from(depth)
            .map_err(|_| SpiredFoundationError::DepthNotRepresentable { depth })?;
        check_limit(DEPTH, depth, self.limits.max_depth)?;

        let offset_count = try_signed_l1_offset_count(self.arity, depth)?;
        check_limit(OFFSETS, offset_count, self.limits.max_offsets_per_shell)?;
        let request_count = checked_mul(REQUESTS, offset_count, self.source_row_count)?;
        check_limit(REQUESTS, request_count, self.limits.max_requests_per_shell)?;

        Ok(SignedL1DepthShell {
            depth,
            offset_count,
            request_count,
            source_row_count: self.source_row_count,
            emitted_offset_count: 0,
            emitted_request_count: 0,
            current_offset: None,
            next_source_ordinal: 0,
            cursor: SignedL1OffsetCursor::try_new(self.arity, depth, signed_depth)?,
            limits: self.limits,
            poisoned: false,
        })
    }
}

/// One bounded consecutive window of a depth shell.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SignedL1RequestChunk {
    first_request_ordinal: usize,
    requests: Vec<TranslatedSourceRequest>,
}

impl SignedL1RequestChunk {
    pub(crate) const fn first_request_ordinal(&self) -> usize {
        self.first_request_ordinal
    }

    pub(crate) fn requests(&self) -> &[TranslatedSourceRequest] {
        self.requests.as_slice()
    }

    pub(crate) fn len(&self) -> usize {
        self.requests.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
}

/// One complete exact-depth source schedule backed by an O(arity) cursor.
///
/// Requests are emitted first by lexicographic signed offset and then by
/// source chronology. Chunk boundaries do not alter that semantic order and
/// may split one offset's source block without rebuilding the shift: all
/// requests for that offset clone the same Arc-backed [`IntegralShift`].
#[derive(Debug)]
pub(crate) struct SignedL1DepthShell {
    depth: usize,
    offset_count: usize,
    request_count: usize,
    source_row_count: usize,
    emitted_offset_count: usize,
    emitted_request_count: usize,
    current_offset: Option<IntegralShift>,
    next_source_ordinal: usize,
    cursor: SignedL1OffsetCursor,
    limits: SignedL1ScheduleLimits,
    poisoned: bool,
}

impl SignedL1DepthShell {
    pub(crate) const fn depth(&self) -> usize {
        self.depth
    }

    pub(crate) const fn offset_count(&self) -> usize {
        self.offset_count
    }

    pub(crate) const fn request_count(&self) -> usize {
        self.request_count
    }

    pub(crate) const fn source_row_count(&self) -> usize {
        self.source_row_count
    }

    pub(crate) const fn emitted_request_count(&self) -> usize {
        self.emitted_request_count
    }

    pub(crate) const fn remaining_request_count(&self) -> usize {
        self.request_count - self.emitted_request_count
    }

    pub(crate) const fn is_exhausted(&self) -> bool {
        self.emitted_request_count == self.request_count
    }

    /// Materialize at most `max_requests` consecutive requests.
    ///
    /// The returned vector is the only request-sized allocation. A zero bound
    /// is rejected explicitly so callers cannot confuse lack of progress with
    /// shell exhaustion.
    pub(crate) fn try_next_chunk(
        &mut self,
        max_requests: usize,
    ) -> Result<Option<SignedL1RequestChunk>, SpiredFoundationError> {
        if self.poisoned {
            return Err(SpiredFoundationError::Invariant {
                detail: "a signed-L1 shell cannot resume after failed cursor advancement",
            });
        }
        if max_requests == 0 {
            return Err(SpiredFoundationError::EmptyRequestChunk);
        }
        if self.is_exhausted() {
            self.verify_exhausted()?;
            return Ok(None);
        }

        let chunk_capacity = max_requests.min(self.remaining_request_count());
        check_limit(
            CHUNK_REQUESTS,
            chunk_capacity,
            self.limits.max_requests_per_chunk,
        )?;
        let coordinate_cells = checked_mul(CHUNK_OFFSET_COORDINATES, chunk_capacity, self.arity())?;
        check_limit(
            CHUNK_OFFSET_COORDINATES,
            coordinate_cells,
            self.limits.max_offset_coordinate_cells_per_chunk,
        )?;
        let mut requests = Vec::new();
        try_reserve_exact(&mut requests, chunk_capacity, CHUNK_REQUESTS)?;
        let first_request_ordinal = self.emitted_request_count;
        if let Err(error) = self.fill_chunk(&mut requests, chunk_capacity) {
            self.poisoned = true;
            return Err(error);
        }
        if requests.is_empty() {
            self.poisoned = true;
            return Err(SpiredFoundationError::Invariant {
                detail: "a nonempty signed-L1 remainder produced an empty request chunk",
            });
        }
        Ok(Some(SignedL1RequestChunk {
            first_request_ordinal,
            requests,
        }))
    }

    const fn arity(&self) -> usize {
        self.cursor.coordinates.len()
    }

    fn fill_chunk(
        &mut self,
        requests: &mut Vec<TranslatedSourceRequest>,
        chunk_capacity: usize,
    ) -> Result<(), SpiredFoundationError> {
        while requests.len() < chunk_capacity {
            if self.current_offset.is_none() {
                let Some(offset) = self.cursor.try_next_offset()? else {
                    return Err(SpiredFoundationError::Invariant {
                        detail: "signed-L1 cursor ended before its exact request count",
                    });
                };
                self.emitted_offset_count = checked_add(OFFSETS, self.emitted_offset_count, 1)?;
                if self.emitted_offset_count > self.offset_count {
                    return Err(SpiredFoundationError::Invariant {
                        detail: "signed-L1 cursor exceeded its exact offset count",
                    });
                }
                self.current_offset = Some(offset);
                self.next_source_ordinal = 0;
            }
            let offset = self
                .current_offset
                .as_ref()
                .ok_or(SpiredFoundationError::Invariant {
                    detail: "signed-L1 cursor lost its current offset",
                })?;
            requests.push(TranslatedSourceRequest::new(
                self.next_source_ordinal,
                offset.clone(),
            ));
            self.next_source_ordinal = checked_add(SOURCE_ROWS, self.next_source_ordinal, 1)?;
            self.emitted_request_count = checked_add(REQUESTS, self.emitted_request_count, 1)?;
            if self.next_source_ordinal == self.source_row_count {
                self.current_offset = None;
                self.next_source_ordinal = 0;
            }
        }
        if self.emitted_request_count > self.request_count {
            return Err(SpiredFoundationError::Invariant {
                detail: "signed-L1 cursor exceeded its exact request count",
            });
        }
        if self.is_exhausted() {
            self.verify_exhausted()?;
        }
        Ok(())
    }

    fn verify_exhausted(&mut self) -> Result<(), SpiredFoundationError> {
        if self.emitted_offset_count != self.offset_count
            || self.current_offset.is_some()
            || self.next_source_ordinal != 0
        {
            return Err(SpiredFoundationError::Invariant {
                detail: "signed-L1 exhausted state disagreed with its exact preflight",
            });
        }
        // The cursor yields an offset before advancing beyond it. Consume only
        // its O(arity) traversal state here so the exact preflight is checked
        // against the enumerator without retaining a shell-sized census.
        if !self.cursor.is_exhausted() && self.cursor.try_next_offset()?.is_some() {
            return Err(SpiredFoundationError::Invariant {
                detail: "signed-L1 cursor exceeded its exact offset count",
            });
        }
        if !self.cursor.is_exhausted() {
            return Err(SpiredFoundationError::Invariant {
                detail: "signed-L1 cursor did not reach a terminal state",
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
struct SignedL1OffsetCursor {
    coordinates: Vec<i64>,
    remaining: Vec<usize>,
    next: Vec<Option<i64>>,
    position: usize,
    positive_leaf_pending: bool,
    exhausted: bool,
}

impl SignedL1OffsetCursor {
    fn try_new(
        arity: usize,
        depth: usize,
        signed_depth: i64,
    ) -> Result<Self, SpiredFoundationError> {
        let coordinates = try_zeroed_vec(ENUMERATION_WORKSPACE, arity)?;
        let mut remaining = try_zeroed_vec(ENUMERATION_WORKSPACE, arity)?;
        let mut next = try_none_vec(ENUMERATION_WORKSPACE, arity)?;
        remaining[0] = depth;
        next[0] = Some(-signed_depth);
        Ok(Self {
            coordinates,
            remaining,
            next,
            position: 0,
            positive_leaf_pending: false,
            exhausted: false,
        })
    }

    const fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    fn try_next_offset(&mut self) -> Result<Option<IntegralShift>, SpiredFoundationError> {
        while !self.exhausted {
            if self.position == self.coordinates.len() - 1 {
                let magnitude = i64::try_from(self.remaining[self.position]).map_err(|_| {
                    SpiredFoundationError::Invariant {
                        detail: "a preflighted shell remainder became unrepresentable",
                    }
                })?;
                if magnitude != 0 && !self.positive_leaf_pending {
                    self.coordinates[self.position] = -magnitude;
                    self.positive_leaf_pending = true;
                    return self.try_retain_current_offset().map(Some);
                }
                self.coordinates[self.position] = magnitude;
                self.positive_leaf_pending = false;
                self.backtrack_after_leaf();
                return self.try_retain_current_offset().map(Some);
            }

            let Some(coordinate) = self.next[self.position] else {
                self.backtrack();
                continue;
            };
            let maximum = i64::try_from(self.remaining[self.position]).map_err(|_| {
                SpiredFoundationError::Invariant {
                    detail: "a preflighted shell remainder became unrepresentable",
                }
            })?;
            self.next[self.position] = if coordinate == maximum {
                None
            } else {
                Some(
                    coordinate
                        .checked_add(1)
                        .ok_or(SpiredFoundationError::Invariant {
                            detail: "a signed-L1 enumeration cursor overflowed",
                        })?,
                )
            };
            self.coordinates[self.position] = coordinate;
            let consumed = usize::try_from(coordinate.unsigned_abs()).map_err(|_| {
                SpiredFoundationError::Invariant {
                    detail: "a signed-L1 coordinate magnitude did not fit usize",
                }
            })?;
            self.remaining[self.position + 1] = self.remaining[self.position] - consumed;
            self.position += 1;
            self.positive_leaf_pending = false;
            if self.position < self.coordinates.len() - 1 {
                let remainder = i64::try_from(self.remaining[self.position]).map_err(|_| {
                    SpiredFoundationError::Invariant {
                        detail: "a preflighted shell remainder became unrepresentable",
                    }
                })?;
                self.next[self.position] = Some(-remainder);
            }
        }
        Ok(None)
    }

    fn try_retain_current_offset(&self) -> Result<IntegralShift, SpiredFoundationError> {
        IntegralShift::try_new_with_component_limit(
            self.coordinates.iter().copied(),
            self.coordinates.len(),
        )
        .map_err(Into::into)
    }

    fn backtrack_after_leaf(&mut self) {
        if self.position == 0 {
            self.exhausted = true;
        } else {
            self.position -= 1;
        }
    }

    fn backtrack(&mut self) {
        if self.position == 0 {
            self.exhausted = true;
        } else {
            self.position -= 1;
        }
    }
}

fn try_signed_l1_offset_count(arity: usize, depth: usize) -> Result<usize, SpiredFoundationError> {
    if depth == 0 {
        return Ok(1);
    }
    let mut count = 0usize;
    let mut sign_choices = 1usize;
    for nonzero_coordinates in 1..=arity.min(depth) {
        sign_choices = checked_mul(OFFSETS, sign_choices, 2)?;
        let supports = checked_binomial(arity, nonzero_coordinates)?;
        let compositions = checked_binomial(depth - 1, nonzero_coordinates - 1)?;
        let unsigned = checked_mul(OFFSETS, supports, compositions)?;
        let signed = checked_mul(OFFSETS, unsigned, sign_choices)?;
        count = checked_add(OFFSETS, count, signed)?;
    }
    Ok(count)
}

fn checked_binomial(n: usize, k: usize) -> Result<usize, SpiredFoundationError> {
    if k > n {
        return Ok(0);
    }
    let k = k.min(n - k);
    let mut result = 1usize;
    for step in 1..=k {
        let mut numerator = n - k + step;
        let mut denominator = step;
        let common = gcd(numerator, denominator);
        numerator /= common;
        denominator /= common;
        let common = gcd(result, denominator);
        result /= common;
        denominator /= common;
        if denominator != 1 {
            return Err(SpiredFoundationError::Invariant {
                detail: "exact signed-L1 binomial cancellation left a denominator",
            });
        }
        result = checked_mul(OFFSETS, result, numerator)?;
    }
    Ok(result)
}

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredFoundationError> {
    left.checked_add(right)
        .ok_or(SpiredFoundationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredFoundationError> {
    left.checked_mul(right)
        .ok_or(SpiredFoundationError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredFoundationError> {
    if requested > limit {
        Err(SpiredFoundationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_reserve_exact<T>(
    retained: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), SpiredFoundationError> {
    let requested = checked_add(resource, retained.len(), additional)?;
    retained
        .try_reserve_exact(additional)
        .map_err(|_| SpiredFoundationError::AllocationFailure {
            resource,
            requested,
        })
}

fn try_zeroed_vec<T>(resource: &'static str, len: usize) -> Result<Vec<T>, SpiredFoundationError>
where
    T: Default + Clone,
{
    let mut retained = Vec::new();
    try_reserve_exact(&mut retained, len, resource)?;
    retained.resize(len, T::default());
    Ok(retained)
}

fn try_none_vec<T>(
    resource: &'static str,
    len: usize,
) -> Result<Vec<Option<T>>, SpiredFoundationError> {
    let mut retained = Vec::new();
    try_reserve_exact(&mut retained, len, resource)?;
    retained.resize_with(len, || None);
    Ok(retained)
}
