use {
    miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, JSONReportHandler},
    nexus_sdk::{
        nexus::error::{NexusError, TransactionError, TransactionErrorState},
        scheduler::{ScheduleError, SchedulerError},
        sui,
    },
    regex::Regex,
    std::sync::LazyLock,
    thiserror::Error,
};

static SUI_ADDRESS_BALANCE_SHORTFALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)insufficient address balance of coin type 0x2::sui::SUI.*",
        r"transaction requires ([0-9]+) but only ([0-9]+) is available",
    ))
    .expect("the SUI address balance error pattern is valid")
});

fn sui_address_balance_shortfall(error: &TransactionError) -> Option<u64> {
    let message = error.submission_rejection()?.message();
    let captures = SUI_ADDRESS_BALANCE_SHORTFALL.captures(message)?;
    let required = captures.get(1)?.as_str().parse::<u64>().ok()?;
    let available = captures.get(2)?.as_str().parse::<u64>().ok()?;
    required
        .checked_sub(available)
        .filter(|shortfall| *shortfall > 0)
}

/// Errors returned by Nexus CLI commands.
#[derive(Debug, Error)]
pub(crate) enum NexusCliError {
    #[error("I/O error: {0}")]
    #[allow(clippy::upper_case_acronyms)]
    Io(std::io::Error),
    #[error(transparent)]
    Any(anyhow::Error),
    #[error("HTTP request failed: {0}")]
    Http(reqwest::Error),
    #[error("Sui RPC failed: {0}")]
    Rpc(anyhow::Error),
    #[error(transparent)]
    Nexus(NexusError),
    #[error(transparent)]
    Schedule(#[from] ScheduleError),
    #[error(transparent)]
    Scheduler(#[from] SchedulerError),
    #[error("could not settle occurrence {occurrence_id} on Task '{task_id}': {source}")]
    OccurrenceSettlement {
        task_id: sui::types::Address,
        occurrence_id: u64,
        #[source]
        source: Box<SchedulerError>,
    },
}

impl NexusCliError {
    fn nexus_error(&self) -> Option<&NexusError> {
        match self {
            Self::Nexus(error) => Some(error),
            Self::Scheduler(SchedulerError::Client(error)) => Some(error.as_ref()),
            Self::OccurrenceSettlement { source, .. } => match source.as_ref() {
                SchedulerError::Client(error) => Some(error.as_ref()),
                _ => None,
            },
            _ => None,
        }
    }

    fn transaction_error(&self) -> Option<&TransactionError> {
        match self.nexus_error() {
            Some(NexusError::Transaction(error)) => Some(error),
            _ => None,
        }
    }

    fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Scheduler(SchedulerError::TaskEntryGroupNotFound { .. }) => {
                return Some("nexus::scheduler::entry_group_not_found");
            }
            Self::Scheduler(SchedulerError::TaskInputsMismatch { .. }) => {
                return Some("nexus::scheduler::task_inputs_mismatch");
            }
            Self::Scheduler(SchedulerError::OccurrenceNotDispatched { .. }) => {
                return Some("nexus::scheduler::occurrence_not_dispatched");
            }
            Self::Scheduler(SchedulerError::WatchTimedOut { .. }) => {
                return Some("nexus::scheduler::watch_timed_out");
            }
            _ => {}
        }
        match self.nexus_error() {
            Some(NexusError::ClientUpgradeRequired(_)) => {
                Some("nexus::state::client_upgrade_required")
            }
            Some(NexusError::Transaction(error)) => Some(match error.state() {
                TransactionErrorState::SubmissionRejected => {
                    "nexus::transaction::submission_rejected"
                }
                TransactionErrorState::SubmissionUnknown => {
                    "nexus::transaction::submission_unknown"
                }
                TransactionErrorState::ConfirmationUnknown => {
                    "nexus::transaction::confirmation_unknown"
                }
                TransactionErrorState::ExecutionFailed => "nexus::transaction::execution_failed",
            }),
            _ => None,
        }
    }
}

