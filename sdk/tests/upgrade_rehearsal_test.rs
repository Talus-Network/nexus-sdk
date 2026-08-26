#![cfg(feature = "upgrade_test")]

use {
    nexus_sdk::{
        nexus::{client::NexusClientBuilder, tool::ToolCompatibility},
        types::{NexusObjects, PackageRole},
    },
    serde_json::Value,
    std::{env, fs},
};

#[tokio::test]
#[ignore = "requires the physical Nexus package upgrade fixture"]
async fn physical_upgrade_obeys_live_object_authority() {
    let rpc_url = required_env("NEXUS_UPGRADE_RPC");
    let objects: NexusObjects = toml::from_str(
        &fs::read_to_string(required_env("NEXUS_UPGRADE_OBJECTS"))
            .expect("read stable Nexus environment"),
    )
    .expect("decode stable Nexus environment");
    let deployment: Value = serde_json::from_str(
        &fs::read_to_string(required_env("NEXUS_UPGRADE_DEPLOYMENT"))
            .expect("read operator deployment"),
    )
    .expect("decode operator deployment");
    let candidate: Value = serde_json::from_str(
        &fs::read_to_string(required_env("NEXUS_UPGRADE_CANDIDATE"))
            .expect("read package candidate"),
    )
    .expect("decode package candidate");
    let active = required_env("NEXUS_UPGRADE_ACTIVE") == "1";

    let client = NexusClientBuilder::new()
        .with_rpc_url(&rpc_url)
        .with_nexus_objects(objects.clone())
        .build()
        .await
        .expect("build read only Nexus client");

    for root in [
        objects.tool_registry,
        objects.network_auth,
        objects.agent_registry,
        objects.leader_registry,
        objects.priority_fee_vault,
    ] {
        let observed = client
            .state_resolver()
            .observe(root.object_id())
            .await
            .expect("observe canonical root state");
        assert_eq!(
            observed.witness_type().name().as_str(),
            if active { "V2" } else { "V1" }
        );
        if root == objects.agent_registry {
            assert_eq!(
                observed.inner_type().name().as_str(),
                if active {
                    "AgentRegistryInnerV2"
                } else {
                    "AgentRegistryInnerV1"
                }
            );
        }
    }

    for (role, roots) in [
        (PackageRole::Workflow, vec![objects.tool_registry]),
        (
            PackageRole::Scheduler,
            vec![objects.agent_registry, objects.leader_registry],
        ),
    ] {
        let name = role.as_str();
        let old_package = package_id(&deployment, name);
        let new_package = package_id(&candidate, name);
        let old = client.context_for_creator(old_package, role, &roots).await;
        let new = client.context_for_creator(new_package, role, &roots).await;
        if active {
            assert!(old.is_err(), "old {name} creator remained compatible");
            assert!(new.is_ok(), "new {name} creator was rejected: {new:?}");
        } else {
            assert!(old.is_ok(), "old {name} creator was rejected: {old:?}");
            assert!(
                new.is_err(),
                "new {name} creator became active before roots"
            );
        }
    }

    let legacy_fqn = required_env("NEXUS_UPGRADE_LEGACY_TOOL_FQN");
    let migrated_fqn = required_env("NEXUS_UPGRADE_MIGRATED_TOOL_FQN");
    let inventory = client
        .tool()
        .list_tools()
        .await
        .expect("list every Tool independently");
    let legacy = inventory
        .iter()
        .find(|tool| tool.fqn.to_string() == legacy_fqn)
        .expect("legacy Tool remains visible");
    let migrated = inventory
        .iter()
        .find(|tool| tool.fqn.to_string() == migrated_fqn)
        .expect("migrated Tool remains visible");
    let expected_legacy = if active {
        ToolCompatibility::MigrationRequired
    } else {
        ToolCompatibility::Current
    };
    assert_eq!(legacy.compatibility, expected_legacy);
    assert_eq!(migrated.compatibility, ToolCompatibility::Current);

    if active {
        for variable in [
            "NEXUS_UPGRADE_WORKFLOW_CREATED",
            "NEXUS_UPGRADE_SCHEDULER_CREATED",
        ] {
            let object_id = required_env(variable)
                .parse()
                .unwrap_or_else(|error| panic!("{variable} is invalid: {error}"));
            let observed = client
                .state_resolver()
                .observe(object_id)
                .await
                .unwrap_or_else(|error| panic!("observe {variable}: {error}"));
            assert_eq!(observed.anchor_type.name().as_str(), "CreatedV2");
            assert_eq!(observed.witness_type().name().as_str(), "V2");
            assert_eq!(observed.inner_type().name().as_str(), "CreatedV2InnerV1");
        }
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}

fn package_id(document: &Value, package: &str) -> nexus_sdk::sui::types::Address {
    document["packages"][package]["storage_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{package} package ID is missing"))
        .parse()
        .unwrap_or_else(|error| panic!("{package} package ID is invalid: {error}"))
}
