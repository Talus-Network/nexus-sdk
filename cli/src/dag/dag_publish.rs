use {
    crate::{
        command_title,
        dag::dag_validate::validate_dag,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::{idents::workflow, transactions::dag, types::DEFAULT_ENTRY_GROUP},
};

/// Publish the provided Nexus DAG to the currently active Sui net. This also
/// performs validation on the DAG before publishing.
pub(crate) async fn publish_dag(
    path: PathBuf,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let dag = validate_dag(path).await?;

    command_title!("Publishing Nexus DAG");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        workflow_pkg_id,
        primitives_pkg_id,
        ..
    } = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(conf.sui.net).await?;

    let address = match wallet.active_address() {
        Ok(address) => address,
        Err(e) => {
            return Err(NexusCliError::Any(e));
        }
    };

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, conf.sui.net, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Craft a TX to publish the DAG.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // Create an empty DAG.
    let mut dag_arg = dag::empty(&mut tx, workflow_pkg_id);

    // Create all entry vertices.
    for entry_vertex in dag.entry_vertices {
        // Find which entry groups this vertex belongs to.
        let entry_groups = dag
            .entry_groups
            .as_ref()
            .map(|groups| {
                groups
                    .iter()
                    .filter(|group| group.vertices.contains(&entry_vertex.name))
                    .map(|group| group.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![DEFAULT_ENTRY_GROUP.to_string()]);

        dag_arg = match dag::create_entry_vertex(
            &mut tx,
            workflow_pkg_id,
            dag_arg,
            entry_vertex,
            entry_groups,
        ) {
            Ok(dag_arg) => dag_arg,
            Err(e) => {
                tx_handle.error();

                return Err(NexusCliError::Any(e));
            }
        }
    }

    // Create all vertices.
    for vertex in dag.vertices {
        dag_arg = match dag::create_vertex(&mut tx, workflow_pkg_id, dag_arg, &vertex) {
            Ok(dag_arg) => dag_arg,
            Err(e) => {
                tx_handle.error();

                return Err(NexusCliError::Any(e));
            }
        }
    }

    // Create all default values if present.
    if let Some(default_values) = dag.default_values {
        for default_value in default_values {
            dag_arg = match dag::create_default_value(
                &mut tx,
                workflow_pkg_id,
                primitives_pkg_id,
                dag_arg,
                &default_value,
            ) {
                Ok(dag_arg) => dag_arg,
                Err(e) => {
                    tx_handle.error();

                    return Err(NexusCliError::Any(e));
                }
            }
        }
    }

    // Create all edges.
    for edge in dag.edges {
        dag_arg = match dag::create_edge(&mut tx, workflow_pkg_id, dag_arg, &edge) {
            Ok(dag_arg) => dag_arg,
            Err(e) => {
                tx_handle.error();

                return Err(NexusCliError::Any(e));
            }
        }
    }

    // Public share the DAG, locking it.
    dag::publish(&mut tx, workflow_pkg_id, dag_arg);

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign the transaction and send it to the network.
    let response = sign_transaction(&sui, &wallet, tx_data).await?;

    // We need to parse the DAG object ID from the response.
    let dag = response
        .object_changes
        .unwrap_or_default()
        .into_iter()
        .find_map(|change| match change {
            sui::ObjectChange::Created {
                object_type,
                object_id,
                ..
            } if object_type.address == *workflow_pkg_id
                && object_type.module == workflow::Dag::DAG.module.into()
                && object_type.name == workflow::Dag::DAG.name.into() =>
            {
                Some(object_id)
            }
            _ => None,
        });

    let Some(object_id) = dag else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the DAG object ID in the transaction response."
        )));
    };

    notify_success!(
        "Published DAG with object ID: {id}",
        id = object_id.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({ "digest": response.digest, "dag_id": object_id }))?;

    Ok(())
}
