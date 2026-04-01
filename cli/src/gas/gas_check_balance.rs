use {
    crate::{
        command_title,
        display::json_output,
        item,
        loading,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::nexus::models::Scope,
    num_format::{Locale, ToFormattedString},
};

/// Check the current Nexus gas balance for the invoker.
pub(crate) async fn check_balance() -> AnyResult<(), NexusCliError> {
    command_title!("Checking invoker's Nexus gas balance");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;

    let fetch_handle = loading!("Fetching gas balance from Sui...");

    let result = match nexus_client
        .gas()
        .balance()
        .await
        .map_err(NexusCliError::Nexus)
    {
        Ok(balance) => balance,
        Err(e) => {
            fetch_handle.error();

            return Err(e);
        }
    };

    fetch_handle.success();

    notify_success!("Balance in invoker's gas vault:");

    for (scope, funds) in &result.funds {
        let scope = match scope {
            Scope::Execution(addr) => {
                format!("Execution({})", addr.to_string().truecolor(100, 100, 100))
            }
            Scope::InvokerAddress(addr) => format!(
                "InvokerAddress({})",
                addr.to_string().truecolor(100, 100, 100)
            ),
            Scope::WorksheetType(name) => {
                format!("WorksheetType({})", name.name.truecolor(100, 100, 100))
            }
        };

        item!(
            "{}: Total {}; Locked {}",
            scope,
            format!("{} MIST", funds.bal.to_formatted_string(&Locale::en)).truecolor(100, 100, 100),
            format!("{} MIST", funds.locked.to_formatted_string(&Locale::en))
                .truecolor(100, 100, 100),
        );
    }

    json_output(
        &result
            .funds
            .iter()
            .map(|(scope, funds)| {
                json!({
                    "scope": scope,
                    "total": funds.bal,
                    "locked": funds.locked
                })
            })
            .collect::<Vec<_>>(),
    )?;

    Ok(())
}

// TODO: tests for balance, cost and models
