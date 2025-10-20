use {
    crate::{
        command_title,
        display::json_output,
        item,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::{
        self,
        events::{NexusEvent, NexusEventKind},
        idents::primitives,
        types::Storable,
    },
    std::sync::Arc,
};

/// Inspect a Nexus DAG execution process based on the provided object ID and
/// execution digest.
pub(crate) async fn inspect_dag_execution(
    dag_execution_id: sui::ObjectID,
    execution_digest: sui::TransactionDigest,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Nexus DAG Execution '{dag_execution_id}'");

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Nexus objects must be present in the configuration.
    let primitives_pkg_id = {
        let NexusObjects {
            primitives_pkg_id, ..
        } = &get_nexus_objects(&mut conf).await?;
        *primitives_pkg_id // ObjectID is Copy
    };

    // Build Sui client.
    let sui_conf = conf.sui.clone();
    let sui = build_sui_client(&sui_conf).await?;

    // Remote storage conf.
    let storage_conf = conf.data_storage.clone().into();

    // Check if we have authentication for potential decryption and get the session
    let session = get_active_session().await?;

    let limit = None;
    let descending_order = false;

    // Starting cursor is the provided event digest and `event_seq` always 0.
    let mut cursor = Some(sui::EventID {
        tx_digest: execution_digest,
        event_seq: 0,
    });

    let mut json_trace = Vec::new();

    // Loop until we find an `ExecutionFinished` event.
    'query: loop {
        let query = sui::EventFilter::MoveEventModule {
            package: primitives_pkg_id,
            module: primitives::Event::EVENT_WRAPPER.module.into(),
        };

        let events = match sui
            .event_api()
            .query_events(query, cursor, limit, descending_order)
            .await
        {
            Ok(page) => {
                cursor = page.next_cursor;

                page.data
            }
            Err(_) => {
                // If RPC call fails, wait and retry.
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                continue;
            }
        };

        // Parse `SuiEvent` into `NexusEvent`.
        let events = events.into_iter().filter_map(|e| match e.try_into() {
            Ok(event) => Some::<NexusEvent>(event),
            Err(e) => {
                eprintln!("Failed to parse event: {:?}", e);
                None
            }
        });

        for event in events {
            match event.data {
                NexusEventKind::WalkAdvanced(e) if e.execution == dag_execution_id => {
                    notify_success!(
                        "Vertex '{vertex}' evaluated with output variant '{variant}'.",
                        vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                        variant = e.variant.name.truecolor(100, 100, 100),
                    );

                    let mut json_data = Vec::new();

                    let fetched_data = e
                        .variant_ports_to_data
                        .fetch_all(&storage_conf, Arc::clone(&session))
                        .await
                        .map_err(|e| NexusCliError::Any(anyhow!(
                            "Failed to fetch data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                            e = e,
                            command = "$ nexus conf set --data-storage.walrus-aggregator-url <URL>",
                            testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
                        )))?;

                    for (port, data) in fetched_data {
                        let (display_data, json_data_value) = (
                            format!("{}", data.as_json()),
                            json!({ "port": port.name, "data": data.as_json(), "was_encrypted": data.is_encrypted(), "storage": data.storage_kind() }),
                        );

                        item!(
                            "Port '{port}' produced data: {data}",
                            port = port.name.truecolor(100, 100, 100),
                            data = display_data.truecolor(100, 100, 100),
                        );

                        json_data.push(json_data_value);
                    }

                    json_trace.push(json!({
                        "end_state": false,
                        "vertex": e.vertex,
                        "variant": e.variant.name,
                        "data": json_data,
                    }));
                }

                NexusEventKind::EndStateReached(e) if e.execution == dag_execution_id => {
                    notify_success!(
                        "{end_state} Vertex '{vertex}' evaluated with output variant '{variant}'.",
                        vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                        variant = e.variant.name.truecolor(100, 100, 100),
                        end_state = "END STATE".truecolor(100, 100, 100)
                    );

                    let mut json_data = Vec::new();

                    let fetched_data = e
                        .variant_ports_to_data
                        .fetch_all(&storage_conf, Arc::clone(&session))
                        .await
                        .map_err(|e| NexusCliError::Any(anyhow!(
                            "Failed to fetch data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                            e = e,
                            command = "$ nexus conf set --data-storage.walrus-aggregator-url <URL>",
                            testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
                        )))?;

                    for (port, data) in fetched_data {
                        let (display_data, json_data_value) = (
                            format!("{}", data.as_json()),
                            json!({ "port": port.name, "data": data.as_json(), "was_encrypted": data.is_encrypted(), "storage": data.storage_kind() }),
                        );

                        item!(
                            "Port '{port}' produced data: {data}",
                            port = port.name.truecolor(100, 100, 100),
                            data = display_data.truecolor(100, 100, 100),
                        );

                        json_data.push(json_data_value);
                    }

                    json_trace.push(json!({
                        "end_state": true,
                        "vertex": e.vertex,
                        "variant": e.variant.name,
                        "data": json_data,
                    }));
                }

                NexusEventKind::ExecutionFinished(e) if e.execution == dag_execution_id => {
                    if e.has_any_walk_failed {
                        notify_error!("DAG execution finished unsuccessfully");

                        break 'query;
                    }

                    notify_success!("DAG execution finished successfully");

                    break 'query;
                }

                _ => {}
            }
        }
    }

    // TODO: should have like SessionBag.
    // Update the session in the configuration.
    let mut crypto_conf = CryptoConf::load().await.unwrap_or_default();
    let Ok(session) = Arc::try_unwrap(session) else {
        return Err(NexusCliError::Any(anyhow!(
            "Failed to unwrap session Arc for saving"
        )));
    };
    let session = session.into_inner();
    crypto_conf.sessions.insert(*session.id(), session);
    crypto_conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&json_trace)?;

    Ok(())
}
