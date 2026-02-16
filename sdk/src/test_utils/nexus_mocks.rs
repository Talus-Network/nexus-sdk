use crate::{nexus::client::NexusClient, sui, test_utils::sui_mocks, types::NexusObjects};

/// Create a mock [`NexusClient`] that is connected to a mock RPC using [`mockito`].
pub async fn mock_nexus_client(
    nexus_objects: &NexusObjects,
    rpc_url: &str,
    gql_url: Option<&str>,
) -> NexusClient {
    let mut rng = rand::thread_rng();
    let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);

    let coin = sui_mocks::mock_sui_object_ref();

    let builder = if let Some(gql_url) = gql_url {
        NexusClient::builder().with_gql_url(gql_url)
    } else {
        NexusClient::builder()
    };

    builder
        .with_private_key(pk)
        .with_rpc_url(rpc_url)
        .with_nexus_objects(nexus_objects.clone())
        .with_gas(vec![coin], 1000)
        .build()
        .await
        .expect("Failed to build NexusClient")
}