impl Diagnostic for NexusCliError {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.diagnostic_code()
            .map(|code| Box::new(code) as Box<dyn std::fmt::Display>)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        if let Some(NexusError::ClientUpgradeRequired(error)) = self.nexus_error() {
            return Some(Box::new(format!(
                "Object '{}' uses witness '{}' and stored layout '{}', which this CLI does not \
                 support. Upgrade the Nexus CLI and run the command again.",
                error.object_id,
                error.witness_type,
                error
                    .inner_type
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "<unavailable>".to_owned())
            )));
        }

        if let Self::OccurrenceSettlement {
            task_id,
            occurrence_id,
            ..
        } = self
        {
            return Some(Box::new(format!(
                "Inspect the occurrence state with:\n  nexus task occurrence inspect --task-id \
                 {task_id} --occurrence-id {occurrence_id}"
            )));
        }

        if let Self::Scheduler(
            SchedulerError::TaskEntryGroupNotFound { dag_id, .. }
            | SchedulerError::TaskInputsMismatch { dag_id, .. },
        ) = self
        {
            return Some(Box::new(format!(
                "Inspect every entry group and required input port with:\n  nexus dag inspect \
                 --dag-id {dag_id}\nProvide exactly that input shape with --input-json, then run the \
                 original command again."
            )));
        }

        if let Self::Scheduler(SchedulerError::OccurrenceNotDispatched {
            task_id,
            occurrence_id,
        }) = self
        {
            return Some(Box::new(format!(
                "An Execution exists only after dispatch. Inspect and follow the durable \
                 occurrence state with:\n  nexus task occurrence inspect --task-id {task_id} \
                 --occurrence-id {occurrence_id} --follow"
            )));
        }

        if let Self::Scheduler(SchedulerError::WatchTimedOut { last_snapshot }) = self {
            let reference = last_snapshot.reference();
            return Some(Box::new(format!(
                "The last observed occurrence state was {:?}. Inspect it again with:\n  nexus task \
                 occurrence inspect --task-id {} --occurrence-id {}",
                last_snapshot.status(),
                reference.task_id(),
                reference.occurrence_id(),
            )));
        }

        let transaction = self.transaction_error()?;
        let digest = transaction.digest();
        if let Some(shortfall) = sui_address_balance_shortfall(transaction) {
            return Some(Box::new(format!(
                "The Sui address balance needs {shortfall} more MIST. Deposit that amount from an \
                 owned SUI coin, then run the original command again:\n  nexus gas deposit \
                 --amount {shortfall}\nInspect both balance stores with:\n  nexus gas balance"
            )));
        }
        let help = match transaction.state() {
            TransactionErrorState::SubmissionRejected => format!(
                "The network rejected transaction {digest} before execution. Correct the reported \
                 cause before submitting another transaction."
            ),
            TransactionErrorState::SubmissionUnknown => format!(
                "The network response does not prove whether transaction {digest} was submitted. \
                 Inspect it before submitting another transaction:\n  sui client tx-block {digest}"
            ),
            TransactionErrorState::ConfirmationUnknown => format!(
                "Transaction {digest} executed, but checkpoint confirmation is unknown. Inspect \
                 it before submitting another transaction:\n  sui client tx-block {digest}"
            ),
            TransactionErrorState::ExecutionFailed => format!(
                "Transaction {digest} is confirmed and failed. Correct the reported execution \
                 error before submitting another transaction."
            ),
        };
        Some(Box::new(help))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ErrorReportMode {
    Human { verbose: bool, color: bool },
    Quiet,
    Json,
}

