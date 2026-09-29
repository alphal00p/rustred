//! Bounded selected fields, not an entire record Value. Diagnostic arrays and
//! error text are skipped with serde's IgnoredAny; their bytes remain hashed.
use super::super::super::invalid;
use serde::Deserialize;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::cell::Cell;
use std::fmt;
use std::io;

const KEY_BYTES: u64 = 128;
// Canonical geometry, stats and epoch metadata are fixed-field objects with
// at most32 coordinate axes/u64 counters. This is not a whole-record cap.
const FIELD_BYTES: u64 = 8192;

pub(super) struct Fields {
    values: Map<String, Value>,
    pub frontiers: Option<u64>,
    pub has_error: Option<bool>,
}

impl Fields {
    pub fn value(&self, key: &str) -> io::Result<&Value> {
        self.values
            .get(key)
            .ok_or_else(|| invalid("epoch record required field missing"))
    }
    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
    pub fn u64(&self, key: &str) -> io::Result<u64> {
        self.value(key)?
            .as_u64()
            .ok_or_else(|| invalid("epoch record integer field"))
    }
    pub fn text(&self, key: &str) -> io::Result<&str> {
        self.value(key)?
            .as_str()
            .ok_or_else(|| invalid("epoch record text field"))
    }
    pub fn boolean(&self, key: &str) -> io::Result<bool> {
        self.value(key)?
            .as_bool()
            .ok_or_else(|| invalid("epoch record bool field"))
    }
}

struct Count(u64);
impl<'de> Deserialize<'de> for Count {
    fn deserialize<D: de::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Counter;
        impl<'de> Visitor<'de> for Counter {
            type Value = Count;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("frontier array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<Count, A::Error> {
                let mut count = 0u64;
                while items.next_element::<de::IgnoredAny>()?.is_some() {
                    count = count
                        .checked_add(1)
                        .filter(|&n| n <= u32::MAX as u64)
                        .ok_or_else(|| de::Error::custom("epoch record frontier count range"))?;
                }
                Ok(Count(count))
            }
        }
        decoder.deserialize_seq(Counter)
    }
}

pub(super) struct Seed<'a>(pub &'a Cell<u64>);
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Fields;
    fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<Fields, D::Error> {
        self.0.set(KEY_BYTES);
        decoder.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Fields;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("epoch record object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Fields, A::Error> {
        let mut fields = Fields {
            values: Map::new(),
            frontiers: None,
            has_error: None,
        };
        loop {
            self.0.set(KEY_BYTES);
            let Some(key) = map.next_key::<String>()? else {
                break;
            };
            match key.as_str() {
                "frontiers" => {
                    if fields.frontiers.is_some() {
                        return Err(de::Error::duplicate_field("frontiers"));
                    }
                    self.0.set(u64::MAX);
                    fields.frontiers = Some(map.next_value::<Count>()?.0);
                }
                "error" => {
                    if fields.has_error.is_some() {
                        return Err(de::Error::duplicate_field("error"));
                    }
                    // Only nullness controls local coverage; text is diagnostic
                    // and can be arbitrarily long. No String is allocated here.
                    self.0.set(u64::MAX);
                    fields.has_error = Some(map.next_value::<Option<de::IgnoredAny>>()?.is_some());
                }
                "g2" => {
                    if fields.values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate epoch G2 field"));
                    }
                    // Bounded by the planner's point/anchor cap; ordinary
                    // geometry and counter fields keep their small budget.
                    self.0.set(128 * (1 << 18) + FIELD_BYTES);
                    fields.values.insert(key, map.next_value::<Value>()?);
                }
                "id"
                | "record_kind"
                | "phase"
                | "owner"
                | "lower"
                | "upper"
                | "rank"
                | "power_bounds"
                | "epoch"
                | "stats"
                | "accepted_events"
                | "seconds"
                | "representative_id"
                | "local_inspection_finished"
                | "local_classification_discharged"
                | "residual_inspection_finished"
                | "native_inspection_scope"
                | "initial_overlap"
                | "containment_authority"
                | "responsibility_status"
                | "conservative_route_overcover"
                | "optional_refusal_provenance_truncated"
                | "optional_refusal_provenance_scope" => {
                    if fields.values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate epoch record field"));
                    }
                    self.0.set(FIELD_BYTES);
                    fields.values.insert(key, map.next_value::<Value>()?);
                }
                _ => {
                    // Refusal/frontier diagnostics and other non-authority
                    // details are authenticated but belong to cold reinspection.
                    self.0.set(u64::MAX);
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
        }
        Ok(fields)
    }
}
