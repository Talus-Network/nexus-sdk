use {
    crate::prelude::*,
    nexus_sdk::{nexus::scheduler::OccurrenceSpec, types::effective_priority_fee_percentage},
};

/// Resolves one occurrence against the current chain time.
pub(crate) fn occurrence_spec(
    clock_ms: u64,
    start_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
    priority_fee_percentage: Option<u64>,
) -> AnyResult<OccurrenceSpec, NexusCliError> {
    let start_time_ms = match (start_ms, start_offset_ms) {
        (Some(start_time_ms), None) => start_time_ms,
        (None, Some(offset_ms)) => clock_ms
            .checked_add(offset_ms)
            .ok_or_else(|| NexusCliError::Any(anyhow!("occurrence start time overflows u64")))?,
        (None, None) => clock_ms,
        (Some(_), Some(_)) => {
            return Err(NexusCliError::Any(anyhow!(
                "absolute start and start offset are mutually exclusive"
            )));
        }
    };
    let deadline_ms = deadline_offset_ms
        .map(|offset_ms| {
            start_time_ms
                .checked_add(offset_ms)
                .ok_or_else(|| anyhow!("occurrence deadline overflows u64"))
        })
        .transpose()
        .map_err(NexusCliError::Any)?;
    let priority_fee_percentage =
        effective_priority_fee_percentage(priority_fee_percentage).map_err(NexusCliError::Any)?;

    OccurrenceSpec {
        start_time_ms,
        deadline_ms,
        priority_fee_percentage,
    }
    .validate()
    .map_err(NexusCliError::Any)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_start_uses_chain_time() {
        let occurrence =
            occurrence_spec(1_000, None, None, Some(50), None).expect("occurrence is valid");

        assert_eq!(occurrence.start_time_ms, 1_000);
        assert_eq!(occurrence.deadline_ms, Some(1_050));
    }

    #[test]
    fn offset_is_relative_to_chain_time() {
        let occurrence =
            occurrence_spec(1_000, None, Some(250), None, Some(25)).expect("occurrence is valid");

        assert_eq!(occurrence.start_time_ms, 1_250);
        assert_eq!(occurrence.priority_fee_percentage, 25);
    }

    #[test]
    fn overflowing_deadline_is_rejected() {
        let error = occurrence_spec(u64::MAX, None, None, Some(1), None)
            .expect_err("deadline overflow must fail");

        assert!(error.to_string().contains("deadline overflows"));
    }
}
