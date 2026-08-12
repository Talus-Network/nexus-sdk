//! Constructors, validation, commitments, and deterministic JSON for direct Nexus values.

use {
    crate::{
        move_bindings::{
            interface::meta_schema::{PortCommitment, ValueKind},
            primitives::data::{NexusData, NexusValue},
            sui_framework::object::ID,
        },
        sui,
    },
    anyhow::{anyhow, bail, Context as _},
    base64::{engine::general_purpose::STANDARD as BASE64, Engine as _},
    serde_json::{json, Value},
    sha2::{Digest as _, Sha256},
    std::str::FromStr as _,
};

const SHA256_LEN: usize = 32;
const MAX_MANY_VALUES: usize = 256;
pub(crate) const MAX_INLINE_DATA_BYTES: usize = 61_440;
const MAX_WALRUS_STORAGE_KEY_BYTES: usize = 1_024;
const MAX_NEXUS_DATA_BYTES: usize = 65_536;

impl NexusValue {
    pub fn object(id: sui::types::Address) -> Self {
        Self::Object { id: ID::new(id) }
    }

    pub fn inline_data(bytes: impl Into<Vec<u8>>) -> anyhow::Result<Self> {
        let bytes = bytes.into();
        if bytes.len() > MAX_INLINE_DATA_BYTES {
            bail!("inline data exceeds {MAX_INLINE_DATA_BYTES} bytes");
        }
        Ok(Self::InlineData { bytes })
    }

    pub fn walrus_data(
        storage_key: impl Into<Vec<u8>>,
        content_digest: impl Into<Vec<u8>>,
    ) -> anyhow::Result<Self> {
        let storage_key = storage_key.into();
        let content_digest = content_digest.into();
        if storage_key.len() > MAX_WALRUS_STORAGE_KEY_BYTES {
            bail!("Walrus storage key exceeds {MAX_WALRUS_STORAGE_KEY_BYTES} bytes");
        }
        if content_digest.len() != SHA256_LEN {
            bail!("Walrus content digest must contain exactly {SHA256_LEN} bytes");
        }
        Ok(Self::WalrusData {
            storage_key,
            content_digest,
        })
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object { .. })
    }

    pub fn is_data(&self) -> bool {
        matches!(self, Self::InlineData { .. } | Self::WalrusData { .. })
    }

    pub fn is_well_formed(&self) -> bool {
        if bcs::to_bytes(self).map_or(true, |bytes| bytes.len() > MAX_NEXUS_DATA_BYTES) {
            return false;
        }
        match self {
            Self::Object { .. } => true,
            Self::InlineData { bytes } => bytes.len() <= MAX_INLINE_DATA_BYTES,
            Self::WalrusData {
                storage_key,
                content_digest,
            } => {
                storage_key.len() <= MAX_WALRUS_STORAGE_KEY_BYTES
                    && content_digest.len() == SHA256_LEN
            }
        }
    }
}

