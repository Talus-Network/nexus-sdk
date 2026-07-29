use {
    crate::{
        move_bindings::{
            scheduler::task::{Task, TaskController},
            sui_framework::clock::Clock,
        },
        move_boundary,
        nexus::{client::NexusClient, crawler::Response, tap},
        scheduler::{
            Deadline,
            Occurrence,
            Recurrence,
            Schedule,
            ScheduleError,
            SchedulerError,
            StartTime,
            TaskFunding,
            TaskSpec,
        },
        transactions::scheduler::{
            PreparedFunding,
            PreparedOccurrence,
            PreparedRecurrence,
            PreparedSchedule,
            PreparedTask,
            ResolvedAuthority,
        },
    },
    anyhow::Context as _,
};

pub(super) struct ResolvedTask {
    pub(super) object: Response<Task>,
    pub(super) authority: ResolvedAuthority,
}

pub(crate) async fn prepare_task(
    client: &NexusClient,
    task: &TaskSpec,
) -> Result<PreparedTask, SchedulerError> {
    task.validate()?;
    let sender = client.owner().map_err(SchedulerError::transport)?;
    let agent = match task.operation().agent_id() {
        Some(agent_id) => Some(agent_input(client, agent_id).await?),
        None => None,
    };
    let funding = match task.funding() {
        TaskFunding::Address {
            prepay_amount_mist,
            refund_recipient,
        } => PreparedFunding::Address {
            prepay_amount_mist,
            refund_recipient: refund_recipient.unwrap_or(sender),
        },
        TaskFunding::Agent { prepay_amount_mist } if agent.is_some() => {
            PreparedFunding::Agent { prepay_amount_mist }
        }
        TaskFunding::Agent { .. } => {
            return Err(ScheduleError::IncompatibleFunding {
                message: "a default DAG Task cannot use Agent vault funding",
            }
            .into());
        }
    };

    Ok(PreparedTask {
        operation: task.operation().clone(),
        agent,
        entry_group: task.entry_group().to_owned(),
        inputs: task.inputs().clone(),
        funding,
        occurrence_budget_mist: task.occurrence_budget_mist(),
        failure_policy: task.failure_policy(),
    })
}

pub(crate) async fn prepare_schedule(
    client: &NexusClient,
    schedule: &Schedule,
) -> Result<PreparedSchedule, SchedulerError> {
    schedule.validate()?;
    let clock_ms = if schedule_requires_clock(schedule) {
        Some(clock_timestamp_ms(client).await?)
    } else {
        None
    };
    let occurrences = schedule
        .occurrences()
        .iter()
        .map(|occurrence| resolve_occurrence(occurrence, clock_ms))
        .collect::<Result<Vec<_>, _>>()?;
    let recurrence = schedule
        .recurrence()
        .map(|recurrence| resolve_recurrence(recurrence, clock_ms))
        .transpose()?;

    Ok(PreparedSchedule::new(occurrences, recurrence))
}

pub(super) async fn prepare_occurrence(
    client: &NexusClient,
    occurrence: &Occurrence,
) -> Result<PreparedOccurrence, SchedulerError> {
    occurrence.validate()?;
    let clock_ms = if occurrence_requires_clock(occurrence) {
        Some(clock_timestamp_ms(client).await?)
    } else {
        None
    };
    resolve_occurrence(occurrence, clock_ms)
}

pub(super) async fn prepare_recurrence(
    client: &NexusClient,
    recurrence: &Recurrence,
) -> Result<PreparedRecurrence, SchedulerError> {
    recurrence.validate()?;
    let clock_ms = if occurrence_requires_clock(recurrence.first()) {
        Some(clock_timestamp_ms(client).await?)
    } else {
        None
    };
    resolve_recurrence(recurrence, clock_ms)
}

pub(super) async fn fetch_task(
    client: &NexusClient,
    task_id: crate::sui::types::Address,
) -> Result<Response<Task>, SchedulerError> {
    client
        .crawler()
        .get_optional_object::<Task>(task_id)
        .await
        .map_err(SchedulerError::transport)?
        .ok_or(SchedulerError::TaskNotFound { task_id })
}

pub(super) async fn resolve_task(
    client: &NexusClient,
    task_id: crate::sui::types::Address,
) -> Result<ResolvedTask, SchedulerError> {
    let object = fetch_task(client, task_id).await?;
    let authority = resolve_authority(client, &object).await?;
    Ok(ResolvedTask { object, authority })
}

async fn resolve_authority(
    client: &NexusClient,
    task: &Response<Task>,
) -> Result<ResolvedAuthority, SchedulerError> {
    match &task.data.controller {
        TaskController::Address { pos0 } => {
            let sender = client.owner().map_err(SchedulerError::transport)?;
            if *pos0 != sender {
                return Err(SchedulerError::AuthorityUnavailable {
                    task_id: task.object_id,
                    message: format!(
                        "controller address '{pos0}' differs from active address '{sender}'"
                    ),
                });
            }
            Ok(ResolvedAuthority::Address)
        }
        TaskController::Agent { pos0 } => Ok(ResolvedAuthority::Agent(
            agent_input(client, pos0.bytes).await?,
        )),
    }
}

