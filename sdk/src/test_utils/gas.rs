use crate::sui;

/// Fetch gas coin for the provided address.
pub async fn fetch_gas_coin(sui: &sui::Client, addr: sui::Address) -> anyhow::Result<sui::Coin> {
    let limit = Some(1);
    let cursor = None;
    let default_to_sui_coin_type = None;

    let response = sui
        .coin_read_api()
        .get_coins(addr, default_to_sui_coin_type, cursor, limit)
        .await?;

    Ok(response
        .data
        .iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No coin in wallet."))?
        .clone())
}
