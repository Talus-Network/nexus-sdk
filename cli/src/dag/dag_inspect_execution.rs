use {
    crate::{command_title, prelude::*, sui::*},
    nexus_types::{
        events::{NexusEvent, NexusEventKind},
        idents::primitives,
        types::{NexusData, TypeName},
    },
    std::collections::HashMap,
};

/// Inspect a Nexus DAG execution process based on the provided object ID and
/// execution digest.
pub(crate) async fn inspect_dag_execution(
    dag_execution_id: sui::ObjectID,
    execution_digest: sui::TransactionDigest,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Nexus DAG Execution '{dag_execution_id}'");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        primitives_pkg_id, ..
    } = get_nexus_objects(&conf)?;

    // Build Sui client.
    let sui = build_sui_client(conf.sui.net).await?;

    let limit = None;
    let descending_order = false;

    // Starting cursor is the provided event digest and `event_seq` always 0.
    let mut cursor = Some(sui::EventID {
        tx_digest: execution_digest,
        event_seq: 0,
    });

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
                    println!(
                        "[{check}] Vertex '{vertex}' evaluated with output variant '{variant}'.",
                        check = "✔".green().bold(),
                        vertex = e.vertex.name.truecolor(100, 100, 100),
                        variant = e.variant.name.truecolor(100, 100, 100),
                    );

                    let Ok(variant_ports_to_data) =
                        serde_json::from_value::<PortsToData>(e.variant_ports_to_data.clone())
                    else {
                        println!(
                            "    {arrow} With data: {data}",
                            arrow = "▶".truecolor(100, 100, 100),
                            data =
                                format!("{:?}", e.variant_ports_to_data).truecolor(100, 100, 100),
                        );

                        continue;
                    };

                    for (port, data) in variant_ports_to_data.values {
                        println!(
                            "    {arrow} Port '{port}' produced data: {data}",
                            arrow = "▶".truecolor(100, 100, 100),
                            port = port.name.truecolor(100, 100, 100),
                            data = format!("{data:?}").truecolor(100, 100, 100),
                        );
                    }

                    println!()
                }

                NexusEventKind::EndStateReached(e) if e.execution == dag_execution_id => {
                    println!(
                        "[{check}] {end_state} Vertex '{vertex}' evaluated with output variant '{variant}'.",
                        check = "✔".green().bold(),
                        vertex = e.vertex.name.truecolor(100, 100, 100),
                        variant = e.variant.name.truecolor(100, 100, 100),
                        end_state = "END STATE".truecolor(100, 100, 100)
                    );

                    let Ok(variant_ports_to_data) =
                        serde_json::from_value::<PortsToData>(e.variant_ports_to_data.clone())
                    else {
                        println!(
                            "    {arrow} With data: {data}",
                            arrow = "▶".truecolor(100, 100, 100),
                            data =
                                format!("{:?}", e.variant_ports_to_data).truecolor(100, 100, 100),
                        );

                        continue;
                    };

                    for (port, data) in variant_ports_to_data.values {
                        println!(
                            "    {arrow} Port '{port}' produced data: {data}",
                            arrow = "▶".truecolor(100, 100, 100),
                            port = port.name.truecolor(100, 100, 100),
                            data = format!("{data:?}").truecolor(100, 100, 100),
                        );
                    }

                    println!()
                }

                NexusEventKind::ExecutionFinished(e) if e.execution == dag_execution_id => {
                    if e.has_any_walk_failed {
                        println!(
                            "[{ballot}] DAG execution finished unsuccessfully",
                            ballot = "✘".red().bold()
                        );

                        break 'query;
                    }

                    println!(
                        "[{check}] DAG execution finished successfully",
                        check = "✔".green().bold()
                    );

                    break 'query;
                }

                _ => {}
            }
        }
    }

    Ok(())
}

/// Struct defining deser of the `variant_ports_to_data` field in the
/// `WalkAdvanced` and `EndStateReached` events.
// TODO: This can be later improved by making some bigger changes to the object
// crawler and porting it to the Nexus SDK.
#[derive(Clone, Debug)]
struct PortsToData {
    values: HashMap<TypeName, NexusData>,
}

impl<'de> serde::Deserialize<'de> for PortsToData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct VecMapWrapper {
            contents: Vec<VecMapEntry>,
        }

        #[derive(Deserialize)]
        struct VecMapEntry {
            key: TypeName,
            value: NexusData,
        }

        let values = VecMapWrapper::deserialize(deserializer)?;

        Ok(PortsToData {
            values: values
                .contents
                .into_iter()
                .map(|entry| (entry.key, entry.value))
                .collect(),
        })
    }
}
