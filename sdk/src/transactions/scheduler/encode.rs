use {
    super::command::{PreparedOccurrence, PreparedRecurrence, PreparedTask},
    crate::{
        move_bindings::{
            interface::{
                agent as agent_binding,
                authorization::{self as authorization_binding, AgentVertexAuthorizationTemplate},
                graph::{InputPort, Vertex},
            },
            move_std::option::Option as MoveOption,
            primitives::data::NexusData,
            scheduler::scheduler as scheduler_binding,
            sui_framework::{
                object::ID as MoveObjectId,
                vec_map::{self as vec_map_binding, VecMap},
            },
        },
        move_boundary::NexusPtbBuilder,
        scheduler::{AuthorizationTemplate, FailurePolicy, SchedulerError, TaskOperation},
        sui,
    },
    sui_sdk_types::Argument,
};

type VertexInputs = VecMap<InputPort, NexusData>;

pub(super) fn execution_config_arg(
    transaction: &mut NexusPtbBuilder,
    task: &PreparedTask,
) -> Result<Argument, SchedulerError> {
    let network = transaction
        .object_id(transaction.objects().network_id)
        .map_err(SchedulerError::transaction)?;
    let entry_group = transaction
        .graph_entry_group(&task.entry_group)
        .map_err(SchedulerError::transaction)?;
    let inputs = execution_inputs_arg(transaction, &task.inputs)?;

    match &task.operation {
        TaskOperation::DefaultDag { dag_id } => {
            let dag_id = transaction
                .object_id(*dag_id)
                .map_err(SchedulerError::transaction)?;
            transaction
                .call_target(
                    agent_binding::new_default_agent_execution_config_target,
                    vec![dag_id, network, entry_group, inputs],
                )
                .map_err(SchedulerError::transaction)
        }
        TaskOperation::AgentSkill {
            agent_id,
            skill_id,
            selected_dag,
            authorization_templates,
        } => {
            let agent_id = transaction
                .object_id(*agent_id)
                .map_err(SchedulerError::transaction)?;
            let skill_id = transaction
                .arg(skill_id)
                .map_err(SchedulerError::transaction)?;
            let selected_dag = optional_object_id_arg(transaction, *selected_dag)?;
            let authorization_templates =
                authorization_templates_arg(transaction, authorization_templates)?;
            transaction
                .call_target(
                    agent_binding::new_agent_execution_config_target,
                    vec![
                        agent_id,
                        network,
                        entry_group,
                        inputs,
                        skill_id,
                        selected_dag,
                        authorization_templates,
                    ],
                )
                .map_err(SchedulerError::transaction)
        }
    }
}

fn execution_inputs_arg(
    transaction: &mut NexusPtbBuilder,
    inputs: &crate::scheduler::TaskInputs,
) -> Result<Argument, SchedulerError> {
    let execution_inputs = transaction
        .call_target(
            vec_map_binding::empty_target::<Vertex, VertexInputs>,
            vec![],
        )
        .map_err(SchedulerError::transaction)?;

    for (vertex, inputs) in inputs {
        let vertex = transaction
            .graph_vertex(vertex)
            .map_err(SchedulerError::transaction)?;
        let inputs_for_vertex = transaction
            .call_target(
                vec_map_binding::empty_target::<InputPort, NexusData>,
                vec![],
            )
            .map_err(SchedulerError::transaction)?;

        for (input, value) in inputs {
            let input = transaction
                .graph_input_port(input)
                .map_err(SchedulerError::transaction)?;
            let value = transaction
                .nexus_data(value)
                .map_err(SchedulerError::transaction)?;
            transaction
                .call_target(
                    vec_map_binding::insert_target::<InputPort, NexusData>,
                    vec![inputs_for_vertex, input, value],
                )
                .map_err(SchedulerError::transaction)?;
        }

        transaction
            .call_target(
                vec_map_binding::insert_target::<Vertex, VertexInputs>,
                vec![execution_inputs, vertex, inputs_for_vertex],
            )
            .map_err(SchedulerError::transaction)?;
    }

    Ok(execution_inputs)
}

fn optional_object_id_arg(
    transaction: &mut NexusPtbBuilder,
    value: Option<sui::types::Address>,
) -> Result<Argument, SchedulerError> {
    let value = value
        .map(|value| transaction.object_id(value))
        .transpose()
        .map_err(SchedulerError::transaction)?;
    transaction
        .option::<MoveObjectId>(value)
        .map_err(SchedulerError::transaction)
}

fn authorization_template_arg(
    transaction: &mut NexusPtbBuilder,
    template: &AuthorizationTemplate,
) -> Result<Argument, SchedulerError> {
    let skill_id = transaction
        .arg(&template.skill_id())
        .map_err(SchedulerError::transaction)?;
    let vertex = transaction
        .ascii_string(template.vertex())
        .map_err(SchedulerError::transaction)?;
    let recipient_id = transaction
        .object_id(template.recipient_id())
        .map_err(SchedulerError::transaction)?;
    transaction
        .call_target(
            authorization_binding::agent_vertex_authorization_template_target,
            vec![skill_id, vertex, recipient_id],
        )
        .map_err(SchedulerError::transaction)
}

fn authorization_templates_arg(
    transaction: &mut NexusPtbBuilder,
    templates: &[AuthorizationTemplate],
) -> Result<Argument, SchedulerError> {
    let templates = templates
        .iter()
        .map(|template| authorization_template_arg(transaction, template))
        .collect::<Result<Vec<_>, _>>()?;
    transaction
        .move_vector::<AgentVertexAuthorizationTemplate>(templates)
        .map_err(SchedulerError::transaction)
}

pub(super) fn failure_policy_arg(
    transaction: &mut NexusPtbBuilder,
    policy: FailurePolicy,
) -> Result<Argument, SchedulerError> {
    let target = match policy {
        FailurePolicy::Continue => scheduler_binding::continue_on_failure_target,
        FailurePolicy::Pause => scheduler_binding::pause_on_failure_target,
    };
    transaction
        .call_target(target, vec![])
        .map_err(SchedulerError::transaction)
}

pub(super) fn occurrence_args(
    transaction: &mut NexusPtbBuilder,
    occurrence: &PreparedOccurrence,
) -> Result<(Argument, Argument, Argument), SchedulerError> {
    let start_time_ms = transaction
        .arg(&occurrence.start_time_ms)
        .map_err(SchedulerError::transaction)?;
    let deadline_ms = transaction
        .arg(&MoveOption::from_option(occurrence.deadline_ms))
        .map_err(SchedulerError::transaction)?;
    let priority_fee_percentage = transaction
        .arg(&occurrence.priority_fee_percentage)
        .map_err(SchedulerError::transaction)?;
    Ok((start_time_ms, deadline_ms, priority_fee_percentage))
}

pub(super) fn recurrence_args(
    transaction: &mut NexusPtbBuilder,
    recurrence: &PreparedRecurrence,
) -> Result<(Argument, Argument, Argument, Argument, Argument), SchedulerError> {
    let (start_time_ms, deadline_ms, priority_fee_percentage) =
        occurrence_args(transaction, &recurrence.first)?;
    let interval_ms = transaction
        .arg(&recurrence.interval_ms)
        .map_err(SchedulerError::transaction)?;
    let occurrences = transaction
        .arg(&MoveOption::from_option(recurrence.occurrences))
        .map_err(SchedulerError::transaction)?;
    Ok((
        start_time_ms,
        deadline_ms,
        interval_ms,
        occurrences,
        priority_fee_percentage,
    ))
}
