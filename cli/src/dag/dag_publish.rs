use {
    crate::{
        command_title,
        dag::{
            dag_validate::validate_dag,
            parser::{
                Data,
                DefaultValue,
                Edge,
                EntryVertex,
                Vertex,
                VertexKind,
                DEFAULT_ENTRY_GROUP,
            },
        },
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::idents::{move_std, primitives, sui_framework, workflow},
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
    let mut dag_arg = tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::NEW.module.into(),
        workflow::Dag::NEW.name.into(),
        vec![],
        vec![],
    );

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

        dag_arg = match create_entry_vertex(
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
        dag_arg = match create_vertex(&mut tx, workflow_pkg_id, dag_arg, &vertex) {
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
            dag_arg = match create_default_value(
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
        dag_arg = match create_edge(&mut tx, workflow_pkg_id, dag_arg, &edge) {
            Ok(dag_arg) => dag_arg,
            Err(e) => {
                tx_handle.error();

                return Err(NexusCliError::Any(e));
            }
        }
    }

    // Public share the DAG, locking it.
    let dag_type = workflow::into_type_tag(workflow_pkg_id, workflow::Dag::DAG);

    tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module.into(),
        sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name.into(),
        vec![dag_type],
        vec![dag_arg],
    );

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

/// Craft transaction arguments to create a [crate::dag::parser::EntryVertex]
/// on-chain.
fn create_entry_vertex(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    vertex: EntryVertex,
    groups: Vec<String>,
) -> AnyResult<sui::Argument> {
    // `entry_groups: vector<EntryGroup>`
    let entry_group_type = workflow::into_type_tag(workflow_pkg_id, workflow::Dag::ENTRY_GROUP);

    let entry_groups = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        move_std::Vector::EMPTY.module.into(),
        move_std::Vector::EMPTY.name.into(),
        vec![entry_group_type.clone()],
        vec![],
    );

    for entry_group in groups {
        // `entry_group: EntryGroup`
        let entry_group = workflow::Dag::entry_group_from_str(tx, workflow_pkg_id, entry_group)?;

        // `entry_groups.push_back(entry_group)`
        tx.programmable_move_call(
            sui::MOVE_STDLIB_PACKAGE_ID,
            move_std::Vector::PUSH_BACK.module.into(),
            move_std::Vector::PUSH_BACK.name.into(),
            vec![entry_group_type.clone()],
            vec![entry_groups, entry_group],
        );
    }

    // `name: Vertex`
    let name = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, vertex.name)?;

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => {
            // `tool_fqn: AsciiString`
            workflow::Dag::off_chain_vertex_kind_from_fqn(tx, workflow_pkg_id, tool_fqn)?
        }
        VertexKind::OnChain { .. } => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
        }
    };

    // `input_ports: VecSet<InputPort>`
    let input_port_type = workflow::into_type_tag(workflow_pkg_id, workflow::Dag::INPUT_PORT);

    let input_ports = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::VecSet::EMPTY.module.into(),
        sui_framework::VecSet::EMPTY.name.into(),
        vec![input_port_type.clone()],
        vec![],
    );

    for input_port in vertex.input_ports {
        // `input_port: InputPort`
        let input_port = workflow::Dag::input_port_from_str(tx, workflow_pkg_id, input_port)?;

        // `input_ports.insert(input_port)`
        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecSet::INSERT.module.into(),
            sui_framework::VecSet::INSERT.name.into(),
            vec![input_port_type.clone()],
            vec![input_ports, input_port],
        );
    }

    // `dag.with_entry_vertex(name, kind, input_ports)`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::WITH_ENTRY_VERTEX_IN_GROUPS.module.into(),
        workflow::Dag::WITH_ENTRY_VERTEX_IN_GROUPS.name.into(),
        vec![],
        vec![dag, entry_groups, name, kind, input_ports],
    ))
}

/// Craft transaction arguments to create a [crate::dag::parser::Vertex] on-chain.
fn create_vertex(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    vertex: &Vertex,
) -> AnyResult<sui::Argument> {
    // `name: Vertex`
    let name = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, &vertex.name)?;

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => {
            // `tool_fqn: AsciiString`
            workflow::Dag::off_chain_vertex_kind_from_fqn(tx, workflow_pkg_id, tool_fqn)?
        }
        VertexKind::OnChain { .. } => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
        }
    };

    // `dag.with_vertex(name, kind)`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::WITH_VERTEX.module.into(),
        workflow::Dag::WITH_VERTEX.name.into(),
        vec![],
        vec![dag, name, kind],
    ))
}

/// Craft transaction arguments to set the default value for
/// a [crate::dag::parser::Vertex] input port on-chain.
fn create_default_value(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    primitives_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    default_value: &DefaultValue,
) -> AnyResult<sui::Argument> {
    // `vertex: Vertex`
    let vertex = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, &default_value.vertex)?;

    // `port: InputPort`
    let port = workflow::Dag::input_port_from_str(tx, workflow_pkg_id, &default_value.input_port)?;

    // `value: NexusData`
    let value = match &default_value.value {
        Data::Inline { data } => {
            primitives::Data::nexus_data_from_json(tx, primitives_pkg_id, data)?
        }
        // Allowing to remind us that any other data storages can be added here.
        #[allow(unreachable_patterns)]
        _ => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/30>")
        }
    };

    // `dag.with_default_value(vertex, port, value)`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::WITH_DEFAULT_VALUE.module.into(),
        workflow::Dag::WITH_DEFAULT_VALUE.name.into(),
        vec![],
        vec![dag, vertex, port, value],
    ))
}

/// Craft transaction arguments to create a [crate::dag::parser::Edge] on-chain.
fn create_edge(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    edge: &Edge,
) -> AnyResult<sui::Argument> {
    // `from_vertex: Vertex`
    let from_vertex = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, &edge.from.vertex)?;

    // `from_variant: OutputVariant`
    let from_variant =
        workflow::Dag::output_variant_from_str(tx, workflow_pkg_id, &edge.from.output_variant)?;

    // `from_port: OutputPort`
    let from_port =
        workflow::Dag::output_port_from_str(tx, workflow_pkg_id, &edge.from.output_port)?;

    // `to_vertex: Vertex`
    let to_vertex = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, &edge.to.vertex)?;

    // `to_port: InputPort`
    let to_port = workflow::Dag::input_port_from_str(tx, workflow_pkg_id, &edge.to.input_port)?;

    // `dag.with_edge(frpm_vertex, from_variant, from_port, to_vertex, to_port)`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::WITH_EDGE.module.into(),
        workflow::Dag::WITH_EDGE.name.into(),
        vec![],
        vec![
            dag,
            from_vertex,
            from_variant,
            from_port,
            to_vertex,
            to_port,
        ],
    ))
}
