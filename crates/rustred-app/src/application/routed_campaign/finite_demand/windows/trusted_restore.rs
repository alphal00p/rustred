//! Bounded current-state validation of an explicitly trusted-generated cursor.
//! Prior completed sibling history is trusted through the captured generation,
//! not independently proved by its hash/counters. Deep replay remains separate.
use super::*;
use crate::application::routed_campaign::finite_demand::storage::{
    ScopeRow, ValidatedCursorRecord,
};

/// Cheap metadata about CURRENT's trusted-generated committed scope. This is
/// not a source-count proof, restored cursor or publication capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommittedCursorStatus {
    pub exhausted: bool,
    pub completed_windows: u64,
    pub input_occurrences: u64,
}

pub(crate) fn inspect_committed_cursor(
    token: &ValidatedCursorRecord<'_>,
    physical_arity: usize,
) -> Result<CommittedCursorStatus> {
    let record: CursorRecord = decode(token.encoded_cursor())?;
    if physical_arity == 0
        || record.version != VERSION
        || record.binding != token.source_policy()
        || record.windows != token.completed_windows()
        || record.emitted != Integer::from(token.input_occurrences())
        || record.frames.len() != record.prefix.len()
        || record.prefix.len() > physical_arity
        || (record.finished && !record.prefix.is_empty())
    {
        return Err(invalid("committed cursor status binding/count/shape"));
    }
    for (coordinate, frame) in record.prefix.iter().zip(&record.frames) {
        if frame.lower > frame.current
            || frame.current > frame.upper
            || *coordinate != frame.current
            || frame.parent_count <= 0
            || frame.completed_children < 0
            || frame.completed_children > frame.parent_count
        {
            return Err(invalid("committed cursor status frame shape"));
        }
    }
    Ok(CommittedCursorStatus {
        exhausted: record.finished,
        completed_windows: token.completed_windows(),
        input_occurrences: token.input_occurrences(),
    })
}

impl<const N: usize> PrefixWindows<N> {
    /// Read-only portable completed-cursor check. This returns no cursor,
    /// resume token or reduction capability; it only cross-checks committed
    /// bytes against this exact native counted source/window policy.
    pub(crate) fn validate_completed_commitment(
        &self,
        encoded: &[u8],
        windows: u64,
        occurrences: u64,
        cancel: &AtomicBool,
    ) -> Result<()> {
        let record: CursorRecord = decode(encoded)?;
        if record.windows != windows || record.emitted != Integer::from(occurrences) {
            return Err(invalid("completed portable cursor counter mismatch"));
        }
        let (cursor, _) = self.validate_generated_cursor(record, cancel)?;
        if !cursor.is_exhausted() {
            return Err(invalid("portable source cursor is not exhausted"));
        }
        Ok(())
    }
    /// Binding for a generated cursor record, not an authority constructor.
    pub(crate) fn source_policy_binding(&self) -> [u8; 32] {
        self.binding
    }

    /// Restore only a cursor captured from an integrity-validated,
    /// TRUSTED-GENERATED immutable generation. The backend must authenticate
    /// that same generation's reducer/scope/Work/census separately. This method
    /// does not create reduction closure, a master, or a core baseline.
    ///
    /// Expected identity arguments only reject mismatches: arbitrary bytes plus
    /// those hashes/counts cannot enter this API without the opaque token.
    /// Native current-node checks use at most physical depth + 1 calls and
    /// bounded O(arity * depth) metadata work. No earlier siblings/windows are
    /// replayed. Optional `audit_restore_cursor` supplies the stronger audit.
    pub(crate) fn restore_trusted_generation_cursor(
        &self,
        token: &ValidatedCursorRecord<'_>,
        expected_generation: [u8; 32],
        expected_row: &ScopeRow,
        cancel: &AtomicBool,
    ) -> Result<PrefixCursor> {
        check(cancel)?;
        if token.generation() != expected_generation
            || token.row() != expected_row
            || token.source_policy() != self.binding
        {
            return Err(invalid(
                "trusted cursor generation/scope/source-policy binding",
            ));
        }
        let record: CursorRecord = decode(token.encoded_cursor())?;
        if record.windows != token.completed_windows()
            || record.emitted != Integer::from(token.input_occurrences())
        {
            return Err(invalid("trusted cursor generation counters differ"));
        }
        self.validate_generated_cursor(record, cancel)
            .map(|(cursor, _)| cursor)
    }

