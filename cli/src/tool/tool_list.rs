use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::{
        move_bindings::{
            move_std::ascii::String as MoveAsciiString,
            registry::tool_registry::ToolRegistry,
            sui_framework::linked_table::Node as LinkedTableNode,
        },
        types::Tool,
    },
    prettytable::{row, Table},
};

/// List tools available in the tool registry.
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

    notify_success!("Successfully fetched {} tools", tools.len());

    let mut tools_json = Vec::new();

    let mut table = Table::new();

    table.add_row(row![
        "FQN",
        "Reference",
        "Timeout",
        "Registered At",
        "Unregistered At"
    ]);

    for tool in tools {
        let fqn = tool.fqn_string().map_err(NexusCliError::Any)?;
        let registered_at = tool.registered_at_datetime().map_err(NexusCliError::Any)?;
        let unregistered_at = tool
            .unregistered_at_datetime()
            .map_err(NexusCliError::Any)?;
        let timeout = timeouts
            .get(&fqn)
            .map(|timeout| format!("{timeout} ms"))
            .unwrap_or_else(|| "N/A".to_string());

        tools_json.push(json!(tool));

        table.add_row(row![
            fqn,
            tool.r#ref.to_string(),
            timeout,
            registered_at.to_string(),
            unregistered_at.map_or("N/A".to_string(), |t| t.to_string())
        ]);
    }

    if !JSON_MODE.load(Ordering::Relaxed) {
        table.printstd();
    }

    json_output(&tools_json)?;

    Ok(())
}

async fn fetch_tools_with_client(
    nexus_client: &nexus_sdk::nexus::client::NexusClient,
) -> AnyResult<(HashMap<String, u64>, Vec<Tool>), NexusCliError> {
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let crawler = nexus_client.crawler();
    let tool_registry = crawler
        .get_object::<ToolRegistry>(*nexus_objects.tool_registry.object_id())
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

    let tools = crawler
        .get_objects::<Tool>(&tool_ids)
        .await
        .map_err(NexusCliError::Any)?
        .into_iter()
        .map(|response| response.data)
        .collect();

    Ok((timeouts, tools))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            move_bindings::{
                interface::verifier::ToolVerifierSupport,
                sui_framework::{
                    linked_table::LinkedTable,
                    object::{ID, UID},
                    table::Table as MoveTable,
                },
            },
            test_utils::{nexus_mocks, sui_mocks},
        },
    };

    #[tokio::test]
    async fn fetch_tools_succeeds_without_owned_coins() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let registry_id = *nexus_objects.tool_registry.object_id();
        let registry = ToolRegistry::new(
            UID::new(registry_id),
            LinkedTable::<MoveAsciiString, ID>::new(sui::types::Address::from_static("0x101"), 0),
            MoveTable::<ID, bool>::new(sui::types::Address::from_static("0x102"), 0),
            LinkedTable::<MoveAsciiString, u64>::new(sui::types::Address::from_static("0x103"), 0),
            MoveTable::<ID, ToolVerifierSupport>::new(sui::types::Address::from_static("0x104"), 0),
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
