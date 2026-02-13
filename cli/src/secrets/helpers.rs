use crate::prelude::*;

pub(crate) async fn load_cli_conf(path: &PathBuf) -> CliConf {
    CliConf::load_from_path(path).await.unwrap_or_default()
}

pub(crate) async fn load_crypto_conf_if_exists(
    path: &PathBuf,
) -> Result<Option<CryptoConf>, NexusCliError> {
    if !tokio::fs::try_exists(path)
        .await
        .map_err(|e| NexusCliError::Any(e.into()))?
    {
        return Ok(None);
    }

    let conf = CryptoConf::load_from_path(path)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!("Failed to read {}: {e}", path.display())))?;
    Ok(Some(conf))
}
