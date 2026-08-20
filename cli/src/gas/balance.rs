use {
    crate::{
        command_title,
        display::{human_output, json_output},
        loading, notify_success,
        prelude::*,
        sui::get_owner_nexus_client,
    },
    nexus_sdk::nexus::{client::GasSource, error::NexusError},
    num_format::{Locale, ToFormattedString},
    std::{fmt::Write as _, num::NonZeroU64},
};

const SUI_COIN_TYPE: &str = "0x2::sui::SUI";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct SuiBalanceOutput {
    address: sui::types::Address,
    total_mist: u64,
    address_balance_mist: u64,
    coin_balance_mist: u64,
}

#[derive(Debug, Serialize)]
struct DepositOutput {
    address: sui::types::Address,
    deposited_mist: u64,
    digest: sui::types::Digest,
}

fn required_coin_balance(amount_mist: u64, gas_budget: u64) -> AnyResult<u64> {
    amount_mist.checked_add(gas_budget).ok_or_else(|| {
        anyhow!("The deposit amount plus the transaction gas budget exceeds the maximum MIST value")
    })
}

fn select_deposit_coin(
    mut coins: Vec<(sui::types::ObjectReference, u64)>,
    requested: Option<sui::types::Address>,
    amount_mist: u64,
    gas_budget: u64,
) -> AnyResult<sui::types::ObjectReference> {
    let required = required_coin_balance(amount_mist, gas_budget)?;

    let selected = match requested {
        Some(requested) => coins
            .into_iter()
            .find(|(coin, _)| coin.object_id() == &requested)
            .ok_or_else(|| anyhow!("Owned SUI coin '{requested}' was not found"))?,
        None => {
            coins.sort_by(|(left, left_balance), (right, right_balance)| {
                right_balance
                    .cmp(left_balance)
                    .then_with(|| left.object_id().cmp(right.object_id()))
            });
            coins
                .into_iter()
                .find(|(_, balance)| *balance >= required)
                .ok_or_else(|| {
                    anyhow!(
                        "No owned SUI coin can cover the {required} MIST required for the deposit \
                         amount and transaction gas. Inspect both balance stores with:\n  nexus gas \
                         balance"
                    )
                })?
        }
    };

    if selected.1 < required {
        bail!(
            "Owned SUI coin '{}' contains {} MIST but this deposit requires {required} MIST. \
             Inspect both balance stores with:\n  nexus gas balance",
            selected.0.object_id(),
            selected.1,
        );
    }

    Ok(selected.0)
}

fn sui_balance_request(address: sui::types::Address) -> sui::grpc::GetBalanceRequest {
    sui::grpc::GetBalanceRequest::default()
        .with_owner(address)
        .with_coin_type(SUI_COIN_TYPE)
}

async fn fetch_sui_balance(
    client: &nexus_sdk::nexus::client::NexusClient,
) -> Result<SuiBalanceOutput, NexusCliError> {
    let address = client.owner().map_err(NexusCliError::Nexus)?;
    let request = sui_balance_request(address);
    let mut grpc = client.clone_grpc_client();
    let balance = grpc
        .state_client()
        .get_balance(request)
        .await
        .map_err(|error| NexusCliError::Rpc(anyhow!("Could not read SUI balances: {error}")))?
        .into_inner()
        .balance
        .unwrap_or_default();

    Ok(SuiBalanceOutput {
        address,
        total_mist: balance.balance(),
        address_balance_mist: balance.address_balance(),
        coin_balance_mist: balance.coin_balance(),
    })
}

fn render_balance(balance: &SuiBalanceOutput) -> String {
    const LABEL_WIDTH: usize = 17;
    let mut output = String::new();
    writeln!(output, "{:<LABEL_WIDTH$}{}", "Address", balance.address)
        .expect("writing to a String cannot fail");
    writeln!(
        output,
        "{:<LABEL_WIDTH$}{} MIST",
        "Total",
        balance.total_mist.to_formatted_string(&Locale::en)
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "{:<LABEL_WIDTH$}{} MIST",
        "Address balance",
        balance
            .address_balance_mist
            .to_formatted_string(&Locale::en)
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "{:<LABEL_WIDTH$}{} MIST",
        "Coin objects",
        balance.coin_balance_mist.to_formatted_string(&Locale::en)
    )
    .expect("writing to a String cannot fail");
    output.push_str(
        "\nThe address balance funds scheduler reserves and default transaction gas.\n\
         Move MIST into it from an owned coin with:\n\
         nexus gas deposit --amount <MIST>\n",
    );
    output
}

