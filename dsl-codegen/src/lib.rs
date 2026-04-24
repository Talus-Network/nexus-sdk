//! Shared codegen that emits Rust and TypeScript DSL tool descriptors from
//! `tool-meta.json` artifacts (produced by `nexus-toolkit`'s `--meta`
//! flag).
//!
//! One source of truth — the tool's JSON Schema metadata — drives both
//! language outputs. The emitted Rust descriptors plug into
//! `nexus-dag-dsl`'s [`TypedDagBuilder`] via [`ToolDescriptor`]; the
//! emitted TypeScript descriptors plug into the matching `ts-dsl`
//! package.
//!
//! Supported JSON Schema subset (sufficient for the common tool meta
//! shapes):
//! - `type: string` → Rust `String`, TS `string`.
//! - `type: integer` → Rust `i64`, TS `number`.
//! - `type: number` → Rust `f64`, TS `number`.
//! - `type: boolean` → Rust `bool`, TS `boolean`.
//! - `type: array` with primitive `items` → `Vec<T>` / `T[]`.
//! - Anything more complex (nested objects, `oneOf`, refs) → fallback to
//!   `serde_json::Value` / `unknown`; flagged to stderr so the user can
//!   decide whether to hand-tune.
//!
//! Input-schema shape is expected to be an `object` with a `properties`
//! map — each property becomes an input port.
//!
//! Output-schema shape is expected to be either a single `object` with
//! `properties` (one variant `ok` with those properties) or a `oneOf`
//! list where each alternative is an `object` with a single property
//! `<variant_name>` whose value is itself an object with per-port
//! properties (matching how `nexus-toolkit` encodes enum outputs). See
//! module docs in [`output_variants`] for the parse rule.
//!
//! [`TypedDagBuilder`]: https://docs.rs/nexus-dag-dsl
//! [`ToolDescriptor`]: https://docs.rs/nexus-dag-dsl

use {
    anyhow::{anyhow, Context, Result},
    nexus_sdk::types::ToolMeta,
    serde_json::Value,
    std::{fs, path::Path},
};

// ---------------------------------------------------------------------------
// Intermediate representation
// ---------------------------------------------------------------------------

/// Parsed descriptor shape — the shared intermediate representation used
/// by both the Rust and TypeScript emitters.
#[derive(Debug, Clone)]
pub struct Descriptor {
    /// Canonical FQN — e.g. `xyz.taluslabs.math.i64.add@1`.
    pub fqn: String,
    /// Tool name suitable for use as a Rust struct identifier (PascalCase
    /// derived from the FQN's name segment).
    pub type_name: String,
    /// Input ports, in declared order.
    pub inputs: Vec<Port>,
    /// Output variants, each with its own port list.
    pub variants: Vec<OutputVariant>,
}

/// A single input or output port.
#[derive(Debug, Clone)]
pub struct Port {
    /// Port name as it appears on the wire.
    pub name: String,
    /// Mapped Rust type (e.g. `i64`, `String`, `Vec<i64>`).
    pub rust_ty: String,
    /// Mapped TS type (e.g. `number`, `string`, `number[]`).
    pub ts_ty: String,
}

/// One output variant.
#[derive(Debug, Clone)]
pub struct OutputVariant {
    /// Variant name (e.g. `ok`, `err`, `lt`, `gt`).
    pub name: String,
    /// Ports declared under this variant.
    pub ports: Vec<Port>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Read and parse a `tool-meta.json` file from disk, returning the
/// intermediate [`Descriptor`].
pub fn load_descriptor(path: &Path) -> Result<Descriptor> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading tool-meta file {}", path.display()))?;
    let meta: ToolMeta = serde_json::from_str(&content)
        .with_context(|| format!("parsing tool-meta JSON in {}", path.display()))?;
    Descriptor::from_meta(&meta)
}

impl Descriptor {
    /// Parse a descriptor from a [`ToolMeta`] value.
    pub fn from_meta(meta: &ToolMeta) -> Result<Self> {
        let fqn_str = meta.fqn.to_string();
        let type_name = fqn_to_type_name(&fqn_str);
        let inputs = parse_object_ports(&meta.input_schema, "input")?;
        let variants = output_variants::parse(&meta.output_schema)?;
        Ok(Descriptor {
            fqn: fqn_str,
            type_name,
            inputs,
            variants,
        })
    }
}

fn parse_object_ports(schema: &Value, context: &'static str) -> Result<Vec<Port>> {
    let props = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("{context} schema must be an object with `properties`"))?;

    let mut ports = Vec::new();
    for (name, ty_schema) in props {
        let (rust_ty, ts_ty) = map_scalar_or_array(ty_schema);
        ports.push(Port {
            name: name.clone(),
            rust_ty,
            ts_ty,
        });
    }
    Ok(ports)
}

