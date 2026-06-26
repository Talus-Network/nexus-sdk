//! Regenerates the committed Move-binding IR.
//!
//! This is the *network* half of the binding pipeline (see `sdk/src/idents/`):
//! it fetches normalized Move package metadata from a running Sui gRPC endpoint
//! and persists it as committed JSON. The *offline* half (rendering address-free
//! identifier constants from that JSON) happens deterministically in `build.rs`,
//! so normal builds never touch the network.
//!
//! Run it deliberately against a node that exposes the target packages (a
//! localnet with the Nexus packages published, plus the `0x1`/`0x2` framework
//! packages every node carries). `just sdk rebind` wraps the whole
//! pipeline:
//!
//! ```bash
//! NEXUS_BINDING_GRPC_URL=http://127.0.0.1:9000 \
//! NEXUS_BINDING_PACKAGES="primitives=0x..,interface=0x..,workflow=0x..,move_std=0x1,sui_framework=0x2" \
//!   cargo run -p nexus-sdk --features binding_codegen --bin generate_binding
//! ```
//!
//! Each `name=0xid` pair becomes `sdk/src/idents/generated/ir/<name>.json`.
//!
//! # Step-by-step
//!
//! 1. Select a `sui` toolchain matching `nexus-next` (otherwise the Move build
//!    fails, e.g. `Unbound function 'exists' in module 'sui::dynamic_field'`):
//!
//!    ```bash
//!    suiup install sui@testnet-1.73.1 && suiup default set sui@testnet-1.73.1
//!    ```
//!
//! 2. Start a localnet and fund the active address (leave it running):
//!
//!    ```bash
//!    sui start --with-faucet --force-regenesis
//!    sui client switch --env localnet && sui client faucet
//!    ```
//!
//! 3. Publish the Nexus packages; this writes their ids to
//!    `nexus-next/sui/bin/target/objects.localnet.toml`:
//!
//!    ```bash
//!    (cd ../nexus-next/sui && NEXUS_PUBLISH_OVERWRITE=1 SUI_ENV=localnet ./bin/publish.sh publish)
//!    ```
//!
//! 4. Fetch the IR (the `just` recipe reads the package ids from the objects
//!    TOML and adds the `0x1`/`0x2` framework packages automatically):
//!
//!    ```bash
//!    just sdk rebind
//!    ```
//!
//! 5. Rebuild so `build.rs` re-renders the constants, then review the diff under
//!    `sdk/src/idents/generated/ir/` and fix any call sites the compiler flags:
//!
//!    ```bash
//!    cargo +stable check --all-features -p nexus-sdk
//!    ```
//!
//! 6. Stop the localnet (Ctrl-C the `sui start` shell).

use {
    nexus_sdk::sui::{grpc::Client, types::Address},
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
        process::ExitCode,
        str::FromStr,
    },
    sui_move_codegen::fetch_package,
};

/// Directory (relative to the crate manifest) that holds the committed IR.
const IR_DIR: &str = "src/idents/generated/ir";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let grpc_url = std::env::var("NEXUS_BINDING_GRPC_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());

    let packages = std::env::var("NEXUS_BINDING_PACKAGES").map_err(|_| {
        "set NEXUS_BINDING_PACKAGES to a comma-separated list of `name=0xpackageid` pairs \
         (e.g. \"primitives=0x..,move_std=0x1,sui_framework=0x2\")"
            .to_string()
    })?;
    let packages = parse_packages(&packages)?;
    let normalize_package_ids = normalize_package_ids_enabled()?;
    let mut replacements = if normalize_package_ids {
        package_id_replacements(&packages)
    } else {
        Vec::new()
    };
    let nexus_sui_dir = std::env::var_os("NEXUS_BINDING_NEXUS_SUI_DIR").map(PathBuf::from);

    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(IR_DIR);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    let mut client =
        Client::new(&grpc_url).map_err(|e| format!("gRPC client for {grpc_url}: {e}"))?;

    let mut fetched_packages = Vec::with_capacity(packages.len());
    for (name, package_id) in packages {
        let package = fetch_package(&mut client, package_id)
            .await
            .map_err(|e| format!("fetch `{name}` ({package_id}): {e}"))?;
        let module_count = package.modules.len();
        let json = package
            .to_json_string()
            .map_err(|e| format!("serialize IR for `{name}`: {e}"))?;

        if normalize_package_ids && !is_framework_package(&name, package_id) {
            add_package_ir_id_replacements(&mut replacements, &name, &json);
        }

        fetched_packages.push((name, module_count, json));
    }

    for (name, module_count, mut json) in fetched_packages {
        normalize_json_package_ids(&mut json, &replacements);
        restore_source_parameter_names(&mut json, &name, nexus_sui_dir.as_deref())?;
        if normalize_package_ids {
            validate_normalized_package_ids(&name, &json, &replacements)?;
        }

        let path = out_dir.join(format!("{name}.json"));
        std::fs::write(&path, format!("{json}\n"))
            .map_err(|e| format!("write {}: {e}", path.display()))?;

        println!("wrote {} ({} modules)", path.display(), module_count);
    }

    Ok(())
}