impl NexusData {
    /// Decodes the active published storage envelope into exact typed witnesses.
    pub fn values(&self) -> anyhow::Result<Vec<NexusValue>> {
        if self.storage != b"nexus_value" {
            bail!("NexusData uses an unsupported storage discriminator");
        }
        let (encoded, is_many) = match (self.one.is_empty(), self.many.is_empty()) {
            (false, true) => (std::slice::from_ref(&self.one), false),
            (true, false) => (self.many.as_slice(), true),
            _ => bail!("NexusData must contain exactly one active One or Many payload"),
        };
        let values = encoded
            .iter()
            .map(|bytes| {
                let value: NexusValue = bcs::from_bytes(bytes)
                    .context("NexusData contains an invalid NexusValue payload")?;
                if bcs::to_bytes(&value)? != *bytes {
                    bail!("NexusData contains a non-canonical NexusValue payload");
                }
                Ok(value)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        validate_values(&values, is_many)?;
        if bcs::to_bytes(self)?.len() > MAX_NEXUS_DATA_BYTES {
            bail!("NexusData exceeds {MAX_NEXUS_DATA_BYTES} encoded bytes");
        }
        Ok(values)
    }

    pub fn into_values(self) -> anyhow::Result<Vec<NexusValue>> {
        self.values()
    }

    pub fn object(id: sui::types::Address) -> Self {
        Self::from_values(vec![NexusValue::object(id)], false)
            .expect("an Object ID is always valid NexusData")
    }

    pub fn inline_data(bytes: impl Into<Vec<u8>>) -> anyhow::Result<Self> {
        Self::from_values(vec![NexusValue::inline_data(bytes)?], false)
    }

    pub fn walrus_data(
        storage_key: impl Into<Vec<u8>>,
        content_digest: impl Into<Vec<u8>>,
    ) -> anyhow::Result<Self> {
        Self::from_values(
            vec![NexusValue::walrus_data(storage_key, content_digest)?],
            false,
        )
    }

    pub fn object_many(ids: impl IntoIterator<Item = sui::types::Address>) -> anyhow::Result<Self> {
        Self::many(ids.into_iter().map(NexusValue::object).collect())
    }

    pub fn inline_data_many<I, B>(values: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = B>,
        B: Into<Vec<u8>>,
    {
        Self::many(
            values
                .into_iter()
                .map(NexusValue::inline_data)
                .collect::<anyhow::Result<Vec<_>>>()?,
        )
    }

    pub fn walrus_data_many<I, K, D>(values: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = (K, D)>,
        K: Into<Vec<u8>>,
        D: Into<Vec<u8>>,
    {
        Self::many(
            values
                .into_iter()
                .map(|(key, digest)| NexusValue::walrus_data(key, digest))
                .collect::<anyhow::Result<Vec<_>>>()?,
        )
    }

    pub fn many(values: Vec<NexusValue>) -> anyhow::Result<Self> {
        Self::from_values(values, true)
    }

    pub fn from_values(values: Vec<NexusValue>, many: bool) -> anyhow::Result<Self> {
        validate_values(&values, many)?;
        let mut encoded = values
            .iter()
            .map(bcs::to_bytes)
            .collect::<Result<Vec<_>, _>>()?;
        let data = if many {
            Self::new(b"nexus_value".to_vec(), Vec::new(), encoded)
        } else {
            Self::new(
                b"nexus_value".to_vec(),
                encoded.pop().expect("One validation requires one value"),
                Vec::new(),
            )
        };
        if bcs::to_bytes(&data)?.len() > MAX_NEXUS_DATA_BYTES {
            bail!("NexusData exceeds {MAX_NEXUS_DATA_BYTES} encoded bytes");
        }
        Ok(data)
    }

    pub fn is_one(&self) -> bool {
        !self.one.is_empty() && self.many.is_empty()
    }

    pub fn is_many(&self) -> bool {
        self.one.is_empty() && !self.many.is_empty()
    }

    pub fn is_well_formed(&self) -> bool {
        self.values().is_ok()
    }

    pub fn is_object(&self) -> bool {
        self.values()
            .is_ok_and(|values| values.iter().all(NexusValue::is_object))
    }

    pub fn is_data(&self) -> bool {
        self.values()
            .is_ok_and(|values| values.iter().all(NexusValue::is_data))
    }

    pub fn has_walrus(&self) -> bool {
        self.values().is_ok_and(|values| {
            values
                .iter()
                .any(|value| matches!(value, NexusValue::WalrusData { .. }))
        })
    }

    pub fn inline_data_bytes(&self) -> Option<Vec<u8>> {
        match self.values().ok()?.as_slice() {
            [NexusValue::InlineData { bytes }] if self.is_one() => Some(bytes.clone()),
            _ => None,
        }
    }

    /// Builds the exact transient commitment shape hashed by Move for one schema port.
    pub fn port_commitment(&self, schema_kind: ValueKind) -> anyhow::Result<PortCommitment> {
        let values = self.values().context("cannot commit malformed NexusData")?;
        if self.is_one() {
            let value = values
                .first()
                .expect("well-formed One contains exactly one value");
            if value_kind(value) != schema_kind {
                bail!("NexusData value kind does not match schema");
            }
            Ok(PortCommitment::One {
                kind: schema_kind,
                commitment: value_content_commitment(value)?,
            })
        } else {
            if values
                .first()
                .is_some_and(|value| value_kind(value) != schema_kind)
            {
                bail!("NexusData value kind does not match schema");
            }
            Ok(PortCommitment::Many {
                kind: schema_kind,
                commitments: values
                    .iter()
                    .map(value_content_commitment)
                    .collect::<anyhow::Result<Vec<_>>>()?,
            })
        }
    }

    pub fn to_json_value(&self) -> anyhow::Result<Value> {
        if !self.is_well_formed() {
            bail!("cannot encode malformed NexusData");
        }
        let values = self.values()?.iter().map(value_to_json).collect::<Vec<_>>();
        if self.is_one() {
            Ok(json!({ "one": values.into_iter().next().expect("One has one value") }))
        } else {
            Ok(json!({ "many": values }))
        }
    }

    /// Validates the canonical human-readable One/Many JSON shape without serializing inline Data.
    pub fn validate_json_value(value: &Value) -> anyhow::Result<()> {
        let object = value
            .as_object()
            .ok_or_else(|| anyhow!("NexusData must be a JSON object"))?;
        let cardinality_key = match (object.contains_key("one"), object.contains_key("many")) {
            (true, false) => "one",
            (false, true) => "many",
            (true, true) => bail!("NexusData cannot contain both 'one' and 'many'"),
            (false, false) => bail!("NexusData must contain exactly one of 'one' or 'many'"),
        };
        ensure_exact_keys(object, &[cardinality_key], "NexusData")?;
        match cardinality_key {
            "one" => validate_value_json(&object[cardinality_key]),
            "many" => {
                let values = object[cardinality_key]
                    .as_array()
                    .ok_or_else(|| anyhow!("NexusData property 'many' must be an array"))?;
                if values.is_empty() {
                    bail!("NexusData::Many requires at least one value");
                }
                if values.len() > MAX_MANY_VALUES {
                    bail!("NexusData::Many exceeds {MAX_MANY_VALUES} values");
                }
                for value in values {
                    validate_value_json(value)?;
                }
                if let Some(first) = values.first() {
                    let object_kind = value_json_is_object(first);
                    if values
                        .iter()
                        .any(|value| value_json_is_object(value) != object_kind)
                    {
                        bail!("NexusData::Many values must have one homogeneous value kind");
                    }
                }
                Ok(())
            }
            _ => unreachable!("cardinality key was validated above"),
        }
    }

    pub fn from_json_value(value: &Value) -> anyhow::Result<Self> {
        Self::validate_json_value(value)?;
        let object = value
            .as_object()
            .ok_or_else(|| anyhow!("NexusData must be a JSON object"))?;
        let cardinality_key = match (object.contains_key("one"), object.contains_key("many")) {
            (true, false) => "one",
            (false, true) => "many",
            (true, true) => bail!("NexusData cannot contain both 'one' and 'many'"),
            (false, false) => bail!("NexusData must contain exactly one of 'one' or 'many'"),
        };
        ensure_exact_keys(object, &[cardinality_key], "NexusData")?;
        match cardinality_key {
            "one" => Self::from_values(vec![value_from_json(&object[cardinality_key])?], false),
            "many" => Self::from_values(
                object[cardinality_key]
                    .as_array()
                    .ok_or_else(|| anyhow!("NexusData property 'many' must be an array"))?
                    .iter()
                    .map(value_from_json)
                    .collect::<anyhow::Result<Vec<_>>>()?,
                true,
            ),
            _ => unreachable!("cardinality key was validated above"),
        }
    }
}

fn validate_values(values: &[NexusValue], many: bool) -> anyhow::Result<()> {
    if !many && values.len() != 1 {
        bail!("NexusData::One requires one value, got {}", values.len());
    }
    if many && values.is_empty() {
        bail!("NexusData::Many requires at least one value");
    }
    if values.len() > MAX_MANY_VALUES {
        bail!("NexusData::Many exceeds {MAX_MANY_VALUES} values");
    }
    if values.iter().any(|value| !value.is_well_formed()) {
        bail!("NexusData::Many value exceeds protocol bounds");
    }
    if let Some(first) = values.first() {
        let object_kind = first.is_object();
        if values.iter().any(|value| value.is_object() != object_kind) {
            bail!("NexusData::Many values must have one homogeneous value kind");
        }
    }
    Ok(())
}

fn value_kind(value: &NexusValue) -> ValueKind {
    if value.is_object() {
        ValueKind::Object
    } else {
        ValueKind::Data
    }
}

fn value_content_commitment(value: &NexusValue) -> anyhow::Result<Vec<u8>> {
    match value {
        NexusValue::Object { id } => Ok(id.address().as_bytes().to_vec()),
        NexusValue::InlineData { bytes } => Ok(Sha256::digest(bytes).to_vec()),
        NexusValue::WalrusData { content_digest, .. } => {
            if content_digest.len() != SHA256_LEN {
                bail!("Walrus content digest must contain exactly {SHA256_LEN} bytes");
            }
            Ok(content_digest.clone())
        }
    }
}

fn value_to_json(value: &NexusValue) -> Value {
    match value {
        NexusValue::Object { id } => json!({ "kind": "object", "id": id.to_string() }),
        NexusValue::InlineData { bytes } => json!({
            "kind": "data",
            "data": decode_inline_data_json(bytes),
        }),
        NexusValue::WalrusData {
            storage_key,
            content_digest,
        } => json!({
            "kind": "walrus",
            "storage_key": BASE64.encode(storage_key),
            "content_digest": BASE64.encode(content_digest),
        }),
    }
}

fn value_from_json(value: &Value) -> anyhow::Result<NexusValue> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("NexusData value must be a JSON object"))?;
    match object.get("kind").and_then(Value::as_str) {
        Some("object") => {
            ensure_exact_keys(object, &["kind", "id"], "Object NexusData value")?;
            let id = required_string(object.get("id"), "object id")?;
            Ok(NexusValue::object(
                sui::types::Address::from_str(id)
                    .with_context(|| format!("invalid Object ID '{id}'"))?,
            ))
        }
        Some("data") => {
            ensure_exact_keys(object, &["kind", "data"], "inline Data NexusData value")?;
            NexusValue::inline_data(serde_json::to_vec(&object["data"])?)
        }
        Some("walrus") => {
            ensure_exact_keys(
                object,
                &["kind", "storage_key", "content_digest"],
                "Walrus NexusData value",
            )?;
            NexusValue::walrus_data(
                decode_base64(object.get("storage_key"), "Walrus storage key")?,
                decode_base64(object.get("content_digest"), "Walrus content digest")?,
            )
        }
        Some(kind) => bail!("unknown NexusData value kind '{kind}'"),
        None => bail!("NexusData value is missing string property 'kind'"),
    }
}

fn validate_value_json(value: &Value) -> anyhow::Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("NexusData value must be a JSON object"))?;
    match object.get("kind").and_then(Value::as_str) {
        Some("object") => {
            ensure_exact_keys(object, &["kind", "id"], "Object NexusData value")?;
            let id = required_string(object.get("id"), "object id")?;
            sui::types::Address::from_str(id)
                .with_context(|| format!("invalid Object ID '{id}'"))?;
            Ok(())
        }
        Some("data") => ensure_exact_keys(object, &["kind", "data"], "inline Data NexusData value"),
        Some("walrus") => {
            ensure_exact_keys(
                object,
                &["kind", "storage_key", "content_digest"],
                "Walrus NexusData value",
            )?;
            let storage_key = decode_base64(object.get("storage_key"), "Walrus storage key")?;
            if storage_key.len() > MAX_WALRUS_STORAGE_KEY_BYTES {
                bail!("Walrus storage key exceeds {MAX_WALRUS_STORAGE_KEY_BYTES} bytes");
            }
            let digest = decode_base64(object.get("content_digest"), "Walrus content digest")?;
            if digest.len() != SHA256_LEN {
                bail!("Walrus content digest must contain exactly {SHA256_LEN} bytes");
            }
            Ok(())
        }
        Some(kind) => bail!("unknown NexusData value kind '{kind}'"),
        None => bail!("NexusData value is missing string property 'kind'"),
    }
}

