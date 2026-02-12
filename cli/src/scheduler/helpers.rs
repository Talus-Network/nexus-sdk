use {
    crate::{prelude::*, workflow},
    nexus_sdk::nexus::crawler::Crawler,
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

/// Fetch the encrypted entry port mapping for the provided DAG entry group.
pub(crate) async fn fetch_encryption_targets(
    crawler: &Crawler,
    dag_id: &sui::types::Address,
    entry_group: &str,
) -> AnyResult<HashMap<String, Vec<String>>, NexusCliError> {
    workflow::fetch_encrypted_entry_ports(crawler, entry_group.to_owned(), dag_id).await
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
}
