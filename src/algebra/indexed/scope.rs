//! Stable private naming and authentication identities for indexed fields.

use crate::algebra::CoefficientContext;

pub(super) fn encode_symbol_component(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

pub(super) fn base_context_fingerprint(base: &CoefficientContext) -> String {
    let mut result = format!(
        "rustred-base-context-v1|parameters={}",
        base.parameter_names().len()
    );
    for name in base.parameter_names() {
        result.push('|');
        result.push_str(&name.len().to_string());
        result.push(':');
        result.push_str(name);
    }
    result
}