fn value_json_is_object(value: &Value) -> bool {
    value.get("kind").and_then(Value::as_str) == Some("object")
}

fn decode_inline_data_json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(bytes).into_owned()))
}

fn ensure_exact_keys(
    object: &serde_json::Map<String, Value>,
    expected: &[&str],
    name: &str,
) -> anyhow::Result<()> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        bail!(
            "{name} must contain exactly the keys: {}",
            expected.join(", ")
        );
    }
    Ok(())
}

fn required_string<'a>(value: Option<&'a Value>, name: &str) -> anyhow::Result<&'a str> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing string {name}"))
}

fn decode_base64(value: Option<&Value>, name: &str) -> anyhow::Result<Vec<u8>> {
    let encoded = required_string(value, name)?;
    BASE64
        .decode(encoded)
        .with_context(|| format!("invalid RFC 4648 base64 {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_inline_value_roundtrips_bcs_and_json() {
        let value = NexusData::inline_data(br#"{"reason":"failure"}"#).unwrap();
        let encoded = bcs::to_bytes(&value).unwrap();
        assert_eq!(bcs::from_bytes::<NexusData>(&encoded).unwrap(), value);
        let json = value.to_json_value().unwrap();
        assert_eq!(
            json,
            json!({ "one": { "kind": "data", "data": { "reason": "failure" } } }),
        );
        assert_eq!(NexusData::from_json_value(&json).unwrap(), value);
    }

    #[test]
    fn direct_json_rejects_mixed_and_extra_outer_keys() {
        let data = json!({ "kind": "data", "data": null });

        assert!(NexusData::from_json_value(&json!({ "one": data, "many": [] })).is_err());
        assert!(NexusData::from_json_value(
            &json!({ "one": { "kind": "data", "data": null }, "extra": true }),
        )
        .is_err());
    }

    #[test]
    fn direct_json_rejects_extra_and_legacy_value_keys() {
        assert!(NexusData::from_json_value(
            &json!({ "one": { "kind": "object", "id": "0x1", "data": null } }),
        )
        .is_err());
        assert!(NexusData::from_json_value(
            &json!({ "one": { "kind": "data", "data": null, "storage": "inline" } }),
        )
        .is_err());
        assert!(
            NexusData::from_json_value(&json!({ "one": { "kind": "data", "bytes": "" } }),)
                .is_err()
        );
        assert!(NexusData::from_json_value(
            &json!({ "one": { "kind": "walrus", "storage_key": "", "content_digest": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "data": "legacy" } }),
        )
        .is_err());
    }

    #[test]
    fn direct_many_rejects_mixed_object_and_data_values() {
        let mixed = NexusData::many(vec![
            NexusValue::inline_data(b"inline").unwrap(),
            NexusValue::object(sui::types::Address::from_static("0x1")),
        ]);
        assert!(mixed.is_err());
    }

    #[test]
    fn empty_many_is_rejected_by_construction_json_storage_and_commitments() {
        let value = NexusData::new(b"nexus_value".to_vec(), Vec::new(), Vec::new());
        let json = json!({ "many": [] });

        assert!(NexusData::many(Vec::new()).is_err());
        assert!(NexusData::validate_json_value(&json).is_err());
        assert!(NexusData::from_json_value(&json).is_err());
        assert!(value.values().is_err());
        assert!(!value.is_well_formed());
        assert!(value.to_json_value().is_err());
        assert!(value.port_commitment(ValueKind::Data).is_err());
        assert!(value.port_commitment(ValueKind::Object).is_err());
    }

    #[test]
    fn aggregate_bound_accepts_max_count_with_bounded_payloads() {
        let value = NexusData::inline_data_many((0..128).map(|_| vec![0; 500])).unwrap();

        assert!(value.is_well_formed());
    }

    #[test]
    fn aggregate_bound_rejects_oversized_many() {
        let error = NexusData::inline_data_many((0..128).map(|_| vec![0; 512])).unwrap_err();

        assert!(error.to_string().contains("65536 encoded bytes"));
    }
}
