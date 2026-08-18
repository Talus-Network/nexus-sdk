//! Typed Tool schema conversion, validation, and runtime conformance helpers.

use {
    super::nexus_data::{
        resolved_values_from_json,
        resolved_values_port_commitment,
        resolved_values_to_json,
        validate_resolved_values,
    },
    crate::{
        move_bindings::{
            interface::meta_schema::{MetaSchema, OutputVariantSchema, PortSchema, ValueKind},
            protocol_limits::{
                interface::meta_schema::{
                    MAX_IDENTIFIER_BYTES,
                    MAX_INPUT_PORTS,
                    MAX_META_SCHEMA_BYTES,
                    MAX_OUTPUT_VARIANTS,
                    MAX_PORTS_PER_OUTPUT_VARIANT,
                    MAX_RAW_OUTPUT_BYTES,
                },
                primitives::data::MAX_NEXUS_DATA_BYTES,
            },
        },
        types::{NexusData, NexusValue, OffchainToolOutput},
    },
    anyhow::{anyhow, bail, Context as _},
    serde::Serialize,
    serde_json::{json, Map, Value},
    sha2::{Digest as _, Sha256},
    std::collections::{HashMap, HashSet},
};

const FAILURE_VARIANT: &[u8] = b"_err_eval";
const FAILURE_PORT: &[u8] = b"reason";
pub const RESOLVED_INPUT_DOMAIN: &[u8] = b"nexus.direct.v3.resolved-input";

