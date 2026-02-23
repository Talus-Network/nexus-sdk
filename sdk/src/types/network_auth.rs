//! Rust projections of `nexus_workflow::network_auth` on-chain types.

use {
    crate::{
        nexus::crawler::DynamicMap,
        sui,
        types::{
            deserialize_sui_option_u64,
            deserialize_sui_u64,
            serialize_sui_option_u64,
            serialize_sui_u64,
        },
    },
    base64::{prelude::BASE64_STANDARD, Engine as _},
    serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize},
};

/// Move `std::ascii::String` (a wrapper around `vector<u8>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveAsciiString {
    pub bytes: Vec<u8>,
}

impl MoveAsciiString {
    /// Construct from a Rust string.
    pub fn from_str(s: &str) -> Self {
        Self {
            bytes: s.as_bytes().to_vec(),
        }
    }

    /// Attempt to interpret the bytes as UTF-8.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.bytes).to_string()
    }
}

/// Move `nexus_workflow::network_auth::IdentityKey`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityKey {
    /// `IdentityKey::Leader { leader_cap_id }`
    Leader {
        /// ID of the leader's `leader_cap::OverNetwork` capability object.
        leader_cap_id: sui::types::Address,
    },
    /// `IdentityKey::Tool { fqn }`
    Tool { fqn: MoveAsciiString },
}

impl IdentityKey {
    /// Construct a leader identity key.
    pub fn leader(leader_cap_id: sui::types::Address) -> Self {
        Self::Leader { leader_cap_id }
    }

    /// Construct a tool identity key from a tool FQN string.
    pub fn tool_fqn(fqn: &str) -> Self {
        Self::Tool {
            fqn: MoveAsciiString::from_str(fqn),
        }
    }
}

impl<'de> Deserialize<'de> for MoveAsciiString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Standard {
            bytes: Vec<u8>,
        }

        if !deserializer.is_human_readable() {
            let value = Standard::deserialize(deserializer)?;
            return Ok(Self { bytes: value.bytes });
        }

        fn parse_bytes(value: serde_json::Value) -> Result<Vec<u8>, String> {
            match value {
                serde_json::Value::String(encoded) => BASE64_STANDARD
                    .decode(encoded)
                    .map_err(|e| format!("invalid base64 bytes: {e}")),
                serde_json::Value::Array(items) => items
                    .into_iter()
                    .map(|v| {
                        v.as_u64()
                            .and_then(|n| u8::try_from(n).ok())
                            .ok_or_else(|| "expected byte array (u8)".to_string())
                    })
                    .collect(),
                other => Err(format!("unexpected bytes value: {other}")),
            }
        }

        let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
        match value {
            serde_json::Value::Object(mut obj) => {
                let bytes = obj
                    .remove("bytes")
                    .ok_or_else(|| serde::de::Error::custom("MoveAsciiString missing 'bytes'"))?;
                let bytes = parse_bytes(bytes).map_err(serde::de::Error::custom)?;
                Ok(Self { bytes })
            }
            serde_json::Value::String(encoded) => {
                let bytes = parse_bytes(serde_json::Value::String(encoded))
                    .map_err(serde::de::Error::custom)?;
                Ok(Self { bytes })
            }
            other => Err(serde::de::Error::custom(format!(
                "unexpected MoveAsciiString value: {other}"
            ))),
        }
    }
}