fn render_deposit(output: &DepositOutput) -> String {
    const LABEL_WIDTH: usize = 17;
    let mut rendered = String::new();
    writeln!(
        rendered,
        "{:<LABEL_WIDTH$}{} MIST",
        "Deposited",
        output.deposited_mist.to_formatted_string(&Locale::en)
    )
    .expect("writing to a String cannot fail");
    writeln!(rendered, "{:<LABEL_WIDTH$}{}", "Address", output.address)
        .expect("writing to a String cannot fail");
    writeln!(rendered, "{:<LABEL_WIDTH$}{}", "Transaction", output.digest)
        .expect("writing to a String cannot fail");
    rendered.push_str("\nNext command\nnexus gas balance\n");
    rendered
}

pub(super) async fn show() -> Result<(), NexusCliError> {
    command_title!("Reading SUI balance");
    let client = get_owner_nexus_client().await?;
    let progress = loading!("Reading address balance and coin objects...");
    let balance = match fetch_sui_balance(&client).await {
        Ok(balance) => {
            progress.success();
            balance
        }
        Err(error) => {
            progress.error();
            return Err(error);
        }
    };

    human_output(&render_balance(&balance));
    json_output(&balance)
}

pub(super) async fn deposit(amount: NonZeroU64, gas: GasArgs) -> Result<(), NexusCliError> {
    command_title!("Depositing SUI into the address balance");
    let client = get_owner_nexus_client().await?;
    let address = client.owner().map_err(NexusCliError::Nexus)?;

    let progress = loading!("Selecting an owned SUI coin...");
    let coins = match client
        .fetch_coins_by_type(sui::types::StructTag::gas_coin())
        .await
    {
        Ok(coins) => coins,
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    let coin = match select_deposit_coin(coins, gas.sui_gas_coin, amount.get(), gas.sui_gas_budget)
    {
        Ok(coin) => {
            progress.success();
            coin
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Any(error));
        }
    };

    client
        .set_gas_source(GasSource::coin(vec![coin], gas.sui_gas_budget))
        .await
        .map_err(NexusCliError::Nexus)?;
    let objects = client.get_nexus_objects();
    let transaction = nexus_sdk::transactions::gas::deposit_sui_to_address_balance(
        &objects,
        amount.get(),
        address,
    )
    .map_err(|error| NexusCliError::Nexus(NexusError::TransactionBuilding(error)))?;

    let progress = loading!("Submitting deposit transaction...");
    let response = match client.submit_transaction(transaction, address).await {
        Ok(response) => {
            progress.success();
            response
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    let output = DepositOutput {
        address,
        deposited_mist: amount.get(),
        digest: response.digest,
    };

    notify_success!(
        "Deposited {} MIST into the address balance",
        amount.get().to_formatted_string(&Locale::en)
    );
    human_output(&render_deposit(&output));
    json_output(&output)
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::sui};

    fn coin(id: &'static str, balance: u64) -> (sui::types::ObjectReference, u64) {
        (
            sui::types::ObjectReference::new(
                sui::types::Address::from_static(id),
                1,
                sui::types::Digest::ZERO,
            ),
            balance,
        )
    }

    #[test]
    fn automatic_coin_selection_is_sufficient_and_deterministic() {
        let selected = select_deposit_coin(
            vec![coin("0x3", 200), coin("0x2", 200), coin("0x1", 50)],
            None,
            100,
            50,
        )
        .unwrap();

        assert_eq!(
            selected.object_id(),
            &sui::types::Address::from_static("0x2")
        );
    }

    #[test]
    fn coin_selection_reports_the_exact_required_balance() {
        let error = select_deposit_coin(vec![coin("0x1", 149)], None, 100, 50)
            .unwrap_err()
            .to_string();

        assert!(error.contains("150 MIST"));
        assert!(error.contains("nexus gas balance"));
    }

    #[test]
    fn human_balance_output_explains_both_balance_stores() {
        let output = render_balance(&SuiBalanceOutput {
            address: sui::types::Address::from_static("0x42"),
            total_mist: 75_000_000,
            address_balance_mist: 25_000_000,
            coin_balance_mist: 50_000_000,
        });

        assert!(output.contains("Address balance  25,000,000 MIST"));
        assert!(output.contains("Coin objects     50,000,000 MIST"));
        assert!(output.contains("nexus gas deposit --amount <MIST>"));
    }

    #[test]
    fn balance_query_uses_the_sui_asset_type_not_the_coin_object_type() {
        let request = sui_balance_request(sui::types::Address::from_static("0x42"));

        assert_eq!(request.coin_type_opt(), Some("0x2::sui::SUI"));
    }
}
