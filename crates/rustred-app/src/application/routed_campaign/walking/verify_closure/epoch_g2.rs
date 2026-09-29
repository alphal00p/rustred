//! Independent Epoch-to-oracle record adapter. The raw CP6 anchors and ledger
//! are decoded independently; CP5 publication positions are never reused.
use super::super::queue::CompactDomain;
use super::{
    G2AnchorRow, G2RecordRow, PowersRow, RecordRow,
    epoch_export::{AnchorRow, EpochSections},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) struct View<'a> {
    sections: &'a EpochSections,
    anchors: BTreeMap<usize, &'a AnchorRow>,
}

impl<'a> View<'a> {
    pub fn new(sections: &'a EpochSections) -> Result<Self, String> {
        let mut anchors = BTreeMap::new();
        let mut previous = None;
        for row in &sections.anchors {
            if previous.is_some_and(|id| id >= row.0)
                || row.0 as usize >= sections.ledger.len()
                || anchors.insert(row.0 as usize, row).is_some()
            {
                return Err("Epoch anchor node range/order differs".into());
            }
            previous = Some(row.0);
        }
        Ok(Self { sections, anchors })
    }

    pub fn position(&self, id: usize) -> u64 {
        self.sections
            .ledger
            .get(id)
            .map_or(u64::MAX, |word| word & ((1 << 48) - 1))
    }

    pub fn normalize<const N: usize>(
        &self,
        row: &mut RecordRow,
        domains: &[CompactDomain<N>],
    ) -> Result<(), String> {
        if row.g2_residual_anchors.is_some() {
            return Err("CP5 G2 publication block in Epoch record".into());
        }
        let Some(image) = domains.get(row.id) else {
            return Ok(());
        };
        let record = self.anchors.get(&row.id).copied();
        if row.record_kind != "g2_residual_inspection" {
            if record.is_some_and(|a| a.1 != 0) || row.g2.is_some() {
                return Err(format!(
                    "Epoch record {} does not match residual anchors",
                    row.id
                ));
            }
            return Ok(());
        }
        let (_, kind, dispatch, refs, scope) =
            record.ok_or("Epoch G2 record has no raw anchor row")?;
        if !matches!(kind, 1 | 2) || scope.len() < 4 {
            return Err("Epoch G2 kind or scope differs".into());
        }
        let count = u32::from_le_bytes(scope[..4].try_into().expect("4")) as usize;
        // The indexed production planner emits a single D band or full cover.
        // Refuse other scope shapes explicitly rather than guessing a hull.
        if count > 1 || scope.len() != 4 + count * (18 + 4 * N) {
            return Err("Epoch G2 residual is not a supported D band".into());
        }
        let mut pieces = Vec::<Value>::new();
        let residual_power_bounds = if count == 0 {
            None
        } else {
            let optional = |at: usize| -> Result<Option<i64>, String> {
                let present = scope[at];
                let value = i64::from_le_bytes(scope[at + 1..at + 9].try_into().expect("8"));
                match present {
                    0 if value == 0 => Ok(None),
                    1 => Ok(Some(value)),
                    _ => Err("Epoch residual option is not canonical".into()),
                }
            };
            let lo = optional(4)?;
            let hi = optional(13)?;
            if lo.is_none() || hi.is_none() || lo > hi {
                return Err("Epoch residual D interval".into());
            }
            let lower: Vec<u16> = (0..N)
                .map(|a| u16::from_le_bytes(scope[22 + 2 * a..24 + 2 * a].try_into().expect("2")))
                .collect();
            let upper: Vec<u16> = (0..N)
                .map(|a| {
                    u16::from_le_bytes(
                        scope[22 + 2 * N + 2 * a..24 + 2 * N + 2 * a]
                            .try_into()
                            .expect("2"),
                    )
                })
                .collect();
            let (original_lower, original_upper) = image.raw_bounds();
            if lower != original_lower.as_slice() || upper != original_upper.as_slice() {
                return Err("Epoch residual changes the original coordinate scope".into());
            }
            pieces.push(json!({"d_lo":lo,"d_hi":hi,"lower":lower,"upper":upper}));
            let whole = image.expand().powers;
            Some(PowersRow {
                max_positive_power: whole.max_positive_power,
                min_power_difference: lo,
                max_power_difference: hi,
            })
        };
        let expected = json!({"kind":if *kind == 1 {"g2_native"} else {"g2_residual"},
            "dispatch_version":dispatch,"anchors":refs.iter().map(|&(id,lent,stamp)| json!({"id":id,"stamp":stamp,
                "lent":if lent == 0 {"domain"} else {"inspected_low_D_slice"}})).collect::<Vec<_>>(),
            "residual":pieces,"authority":"exact_union_cover_lattice"});
        if row.g2.as_ref() != Some(&expected) || row.initial_overlap.is_some() {
            return Err(format!(
                "Epoch G2 record {} differs from saved anchor authority",
                row.id
            ));
        }
        let own = self.position(row.id);
        let epoch = row
            .epoch
            .as_ref()
            .ok_or("Epoch G2 record epoch metadata missing")?;
        if epoch["merge_epoch"].as_u64() != Some(own)
            || epoch["v0"].as_u64() != Some(*dispatch)
            || *dispatch >= own
        {
            return Err("Epoch G2 record stamp differs from raw ledger".into());
        }
        let mut anchors = Vec::with_capacity(refs.len());
        for &(id, lent, stamp) in refs {
            let source = self.anchors.get(&(id as usize));
            let name = match source {
                None if lent == 0 => "native",
                Some(a) if a.1 == 0 && lent == 1 && *kind == 2 => "initial_d_band",
                Some(a) if matches!(a.1, 1 | 2) && lent == 0 && *kind == 2 => "g2_residual",
                _ => return Err("Epoch G2 lent scope/kind differs from its lender".into()),
            };
            anchors.push(G2AnchorRow {
                id: id as usize,
                stamp,
                kind: name.into(),
            });
        }
        row.record_kind = "g2_residual_anchor_inspection".into();
        row.g2_residual_anchors = Some(G2RecordRow {
            merge_stamp: Some(own),
            snapshot_stamp: dispatch.checked_add(1).ok_or("Epoch dispatch overflow")?,
            residual_power_bounds,
            anchors,
        });
        Ok(())
    }
}
