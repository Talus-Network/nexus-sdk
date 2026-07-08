use {
    crate::cli_conf::StorageKind,
    nexus_sdk::move_bindings::primitives::data::NexusData,
    serde_json::Value,
    std::collections::HashMap,
};

const NEXUS_BASE_TRANSACTION_SIZE: usize = 8 * 1024;
const MAX_TRANSACTION_SIZE: usize = 128 * 1024;
const ENTRY_PORTS_RESERVED_BYTES: usize = 64 * 1024;
const WALRUS_BLOB_ID_LENGTH: usize = 44;

pub(crate) fn nexus_data_from_json_value(storage_kind: StorageKind, data: Value) -> NexusData {
    match (storage_kind, data) {
        (StorageKind::Inline, Value::Array(values)) => NexusData::inline_many(
            values
                .into_iter()
                .map(|value| serde_json::to_vec(&value).expect("JSON value must encode")),
        ),
        (StorageKind::Inline, value) => {
            NexusData::inline_one(serde_json::to_vec(&value).expect("JSON value must encode"))
        }
        (StorageKind::Walrus, Value::Array(values)) => NexusData::walrus_many(
            values
                .into_iter()
                .map(|value| serde_json::to_vec(&value).expect("JSON value must encode")),
        ),
        (StorageKind::Walrus, value) => {
            NexusData::walrus_one(serde_json::to_vec(&value).expect("JSON value must encode"))
        }
    }
}

pub(crate) fn nexus_data_to_json_value(data: &NexusData) -> Value {
    if data.one.is_empty() && data.many.is_empty() {
        return Value::Array(vec![]);
    }

    if data.many.is_empty() {
        return decode_nexus_data_json(&data.one);
    }

    Value::Array(
        data.many
            .iter()
            .map(|bytes| decode_nexus_data_json(bytes))
            .collect(),
    )
}

pub(crate) fn json_to_nexus_data_map(
    json: &Value,
    remote_fields: &[String],
    preferred_remote_storage: Option<StorageKind>,
) -> anyhow::Result<HashMap<String, NexusData>> {
    let preferred_remote_storage = preferred_remote_storage.unwrap_or(StorageKind::Walrus);

    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    let mut map = HashMap::new();

    for (key, value) in obj {
        let remote = remote_fields.contains(key);
        let key = key.clone();
        let value = value.clone();

        match remote {
            false => map.insert(key, nexus_data_from_json_value(StorageKind::Inline, value)),
            true => match preferred_remote_storage {
                StorageKind::Walrus => {
                    map.insert(key, nexus_data_from_json_value(StorageKind::Walrus, value))
                }
                StorageKind::Inline => {
                    anyhow::bail!("Cannot store data remotely using inline storage")
                }
            },
        };
    }

    Ok(map)
}

pub(crate) fn hint_remote_fields(json: &Value) -> anyhow::Result<Vec<String>> {
    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    let mut fields: Vec<(&String, usize)> = obj
        .iter()
        .map(|(key, value)| (key, key.len() + value.to_string().len()))
        .collect();

    fields.sort_by_key(|x| std::cmp::Reverse(x.1));

    let available_size = (MAX_TRANSACTION_SIZE - NEXUS_BASE_TRANSACTION_SIZE)
        .saturating_sub(ENTRY_PORTS_RESERVED_BYTES);
    let mut required_size = fields.iter().map(|(_, size)| size).sum::<usize>();

    if required_size <= available_size {
        return Ok(vec![]);
    }

    let mut remote_fields = vec![];
    for (key, size) in fields {
        let key = key.clone();
        let value = obj.get(&key).expect("Key must exist");
        let storage_cost = match value {
            Value::Array(arr) => WALRUS_BLOB_ID_LENGTH * arr.len(),
            _ => WALRUS_BLOB_ID_LENGTH,
        };

        required_size = required_size.saturating_sub(size) + storage_cost;
        remote_fields.push(key);

        if required_size <= available_size {
            break;
        }
    }

    if required_size > available_size {
        anyhow::bail!(
            "Cannot fit data within max transaction size, even after storing all fields remotely"
        );
    }

    Ok(remote_fields)
}

fn decode_nexus_data_json(bytes: &[u8]) -> Value {
    let text = std::str::from_utf8(bytes).expect("NexusData JSON bytes must be UTF-8");
    let adjusted = wrap_large_numbers_as_string(text.trim());
    serde_json::from_str(&adjusted).unwrap_or_else(|_| Value::String(text.to_string()))
}

fn is_large_number(s: &str) -> bool {
    if let Some(stripped) = s.strip_prefix('-') {
        stripped.chars().all(|c| c.is_ascii_digit()) && s.len() > 21
    } else {
        s.chars().all(|c| c.is_ascii_digit()) && s.len() > 20
    }
}

fn wrap_large_numbers_as_string(value: &str) -> String {
    if is_large_number(value) {
        format!(r#""{value}""#)
    } else {
        value.to_string()
    }
}