pub(crate) fn render_error(error: &NexusCliError, mode: ErrorReportMode) -> String {
    let mut output = String::new();
    match mode {
        ErrorReportMode::Human { verbose, color } => {
            let theme = if color {
                GraphicalTheme::unicode()
            } else {
                GraphicalTheme::unicode_nocolor()
            };
            let handler = GraphicalReportHandler::new_themed(theme).with_links(false);
            let handler = if verbose {
                handler.with_cause_chain()
            } else {
                handler.without_cause_chain()
            };
            handler
                .render_report(&mut output, error)
                .expect("writing a diagnostic to a String cannot fail");
        }
        ErrorReportMode::Quiet => {
            let code = error.diagnostic_code().unwrap_or("nexus::error");
            output = format!("{code}: {error}");
        }
        ErrorReportMode::Json => {
            JSONReportHandler::new()
                .render_report(&mut output, error)
                .expect("writing a diagnostic to a String cannot fail");
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use {
        super::{render_error, ErrorReportMode, NexusCliError},
        miette::Diagnostic as _,
        nexus_sdk::{
            nexus::error::{ClientUpgradeRequired, NexusError, TransactionError},
            scheduler::SchedulerError,
            sui,
        },
        std::time::Duration,
    };

    fn client_upgrade_required() -> NexusError {
        let tag = |package, module, name| {
            sui::types::StructTag::new(
                sui::types::Address::from_static(package),
                sui::types::Identifier::new(module).unwrap(),
                sui::types::Identifier::new(name).unwrap(),
                vec![],
            )
        };
        ClientUpgradeRequired::new(
            sui::types::Address::from_static("0x42"),
            tag("0xa7", "witness", "V2"),
            Some(tag("0xa7", "tool_registry", "ToolInnerV2")),
        )
        .into()
    }

    #[test]
    fn state_guidance_uses_the_typed_scheduler_client_error() {
        let error = NexusCliError::Scheduler(SchedulerError::from(client_upgrade_required()));

        assert_eq!(
            error.code().unwrap().to_string(),
            "nexus::state::client_upgrade_required"
        );
        assert!(error
            .help()
            .unwrap()
            .to_string()
            .contains("Upgrade the Nexus CLI"));
    }

    #[test]
    fn confirmation_unknown_has_a_stable_code() {
        let transaction = TransactionError::confirmation_timed_out(
            sui::types::Digest::new([7; 32]),
            Duration::from_secs(30),
            sui::grpc::ExecuteTransactionResponse::default(),
        );
        let error = NexusCliError::Nexus(transaction.into());

        assert_eq!(
            error.code().unwrap().to_string(),
            "nexus::transaction::confirmation_unknown"
        );
        assert!(error
            .help()
            .unwrap()
            .to_string()
            .contains("sui client tx-block"));
    }

    #[test]
    fn submission_rejection_is_not_reported_as_unknown() {
        let transaction = TransactionError::submission_rejected(
            sui::types::Digest::new([7; 32]),
            tonic::Status::invalid_argument("invalid gas reservation"),
        );
        let error = NexusCliError::Nexus(transaction.into());

        assert_eq!(
            error.code().unwrap().to_string(),
            "nexus::transaction::submission_rejected"
        );
        let help = error.help().unwrap().to_string();
        assert!(help.contains("Correct the reported cause"));
        assert!(!help.contains("unknown"));
    }

    #[test]
    fn insufficient_address_balance_has_an_exact_recovery_command() {
        let transaction = TransactionError::submission_rejected(
            sui::types::Digest::new([7; 32]),
            tonic::Status::invalid_argument(
                "Invalid withdraw reservation: Insufficient address balance of coin type \
                 0x2::sui::SUI for address 0x42: transaction requires 50000000 but only 12000000 \
                 is available. Note address balance excludes Coin objects.",
            ),
        );
        let error = NexusCliError::Nexus(transaction.into());

        let help = error.help().unwrap().to_string();
        assert!(help.contains("The Sui address balance needs 38000000 more MIST."));
        assert!(help.contains("nexus gas deposit --amount 38000000"));
        assert!(help.contains("nexus gas balance"));
        assert!(!help.contains("Correct the reported cause"));
    }

    #[test]
    fn settlement_error_has_the_exact_inspection_command() {
        let task_id = sui::types::Address::from_static("0x42");
        let error = NexusCliError::OccurrenceSettlement {
            task_id,
            occurrence_id: 7,
            source: Box::new(SchedulerError::OccurrenceNotDispatched {
                task_id,
                occurrence_id: 7,
            }),
        };

        assert_eq!(
            error.help().unwrap().to_string(),
            format!(
                "Inspect the occurrence state with:\n  nexus task occurrence inspect --task-id \
                 {task_id} --occurrence-id 7"
            )
        );
    }

    #[test]
    fn task_input_mismatch_has_a_stable_code_and_dag_inspection_command() {
        let dag_id = sui::types::Address::from_static("0xd");
        let error = NexusCliError::Scheduler(SchedulerError::TaskInputsMismatch {
            dag_id,
            entry_group: "_default_group".to_owned(),
            expected: r#"{"sum":{"0":"<value>"}}"#.to_owned(),
            received: "{}".to_owned(),
        });

        assert_eq!(
            error.code().unwrap().to_string(),
            "nexus::scheduler::task_inputs_mismatch"
        );
        assert!(error
            .help()
            .unwrap()
            .to_string()
            .contains(&format!("nexus dag inspect --dag-id {dag_id}")));
    }

    #[test]
    fn occurrence_without_execution_points_back_to_its_durable_state() {
        let task_id = sui::types::Address::from_static("0x42");
        let error = NexusCliError::Scheduler(SchedulerError::OccurrenceNotDispatched {
            task_id,
            occurrence_id: 7,
        });

        assert_eq!(
            error.code().unwrap().to_string(),
            "nexus::scheduler::occurrence_not_dispatched"
        );
        assert!(error.help().unwrap().to_string().contains(&format!(
            "nexus task occurrence inspect --task-id {task_id} --occurrence-id 7 --follow"
        )));
    }

    #[test]
    fn default_report_includes_the_transport_cause() {
        let error = NexusCliError::Scheduler(SchedulerError::Transport {
            source: anyhow::anyhow!("connection refused").into_boxed_dyn_error(),
        });

        let report = render_error(
            &error,
            ErrorReportMode::Human {
                verbose: false,
                color: false,
            },
        );

        assert!(report.contains("scheduler transport failed: connection refused"));
    }

    #[test]
    fn quiet_report_is_one_searchable_line() {
        let error = NexusCliError::Nexus(client_upgrade_required());

        let report = render_error(&error, ErrorReportMode::Quiet);

        assert_eq!(report.lines().count(), 1);
        assert!(report.starts_with("nexus::state::client_upgrade_required: "));
    }

    #[test]
    fn json_report_is_structured_and_human_report_can_disable_color() {
        let error = NexusCliError::Nexus(client_upgrade_required());

        let json: serde_json::Value =
            serde_json::from_str(&render_error(&error, ErrorReportMode::Json)).unwrap();
        assert_eq!(json["code"], "nexus::state::client_upgrade_required");
        assert_eq!(json["severity"], "error");
        assert!(json["help"].as_str().unwrap().contains("Upgrade"));

        let human = render_error(
            &error,
            ErrorReportMode::Human {
                verbose: false,
                color: false,
            },
        );
        assert!(!human.contains('\u{1b}'));
    }
}