async fn agent_input(
    client: &NexusClient,
    agent_id: crate::types::AgentId,
) -> Result<crate::transactions::agent_input::AgentInput, SchedulerError> {
    let metadata = client
        .crawler()
        .get_object_metadata(agent_id)
        .await
        .map_err(SchedulerError::transport)?;
    tap::agent_input_from_metadata(&metadata).map_err(|error| SchedulerError::InvalidObject {
        object_id: agent_id,
        message: error.to_string(),
    })
}

async fn clock_timestamp_ms(client: &NexusClient) -> Result<u64, SchedulerError> {
    client
        .crawler()
        .get_object::<Clock>(move_boundary::CLOCK_OBJECT_ID)
        .await
        .context("could not read the Sui Clock")
        .map(|clock| clock.data.timestamp_ms)
        .map_err(SchedulerError::transport)
}

fn schedule_requires_clock(schedule: &Schedule) -> bool {
    schedule.occurrences().iter().any(occurrence_requires_clock)
        || schedule
            .recurrence()
            .is_some_and(|recurrence| occurrence_requires_clock(recurrence.first()))
}

fn occurrence_requires_clock(occurrence: &Occurrence) -> bool {
    !matches!(occurrence.start(), StartTime::At { .. })
        || matches!(occurrence.deadline(), Some(Deadline::AfterStart { .. }))
}

fn resolve_recurrence(
    recurrence: &Recurrence,
    clock_ms: Option<u64>,
) -> Result<PreparedRecurrence, SchedulerError> {
    Ok(PreparedRecurrence::new(
        resolve_occurrence(recurrence.first(), clock_ms)?,
        recurrence.interval_ms(),
        recurrence.occurrences(),
    ))
}

fn resolve_occurrence(
    occurrence: &Occurrence,
    clock_ms: Option<u64>,
) -> Result<PreparedOccurrence, SchedulerError> {
    let start_time_ms = match occurrence.start() {
        StartTime::At { timestamp_ms } => timestamp_ms,
        StartTime::Now => required_clock(clock_ms)?,
        StartTime::After { offset_ms } => {
            checked_time("occurrence start", required_clock(clock_ms)?, offset_ms)?
        }
    };
    let deadline_ms = match occurrence.deadline() {
        None => None,
        Some(Deadline::At { timestamp_ms }) => {
            if timestamp_ms < start_time_ms {
                return Err(ScheduleError::DeadlineBeforeStart {
                    start_time_ms,
                    deadline_ms: timestamp_ms,
                }
                .into());
            }
            Some(timestamp_ms)
        }
        Some(Deadline::AfterStart { offset_ms }) => Some(checked_time(
            "occurrence deadline",
            start_time_ms,
            offset_ms,
        )?),
    };

    Ok(PreparedOccurrence::new(
        start_time_ms,
        deadline_ms,
        occurrence.priority_fee_percentage(),
    ))
}

fn required_clock(clock_ms: Option<u64>) -> Result<u64, SchedulerError> {
    clock_ms.ok_or_else(|| SchedulerError::InconsistentChainState {
        message: "relative schedule preparation omitted its Clock snapshot".to_owned(),
    })
}