/// Output variant parsing.
///
/// Two accepted shapes:
///
/// 1. Single variant: the output schema is itself an `object` with
///    `properties`; we synthesize one variant named `ok` with those
///    properties.
///
/// 2. Multi-variant: the output schema is `{ "oneOf": [ ... ] }` where
///    each alternative is an object with exactly one property whose key
///    is the variant name and whose value is an `object` with the
///    per-variant port properties. This matches how `nexus-toolkit`
///    encodes Rust enums with named struct variants via schemars.
pub mod output_variants {
    use super::*;

    /// Parse the output schema into a list of variants.
    pub fn parse(schema: &Value) -> Result<Vec<OutputVariant>> {
        if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
            let mut variants = Vec::with_capacity(one_of.len());
            for alt in one_of {
                variants.push(parse_variant_from_oneof_alt(alt)?);
            }
            Ok(variants)
        } else if schema.get("properties").is_some() {
            Ok(vec![OutputVariant {
                name: "ok".to_string(),
                ports: parse_object_ports(schema, "output")?,
            }])
        } else {
            Err(anyhow!(
                "output schema must be either an object with `properties` or \
                 a `oneOf` list of variant alternatives"
            ))
        }
    }

    fn parse_variant_from_oneof_alt(alt: &Value) -> Result<OutputVariant> {
        let properties = alt
            .get("properties")
            .and_then(|v| v.as_object())
            .ok_or_else(|| {
                anyhow!("each `oneOf` alternative must be an object with `properties`")
            })?;
        let (variant_name, variant_payload) = properties.iter().next().ok_or_else(|| {
            anyhow!("`oneOf` alternative must declare exactly one variant property")
        })?;
        let ports = parse_object_ports(variant_payload, "output variant")?;
        Ok(OutputVariant {
            name: variant_name.clone(),
            ports,
        })
    }
}

// Primitive JSON Schema → Rust / TS type mapping.
//
// Unhandled shapes fall back to `serde_json::Value` / `unknown` and emit
// a warning to stderr so the user can decide whether to hand-tune.
fn map_scalar_or_array(schema: &Value) -> (String, String) {
    let ty = schema.get("type").and_then(|v| v.as_str());
    match ty {
        Some("string") => ("String".into(), "string".into()),
        Some("integer") => ("i64".into(), "number".into()),
        Some("number") => ("f64".into(), "number".into()),
        Some("boolean") => ("bool".into(), "boolean".into()),
        Some("array") => {
            let item_schema = schema.get("items").cloned().unwrap_or(Value::Null);
            let (r, t) = map_scalar_or_array(&item_schema);
            (format!("Vec<{r}>"), format!("{t}[]"))
        }
        _ => {
            eprintln!(
                "nexus-dsl-codegen: falling back to `serde_json::Value` for schema: {schema}"
            );
            ("serde_json::Value".into(), "unknown".into())
        }
    }
}

fn fqn_to_type_name(fqn: &str) -> String {
    // `xyz.taluslabs.math.i64.add@1` → name segment is `add` → PascalCase
    // `Add`. Full grammar per sdk/src/tool_fqn.rs: `domain.name@version`
    // where `domain` has ≥2 dotted segments.
    let before_at = fqn.split('@').next().unwrap_or(fqn);
    let name_segment = before_at.rsplit('.').next().unwrap_or(before_at);
    to_pascal_case(name_segment)
}

fn to_pascal_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            out.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Rust emission
// ---------------------------------------------------------------------------

/// Emit a Rust `ToolDescriptor` implementation for the given descriptor.
///
/// The generated code uses `nexus_dag_dsl::tool_descriptor!` so it stays
/// in sync with whatever the declarative macro's current expansion is.
pub fn emit_rust(desc: &Descriptor) -> String {
    let mut buf = String::new();
    buf.push_str("// AUTO-GENERATED by nexus-dsl-codegen. Do not edit by hand.\n");
    buf.push_str("// Source: tool-meta.json\n");
    buf.push('\n');
    buf.push_str("nexus_dag_dsl::tool_descriptor! {\n");
    buf.push_str(&format!("    pub struct {};\n", desc.type_name));
    buf.push_str(&format!("    fqn = \"{}\";\n", desc.fqn));
    buf.push_str("    inputs {\n");
    for p in &desc.inputs {
        buf.push_str(&format!("        {}: {},\n", p.name, p.rust_ty));
    }
    buf.push_str("    }\n");
    buf.push_str("    outputs {\n");
    for v in &desc.variants {
        buf.push_str(&format!("        {} {{\n", v.name));
        for p in &v.ports {
            buf.push_str(&format!("            {}: {},\n", p.name, p.rust_ty));
        }
        buf.push_str("        }\n");
    }
    buf.push_str("    }\n");
    buf.push_str("}\n");
    buf
}

// ---------------------------------------------------------------------------
// TypeScript emission
// ---------------------------------------------------------------------------

