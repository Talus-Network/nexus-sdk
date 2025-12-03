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
    nexus_sdk::{events::NexusEventKind, sui, types::Storable},
    std::sync::Arc,
};

/// Inspect a Nexus DAG execution process based on the provided object ID and
/// execution digest.
pub(crate) async fn inspect_dag_execution(
    dag_execution_id: sui::types::Address,
    execution_digest: sui::types::Digest,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Nexus DAG Execution '{dag_execution_id}'");

    let (nexus_client, _) = get_nexus_client(None, sui::MIST_PER_SUI / 10).await?;

    let mut result = nexus_client
        .workflow()
        .inspect_execution(dag_execution_id, execution_digest, None)
        .await
        .map_err(NexusCliError::Nexus)?;

    // Remote storage conf.
    let conf = CliConf::load().await.unwrap_or_default();
    let storage_conf = conf.data_storage.clone().into();

    // Get the active session for potential encryption
    let session = CryptoConf::get_active_session(None).await.map_err(|e|
        NexusCliError::Any(
            anyhow!(
                "Failed to get active session: {}.\nPlease initiate a session first.\n\n{init_key}\n{crypto_auth}",
                e,
                init_key = "$ nexus crypto init-key --force",
                crypto_auth = "$ nexus crypto auth"
            )
        )
    )?;

    let mut json_trace = Vec::new();

    while let Some(event) = result.next_event.recv().await {
        match event.data {
            NexusEventKind::WalkAdvanced(e) => {
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
                        json!({ "port": port, "data": data.as_json(), "was_encrypted": data.is_encrypted(), "storage": data.storage_kind() }),
                    );

                    item!(
                        "Port '{port}' produced data: {data}",
                        port = port.truecolor(100, 100, 100),
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

            NexusEventKind::EndStateReached(e) => {
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
                        json!({ "port": port, "data": data.as_json(), "was_encrypted": data.is_encrypted(), "storage": data.storage_kind() }),
                    );

                    item!(
                        "Port '{port}' produced data: {data}",
                        port = port.truecolor(100, 100, 100),
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

            NexusEventKind::WalkFailed(e) => {
                notify_error!(
                    "Vertex '{vertex}' failed to execute: '{reason}'.",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                    reason = e.reason.truecolor(100, 100, 100),
                );
            }

            NexusEventKind::ExecutionFinished(e) => {
                if e.has_any_walk_failed {
                    notify_error!("DAG execution finished unsuccessfully");

                    break;
                }

                notify_success!("DAG execution finished successfully");

                break;
            }

            _ => {}
        }
    }

    // Update the session in the configuration.
    CryptoConf::release_session(session, None)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!("Failed to release session: {}", e)))?;

    json_output(&json_trace)?;

    Ok(())
}