fn checked_time(field: &'static str, base_ms: u64, offset_ms: u64) -> Result<u64, SchedulerError> {
    base_ms.checked_add(offset_ms).ok_or_else(|| {
        ScheduleError::TimeOverflow {
            field,
            base_ms,
            offset_ms,
        }
        .into()
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            nexus::client::AddressBalanceGas,
            scheduler::Occurrence,
            test_utils::sui_mocks,
            transactions::scheduler::{PreparedOccurrence, PreparedRecurrence, PreparedSchedule},
            types::DEFAULT_PRIORITY_FEE_PERCENTAGE,
        },
    };

    async fn test_client(mocks: sui_mocks::grpc::ServerMocks) -> NexusClient {
        let rpc_url = sui_mocks::grpc::mock_server(mocks);
        let private_key = crate::sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());

        NexusClient::builder()
            .with_private_key(private_key)
            .with_rpc_url(&rpc_url)
            .with_address_balance_gas_config(AddressBalanceGas::new(1_000))
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client builds")
    }

    #[tokio::test]
    async fn mixed_schedule_uses_one_clock_snapshot() {
        let clock_ms = 1_000;
        let clock_ref = sui_mocks::object_ref_for_id(move_boundary::CLOCK_OBJECT_ID);
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            clock_ref,
            crate::sui::types::Owner::Shared(1),
            bcs::to_bytes(&Clock::new(move_boundary::CLOCK_OBJECT_ID, clock_ms))
                .expect("Clock serializes"),
        );
        let client = test_client(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        })
        .await;
        let recurrence = Recurrence::new(Occurrence::after_ms(30), 100)
            .expect("recurrence is valid")
            .finite(2)
            .expect("recurrence count is valid");
        let schedule = Schedule::new()
            .with_occurrence(Occurrence::now())
            .with_occurrence(Occurrence::after_ms(20).deadline_after_ms(5))
            .with_occurrence(Occurrence::at_ms(2_000))
            .with_recurrence(recurrence);

        let prepared = prepare_schedule(&client, &schedule)
            .await
            .expect("Schedule resolves");

        assert_eq!(
            prepared,
            PreparedSchedule::new(
                vec![
                    PreparedOccurrence::new(clock_ms, None, DEFAULT_PRIORITY_FEE_PERCENTAGE,),
                    PreparedOccurrence::new(
                        clock_ms + 20,
                        Some(clock_ms + 25),
                        DEFAULT_PRIORITY_FEE_PERCENTAGE,
                    ),
                    PreparedOccurrence::new(2_000, None, DEFAULT_PRIORITY_FEE_PERCENTAGE,),
                ],
                Some(PreparedRecurrence::new(
                    PreparedOccurrence::new(clock_ms + 30, None, DEFAULT_PRIORITY_FEE_PERCENTAGE,),
                    100,
                    Some(2),
                )),
            )
        );
    }

    #[tokio::test]
    async fn absolute_schedule_preparation_does_not_read_the_clock() {
        let client = test_client(sui_mocks::grpc::ServerMocks::default()).await;
        let occurrence = Occurrence::at_ms(2_000)
            .deadline_at_ms(2_500)
            .expect("deadline follows start");
        let recurrence =
            Recurrence::new(Occurrence::at_ms(3_000), 200).expect("recurrence is valid");
        let schedule = Schedule::new()
            .with_occurrence(occurrence)
            .with_recurrence(recurrence.clone());

        assert_eq!(
            prepare_schedule(&client, &schedule)
                .await
                .expect("absolute schedule resolves"),
            PreparedSchedule::new(
                vec![PreparedOccurrence::new(
                    2_000,
                    Some(2_500),
                    DEFAULT_PRIORITY_FEE_PERCENTAGE,
                )],
                Some(PreparedRecurrence::new(
                    PreparedOccurrence::new(3_000, None, DEFAULT_PRIORITY_FEE_PERCENTAGE,),
                    200,
                    None,
                )),
            )
        );
        assert_eq!(
            prepare_occurrence(&client, &occurrence)
                .await
                .expect("absolute occurrence resolves"),
            PreparedOccurrence::new(2_000, Some(2_500), DEFAULT_PRIORITY_FEE_PERCENTAGE,)
        );
        assert_eq!(
            prepare_recurrence(&client, &recurrence)
                .await
                .expect("absolute recurrence resolves"),
            PreparedRecurrence::new(
                PreparedOccurrence::new(3_000, None, DEFAULT_PRIORITY_FEE_PERCENTAGE),
                200,
                None,
            )
        );
    }

    #[tokio::test]
    async fn address_funded_task_preparation_preserves_authored_values() {
        let client = test_client(sui_mocks::grpc::ServerMocks::default()).await;
        let dag_id = crate::sui::types::Address::from_static("0x31");
        let refund_recipient = crate::sui::types::Address::from_static("0x32");
        let task = TaskSpec::new(
            crate::scheduler::TaskOperation::default_dag(dag_id),
            "main",
            TaskFunding::address_with_refund(90, refund_recipient),
            30,
        )
        .expect("Task is valid")
        .with_failure_policy(crate::scheduler::FailurePolicy::Pause);

        let prepared = prepare_task(&client, &task)
            .await
            .expect("Task preparation succeeds");

        assert!(matches!(
            prepared.operation,
            crate::scheduler::TaskOperation::DefaultDag {
                dag_id: stored_dag,
            } if stored_dag == dag_id
        ));
        assert!(prepared.agent.is_none());
        assert_eq!(prepared.entry_group, "main");
        assert!(prepared.inputs.is_empty());
        assert_eq!(
            prepared.funding,
            PreparedFunding::Address {
                prepay_amount_mist: 90,
                refund_recipient,
            }
        );
        assert_eq!(prepared.occurrence_budget_mist, 30);
        assert_eq!(
            prepared.failure_policy,
            crate::scheduler::FailurePolicy::Pause
        );
    }

    #[test]
    fn relative_time_resolution_reports_missing_clock_and_overflow() {
        assert!(matches!(
            resolve_occurrence(&Occurrence::now(), None),
            Err(SchedulerError::InconsistentChainState { .. })
        ));
        assert!(matches!(
            resolve_occurrence(&Occurrence::after_ms(1), Some(u64::MAX)),
            Err(SchedulerError::Schedule(ScheduleError::TimeOverflow {
                field: "occurrence start",
                ..
            }))
        ));
        assert!(matches!(
            resolve_occurrence(&Occurrence::at_ms(u64::MAX).deadline_after_ms(1), None,),
            Err(SchedulerError::Schedule(ScheduleError::TimeOverflow {
                field: "occurrence deadline",
                ..
            }))
        ));
        assert!(matches!(
            resolve_occurrence(
                &Occurrence::after_ms(10)
                    .deadline_at_ms(5)
                    .expect("relative start is resolved later"),
                Some(100),
            ),
            Err(SchedulerError::Schedule(
                ScheduleError::DeadlineBeforeStart {
                    start_time_ms: 110,
                    deadline_ms: 5,
                }
            ))
        ));
    }
}
