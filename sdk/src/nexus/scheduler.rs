//! Scheduler-oriented actions exposed through [`NexusClient`].

use {
    crate::{
        events::NexusEventKind,
        idents::sui_framework,
        nexus::{
            client::NexusClient,
            crawler::Response,
            error::NexusError,
            signer::ExecutedTransaction,
        },
        sui,
        transactions::scheduler as scheduler_tx,
        types::{DataStorage, Task},
    },
    anyhow::anyhow,
    std::collections::HashMap,
};

/// High-level interface for scheduler operations.
#[derive(Clone)]
pub struct SchedulerActions {
    pub(super) client: NexusClient,
}

/// Supported generator types for a scheduler task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorKind {
    Queue,
    Periodic,
}

impl From<GeneratorKind> for scheduler_tx::OccurrenceGenerator {
    fn from(value: GeneratorKind) -> Self {
        match value {
            GeneratorKind::Queue => scheduler_tx::OccurrenceGenerator::Queue,
            GeneratorKind::Periodic => scheduler_tx::OccurrenceGenerator::Periodic,
        }
    }
}

/// Parameters required to create a scheduler task.
pub struct CreateTaskParams {
    pub dag_id: sui::types::Address,
    pub entry_group: String,
    pub input_data: HashMap<String, HashMap<String, DataStorage>>,
    pub metadata: Vec<(String, String)>,
    pub execution_gas_price: u64,
    pub initial_schedule: Option<OccurrenceRequest>,
    pub generator: GeneratorKind,
}

/// Result returned after creating a scheduler task.
pub struct CreateTaskResult {
    pub tx_digest: sui::types::Digest,
    pub task_id: sui::types::Address,
    pub initial_schedule: Option<ScheduleExecutionResult>,
}

/// Result returned after enqueuing an occurrence.
pub struct ScheduleExecutionResult {
    pub tx_digest: sui::types::Digest,
    pub event: Option<NexusEventKind>,
}

/// Parameters for a sporadic occurrence (start/deadline offsets).
#[derive(Clone, Debug)]
pub struct OccurrenceRequest {
    pub start_ms: Option<u64>,
    pub deadline_ms: Option<u64>,
    pub start_offset_ms: Option<u64>,
    pub deadline_offset_ms: Option<u64>,
    pub gas_price: u64,
}

impl OccurrenceRequest {
    pub fn new(
        start_ms: Option<u64>,
        deadline_ms: Option<u64>,
        start_offset_ms: Option<u64>,
        deadline_offset_ms: Option<u64>,
        gas_price: u64,
        require_start: bool,
    ) -> Result<Self, NexusError> {
        validate_schedule_options(
            start_ms,
            deadline_ms,
            start_offset_ms,
            deadline_offset_ms,
            require_start,
        )?;

        Ok(Self {
            start_ms,
            deadline_ms,
            start_offset_ms,
            deadline_offset_ms,
            gas_price,
        })
    }
}

/// Actions supported when mutating task state.
#[derive(Clone, Copy, Debug)]
pub enum TaskStateAction {
    Pause,
    Resume,
    Cancel,
}

/// Configuration for periodic scheduling.
pub struct PeriodicScheduleConfig {
    pub first_start_ms: u64,
    pub period_ms: u64,
    pub deadline_offset_ms: Option<u64>,
    pub max_iterations: Option<u64>,
    pub gas_price: u64,
}

pub struct PeriodicScheduleResult {
    pub tx_digest: sui::types::Digest,
}

pub struct DisablePeriodicResult {
    pub tx_digest: sui::types::Digest,
}

pub struct UpdateMetadataResult {
    pub tx_digest: sui::types::Digest,
    pub entries: usize,
}

pub struct TaskStateResult {
    pub tx_digest: sui::types::Digest,
    pub state: TaskStateAction,
}

