use {
    crate::{prelude::*, workflow},
    nexus_sdk::{
        crypto::session::Session,
        idents::{primitives, sui_framework, workflow as workflow_idents},
        sui::{self, move_ident_str, MoveStructTag, MoveTypeTag},
        transactions::scheduler as scheduler_tx,
        types::{NexusObjects, DEFAULT_ENTRY_GROUP},
    },
    serde_json::Value,
    std::collections::HashMap,
};

/// Parse metadata pairs provided as `key=value` strings.
pub(crate) fn parse_metadata(pairs: &[String]) -> AnyResult<Vec<(String, String)>, NexusCliError> {
    let mut result = Vec::with_capacity(pairs.len());
    for pair in pairs {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(NexusCliError::Any(anyhow!(
                "Invalid metadata entry '{pair}'. Expected format key=value"
            )));
        };
        if key.trim().is_empty() {
            return Err(NexusCliError::Any(anyhow!(
                "Metadata key in '{pair}' cannot be empty"
            )));
        }
        result.push((key.trim().to_owned(), value.trim().to_owned()));
    }
    Ok(result)
}

pub(crate) fn metadata_argument(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    metadata: &[(String, String)],
) -> AnyResult<sui::Argument, NexusCliError> {
    scheduler_tx::new_metadata(tx, objects, metadata.iter().cloned())
        .map_err(|e| NexusCliError::Any(anyhow!(e)))
}

pub(crate) fn constraints_policy_argument(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
) -> AnyResult<sui::Argument, NexusCliError> {
    let symbol_type = policy_symbol_type_tag(objects.primitives_pkg_id);
    let time_constraint_tag = time_constraint_type_tag(objects.workflow_pkg_id);

    let constraint_symbol = tx.programmable_move_call(
        objects.primitives_pkg_id,
        move_ident_str!("policy").into(),
        move_ident_str!("witness_symbol").into(),
        vec![time_constraint_tag],
        vec![],
    );

    let constraint_sequence = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        move_ident_str!("vector").into(),
        move_ident_str!("singleton").into(),
        vec![symbol_type.clone()],
        vec![constraint_symbol],
    );

    let constraints = tx.programmable_move_call(
        objects.workflow_pkg_id,
        move_ident_str!("scheduler").into(),
        move_ident_str!("new_constraints_policy").into(),
        vec![],
        vec![constraint_sequence],
    );

    let config = scheduler_tx::new_time_constraint_config(tx, objects)
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    scheduler_tx::register_time_constraint(tx, objects, constraints.clone(), config)
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    Ok(constraints)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execution_policy_argument(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag_id: sui::ObjectID,
    gas_price: u64,
    inputs: &Value,
    entry_group: Option<&str>,
    encrypt_handles: Option<&HashMap<String, Vec<String>>>,
) -> AnyResult<sui::Argument, NexusCliError> {
    let symbol_type = policy_symbol_type_tag(objects.primitives_pkg_id);
    let witness_tag = begin_execution_witness_type_tag(objects.workflow_pkg_id);

    let execution_symbol = tx.programmable_move_call(
        objects.primitives_pkg_id,
        move_ident_str!("policy").into(),
        move_ident_str!("witness_symbol").into(),
        vec![witness_tag],
        vec![],
    );

    let execution_sequence = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        move_ident_str!("vector").into(),
        move_ident_str!("singleton").into(),
        vec![symbol_type.clone()],
        vec![execution_symbol],
    );

    let execution = tx.programmable_move_call(
        objects.workflow_pkg_id,
        move_ident_str!("scheduler").into(),
        move_ident_str!("new_execution_policy").into(),
        vec![],
        vec![execution_sequence],
    );

    let dag_id_arg = sui_framework::Object::id_from_object_id(tx, dag_id)
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    let network_id_arg = sui_framework::Object::id_from_object_id(tx, objects.network_id)
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    let gas_price_arg = tx
        .pure(gas_price)
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    let entry_group = workflow_idents::Dag::entry_group_from_str(
        tx,
        objects.workflow_pkg_id,
        entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
    )
    .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    let with_vertex_inputs = build_inputs_vec_map(tx, objects, inputs, encrypt_handles)?;

    let config = tx.programmable_move_call(
        objects.workflow_pkg_id,
        move_ident_str!("dag").into(),
        move_ident_str!("new_dag_execution_config").into(),
        vec![],
        vec![
            dag_id_arg,
            network_id_arg,
            gas_price_arg,
            entry_group,
            with_vertex_inputs,
        ],
    );

    scheduler_tx::register_begin_execution(tx, objects, execution.clone(), config)
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    Ok(execution)
}