/// Parses a `NEXUS_BINDING_PACKAGES` spec into `(name, package_id)` pairs.
///
/// The spec is a comma-separated list of `name=0xpackageid` entries. Surrounding
/// whitespace around entries, names, and ids is ignored, and empty entries (e.g.
/// a trailing comma) are skipped. Returns an error string on a missing `=` or an
/// unparsable address.
fn parse_packages(spec: &str) -> Result<Vec<(String, Address)>, String> {
    spec.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (name, id) = entry.split_once('=').ok_or_else(|| {
                format!("malformed package entry `{entry}`, expected `name=0xid`")
            })?;

            let name = name.trim();
            if name.is_empty() {
                return Err(format!("empty package name in entry `{entry}`"));
            }

            let id = id.trim();
            let address =
                Address::from_str(id).map_err(|e| format!("invalid package id `{id}`: {e}"))?;

            Ok((name.to_string(), address))
        })
        .collect()
}

fn normalize_package_ids_enabled() -> Result<bool, String> {
    match std::env::var("NEXUS_BINDING_NORMALIZE_PACKAGE_IDS") {
        Ok(value) => parse_bool_env("NEXUS_BINDING_NORMALIZE_PACKAGE_IDS", &value),
        Err(std::env::VarError::NotPresent) => Ok(true),
        Err(err) => Err(format!(
            "read NEXUS_BINDING_NORMALIZE_PACKAGE_IDS environment variable: {err}"
        )),
    }
}

fn parse_bool_env(name: &str, value: &str) -> Result<bool, String> {
    match value {
        "1" | "true" | "TRUE" | "yes" | "YES" => Ok(true),
        "0" | "false" | "FALSE" | "no" | "NO" | "" => Ok(false),
        _ => Err(format!(
            "{name} must be 0/1, true/false, or yes/no, got `{value}`"
        )),
    }
}

fn package_id_replacements(packages: &[(String, Address)]) -> Vec<(Address, Address)> {
    packages
        .iter()
        .enumerate()
        .filter_map(|(index, (name, address))| {
            if is_framework_package(name, *address) {
                None
            } else {
                Some((*address, placeholder_address_for(name, index)))
            }
        })
        .collect()
}

fn add_package_ir_id_replacements(
    replacements: &mut Vec<(Address, Address)>,
    name: &str,
    json: &str,
) {
    let placeholder = placeholder_address_for(name, 0);
    for field in ["storage_id", "original_id"] {
        let Some(address) = extract_json_string_fields(json, field)
            .into_iter()
            .next()
            .and_then(|value| Address::from_str(&value).ok())
        else {
            continue;
        };
        add_replacement_if_missing(replacements, address, placeholder);
    }
}

fn extract_json_string_fields(json: &str, field: &str) -> Vec<String> {
    let pattern = format!("\"{field}\": \"");
    let mut values = Vec::new();
    let mut offset = 0;

    while let Some(relative_start) = json[offset..].find(&pattern) {
        let start = offset + relative_start + pattern.len();
        let Some(relative_end) = json[start..].find('"') else {
            break;
        };
        let end = start + relative_end;
        values.push(json[start..end].to_string());
        offset = end + 1;
    }

    values
}

fn add_replacement_if_missing(
    replacements: &mut Vec<(Address, Address)>,
    actual: Address,
    placeholder: Address,
) {
    if actual == placeholder || replacements.iter().any(|(existing, _)| *existing == actual) {
        return;
    }
    replacements.push((actual, placeholder));
}

fn is_framework_package(name: &str, address: Address) -> bool {
    name == "move_std"
        || name == "sui_framework"
        || address == Address::from_str("0x1").expect("0x1 is valid")
        || address == Address::from_str("0x2").expect("0x2 is valid")
}