impl SchedulerActions {
    /// Create a scheduler task and optionally enqueue its first occurrence.
    pub async fn create_task(
        &self,
        params: CreateTaskParams,
    ) -> Result<CreateTaskResult, NexusError> {
        let CreateTaskParams {
            dag_id,
            entry_group,
            input_data,
            metadata,
            execution_gas_price,
            initial_schedule: initial_schedule_request,
            generator,
        } = params;
        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;

        let mut tx = sui::tx::TransactionBuilder::new();

        let metadata_arg = scheduler_tx::new_metadata(&mut tx, objects, metadata.iter().cloned())
            .map_err(NexusError::TransactionBuilding)?;

        let constraints_arg =
            scheduler_tx::new_constraints_policy(&mut tx, objects, generator.into())
                .map_err(NexusError::TransactionBuilding)?;

        let execution_arg = scheduler_tx::new_execution_policy(
            &mut tx,
            objects,
            dag_id,
            execution_gas_price,
            entry_group.as_str(),
            &input_data,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let task = scheduler_tx::new_task(
            &mut tx,
            objects,
            metadata_arg,
            constraints_arg,
            execution_arg,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let task_type = crate::idents::workflow::into_type_tag(
            objects.workflow_pkg_id,
            crate::idents::workflow::Scheduler::TASK,
        );

        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
                sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
                vec![task_type],
            ),
            vec![task],
        );

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        let task_id = extract_task_id(&response)?;

        let mut initial_schedule_result = None;
        if initial_schedule_request.is_some() && generator != GeneratorKind::Queue {
            return Err(NexusError::Configuration(
                "Initial queue schedule can only be used with the queue generator".into(),
            ));
        }

        if let Some(schedule) = initial_schedule_request {
            let task_object = self.fetch_task(task_id).await?;
            initial_schedule_result = Some(
                self.enqueue_occurrence(&task_object, schedule, address)
                    .await?,
            );
        }

        Ok(CreateTaskResult {
            tx_digest: response.digest,
            task_id,
            initial_schedule: initial_schedule_result,
        })
    }

    /// Update metadata entries associated with a task.
    pub async fn update_metadata(
        &self,
        task_id: sui::types::Address,
        metadata: Vec<(String, String)>,
    ) -> Result<UpdateMetadataResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;

        let task = self.fetch_task(task_id).await?;

        let metadata_arg =
            scheduler_tx::new_metadata(&mut tx, objects, metadata.clone().into_iter())
                .map_err(NexusError::TransactionBuilding)?;

        scheduler_tx::update_metadata(&mut tx, objects, &task.object_ref(), metadata_arg)
            .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(UpdateMetadataResult {
            tx_digest: response.digest,
            entries: metadata.len(),
        })
    }

    /// Set the scheduler state for a task (pause/resume/cancel).
    pub async fn set_task_state(
        &self,
        task_id: sui::types::Address,
        request: TaskStateAction,
    ) -> Result<TaskStateResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;

        let task = self.fetch_task(task_id).await?;

        match request {
            TaskStateAction::Pause => {
                scheduler_tx::pause_time_constraint_for_task(&mut tx, objects, &task.object_ref())
            }
            TaskStateAction::Resume => {
                scheduler_tx::resume_time_constraint_for_task(&mut tx, objects, &task.object_ref())
            }
            TaskStateAction::Cancel => {
                scheduler_tx::cancel_time_constraint_for_task(&mut tx, objects, &task.object_ref())
            }
        }
        .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(TaskStateResult {
            tx_digest: response.digest,
            state: request,
        })
    }

    /// Add a sporadic occurrence for an existing task.
    pub async fn add_occurrence(
        &self,
        task_id: sui::types::Address,
        request: OccurrenceRequest,
    ) -> Result<ScheduleExecutionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;

        self.enqueue_occurrence(&task, request, address).await
    }

    /// Configure or update periodic scheduling for a task.
    pub async fn configure_periodic(
        &self,
        task_id: sui::types::Address,
        config: PeriodicScheduleConfig,
    ) -> Result<PeriodicScheduleResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;
        let task = self.fetch_task(task_id).await?;

        scheduler_tx::new_or_modify_periodic_for_task(
            &mut tx,
            objects,
            &task.object_ref(),
            scheduler_tx::PeriodicScheduleInputs {
                first_start_ms: config.first_start_ms,
                period_ms: config.period_ms,
                deadline_offset_ms: config.deadline_offset_ms,
                max_iterations: config.max_iterations,
                gas_price: config.gas_price,
            },
        )
        .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(PeriodicScheduleResult {
            tx_digest: response.digest,
        })
    }

    /// Disable periodic scheduling for a task.
    pub async fn disable_periodic(
        &self,
        task_id: sui::types::Address,
    ) -> Result<DisablePeriodicResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;
        let task = self.fetch_task(task_id).await?;

        scheduler_tx::disable_periodic_for_task(&mut tx, objects, &task.object_ref())
            .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(DisablePeriodicResult {
            tx_digest: response.digest,
        })
    }

    async fn enqueue_occurrence(
        &self,
        task: &Response<Task>,
        request: OccurrenceRequest,
        address: sui::types::Address,
    ) -> Result<ScheduleExecutionResult, NexusError> {
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;

        if let Some(start_ms) = request.start_ms {
            let deadline_offset = request
                .deadline_offset_ms
                .or_else(|| request.deadline_ms.map(|deadline| deadline - start_ms));

            scheduler_tx::add_occurrence_absolute_for_task(
                &mut tx,
                objects,
                &task.object_ref(),
                start_ms,
                deadline_offset,
                request.gas_price,
            )
        } else {
            scheduler_tx::add_occurrence_relative_for_task(
                &mut tx,
                objects,
                &task.object_ref(),
                request.start_offset_ms.expect("validated start offset"),
                request.deadline_offset_ms,
                request.gas_price,
            )
        }
        .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(ScheduleExecutionResult {
            tx_digest: response.digest,
            event: extract_occurrence_event(&response),
        })
    }

    async fn fetch_task(&self, task_id: sui::types::Address) -> Result<Response<Task>, NexusError> {
        self.client
            .crawler()
            .get_object::<Task>(task_id)
            .await
            .map_err(NexusError::Rpc)
    }
}

