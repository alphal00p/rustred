//! Read the existing bincode directory without trusting allocation counts.
//! Every element occupies at least one wire byte; exact binding and key arity
//! are checked before allocating their variable-size fields.
use super::*;
use rustred::{family::IntegralKey, persistence::application_cache::ApplicationCacheStage};

struct Frame<'a>(&'a [u8]);

impl Frame<'_> {
    fn value<T: bincode::Decode<()>>(&mut self) -> Result<T, AppError> {
        resource_checkpoint()?;
        let (value, consumed) =
            bincode::decode_from_slice(self.0, bincode::config::standard()).map_err(fail)?;
        self.0 = self
            .0
            .get(consumed..)
            .ok_or_else(|| fail("directory cursor overflow"))?;
        Ok(value)
    }

    fn vector<T>(
        &mut self,
        mut read: impl FnMut(&mut Self) -> Result<T, AppError>,
    ) -> Result<Vec<T>, AppError> {
        let count: usize = self.value()?;
        if count > self.0.len() {
            return Err(fail("directory collection exceeds its actual frame"));
        }
        let bytes = count
            .checked_mul(size_of::<T>())
            .ok_or_else(|| fail("directory allocation size overflow"))?;
        resource_allocation(bytes)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(fail)?;
        for _ in 0..count {
            resource_checkpoint()?;
            values.push(read(self)?);
        }
        Ok(values)
    }

    fn key(&mut self, arity: usize) -> Result<ApplicationCacheKey, AppError> {
        let stage: ApplicationCacheStage = self.value()?;
        let owners: usize = self.value()?;
        let expected_owners = usize::from(stage == ApplicationCacheStage::Apply) * arity;
        if owners != expected_owners || owners > self.0.len() {
            return Err(fail("directory key owner differs from its binding"));
        }
        resource_allocation(owners)?;
        let mut owner = Vec::new();
        owner.try_reserve_exact(owners).map_err(fail)?;
        for _ in 0..owners {
            owner.push(self.value::<bool>()?);
        }
        let powers: usize = self.value()?;
        if powers != arity || powers > self.0.len() {
            return Err(fail("directory key arity differs from its binding"));
        }
        resource_allocation(
            powers
                .checked_mul(size_of::<i64>())
                .ok_or_else(|| fail("directory key allocation overflow"))?,
        )?;
        let mut values = Vec::new();
        values.try_reserve_exact(powers).map_err(fail)?;
        for _ in 0..powers {
            values.push(self.value::<i64>()?);
        }
        let integral = IntegralKey::try_new(values).map_err(fail)?;
        let key = match stage {
            ApplicationCacheStage::GuardedRoot => ApplicationCacheKey::guarded_root(&integral),
            ApplicationCacheStage::Route => ApplicationCacheKey::route(&integral),
            ApplicationCacheStage::Apply => ApplicationCacheKey::apply(owner, &integral),
        };
        key.validate(arity).map_err(binary_error)?;
        Ok(key)
    }
}

pub(super) fn decode(
    bytes: &[u8],
    expected: &ApplicationCacheBinding,
    fingerprints: bool,
) -> Result<(Manifest, usize), AppError> {
    let mut binding = BoundedBytes::new(bytes.len());
    bincode::encode_into_std_write(expected, &mut binding, bincode::config::standard())
        .map_err(encode_error)?;
    let binding = binding.finish();
    if !bytes.starts_with(&binding) {
        return Err(fail(
            "directory binding differs from its expected application",
        ));
    }
    let mut frame = Frame(&bytes[binding.len()..]);
    let bundle_managed = frame.value()?;
    let generation = frame.value()?;
    let runs = frame.vector(|frame| {
        frame.vector(|frame| {
            let digest = frame.value()?;
            let bytes = frame.value()?;
            let decoded_digest = frame.value()?;
            let decoded_bytes = frame.value()?;
            let records = frame.value()?;
            let first = frame.key(expected.physical_arity)?;
            let last = frame.key(expected.physical_arity)?;
            let key_fingerprints = if fingerprints && frame.value::<Option<()>>()?.is_some() {
                let values = frame.vector(|frame| frame.value::<u64>())?;
                if values.len() != records {
                    return Err(fail("directory fingerprint count differs from its records"));
                }
                Some(values)
            } else {
                None
            };
            Ok(Chunk {
                digest,
                bytes,
                decoded_digest,
                decoded_bytes,
                records,
                first,
                last,
                key_fingerprints,
            })
        })
    })?;
    let consumed = bytes.len() - frame.0.len();
    resource_allocation(
        expected
            .family_fingerprint
            .len()
            .saturating_add(expected.layer.len()),
    )?;
    let manifest = Manifest {
        binding: expected.clone(),
        bundle_managed,
        generation,
        runs,
    };
    resource_checkpoint()?;
    Ok((manifest, consumed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Manifest {
        let binding = ApplicationCacheBinding {
            artifact_digest: [1; 32],
            family_fingerprint: "frame-fixture".into(),
            physical_arity: 1,
            layer: "guarded-root-v1".into(),
        };
        let key = ApplicationCacheKey::guarded_root(&IntegralKey::try_new([1]).unwrap());
        let chunk = Chunk {
            digest: [2; 32],
            bytes: 1,
            decoded_digest: [3; 32],
            decoded_bytes: 1,
            records: 1,
            first: key.clone(),
            last: key,
            key_fingerprints: None,
        };
        Manifest {
            binding,
            bundle_managed: false,
            generation: 7,
            runs: vec![vec![chunk]],
        }
    }

    #[test]
    fn current_and_legacy_directory_wire_encodings_retain_exact_contents() {
        let manifest = fixture();
        let limits = SavedApplicationCachePackLimits::default();
        let current = super::super::encode(&manifest, limits).unwrap();
        assert_eq!(
            super::super::decode(&current, &manifest.binding, limits)
                .unwrap()
                .0,
            manifest
        );
        let old = legacy::encode_for_test(&manifest);
        assert_eq!(
            super::super::decode(&old, &manifest.binding, limits)
                .unwrap()
                .0,
            manifest
        );
    }

    #[test]
    fn declared_directory_vector_cannot_allocate_beyond_its_actual_frame() {
        let manifest = fixture();
        let mut bytes = MAGIC.to_vec();
        bincode::encode_into_std_write(
            (&manifest.binding, false, 0u64, usize::MAX),
            &mut bytes,
            bincode::config::standard(),
        )
        .unwrap();
        bytes.extend_from_slice(blake3::hash(&bytes).as_bytes());
        let error = super::super::decode(
            &bytes,
            &manifest.binding,
            SavedApplicationCachePackLimits::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("actual frame"));
    }

    #[test]
    fn directory_decode_preserves_real_allocation_resource_failures() {
        use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};
        let manifest = fixture();
        let limits = SavedApplicationCachePackLimits::default();
        let bytes = super::super::encode(&manifest, limits).unwrap();
        let budget = ResourceBudget::with_memory_monitor(
            ResourceLimits {
                max_process_rss_bytes: Some(100),
                ..Default::default()
            },
            || Ok(100),
        );
        let error = budget
            .run(|| super::super::decode(&bytes, &manifest.binding, limits))
            .unwrap_err();
        assert!(matches!(
            error.resource_limit_error(),
            Some(ResourceLimitError::Memory { .. })
        ));
    }
}