impl Serialize for MoveAsciiString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Standard<'a> {
            bytes: &'a [u8],
        }

        if !serializer.is_human_readable() {
            return Standard { bytes: &self.bytes }.serialize(serializer);
        }

        #[derive(Serialize)]
        struct Human<'a> {
            bytes: &'a str,
        }

        let encoded = BASE64_STANDARD.encode(&self.bytes);
        Human { bytes: &encoded }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for IdentityKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum Standard {
            Leader { leader_cap_id: sui::types::Address },
            Tool { fqn: MoveAsciiString },
        }

        if !deserializer.is_human_readable() {
            return match Standard::deserialize(deserializer)? {
                Standard::Leader { leader_cap_id } => Ok(Self::Leader { leader_cap_id }),
                Standard::Tool { fqn } => Ok(Self::Tool { fqn }),
            };
        }

        let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
        let obj = value.as_object().ok_or_else(|| {
            serde::de::Error::custom(format!("unexpected IdentityKey value: {value}"))
        })?;

        // Accept external tagging: { "Leader": { ... } } / { "Tool": { ... } }.
        if obj.len() == 1 {
            if let Some(inner) = obj.get("Leader") {
                #[derive(Deserialize)]
                struct LeaderFields {
                    leader_cap_id: sui::types::Address,
                }
                let fields: LeaderFields =
                    serde_json::from_value(inner.clone()).map_err(serde::de::Error::custom)?;
                return Ok(Self::Leader {
                    leader_cap_id: fields.leader_cap_id,
                });
            }

            if let Some(inner) = obj.get("Tool") {
                #[derive(Deserialize)]
                struct ToolFields {
                    fqn: MoveAsciiString,
                }
                let fields: ToolFields =
                    serde_json::from_value(inner.clone()).map_err(serde::de::Error::custom)?;
                return Ok(Self::Tool { fqn: fields.fqn });
            }
        }

        // Accept Sui JSON variant tagging: { "@variant": "...", ... } (or "variant"/"_variant_name").
        let variant = obj
            .get("@variant")
            .or_else(|| obj.get("variant"))
            .or_else(|| obj.get("_variant_name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| serde::de::Error::custom("IdentityKey missing variant tag"))?;

        let fields = obj.get("fields").and_then(|v| v.as_object()).unwrap_or(obj);

        match variant {
            "Leader" => {
                let leader_cap_id = fields.get("leader_cap_id").ok_or_else(|| {
                    serde::de::Error::custom("IdentityKey::Leader missing leader_cap_id")
                })?;
                let leader_cap_id =
                    serde_json::from_value::<sui::types::Address>(leader_cap_id.clone())
                        .map_err(serde::de::Error::custom)?;
                Ok(Self::Leader { leader_cap_id })
            }
            "Tool" => {
                let fqn = fields
                    .get("fqn")
                    .ok_or_else(|| serde::de::Error::custom("IdentityKey::Tool missing fqn"))?;
                let fqn = serde_json::from_value::<MoveAsciiString>(fqn.clone())
                    .map_err(serde::de::Error::custom)?;
                Ok(Self::Tool { fqn })
            }
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["Leader", "Tool"],
            )),
        }
    }
}

impl Serialize for IdentityKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        enum Standard<'a> {
            Leader { leader_cap_id: sui::types::Address },
            Tool { fqn: &'a MoveAsciiString },
        }

        if !serializer.is_human_readable() {
            return match self {
                Self::Leader { leader_cap_id } => Standard::Leader {
                    leader_cap_id: *leader_cap_id,
                }
                .serialize(serializer),
                Self::Tool { fqn } => Standard::Tool { fqn }.serialize(serializer),
            };
        }

        let mut map = serde_json::Map::new();
        match self {
            Self::Leader { leader_cap_id } => {
                map.insert(
                    "@variant".to_string(),
                    serde_json::Value::String("Leader".to_string()),
                );
                map.insert(
                    "leader_cap_id".to_string(),
                    serde_json::to_value(leader_cap_id).map_err(serde::ser::Error::custom)?,
                );
            }
            Self::Tool { fqn } => {
                map.insert(
                    "@variant".to_string(),
                    serde_json::Value::String("Tool".to_string()),
                );
                map.insert(
                    "fqn".to_string(),
                    serde_json::to_value(fqn).map_err(serde::ser::Error::custom)?,
                );
            }
        }
        serde_json::Value::Object(map).serialize(serializer)
    }
}

/// Move `sui::vec_set::VecSet<T>`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MoveVecSet<T> {
    pub contents: Vec<T>,
}

/// Move `nexus_workflow::network_auth::NetworkAuth`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct NetworkAuth {
    pub id: sui::types::Address,
    pub identities: MoveVecSet<IdentityKey>,
}