fn extract_task_id(response: &ExecutedTransaction) -> Result<sui::types::Address, NexusError> {
    response
        .events
        .iter()
        .find_map(|event| match &event.data {
            NexusEventKind::TaskCreated(e) => Some(e.task),
            _ => None,
        })
        .ok_or_else(|| NexusError::Parsing(anyhow!("TaskCreatedEvent not found in response")))
}

fn extract_occurrence_event(response: &ExecutedTransaction) -> Option<NexusEventKind> {
    // TODO: @david check that the bcs deser works here.
    response.events.iter().find_map(|event| match &event.data {
        NexusEventKind::OccurrenceScheduled(_) => Some(event.data.clone()),
        _ => None,
    })
}

fn validate_schedule_options(
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
    require_start: bool,
) -> Result<(), NexusError> {
    if require_start && start_ms.is_none() && start_offset_ms.is_none() {
        return Err(NexusError::Configuration(
            "Provide either an absolute start or a start offset".into(),
        ));
    }

    if deadline_ms.is_some() && start_ms.is_none() {
        return Err(NexusError::Configuration(
            "Absolute deadlines require an absolute start time".into(),
        ));
    }

    if start_ms.is_none()
        && start_offset_ms.is_none()
        && (deadline_ms.is_some() || deadline_offset_ms.is_some())
    {
        return Err(NexusError::Configuration(
            "Deadline flags require a corresponding start flag".into(),
        ));
    }

    if let Some(start) = start_ms {
        ensure_start_before_deadline(Some(start), deadline_ms)?;
    }

    ensure_offset_deadline_valid(start_ms, start_offset_ms, deadline_offset_ms)?;

    Ok(())
}

fn ensure_start_before_deadline(
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
) -> Result<(), NexusError> {
    if let (Some(start), Some(deadline)) = (start_ms, deadline_ms) {
        if deadline < start {
            return Err(NexusError::Configuration(format!(
                "Deadline ({deadline}) cannot be earlier than start ({start})"
            )));
        }
    }
    Ok(())
}

