//! Ordered active-v3 off-chain Tool output body.

use {
    crate::move_bindings::primitives::data::NexusValue,
    serde::{Deserialize, Serialize},
};

/// One producer-named output port in the signed HTTP v3 body.
///
/// This type retains the Tool producer's raw name and witness group until MetaSchema validation;
/// it is not a second stored `NexusData` representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffchainToolOutputPort {
    pub port_name: Vec<u8>,
    pub values: Vec<NexusValue>,
}

/// Schema-ordered Tool output serialized directly as the signed HTTP v3 body.
///
/// The generated Move `TaggedOutput` contains named stored `NexusData`, so reusing it here would change the authenticated bytes and prevent the Leader from carrying decoded witnesses into the typed on-chain boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffchainToolOutput {
    pub tag: Vec<u8>,
    pub ports: Vec<OffchainToolOutputPort>,
}

impl OffchainToolOutput {
    /// Builds a response from explicit protocol values, including Walrus references.
    /// Supply ports in metadata order. The serving runtime validates the schema
    /// before signing the canonical response.
    pub fn from_ports(
        tag: impl Into<Vec<u8>>,
        ports: impl IntoIterator<Item = (String, crate::types::NexusData)>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            tag: tag.into(),
            ports: ports
                .into_iter()
                .map(|(name, data)| {
                    Ok(OffchainToolOutputPort {
                        port_name: name.into_bytes(),
                        values: data.into_values()?,
                    })
                })
                .collect::<anyhow::Result<_>>()?,
        })
    }

    /// Encodes ordinary externally tagged JSON outputs as inline protocol values.
    /// An array denotes a Many port; references require explicit `from_ports`.
    pub fn from_json(value: serde_json::Value) -> anyhow::Result<Self> {
        use crate::types::NexusData;
        let serde_json::Value::Object(variants) = value else {
            anyhow::bail!("tool output must serialize as an externally tagged enum")
        };
        anyhow::ensure!(
            variants.len() == 1,
            "tool output must contain exactly one variant"
        );
        let (tag, payload) = variants.into_iter().next().expect("length checked");
        let serde_json::Value::Object(payload) = payload else {
            anyhow::bail!("tool output variant payload must be an object")
        };
        let ports = payload
            .into_iter()
            .map(|(name, value)| {
                let data = if let serde_json::Value::Array(values) = value {
                    NexusData::inline_data_many(
                        values
                            .iter()
                            .map(serde_json::to_vec)
                            .collect::<Result<Vec<_>, _>>()?,
                    )?
                } else {
                    NexusData::inline_data(serde_json::to_vec(&value)?)?
                };
                Ok((name, data))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Self::from_ports(tag.into_bytes(), ports)
    }
}
