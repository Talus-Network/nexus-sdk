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
    pub execution_priority_fee_per_gas_unit: u64,
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
    pub priority_fee_per_gas_unit: u64,
}

impl OccurrenceRequest {
    pub fn new(
        start_ms: Option<u64>,
        deadline_ms: Option<u64>,
        start_offset_ms: Option<u64>,
        deadline_offset_ms: Option<u64>,
        priority_fee_per_gas_unit: u64,
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
            priority_fee_per_gas_unit,
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
    pub priority_fee_per_gas_unit: u64,
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
            execution_priority_fee_per_gas_unit,
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
            execution_priority_fee_per_gas_unit,
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
        let task_ref = task.object_ref();

        let metadata_arg =
            scheduler_tx::new_metadata(&mut tx, objects, metadata.clone().into_iter())
                .map_err(NexusError::TransactionBuilding)?;

        scheduler_tx::update_metadata(&mut tx, objects, &task_ref, metadata_arg)
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
        let task_ref = task.object_ref();

        match request {
            TaskStateAction::Pause => {
                scheduler_tx::pause_time_constraint_for_task(&mut tx, objects, &task_ref)
            }
            TaskStateAction::Resume => {
                scheduler_tx::resume_time_constraint_for_task(&mut tx, objects, &task_ref)
            }
            TaskStateAction::Cancel => {
                scheduler_tx::cancel_time_constraint_for_task(&mut tx, objects, &task_ref)
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
        let task_ref = task.object_ref();

        scheduler_tx::new_or_modify_periodic_for_task(
            &mut tx,
            objects,
            &task_ref,
            scheduler_tx::PeriodicScheduleInputs {
                first_start_ms: config.first_start_ms,
                period_ms: config.period_ms,
                deadline_offset_ms: config.deadline_offset_ms,
                max_iterations: config.max_iterations,
                priority_fee_per_gas_unit: config.priority_fee_per_gas_unit,
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
        let task_ref = task.object_ref();

        scheduler_tx::disable_periodic_for_task(&mut tx, objects, &task_ref)
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
        let task_ref = task.object_ref();

        if let Some(start_ms) = request.start_ms {
            let deadline_offset = request
                .deadline_offset_ms
                .or_else(|| request.deadline_ms.map(|deadline| deadline - start_ms));

            scheduler_tx::add_occurrence_absolute_for_task(
                &mut tx,
                objects,
                &task_ref,
                start_ms,
                deadline_offset,
                request.priority_fee_per_gas_unit,
            )
        } else {
            scheduler_tx::add_occurrence_relative_for_task(
                &mut tx,
                objects,
                &task_ref,
                request.start_offset_ms.expect("validated start offset"),
                request.deadline_offset_ms,
                request.priority_fee_per_gas_unit,
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
    response.events.iter().find_map(|event| match &event.data {
        NexusEventKind::RequestScheduledOccurrence(_) => Some(event.data.clone()),
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
    use {
        super::*,
        crate::{
            events::{
                NexusEvent,
                NexusEventKind,
                OccurrenceScheduledEvent,
                RequestScheduledOccurrenceEvent,
                TaskCreatedEvent,
            },
            nexus::{
                client::NexusClient,
                crawler::{Bag, Map, ObjectBag, TableVec},
                error::NexusError,
                signer::ExecutedTransaction,
            },
            sui,
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                ConfiguredAutomaton,
                ConstraintsData,
                DataStorage,
                DeterministicAutomaton,
                ExecutionData,
                Metadata,
                NexusData,
                NexusObjects,
                Policy,
                PolicySymbol,
                TaskState,
                TypeName,
            },
        },
        rand::thread_rng,
        serde::Serialize,
        serde_json::json,
    };

    fn dummy_executed_transaction(events: Vec<NexusEvent>) -> ExecutedTransaction {
        let mut rng = thread_rng();

        ExecutedTransaction {
            effects: sui::types::TransactionEffectsV2 {
                status: sui::types::ExecutionStatus::Success,
                epoch: 0,
                gas_used: sui::types::GasCostSummary::new(0, 0, 0, 0),
                transaction_digest: sui::types::Digest::generate(&mut rng),
                gas_object_index: None,
                events_digest: None,
                dependencies: vec![],
                lamport_version: 0,
                changed_objects: vec![],
                unchanged_consensus_objects: vec![],
                auxiliary_data_digest: None,
            },
            events,
            objects: vec![],
            digest: sui::types::Digest::generate(&mut rng),
            checkpoint: 0,
        }
    }
    fn sample_input_data() -> HashMap<String, HashMap<String, DataStorage>> {
        HashMap::from([(
            "entry_vertex".to_string(),
            HashMap::from([(
                "entry_port".to_string(),
                NexusData::new_inline(json!("payload")).commit_inline_plain(),
            )]),
        )])
    }

    fn mock_task_object(
        task_id: sui::types::Address,
        owner: sui::types::Address,
    ) -> serde_json::Value {
        let metadata = Metadata {
            values: Map::from_iter([("initial".to_string(), "value".to_string())]),
        };

        let dfa = DeterministicAutomaton {
            states: TableVec::new(sui::types::Address::from_static("0x100"), 1),
            alphabet: TableVec::new(sui::types::Address::from_static("0x101"), 0),
            transition: TableVec::new(sui::types::Address::from_static("0x102"), 1),
            accepting: TableVec::new(sui::types::Address::from_static("0x103"), 1),
            start: 0,
        };

        let constraints = Policy {
            id: sui::types::Address::from_static("0x400"),
            dfa: ConfiguredAutomaton {
                id: sui::types::Address::from_static("0x401"),
                dfa: dfa.clone(),
            },
            state_index: 0,
            data: ConstraintsData::default(),
        };

        let execution = Policy {
            id: sui::types::Address::from_static("0x500"),
            dfa: ConfiguredAutomaton {
                id: sui::types::Address::from_static("0x600"),
                dfa,
            },
            state_index: 0,
            data: ExecutionData::default(),
        };

        let task = Task {
            id: task_id,
            owner,
            metadata,
            constraints,
            execution,
            state: TaskState::Active,
            data: Bag::new(sui::types::Address::from_static("0x700"), 0),
            objects: ObjectBag::new(sui::types::Address::from_static("0x701"), 0),
        };

        serde_json::to_value(task).expect("serialize task")
    }

    fn generator_symbol(workflow_pkg_id: sui::types::Address, name: &str) -> PolicySymbol {
        PolicySymbol::Witness(TypeName::new(&format!(
            "{workflow_pkg_id}::scheduler::{name}"
        )))
    }

    fn queue_generator_symbol(workflow_pkg_id: sui::types::Address) -> PolicySymbol {
        generator_symbol(workflow_pkg_id, "QueueGeneratorWitness")
    }

    fn event_bcs(
        primitives_pkg: sui::types::Address,
        workflow_pkg: sui::types::Address,
        kind: NexusEventKind,
    ) -> sui::types::Event {
        #[derive(Serialize)]
        struct Wrapper<'a, T: Serialize + ?Sized> {
            event: &'a T,
        }

        let event_type = sui::types::StructTag::new(
            workflow_pkg,
            sui::types::Identifier::from_static("scheduler"),
            sui::types::Identifier::new(kind.name()).unwrap(),
            vec![],
        );

        let wrapper_tag = sui::types::StructTag::new(
            primitives_pkg,
            crate::idents::primitives::Event::EVENT_WRAPPER.module,
            crate::idents::primitives::Event::EVENT_WRAPPER.name,
            vec![sui::types::TypeTag::Struct(Box::new(event_type))],
        );

        let bcs = match &kind {
            NexusEventKind::RequestScheduledOccurrence(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::RequestScheduledWalk(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::OccurrenceScheduled(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::TaskCreated(ev) => bcs::to_bytes(&Wrapper { event: ev }).unwrap(),
            NexusEventKind::TaskPaused(ev) => bcs::to_bytes(&Wrapper { event: ev }).unwrap(),
            NexusEventKind::TaskResumed(ev) => bcs::to_bytes(&Wrapper { event: ev }).unwrap(),
            NexusEventKind::TaskCanceled(ev) => bcs::to_bytes(&Wrapper { event: ev }).unwrap(),
            _ => bcs::to_bytes(&Wrapper { event: &kind }).unwrap(),
        };

        sui_mocks::mock_sui_event(primitives_pkg, wrapper_tag, bcs)
    }

    async fn mock_nexus_client_with_server(
        ledger_service_mock: sui_mocks::grpc::MockLedgerService,
        execution_service_mock: sui_mocks::grpc::MockTransactionExecutionService,
        subscription_service_mock: sui_mocks::grpc::MockSubscriptionService,
        nexus_objects: NexusObjects,
    ) -> (String, NexusClient) {
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(execution_service_mock),
            subscription_service_mock: Some(subscription_service_mock),
            ..Default::default()
        });

        let nexus_client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

        (rpc_url, nexus_client)
    }

    #[tokio::test]
    async fn test_scheduler_create_task_without_initial_schedule() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let owner = sui::types::Address::generate(&mut rng);
        let task_id = sui::types::Address::generate(&mut rng);
        let dag_id = sui::types::Address::generate(&mut rng);

        let events = vec![event_bcs(
            nexus_objects.primitives_pkg_id,
            nexus_objects.workflow_pkg_id,
            NexusEventKind::TaskCreated(TaskCreatedEvent {
                task: task_id,
                owner,
            }),
        )];

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            events,
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let params = CreateTaskParams {
            dag_id,
            entry_group: "entry".into(),
            input_data: sample_input_data(),
            metadata: vec![("team".into(), "sdk".into())],
            execution_priority_fee_per_gas_unit: 1,
            initial_schedule: None,
            generator: GeneratorKind::Queue,
        };

        let result = nexus_client
            .scheduler()
            .create_task(params)
            .await
            .expect("task created");

        assert_eq!(result.task_id, task_id);
        assert_eq!(result.tx_digest, digest);
        assert!(result.initial_schedule.is_none());
    }

    #[tokio::test]
    async fn test_scheduler_create_task_with_initial_schedule() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let creation_digest = sui::types::Digest::generate(&mut rng);
        let schedule_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let owner = sui::types::Address::generate(&mut rng);
        let task_id = sui::types::Address::generate(&mut rng);
        let dag_id = sui::types::Address::generate(&mut rng);

        let creation_events = vec![event_bcs(
            nexus_objects.primitives_pkg_id,
            nexus_objects.workflow_pkg_id,
            NexusEventKind::TaskCreated(TaskCreatedEvent {
                task: task_id,
                owner,
            }),
        )];

        // Creation TX
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            creation_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            creation_events,
        );

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        // Scheduled occurrence event
        let queue_generator = queue_generator_symbol(nexus_objects.workflow_pkg_id);
        let scheduled_event =
            NexusEventKind::RequestScheduledOccurrence(RequestScheduledOccurrenceEvent {
                request: OccurrenceScheduledEvent {
                    task: task_id,
                    generator: queue_generator.clone(),
                },
                priority: 10,
                request_ms: 100,
                start_ms: 200,
                deadline_ms: 300,
            });

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            schedule_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![event_bcs(
                nexus_objects.primitives_pkg_id,
                nexus_objects.workflow_pkg_id,
                scheduled_event.clone(),
            )],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let initial_schedule =
            OccurrenceRequest::new(Some(1_000), Some(2_000), None, None, 5, true)
                .expect("valid request");

        let params = CreateTaskParams {
            dag_id,
            entry_group: "entry".into(),
            input_data: sample_input_data(),
            metadata: vec![],
            execution_priority_fee_per_gas_unit: 5,
            initial_schedule: Some(initial_schedule),
            generator: GeneratorKind::Queue,
        };

        let result = nexus_client
            .scheduler()
            .create_task(params)
            .await
            .expect("task created");

        assert_eq!(result.task_id, task_id);
        let schedule = result.initial_schedule.expect("schedule created");
        assert_eq!(schedule.tx_digest, schedule_digest);
        assert!(matches!(
            schedule.event,
            Some(NexusEventKind::RequestScheduledOccurrence(env))
                if env.request.task == task_id && env.priority == 10 && env.start_ms == 200
        ));
    }

    #[tokio::test]
    async fn test_scheduler_update_metadata() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let metadata = vec![
            ("region".into(), "us".into()),
            ("tier".into(), "gold".into()),
        ];
        let result = nexus_client
            .scheduler()
            .update_metadata(task_id, metadata.clone())
            .await
            .expect("metadata updated");

        assert_eq!(result.tx_digest, digest);
        assert_eq!(result.entries, metadata.len());
    }

    #[tokio::test]
    async fn test_scheduler_set_task_state_pause() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let result = nexus_client
            .scheduler()
            .set_task_state(task_id, TaskStateAction::Pause)
            .await
            .expect("state updated");

        assert_eq!(result.tx_digest, digest);
        assert!(matches!(result.state, TaskStateAction::Pause));
    }

    #[tokio::test]
    async fn test_scheduler_add_occurrence_absolute() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        let generator = queue_generator_symbol(nexus_objects.workflow_pkg_id);
        let scheduled_event = NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
            task: task_id,
            generator: generator.clone(),
        });

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![event_bcs(
                nexus_objects.primitives_pkg_id,
                nexus_objects.workflow_pkg_id,
                scheduled_event.clone(),
            )],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let request =
            OccurrenceRequest::new(Some(2_000), Some(2_500), None, None, 7, true).unwrap();

        let result = nexus_client
            .scheduler()
            .add_occurrence(task_id, request)
            .await
            .expect("occurrence enqueued");

        assert_eq!(result.tx_digest, digest);
        assert!(matches!(
            result.event,
            Some(NexusEventKind::OccurrenceScheduled(_))
        ));
    }

