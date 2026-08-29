use super::CampaignExecutionWidthError;
use crate::campaign::CampaignBytes;

pub(super) fn bytes_add(
    left: CampaignBytes,
    right: CampaignBytes,
    operation: &'static str,
) -> Result<CampaignBytes, CampaignExecutionWidthError> {
    left.get()
        .checked_add(right.get())
        .map(CampaignBytes::new)
        .ok_or(CampaignExecutionWidthError::ByteCountOverflow { operation })
}

pub(super) fn bytes_sub(
    left: CampaignBytes,
    right: CampaignBytes,
    operation: &'static str,
) -> Result<CampaignBytes, CampaignExecutionWidthError> {
    left.get()
        .checked_sub(right.get())
        .map(CampaignBytes::new)
        .ok_or(CampaignExecutionWidthError::ByteCountOverflow { operation })
}

pub(super) fn bytes_mul_usize(
    bytes: CampaignBytes,
    count: usize,
    operation: &'static str,
) -> Result<CampaignBytes, CampaignExecutionWidthError> {
    let count = u64::try_from(count).map_err(|_| {
        CampaignExecutionWidthError::CoreCountDoesNotFitByteArithmetic { operation, count }
    })?;
    bytes
        .get()
        .checked_mul(count)
        .map(CampaignBytes::new)
        .ok_or(CampaignExecutionWidthError::ByteCountOverflow { operation })
}