impl MetaSchema {
    /// Converts Schemars-style off-chain input and externally tagged output schemas.
    pub fn from_offchain_json_schemas(
        input_schema: &[u8],
        output_schema: &[u8],
    ) -> anyhow::Result<Self> {
        let input_root = serde_json::from_slice::<Value>(input_schema)
            .context("Tool input schema is not valid JSON")?;
        let output_root = serde_json::from_slice::<Value>(output_schema)
            .context("Tool output schema is not valid JSON")?;
        let input_ports = resolved_object_properties(&input_root, &input_root)?
            .iter()
            .map(|(name, schema)| {
                Ok(PortSchema::new(
                    name.as_bytes().to_vec(),
                    json_is_many(&input_root, schema)?,
                    ValueKind::Data,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let variants = output_root
            .get("oneOf")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Tool output schema must contain a top-level 'oneOf' array"))?;
        let output_variants = variants
            .iter()
            .map(|variant| offchain_output_variant(&output_root, variant))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let schema = Self::new(input_ports, output_variants);
        schema.validate_for_tool(true)?;
        Ok(schema)
    }

    /// Converts on-chain Move-introspection JSON into the protocol schema.
    pub fn from_onchain_json_schemas(
        input_schema: &str,
        output_schema: &str,
    ) -> anyhow::Result<Self> {
        let input_root = serde_json::from_str::<Value>(input_schema)
            .context("on-chain Tool input schema is not valid JSON")?;
        let output_root = serde_json::from_str::<Value>(output_schema)
            .context("on-chain Tool output schema is not valid JSON")?;
        let input_ports = input_root
            .as_object()
            .ok_or_else(|| anyhow!("on-chain Tool input schema must be a JSON object"))?
            .iter()
            .map(|(name, schema)| {
                Ok(PortSchema::new(
                    name.as_bytes().to_vec(),
                    json_is_many(&input_root, schema)?,
                    json_value_kind(&input_root, schema)?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let output_variants = output_root
            .as_object()
            .ok_or_else(|| anyhow!("on-chain Tool output schema must be a JSON object"))?
            .iter()
            .map(|(variant_name, variant)| {
                let fields = variant
                    .get("fields")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        anyhow!(
                            "on-chain output variant '{variant_name}' is missing object 'fields'"
                        )
                    })?;
                let ports = fields
                    .iter()
                    .map(|(port_name, schema)| {
                        Ok(PortSchema::new(
                            port_name.as_bytes().to_vec(),
                            json_is_many(&output_root, schema)?,
                            json_value_kind(&output_root, schema)?,
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                Ok(OutputVariantSchema::new(
                    variant_name.as_bytes().to_vec(),
                    ports,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let schema = Self::new(input_ports, output_variants);
        schema.validate_for_tool(false)?;
        Ok(schema)
    }

    /// Mirrors the authoritative Move structural validation and resource limits.
    pub fn validate_for_tool(&self, is_offchain: bool) -> anyhow::Result<()> {
        if self.input_ports.len() > MAX_INPUT_PORTS as usize {
            bail!("MetaSchema exceeds {MAX_INPUT_PORTS} input ports");
        }
        if self.output_variants.is_empty()
            || self.output_variants.len() > MAX_OUTPUT_VARIANTS as usize
        {
            bail!("MetaSchema must contain 1..={MAX_OUTPUT_VARIANTS} output variants");
        }
        if bcs::to_bytes(self)?.len() > MAX_META_SCHEMA_BYTES as usize {
            bail!("MetaSchema exceeds {MAX_META_SCHEMA_BYTES} encoded bytes");
        }
        validate_unique_ports(&self.input_ports, "input")?;
        if is_offchain
            && self
                .input_ports
                .iter()
                .any(|port| port.value_kind != ValueKind::Data)
        {
            bail!("off-chain input ports must contain opaque Data values");
        }

        let mut variants = HashSet::new();
        for variant in &self.output_variants {
            validate_identifier(&variant.variant_name, "output variant")?;
            if variant.variant_name == FAILURE_VARIANT {
                bail!("reserved output variant '_err_eval' cannot be registered");
            }
            if !variants.insert(variant.variant_name.as_slice()) {
                bail!("duplicate output variant name");
            }
            if variant.ports.len() > MAX_PORTS_PER_OUTPUT_VARIANT as usize {
                bail!("output variant exceeds {MAX_PORTS_PER_OUTPUT_VARIANT} ports");
            }
            validate_unique_ports(&variant.ports, "output")?;
        }
        Ok(())
    }

    /// Returns whether a typed value exactly conforms to one port schema.
    pub fn conforms_port(schema: &PortSchema, value: &NexusData) -> bool {
        if bcs::to_bytes(value).map_or(true, |bytes| bytes.len() > MAX_NEXUS_DATA_BYTES as usize)
            || !value.is_well_formed()
        {
            return false;
        }
        if schema.is_many != value.is_many() {
            return false;
        }
        match schema.value_kind {
            ValueKind::Object => value.is_object(),
            ValueKind::Data => value.is_data(),
        }
    }

    /// Returns whether transient resolved values exactly conform to one port schema.
    pub fn conforms_resolved_port(schema: &PortSchema, values: &[NexusValue]) -> bool {
        if validate_resolved_values(values, schema.is_many).is_err() {
            return false;
        }
        match schema.value_kind {
            ValueKind::Object => values.iter().all(NexusValue::is_object),
            ValueKind::Data => values
                .iter()
                .all(|value| matches!(value, NexusValue::InlineData { .. })),
        }
    }

    /// Returns complete typed inputs in immutable schema order.
    pub fn canonical_input_values(
        &self,
        input_ports: &HashMap<String, NexusData>,
    ) -> anyhow::Result<Vec<NexusData>> {
        if input_ports.len() != self.input_ports.len() {
            bail!(
                "Tool input contains {} ports but schema requires {}",
                input_ports.len(),
                self.input_ports.len()
            );
        }
        self.input_ports
            .iter()
            .map(|port| {
                let name =
                    std::str::from_utf8(&port.port_name).context("input port name is not UTF-8")?;
                let value = input_ports
                    .get(name)
                    .ok_or_else(|| anyhow!("Tool input is missing schema port '{name}'"))?;
                if !Self::conforms_port(port, value) {
                    bail!("Tool input port '{name}' does not conform to MetaSchema");
                }
                Ok(value.clone())
            })
            .collect()
    }

    /// Encodes complete resolved off-chain inputs in immutable schema order.
    pub fn resolved_inputs_to_json(
        &self,
        input_ports: &HashMap<String, Vec<NexusValue>>,
    ) -> anyhow::Result<Value> {
        let ordered = self.resolved_input_values(input_ports)?;
        Ok(json!({
            "ports": self
                .input_ports
                .iter()
                .zip(ordered)
                .map(|(port, value)| {
                    Ok(json!({
                        "port_name": utf8_name(&port.port_name, "input port")?,
                        "value": resolved_values_to_json(value, port.is_many)?,
                    }))
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
        }))
    }

    /// Parses the strict named transport and returns direct resolved value vectors.
    pub fn resolved_inputs_from_json(
        &self,
        value: &Value,
    ) -> anyhow::Result<HashMap<String, Vec<NexusValue>>> {
        let object = value
            .as_object()
            .ok_or_else(|| anyhow!("resolved Tool inputs must be a JSON object"))?;
        if object.len() != 1 || !object.contains_key("ports") {
            bail!("resolved Tool inputs must contain only the 'ports' property");
        }
        let ports = object["ports"]
            .as_array()
            .ok_or_else(|| anyhow!("resolved Tool input 'ports' must be an array"))?;
        let mut names = HashSet::new();
        let input_ports = ports
            .iter()
            .map(|entry| {
                let entry = entry
                    .as_object()
                    .ok_or_else(|| anyhow!("resolved Tool input port must be an object"))?;
                if entry.len() != 2
                    || !entry.contains_key("port_name")
                    || !entry.contains_key("value")
                {
                    bail!("resolved Tool input port must contain 'port_name' and 'value'");
                }
                let port_name = entry["port_name"]
                    .as_str()
                    .ok_or_else(|| anyhow!("resolved Tool input port name must be a string"))?;
                if !names.insert(port_name.to_owned()) {
                    bail!("resolved Tool inputs contain duplicate port '{port_name}'");
                }
                let port_schema = self
                    .input_ports
                    .iter()
                    .find(|port| port.port_name == port_name.as_bytes())
                    .ok_or_else(|| {
                        anyhow!("Tool input contains unknown schema port '{port_name}'")
                    })?;
                let (values, many) = resolved_values_from_json(&entry["value"])?;
                if port_schema.is_many != many {
                    bail!("Tool input port '{port_name}' cardinality does not match MetaSchema");
                }
                Ok((port_name.to_owned(), values))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()?;
        self.resolved_input_values(&input_ports)?;
        Ok(input_ports)
    }

    /// Returns the RegisteredKey commitment over exact schema names and resolved content.
    pub fn resolved_inputs_sha256(
        &self,
        input_ports: &HashMap<String, Vec<NexusValue>>,
    ) -> anyhow::Result<[u8; 32]> {
        let ordered = self.resolved_input_values(input_ports)?;
        self.resolved_input_commitment_sha256(&ordered)
    }

    /// Returns the Move-compatible commitment for canonical on-chain or off-chain inputs.
    pub fn canonical_inputs_sha256(
        &self,
        input_ports: &HashMap<String, NexusData>,
    ) -> anyhow::Result<[u8; 32]> {
        self.validate_for_tool(false)?;
        let ordered = self.canonical_input_values(input_ports)?;
        self.input_commitment_sha256(&ordered)
    }

    /// Projects resolved inline Data into the semantic JSON consumed by a Toolkit Tool.
    pub fn resolved_inputs_to_semantic_json(
        &self,
        input_ports: &HashMap<String, Vec<NexusValue>>,
    ) -> anyhow::Result<Value> {
        let ordered = self.resolved_input_values(input_ports)?;
        let mut semantic = Map::new();
        for (port, value) in self.input_ports.iter().zip(ordered) {
            let port_name = utf8_name(&port.port_name, "input port")?;
            let decoded = if port.is_many {
                Value::Array(
                    value
                        .iter()
                        .map(|value| offchain_element_json(port_name, value))
                        .collect::<anyhow::Result<Vec<_>>>()?,
                )
            } else {
                offchain_element_json(
                    port_name,
                    value
                        .first()
                        .expect("well-formed One contains exactly one value"),
                )?
            };
            semantic.insert(port_name.to_owned(), decoded);
        }
        Ok(Value::Object(semantic))
    }

    fn resolved_input_values<'a>(
        &self,
        input_ports: &'a HashMap<String, Vec<NexusValue>>,
    ) -> anyhow::Result<Vec<&'a [NexusValue]>> {
        self.validate_for_tool(true)?;
        if input_ports.len() != self.input_ports.len() {
            bail!(
                "Tool input contains {} ports but schema requires {}",
                input_ports.len(),
                self.input_ports.len()
            );
        }
        self.input_ports
            .iter()
            .map(|port| {
                let name = utf8_name(&port.port_name, "input port")?;
                let value = input_ports
                    .get(name)
                    .ok_or_else(|| anyhow!("Tool input is missing schema port '{name}'"))?;
                if !Self::conforms_resolved_port(port, value) {
                    bail!("Tool input port '{name}' does not conform to MetaSchema");
                }
                Ok(value.as_slice())
            })
            .collect()
    }

    fn resolved_input_commitment_sha256(
        &self,
        ordered: &[&[NexusValue]],
    ) -> anyhow::Result<[u8; 32]> {
        #[derive(Serialize)]
        struct PortInputCommitment<'a> {
            port_name: &'a [u8],
            commitment: crate::move_bindings::interface::meta_schema::PortCommitment,
        }

        let commitments = self
            .input_ports
            .iter()
            .zip(ordered)
            .map(|(port, value)| {
                Ok(PortInputCommitment {
                    port_name: &port.port_name,
                    commitment: resolved_values_port_commitment(
                        value,
                        port.value_kind,
                        port.is_many,
                    )?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mut hasher = Sha256::new();
        hasher.update(RESOLVED_INPUT_DOMAIN);
        hasher.update(bcs::to_bytes(&commitments)?);
        Ok(hasher.finalize().into())
    }

    fn input_commitment_sha256(&self, ordered: &[NexusData]) -> anyhow::Result<[u8; 32]> {
        #[derive(Serialize)]
        struct PortInputCommitment<'a> {
            port_name: &'a [u8],
            commitment: crate::move_bindings::interface::meta_schema::PortCommitment,
        }

        let commitments = self
            .input_ports
            .iter()
            .zip(ordered)
            .map(|(port, value)| {
                Ok(PortInputCommitment {
                    port_name: &port.port_name,
                    commitment: value.port_commitment(port.value_kind)?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mut hasher = Sha256::new();
        hasher.update(RESOLVED_INPUT_DOMAIN);
        hasher.update(bcs::to_bytes(&commitments)?);
        Ok(hasher.finalize().into())
    }

    /// Validates one exact schema-ordered canonical Tool response.
    pub fn conforms_raw_output(&self, output: &OffchainToolOutput) -> bool {
        if bcs::to_bytes(output).map_or(true, |bytes| bytes.len() > MAX_RAW_OUTPUT_BYTES as usize) {
            return false;
        }
        if output.tag == FAILURE_VARIANT {
            return Self::conforms_safe_failure(output);
        }
        let Some(variant) = self
            .output_variants
            .iter()
            .find(|variant| variant.variant_name == output.tag)
        else {
            return false;
        };
        variant.ports.len() == output.ports.len()
            && variant
                .ports
                .iter()
                .zip(&output.ports)
                .all(|(schema, port)| {
                    schema.port_name == port.port_name
                        && NexusData::from_values(port.values.clone(), schema.is_many)
                            .is_ok_and(|value| Self::conforms_port(schema, &value))
                })
    }

    /// Projects one valid canonical response into schema-named typed output ports.
    pub fn canonical_output_ports(
        &self,
        output: &OffchainToolOutput,
    ) -> anyhow::Result<Vec<(Vec<u8>, NexusData)>> {
        if !self.conforms_raw_output(output) {
            bail!("canonical Tool response does not conform to MetaSchema");
        }
        if output.tag == FAILURE_VARIANT {
            return Ok(vec![(
                FAILURE_PORT.to_vec(),
                NexusData::from_values(output.ports[0].values.clone(), false)?,
            )]);
        }
        let variant = self
            .output_variants
            .iter()
            .find(|variant| variant.variant_name == output.tag)
            .expect("conforming response variant exists");
        variant
            .ports
            .iter()
            .zip(&output.ports)
            .map(|(schema, port)| {
                Ok((
                    port.port_name.clone(),
                    NexusData::from_values(port.values.clone(), schema.is_many)?,
                ))
            })
            .collect()
    }

    /// Returns true only for the universally routable typed `_err_eval` representation.
    fn conforms_safe_failure(output: &OffchainToolOutput) -> bool {
        if output.tag != FAILURE_VARIANT {
            return false;
        }
        let [port] = output.ports.as_slice() else {
            return false;
        };
        if port.port_name != FAILURE_PORT {
            return false;
        }
        NexusData::from_values(port.values.clone(), false).is_ok_and(|value| {
            bcs::to_bytes(&value).is_ok_and(|bytes| bytes.len() <= MAX_NEXUS_DATA_BYTES as usize)
                && value.inline_data_bytes().is_some()
        })
    }

    /// Encodes a readable deterministic JSON projection of the typed schema.
    pub fn to_json_value(&self) -> anyhow::Result<Value> {
        Ok(json!({
            "input_ports": self
                .input_ports
                .iter()
                .map(port_schema_json)
                .collect::<anyhow::Result<Vec<_>>>()?,
            "output_variants": self
                .output_variants
                .iter()
                .map(|variant| {
                    Ok(json!({
                        "variant_name": utf8_name(&variant.variant_name, "variant")?,
                        "ports": variant
                            .ports
                            .iter()
                            .map(port_schema_json)
                            .collect::<anyhow::Result<Vec<_>>>()?,
                    }))
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
        }))
    }
}

fn offchain_element_json(
    port_name: &str,
    value: &crate::move_bindings::primitives::data::NexusValue,
) -> anyhow::Result<Value> {
    let crate::move_bindings::primitives::data::NexusValue::InlineData { bytes } = value else {
        bail!("off-chain Tool input port '{port_name}' must contain resolved inline Data");
    };
    serde_json::from_slice(bytes)
        .with_context(|| format!("off-chain Tool input port '{port_name}' is not JSON"))
}

fn offchain_output_variant(root: &Value, variant: &Value) -> anyhow::Result<OutputVariantSchema> {
    let variant = resolve_local_ref(root, variant)?;
    if let Some(name) = variant.get("const").and_then(Value::as_str).or_else(|| {
        variant
            .get("enum")
            .and_then(Value::as_array)
            .and_then(|values| (values.len() == 1).then(|| values[0].as_str()).flatten())
    }) {
        return Ok(OutputVariantSchema::new(name.as_bytes().to_vec(), vec![]));
    }
    let variants = variant
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("output oneOf item must describe one externally tagged variant"))?;
    let variant_entries = variants.iter().collect::<Vec<_>>();
    let [(variant_name, payload)] = variant_entries.as_slice() else {
        bail!("output oneOf item must contain exactly one variant property");
    };
    let properties = resolve_local_ref(root, payload)?
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let ports = properties
        .iter()
        .map(|(port_name, schema)| {
            Ok(PortSchema::new(
                port_name.as_bytes().to_vec(),
                json_is_many(root, schema)?,
                ValueKind::Data,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(OutputVariantSchema::new(
        variant_name.as_bytes().to_vec(),
        ports,
    ))
}

fn resolved_object_properties<'a>(
    root: &'a Value,
    schema: &'a Value,
) -> anyhow::Result<&'a Map<String, Value>> {
    resolve_local_ref(root, schema)?
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("input schema must contain object 'properties'"))
}

fn resolve_local_ref<'a>(root: &'a Value, schema: &'a Value) -> anyhow::Result<&'a Value> {
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return Ok(schema);
    };
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| anyhow!("only local JSON schema references are supported"))?;
    root.pointer(pointer)
        .ok_or_else(|| anyhow!("JSON schema reference '{reference}' does not exist"))
}

fn json_is_many(root: &Value, schema: &Value) -> anyhow::Result<bool> {
    let schema = resolve_local_ref(root, schema)?;
    Ok(matches!(
        schema.get("type").and_then(Value::as_str),
        Some("array" | "vector")
    ))
}

fn json_value_kind(root: &Value, schema: &Value) -> anyhow::Result<ValueKind> {
    let schema = resolve_local_ref(root, schema)?;
    match schema.get("type").and_then(Value::as_str) {
        Some("object" | "object_id") => Ok(ValueKind::Object),
        Some("vector") => {
            let element = schema
                .get("element_type")
                .ok_or_else(|| anyhow!("vector schema is missing 'element_type'"))?;
            json_value_kind(root, element)
        }
        _ => Ok(ValueKind::Data),
    }
}

fn validate_unique_ports(ports: &[PortSchema], kind: &str) -> anyhow::Result<()> {
    let mut names = HashSet::new();
    for port in ports {
        validate_identifier(&port.port_name, kind)?;
        if !names.insert(port.port_name.as_slice()) {
            bail!("duplicate {kind} port name");
        }
    }
    Ok(())
}

fn validate_identifier(name: &[u8], kind: &str) -> anyhow::Result<()> {
    if name.is_empty() || name.len() > MAX_IDENTIFIER_BYTES as usize || !name.is_ascii() {
        bail!("{kind} name must contain 1..={MAX_IDENTIFIER_BYTES} ASCII bytes");
    }
    Ok(())
}

fn utf8_name<'a>(name: &'a [u8], kind: &str) -> anyhow::Result<&'a str> {
    std::str::from_utf8(name).with_context(|| format!("{kind} name is not UTF-8"))
}

fn port_schema_json(port: &PortSchema) -> anyhow::Result<Value> {
    Ok(json!({
        "port_name": utf8_name(&port.port_name, "port")?,
        "cardinality": if port.is_many { "many" } else { "one" },
        "value_kind": match port.value_kind {
            ValueKind::Object => "object",
            ValueKind::Data => "data",
        },
    }))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::OffchainToolOutputPort};

    fn output_port(port_name: &[u8], value: NexusData) -> OffchainToolOutputPort {
        OffchainToolOutputPort {
            port_name: port_name.to_vec(),
            values: value.into_values().expect("fixture should decode"),
        }
    }

    fn canonical_schema_mismatch_failure() -> OffchainToolOutput {
        OffchainToolOutput {
            tag: FAILURE_VARIANT.to_vec(),
            ports: vec![OffchainToolOutputPort {
                port_name: FAILURE_PORT.to_vec(),
                values: NexusData::inline_data(b"Tool output schema mismatch")
                    .unwrap()
                    .into_values()
                    .expect("fixture should decode"),
            }],
        }
    }

    fn offchain_schema() -> MetaSchema {
        MetaSchema::from_offchain_json_schemas(
            br#"{"type":"object","properties":{"prompt":{"type":"string"}}}"#,
            br#"{"oneOf":[{"type":"object","properties":{"Ok":{"type":"object","properties":{"message":{"type":"string"},"items":{"type":"array"}}}}}]}"#,
        )
        .expect("schema fixture should convert")
    }

    #[test]
    fn offchain_json_conversion_preserves_ports_and_array_cardinality() {
        let schema = offchain_schema();

        assert_eq!(schema.input_ports[0].port_name, b"prompt");
        assert_eq!(schema.output_variants[0].variant_name, b"Ok");
        assert!(schema.output_variants[0].ports[1].is_many);
        assert_eq!(schema.input_ports[0].value_kind, ValueKind::Data);
    }

    #[test]
    fn raw_output_conformance_rejects_wrong_output_group_count() {
        let schema = offchain_schema();
        let value = NexusData::inline_data(br#""ok""#)
            .unwrap()
            .into_values()
            .expect("fixture should decode");
        let output = OffchainToolOutput {
            tag: b"Ok".to_vec(),
            ports: vec![
                OffchainToolOutputPort {
                    port_name: b"message".to_vec(),
                    values: value.clone(),
                },
                OffchainToolOutputPort {
                    port_name: b"items".to_vec(),
                    values: value,
                },
                OffchainToolOutputPort {
                    port_name: b"extra".to_vec(),
                    values: Vec::new(),
                },
            ],
        };

        assert!(!schema.conforms_raw_output(&output));
    }

    #[test]
    fn raw_output_conformance_authenticates_same_typed_port_names_and_order() {
        let schema = MetaSchema::new(
            Vec::new(),
            vec![OutputVariantSchema::new(
                b"Ok".to_vec(),
                vec![
                    PortSchema::new(b"first".to_vec(), false, ValueKind::Data),
                    PortSchema::new(b"second".to_vec(), false, ValueKind::Data),
                ],
            )],
        );
        let first = NexusData::inline_data(b"same-kind-one").unwrap();
        let second = NexusData::inline_data(b"same-kind-two").unwrap();
        let conforming = OffchainToolOutput {
            tag: b"Ok".to_vec(),
            ports: vec![
                output_port(b"first", first.clone()),
                output_port(b"second", second.clone()),
            ],
        };
        let renamed = OffchainToolOutput {
            tag: b"Ok".to_vec(),
            ports: vec![
                output_port(b"renamed", first.clone()),
                output_port(b"second", second.clone()),
            ],
        };
        let swapped = OffchainToolOutput {
            tag: b"Ok".to_vec(),
            ports: vec![output_port(b"second", second), output_port(b"first", first)],
        };

        assert!(schema.conforms_raw_output(&conforming));
        assert!(schema.canonical_output_ports(&conforming).is_ok());
        for malformed in [renamed, swapped] {
            assert!(!schema.conforms_raw_output(&malformed));
            assert!(schema.canonical_output_ports(&malformed).is_err());
        }
    }

    #[test]
    fn canonical_output_projection_rejects_missing_and_extra_named_ports() {
        let schema = offchain_schema();
        let message = NexusData::inline_data(br#""ok""#).unwrap();
        let items = NexusData::inline_data_many([b"one".as_slice()]).unwrap();
        let missing = OffchainToolOutput {
            tag: b"Ok".to_vec(),
            ports: vec![output_port(b"message", message.clone())],
        };
        let extra = OffchainToolOutput {
            tag: b"Ok".to_vec(),
            ports: vec![
                output_port(b"message", message),
                output_port(b"items", items),
                output_port(b"extra", NexusData::inline_data(b"extra").unwrap()),
            ],
        };

        assert!(schema.canonical_output_ports(&missing).is_err());
        assert!(schema.canonical_output_ports(&extra).is_err());
    }

    #[test]
    fn canonical_output_projection_restores_schema_port_names_and_cardinality() {
        let schema = offchain_schema();
        let message = NexusData::inline_data(br#""ok""#).unwrap();
        let items = NexusData::inline_data_many([b"one".as_slice(), b"two".as_slice()]).unwrap();
        let output = OffchainToolOutput {
            tag: b"Ok".to_vec(),
            ports: vec![
                OffchainToolOutputPort {
                    port_name: b"message".to_vec(),
                    values: message.into_values().expect("fixture should decode"),
                },
                OffchainToolOutputPort {
                    port_name: b"items".to_vec(),
                    values: items.into_values().expect("fixture should decode"),
                },
            ],
        };

        let ports = schema.canonical_output_ports(&output).unwrap();

        assert_eq!(ports[0].0, b"message");
        assert!(ports[0].1.is_one());
        assert_eq!(ports[1].0, b"items");
        assert!(ports[1].1.is_many());
    }

    #[test]
    fn runtime_conformance_checks_cardinality_and_value_kind() {
        let schema = PortSchema::new(b"input".to_vec(), false, ValueKind::Object);
        let object = NexusData::object(crate::sui::types::Address::from_static("0x42"));
        let data = NexusData::inline_data(b"payload").unwrap();
        let many =
            NexusData::object_many([crate::sui::types::Address::from_static("0x1")]).unwrap();

        assert!(MetaSchema::conforms_port(&schema, &object));
        assert!(!MetaSchema::conforms_port(&schema, &data));
        assert!(!MetaSchema::conforms_port(&schema, &many));
    }

    #[test]
    fn resolved_vectors_use_schema_cardinality_and_reject_unresolved_or_oversized_values() {
        let schema = MetaSchema::new(
            vec![PortSchema::new(b"items".to_vec(), true, ValueKind::Data)],
            vec![OutputVariantSchema::new(b"ok".to_vec(), vec![])],
        );
        let value = NexusValue::inline_data(br#""one""#).unwrap();
        let inputs = HashMap::from([("items".to_string(), vec![value.clone()])]);

        let encoded = schema.resolved_inputs_to_json(&inputs).unwrap();
        assert!(encoded["ports"][0]["value"].get("many").is_some());
        assert_eq!(schema.resolved_inputs_from_json(&encoded).unwrap(), inputs);
        assert!(schema
            .resolved_inputs_from_json(&json!({
                "ports": [{
                    "port_name": "items",
                    "value": { "one": { "kind": "data", "data": "one" } }
                }]
            }))
            .is_err());
        let oversized = vec![value; 257];
        assert!(!MetaSchema::conforms_resolved_port(
            &schema.input_ports[0],
            &oversized,
        ));
        assert!(!MetaSchema::conforms_resolved_port(
            &schema.input_ports[0],
            &[
                NexusValue::walrus_data(
                    b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    vec![0; 32],
                )
                .unwrap()
            ],
        ));
    }

    #[test]
    fn canonical_input_values_follow_schema_order_and_require_complete_input() {
        let schema = MetaSchema::new(
            vec![
                PortSchema::new(b"second".to_vec(), false, ValueKind::Data),
                PortSchema::new(b"first".to_vec(), false, ValueKind::Data),
            ],
            vec![OutputVariantSchema::new(b"ok".to_vec(), vec![])],
        );
        let first = NexusData::inline_data(b"one").unwrap();
        let second = NexusData::inline_data(b"two").unwrap();
        let inputs = HashMap::from([
            ("first".to_string(), first.clone()),
            ("second".to_string(), second.clone()),
        ]);

        assert_eq!(
            schema.canonical_input_values(&inputs).unwrap(),
            vec![second, first]
        );
        assert!(schema
            .canonical_input_values(&HashMap::from([(
                "first".to_string(),
                NexusData::inline_data(b"one").unwrap(),
            )]))
            .is_err());
    }

    #[test]
    fn invalid_offchain_object_input_is_rejected() {
        let schema = MetaSchema::new(
            vec![PortSchema::new(
                b"object".to_vec(),
                false,
                ValueKind::Object,
            )],
            vec![OutputVariantSchema::new(b"Ok".to_vec(), vec![])],
        );

        assert!(schema.validate_for_tool(true).is_err());
    }

    #[test]
    fn onchain_conversion_classifies_vector_objects_as_many_object_values() {
        let schema = MetaSchema::from_onchain_json_schemas(
            r#"{"objects":{"type":"vector","element_type":{"type":"object"}},"count":{"type":"u64"}}"#,
            r#"{"Ok":{"fields":{"ids":{"type":"vector","element_type":{"type":"object_id"}}}}}"#,
        )
        .expect("on-chain schema should convert");

        assert!(schema.input_ports[0].is_many);
        assert_eq!(schema.input_ports[0].value_kind, ValueKind::Object);
        assert_eq!(schema.input_ports[1].value_kind, ValueKind::Data);
        assert_eq!(
            schema.output_variants[0].ports[0].value_kind,
            ValueKind::Object
        );
    }

    #[test]
    fn onchain_conversion_recurses_nested_vectors_to_the_innermost_value_kind() {
        let schema = MetaSchema::from_onchain_json_schemas(
            r#"{"objects":{"type":"vector","element_type":{"type":"vector","element_type":{"type":"object"}}}}"#,
            r#"{"Ok":{"fields":{"bytes":{"type":"vector","element_type":{"type":"vector","element_type":{"type":"u8"}}}}}}"#,
        )
        .expect("nested vectors retain outer cardinality and inner semantic kind");

        assert!(schema.input_ports[0].is_many);
        assert_eq!(schema.input_ports[0].value_kind, ValueKind::Object);
        assert!(schema.output_variants[0].ports[0].is_many);
        assert_eq!(
            schema.output_variants[0].ports[0].value_kind,
            ValueKind::Data
        );
    }

    #[test]
    fn typed_safe_failure_requires_inline_data_reason() {
        let failure = canonical_schema_mismatch_failure();
        let malformed = OffchainToolOutput {
            tag: FAILURE_VARIANT.to_vec(),
            ports: vec![OffchainToolOutputPort {
                port_name: FAILURE_PORT.to_vec(),
                values: NexusData::object(crate::sui::types::Address::from_static("0x1"))
                    .into_values()
                    .expect("fixture should decode"),
            }],
        };

        assert!(MetaSchema::conforms_safe_failure(&failure));
        assert!(!MetaSchema::conforms_safe_failure(&malformed));
    }

    #[test]
    fn typed_safe_failure_requires_exact_reason_name_and_shape() {
        let reason = NexusData::inline_data(b"reason").unwrap();
        let renamed = OffchainToolOutput {
            tag: FAILURE_VARIANT.to_vec(),
            ports: vec![output_port(b"message", reason.clone())],
        };
        let missing = OffchainToolOutput {
            tag: FAILURE_VARIANT.to_vec(),
            ports: Vec::new(),
        };
        let extra = OffchainToolOutput {
            tag: FAILURE_VARIANT.to_vec(),
            ports: vec![
                output_port(FAILURE_PORT, reason.clone()),
                output_port(b"extra", reason.clone()),
            ],
        };
        let many = OffchainToolOutput {
            tag: FAILURE_VARIANT.to_vec(),
            ports: vec![output_port(
                FAILURE_PORT,
                NexusData::inline_data_many([b"one".as_slice(), b"two".as_slice()]).unwrap(),
            )],
        };
        let object = OffchainToolOutput {
            tag: FAILURE_VARIANT.to_vec(),
            ports: vec![output_port(
                FAILURE_PORT,
                NexusData::object(crate::sui::types::Address::from_static("0x1")),
            )],
        };

        for malformed in [renamed, missing, extra, many, object] {
            assert!(!MetaSchema::conforms_safe_failure(&malformed));
            assert!(offchain_schema()
                .canonical_output_ports(&malformed)
                .is_err());
        }
    }

    #[test]
    fn canonical_failure_projection_restores_reason_port() {
        let schema = offchain_schema();
        let failure = canonical_schema_mismatch_failure();

        let ports = schema.canonical_output_ports(&failure).unwrap();

        assert_eq!(ports[0].0, FAILURE_PORT);
        assert!(ports[0].1.is_one());
    }

    #[test]
    fn typed_safe_failure_rejects_oversized_raw_inline_reason() {
        let oversized = OffchainToolOutput {
            tag: FAILURE_VARIANT.to_vec(),
            ports: vec![OffchainToolOutputPort {
                port_name: FAILURE_PORT.to_vec(),
                values: vec![
                    crate::move_bindings::primitives::data::NexusValue::InlineData {
                        bytes: vec![0; 61_441],
                    },
                ],
            }],
        };

        assert!(!MetaSchema::conforms_safe_failure(&oversized));
    }

    #[test]
    fn committed_binding_ir_uses_one_boolean_port_shape_bit() {
        let ir: Value = serde_json::from_str(include_str!("../ir/interface.json")).unwrap();
        let module = &ir["modules"]["meta_schema"];
        let datatypes = module["datatypes"].as_array().unwrap();
        let functions = module["functions"].as_array().unwrap();
        let port_schema = datatypes
            .iter()
            .find(|datatype| datatype["name"] == "PortSchema")
            .unwrap();
        let fields = port_schema["kind"]["Struct"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| field["name"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(fields, ["port_name", "is_many", "value_kind"]);
        assert!(datatypes
            .iter()
            .all(|datatype| datatype["name"] != "Cardinality"));
        assert!(functions.iter().all(|function| !matches!(
            function["name"].as_str(),
            Some("cardinality_one" | "cardinality_many" | "port_cardinality")
        )));
    }
}
