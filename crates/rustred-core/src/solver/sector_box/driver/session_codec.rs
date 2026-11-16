//! Session codec: the materialisation state of a [`BoxLaportaSession`]
//! after `extend` (preferred set, demand, seeded regions, masters and the
//! modular selection), so that a long job can checkpoint after its modular
//! phase and a restarted process materialises (finite-field or exact lane,
//! certificates) without repeating the elimination.
//!
//! The bytes are bound to the family fingerprint, the router fingerprint and
//! the options that affect the row space and the column order; decoding
//! refuses any mismatch and any checksum error, and recomputes the selection
//! hashes from the decoded rows and masters. A decoded session is
//! materialise-only: `extend` on it is an error (the modular row space is not
//! part of the checkpoint).
//!
//! Layout: magic, format version, blake3 key, then little-endian sections,
//! then a blake3 checksum of everything before it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::family::IntegralFamily;
use crate::sector::CutConstraint;

use super::{BoxLaportaSession, integral, powers};
use crate::solver::SolverError;
use crate::solver::sector_box::router::SectorRouter;
use crate::solver::sector_box::seeds::{SeedBox, SeedRegion};
use crate::solver::sector_box::system::SelectionState;
use crate::solver::sector_box::{BoxOptions, ColumnOrder, SlotPriority};

const MAGIC: &[u8; 16] = b"RUSTRED-SBSESS\0\0";
const VERSION: u32 = 1;

fn corrupt(message: impl Into<String>) -> SolverError {
    SolverError::InvalidInput(format!("sector-box session bytes: {}", message.into()))
}

/// Options that change the row space, the column order or the selection.
fn options_digest(options: &BoxOptions) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"rustred-sector-box-session-options-v1\0");
    // Explicit bytes, not `Debug` renderings: the digest is a file key and
    // must not change with a formatting detail.
    match &options.order {
        ColumnOrder::Numeric => {
            hasher.update(&[0]);
        }
        ColumnOrder::LinesDotsRank => {
            hasher.update(&[1]);
        }
        ColumnOrder::LinesRankDots => {
            hasher.update(&[2]);
        }
        ColumnOrder::Compiled(order) => {
            let canonical = order.canonical_bytes();
            hasher.update(&[3]);
            hasher.update(&(canonical.len() as u64).to_le_bytes());
            hasher.update(canonical);
        }
    }
    match &options.slot_priority {
        SlotPriority::Natural => {
            hasher.update(&[0]);
        }
        SlotPriority::TopSectorFirst(top) => {
            hasher.update(&[1]);
            hasher.update(&top.to_le_bytes());
        }
        SlotPriority::Permutation(slots) => {
            hasher.update(&[2]);
            hasher.update(&(slots.len() as u64).to_le_bytes());
            hasher.update(slots);
        }
    }
    hasher.update(&options.sample.prime.to_le_bytes());
    hasher.update(&options.sample.seed.to_le_bytes());
    *hasher.finalize().as_bytes()
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }
    fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn i16(&mut self, value: i16) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.0.extend_from_slice(value);
    }
    fn powers<const N: usize>(&mut self, value: &[i16; N]) {
        for &power in value {
            self.i16(power);
        }
    }
    fn list<const N: usize>(&mut self, values: impl ExactSizeIterator<Item = [i16; N]>) {
        self.u64(values.len() as u64);
        for value in values {
            self.powers(&value);
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], SolverError> {
        let end = self
            .at
            .checked_add(n)
            .ok_or_else(|| corrupt("length overflow"))?;
        if end > self.bytes.len() {
            return Err(corrupt("truncated"));
        }
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, SolverError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, SolverError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, SolverError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, SolverError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, SolverError> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn len(&mut self, unit: usize) -> Result<usize, SolverError> {
        let n = self.u64()? as usize;
        if n.saturating_mul(unit.max(1)) > self.bytes.len() - self.at {
            return Err(corrupt("length beyond the end"));
        }
        Ok(n)
    }
    fn bytes(&mut self) -> Result<&'a [u8], SolverError> {
        let n = self.len(1)?;
        self.take(n)
    }
    fn string(&mut self) -> Result<String, SolverError> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| corrupt("utf-8"))
    }
    fn powers<const N: usize>(&mut self) -> Result<[i16; N], SolverError> {
        let mut out = [0i16; N];
        for value in &mut out {
            *value = self.i16()?;
        }
        Ok(out)
    }
    fn list<const N: usize>(&mut self) -> Result<Vec<[i16; N]>, SolverError> {
        let n = self.len(2 * N)?;
        (0..n).map(|_| self.powers()).collect()
    }
}

