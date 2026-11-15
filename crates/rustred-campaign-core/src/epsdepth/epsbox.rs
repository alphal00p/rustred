//! The user's EPS BOX (addendum 8.2; interfaces/epsdepth.md 7): OPTIONAL data, never read by the solve or the closure.
//!
//! Per terminal (by key, or one default for all) the eps order up to which its expansion is available / wanted, and the
//! eps order to which the reduced integrals are needed (default 0 = finite part). The required depth of a terminal T at a
//! box key K is `needed_order + p_T(K)` (p_T = composed pole order at d0): `within` or `exceeds by k`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Schema word of the eps box object (written in reports).
pub const EPS_BOX_SCHEMA: &str = "rustred.campaign.eps-box.v1";

/// One per-terminal entry of the box.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpsBoxKey {
    pub key: Vec<i32>,
    pub order: i64,
}

/// `available`: default + per-key overrides.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpsAvailable {
    #[serde(default)]
    pub default: Option<i64>,
    #[serde(default)]
    pub keys: Vec<EpsBoxKey>,
}

/// The eps box as written in the spec.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpsBox {
    #[serde(default)]
    pub available: EpsAvailable,
    /// Eps order to which reduced integrals are needed (0 = finite part).
    #[serde(default)]
    pub needed_order: i64,
}

/// Where a terminal's required depth stands against the box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoxStatus {
    Within,
    /// Required depth exceeds the available order by this many orders (>= 1).
    Exceeds(i64),
}

impl BoxStatus {
    pub fn word(&self) -> String {
        match self {
            BoxStatus::Within => "within".into(),
            BoxStatus::Exceeds(k) => format!("exceeds:{k}"),
        }
    }
}

/// A validated eps box (lookups by key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpsBoxChecked {
    pub spec: EpsBox,
    by_key: BTreeMap<Vec<i32>, i64>,
}

impl EpsBox {
    /// Parse a JSON value (unknown fields refused) and validate it.
    pub fn from_value(v: &serde_json::Value) -> Result<EpsBoxChecked, String> {
        let b: EpsBox = serde_json::from_value(v.clone()).map_err(|e| format!("eps_box: {e}"))?;
        b.validate()
    }

    /// Validation: keys unique and of one arity (when `arity` is known it must match).
    pub fn validate(self) -> Result<EpsBoxChecked, String> {
        let mut by_key = BTreeMap::new();
        let mut arity = None;
        for k in &self.available.keys {
            if k.key.is_empty() {
                return Err("eps_box: an empty key".into());
            }
            match arity {
                None => arity = Some(k.key.len()),
                Some(a) if a != k.key.len() => {
                    return Err(format!("eps_box: key {:?} of arity {}, expected {a}", k.key, k.key.len()));
                }
                _ => {}
            }
            if by_key.insert(k.key.clone(), k.order).is_some() {
                return Err(format!("eps_box: key {:?} listed twice", k.key));
            }
        }
        Ok(EpsBoxChecked { spec: self, by_key })
    }
}

impl EpsBoxChecked {
    pub fn needed_order(&self) -> i64 {
        self.spec.needed_order
    }

    /// The available order of a terminal: its own entry, else the default; `Err` when neither exists (typed: never a
    /// silent default).
    pub fn available(&self, terminal: &[i32]) -> Result<i64, String> {
        if let Some(o) = self.by_key.get(terminal) {
            return Ok(*o);
        }
        self.spec
            .available
            .default
            .ok_or_else(|| format!("eps_box: terminal {terminal:?} has no available order and no default"))
    }

    /// Check that every terminal of a node is covered (call before a report).
    pub fn covers(&self, terminals: &[Vec<i32>]) -> Result<(), String> {
        for t in terminals {
            self.available(t)?;
        }
        Ok(())
    }

    /// Required depth of a terminal for a pole of order `p` on its coefficient: `needed_order + p`.
    pub fn required(&self, p: u32) -> i64 {
        self.spec.needed_order + i64::from(p)
    }

    /// Status of a terminal at one box key with composed pole order `p`.
    pub fn status(&self, terminal: &[i32], p: u32) -> Result<BoxStatus, String> {
        let avail = self.available(terminal)?;
        let req = self.required(p);
        Ok(if req <= avail { BoxStatus::Within } else { BoxStatus::Exceeds(req - avail) })
    }
}
