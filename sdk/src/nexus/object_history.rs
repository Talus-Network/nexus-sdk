//! Shared object update history reconstruction.

use {
    crate::{
        nexus::{
            crawler::{Crawler, ObjectUpdateReference, TransactionUpdate},
            error::NexusError,
        },
        sui,
    },
    anyhow::anyhow,
    tokio::time::{Duration, Instant},
};

pub(crate) const MAX_TRANSACTION_NOT_FOUND_RETRIES: usize = 3;

/// Input that anchors one shared object history reconstruction.
pub(crate) struct ObjectHistoryRequest<'a> {
    pub object_name: &'a str,
    pub object_id: sui::types::Address,
    pub expected_type: sui::types::StructTag,
    pub latest: ObjectUpdateReference,
    pub after_version: Option<sui::types::Version>,
    pub poll_interval: Duration,
    pub deadline: Instant,
}

pub(crate) fn version_or_none(version: Option<sui::types::Version>) -> String {
    version
        .map(|version| version.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn is_transaction_not_found(error: &anyhow::Error) -> bool {
    error.chain().any(|source| {
        source
            .downcast_ref::<tonic::Status>()
            .is_some_and(|status| status.code() == tonic::Code::NotFound)
    })
}

async fn fetch_transaction_update_with_visibility_retry(
    crawler: &Crawler,
    digest: sui::types::Digest,
    poll_interval: Duration,
    deadline: Instant,
) -> anyhow::Result<TransactionUpdate> {
    let mut retries = 0;

    loop {
        match crawler.get_transaction_update(digest).await {
            Ok(update) => return Ok(update),
            Err(error)
                if retries < MAX_TRANSACTION_NOT_FOUND_RETRIES
                    && is_transaction_not_found(&error) =>
            {
                let Some(retry_at) = Instant::now().checked_add(poll_interval) else {
                    return Err(error);
                };
                if retry_at >= deadline {
                    return Err(error);
                }

                retries += 1;
                tokio::time::sleep_until(retry_at).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Reconstructs validated shared object updates in chronological order.
pub(crate) async fn fetch_shared_object_history(
    crawler: &Crawler,
    request: ObjectHistoryRequest<'_>,
) -> Result<Vec<TransactionUpdate>, NexusError> {
    let ObjectHistoryRequest {
        object_name,
        object_id,
        expected_type,
        latest,
        after_version,
        poll_interval,
        deadline,
    } = request;
    let mut cursor = latest;
    let mut reverse_updates = Vec::new();
    let last_reconstructed = version_or_none(after_version);

    loop {
        if !matches!(cursor.owner, sui::types::Owner::Shared(_)) {
            return Err(NexusError::Parsing(anyhow!(
                "{object_name} object '{object_id}' at version {} is not shared",
                cursor.version
            )));
        }
        if cursor.object_type != expected_type {
            return Err(NexusError::Parsing(anyhow!(
                "{object_name} object '{object_id}' at version {} has type '{}', expected '{}'",
                cursor.version,
                cursor.object_type,
                expected_type
            )));
        }
        if after_version == Some(cursor.version) {
            break;
        }
        if let Some(after_version) = after_version {
            if cursor.version < after_version {
                return Err(NexusError::Rpc(anyhow!(
                    "{object_name} object '{object_id}' moved backwards from delivered version {after_version} to observed version {}",
                    cursor.version
                )));
            }
        }

        let update = fetch_transaction_update_with_visibility_retry(
            crawler,
            cursor.previous_transaction,
            poll_interval,
            deadline,
        )
        .await
        .map_err(|error| {
            NexusError::Rpc(error.context(format!(
                "{object_name} '{object_id}' history is incomplete: missing transaction '{}' for object version {}; last successfully reconstructed version {last_reconstructed}",
                cursor.previous_transaction, cursor.version
            )))
        })?;
        if update.effects.lamport_version != cursor.version {
            return Err(NexusError::Rpc(anyhow!(
                "Transaction '{}' produced version {} while {object_name} object '{object_id}' is at version {}",
                update.digest,
                update.effects.lamport_version,
                cursor.version
            )));
        }

        let changed = update
            .effects
            .changed_objects
            .iter()
            .find(|changed| changed.object_id == object_id)
            .ok_or_else(|| {
                NexusError::Rpc(anyhow!(
                    "Transaction '{}' did not update {object_name} object '{object_id}'",
                    update.digest
                ))
            })?;
        let output_digest = match &changed.output_state {
            sui::types::ObjectOut::ObjectWrite { digest, .. } => *digest,
            output => {
                return Err(NexusError::Rpc(anyhow!(
                    "Transaction '{}' has unsupported output state {output:?} for {object_name} object '{object_id}'",
                    update.digest
                )))
            }
        };
        if output_digest != cursor.digest {
            return Err(NexusError::Rpc(anyhow!(
                "Transaction '{}' output digest for {object_name} object '{object_id}' does not match object version {}",
                update.digest,
                cursor.version
            )));
        }

        let (previous_version, previous_digest) = match &changed.input_state {
            sui::types::ObjectIn::NotExist => {
                if let Some(after_version) = after_version {
                    return Err(NexusError::Rpc(anyhow!(
                        "{object_name} object '{object_id}' update chain ended before delivered version {after_version}"
                    )));
                }
                reverse_updates.push(update);
                break;
            }
            sui::types::ObjectIn::Exist {
                version, digest, ..
            } => (*version, *digest),
            input => {
                return Err(NexusError::Rpc(anyhow!(
                    "Transaction '{}' has unsupported input state {input:?} for {object_name} object '{object_id}'",
                    update.digest
                )))
            }
        };
        reverse_updates.push(update);

        if previous_version >= cursor.version {
            return Err(NexusError::Rpc(anyhow!(
                "{object_name} object '{object_id}' update chain did not move backwards from version {} to {previous_version}",
                cursor.version
            )));
        }
        if after_version == Some(previous_version) {
            break;
        }
        if let Some(after_version) = after_version {
            if previous_version < after_version {
                return Err(NexusError::Rpc(anyhow!(
                    "{object_name} object '{object_id}' update chain crossed delivered version {after_version} at version {previous_version}"
                )));
            }
        }

        cursor = crawler
            .get_object_update_reference(object_id, Some(previous_version))
            .await
            .map_err(|error| {
                NexusError::Rpc(error.context(format!(
                    "{object_name} '{object_id}' history is incomplete: missing object version {previous_version}; last successfully reconstructed version {last_reconstructed}"
                )))
            })?;
        if cursor.digest != previous_digest {
            return Err(NexusError::Rpc(anyhow!(
                "{object_name} object '{object_id}' digest at historical version {previous_version} does not match transaction '{}' input",
                reverse_updates
                    .last()
                    .expect("the current update was retained")
                    .digest
            )));
        }
    }

    reverse_updates.reverse();
    Ok(reverse_updates)
}
