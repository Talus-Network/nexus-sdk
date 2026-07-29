mod args;
mod create;
mod list;
mod occurrence;
mod recurrence;
mod schedule;
mod state;

use {
    self::args::{OccurrenceArgs, RecurrenceArgs, ScheduleArgs, TaskArgs},
    crate::prelude::*,
};

const TASK_HELP: &str = r#"Task -> Schedule -> Occurrence

A Task owns reusable work, funding, controller authority, and one Schedule.
A Schedule contains standalone occurrences and at most one recurrence.
An Occurrence is one scheduling opportunity and has a permanent record under
its Task.

Commands:
  task create    Creates an empty Task for later composition.
  task schedule  Creates and schedules atomically in one transaction.
  task list      Lists Tasks discoverable through owned TaskPointer objects.

Use --now to schedule at the current Sui Clock time. Task inspection and
occurrence inspection read durable object state.

Examples:
  nexus task create --dag-id 0x42 \
    --prepay-amount-mist 50000000 \
    --occurrence-budget-mist 50000000
  nexus task schedule --dag-id 0x42 \
    --prepay-amount-mist 50000000 \
    --occurrence-budget-mist 50000000 --now
  nexus task schedule --dag-id 0x42 \
    --prepay-amount-mist 50000000 \
    --occurrence-budget-mist 50000000 \
    --now --at-ms 2000000000000 \
    --recurrence-interval-ms 60000
  nexus task schedule --dag-id 0x42 \
    --prepay-amount-mist 50000000 \
    --occurrence-budget-mist 50000000 \
    --schedule-file schedule.json
  nexus task list --limit 50
  nexus task inspect --task-id 0x123
  nexus task occurrence list --task-id 0x123 --json
  nexus task occurrence inspect --task-id 0x123 --occurrence-id 7"#;

const CREATE_HELP: &str = r#"Creates and shares an empty Task. No occurrence is allocated.

Outcome:
  The command returns a Task receipt. Add work later with
  `task occurrence add` or `task recurrence set`.

Example:
  nexus task create --dag-id 0x42 \
    --prepay-amount-mist 50000000 \
    --occurrence-budget-mist 50000000"#;

const SCHEDULE_HELP: &str = r#"Creates a Task, applies a complete nonempty Schedule, and shares the Task in
one transaction. At least one standalone occurrence or recurrence is required.

Outcome:
  The command returns the Task identifier, transaction reference, and every
  occurrence allocated by the transaction.

Examples:
  nexus task schedule --dag-id 0x42 \
    --prepay-amount-mist 50000000 \
    --occurrence-budget-mist 50000000 --now
  nexus task schedule --dag-id 0x42 \
    --prepay-amount-mist 50000000 \
    --occurrence-budget-mist 50000000 \
    --after-ms 30000 \
    --deadline-after-ms 60000 --priority-fee-percentage 20
  nexus task schedule --dag-id 0x42 \
    --prepay-amount-mist 50000000 \
    --occurrence-budget-mist 50000000 \
    --recurrence-interval-ms 60000 --recurrence-occurrences 10"#;

/// Commands for creating, scheduling, mutating, and inspecting Tasks.
#[derive(Subcommand)]
#[command(about = "Create and operate scheduled Tasks", long_about = TASK_HELP)]
pub(crate) enum TaskCommand {
    #[command(
        about = "Create and share an empty Task",
        long_about = CREATE_HELP
    )]
    Create(Box<CreateTaskArgs>),

    #[command(
        about = "Create a Task with a complete Schedule",
        long_about = SCHEDULE_HELP
    )]
    Schedule(Box<ScheduleTaskArgs>),

    #[command(about = "List Tasks owned by the configured signer")]
    List {
        #[arg(
            long,
            value_name = "HEX",
            help = "Opaque cursor returned by the previous page"
        )]
        cursor: Option<String>,
        #[arg(
            long,
            value_name = "COUNT",
            default_value_t = 50,
            value_parser = parse_page_limit,
            help = "Maximum TaskPointer objects read from one RPC page"
        )]
        limit: usize,
    },

    #[command(about = "Inspect current Task object state")]
    Inspect {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
    },

    #[command(about = "Pause future Task dispatch")]
    Pause {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Resume retained Task work")]
    Resume {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Cancel future Task work")]
    Cancel {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Add MIST to the Task reserve")]
    Refill {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long, value_name = "MIST")]
        amount_mist: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Release live Task resources")]
    Close {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(subcommand, about = "Operate permanent Task occurrences")]
    Occurrence(OccurrenceCommand),

    #[command(subcommand, about = "Set or clear the Task recurrence")]
    Recurrence(RecurrenceCommand),
}

#[derive(Args)]
pub(crate) struct CreateTaskArgs {
    #[command(flatten)]
    task: TaskArgs,
    #[command(flatten)]
    gas: GasArgs,
}

#[derive(Args)]
pub(crate) struct ScheduleTaskArgs {
    #[command(flatten)]
    task: TaskArgs,
    #[command(flatten)]
    schedule: ScheduleArgs,
    #[command(flatten)]
    gas: GasArgs,
}

/// Commands for standalone occurrence records.
#[derive(Subcommand)]
pub(crate) enum OccurrenceCommand {
    #[command(about = "List permanent occurrence records")]
    List {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(
            long,
            value_name = "HEX",
            help = "Opaque cursor returned by the previous page"
        )]
        cursor: Option<String>,
        #[arg(
            long,
            value_name = "COUNT",
            default_value_t = 50,
            value_parser = parse_page_limit,
            help = "Maximum dynamic fields read from one RPC page"
        )]
        limit: usize,
    },

    #[command(about = "Add one standalone occurrence")]
    Add {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        occurrence: OccurrenceArgs,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Inspect one permanent occurrence record")]
    Inspect {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long, value_name = "U64")]
        occurrence_id: u64,
        #[arg(long, help = "Poll until scheduler processing is terminal")]
        follow: bool,
    },

    #[command(about = "Record an advertised occurrence as missed")]
    Expire {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long, value_name = "U64")]
        occurrence_id: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Inspect occurrence payment accounting")]
    Cost {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long, value_name = "U64")]
        occurrence_id: u64,
    },

    #[command(about = "Abort expired runtime work")]
    AbortExpired {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long, value_name = "U64")]
        occurrence_id: u64,
        #[arg(long, value_name = "OBJECT_ID")]
        tool_gas_id: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },
}

