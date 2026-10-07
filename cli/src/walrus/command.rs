//! Execute Walrus commands and present SDK results.

use {
    super::{
        args::WalrusCommand,
        receipt::{self, atomic_write, read_bounded},
        settings::{load_conf, Settings},
        upload::Uploader,
    },
    crate::{
        display::{human_output, json_output},
        prelude::*,
    },
    anyhow::{ensure, Context as _},
    nexus_sdk::{
        execution_limits::MAX_RESOLVED_DATA_BYTES,
        walrus::{WalrusReader, WalrusUploadData},
    },
    nexus_walrus::WalrusStorage,
    serde_json::Value,
};

pub(crate) async fn handle(command: WalrusCommand) -> AnyResult<(), NexusCliError> {
    run(command).await.map_err(NexusCliError::Any)
}

async fn run(command: WalrusCommand) -> AnyResult<()> {
    let mut conf = load_conf().await?;
    if let WalrusCommand::Configure {
        epochs,
        aggregator,
        network,
        reset_aggregator,
    } = command
    {
        if let Some(epochs) = epochs {
            conf.data_storage.walrus_save_for_epochs = Some(epochs);
        }
        if let Some(url) = aggregator {
            WalrusReader::new(url.as_str(), MAX_RESOLVED_DATA_BYTES)?;
            conf.data_storage.walrus_aggregator_url = Some(url);
            conf.data_storage.walrus_network = network;
        }
        if reset_aggregator {
            conf.data_storage.walrus_aggregator_url = None;
            conf.data_storage.walrus_network = None;
        }
        conf.save().await?;
        return print_value(&conf.data_storage);
    }
    let settings = Settings::load(&conf).await?;
    match command {
        WalrusCommand::Status => {
            let owner = if conf.sui.pk.is_some() || std::env::var_os("SUI_PK").is_some() {
                Some(
                    crate::sui::get_signing_key(&conf)
                        .await?
                        .public_key()
                        .derive_address(),
                )
            } else {
                None
            };
            print_value(
                &json!({"network": settings.network, "chain_id": settings.chain_id, "payer": owner,
                "aggregator": settings.aggregator, "epochs": settings.epochs, "max_resolved_data_bytes": MAX_RESOLVED_DATA_BYTES}),
            )?;
        }
        WalrusCommand::Upload {
            file,
            storage,
            many,
            deletable,
            estimate,
            resume,
            out,
        } => {
            let data = WalrusUploadData::from_json_document(
                read_bounded(&file, MAX_RESOLVED_DATA_BYTES).await?,
                many,
            )?;
            let uploader =
                Uploader::new(settings.wallet(&conf).await?, settings, storage, deletable).await?;
            let out =
                out.unwrap_or_else(|| PathBuf::from(format!("{}.walrus.json", file.display())));
            if estimate {
                print_value(&uploader.estimate(&data).await?)?;
            } else {
                let reference = uploader.upload(&data, &out, resume).await?;
                human_output(&format!("Saved Walrus reference: {}", out.display()));
                json_output(&json!({"reference": out, "receipt": reference}))?;
            }
        }
        WalrusCommand::Inspect { reference } => {
            let saved = receipt::load(&reference).await?;
            settings.verify(&saved).await?;
            print_value(&json!({"reference": saved, "content_verified": true}))?;
        }
        WalrusCommand::Download { reference, out } => {
            let saved = receipt::load(&reference).await?;
            let values = settings.verify(&saved).await?;
            let bytes = if saved.many {
                serde_json::to_vec(
                    &values
                        .iter()
                        .map(|bytes| serde_json::from_slice::<Value>(bytes))
                        .collect::<Result<Vec<_>, _>>()?,
                )?
            } else {
                values
                    .into_iter()
                    .next()
                    .context("reference contains no blob")?
            };
            atomic_write(&out, &bytes, false)?;
            human_output(&format!("Verified data saved to {}", out.display()));
            json_output(&json!({"out": out, "bytes": bytes.len(), "verified": true}))?;
        }
        WalrusCommand::List { include_expired } => {
            let client =
                WalrusStorage::new(settings.wallet(&conf).await?, Some(&settings.aggregator))
                    .await?;
            print_value(&client.list(include_expired).await?)?;
        }
        WalrusCommand::Extend {
            reference,
            epochs,
            max_storage_cost_frost,
            storage_gas_budget,
        } => {
            let mut saved = receipt::load(&reference).await?;
            saved.check_network(settings.network, &settings.chain_id)?;
            let client =
                WalrusStorage::new(settings.wallet(&conf).await?, Some(&settings.aggregator))
                    .await?;
            for index in 0..saved.blobs.len() {
                saved.blobs[index] = client
                    .extend(
                        &saved.blobs[index],
                        epochs,
                        max_storage_cost_frost,
                        storage_gas_budget,
                    )
                    .await?;
                receipt::save(&saved, &reference, true)?;
            }
            print_value(&saved)?;
        }
        WalrusCommand::Delete {
            reference,
            yes: _,
            storage_gas_budget,
        } => {
            let saved = receipt::load(&reference).await?;
            saved.check_network(settings.network, &settings.chain_id)?;
            let client =
                WalrusStorage::new(settings.wallet(&conf).await?, Some(&settings.aggregator))
                    .await?;
            for blob in &saved.blobs {
                ensure!(
                    client.inspect(blob).await?.deletable,
                    "permanent storage cannot be deleted"
                );
            }
            for blob in &saved.blobs {
                client.delete(blob, storage_gas_budget).await?;
            }
            print_value(&json!({"reference": reference, "deleted": true}))?;
        }
        WalrusCommand::Configure { .. } => unreachable!(),
    }
    Ok(())
}

fn print_value(value: &impl Serialize) -> AnyResult<()> {
    human_output(&serde_json::to_string_pretty(value)?);
    json_output(value)?;
    Ok(())
}
