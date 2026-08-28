use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum MetadataValue {
    String(String),
    StringArray(Vec<String>),
}

/// Application-owned view of the parser/lowering boundary.
///
/// `lowering.rs` is the only module allowed to translate the public
/// `rustred::input` API into this view. Keeping the rest of the
/// application on this small DTO prevents it from growing a second expression
/// grammar or duplicating affine lowering.
#[derive(Clone, Debug)]
pub(crate) struct LoweredProject {
    input_form: &'static str,
    input_schema: String,
    metadata: BTreeMap<String, MetadataValue>,
    lowered: rustred::input::LoweredProject,
}

impl LoweredProject {
    pub(crate) fn new(
        input_form: &'static str,
        input_schema: String,
        metadata: BTreeMap<String, MetadataValue>,
        lowered: rustred::input::LoweredProject,
    ) -> Self {
        Self {
            input_form,
            input_schema,
            metadata,
            lowered,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        &'static str,
        String,
        BTreeMap<String, MetadataValue>,
        rustred::input::LoweredProject,
    ) {
        (
            self.input_form,
            self.input_schema,
            self.metadata,
            self.lowered,
        )
    }
}
