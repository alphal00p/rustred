use super::*;

#[test]
fn trace_support_matches_verified_affine_transport_order_and_logical_usage() {
    let (source, target) = fixture();
    let prepared = prepared(&source, target, [true, true, false]);
    for powers in [[1, 1, 0], [1, 1, -1], [2, 3, -3], [0, 1, -2], [-2, 0, -1]] {
        let source = key(powers);
        let mut materialized_usage = None;
        let result = prepared
            .transport_with_usage(&source, Default::default(), |usage| {
                materialized_usage = Some((usage.operations, usage.endpoints));
                Ok(())
            })
            .unwrap();
        let mut support_usage = None;
        let mut support = prepared
            .transport_support_with_usage(&source, Default::default(), |usage| {
                support_usage = Some((usage.operations, usage.endpoints));
                Ok(())
            })
            .unwrap();
        assert_eq!(materialized_usage, support_usage);
        let keys = support.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            keys,
            result
                .terms()
                .iter()
                .map(|term| term.key().clone())
                .collect::<Vec<_>>()
        );
        assert!(support.next().is_none());
        assert!(support.next().is_none());
        assert_eq!(support.size_hint(), (0, Some(0)));
    }
}

#[test]
fn trace_support_transport_input_and_virtual_limits_preserve_atomic_errors() {
    let (source, target) = fixture();
    let prepared = prepared(&source, target, [true, true, false]);
    for source in [
        IntegralKey::try_new([1]).unwrap(),
        key([1, 1, 1]),
        key([1, 1, i64::MIN]),
    ] {
        let full = prepared
            .transport(&source, Default::default())
            .err()
            .unwrap();
        let support = prepared
            .transport_support_with_usage(&source, Default::default(), |_| Ok(()))
            .err()
            .unwrap();
        assert_eq!(full, support);
    }
    for limits in [
        ExpansionLimits {
            max_endpoints: 1,
            ..Default::default()
        },
        ExpansionLimits {
            max_retained_coefficient_terms: 1,
            ..Default::default()
        },
        ExpansionLimits {
            max_retained_coefficient_clone_owned_bytes: 1,
            ..Default::default()
        },
        ExpansionLimits {
            max_retained_endpoint_key_bytes: 1,
            ..Default::default()
        },
    ] {
        let source = key([1, 1, -3]);
        let full = prepared.transport(&source, limits).err().unwrap();
        let mut visited = 0;
        let support = prepared
            .transport_support_with_usage(&source, limits, |_| Ok(()))
            .and_then(|mut keys| {
                keys.try_for_each(|key| {
                    key?;
                    visited += 1;
                    Ok(())
                })
            });
        assert_eq!(support, Err(full));
        assert_eq!(visited, 0);
    }
}