pub(crate) fn build_inputs_vec_map(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    inputs: &Value,
    encrypt_handles: Option<&HashMap<String, Vec<String>>>,
) -> AnyResult<sui::Argument, NexusCliError> {
    let inner_vec_map_type = vec![
        workflow_idents::into_type_tag(objects.workflow_pkg_id, workflow_idents::Dag::INPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        workflow_idents::into_type_tag(objects.workflow_pkg_id, workflow_idents::Dag::VERTEX),
        MoveTypeTag::Struct(Box::new(MoveStructTag {
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

    let data = inputs.as_object().ok_or_else(|| {
        NexusCliError::Any(anyhow!(
            "Input JSON must map vertex names to objects of port -> value"
        ))
    })?;

    for (vertex_name, value) in data {
        let vertex_inputs = value.as_object().ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "Vertex '{vertex_name}' value must be an object mapping ports to data"
            ))
        })?;

        let vertex =
            workflow_idents::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex_name)
                .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

        let inner_map = tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module.into(),
            sui_framework::VecMap::EMPTY.name.into(),
            inner_vec_map_type.clone(),
            vec![],
        );

        for (port_name, port_value) in vertex_inputs {
            let encrypted = encrypt_handles.map_or(false, |handles| {
                handles
                    .get(vertex_name)
                    .map_or(false, |ports| ports.iter().any(|p| p == port_name))
            });

            let port = if encrypted {
                workflow_idents::Dag::encrypted_input_port_from_str(
                    tx,
                    objects.workflow_pkg_id,
                    port_name,
                )
                .map_err(|e| NexusCliError::Any(anyhow!(e)))?
            } else {
                workflow_idents::Dag::input_port_from_str(tx, objects.workflow_pkg_id, port_name)
                    .map_err(|e| NexusCliError::Any(anyhow!(e)))?
            };

            let nexus_data = primitives::Data::nexus_data_from_json(
                tx,
                objects.primitives_pkg_id,
                port_value,
                encrypted,
            )
            .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

            tx.programmable_move_call(
                sui::FRAMEWORK_PACKAGE_ID,
                sui_framework::VecMap::INSERT.module.into(),
                sui_framework::VecMap::INSERT.name.into(),
                inner_vec_map_type.clone(),
                vec![inner_map.clone(), port, nexus_data],
            );
        }

        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::INSERT.module.into(),
            sui_framework::VecMap::INSERT.name.into(),
            outer_vec_map_type.clone(),
            vec![with_vertex_inputs.clone(), vertex, inner_map],
        );
    }

    Ok(with_vertex_inputs)
}

fn policy_symbol_type_tag(primitives_pkg_id: sui::ObjectID) -> MoveTypeTag {
    MoveTypeTag::Struct(Box::new(MoveStructTag {
        address: primitives_pkg_id.into(),
        module: move_ident_str!("policy").into(),
        name: move_ident_str!("Symbol").into(),
        type_params: vec![],
    }))
}

fn time_constraint_type_tag(workflow_pkg_id: sui::ObjectID) -> MoveTypeTag {
    MoveTypeTag::Struct(Box::new(MoveStructTag {
        address: workflow_pkg_id.into(),
        module: move_ident_str!("scheduler").into(),
        name: move_ident_str!("TimeConstraint").into(),
        type_params: vec![],
    }))
}

fn begin_execution_witness_type_tag(workflow_pkg_id: sui::ObjectID) -> MoveTypeTag {
    MoveTypeTag::Struct(Box::new(MoveStructTag {
        address: workflow_pkg_id.into(),
        module: move_ident_str!("default_tap").into(),
        name: move_ident_str!("BeginDagExecutionWitness").into(),
        type_params: vec![],
    }))
}

pub(crate) fn choices_are_mutually_exclusive(
    pairs: &[(&str, bool)],
) -> AnyResult<(), NexusCliError> {
    let selected: Vec<_> = pairs.iter().filter(|(_, present)| *present).collect();
    if selected.len() > 1 {
        let flags = selected
            .into_iter()
            .map(|(flag, _)| *flag)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(NexusCliError::Any(anyhow!(
            "Flags {flags} are mutually exclusive"
        )));
    }
    Ok(())
}

