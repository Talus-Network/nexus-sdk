use {
    crate::{
        crypto::{
            session::{Message, Session},
            x3dh::{IdentityKey, PreKeyBundle},
        },
        nexus::client::NexusClient,
        sui,
        test_utils::sui_mocks,
    },
    mockito::{Server, ServerGuard},
    std::sync::Arc,
    tokio::sync::Mutex,
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

/// Create a mock [`Session`] for testing encryption.
pub fn mock_session() -> (Arc<Mutex<Session>>, Arc<Mutex<Session>>) {
    let sender_id = IdentityKey::generate();
    let receiver_id = IdentityKey::generate();
    let spk_secret = IdentityKey::generate().secret().clone();
    let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

    let (message, mut sender_sess) =
        Session::initiate(&sender_id, &bundle, b"test").expect("Failed to initiate session");

    let initial_msg = match message {
        Message::Initial(msg) => msg,
        _ => panic!("Expected Initial message type"),
    };

    let (mut receiver_sess, _) =
        Session::recv(&receiver_id, &spk_secret, &bundle, &initial_msg, None)
            .expect("Failed to receive session");

    // Exchange messages to establish the ratchet properly
    let setup_msg = sender_sess
        .encrypt(b"setup")
        .expect("Failed to encrypt setup message");
    let _ = receiver_sess
        .decrypt(&setup_msg)
        .expect("Failed to decrypt setup message");

    (
        Arc::new(Mutex::new(sender_sess)),
        Arc::new(Mutex::new(receiver_sess)),
    )
}
