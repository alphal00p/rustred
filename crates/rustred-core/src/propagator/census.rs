//! The p census: sector fates and owner classes (types: M2-I0 rr-iface;
//! p_census and completeness: M2-T2 rr-census).
use std::collections::BTreeMap;
use std::sync::Arc;

use super::class::{ClassLimits, OwnerClassKey};
use super::error::PError;
use super::presentation::{Presentation, determinant};
use super::topology::Dag;
use super::{NodeIndex, OwnerClassId, PresentationId};

/// The substitution `k_in[i] = Sum_j A[i][j] k_out[j] + b[i] q_out`,
/// `q_in = s q_out`, from a source to a target momentum basis.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SignedAffineLoopMap {
    pub loops_in: u8,
    pub loops_out: u8,
    /// `A`, row major, `loops_in * loops_out`.
    pub loop_linear: Box<[i64]>,
    /// `b`, `loops_in` entries.
    pub loop_external: Box<[i64]>,
    /// `s`, `+1` or `-1`.
    pub external_sign: i8,
}

impl SignedAffineLoopMap {
    /// The target row of a source momentum over `(k_in.., q)`.
    pub fn map_momentum(&self, momentum: &[i64]) -> Result<Vec<i64>, PError> {
        let (rows, columns) = (usize::from(self.loops_in), usize::from(self.loops_out));
        if momentum.len() != rows + 1
            || self.loop_linear.len() != rows * columns
            || self.loop_external.len() != rows
        {
            return Err(PError::Contract(format!(
                "a {rows}->{columns} loop map cannot act on a momentum of {} entries",
                momentum.len()
            )));
        }
        let overflow = || PError::Contract("momentum coefficient overflow".to_owned());
        let mut out = vec![0_i64; columns + 1];
        for (row, &value) in momentum[..rows].iter().enumerate() {
            for (column, target) in out[..columns].iter_mut().enumerate() {
                let term = value
                    .checked_mul(self.loop_linear[row * columns + column])
                    .ok_or_else(overflow)?;
                *target = target.checked_add(term).ok_or_else(overflow)?;
            }
            let term = value
                .checked_mul(self.loop_external[row])
                .ok_or_else(overflow)?;
            out[columns] = out[columns].checked_add(term).ok_or_else(overflow)?;
        }
        let term = momentum[rows]
            .checked_mul(i64::from(self.external_sign))
            .ok_or_else(overflow)?;
        out[columns] = out[columns].checked_add(term).ok_or_else(overflow)?;
        Ok(out)
    }

    /// `A` square with determinant `+-1` and `s = +-1`.
    pub fn is_unimodular(&self) -> bool {
        let size = usize::from(self.loops_in);
        if self.loops_in != self.loops_out
            || self.external_sign.abs() != 1
            || self.loop_linear.len() != size * size
            || self.loop_external.len() != size
        {
            return false;
        }
        let det = determinant(size, &self.loop_linear);
        det == 1 || det == -1
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PSectorFate {
    Zero,
    Owner {
        class: OwnerClassId,
        map: SignedAffineLoopMap,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PClassRecord {
    pub key: OwnerClassKey,
    /// Owner presentation and support.
    pub owner: (PresentationId, u64),
    /// The min-GraphKey DAG node.
    pub node: Option<NodeIndex>,
    pub nodes: Vec<NodeIndex>,
    pub t: u8,
    pub eps_lines: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PCensus {
    /// `(presentation, support)` to its fate; `support = raw | forced`.
    pub sectors: BTreeMap<(PresentationId, u64), PSectorFate>,
    pub classes: BTreeMap<OwnerClassId, PClassRecord>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PCensusOptions {
    pub limits: ClassLimits,
}

/// The census of `presentations` (`dag`: owners at min-GraphKey nodes).
pub fn p_census(
    _presentations: &[Arc<Presentation>],
    _dag: Option<&Dag>,
    _options: &PCensusOptions,
) -> Result<PCensus, PError> {
    Err(PError::NotImplemented("M2-T2"))
}

impl PCensus {
    /// Register a class under its id; distinct bytes under one id fail closed.
    pub fn insert_class(&mut self, record: PClassRecord) -> Result<OwnerClassId, PError> {
        self.insert_class_as(record.key.id(), record)
    }

    /// [`Self::insert_class`] under an explicit id: the entry point of the
    /// forced-collision test; every other caller uses `insert_class`.
    #[doc(hidden)]
    pub fn insert_class_as(
        &mut self,
        id: OwnerClassId,
        record: PClassRecord,
    ) -> Result<OwnerClassId, PError> {
        if let Some(present) = self.classes.get(&id)
            && present.key.canonical != record.key.canonical
        {
            return Err(PError::MalformedLibrary(format!(
                "owner class id {:016x} names two canonical keys ({} and {})",
                id.0,
                present.key.label(),
                record.key.label()
            )));
        }
        self.classes.insert(id, record);
        Ok(id)
    }

    /// Every class is stored under the id of its own canonical bytes.
    pub fn check_class_ids(&self) -> Result<(), PError> {
        for (id, record) in &self.classes {
            if record.key.id() != *id {
                return Err(PError::MalformedLibrary(format!(
                    "owner class id {:016x} holds the canonical key {}",
                    id.0,
                    record.key.label()
                )));
            }
        }
        Ok(())
    }

    /// Strong completeness over `presentations` and `dag`.
    pub fn check_complete(
        &self,
        _presentations: &[Arc<Presentation>],
        _dag: Option<&Dag>,
    ) -> Result<(), PError> {
        Err(PError::NotImplemented("M2-T2"))
    }
}
