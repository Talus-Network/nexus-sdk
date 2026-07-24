mod occurrence_abort_expired;
mod occurrence_add;
mod occurrence_cost;
mod occurrence_inspect;

use crate::prelude::*;

#[derive(Args, Debug, Clone)]
#[group(id = "occurrence-start", multiple = false)]
pub(crate) struct OccurrenceStartOptions {
    /// Absolute start time in milliseconds since epoch.
    #[arg(long = "start-ms", value_name = "MILLIS")]
    start_ms: Option<u64>,
    /// Start offset in milliseconds from the current chain time.
    #[arg(long = "start-offset-ms", value_name = "MILLIS")]
    start_offset_ms: Option<u64>,
}

#[derive(Subcommand)]
pub(crate) enum OccurrenceCommand {
    #[command(about = "Add a standalone occurrence to a Task")]
    Add {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        start: OccurrenceStartOptions,
        /// Deadline offset from the occurrence start.
        #[arg(long = "deadline-offset-ms", value_name = "MILLIS")]
        deadline_offset_ms: Option<u64>,
        /// Priority fee percentage applied to the occurrence.
        #[arg(long = "priority-fee-percentage", value_name = "PERCENTAGE")]
        priority_fee_percentage: Option<u64>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Expire an advertised occurrence after its deadline")]
    Expire {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long = "occurrence-id", value_name = "U64")]
        occurrence_id: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Inspect an occurrence lifecycle")]
    Inspect {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long = "occurrence-id", value_name = "U64")]
        occurrence_id: u64,
        /// Follow the occurrence until scheduler processing is complete.
        #[arg(long = "follow")]
        follow: bool,
    },
    #[command(about = "Show the payment cost for a dispatched occurrence")]
    Cost {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long = "occurrence-id", value_name = "U64")]
        occurrence_id: u64,
    },
    #[command(about = "Abort expired runtime work for an occurrence")]
    AbortExpired {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long = "occurrence-id", value_name = "U64")]
        occurrence_id: u64,
        /// Optional ToolGas object required for the abort.
        #[arg(long = "tool-gas-id", value_name = "OBJECT_ID")]
        tool_gas_id: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: OccurrenceCommand) -> AnyResult<(), NexusCliError> {
    match command {
        OccurrenceCommand::Add {
            task_id,
            start,
            deadline_offset_ms,
            priority_fee_percentage,
            gas,
        } => {
            occurrence_add::add_occurrence_to_task(
                task_id,
                start.start_ms,
                start.start_offset_ms,
                deadline_offset_ms,
                priority_fee_percentage,
                gas,
            )
            .await
        }
        OccurrenceCommand::Expire {
            task_id,
            occurrence_id,
            gas,
        } => occurrence_add::expire_occurrence(task_id, occurrence_id, gas).await,
        OccurrenceCommand::Inspect {
            task_id,
            occurrence_id,
            follow,
        } => occurrence_inspect::inspect_occurrence(task_id, occurrence_id, follow).await,
        OccurrenceCommand::Cost {
            task_id,
            occurrence_id,
        } => occurrence_cost::occurrence_cost(task_id, occurrence_id).await,
        OccurrenceCommand::AbortExpired {
            task_id,
            occurrence_id,
            tool_gas_id,
            gas,
        } => {
            occurrence_abort_expired::abort_expired_occurrence(
                task_id,
                occurrence_id,
                tool_gas_id,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn inspect_parses_occurrence_reference_and_follow() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "schedule",
            "occurrence",
            "inspect",
            "--task-id",
            "0x42",
            "--occurrence-id",
            "7",
            "--follow",
        ])
        .expect("occurrence inspection arguments should parse");

        let crate::Command::Schedule(crate::schedule::ScheduleCommand::Occurrence(
            OccurrenceCommand::Inspect {
                task_id,
                occurrence_id,
                follow,
            },
        )) = cli.command
        else {
            panic!("expected schedule occurrence inspect command");
        };

        assert_eq!(task_id, sui::types::Address::from_static("0x42"));
        assert_eq!(occurrence_id, 7);
        assert!(follow);
    }

    #[test]
    fn abort_expired_parses_occurrence_reference() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "schedule",
            "occurrence",
            "abort-expired",
            "--task-id",
            "0x42",
            "--occurrence-id",
            "7",
        ])
        .expect("occurrence abort arguments should parse");

        assert!(matches!(
            cli.command,
            crate::Command::Schedule(crate::schedule::ScheduleCommand::Occurrence(
                OccurrenceCommand::AbortExpired {
                    occurrence_id: 7,
                    ..
                }
            ))
        ));
    }
}
