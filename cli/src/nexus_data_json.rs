use {
    nexus_sdk::types::NexusData,
    serde_json::Value,
    std::collections::{HashMap, HashSet},
};

const NEXUS_BASE_TRANSACTION_SIZE: usize = 8 * 1024;
const MAX_TRANSACTION_SIZE: usize = 128 * 1024;
const ENTRY_PORTS_RESERVED_BYTES: usize = 64 * 1024;
const WALRUS_BLOB_ID_LENGTH: usize = 44;
const SHA256_DIGEST_LENGTH: usize = 32;

pub(crate) fn nexus_data_from_json_value(data: Value) -> anyhow::Result<NexusData> {
    if is_canonical_nexus_data(&data) {
        return NexusData::from_json_value(&data);
    }

    match data {
        Value::Array(values) => NexusData::inline_data_many(
            values
                .into_iter()
                .map(|value| serde_json::to_vec(&value))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        value => NexusData::inline_data(serde_json::to_vec(&value)?),
    }
}

fn is_canonical_nexus_data(value: &Value) -> bool {
    let Some(object) = value.as_object().filter(|object| object.len() == 1) else {
        return false;
    };

    match (object.get("one"), object.get("many")) {
        (Some(value), None) => value.get("kind").is_some(),
        (None, Some(Value::Array(values))) => {
            values.iter().all(|value| value.get("kind").is_some())
        }
        _ => false,
    }
}

#[cfg(test)]
pub(crate) fn nexus_data_to_json_value(data: &NexusData) -> Value {
    use nexus_sdk::move_bindings::primitives::data::NexusValue;

    let decode = |value: &NexusValue| match value {
        NexusValue::InlineData { bytes } => decode_nexus_data_json(bytes),
        NexusValue::Object { .. } | NexusValue::WalrusData { .. } => Value::Null,
    };
    let values = data.values().unwrap_or_default();
    if data.is_one() {
        values.first().map(decode).unwrap_or(Value::Null)
    } else {
        Value::Array(values.iter().map(decode).collect())
    }
}

pub(crate) fn json_to_nexus_data_map(json: &Value) -> anyhow::Result<HashMap<String, NexusData>> {
    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    let mut map = HashMap::new();

    for (key, value) in obj {
        map.insert(key.clone(), nexus_data_from_json_value(value.clone())?);
    }

    Ok(map)
}

pub(crate) fn hint_remote_fields(
    json: &Value,
    selected_remote_fields: &HashSet<String>,
) -> anyhow::Result<Vec<String>> {
    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    let mut fields: Vec<(&String, &Value, usize)> = obj
        .iter()
        .map(|(key, value)| (key, value, key.len() + value.to_string().len()))
        .collect();

    fields.sort_by_key(|(_, _, inline_size)| std::cmp::Reverse(*inline_size));

    let available_size = (MAX_TRANSACTION_SIZE - NEXUS_BASE_TRANSACTION_SIZE)
        .saturating_sub(ENTRY_PORTS_RESERVED_BYTES);
    let mut required_size = fields
        .iter()
        .fold(0usize, |total, (key, value, inline_size)| {
            total.saturating_add(if selected_remote_fields.contains(*key) {
                walrus_reference_value_size(value)
            } else {
                *inline_size
            })
        });

    if required_size <= available_size {
        return Ok(vec![]);
    }

    let mut remote_fields = vec![];
    for (key, value, inline_size) in fields {
        if selected_remote_fields.contains(key) {
            continue;
        }
        let key = key.clone();
        let storage_cost = walrus_reference_value_size(value);

        required_size = required_size
            .saturating_sub(inline_size)
            .saturating_add(storage_cost);
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

fn walrus_reference_value_size(value: &Value) -> usize {
    match value {
        Value::Array(values) => walrus_reference_input_size().saturating_mul(values.len()),
        _ => walrus_reference_input_size(),
    }
}

fn walrus_reference_input_size() -> usize {
    encoded_pure_vector_size(WALRUS_BLOB_ID_LENGTH) + encoded_pure_vector_size(SHA256_DIGEST_LENGTH)
}

fn encoded_pure_vector_size(value_len: usize) -> usize {
    let move_value_len = uleb128_size(value_len) + value_len;
    1 + uleb128_size(move_value_len) + move_value_len
}

fn uleb128_size(mut value: usize) -> usize {
    let mut size = 1;
    while value >= 128 {
        value >>= 7;
        size += 1;
    }
    size
}

#[cfg(test)]
fn decode_nexus_data_json(bytes: &[u8]) -> Value {
    let text = std::str::from_utf8(bytes).expect("NexusData JSON bytes must be UTF-8");
    let adjusted = wrap_large_numbers_as_string(text.trim());
    serde_json::from_str(&adjusted).unwrap_or_else(|_| Value::String(text.to_string()))
}

#[cfg(test)]
fn is_large_number(s: &str) -> bool {
    if let Some(stripped) = s.strip_prefix('-') {
        stripped.chars().all(|c| c.is_ascii_digit()) && s.len() > 21
    } else {
        s.chars().all(|c| c.is_ascii_digit()) && s.len() > 20
    }
}

#[cfg(test)]
fn wrap_large_numbers_as_string(value: &str) -> String {
    if is_large_number(value) {
        format!(r#""{value}""#)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn maximal_many_fields(count: usize) -> Value {
        Value::Object(
            (0..count)
                .map(|index| {
                    (
                        format!("field-{index}"),
                        Value::Array((0..128).map(|_| Value::String("x".repeat(100))).collect()),
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn plain_json_roundtrips_without_inline_base64_shape() {
        let original = serde_json::json!({
            "object": { "nested": true },
            "number": 7,
        });
        let data = nexus_data_from_json_value(original.clone()).unwrap();

        assert_eq!(nexus_data_to_json_value(&data), original);
    }

    #[test]
    fn ordinary_one_property_object_remains_inline_data() {
        let original = serde_json::json!({ "one": { "nested": true } });
        let data = nexus_data_from_json_value(original.clone()).unwrap();

        assert!(data.is_data());
        assert_eq!(nexus_data_to_json_value(&data), original);
    }

    #[test]
    fn canonical_one_object_preserves_object_kind() {
        let data = nexus_data_from_json_value(serde_json::json!({
            "one": { "kind": "object", "id": "0x42" }
        }))
        .unwrap();

        assert!(data.is_one());
        assert!(data.values().unwrap()[0].is_object());
    }

    #[test]
    fn canonical_many_objects_preserve_object_kind() {
        let data = nexus_data_from_json_value(serde_json::json!({
            "many": [
                { "kind": "object", "id": "0x42" },
                { "kind": "object", "id": "0x43" }
            ]
        }))
        .unwrap();

        assert!(data.is_many());
        assert!(data.values().unwrap().iter().all(|value| value.is_object()));
    }

    #[test]
    fn malformed_canonical_object_is_rejected() {
        let error = nexus_data_from_json_value(serde_json::json!({
            "one": { "kind": "object", "id": "not-an-object" }
        }))
        .unwrap_err();

        assert!(error.to_string().contains("invalid Object ID"));
    }

    #[test]
    fn array_input_remains_ordered_independent_many_values() {
        let original = serde_json::json!([1, { "ordered": 2 }, "three"]);
        let data = nexus_data_from_json_value(original.clone()).unwrap();

        assert!(data.is_many());
        assert_eq!(nexus_data_to_json_value(&data), original);
    }

    #[test]
    fn empty_array_input_is_rejected() {
        let error = nexus_data_from_json_value(serde_json::json!([])).unwrap_err();

        assert!(error.to_string().contains("requires at least one value"));
    }

    #[test]
    fn walrus_reference_size_includes_digest_and_bcs_encoding() {
        assert_eq!(walrus_reference_input_size(), 82);
    }

    #[test]
    fn maximal_many_walrus_references_respect_near_limit_budget() {
        let selected_remote_fields = HashSet::new();
        assert_eq!(
            hint_remote_fields(&maximal_many_fields(5), &selected_remote_fields)
                .unwrap()
                .len(),
            4
        );
        assert!(hint_remote_fields(&maximal_many_fields(6), &selected_remote_fields).is_err());
    }
}