fn placeholder_address_for(name: &str, index: usize) -> Address {
    let value = match name {
        "primitives" => 0xa1,
        "interface" => 0xa2,
        "registry" => 0xa3,
        "workflow" => 0xa4,
        "scheduler" => 0xa5,
        _ => 0xa000 + index as u64,
    };
    Address::from_str(&format!("0x{value:x}")).expect("placeholder address is valid")
}

fn normalize_json_package_ids(json: &mut String, replacements: &[(Address, Address)]) {
    for (actual, placeholder) in replacements {
        let placeholder = compact_address_string(*placeholder);
        for actual in address_string_variants(*actual) {
            *json = json.replace(&actual, &placeholder);
        }
    }
}

fn address_string_variants(address: Address) -> [String; 2] {
    let padded = address.to_string();
    let compact = compact_address_string(address);
    [padded, compact]
}

fn compact_address_string(address: Address) -> String {
    let padded = address.to_string();
    let digits = padded
        .strip_prefix("0x")
        .expect("Address string has 0x prefix")
        .trim_start_matches('0');
    if digits.is_empty() {
        "0x0".to_string()
    } else {
        format!("0x{digits}")
    }
}

fn validate_normalized_package_ids(
    name: &str,
    json: &str,
    replacements: &[(Address, Address)],
) -> Result<(), String> {
    let mut allowed = vec!["0x1".to_string(), "0x2".to_string()];
    for (_, placeholder) in replacements {
        let placeholder = compact_address_string(*placeholder);
        if !allowed.contains(&placeholder) {
            allowed.push(placeholder);
        }
    }

    let mut unexpected = Vec::new();
    for field in ["storage_id", "original_id", "address"] {
        for value in extract_json_string_fields(json, field) {
            let Ok(address) = Address::from_str(&value) else {
                continue;
            };
            let compact = compact_address_string(address);
            if !allowed.contains(&compact) && !unexpected.contains(&compact) {
                unexpected.push(compact);
            }
        }
    }

    if unexpected.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "`{name}` IR still contains non-placeholder package address(es): {}",
            unexpected.join(", ")
        ))
    }
}

fn restore_source_parameter_names(
    json: &mut String,
    package_name: &str,
    nexus_sui_dir: Option<&Path>,
) -> Result<(), String> {
    let Some(nexus_sui_dir) = nexus_sui_dir else {
        return Ok(());
    };
    if package_name == "move_std" || package_name == "sui_framework" {
        return Ok(());
    }

    let signatures = source_parameter_names_for_package(nexus_sui_dir, package_name)?;
    if signatures.is_empty() {
        return Ok(());
    }

    let mut value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| format!("parse IR JSON for `{package_name}` before name restore: {e}"))?;
    let modules = value
        .get_mut("modules")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| format!("IR JSON for `{package_name}` does not contain modules object"))?;

    for (module_name, module) in modules {
        let Some(functions) = module
            .get_mut("functions")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };

        for function in functions {
            let Some(function_name) = function
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
            else {
                continue;
            };
            let Some(source_names) = signatures.get(&(module_name.to_owned(), function_name))
            else {
                continue;
            };
            let Some(parameters) = function
                .get_mut("parameters")
                .and_then(serde_json::Value::as_array_mut)
            else {
                continue;
            };
            if parameters.len() != source_names.len() {
                return Err(format!(
                    "`{package_name}::{module_name}` function parameter count mismatch for source names: IR has {}, source has {}",
                    parameters.len(),
                    source_names.len()
                ));
            }

            for (index, (parameter, source_name)) in
                parameters.iter_mut().zip(source_names.iter()).enumerate()
            {
                let current_name = parameter.get("name").and_then(serde_json::Value::as_str);
                if current_name == Some(&format!("arg{index}")) {
                    let parameter = parameter.as_object_mut().ok_or_else(|| {
                        format!("IR parameter for `{package_name}::{module_name}` is not an object")
                    })?;
                    parameter.insert(
                        "name".to_string(),
                        serde_json::Value::String(source_name.to_string()),
                    );
                }
            }
        }
    }

    *json = serde_json::to_string_pretty(&value)
        .map_err(|e| format!("serialize IR JSON for `{package_name}` after name restore: {e}"))?;
    Ok(())
}