fn ensure_offset_deadline_valid(
    start_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
) -> Result<(), NexusError> {
    if deadline_offset_ms.is_some() && start_offset_ms.is_none() && start_ms.is_none() {
        return Err(NexusError::Configuration(
            "Deadline offset requires either an absolute start or a start offset".into(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // TODO: @david to adjust and re-enable
    // use {
    //     super::*,
    //     crate::{
    //         events::{NexusEventKind, OccurrenceScheduledEvent, TaskCreatedEvent},
    //         idents::{primitives, workflow, ModuleAndNameIdent},
    //         nexus::error::NexusError,
    //         sui,
    //         test_utils::{nexus_mocks, sui_mocks},
    //         types::{
    //             ConfiguredAutomaton,
    //             DataStorage,
    //             MoveTypeName,
    //             NexusData,
    //             Policy,
    //             PolicySymbol,
    //             Task,
    //         },
    //     },
    //     serde_json::json,
    //     std::collections::HashMap,
    // };
    // fn sample_input_data() -> HashMap<String, HashMap<String, DataStorage>> {
    //     HashMap::from([(
    //         "entry_vertex".to_string(),
    //         HashMap::from([(
    //             "entry_port".to_string(),
    //             NexusData::new_inline(json!("payload")).commit_inline_plain(),
    //         )]),
    //     )])
    // }

    // fn mock_task_object(
    //     workflow_pkg_id: sui::types::Address,
    //     task_id: sui::types::Address,
    //     owner: sui::types::Address,
    // ) -> sui::ParsedMoveObject {
    //     let execution_policy = Policy {
    //         id: sui::UID::new(sui::types::Address::random()),
    //         dfa: ConfiguredAutomaton {
    //             id: sui::UID::new(sui::types::Address::random()),
    //             dfa: json!({}),
    //         },
    //         alphabet_index: json!({}),
    //         state_index: 0,
    //         data: json!({}),
    //     };

    //     let task = Task {
    //         id: sui::UID::new(task_id),
    //         owner,
    //         metadata: json!({"initial": "value"}),
    //         constraints: json!({}),
    //         execution: execution_policy,
    //         data: json!({}),
    //         objects: json!({}),
    //     };

    //     let task_value = serde_json::to_value(task).expect("serialize task");
    //     let move_struct: sui::MoveStruct =
    //         serde_json::from_value(task_value).expect("task into move struct");

    //     sui::ParsedMoveObject {
    //         type_: sui::MoveStructTag {
    //             address: workflow_pkg_id.into(),
    //             module: sui::types::Identifier::from_static("scheduler").into(),
    //             name: sui::types::Identifier::from_static("Task").into(),
    //             type_params: vec![],
    //         },
    //         has_public_transfer: false,
    //         fields: move_struct,
    //     }
    // }

    // fn generator_symbol(
    //     workflow_pkg_id: sui::types::Address,
    //     ident: &ModuleAndNameIdent,
    // ) -> PolicySymbol {
    //     PolicySymbol::Witness(MoveTypeName {
    //         name: format!("{}::{}::{}", workflow_pkg_id, ident.module, ident.name),
    //     })
    // }

    // fn queue_generator_symbol(workflow_pkg_id: sui::types::Address) -> PolicySymbol {
    //     generator_symbol(
    //         workflow_pkg_id,
    //         &workflow::Scheduler::QUEUE_GENERATOR_WITNESS,
    //     )
    // }

    // fn periodic_generator_symbol(workflow_pkg_id: sui::types::Address) -> PolicySymbol {
    //     generator_symbol(
    //         workflow_pkg_id,
    //         &workflow::Scheduler::PERIODIC_GENERATOR_WITNESS,
    //     )
    // }

    // fn scheduler_move_tag(
    //     workflow_pkg_id: sui::types::Address,
    //     name: &str,
    //     type_params: Vec<sui::types::TypeTag>,
    // ) -> sui::types::TypeTag {
    //     sui::types::TypeTag::Struct(Box::new(sui::MoveStructTag {
    //         address: workflow_pkg_id.into(),
    //         module: sui::types::Identifier::from_static("scheduler").into(),
    //         name: sui::Identifier::new(name.to_string()).expect("valid identifier"),
    //         type_params,
    //     }))
    // }

    // fn event_move_tag(
    //     workflow_pkg_id: sui::types::Address,
    //     kind: &NexusEventKind,
    // ) -> sui::types::TypeTag {
    //     match kind {
    //         NexusEventKind::Scheduled(request) => scheduler_move_tag(
    //             workflow_pkg_id,
    //             "RequestScheduledExecution",
    //             vec![event_move_tag(workflow_pkg_id, request.request.as_ref())],
    //         ),
    //         _ => scheduler_move_tag(workflow_pkg_id, &kind.name(), vec![]),
    //     }
    // }

    // fn type_params_for_event(
    //     workflow_pkg_id: sui::types::Address,
    //     kind: &NexusEventKind,
    // ) -> Vec<sui::types::TypeTag> {
    //     vec![event_move_tag(workflow_pkg_id, kind)]
    // }

    // fn move_event_payload(kind: &NexusEventKind) -> serde_json::Value {
    //     fn to_string_id(id: sui::types::Address) -> String {
    //         id.to_string()
    //     }

    //     fn request_fields(kind: &NexusEventKind) -> serde_json::Value {
    //         match kind {
    //             NexusEventKind::TaskCreated(ev) => json!({
    //                 "task": to_string_id(ev.task),
    //                 "owner": ev.owner.to_string(),
    //             }),
    //             NexusEventKind::OccurrenceScheduled(ev) => json!({
    //                 "task": to_string_id(ev.task),
    //                 "generator": serde_json::to_value(&ev.generator)
    //                     .unwrap_or_else(|_| serde_json::Value::Null),
    //             }),
    //             NexusEventKind::Scheduled(env) => json!({
    //                 "request": request_fields(&env.request),
    //                 "priority": env.priority.to_string(),
    //                 "request_ms": env.request_ms.to_string(),
    //                 "start_ms": env.start_ms.to_string(),
    //                 "deadline_ms": env.deadline_ms.to_string(),
    //             }),
    //             _ => {
    //                 let value =
    //                     serde_json::to_value(kind).unwrap_or_else(|_| serde_json::Value::Null);
    //                 if let serde_json::Value::Object(mut map) = value {
    //                     map.remove("_nexus_event_type");
    //                     serde_json::Value::Object(map)
    //                 } else {
    //                     value
    //                 }
    //             }
    //         }
    //     }

    //     json!({ "event": request_fields(kind) })
    // }

    // fn make_nexus_event(workflow_pkg_id: sui::types::Address, kind: NexusEventKind) -> sui::Event {
    //     let parsed_json = move_event_payload(&kind);
    //     sui::Event {
    //         id: sui_mocks::mock_sui_event_id(),
    //         package_id: workflow_pkg_id,
    //         transaction_module: primitives::Event::EVENT_WRAPPER.module,
    //         sender: sui::types::Address::random().into(),
    //         type_: sui::MoveStructTag {
    //             address: workflow_pkg_id.into(),
    //             module: primitives::Event::EVENT_WRAPPER.module,
    //             name: primitives::Event::EVENT_WRAPPER.name,
    //             type_params: type_params_for_event(workflow_pkg_id, &kind),
    //         },
    //         parsed_json,
    //         bcs: sui::BcsEvent::Base64 { bcs: vec![] },
    //         timestamp_ms: None,
    //     }
    // }

    // fn block_events(
    //     workflow_pkg_id: sui::types::Address,
    //     kinds: Vec<NexusEventKind>,
    // ) -> sui::TransactionBlockEvents {
    //     sui::TransactionBlockEvents {
    //         data: kinds
    //             .into_iter()
    //             .map(|event| make_nexus_event(workflow_pkg_id, event))
    //             .collect(),
    //     }
    // }

    // #[tokio::test]
    // async fn test_scheduler_create_task_without_initial_schedule() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let workflow_pkg = nexus_client.nexus_objects.workflow_pkg_id;
    //     let dag_id = sui::types::Address::random();
    //     let task_id = sui::types::Address::random();
    //     let owner = nexus_client
    //         .signer
    //         .get_active_address()
    //         .await
    //         .expect("address");
    //     let tx_digest = sui::TransactionDigest::random();

    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             tx_digest,
    //             None,
    //             Some(block_events(
    //                 workflow_pkg,
    //                 vec![NexusEventKind::TaskCreated(TaskCreatedEvent {
    //                     task: task_id,
    //                     owner,
    //                 })],
    //             )),
    //             None,
    //             None,
    //         );

    //     let params = CreateTaskParams {
    //         dag_id,
    //         entry_group: "entry".into(),
    //         input_data: sample_input_data(),
    //         metadata: vec![("team".into(), "sdk".into())],
    //         execution_gas_price: 1,
    //         initial_schedule: None,
    //         generator: GeneratorKind::Queue,
    //     };

    //     let result = nexus_client
    //         .scheduler()
    //         .create_task(params)
    //         .await
    //         .expect("task created");

    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert_eq!(result.task_id, task_id);
    //     assert_eq!(result.tx_digest, tx_digest);
    //     assert!(result.initial_schedule.is_none());
    // }

    // #[tokio::test]
    // async fn test_scheduler_create_task_with_initial_schedule() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let workflow_pkg = nexus_client.nexus_objects.workflow_pkg_id;
    //     let dag_id = sui::types::Address::random();
    //     let task_id = sui::types::Address::random();
    //     let owner = nexus_client
    //         .signer
    //         .get_active_address()
    //         .await
    //         .expect("address");

    //     let creation_digest = sui::TransactionDigest::random();
    //     let (create_exec, create_confirm) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             creation_digest,
    //             None,
    //             Some(block_events(
    //                 workflow_pkg,
    //                 vec![NexusEventKind::TaskCreated(TaskCreatedEvent {
    //                     task: task_id,
    //                     owner,
    //                 })],
    //             )),
    //             None,
    //             None,
    //         );

    //     let queue_generator = queue_generator_symbol(workflow_pkg);
    //     let scheduled_event = NexusEventKind::Scheduled(RequestScheduledExecution {
    //         request: Box::new(NexusEventKind::OccurrenceScheduled(
    //             OccurrenceScheduledEvent {
    //                 task: task_id,
    //                 generator: queue_generator.clone(),
    //             },
    //         )),
    //         priority: 10,
    //         request_ms: 100,
    //         start_ms: 200,
    //         deadline_ms: 300,
    //     });

    //     let schedule_digest = sui::TransactionDigest::random();
    //     let (occ_exec, occ_confirm) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             schedule_digest,
    //             None,
    //             Some(block_events(workflow_pkg, vec![scheduled_event.clone()])),
    //             None,
    //             None,
    //         );

    //     let task_object = mock_task_object(workflow_pkg, task_id, owner);
    //     let get_task_call =
    //         sui_mocks::rpc::mock_read_api_get_object(&mut server, task_id, task_object);

    //     let initial_schedule =
    //         OccurrenceRequest::new(Some(1_000), Some(2_000), None, None, 5, true)
    //             .expect("valid request");

    //     let params = CreateTaskParams {
    //         dag_id,
    //         entry_group: "entry".into(),
    //         input_data: sample_input_data(),
    //         metadata: vec![],
    //         execution_gas_price: 5,
    //         initial_schedule: Some(initial_schedule),
    //         generator: GeneratorKind::Queue,
    //     };

    //     let result = nexus_client
    //         .scheduler()
    //         .create_task(params)
    //         .await
    //         .expect("task created");

    //     create_exec.assert_async().await;
    //     create_confirm.assert_async().await;
    //     occ_exec.assert_async().await;
    //     occ_confirm.assert_async().await;
    //     get_task_call.assert_async().await;

    //     let schedule = result.initial_schedule.expect("schedule created");
    //     assert_eq!(schedule.tx_digest, schedule_digest);
    //     match schedule.event.expect("event present") {
    //         NexusEventKind::Scheduled(envelope) => {
    //             assert_eq!(envelope.priority, 10);
    //             assert_eq!(envelope.start_ms, 200);
    //             assert!(matches!(
    //                 *envelope.request,
    //                 NexusEventKind::OccurrenceScheduled(ev)
    //                     if ev.task == task_id && ev.generator == queue_generator
    //             ));
    //         }
    //         other => panic!("unexpected event {other:?}"),
    //     }
    // }

    // #[tokio::test]
    // async fn test_scheduler_update_metadata() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let workflow_pkg = nexus_client.nexus_objects.workflow_pkg_id;
    //     let task_id = sui::types::Address::random();
    //     let owner = sui::types::Address::random_for_testing_only();
    //     let task_object = mock_task_object(workflow_pkg, task_id, owner);
    //     let get_task_call =
    //         sui_mocks::rpc::mock_read_api_get_object(&mut server, task_id, task_object);

    //     let digest = sui::TransactionDigest::random();
    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             digest,
    //             None,
    //             None,
    //             None,
    //             None,
    //         );

    //     let metadata = vec![
    //         ("region".into(), "us".into()),
    //         ("tier".into(), "gold".into()),
    //     ];
    //     let result = nexus_client
    //         .scheduler()
    //         .update_metadata(task_id, metadata.clone())
    //         .await
    //         .expect("metadata updated");

    //     get_task_call.assert_async().await;
    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert_eq!(result.tx_digest, digest);
    //     assert_eq!(result.entries, metadata.len());
    // }

    // #[tokio::test]
    // async fn test_scheduler_set_task_state_pause() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let workflow_pkg = nexus_client.nexus_objects.workflow_pkg_id;
    //     let task_id = sui::types::Address::random();
    //     let owner = sui::types::Address::random_for_testing_only();
    //     let task_object = mock_task_object(workflow_pkg, task_id, owner);
    //     let get_task_call =
    //         sui_mocks::rpc::mock_read_api_get_object(&mut server, task_id, task_object);

    //     let digest = sui::TransactionDigest::random();
    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             digest,
    //             None,
    //             None,
    //             None,
    //             None,
    //         );

    //     let result = nexus_client
    //         .scheduler()
    //         .set_task_state(task_id, TaskStateAction::Pause)
    //         .await
    //         .expect("state updated");

    //     get_task_call.assert_async().await;
    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert_eq!(result.tx_digest, digest);
    //     assert!(matches!(result.state, TaskStateAction::Pause));
    // }

    // #[tokio::test]
    // async fn test_scheduler_add_occurrence_absolute() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let workflow_pkg = nexus_client.nexus_objects.workflow_pkg_id;
    //     let task_id = sui::types::Address::random();
    //     let owner = sui::types::Address::random_for_testing_only();
    //     let task_object = mock_task_object(workflow_pkg, task_id, owner);
    //     let get_task_call =
    //         sui_mocks::rpc::mock_read_api_get_object(&mut server, task_id, task_object);

    //     let generator = queue_generator_symbol(workflow_pkg);
    //     let scheduled_event = NexusEventKind::Scheduled(RequestScheduledExecution {
    //         request: Box::new(NexusEventKind::OccurrenceScheduled(
    //             OccurrenceScheduledEvent {
    //                 task: task_id,
    //                 generator,
    //             },
    //         )),
    //         priority: 1,
    //         request_ms: 11,
    //         start_ms: 22,
    //         deadline_ms: 33,
    //     });

    //     let digest = sui::TransactionDigest::random();
    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             digest,
    //             None,
    //             Some(block_events(workflow_pkg, vec![scheduled_event.clone()])),
    //             None,
    //             None,
    //         );

    //     let request =
    //         OccurrenceRequest::new(Some(2_000), Some(2_500), None, None, 7, true).unwrap();

    //     let result = nexus_client
    //         .scheduler()
    //         .add_occurrence(task_id, request)
    //         .await
    //         .expect("occurrence enqueued");

    //     get_task_call.assert_async().await;
    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert_eq!(result.tx_digest, digest);
    //     assert!(matches!(result.event, Some(NexusEventKind::Scheduled(_))));
    // }

    // #[tokio::test]
    // async fn test_scheduler_add_occurrence_with_offsets() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let workflow_pkg = nexus_client.nexus_objects.workflow_pkg_id;
    //     let task_id = sui::types::Address::random();
    //     let owner = sui::types::Address::random_for_testing_only();
    //     let task_object = mock_task_object(workflow_pkg, task_id, owner);
    //     let get_task_call =
    //         sui_mocks::rpc::mock_read_api_get_object(&mut server, task_id, task_object);

    //     let digest = sui::TransactionDigest::random();
    //     let generator = queue_generator_symbol(workflow_pkg);
    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             digest,
    //             None,
    //             Some(block_events(
    //                 workflow_pkg,
    //                 vec![NexusEventKind::OccurrenceScheduled(
    //                     OccurrenceScheduledEvent {
    //                         task: task_id,
    //                         generator,
    //                     },
    //                 )],
    //             )),
    //             None,
    //             None,
    //         );

    //     let request =
    //         OccurrenceRequest::new(None, None, Some(500), Some(900), 4, true).expect("valid");

    //     let result = nexus_client
    //         .scheduler()
    //         .add_occurrence(task_id, request)
    //         .await
    //         .expect("occurrence enqueued");

    //     get_task_call.assert_async().await;
    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert!(matches!(
    //         result.event,
    //         Some(NexusEventKind::OccurrenceScheduled(_))
    //     ));
    // }

    // #[tokio::test]
    // async fn test_scheduler_configure_periodic() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let workflow_pkg = nexus_client.nexus_objects.workflow_pkg_id;
    //     let task_id = sui::types::Address::random();
    //     let owner = sui::types::Address::random_for_testing_only();
    //     let task_object = mock_task_object(workflow_pkg, task_id, owner);
    //     let get_task_call =
    //         sui_mocks::rpc::mock_read_api_get_object(&mut server, task_id, task_object);

    //     let digest = sui::TransactionDigest::random();
    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             digest,
    //             None,
    //             None,
    //             None,
    //             None,
    //         );

    //     let config = PeriodicScheduleConfig {
    //         first_start_ms: 10_000,
    //         period_ms: 5_000,
    //         deadline_offset_ms: Some(1_000),
    //         max_iterations: Some(5),
    //         gas_price: 20,
    //     };

    //     let result = nexus_client
    //         .scheduler()
    //         .configure_periodic(task_id, config)
    //         .await
    //         .expect("periodic configured");

    //     get_task_call.assert_async().await;
    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert_eq!(result.tx_digest, digest);
    // }

    // #[tokio::test]
    // async fn test_scheduler_disable_periodic() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let workflow_pkg = nexus_client.nexus_objects.workflow_pkg_id;
    //     let task_id = sui::types::Address::random();
    //     let owner = sui::types::Address::random_for_testing_only();
    //     let task_object = mock_task_object(workflow_pkg, task_id, owner);
    //     let get_task_call =
    //         sui_mocks::rpc::mock_read_api_get_object(&mut server, task_id, task_object);

    //     let digest = sui::TransactionDigest::random();
    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             digest,
    //             None,
    //             None,
    //             None,
    //             None,
    //         );

    //     let result = nexus_client
    //         .scheduler()
    //         .disable_periodic(task_id)
    //         .await
    //         .expect("periodic disabled");

    //     get_task_call.assert_async().await;
    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert_eq!(result.tx_digest, digest);
    // }

    // #[test]
    // fn test_occurrence_request_validation_rules() {
    //     assert!(OccurrenceRequest::new(Some(10), Some(20), None, None, 1, true).is_ok());
    //     assert!(OccurrenceRequest::new(None, None, Some(5), Some(15), 1, true).is_ok());

    //     let err = OccurrenceRequest::new(None, None, None, None, 1, true).unwrap_err();
    //     assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Provide either")));

    //     let err = OccurrenceRequest::new(Some(50), Some(40), None, None, 1, true).unwrap_err();
    //     assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Deadline")));

    //     let err = OccurrenceRequest::new(None, None, None, Some(10), 1, false).unwrap_err();
    //     assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Deadline flags")));
    // }

    // #[test]
    // fn test_extract_task_id_errors_without_event() {
    //     let workflow_pkg = sui::types::Address::random();
    //     let mut response = sui::TransactionBlockResponse::new(sui::TransactionDigest::random());
    //     let generator = queue_generator_symbol(workflow_pkg);
    //     response.events = Some(block_events(
    //         workflow_pkg,
    //         vec![NexusEventKind::OccurrenceScheduled(
    //             OccurrenceScheduledEvent {
    //                 task: sui::types::Address::random(),
    //                 generator,
    //             },
    //         )],
    //     ));

    //     let err = extract_task_id(&response).expect_err("missing event");
    //     assert!(matches!(err, NexusError::Parsing(_)));
    // }

    // #[test]
    // fn test_extract_occurrence_event_variants() {
    //     let workflow_pkg = sui::types::Address::random();
    //     let task_id = sui::types::Address::random();
    //     let periodic_generator = periodic_generator_symbol(workflow_pkg);
    //     let scheduled = NexusEventKind::Scheduled(RequestScheduledExecution {
    //         request: Box::new(NexusEventKind::OccurrenceScheduled(
    //             OccurrenceScheduledEvent {
    //                 task: task_id,
    //                 generator: periodic_generator.clone(),
    //             },
    //         )),
    //         priority: 1,
    //         request_ms: 10,
    //         start_ms: 11,
    //         deadline_ms: 12,
    //     });

    //     let mut response = sui::TransactionBlockResponse::new(sui::TransactionDigest::random());
    //     response.events = Some(block_events(workflow_pkg, vec![scheduled]));
    //     assert!(matches!(
    //         extract_occurrence_event(&response),
    //         Some(NexusEventKind::Scheduled(_))
    //     ));

    //     let mut response_direct =
    //         sui::TransactionBlockResponse::new(sui::TransactionDigest::random());
    //     let queue_generator = queue_generator_symbol(workflow_pkg);
    //     response_direct.events = Some(block_events(
    //         workflow_pkg,
    //         vec![NexusEventKind::OccurrenceScheduled(
    //             OccurrenceScheduledEvent {
    //                 task: task_id,
    //                 generator: queue_generator,
    //             },
    //         )],
    //     ));
    //     assert!(matches!(
    //         extract_occurrence_event(&response_direct),
    //         Some(NexusEventKind::OccurrenceScheduled(_))
    //     ));

    //     let empty_response = sui::TransactionBlockResponse::new(sui::TransactionDigest::random());
    //     assert!(extract_occurrence_event(&empty_response).is_none());
    // }
}
