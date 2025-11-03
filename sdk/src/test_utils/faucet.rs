use {
    crate::sui,
    anyhow::bail,
    reqwest::{header, Client},
    serde::Deserialize,
    std::time::Duration,
    tokio_retry::{strategy::ExponentialBackoff, Retry},
};

/// Request tokens from the Sui faucet for the given address.
pub async fn request_tokens(url: &str, addr: sui::Address) -> anyhow::Result<()> {
    #[derive(Debug, Deserialize)]
    struct FaucetResponse {
        error: Option<String>,
    }

    let json_body = serde_json::json![{
        "FixedAmountRequest": {
            "recipient": &addr.to_string()
        }
    }];

    let retry_strategy = ExponentialBackoff::from_millis(100)
        .max_delay(Duration::from_secs(5))
        .take(5);

    // Retry a couple times in case the faucet is slow to load.
    let response = Retry::spawn(retry_strategy, || async {
        let resp = Client::new()
            .post(url)
            .header(header::USER_AGENT, "nexus-leader")
            .json(&json_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Unexpected status: {}", resp.status())
        }

        Ok(resp)
    })
    .await?;

    let faucet_resp: FaucetResponse = response.json().await?;

    if let Some(err) = faucet_resp.error {
        bail!("Faucet request was unsuccessful: {err}");
    }

    Ok(())
}