/// Emit a TypeScript descriptor module for the given descriptor.
///
/// The output is a self-contained TypeScript module that imports from
/// the `ts-dsl` package and exports a descriptor object. Assumes the
/// consumer has `@nexus/dag-dsl` (or equivalent name) resolvable via
/// their module resolution.
pub fn emit_ts(desc: &Descriptor) -> String {
    let mut buf = String::new();
    buf.push_str("// AUTO-GENERATED by nexus-dsl-codegen. Do not edit by hand.\n");
    buf.push_str("// Source: tool-meta.json\n");
    buf.push('\n');
    buf.push_str("import { defineTool } from \"@nexus/dag-dsl\";\n");
    buf.push('\n');
    buf.push_str(&format!(
        "export const {} = defineTool({{\n",
        desc.type_name
    ));
    buf.push_str(&format!("  fqn: \"{}\",\n", desc.fqn));
    buf.push_str("  inputs: {\n");
    for p in &desc.inputs {
        buf.push_str(&format!(
            "    {}: \"{}\" as unknown as {},\n",
            p.name, p.name, p.ts_ty
        ));
    }
    buf.push_str("  },\n");
    buf.push_str("  outputs: {\n");
    for v in &desc.variants {
        buf.push_str(&format!("    {}: {{\n", v.name));
        for p in &v.ports {
            buf.push_str(&format!(
                "      {}: \"{}\" as unknown as {},\n",
                p.name, p.name, p.ts_ty
            ));
        }
        buf.push_str("    },\n");
    }
    buf.push_str("  },\n");
    buf.push_str("});\n");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> ToolMeta {
        use {nexus_sdk::fqn, serde_json::json};
        ToolMeta {
            fqn: fqn!("xyz.taluslabs.math.i64.add@1"),
            url: "http://example.com".into(),
            description: "Addition".into(),
            timeout: std::time::Duration::from_millis(5000),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "a": { "type": "integer" },
                    "b": { "type": "integer" }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "result": { "type": "integer" }
                }
            }),
        }
    }

    /// Descriptor parsing maps input / output JSON Schema properties to
    /// ports with the expected Rust / TS types. Single-variant outputs
    /// get the synthesized variant name `ok`.
    ///
    /// What breaks if this test is deleted: future refactors of
    /// `ToolMeta` or the schema-to-IR parser could silently drop ports
    /// or mis-type them, producing generated descriptors that look
    /// right but fail to compile or mismatch the wire format.
    #[test]
    fn parses_simple_meta_into_descriptor() {
        let desc = Descriptor::from_meta(&sample_meta()).expect("parses");
        assert_eq!(desc.type_name, "Add");
        assert_eq!(desc.inputs.len(), 2);
        assert_eq!(desc.inputs[0].rust_ty, "i64");
        assert_eq!(desc.inputs[0].ts_ty, "number");
        assert_eq!(desc.variants.len(), 1);
        assert_eq!(desc.variants[0].name, "ok");
        assert_eq!(desc.variants[0].ports.len(), 1);
        assert_eq!(desc.variants[0].ports[0].name, "result");
    }

    /// Rust emission produces a `tool_descriptor!` invocation that
    /// contains the tool's FQN, one `inputs` entry per input port, and
    /// one `outputs` entry per variant with its ports.
    ///
    /// What breaks if this test is deleted: regressions in the Rust
    /// emitter — wrong template, missing ports, wrong variant names —
    /// would reach users as broken generated code without a localized
    /// failure in codegen itself.
    #[test]
    fn rust_emission_contains_expected_tokens() {
        let desc = Descriptor::from_meta(&sample_meta()).unwrap();
        let emitted = emit_rust(&desc);
        assert!(emitted.contains("pub struct Add;"));
        assert!(emitted.contains("fqn = \"xyz.taluslabs.math.i64.add@1\""));
        assert!(emitted.contains("a: i64"));
        assert!(emitted.contains("b: i64"));
        assert!(emitted.contains("ok {"));
        assert!(emitted.contains("result: i64"));
    }

    /// TypeScript emission produces an ESM module that imports
    /// `defineTool` from the ts-dsl package, exports a PascalCased
    /// descriptor, and lists inputs/outputs with TS-mapped types.
    ///
    /// What breaks if this test is deleted: regressions in the TS
    /// emitter could let broken TS reach users; the round-trip CI test
    /// would catch some of those, but locally-detectable malformations
    /// (syntax, missing fields) deserve their own guard.
    #[test]
    fn ts_emission_contains_expected_tokens() {
        let desc = Descriptor::from_meta(&sample_meta()).unwrap();
        let emitted = emit_ts(&desc);
        assert!(emitted.contains("import { defineTool } from \"@nexus/dag-dsl\""));
        assert!(emitted.contains("export const Add = defineTool"));
        assert!(emitted.contains("a:"));
        assert!(emitted.contains("b:"));
        assert!(emitted.contains("ok:"));
        assert!(emitted.contains("result:"));
    }
}
