use {
    crate::{
        command_title,
        dag::dag_inspect_execution::inspect_dag_execution,
        loading,
        prelude::*,
        sui::*,
    },
    nexus_types::idents::{primitives, sui_framework, workflow},
};

/// Execute a Nexus DAG based on the provided object ID and initial input data.
pub(crate) async fn execute_dag(
    dag_id: sui::ObjectID,
    entry_group: String,
    input_json: serde_json::Value,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
    inspect: bool,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        workflow_pkg_id,
        primitives_pkg_id,
        default_sap_object_id,
        network_id,
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

    // Fetch DAG object for its ObjectRef.
    let dag = fetch_object_by_id(&sui, dag_id).await?;

    // Fetch DefaultSAP object for its ObjectRef.
    let default_sap = fetch_object_by_id(&sui, default_sap_object_id).await?;

    // Craft a TX to publish the DAG.
    let tx_handle = loading!("Crafting transaction...");

    let tx = match prepare_transaction(
        default_sap,
        dag,
        entry_group,
        input_json,
        workflow_pkg_id,
        primitives_pkg_id,
        network_id,
    ) {
        Ok(tx) => tx,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign and send the TX.
    let response = sign_transaction(&sui, &wallet, tx_data).await?;

    // We need to parse the DAGExecution object ID from the response.
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
                && object_type.module == workflow::Dag::DAG_EXECUTION.module.into()
                && object_type.name == workflow::Dag::DAG_EXECUTION.name.into() =>
            {
                Some(object_id)
            }
            _ => None,
        });

    let Some(object_id) = dag else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the DAGExecution object ID in the transaction response."
        )));
    };

    println!(
        "[{check}] DAGExecution object ID: {id}",
        check = "✔".green().bold(),
        id = object_id.to_string().truecolor(100, 100, 100)
    );

    if inspect {
        inspect_dag_execution(object_id, response.digest).await?;
    }

    Ok(())
}

/// Build a programmable transaction to execute a DAG.
fn prepare_transaction(
    default_sap: sui::ObjectRef,
    dag: sui::ObjectRef,
    entry_group: String,
    input_json: serde_json::Value,
    workflow_pkg_id: sui::ObjectID,
    primitives_pkg_id: sui::ObjectID,
    network_id: sui::ObjectID,
) -> AnyResult<sui::ProgrammableTransactionBuilder> {
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // `self: &mut DefaultSAP`
    let default_sap = tx.obj(sui::ObjectArg::SharedObject {
        id: default_sap.object_id,
        initial_shared_version: default_sap.version,
        mutable: true,
    })?;

    // `dag: &DAG`
    let dag = tx.obj(sui::ObjectArg::SharedObject {
        id: dag.object_id,
        initial_shared_version: dag.version,
        mutable: false,
    })?;

    // `network: ID`
    let network = sui_framework::Object::id_from_object_id(&mut tx, network_id)?;

    // `entry_group: EntryGroup`
    let entry_group = workflow::Dag::entry_group_from_str(&mut tx, workflow_pkg_id, entry_group)?;

    // `with_vertex_inputs: VecMap<Vertex, VecMap<InputPort, NexusData>>`
    let inner_vec_map_type = vec![
        workflow::into_type_tag(workflow_pkg_id, workflow::Dag::INPUT_PORT),
        primitives::into_type_tag(primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        workflow::into_type_tag(workflow_pkg_id, workflow::Dag::VERTEX),
        sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::FRAMEWORK_PACKAGE_ID,
            module: sui_framework::VecMap::VEC_MAP.module.into(),
            name: sui_framework::VecMap::VEC_MAP.name.into(),
            type_params: inner_vec_map_type.clone(),
        })),
    ];

    let with_vertex_inputs = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::VecMap::EMPTY.module.into(),
        sui_framework::VecMap::EMPTY.name.into(),
        outer_vec_map_type.clone(),
        vec![],
    );

    let Some(data) = input_json.as_object() else {
        bail!(
            "Input JSON must be an object containing the entry vertices and their respective data."
        );
    };

    for (vertex, data) in data {
        let Some(data) = data.as_object() else {
            bail!(
                "Values of input JSON must be an object containing the input ports and their respective values."
            );
        };

        // `vertex: Vertex`
        let vertex = workflow::Dag::vertex_from_str(&mut tx, workflow_pkg_id, vertex)?;

        // `with_vertex_input: VecMap<InputPort, NexusData>`
        let with_vertex_input = tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module.into(),
            sui_framework::VecMap::EMPTY.name.into(),
            inner_vec_map_type.clone(),
            vec![],
        );

        for (port, value) in data {
            // `port: InputPort`
            let port = workflow::Dag::input_port_from_str(&mut tx, workflow_pkg_id, port)?;

            // `value: NexusData`
            let value = primitives::Data::nexus_data_from_json(&mut tx, primitives_pkg_id, value)?;

            // `with_vertex_input.insert(port, value)`
            tx.programmable_move_call(
                sui::FRAMEWORK_PACKAGE_ID,
                sui_framework::VecMap::INSERT.module.into(),
                sui_framework::VecMap::INSERT.name.into(),
                inner_vec_map_type.clone(),
                vec![with_vertex_input, port, value],
            );
        }

        // `with_vertex_inputs.insert(vertex, with_vertex_input)`
        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::INSERT.module.into(),
            sui_framework::VecMap::INSERT.name.into(),
            outer_vec_map_type.clone(),
            vec![with_vertex_inputs, vertex, with_vertex_input],
        );
    }

    // `workflow::default_sap::begin_dag_execution()`
    tx.programmable_move_call(
        workflow_pkg_id,
        workflow::DefaultSap::BEGIN_DAG_EXECUTION.module.into(),
        workflow::DefaultSap::BEGIN_DAG_EXECUTION.name.into(),
        vec![],
        vec![default_sap, dag, network, entry_group, with_vertex_inputs],
    );

    Ok(tx)
}