/// Commands for the optional lazy recurrence.
#[derive(Subcommand)]
pub(crate) enum RecurrenceCommand {
    #[command(about = "Set or replace the Task recurrence")]
    Set {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        recurrence: RecurrenceArgs,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Clear future recurring work")]
    Clear {
        #[arg(long, short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: TaskCommand) -> AnyResult<(), NexusCliError> {
    match command {
        TaskCommand::Create(args) => create::run(args.task, args.gas).await,
        TaskCommand::Schedule(args) => {
            let ScheduleTaskArgs {
                task,
                schedule,
                gas,
            } = *args;
            schedule::run(task, schedule, gas).await
        }
        TaskCommand::List { cursor, limit } => list::run(cursor, limit).await,
        TaskCommand::Inspect { task_id } => state::inspect(task_id).await,
        TaskCommand::Pause { task_id, gas } => state::pause(task_id, gas).await,
        TaskCommand::Resume { task_id, gas } => state::resume(task_id, gas).await,
        TaskCommand::Cancel { task_id, gas } => state::cancel(task_id, gas).await,
        TaskCommand::Refill {
            task_id,
            amount_mist,
            gas,
        } => state::refill(task_id, amount_mist, gas).await,
        TaskCommand::Close { task_id, gas } => state::close(task_id, gas).await,
        TaskCommand::Occurrence(command) => occurrence::handle(command).await,
        TaskCommand::Recurrence(command) => recurrence::handle(command).await,
    }
}

fn parse_page_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|error| format!("invalid page limit: {error}"))?;
    if limit == 0 {
        return Err("page limit must be greater than zero".to_owned());
    }
    u32::try_from(limit)
        .map(|_| limit)
        .map_err(|_| "page limit must fit in u32".to_owned())
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn create_accepts_an_empty_task() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "task",
            "create",
            "--dag-id",
            "0x42",
            "--prepay-amount-mist",
            "50000000",
            "--occurrence-budget-mist",
            "50000000",
        ])
        .expect("empty Task creation should parse");
        assert!(matches!(
            cli.command,
            crate::Command::Task(task)
                if matches!(*task, TaskCommand::Create(_))
        ));
    }

    #[test]
    fn task_list_uses_the_default_page_limit() {
        let cli =
            crate::Cli::try_parse_from(["nexus", "task", "list"]).expect("Task list should parse");
        assert!(matches!(
            cli.command,
            crate::Command::Task(task)
                if matches!(
                    *task,
                    TaskCommand::List {
                        cursor: None,
                        limit: 50,
                    }
                )
        ));
    }

    #[test]
    fn task_list_accepts_cursor_and_limit() {
        let cli = crate::Cli::try_parse_from([
            "nexus", "task", "list", "--cursor", "0102", "--limit", "7",
        ])
        .expect("Task list page should parse");
        assert!(matches!(
            cli.command,
            crate::Command::Task(task)
                if matches!(
                    *task,
                    TaskCommand::List {
                        cursor: Some(ref cursor),
                        limit: 7,
                    } if cursor == "0102"
                )
        ));
    }

    #[test]
    fn task_list_rejects_a_zero_limit() {
        let error = match crate::Cli::try_parse_from(["nexus", "task", "list", "--limit", "0"]) {
            Ok(_) => panic!("zero page limit should fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("greater than zero"));
    }

    #[test]
    fn schedule_requires_at_least_one_source() {
        let result = crate::Cli::try_parse_from([
            "nexus",
            "task",
            "schedule",
            "--dag-id",
            "0x42",
            "--prepay-amount-mist",
            "50000000",
            "--occurrence-budget-mist",
            "50000000",
        ]);
        let error = match result {
            Ok(_) => panic!("a Schedule source is required"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("--now"));
    }

    #[test]
    fn schedule_accepts_mixed_sources() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "task",
            "schedule",
            "--dag-id",
            "0x42",
            "--prepay-amount-mist",
            "50000000",
            "--occurrence-budget-mist",
            "50000000",
            "--now",
            "--at-ms",
            "2000",
            "--deadline-after-ms",
            "1000",
            "--priority-fee-percentage",
            "30",
            "--recurrence-interval-ms",
            "1000",
        ])
        .expect("mixed Schedule should parse");
        assert!(matches!(
            cli.command,
            crate::Command::Task(task)
                if matches!(*task, TaskCommand::Schedule(_))
        ));
    }

    #[test]
    fn removed_schedule_root_is_rejected() {
        assert!(crate::Cli::try_parse_from(["nexus", "schedule"]).is_err());
        assert!(crate::Cli::try_parse_from(["nexus", "scheduler"]).is_err());
    }

    #[test]
    fn schedule_file_conflicts_with_inline_timing() {
        let result = crate::Cli::try_parse_from([
            "nexus",
            "task",
            "schedule",
            "--dag-id",
            "0x42",
            "--prepay-amount-mist",
            "50000000",
            "--occurrence-budget-mist",
            "50000000",
            "--schedule-file",
            "schedule.json",
            "--now",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn recurrence_options_require_an_interval() {
        let result = crate::Cli::try_parse_from([
            "nexus",
            "task",
            "schedule",
            "--dag-id",
            "0x42",
            "--prepay-amount-mist",
            "50000000",
            "--occurrence-budget-mist",
            "50000000",
            "--recurrence-occurrences",
            "2",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn recurrence_only_rejects_standalone_modifiers() {
        let cases: &[&[&str]] = &[
            &["--deadline-at-ms", "123"],
            &["--deadline-after-ms", "123"],
            &["--priority-fee-percentage", "30"],
            &["--deadline-at-ms", "123", "--priority-fee-percentage", "30"],
            &[
                "--deadline-after-ms",
                "123",
                "--priority-fee-percentage",
                "30",
            ],
        ];

        for options in cases {
            let result = crate::Cli::try_parse_from(
                [
                    "nexus",
                    "task",
                    "schedule",
                    "--dag-id",
                    "0x42",
                    "--prepay-amount-mist",
                    "50000000",
                    "--occurrence-budget-mist",
                    "50000000",
                    "--recurrence-interval-ms",
                    "60000",
                ]
                .into_iter()
                .chain(options.iter().copied()),
            );
            let error = match result {
                Ok(_) => panic!("standalone modifiers without a source should be rejected"),
                Err(error) => error,
            };
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::MissingRequiredArgument,
                "unexpected error for {options:?}: {error}"
            );
        }
    }

    #[test]
    fn agent_funding_requires_an_agent_operation() {
        let result = crate::Cli::try_parse_from([
            "nexus",
            "task",
            "create",
            "--dag-id",
            "0x42",
            "--agent-funded",
            "--prepay-amount-mist",
            "50000000",
            "--occurrence-budget-mist",
            "50000000",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn occurrence_list_requires_a_positive_page_limit() {
        let result = crate::Cli::try_parse_from([
            "nexus",
            "task",
            "occurrence",
            "list",
            "--task-id",
            "0x42",
            "--limit",
            "0",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn task_help_teaches_the_model() {
        let result = crate::Cli::try_parse_from(["nexus", "task", "--help"]);
        let error = match result {
            Ok(_) => panic!("help exits through Clap"),
            Err(error) => error,
        };
        let help = error.to_string();
        assert!(help.contains("Task -> Schedule -> Occurrence"));
        assert!(help.contains("A Task owns reusable work"));
        assert!(help.contains("task create"));
        assert!(help.contains("task schedule"));
        assert!(help.contains("Creates an empty Task"));
        assert!(help.contains("Creates and schedules atomically"));
        assert!(help.contains("durable object state"));
        assert!(help.contains("Examples:"));
    }

    #[test]
    fn schedule_help_explains_inputs_and_outcome() {
        let result = crate::Cli::try_parse_from(["nexus", "task", "schedule", "--help"]);
        let error = match result {
            Ok(_) => panic!("help exits through Clap"),
            Err(error) => error,
        };
        let help = error.to_string();
        for expected in [
            "Outcome:",
            "Examples:",
            "Operation",
            "Inputs",
            "Funding",
            "Policy",
            "Schedule",
            "Timing",
            "Recurrence",
            "Gas",
            "Output",
            "MILLIS",
            "MIST",
        ] {
            assert!(help.contains(expected), "missing help section: {expected}");
        }
    }
}
