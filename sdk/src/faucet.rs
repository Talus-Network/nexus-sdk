use {
    crate::{sui, types::SuiNet},
    anyhow::bail,
    reqwest::{header, Client, StatusCode},
    serde::Deserialize,
};

/// Request tokens from the Faucet for the given address.
///
/// Inspired by:
/// <https://github.com/MystenLabs/sui/blob/aa99382c9191cd592cd65d0e197c33c49e4d9c4f/crates/sui/src/client_commands.rs#L2541>
pub async fn request_tokens(
    faucet_port: u16,
    sui_net: SuiNet,
    addr: sui::Address,
) -> anyhow::Result<()> {
    let url = match sui_net {
        SuiNet::Testnet => "https://faucet.testnet.sui.io/v1/gas",
        SuiNet::Localnet => &format!("http://127.0.0.1:{faucet_port}/gas"),
        _ => bail!("Unsupported network"),
    };

    #[derive(Deserialize)]
    struct FaucetResponse {
        error: Option<String>,
    }

    let json_body = serde_json::json![{
        "FixedAmountRequest": {
            "recipient": &addr.to_string()
        }
    }];

    // Make the request to the faucet JSON RPC API for coin.
    let resp = Client::new()
        .post(url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::USER_AGENT, "nexus-leader")
        .json(&json_body)
        .send()
        .await?;

    match resp.status() {
        StatusCode::ACCEPTED | StatusCode::CREATED => {
            let faucet_resp: FaucetResponse = resp.json().await?;

            if let Some(err) = faucet_resp.error {
                bail!("Faucet request was unsuccessful: {err}")
            }
        }
        StatusCode::TOO_MANY_REQUESTS => {
            bail!("Faucet service received too many requests from this IP address. Please try again after 60 minutes.");
        }
        StatusCode::SERVICE_UNAVAILABLE => {
            bail!("Faucet service is currently overloaded or unavailable. Please try again later.");
        }
        status_code => {
            bail!("Faucet request was unsuccessful: {status_code}");
        }
    }

    Ok(())
}
