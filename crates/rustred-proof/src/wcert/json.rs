//! Compact JSON re-serialisation identical to Python's `json.dumps(v, separators=(',', ':'))` (ensure_ascii, dict
//! order as in the file): the form of the binding hashes `target_rhs_sha256` ('list') and `domain_sha256` of
//! tools/final_cert/fc_tool.py, independent of the whitespace the table writer used.
use crate::core::J;

/// Decode the JSON escapes of a raw string body (between the quotes).
pub fn unescape(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    if !s.contains('\\') {
        return s.into_owned();
    }
    let mut out = String::new();
    let mut it = s.chars().peekable();
    let mut pending_high: Option<u32> = None;
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('/') => out.push('/'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('u') => {
                let h: String = (0..4).filter_map(|_| it.next()).collect();
                let v = u32::from_str_radix(&h, 16).unwrap_or(0xfffd);
                if (0xd800..0xdc00).contains(&v) {
                    pending_high = Some(v);
                } else if (0xdc00..0xe000).contains(&v) {
                    if let Some(hi) = pending_high.take() {
                        let cp = 0x10000 + ((hi - 0xd800) << 10) + (v - 0xdc00);
                        out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                    }
                } else {
                    out.push(char::from_u32(v).unwrap_or('\u{fffd}'));
                }
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

/// Python's `json.dumps` string encoding (ensure_ascii=True).
pub fn py_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", u));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Re-serialise a parsed value of the line `b` compactly, as Python's json.dumps would.
pub fn py_dump(j: &J, b: &[u8], out: &mut String) {
    match j {
        J::Null => out.push_str("null"),
        J::Bool(true) => out.push_str("true"),
        J::Bool(false) => out.push_str("false"),
        J::Num(s, e) => out.push_str(&String::from_utf8_lossy(&b[*s..*e])),
        J::Str(s, e) => py_str(&unescape(&b[*s..*e]), out),
        J::Arr(v) => {
            out.push('[');
            for (i, x) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                py_dump(x, b, out);
            }
            out.push(']');
        }
        J::Obj(v) => {
            out.push('{');
            for (i, ((s, e), x)) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                py_str(&unescape(&b[*s..*e]), out);
                out.push(':');
                py_dump(x, b, out);
            }
            out.push('}');
        }
    }
}
