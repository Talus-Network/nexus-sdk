use {
    super::tool_output::ToolOutput,
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::{
        move_bindings::{
            move_std::ascii::String as MoveAsciiString,
            sui_framework::linked_table::Node as LinkedTableNode,
            tool::tool_registry::{ToolRegistry, ToolRegistryStateV2},
        },
        types::{Tool, ToolAnchor, ToolStateV2},
    },
    prettytable::{row, Table},
};

/// List tools available in the tool registry.
/// TODO: Provide a search based solution or move this functions to nexus API
pub(crate) async fn list_tools() -> AnyResult<(), NexusCliError> {
    command_title!("Listing all available Nexus tools");

    let nexus_client = get_read_only_nexus_client().await?;
    let tools_handle = loading!("Fetching tools from the tool registry...");

    let (timeouts, tools) = match fetch_tools_with_client(&nexus_client).await {
        Ok(result) => result,
        Err(e) => {
            tools_handle.error();

            return Err(e);
        }
    };

    tools_handle.success();

    let tools = tool_list_output(&timeouts, &tools)?;

    notify_success!("Successfully fetched {} tools", tools.len());

    let mut table = Table::new();

    table.add_row(row![
        "FQN",
        "Reference",
        "Timeout",
        "Registered At",
        "Unregistered At"
    ]);

    for tool in &tools {
        let timeout = tool
            .timeout_ms
            .map(|timeout| format!("{timeout} ms"))
            .unwrap_or_else(|| "N/A".to_string());

        table.add_row(row![
            tool.fqn,
            tool.reference.to_string(),
            timeout,
            tool.registered_at.to_string(),
            tool.unregistered_at
                .as_ref()
                .map_or_else(|| "N/A".to_string(), ToString::to_string)
        ]);
    }

    if !JSON_MODE.load(Ordering::Relaxed) {
        table.printstd();
    }

    json_output(&tools)?;

    Ok(())
}

fn tool_list_output(
    timeouts: &HashMap<String, u64>,
    tools: &[ToolStateV2],
) -> AnyResult<Vec<ToolOutput>, NexusCliError> {
    tools
        .iter()
        .map(|tool| {
            let fqn = tool.fqn_string().map_err(NexusCliError::Any)?;
            ToolOutput::try_from_state(tool, timeouts.get(&fqn).copied())
                .map_err(NexusCliError::Any)
        })
        .collect()
}

