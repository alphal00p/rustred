use super::descriptor::{
    CoordinateGroups, DegreeRow, Direction, Error, Limits, OrderDescriptor, reserve,
};

const MAGIC_V1: &[u8; 8] = b"RRDORD01";
const MAGIC_V2: &[u8; 8] = b"RRDORD02";

pub(crate) fn encode(descriptor: &OrderDescriptor) -> Result<Box<[u8]>, Error> {
    let size = descriptor.support_weights.len();
    let bytes = OrderDescriptor::dimensions(
        size,
        descriptor.pre_support_degree_rows.len(),
        descriptor.degree_rows.len(),
        Limits {
            max_encoded_bytes: usize::MAX,
            max_comparison_terms: usize::MAX,
        },
    )?;
    let mut result = reserve(bytes)?;
    let has_prefix = !descriptor.pre_support_degree_rows.is_empty();
    result.extend_from_slice(if has_prefix { MAGIC_V2 } else { MAGIC_V1 });
    result.extend_from_slice(
        &u64::try_from(size)
            .map_err(|_| Error::DimensionOverflow)?
            .to_le_bytes(),
    );
    result.extend_from_slice(
        &u64::try_from(descriptor.degree_rows.len())
            .map_err(|_| Error::DimensionOverflow)?
            .to_le_bytes(),
    );
    if has_prefix {
        result.extend_from_slice(
            &u64::try_from(descriptor.pre_support_degree_rows.len())
                .map_err(|_| Error::DimensionOverflow)?
                .to_le_bytes(),
        );
    }
    result.push(match descriptor.coordinate_groups {
        CoordinateGroups::ActiveFirst => 0,
        CoordinateGroups::InactiveFirst => 1,
        CoordinateGroups::Interleaved => 2,
    });
    result.push(direction(descriptor.active_direction));
    result.push(direction(descriptor.inactive_direction));
    for &weight in &descriptor.support_weights {
        result.extend_from_slice(&weight.to_le_bytes());
    }
    for priority in [
        &descriptor.support_priority,
        &descriptor.coordinate_priority,
    ] {
        for &axis in priority {
            result.extend_from_slice(
                &u64::try_from(axis)
                    .map_err(|_| Error::DimensionOverflow)?
                    .to_le_bytes(),
            );
        }
    }
    for row in descriptor
        .pre_support_degree_rows
        .iter()
        .chain(&descriptor.degree_rows)
    {
        for &weight in row.active.iter().chain(&row.inactive) {
            result.extend_from_slice(&weight.to_le_bytes());
        }
    }
    debug_assert_eq!(bytes, result.len());
    Ok(result.into_boxed_slice())
}

fn direction(value: Direction) -> u8 {
    match value {
        Direction::Ascending => 0,
        Direction::Descending => 1,
    }
}

pub(crate) fn decode(bytes: &[u8], limits: Limits) -> Result<OrderDescriptor, Error> {
    if bytes.len() > limits.max_encoded_bytes {
        return Err(Error::ResourceLimit {
            resource: "encoded bytes",
            actual: bytes.len(),
            limit: limits.max_encoded_bytes,
        });
    }
    let has_prefix = match bytes.get(..8) {
        Some(magic) if magic == MAGIC_V1 => false,
        Some(magic) if magic == MAGIC_V2 => true,
        _ => return Err(Error::InvalidEncoding),
    };
    let mut reader = Reader { bytes, cursor: 8 };
    let size = reader.usize()?;
    let row_count = reader.usize()?;
    let pre_row_count = if has_prefix { reader.usize()? } else { 0 };
    // Canonical v2 always has a nonempty prefix; empty descriptors retain
    // exactly their original v1 bytes, not a second equivalent encoding.
    if has_prefix && pre_row_count == 0 {
        return Err(Error::InvalidEncoding);
    }
    // Reject hostile dimensions, truncated payloads and trailing bytes before
    // any dimension-dependent allocation, including the row container.
    if OrderDescriptor::dimensions(size, pre_row_count, row_count, limits)? != bytes.len() {
        return Err(Error::InvalidEncoding);
    }
    let coordinate_groups = match reader.byte()? {
        0 => CoordinateGroups::ActiveFirst,
        1 => CoordinateGroups::InactiveFirst,
        2 => CoordinateGroups::Interleaved,
        _ => return Err(Error::InvalidEncoding),
    };
    let active_direction = reader.direction()?;
    let inactive_direction = reader.direction()?;
    let support_weights = reader.weights(size)?;
    let support_priority = reader.priority(size)?;
    let coordinate_priority = reader.priority(size)?;
    let mut pre_support_degree_rows = reserve(pre_row_count)?;
    for _ in 0..pre_row_count {
        pre_support_degree_rows.push(DegreeRow {
            active: reader.weights(size)?,
            inactive: reader.weights(size)?,
        });
    }
    let mut degree_rows = reserve(row_count)?;
    for _ in 0..row_count {
        degree_rows.push(DegreeRow {
            active: reader.weights(size)?,
            inactive: reader.weights(size)?,
        });
    }
    debug_assert_eq!(reader.cursor, bytes.len());
    Ok(OrderDescriptor {
        pre_support_degree_rows,
        support_weights,
        support_priority,
        degree_rows,
        coordinate_priority,
        coordinate_groups,
        active_direction,
        inactive_direction,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, Error> {
        let value = *self.bytes.get(self.cursor).ok_or(Error::InvalidEncoding)?;
        self.cursor += 1;
        Ok(value)
    }
    fn word(&mut self) -> Result<u64, Error> {
        let end = self.cursor.checked_add(8).ok_or(Error::DimensionOverflow)?;
        let word = self
            .bytes
            .get(self.cursor..end)
            .ok_or(Error::InvalidEncoding)?;
        self.cursor = end;
        Ok(u64::from_le_bytes(
            word.try_into().expect("eight-byte range"),
        ))
    }
    fn usize(&mut self) -> Result<usize, Error> {
        usize::try_from(self.word()?).map_err(|_| Error::DimensionOverflow)
    }
    fn direction(&mut self) -> Result<Direction, Error> {
        match self.byte()? {
            0 => Ok(Direction::Ascending),
            1 => Ok(Direction::Descending),
            _ => Err(Error::InvalidEncoding),
        }
    }
    fn weights(&mut self, size: usize) -> Result<Vec<u64>, Error> {
        let mut values = reserve(size)?;
        for _ in 0..size {
            values.push(self.word()?);
        }
        Ok(values)
    }
    fn priority(&mut self, size: usize) -> Result<Vec<usize>, Error> {
        let mut values = reserve(size)?;
        for _ in 0..size {
            values.push(self.usize()?);
        }
        Ok(values)
    }
}
