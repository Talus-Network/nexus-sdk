use {
    crate::{nexus::client::NexusClient, sui, test_utils::sui_mocks},
    mockito::{Server, ServerGuard},
};

/// Create a mock [`NexusClient`] that is connected to a mock RPC using [`mockito`].
pub async fn mock_nexus_client() -> (ServerGuard, NexusClient) {
    let mut server = Server::new_async().await;
    let server_url = server.url();

    let (_, mnemonic) = sui_mocks::mock_sui_mnemonic();
    let nexus_objects = sui_mocks::mock_nexus_objects();
    let coin = sui_mocks::mock_sui_coin(1000);

    let mock = sui_mocks::rpc::mock_rpc_discover(&mut server);

    let sui_client = sui::ClientBuilder::default()
        .build(server_url)
        .await
        .expect("Failed to build Sui client");

    mock.assert_async().await;

    let mock = sui_mocks::rpc::mock_reference_gas_price(&mut server, "1000".to_string());

    let client = NexusClient::builder()
        .with_nexus_objects(nexus_objects)
        .with_gas(vec![&coin], 1000)
        .expect("Failed to setup gas")
        .with_mnemonic(sui_client, &mnemonic, sui::SignatureScheme::ED25519)
        .expect("Failed to import mnemonic")
        .build()
        .await
        .expect("Failed to build NexusClient");

    mock.assert_async().await;

    (server, client)
}