pub(crate) fn ensure_start_before_deadline(
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
) -> AnyResult<(), NexusCliError> {
    if let (Some(start), Some(deadline)) = (start_ms, deadline_ms) {
        if deadline < start {
            return Err(NexusCliError::Any(anyhow!(
                "Deadline ({deadline}) cannot be earlier than start ({start})"
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_schedule_options(
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
    require_start: bool,
) -> AnyResult<(), NexusCliError> {
    choices_are_mutually_exclusive(&[
        ("--schedule-start-ms", start_ms.is_some()),
        ("--schedule-start-offset-ms", start_offset_ms.is_some()),
    ])?;

    choices_are_mutually_exclusive(&[
        ("--schedule-deadline-ms", deadline_ms.is_some()),
        (
            "--schedule-deadline-offset-ms",
            deadline_offset_ms.is_some(),
        ),
    ])?;

    if require_start && start_ms.is_none() && start_offset_ms.is_none() {
        return Err(NexusCliError::Any(anyhow!(
            "Provide either --schedule-start-ms or --schedule-start-offset-ms"
        )));
    }

    if start_ms.is_none()
        && start_offset_ms.is_none()
        && (deadline_ms.is_some() || deadline_offset_ms.is_some())
    {
        return Err(NexusCliError::Any(anyhow!(
            "Deadline flags require a corresponding start flag"
        )));
    }

    if let Some(start) = start_ms {
        ensure_start_before_deadline(Some(start), deadline_ms)?;
    }

    ensure_offset_deadline_valid(start_offset_ms, deadline_offset_ms)?;

    Ok(())
}

pub(crate) fn ensure_offset_deadline_valid(
    start_offset: Option<u64>,
    deadline_offset: Option<u64>,
) -> AnyResult<(), NexusCliError> {
    if deadline_offset.is_some() && start_offset.is_none() {
        return Err(NexusCliError::Any(anyhow!(
            "Deadline offset requires --schedule-start-offset-ms"
        )));
    }
    Ok(())
}

pub(crate) async fn fetch_encryption_targets(
    sui: &sui::Client,
    dag_id: &sui::ObjectID,
    entry_group: &str,
) -> AnyResult<HashMap<String, Vec<String>>, NexusCliError> {
    workflow::fetch_encrypted_entry_ports(sui, entry_group.to_owned(), dag_id).await
}

pub(crate) fn encrypt_inputs_once(
    session: &mut nexus_sdk::crypto::session::Session,
    input_json: &mut Value,
    handles: &HashMap<String, Vec<String>>,
) -> AnyResult<(), NexusCliError> {
    workflow::encrypt_entry_ports_once(session, input_json, handles)
}

pub(crate) fn get_active_session(conf: &mut CliConf) -> Result<&mut Session, NexusCliError> {
    match &mut conf.crypto {
        Some(crypto_secret) => {
            if crypto_secret.sessions.is_empty() {
                return Err(NexusCliError::Any(anyhow!(
                    "Authentication required — run `nexus crypto auth` first"
                )));
            }

            let session_id = *crypto_secret.sessions.values().next().unwrap().id();
            crypto_secret
                .sessions
                .get_mut(&session_id)
                .ok_or_else(|| NexusCliError::Any(anyhow!("Session not found in config")))
        }
        None => Err(NexusCliError::Any(anyhow!(
            "Authentication required — run `nexus crypto auth` first"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_metadata_splits_pairs() {
        let result = parse_metadata(&["a=b".to_string(), "c=d".to_string()]).unwrap();
        assert_eq!(
            result,
            vec![("a".into(), "b".into()), ("c".into(), "d".into())]
        );
    }

    #[test]
    fn parse_metadata_rejects_missing_equals() {
        let err = parse_metadata(&["invalid".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Invalid metadata entry"));
    }

    #[test]
    fn choices_exclusive_detects_conflict() {
        let err = choices_are_mutually_exclusive(&[("a", true), ("b", true)]).unwrap_err();
        assert!(err.to_string().contains("mutually exclusive"));
    }

    #[test]
    fn validate_schedule_with_absolute_start_passes() {
        validate_schedule_options(Some(10), None, None, None, false).unwrap();
    }

    #[test]
    fn validate_schedule_requires_start_when_flagged() {
        let err = validate_schedule_options(None, None, None, None, true).unwrap_err();
        assert!(err
            .to_string()
            .contains("Provide either --schedule-start-ms"));
    }

    #[test]
    fn validate_schedule_rejects_deadline_without_start() {
        let err = validate_schedule_options(None, Some(20), None, None, false).unwrap_err();
        assert!(err.to_string().contains("Deadline flags require"));
    }

    #[test]
    fn validate_schedule_allows_deadline_equal_start() {
        validate_schedule_options(Some(25), Some(25), None, None, false).unwrap();
    }

    #[test]
    fn validate_schedule_allows_deadline_offset_shorter_than_start_offset() {
        validate_schedule_options(None, None, Some(60_000), Some(30_000), false).unwrap();
    }

    #[test]
    fn ensure_offset_deadline_requires_start_offset() {
        let err = ensure_offset_deadline_valid(None, Some(5)).unwrap_err();
        assert!(err
            .to_string()
            .contains("Deadline offset requires --schedule-start-offset-ms"));
    }
}
