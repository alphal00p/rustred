//! Existing bincode records with cooperative ambient resource checks.
use std::io::{self, Read};

use super::{BinaryIoError, limits::CappedWriter};

pub(crate) fn decode_structure<T: bincode::Decode<()>>(
    bytes: &[u8],
) -> Result<(T, usize), BinaryIoError> {
    super::resource_checkpoint()?;
    let mut reader = ResourceReader { bytes, consumed: 0 };
    let value = bincode::decode_from_std_read(&mut reader, bincode::config::standard())
        .map_err(super::error::native_decode_error)?;
    super::resource_checkpoint()?;
    Ok((value, reader.consumed))
}

pub(crate) fn encode_structure(
    value: &impl bincode::Encode,
    max_bytes: usize,
) -> Result<Vec<u8>, BinaryIoError> {
    super::resource_checkpoint()?;
    let mut writer = CappedWriter::new("structural program bytes", max_bytes);
    bincode::encode_into_std_write(value, &mut writer, bincode::config::standard())
        .map_err(super::native_encode_error)?;
    Ok(writer.finish())
}

pub(crate) struct ResourceReader<'a> {
    bytes: &'a [u8],
    consumed: usize,
}
impl<'a> ResourceReader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, consumed: 0 }
    }
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len()
    }
}

impl Read for ResourceReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        super::resource_allocation(output.len()).map_err(io::Error::other)?;
        let count = output.len().min(self.bytes.len());
        output[..count].copy_from_slice(&self.bytes[..count]);
        self.bytes = &self.bytes[count..];
        self.consumed = self
            .consumed
            .checked_add(count)
            .ok_or_else(|| io::Error::other("structural reader length overflow"))?;
        Ok(count)
    }
}
impl bincode::de::read::Reader for ResourceReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), bincode::error::DecodeError> {
        std::io::Read::read_exact(self, bytes).map_err(|inner| bincode::error::DecodeError::Io {
            inner,
            additional: bytes.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

    #[test]
    fn cooperative_adapter_preserves_wire_bytes_and_typed_budget_errors() {
        let value = vec![vec![1u64, 2, 3], vec![4, 5]];
        let bytes = bincode::encode_to_vec(&value, bincode::config::standard()).unwrap();
        assert_eq!(encode_structure(&value, usize::MAX).unwrap(), bytes);
        let (decoded, used): (Vec<Vec<u64>>, _) = decode_structure(&bytes).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(used, bytes.len());
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(1),
                ..Default::default()
            },
            || Ok(2),
        );
        let result: Result<(Vec<Vec<u64>>, usize), _> = budget.run(|| decode_structure(&bytes));
        assert!(matches!(
            result,
            Err(BinaryIoError::Resource(ResourceLimitError::Memory { .. }))
        ));
    }

    #[test]
    fn resource_error_during_native_write_remains_typed() {
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        struct Pressure(Arc<AtomicU64>);
        impl bincode::Encode for Pressure {
            fn encode<E: bincode::enc::Encoder>(
                &self,
                encoder: &mut E,
            ) -> Result<(), bincode::error::EncodeError> {
                self.0.store(2, Ordering::Relaxed);
                bincode::Encode::encode(&1u8, encoder)
            }
        }
        let rss = Arc::new(AtomicU64::new(0));
        let observed = rss.clone();
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(1),
                ..Default::default()
            },
            move || Ok(observed.load(Ordering::Relaxed)),
        );
        assert!(matches!(
            budget.run(|| encode_structure(&Pressure(rss), usize::MAX)),
            Err(BinaryIoError::Resource(ResourceLimitError::Memory { .. }))
        ));
    }
}