impl<const N: usize> BoxLaportaSession<N> {
    /// Binds a checkpoint to the family, the routing and the options.
    fn checkpoint_key(&self) -> [u8; 32] {
        checkpoint_key(&self.family, self.router.fingerprint(), &self.options)
    }

    /// Encode the materialisation state (after `extend`). Errors when no
    /// modular selection exists yet.
    pub fn to_native_bytes(&self) -> Result<Vec<u8>, SolverError> {
        let selection = self.selection.as_ref().ok_or_else(|| {
            SolverError::InvalidInput(
                "the session codec needs a modular selection; extend first".into(),
            )
        })?;
        let mut w = Writer(Vec::new());
        w.0.extend_from_slice(MAGIC);
        w.u32(VERSION);
        w.u32(N as u32);
        w.0.extend_from_slice(&self.checkpoint_key());
        w.list(self.preferred.iter().copied());
        w.list(self.demand.iter().copied());
        w.list(self.zero_demand.iter().copied());
        w.list(self.preferred_zero.iter().copied());
        w.list(self.masters.iter().copied());
        match self.truncate {
            Some(l) => {
                w.u8(1);
                w.i16(l);
            }
            None => w.u8(0),
        }
        w.u64(self.boxes.len() as u64);
        for (id, region) in &self.boxes {
            w.u64(*id);
            w.u64(region.boxes().len() as u64);
            for b in region.boxes() {
                w.i16(b.r);
                w.i16(b.s);
                w.i16(b.d);
            }
        }
        w.u64(selection.rows.len() as u64);
        for (seed, source) in &selection.rows {
            w.powers(seed);
            w.u16(*source);
        }
        w.list(selection.pivots.iter().map(powers));
        w.list(selection.masters.iter().map(powers));
        w.list(selection.demand.iter().map(powers));
        w.bytes(selection.selected_hash.as_bytes());
        w.bytes(selection.master_hash.as_bytes());
        let checksum = blake3::hash(&w.0);
        w.0.extend_from_slice(checksum.as_bytes());
        Ok(w.0)
    }

