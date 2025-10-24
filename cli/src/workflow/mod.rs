use {
    crate::{error::NexusCliError, prelude::AnyResult},
    anyhow::anyhow,
    nexus_sdk::{
        object_crawler::{fetch_one, Structure, VecMap, VecSet},
        sui,
        types::TypeName,
    },
    serde::Deserialize,
    std::collections::HashMap,
};

pub(crate) async fn fetch_encrypted_entry_ports(
    sui: &sui::Client,
    entry_group: String,
    dag_id: &sui::ObjectID,
) -> AnyResult<HashMap<String, Vec<String>>, NexusCliError> {
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
    struct EntryPort {
        name: String,
        encrypted: bool,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct Dag {
        entry_groups:
            VecMap<Structure<TypeName>, VecMap<Structure<TypeName>, VecSet<Structure<EntryPort>>>>,
    }

    let result = fetch_one::<Structure<Dag>>(sui, *dag_id)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    let key = TypeName {
        name: entry_group.clone(),
    };

    let entry_group = result
        .data
        .into_inner()
        .entry_groups
        .into_inner()
        .remove(&key.into())
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!("Entry group '{entry_group}' not found in DAG"))
        })?;

    Ok(entry_group
        .into_inner()
        .into_iter()
        .filter_map(|(vertex, ports)| {
            let encrypted_ports: Vec<String> = ports
                .into_inner()
                .into_iter()
                .filter_map(|port| {
                    let port = port.into_inner();
                    port.encrypted.then_some(port.name)
                })
                .collect();

            (!encrypted_ports.is_empty()).then_some((vertex.into_inner().name, encrypted_ports))
        })
        .collect())
}
