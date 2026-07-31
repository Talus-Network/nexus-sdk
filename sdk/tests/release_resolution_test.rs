use {
    nexus_sdk::{
        nexus::{client::NexusClient, release::ReleaseExtras},
        types::NexusObjects,
    },
    std::{env, fs},
};

/// Resolves an actual fresh Nexus publication through its stable protocol root.
///
/// Run with:
///
/// ```text
/// NEXUS_LOCAL_RELEASE_OBJECTS=/path/to/objects.localnet.toml \
/// NEXUS_LOCAL_SUI_RPC=http://127.0.0.1:9000 \
/// cargo test -p nexus-sdk --all-features --test release_resolution_test -- --ignored
/// ```
#[tokio::test]
#[ignore = "requires a published local Nexus release"]
async fn resolves_active_snapshot_from_fresh_publication() {
    let objects_path =
        env::var("NEXUS_LOCAL_RELEASE_OBJECTS").expect("NEXUS_LOCAL_RELEASE_OBJECTS is required");
    let rpc =
        env::var("NEXUS_LOCAL_SUI_RPC").unwrap_or_else(|_| "http://127.0.0.1:9000".to_owned());
    let configured: NexusObjects = toml::from_str(
        &fs::read_to_string(&objects_path)
            .unwrap_or_else(|error| panic!("could not read '{objects_path}': {error}")),
    )
    .unwrap_or_else(|error| panic!("could not parse '{objects_path}': {error}"));

    let client = NexusClient::builder()
        .with_rpc_url(&rpc)
        .with_protocol(configured.protocol.clone())
        .with_release_extras(ReleaseExtras::from(&configured))
        .build()
        .await
        .expect("active release should resolve");
    let resolved = client.get_nexus_objects();

    assert_eq!(resolved.release, configured.release);
    assert_eq!(
        resolved.protocol.object_id(),
        configured.protocol.object_id()
    );
    for (resolved_package, configured_package) in resolved
        .packages
        .all()
        .into_iter()
        .zip(configured.packages.all())
    {
        assert_eq!(resolved_package.initial_id, configured_package.initial_id);
        assert_eq!(resolved_package.storage_id, configured_package.storage_id);
        assert!(resolved_package.version > 0);
        assert!(!resolved_package.type_origins.is_empty());
    }
    assert_eq!(resolved.manifest_hash.len(), 32);
    assert_eq!(
        resolved.tool_registry.object_id(),
        configured.tool_registry.object_id()
    );
    assert_eq!(
        resolved.agent_registry.object_id(),
        configured.agent_registry.object_id()
    );
    assert_eq!(
        resolved.gas_service.object_id(),
        configured.gas_service.object_id()
    );
    assert_eq!(
        resolved.leader_registry.object_id(),
        configured.leader_registry.object_id()
    );
}