    /// Decode a checkpoint into a materialise-only session over the same
    /// family, router and options (`workers`, limits and diagnostics may
    /// differ). The selection hashes are recomputed from the decoded rows
    /// and masters and must match, and every decoded integral must be one of
    /// the family (zero on storage padding).
    pub fn from_native_bytes(
        bytes: &[u8],
        family: Arc<IntegralFamily>,
        router: Arc<dyn SectorRouter<N>>,
        options: BoxOptions,
    ) -> Result<Self, SolverError> {
        if bytes.len() < MAGIC.len() + 8 + 32 + 32 {
            return Err(corrupt("too short"));
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if blake3::hash(body).as_bytes() != checksum {
            return Err(corrupt("checksum mismatch"));
        }
        let mut r = Reader { bytes: body, at: 0 };
        if r.take(MAGIC.len())? != MAGIC {
            return Err(corrupt("bad magic"));
        }
        if r.u32()? != VERSION {
            return Err(corrupt("unsupported version"));
        }
        if r.u32()? as usize != N {
            return Err(corrupt("storage arity differs"));
        }
        let key: [u8; 32] = r.take(32)?.try_into().unwrap();
        if key != checkpoint_key(&family, router.fingerprint(), &options) {
            return Err(corrupt(
                "bound to another family, router or option set (order, slot priority, sample)",
            ));
        }
        let preferred = r.list::<N>()?;
        let demand = r.list::<N>()?;
        let zero_demand = r.list::<N>()?;
        let preferred_zero = r.list::<N>()?;
        let masters = r.list::<N>()?;
        let truncate = match r.u8()? {
            0 => None,
            1 => Some(r.i16()?),
            _ => return Err(corrupt("truncate tag")),
        };
        let mut boxes = BTreeMap::new();
        let regions = r.len(16)?;
        for _ in 0..regions {
            let id = r.u64()?;
            let count = r.len(6)?;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                list.push(SeedBox {
                    r: r.i16()?,
                    s: r.i16()?,
                    d: r.i16()?,
                });
            }
            boxes.insert(id, SeedRegion::from_boxes(list));
        }
        let rows_len = r.len(2 * N + 2)?;
        let mut rows = Vec::with_capacity(rows_len);
        for _ in 0..rows_len {
            let seed = r.powers::<N>()?;
            rows.push((seed, r.u16()?));
        }
        let pivots = r.list::<N>()?;
        let selection_masters = r.list::<N>()?;
        let selection_demand = r.list::<N>()?;
        let selected_hash = r.string()?;
        let master_hash = r.string()?;
        if r.at != body.len() {
            return Err(corrupt("trailing bytes"));
        }
        // The hashes exactly as `select` computes them.
        let mut hasher = blake3::Hasher::new();
        for (seed, source) in &rows {
            for value in seed {
                hasher.update(&value.to_le_bytes());
            }
            hasher.update(&source.to_le_bytes());
        }
        if hasher.finalize().to_hex().as_str() != selected_hash {
            return Err(corrupt("selected-row hash mismatch"));
        }
        let mut hasher = blake3::Hasher::new();
        for master in &selection_masters {
            for value in master {
                hasher.update(&value.to_le_bytes());
            }
        }
        if hasher.finalize().to_hex().as_str() != master_hash {
            return Err(corrupt("master hash mismatch"));
        }
        if masters != selection_masters {
            return Err(corrupt("the session masters are not the selection's"));
        }
        let to_integrals = |list: &[[i16; N]]| -> Result<Vec<_>, SolverError> {
            list.iter().map(|values| integral(*values)).collect()
        };
        let arity = family.denominator_count();
        let cuts = CutConstraint::none(arity).map_err(|error| corrupt(error.to_string()))?;
        let mut session = Self::new(family, &cuts, router, options)?;
        for list in [
            &preferred,
            &demand,
            &zero_demand,
            &preferred_zero,
            &masters,
            &pivots,
        ] {
            for values in list.iter() {
                session
                    .check_powers(values)
                    .map_err(|_| corrupt("an integral outside the family"))?;
            }
        }
        for (seed, _) in &rows {
            session
                .check_powers(seed)
                .map_err(|_| corrupt("a seed outside the family"))?;
        }
        if !preferred.is_empty() {
            session.reorder(preferred)?;
        }
        session.demand = demand.into_iter().collect::<BTreeSet<_>>();
        session.zero_demand = zero_demand.into_iter().collect();
        session.preferred_zero = preferred_zero.into_iter().collect();
        session.masters = masters;
        session.truncate = truncate;
        session.boxes = boxes;
        session.selection = Some(SelectionState {
            rows,
            pivots: to_integrals(&pivots)?,
            masters: to_integrals(&selection_masters)?,
            demand: to_integrals(&selection_demand)?,
            selected_hash,
            master_hash,
        });
        session.restored = true;
        Ok(session)
    }
}

fn checkpoint_key(family: &IntegralFamily, router: [u8; 32], options: &BoxOptions) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"rustred-sector-box-session-checkpoint-v1\0");
    hasher.update(family.fingerprint().as_bytes());
    hasher.update(&router);
    hasher.update(&options_digest(options));
    *hasher.finalize().as_bytes()
}