    #[tokio::test]
    async fn test_scheduler_add_occurrence_with_offsets() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        let generator = queue_generator_symbol(nexus_objects.workflow_pkg_id);
        let scheduled_event = NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
            task: task_id,
            generator: generator.clone(),
        });

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![event_bcs(
                nexus_objects.primitives_pkg_id,
                nexus_objects.workflow_pkg_id,
                scheduled_event.clone(),
            )],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let request =
            OccurrenceRequest::new(None, None, Some(500), Some(900), 4, true).expect("valid");

        let result = nexus_client
            .scheduler()
            .add_occurrence(task_id, request)
            .await
            .expect("occurrence enqueued");

        assert!(matches!(
            result.event,
            Some(NexusEventKind::OccurrenceScheduled(_))
        ));
    }

    #[tokio::test]
    async fn test_scheduler_configure_periodic() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let config = PeriodicScheduleConfig {
            first_start_ms: 10_000,
            period_ms: 5_000,
            deadline_offset_ms: Some(1_000),
            max_iterations: Some(5),
            priority_fee_per_gas_unit: 20,
        };

        let result = nexus_client
            .scheduler()
            .configure_periodic(task_id, config)
            .await
            .expect("periodic configured");

        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn test_scheduler_disable_periodic() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let result = nexus_client
            .scheduler()
            .disable_periodic(task_id)
            .await
            .expect("periodic disabled");

        assert_eq!(result.tx_digest, digest);
    }

    #[test]
    fn test_occurrence_request_validation_rules() {
        assert!(OccurrenceRequest::new(Some(10), Some(20), None, None, 1, true).is_ok());
        assert!(OccurrenceRequest::new(None, None, Some(5), Some(15), 1, true).is_ok());

        let err = OccurrenceRequest::new(None, None, None, None, 1, true).unwrap_err();
        assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Provide either")));

        let err = OccurrenceRequest::new(Some(50), Some(40), None, None, 1, true).unwrap_err();
        assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Deadline")));

        let err = OccurrenceRequest::new(None, None, None, Some(10), 1, false).unwrap_err();
        assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Deadline flags")));
    }

    #[test]
    fn test_extract_task_id_errors_without_event() {
        let mut rng = thread_rng();
        let workflow_pkg = sui::types::Address::generate(&mut rng);
        let response = dummy_executed_transaction(vec![NexusEvent {
            id: (sui::types::Digest::generate(&mut rng), 0),
            generics: vec![],
            data: NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
                task: sui::types::Address::generate(&mut rng),
                generator: PolicySymbol::Uid(workflow_pkg),
            }),
        }]);

        let err = extract_task_id(&response).expect_err("missing event");
        assert!(matches!(err, NexusError::Parsing(_)));
    }

    #[test]
    fn test_extract_occurrence_event_variants() {
        let mut rng = thread_rng();
        let workflow_pkg = sui::types::Address::generate(&mut rng);
        let task_id = sui::types::Address::generate(&mut rng);

        let scheduled =
            NexusEventKind::RequestScheduledOccurrence(RequestScheduledOccurrenceEvent {
                request: OccurrenceScheduledEvent {
                    task: task_id,
                    generator: PolicySymbol::Uid(workflow_pkg),
                },
                priority: 1,
                request_ms: 10,
                start_ms: 11,
                deadline_ms: 12,
            });

        let response = dummy_executed_transaction(vec![NexusEvent {
            id: (sui::types::Digest::generate(&mut rng), 0),
            generics: vec![],
            data: scheduled.clone(),
        }]);
        assert!(matches!(
            extract_occurrence_event(&response),
            Some(NexusEventKind::RequestScheduledOccurrence(_))
        ));

        let direct = dummy_executed_transaction(vec![NexusEvent {
            id: (sui::types::Digest::generate(&mut rng), 0),
            generics: vec![],
            data: NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
                task: task_id,
                generator: PolicySymbol::Uid(workflow_pkg),
            }),
        }]);
        assert!(matches!(
            extract_occurrence_event(&direct),
            Some(NexusEventKind::OccurrenceScheduled(_))
        ));

        let empty = dummy_executed_transaction(vec![]);
        assert!(extract_occurrence_event(&empty).is_none());
    }
}