fn source_parameter_names_for_package(
    nexus_sui_dir: &Path,
    package_name: &str,
) -> Result<HashMap<(String, String), Vec<String>>, String> {
    let source_dir = nexus_sui_dir.join(package_name).join("sources");
    if !source_dir.is_dir() {
        return Ok(HashMap::new());
    }

    let mut names = HashMap::new();
    let entries = std::fs::read_dir(&source_dir)
        .map_err(|e| format!("read Move source directory {}: {e}", source_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read entry under {}: {e}", source_dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("move") {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("read Move source {}: {e}", path.display()))?;
        let source = strip_move_comments(&source);
        let module_name = parse_move_module_name(&source).ok_or_else(|| {
            format!(
                "Move source {} does not declare a `module package::name;` header",
                path.display()
            )
        })?;

        for (function_name, parameter_names) in parse_move_function_parameter_names(&source) {
            names.insert((module_name.clone(), function_name), parameter_names);
        }
    }

    Ok(names)
}

fn strip_move_comments(source: &str) -> String {
    let mut stripped = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            for next in chars.by_ref() {
                if next == '\n' {
                    stripped.push('\n');
                    break;
                }
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            continue;
        }

        stripped.push(ch);
    }

    stripped
}

fn parse_move_module_name(source: &str) -> Option<String> {
    let marker = "module ";
    let start = source.find(marker)? + marker.len();
    let rest = &source[start..];
    let module_start = rest.find("::")? + 2;
    let module = rest[module_start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if module.is_empty() {
        None
    } else {
        Some(module)
    }
}

fn parse_move_function_parameter_names(source: &str) -> Vec<(String, Vec<String>)> {
    let mut functions = Vec::new();
    let bytes = source.as_bytes();
    let mut offset = 0;

    while let Some(relative_fun) = source[offset..].find("fun") {
        let fun_start = offset + relative_fun;
        if !is_keyword_at(bytes, fun_start, b"fun") {
            offset = fun_start + 3;
            continue;
        }

        let mut cursor = fun_start + 3;
        cursor = skip_ascii_whitespace(source, cursor);
        let name_start = cursor;
        while cursor < source.len() && is_identifier_byte(source.as_bytes()[cursor]) {
            cursor += 1;
        }
        if cursor == name_start {
            offset = fun_start + 3;
            continue;
        }
        let function_name = source[name_start..cursor].to_string();
        cursor = skip_ascii_whitespace(source, cursor);
        if source[cursor..].starts_with('<') {
            let Some(after_type_parameters) = skip_balanced(source, cursor, '<', '>') else {
                offset = cursor;
                continue;
            };
            cursor = skip_ascii_whitespace(source, after_type_parameters);
        }
        if !source[cursor..].starts_with('(') {
            offset = cursor;
            continue;
        }
        let Some(params_end) = skip_balanced(source, cursor, '(', ')') else {
            offset = cursor + 1;
            continue;
        };
        let params = &source[cursor + 1..params_end - 1];
        functions.push((function_name, parse_move_parameter_names(params)));
        offset = params_end;
    }

    functions
}

fn parse_move_parameter_names(params: &str) -> Vec<String> {
    split_top_level_commas(params)
        .into_iter()
        .filter_map(|parameter| {
            let parameter = parameter.trim();
            if parameter.is_empty() {
                return None;
            }
            let colon = parameter.find(':')?;
            let name = parameter[..colon].trim();
            let name = name.strip_prefix("mut ").unwrap_or(name).trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

fn split_top_level_commas(input: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0u64;
    let mut paren_depth = 0u64;

    for (index, ch) in input.char_indices() {
        match ch {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            ',' if angle_depth == 0 && paren_depth == 0 => {
                segments.push(&input[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    segments.push(&input[start..]);
    segments
}

fn skip_ascii_whitespace(source: &str, mut cursor: usize) -> usize {
    while cursor < source.len() && source.as_bytes()[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    cursor
}

fn skip_balanced(source: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0u64;
    for (relative, ch) in source[start..].char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(start + relative + ch.len_utf8());
            }
        }
    }
    None
}

fn is_keyword_at(bytes: &[u8], index: usize, keyword: &[u8]) -> bool {
    bytes[index..].starts_with(keyword)
        && index
            .checked_sub(1)
            .map(|prev| !is_identifier_byte(bytes[prev]))
            .unwrap_or(true)
        && bytes
            .get(index + keyword.len())
            .map(|next| !is_identifier_byte(*next))
            .unwrap_or(true)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn parses_named_pairs_and_trims_whitespace() {
        let parsed = parse_packages(
            "  primitives = 0x1 , sui_framework=0x2 ,workflow=0x00000000000000000000000000000000000000000000000000000000000000aa",
        )
        .expect("valid spec");

        assert_eq!(
            parsed,
            vec![
                ("primitives".to_string(), Address::from_str("0x1").unwrap()),
                (
                    "sui_framework".to_string(),
                    Address::from_str("0x2").unwrap()
                ),
                ("workflow".to_string(), Address::from_str("0xaa").unwrap()),
            ]
        );
    }

    #[test]
    fn skips_empty_entries() {
        let parsed = parse_packages("move_std=0x1,,sui_framework=0x2,").expect("valid spec");
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn empty_spec_yields_no_packages() {
        assert!(parse_packages("   ").expect("empty is ok").is_empty());
    }

    #[test]
    fn rejects_entry_without_equals() {
        let err = parse_packages("primitives=0x1,workflow").expect_err("missing `=`");
        assert!(err.contains("malformed package entry `workflow`"), "{err}");
    }

    #[test]
    fn rejects_empty_name() {
        let err = parse_packages("=0x1").expect_err("empty name");
        assert!(err.contains("empty package name"), "{err}");
    }

    #[test]
    fn rejects_invalid_package_id() {
        let err = parse_packages("primitives=not-an-address").expect_err("bad id");
        assert!(err.contains("invalid package id `not-an-address`"), "{err}");
    }

    #[test]
    fn defaults_to_normalized_package_ids() {
        std::env::remove_var("NEXUS_BINDING_NORMALIZE_PACKAGE_IDS");
        assert!(normalize_package_ids_enabled().expect("default parses"));
    }

    #[test]
    fn rejects_invalid_normalize_flag() {
        assert!(
            parse_bool_env("NEXUS_BINDING_NORMALIZE_PACKAGE_IDS", "maybe")
                .expect_err("bad bool")
                .contains("must be 0/1")
        );
    }

    #[test]
    fn package_id_replacements_preserve_framework_packages() {
        let packages = parse_packages(
            "primitives=0x111,interface=0x222,workflow=0x333,move_std=0x1,sui_framework=0x2",
        )
        .expect("valid spec");
        let replacements = package_id_replacements(&packages);

        assert_eq!(replacements.len(), 3);
        assert!(replacements.iter().any(|(actual, placeholder)| {
            *actual == Address::from_str("0x111").unwrap()
                && *placeholder == Address::from_str("0xa1").unwrap()
        }));
        assert!(replacements.iter().any(|(actual, placeholder)| {
            *actual == Address::from_str("0x222").unwrap()
                && *placeholder == Address::from_str("0xa2").unwrap()
        }));
        assert!(replacements.iter().any(|(actual, placeholder)| {
            *actual == Address::from_str("0x333").unwrap()
                && *placeholder == Address::from_str("0xa4").unwrap()
        }));
    }

    #[test]
    fn package_ir_ids_are_added_to_replacements() {
        let mut replacements =
            package_id_replacements(&parse_packages("primitives=0x111").expect("valid spec"));
        let json = r#"{
  "storage_id": "0x222",
  "original_id": "0x333",
  "version": 1
}"#;

        add_package_ir_id_replacements(&mut replacements, "primitives", json);

        assert!(replacements.iter().any(|(actual, placeholder)| {
            *actual == Address::from_str("0x111").unwrap()
                && *placeholder == Address::from_str("0xa1").unwrap()
        }));
        assert!(replacements.iter().any(|(actual, placeholder)| {
            *actual == Address::from_str("0x222").unwrap()
                && *placeholder == Address::from_str("0xa1").unwrap()
        }));
        assert!(replacements.iter().any(|(actual, placeholder)| {
            *actual == Address::from_str("0x333").unwrap()
                && *placeholder == Address::from_str("0xa1").unwrap()
        }));
    }

    #[test]
    fn rejects_unexpected_package_ids_after_normalization() {
        let replacements =
            package_id_replacements(&parse_packages("interface=0x111").expect("valid spec"));
        let json = r#"{
  "storage_id": "0xa2",
  "original_id": "0xa2",
  "modules": {
    "agent": {
      "datatypes": [
        {
          "type_name": {
            "address": "0x999",
            "module": "agent",
            "name": "Agent"
          }
        }
      ]
    }
  }
}"#;

        let err = validate_normalized_package_ids("interface", json, &replacements)
            .expect_err("unexpected package id should fail generation");

        assert!(err.contains("0x999"), "{err}");
    }

    #[test]
    fn normalizes_nested_json_package_ids() {
        let actual = Address::from_str("0x1234").unwrap();
        let placeholder = Address::from_str("0xa4").unwrap();
        let framework = Address::from_str("0x2").unwrap();
        let mut value = format!(
            "{{\n  \"storage_id\": \"{}\",\n  \"nested\": {{\n    \"address\": \"{}\",\n    \"framework\": \"{}\"\n  }}\n}}",
            actual, actual, framework
        );

        normalize_json_package_ids(&mut value, &[(actual, placeholder)]);

        assert!(value.contains("\"storage_id\": \"0xa4\""));
        assert!(value.contains("\"address\": \"0xa4\""));
        assert!(value.contains(&format!("\"framework\": \"{}\"", framework)));
    }

    #[test]
    fn normalizes_compact_actual_package_ids() {
        let actual =
            Address::from_str("0x04cebf181e62e63c3d698b834758ee0443f33f793604063940497a9bc23a7154")
                .unwrap();
        let placeholder = Address::from_str("0xa2").unwrap();
        let mut value = r#"{
  "storage_id": "0x4cebf181e62e63c3d698b834758ee0443f33f793604063940497a9bc23a7154",
  "padded": "0x04cebf181e62e63c3d698b834758ee0443f33f793604063940497a9bc23a7154"
}"#
        .to_string();

        normalize_json_package_ids(&mut value, &[(actual, placeholder)]);

        assert!(value.contains("\"storage_id\": \"0xa2\""));
        assert!(value.contains("\"padded\": \"0xa2\""));
        assert!(!value.contains("4cebf181e62e63c3d698b834758ee0443f33f793604063940497a9bc23a7154"));
    }

    #[test]
    fn parses_move_source_parameter_names() {
        let source = strip_move_comments(
            r#"
module nexus_workflow::execution_settlement;

// fun ignored(arg0: u64) {}
    public fun record_committed_tool_result_gas_charge_by_leader(
    execution: &mut DAGExecution,
    leader_cap: &CloneableOwnerCap<leader_cap::OverNetwork>,
    mut walk_index: u64,
    commit_tx_digest: vector<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) {}
"#,
        );

        assert_eq!(
            parse_move_module_name(&source).as_deref(),
            Some("execution_settlement")
        );
        let functions = parse_move_function_parameter_names(&source);
        assert_eq!(functions.len(), 1);
        assert_eq!(
            functions[0].0,
            "record_committed_tool_result_gas_charge_by_leader"
        );
        assert_eq!(
            functions[0].1,
            vec![
                "execution",
                "leader_cap",
                "walk_index",
                "commit_tx_digest",
                "commit_gas_charge",
                "settlement_gas_charge"
            ]
        );
    }

    #[test]
    fn restores_arg_names_from_move_source() {
        let temp = std::env::temp_dir().join(format!(
            "nexus-binding-source-names-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let source_dir = temp.join("workflow").join("sources");
        std::fs::create_dir_all(&source_dir).expect("create source dir");
        std::fs::write(
            source_dir.join("execution_settlement.move"),
            r#"
module nexus_workflow::execution_settlement;

public fun record_committed_tool_result_gas_charge_by_leader(
    execution: &mut DAGExecution,
    leader_cap: &CloneableOwnerCap<leader_cap::OverNetwork>,
    walk_index: u64,
) {}
"#,
        )
        .expect("write source");

        let mut json = r#"{
  "modules": {
    "execution_settlement": {
      "functions": [
        {
          "name": "record_committed_tool_result_gas_charge_by_leader",
          "parameters": [
            { "name": "arg0" },
            { "name": "arg1" },
            { "name": "arg2" }
          ]
        }
      ]
    }
  }
}"#
        .to_string();

        restore_source_parameter_names(&mut json, "workflow", Some(&temp)).expect("restore names");

        assert!(json.contains("\"name\": \"execution\""));
        assert!(json.contains("\"name\": \"leader_cap\""));
        assert!(json.contains("\"name\": \"walk_index\""));
        assert!(!json.contains("\"name\": \"arg0\""));
        std::fs::remove_dir_all(temp).expect("remove temp dir");
    }
}
