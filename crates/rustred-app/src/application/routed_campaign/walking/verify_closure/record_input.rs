//! Keep CP6's typed projection in memory; do not serialize it to JSON text
//! only to parse it again. CP5 still consumes its original JSON-line format.
use super::{RecordRow, result_binding};
use serde_json::Value;
use std::io::{self, BufRead, BufReader, Read};

pub(super) enum RecordValue {
    Native(Value),
    Text(String),
}

impl RecordValue {
    pub(super) fn decode(
        self,
        digest: bool,
    ) -> Result<(RecordRow, Option<[u8; 16]>), serde_json::Error> {
        if !digest {
            return match self {
                Self::Native(value) => serde_json::from_value(value),
                Self::Text(text) => serde_json::from_str(&text),
            }
            .map(|row| (row, None));
        }
        let mut value = match self {
            Self::Native(value) => value,
            Self::Text(text) => serde_json::from_str(&text)?,
        };
        let digest = result_binding::record_digest(&mut value);
        Ok((serde_json::from_value(value)?, Some(digest)))
    }
}

pub(super) struct Records<R> {
    input: BufReader<R>,
    remaining: Option<usize>,
    ended: bool,
}

impl<R: Read> Records<R> {
    pub(super) fn new(input: R, native_count: Option<usize>) -> Self {
        Self {
            input: BufReader::with_capacity(1 << 20, input),
            remaining: native_count,
            ended: false,
        }
    }

    pub(super) fn next(&mut self) -> io::Result<Option<RecordValue>> {
        if self.ended {
            return Ok(None);
        }
        if let Some(remaining) = &mut self.remaining {
            if *remaining == 0 {
                // Read through EOF on the same captured authenticated reader,
                // even for empty segments. Never accept unread trailing bytes.
                if self.input.read(&mut [0])? != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "epoch record segment trailing bytes",
                    ));
                }
                self.ended = true;
                return Ok(None);
            }
            let value = super::super::epoch::records::wire::read(&mut self.input)?
                .project()
                .map_err(io::Error::other)?;
            *remaining -= 1;
            return Ok(Some(RecordValue::Native(value)));
        }
        loop {
            let mut line = String::new();
            if self.input.read_line(&mut line)? == 0 {
                self.ended = true;
                return Ok(None);
            }
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            if !line.is_empty() {
                return Ok(Some(RecordValue::Text(line)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_reader_rejects_trailing_or_missing_frames() {
        assert!(Records::new(&b""[..], Some(0)).next().unwrap().is_none());
        assert!(Records::new(&b"x"[..], Some(0)).next().is_err());
        assert!(Records::new(&b""[..], Some(1)).next().is_err());
    }

    #[test]
    fn legacy_reader_keeps_lines_semantics() {
        let mut input = Records::new(&b"\n\r\nhello\r\nworld"[..], None);
        for expected in ["hello", "world"] {
            let Some(RecordValue::Text(actual)) = input.next().unwrap() else {
                panic!("missing text")
            };
            assert_eq!(actual, expected);
        }
        assert!(input.next().unwrap().is_none());
        assert!(input.next().unwrap().is_none());
    }
}
