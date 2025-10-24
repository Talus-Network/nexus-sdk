use {
    crate::{prelude::*, workflow},
    std::collections::HashMap,
};

/// Parse metadata pairs provided as `key=value` strings.
pub(crate) fn parse_metadata(pairs: &[String]) -> AnyResult<Vec<(String, String)>, NexusCliError> {
    let mut result = Vec::with_capacity(pairs.len());
    for pair in pairs {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(NexusCliError::Any(anyhow!(
                "Invalid metadata entry '{pair}'. Expected format key=value"
            )));
        };
        if key.trim().is_empty() {
            return Err(NexusCliError::Any(anyhow!(
                "Metadata key in '{pair}' cannot be empty"
            )));
        }
        result.push((key.trim().to_owned(), value.trim().to_owned()));
    }
    Ok(result)
}

pub(crate) fn ensure_start_before_deadline(
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
) -> AnyResult<(), NexusCliError> {
    if let (Some(start), Some(deadline)) = (start_ms, deadline_ms) {
        if deadline < start {
            return Err(NexusCliError::Any(anyhow!(
                "Deadline ({deadline}) cannot be earlier than start ({start})"
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_schedule_options(
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
    require_start: bool,
) -> AnyResult<(), NexusCliError> {
    if require_start && start_ms.is_none() && start_offset_ms.is_none() {
        return Err(NexusCliError::Any(anyhow!(
            "Provide either --schedule-start-ms or --schedule-start-offset-ms"
        )));
    }

    if start_ms.is_none()
        && start_offset_ms.is_none()
        && (deadline_ms.is_some() || deadline_offset_ms.is_some())
    {
        return Err(NexusCliError::Any(anyhow!(
            "Deadline flags require a corresponding start flag"
        )));
    }

    if let Some(start) = start_ms {
        ensure_start_before_deadline(Some(start), deadline_ms)?;
    }

    ensure_offset_deadline_valid(start_offset_ms, deadline_offset_ms)?;

    Ok(())
}

pub(crate) fn ensure_offset_deadline_valid(
    start_offset: Option<u64>,
    deadline_offset: Option<u64>,
) -> AnyResult<(), NexusCliError> {
    if deadline_offset.is_some() && start_offset.is_none() {
        return Err(NexusCliError::Any(anyhow!(
            "Deadline offset requires --schedule-start-offset-ms"
        )));
    }
    Ok(())
}

pub(crate) async fn fetch_encryption_targets(
    sui: &sui::Client,
    dag_id: &sui::ObjectID,
    entry_group: &str,
) -> AnyResult<HashMap<String, Vec<String>>, NexusCliError> {
    workflow::fetch_encrypted_entry_ports(sui, entry_group.to_owned(), dag_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_metadata_splits_pairs() {
        let result = parse_metadata(&["a=b".to_string(), "c=d".to_string()]).unwrap();
        assert_eq!(
            result,
            vec![("a".into(), "b".into()), ("c".into(), "d".into())]
        );
    }

    #[test]
    fn parse_metadata_rejects_missing_equals() {
        let err = parse_metadata(&["invalid".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Invalid metadata entry"));
    }

    #[test]
    fn validate_schedule_with_absolute_start_passes() {
        validate_schedule_options(Some(10), None, None, None, false).unwrap();
    }

    #[test]
    fn validate_schedule_requires_start_when_flagged() {
        let err = validate_schedule_options(None, None, None, None, true).unwrap_err();
        assert!(err
            .to_string()
            .contains("Provide either --schedule-start-ms"));
    }

    #[test]
    fn validate_schedule_rejects_deadline_without_start() {
        let err = validate_schedule_options(None, Some(20), None, None, false).unwrap_err();
        assert!(err.to_string().contains("Deadline flags require"));
    }

    #[test]
    fn validate_schedule_allows_deadline_equal_start() {
        validate_schedule_options(Some(25), Some(25), None, None, false).unwrap();
    }

    #[test]
    fn validate_schedule_allows_deadline_offset_shorter_than_start_offset() {
        validate_schedule_options(None, None, Some(60_000), Some(30_000), false).unwrap();
    }

    #[test]
    fn ensure_offset_deadline_requires_start_offset() {
        let err = ensure_offset_deadline_valid(None, Some(5)).unwrap_err();
        assert!(err
            .to_string()
            .contains("Deadline offset requires --schedule-start-offset-ms"));
    }
}