    // Private validation engine also returns its native node-call count for
    // bounded-complexity tests. Only the token-gated entry point exposes a
    // restored cursor outside this module.
    fn validate_generated_cursor(
        &self,
        record: CursorRecord,
        cancel: &AtomicBool,
    ) -> Result<(PrefixCursor, usize)> {
        check(cancel)?;
        let cursor = PrefixCursor(record);
        self.validate_live(&cursor)?;
        if cursor == self.initial_cursor() {
            return Ok((cursor, 0));
        }
        let record = &cursor.0;
        if self.total == 0 || record.windows == 0 || record.emitted == 0 {
            return Err(invalid(
                "noninitial cursor has no completed nonempty window",
            ));
        }
        let capacity = self.seed_capacity()?;
        let windows = Integer::from(record.windows);
        if windows > record.emitted || record.emitted > &windows * Integer::from(capacity) {
            return Err(invalid("completed window count/seed-capacity consistency"));
        }
        if self.singleton {
            if !record.finished || record.windows != 1 || record.emitted != 1 {
                return Err(invalid("fixed singleton completion shape/count"));
            }
            // Do not accept an unrepresentable fixed power as a finished key.
            self.scatter(&self.region.lower[..self.arity], &[])?;
            check(cancel)?;
            return Ok((cursor, 0));
        }
        if record.finished {
            // validate_live already requires empty stack/prefix and exact total.
            check(cancel)?;
            return Ok((cursor, 0));
        }
        if record.frames.is_empty() {
            return Err(invalid("noninitial unfinished cursor lacks its DFS frames"));
        }

        let mut node_calls = 0usize;
        let mut count_node = |prefix: &[u64]| {
            node_calls = node_calls
                .checked_add(1)
                .ok_or_else(|| Error::new(ErrorKind::Resource, "restore node-call overflow"))?;
            self.node(prefix, cancel)
        };
        let mut parent = count_node(&[])?;
        let mut emitted = Integer::from(0);
        for (axis, frame) in record.frames.iter().enumerate() {
            check(cancel)?;
            if parent.count <= Integer::from(capacity)
                || frame.parent_count != parent.count
                || frame.completed_children < 0
            {
                return Err(invalid("split parent count or completed sibling subtotal"));
            }
            let extrema = parent
                .summary
                .extrema()
                .ok_or_else(|| invalid("empty split parent"))?;
            if frame.lower != extrema.lower()[axis]
                || Some(frame.upper) != extrema.upper()[axis]
                || frame.current < frame.lower
                || frame.current > frame.upper
                || record.prefix[axis] != frame.current
                || (frame.current == frame.lower && frame.completed_children != 0)
                || (frame.current > frame.lower && frame.completed_children <= 0)
            {
                return Err(invalid("DFS frame native extrema/current/subtotal"));
            }
            let child = count_node(&record.prefix[..=axis])?;
            if child.count <= 0 {
                return Err(invalid(
                    "normalized cursor points to an empty current child",
                ));
            }
            let accounted = &frame.completed_children + &child.count;
            if accounted > parent.count
                || (frame.current == frame.upper && accounted != parent.count)
            {
                return Err(invalid(
                    "completed/current children exceed native parent count",
                ));
            }
            emitted += &frame.completed_children;
            parent = child;
        }
        if emitted != record.emitted || parent.count > Integer::from(capacity) {
            return Err(invalid(
                "normalized cursor emitted telescope/current window capacity",
            ));
        }
        check(cancel)?;
        Ok((cursor, node_calls))
    }
}

fn invalid(detail: &'static str) -> Error {
    Error::new(ErrorKind::Cursor, detail)
}

#[cfg(test)]
mod tests;