async fn fetch_tools_with_client(
    nexus_client: &nexus_sdk::nexus::client::NexusClient,
) -> AnyResult<(HashMap<String, u64>, Vec<ToolStateV2>), NexusCliError> {
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let crawler = nexus_client.crawler();
    let tool_registry = crawler
        .get_versioned_object::<ToolRegistry, ToolRegistryStateV2>(
            *nexus_objects.tool_registry.object_id(),
            2,
        )
        .await
        .map_err(NexusCliError::Any)?
        .data;

    let timeouts = crawler
        .get_dynamic_fields::<MoveAsciiString, LinkedTableNode<MoveAsciiString, u64>>(
            tool_registry.timeouts.id(),
            tool_registry.timeouts.size(),
        )
        .await
        .map_err(NexusCliError::Any)?
        .into_iter()
        .map(|(key, node)| (key.into_string(), node.value))
        .collect::<HashMap<_, _>>();

    let tool_ids = timeouts
        .keys()
        .filter_map(|fqn| {
            Tool::derive_id(*nexus_objects.tool_registry.object_id(), &fqn.parse().ok()?).ok()
        })
        .collect::<Vec<_>>();

    let mut tools = Vec::with_capacity(tool_ids.len());
    for tool_id in tool_ids {
        tools.push(
            crawler
                .get_versioned_object::<ToolAnchor, ToolStateV2>(tool_id, 2)
                .await
                .map_err(NexusCliError::Any)?
                .data,
        );
    }

    Ok((timeouts, tools))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            move_bindings::{
                interface::{meta_schema::MetaSchema, verifier::ToolVerifierSupport},
                sui_framework::{
                    balance::Balance,
                    linked_table::LinkedTable,
                    object::{ID, UID},
                    table::Table as MoveTable,
                    versioned::Versioned,
                },
                tool::external_verifier::ExternalVerifier,
            },
            test_utils::{nexus_mocks, sui_mocks},
            types::ToolRef,
        },
    };

    fn fixture_tool() -> ToolStateV2 {
        ToolStateV2 {
            minimum_protocol_version: 1,
            registry: ID::new(sui::types::Address::from_static("0x42")),
            fqn: MoveAsciiString::from("xyz.taluslabs.example@1"),
            r#ref: ToolRef::Http {
                url: b"https://example.com/tool".to_vec(),
            },
            description: b"Example tool".to_vec(),
            meta_schema: MetaSchema::new(vec![], vec![]),
            verified: true,
            vault: Balance {
                value: 25,
                phantom_t0: std::marker::PhantomData,
            },
            workflow_authorization_cap_first: false,
            lock_duration_ms: 5_000,
            registered_at_ms: 0,
            unregistered_at_ms: nexus_sdk::move_bindings::move_std::option::Option::from(None),
        }
    }

    #[test]
    fn tool_list_output_uses_the_semantic_tool_contract() {
        let tools = tool_list_output(
            &HashMap::from([("xyz.taluslabs.example@1".to_owned(), 10_000)]),
            &[fixture_tool()],
        )
        .expect("valid tools should project");
        let value = serde_json::to_value(tools).expect("tool list should serialize");

        assert_eq!(value[0]["fqn"], "xyz.taluslabs.example@1");
        assert_eq!(value[0]["timeout_ms"], 10_000);
        assert_eq!(value[0]["reference"]["url"], "https://example.com/tool");
        assert!(!value.to_string().contains("\"bytes\""));
    }

    #[tokio::test]
    async fn fetch_tools_succeeds_without_owned_coins() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let registry_id = *nexus_objects.tool_registry.object_id();
        let state_id = sui::types::Address::from_static("0x107");
        let registry =
            ToolRegistry::new(UID::new(registry_id), Versioned::new(UID::new(state_id), 2));
        let state = ToolRegistryStateV2::new(
            ID::new(*nexus_objects.protocol.object_id()),
            nexus_objects.protocol_version,
            LinkedTable::<MoveAsciiString, ID>::new(sui::types::Address::from_static("0x101"), 0),
            MoveTable::<ID, bool>::new(sui::types::Address::from_static("0x102"), 0),
            MoveTable::<ID, MetaSchema>::new(sui::types::Address::from_static("0x110"), 0),
            LinkedTable::<MoveAsciiString, u64>::new(sui::types::Address::from_static("0x103"), 0),
            MoveTable::<ID, ToolVerifierSupport>::new(sui::types::Address::from_static("0x104"), 0),
            MoveTable::<ID, ExternalVerifier>::new(sui::types::Address::from_static("0x108"), 0),
            MoveTable::<MoveAsciiString, u64>::new(sui::types::Address::from_static("0x109"), 0),
            LinkedTable::<MoveAsciiString, ID>::new(sui::types::Address::from_static("0x105"), 0),
            LinkedTable::<MoveAsciiString, bool>::new(sui::types::Address::from_static("0x106"), 0),
            0,
            0,
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_get_object_value_bcs_for(
            &mut ledger_service_mock,
            nexus_objects.tool_registry.clone(),
            sui::types::Owner::Shared(1),
            &registry,
            nexus_sdk::move_bindings::struct_tag::<ToolRegistry>(&nexus_objects),
        );
        sui_mocks::grpc::mock_versioned_payload(&mut ledger_service_mock, state_id, 2, state);
        sui_mocks::grpc::mock_empty_dynamic_fields(&mut state_service_mock, 1);
        sui_mocks::grpc::mock_empty_batch_get_objects(&mut ledger_service_mock, 2);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&nexus_objects, &rpc_url).await;

        let (timeouts, tools) = fetch_tools_with_client(&client)
            .await
            .expect("tool registry read should not require owned coins");

        assert!(timeouts.is_empty());
        assert!(tools.is_empty());
    }
}