/// Move `nexus_workflow::network_auth::KeyRecord`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct KeyRecord {
    pub scheme: u8,
    #[serde(
        deserialize_with = "crate::types::deserialize_encoded_bytes",
        serialize_with = "crate::types::serialize_encoded_bytes"
    )]
    pub public_key: Vec<u8>,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub added_at_ms: u64,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub revoked_at_ms: Option<u64>,
}

/// Minimal projection of `nexus_workflow::network_auth::KeyBinding`.
#[derive(Clone, Debug, Deserialize)]
pub struct KeyBinding {
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_sui_u64")]
    pub next_key_id: u64,
    #[serde(deserialize_with = "deserialize_sui_option_u64")]
    pub active_key_id: Option<u64>,
    /// Dynamic fields backing the on-chain `Table<u64, KeyRecord>`.
    pub keys: DynamicMap<u64, KeyRecord>,
}

#[cfg(test)]
mod tests {
    use {super::*, bcs, serde_json::json};

    #[test]
    fn move_ascii_string_roundtrip() {
        let value = MoveAsciiString::from_str("nexus");
        assert_eq!(value.bytes, b"nexus".to_vec());
        assert_eq!(value.to_string_lossy(), "nexus");
    }

    #[test]
    fn identity_key_helpers() {
        let mut rng = rand::thread_rng();
        let address = sui::types::Address::generate(&mut rng);

        let leader = IdentityKey::leader(address);
        assert_eq!(
            leader,
            IdentityKey::Leader {
                leader_cap_id: address
            }
        );

        let tool = IdentityKey::tool_fqn("xyz.demo.tool@1");
        assert_eq!(
            tool,
            IdentityKey::Tool {
                fqn: MoveAsciiString::from_str("xyz.demo.tool@1")
            }
        );
    }

    #[test]
    fn identity_key_bcs_roundtrip() {
        let mut rng = rand::thread_rng();
        let address = sui::types::Address::generate(&mut rng);
        let key = IdentityKey::leader(address);
        let bytes = bcs::to_bytes(&key).unwrap();
        let decoded: IdentityKey = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, key);
    }

    #[test]
    fn identity_key_deserializes_variant_tagged_json() {
        let leader_cap_id = "0x1111";
        let key: IdentityKey =
            serde_json::from_value(json!({"@variant":"Leader","leader_cap_id": leader_cap_id}))
                .unwrap();
        assert_eq!(
            key,
            IdentityKey::Leader {
                leader_cap_id: leader_cap_id.parse().unwrap()
            }
        );

        let tool_fqn = "xyz.demo.tool@1";
        let tool_fqn_b64 = BASE64_STANDARD.encode(tool_fqn.as_bytes());
        let key: IdentityKey = serde_json::from_value(json!({
            "@variant":"Tool",
            "fqn": { "bytes": tool_fqn_b64 }
        }))
        .unwrap();
        match key {
            IdentityKey::Tool { fqn } => assert_eq!(fqn.to_string_lossy(), tool_fqn),
            other => panic!("expected Tool identity, got {other:?}"),
        }
    }

    #[test]
    fn identity_key_deserializes_externally_tagged_json() {
        let leader_cap_id = "0x2222";
        let key: IdentityKey = serde_json::from_value(json!({
            "Leader": { "leader_cap_id": leader_cap_id }
        }))
        .unwrap();
        assert_eq!(
            key,
            IdentityKey::Leader {
                leader_cap_id: leader_cap_id.parse().unwrap()
            }
        );
    }

    #[test]
    fn network_auth_deserializes_identities_vecset() {
        let leader_cap_id = "0x3333";
        let tool_fqn = "xyz.demo.tool@1";
        let tool_fqn_b64 = BASE64_STANDARD.encode(tool_fqn.as_bytes());

        let auth: NetworkAuth = serde_json::from_value(json!({
            "id": "0x1",
            "identities": {
                "contents": [
                    {"@variant":"Leader","leader_cap_id": leader_cap_id},
                    {"@variant":"Tool","fqn": {"bytes": tool_fqn_b64}}
                ]
            }
        }))
        .unwrap();

        assert_eq!(auth.identities.contents.len(), 2);
    }
}
