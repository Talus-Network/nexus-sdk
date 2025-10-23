use {
    crate::{error::NexusCliError, prelude::AnyResult},
    anyhow::anyhow,
    nexus_sdk::{
        crypto::session::Session,
        object_crawler::{fetch_one, Structure, VecMap, VecSet},
        sui,
        types::TypeName,
    },
    serde::Deserialize,
    serde_json::Value,
    std::collections::HashMap,
};

pub(crate) fn encrypt_entry_ports_once(
    session: &mut Session,
    input: &mut Value,
    targets: &HashMap<String, Vec<String>>,
) -> Result<(), NexusCliError> {
    if targets.is_empty() {
        return Ok(());
    }

    for (vertex, ports) in targets {
        for port in ports {
            let data = input
                .get_mut(vertex)
                .and_then(|map| map.get_mut(port))
                .ok_or_else(|| NexusCliError::Any(anyhow!("Input JSON has no {vertex}.{port}")))?;

            session
                .encrypt_nexus_data_json(data)
                .map_err(NexusCliError::Any)?;
        }
    }

    session.commit_sender(None);

    Ok(())
}

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

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::crypto::{
            session::{Message, Session, StandardMessage},
            x3dh::{IdentityKey, PreKeyBundle},
        },
        serde_json::json,
    };

    fn create_mock_session() -> Session {
        let sender = IdentityKey::generate();
        let receiver = IdentityKey::generate();
        let spk_secret = IdentityKey::generate().secret().clone();
        let bundle = PreKeyBundle::new(&receiver, 1, &spk_secret, None, None);

        let (message, mut sender_session) =
            Session::initiate(&sender, &bundle, b"test").expect("initiate session");

        let initial_msg = match message {
            Message::Initial(initial) => initial,
            _ => panic!("Expected initial message"),
        };

        let (mut receiver_session, _) =
            Session::recv(&receiver, &spk_secret, &bundle, &initial_msg, None)
                .expect("receive session");

        let setup_msg = sender_session
            .encrypt(b"setup")
            .expect("encrypt setup message");
        receiver_session
            .decrypt(&setup_msg)
            .expect("decrypt setup message");

        sender_session
    }

    #[test]
    fn encrypt_entry_ports_once_no_targets() {
        let mut session = create_mock_session();
        let mut input = json!({ "v": { "p": "value" } });
        let targets = HashMap::<String, Vec<String>>::new();

        encrypt_entry_ports_once(&mut session, &mut input, &targets).expect("should succeed");
        assert_eq!(input, json!({ "v": { "p": "value" } }));
    }

    #[test]
    fn encrypt_entry_ports_once_missing_vertex() {
        let mut session = create_mock_session();
        let mut input = json!({ "v": { "p": "value" } });
        let targets = HashMap::from([(String::from("other"), vec![String::from("p")])]);

        let err = encrypt_entry_ports_once(&mut session, &mut input, &targets).unwrap_err();
        assert!(err.to_string().contains("Input JSON has no other.p"));
    }

    #[test]
    fn encrypt_entry_ports_once_encrypts() {
        let mut session = create_mock_session();
        let mut input = json!({
            "v": {
                "p1": "value1",
                "p2": "value2"
            }
        });
        let targets = HashMap::from([(
            String::from("v"),
            vec![String::from("p1"), String::from("p2")],
        )]);

        encrypt_entry_ports_once(&mut session, &mut input, &targets).expect("should succeed");

        let msg1 = serde_json::from_value::<StandardMessage>(input["v"]["p1"].clone());
        let msg2 = serde_json::from_value::<StandardMessage>(input["v"]["p2"].clone());
        assert!(msg1.is_ok());
        assert!(msg2.is_ok());
    }
}
