use crate::{sui, walrus::storage};

/// Fetch gas coin for the provided address.
pub async fn fetch_gas_coins(
    rpc_url: &storage,
    addr: sui::Address,
) -> anyhow::Result<Vec<sui::types::ObjectReference>> {
    let client = sui::grpc::Client::new(rpc_url)?;

    let request = sui::grpc::ListOwnedObjectsRequest::default()
        .with_owner(owner)
        .with_page_size(1000)
        .with_object_type(sui::types::StructTag::gas_coin())
        .with_read_mask(sui::grpc::FieldMask::from_paths([
            "object_id",
            "version",
            "digest",
        ]));

    let response = client
        .state_client()
        .list_owned_objects(request)
        .await
        .map(|resp| resp.into_inner())?;

    Ok(response
        .objects()
        .iter()
        .filter_map(|object| {
            Some(sui::types::ObjectReference::new(
                object.object_id_opt()?.parse().ok()?,
                object.version_opt()?,
                object.digest_opt()?.parse().ok()?,
            ))
        })
        .collect())
}
