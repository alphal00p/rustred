//! The top families exported from vakint (owner: M2-I0 rr-iface): the schema
//! `rustred.p-families.v1`, written by `scripts/p_families_from_vakint.py`,
//! and their presentations, in exactly vakint's slot order (lines, then ISPs).
//! The content is versioned by [`P_FAMILIES_VERSION`].
//!
//! [`P_FAMILIES_VERSION`]: super::P_FAMILIES_VERSION
use std::collections::BTreeSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::algebra::CoefficientContext;

use super::error::PError;
use super::presentation::{
    P_EXTERNAL_SQUARE, Presentation, PresentationKind, Slot, SlotIsp, SlotLine, checked_row,
};
use super::{P_FAMILIES_SCHEMA, PresentationId};

/// The file `data/p/p-families.v1.json`.
pub const P_FAMILIES_V1_JSON: &str = include_str!("../../data/p/p-families.v1.json");

/// The file `data/p/p-families-5L.v1.json` (p5l-converter): the 64 five-loop
/// tops of the forcer-tricks census (`tools/p_families_5l.py`; Forcer's
/// `createTree` of these gives the 283 custom nodes of the 5L campaign). Not
/// part of [`P_FAMILIES_V1_JSON`] nor of [`P_FAMILIES_VERSION`]: no shipped
/// package reads it. `present` finds a top's graph here when the v1 file does
/// not hold it, so a families file with these tops (`rustred p-generate
/// --families`, `--max-loops 5`) presents the 5L DAG.
///
/// [`P_FAMILIES_VERSION`]: super::P_FAMILIES_VERSION
pub const P_FAMILIES_5L_V1_JSON: &str = include_str!("../../data/p/p-families-5L.v1.json");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PFamiliesFile {
    pub schema: String,
    pub provenance: String,
    pub families: Vec<PFamilyEntry>,
}

/// One top family; vertices are `1..=vertices`, every momentum row is over
/// `(k_1, .., k_L, q)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PFamilyEntry {
    pub name: String,
    pub loops: u8,
    pub vertices: u8,
    pub lines: Vec<PFamilyLine>,
    /// Momenta whose squares are the ISP slots.
    pub isps: Vec<Vec<i64>>,
    pub external: PFamilyExternal,
    pub gram: Vec<Vec<i64>>,
}

/// A line whose momentum flows from `endpoints[0]` to `endpoints[1]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PFamilyLine {
    pub endpoints: [u8; 2],
    pub momentum: Vec<i64>,
}

/// `q` enters at `incoming` and leaves at `outgoing`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PFamilyExternal {
    #[serde(rename = "in")]
    pub incoming: u8,
    #[serde(rename = "out")]
    pub outgoing: u8,
}

impl PFamilyEntry {
    /// Momentum conservation at every vertex, the kinematics and the slot counts.
    fn check(&self) -> Result<(), PError> {
        let fail =
            |detail: String| PError::MalformedLibrary(format!("family {}: {detail}", self.name));
        let loops = usize::from(self.loops);
        if loops == 0 || self.gram != [vec![P_EXTERNAL_SQUARE]] {
            return Err(fail(format!(
                "{loops} loops and gram {:?}; loops >= 1 and [[{P_EXTERNAL_SQUARE}]] are required",
                self.gram
            )));
        }
        let vertex = |v: u8| (1..=self.vertices).contains(&v);
        let PFamilyExternal { incoming, outgoing } = self.external;
        if !vertex(incoming) || !vertex(outgoing) || incoming == outgoing {
            return Err(fail(format!("external vertices {incoming}, {outgoing}")));
        }
        let width = loops + 1;
        let mut balance = vec![vec![0_i64; width]; usize::from(self.vertices) + 1];
        for line in &self.lines {
            let [tail, head] = line.endpoints;
            if !vertex(tail) || !vertex(head) {
                return Err(fail(format!("line {line:?}")));
            }
            checked_row(&line.momentum, width)?;
            for (column, &value) in line.momentum.iter().enumerate() {
                balance[usize::from(tail)][column] -= value;
                balance[usize::from(head)][column] += value;
            }
        }
        balance[usize::from(incoming)][loops] += 1;
        balance[usize::from(outgoing)][loops] -= 1;
        if let Some(v) = balance.iter().position(|row| row.iter().any(|&c| c != 0)) {
            return Err(fail(format!("momentum is not conserved at vertex {v}")));
        }
        Ok(())
    }

    /// The top presentation of this family, `id` in `context`.
    pub fn presentation(
        &self,
        id: PresentationId,
        context: &CoefficientContext,
    ) -> Result<Presentation, PError> {
        self.check()?;
        let lines = self.lines.iter().map(|line| {
            Slot::Line(SlotLine {
                momentum: line.momentum.clone(),
                eps_units: 0,
            })
        });
        let isps = self.isps.iter().map(|momentum| {
            Slot::Isp(SlotIsp {
                left: momentum.clone(),
                right: momentum.clone(),
            })
        });
        Presentation::from_slots(
            id,
            PresentationKind::Top {
                name: self.name.clone(),
            },
            &self.name,
            self.loops,
            lines.chain(isps).collect(),
            context,
            None,
        )
    }
}

/// Parse the families file and build one top presentation per family: ids
/// are the file positions, all families share one context `{d}`.
pub fn load_p_families(json: &str) -> Result<(PFamiliesFile, Vec<Arc<Presentation>>), PError> {
    let file: PFamiliesFile = serde_json::from_str(json)
        .map_err(|error| PError::MalformedLibrary(format!("p families: {error}")))?;
    if file.schema != P_FAMILIES_SCHEMA {
        return Err(PError::VersionMismatch {
            name: "P_FAMILIES_SCHEMA".to_owned(),
            found: file.schema,
            expected: P_FAMILIES_SCHEMA.to_owned(),
        });
    }
    let mut names = BTreeSet::new();
    if let Some(entry) = file.families.iter().find(|e| !names.insert(&e.name)) {
        return Err(PError::MalformedLibrary(format!(
            "two p families named {}",
            entry.name
        )));
    }
    let context = CoefficientContext::try_new(["d"]).map_err(|_| PError::ContextMismatch)?;
    let presentations = file
        .families
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let id = u32::try_from(index)
                .map_err(|_| PError::MalformedLibrary("too many p families".to_owned()))?;
            entry
                .presentation(PresentationId(id), &context)
                .map(Arc::new)
        })
        .collect::<Result<_, _>>()?;
    Ok((file, presentations))
}
