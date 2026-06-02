use super::*;

pub(crate) async fn scaffold_tap_skill(
    name: String,
    target: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Scaffolding TAP skill '{name}' in '{target}'",
        target = target.display()
    );

    let root = target.join(name.to_case(Case::Kebab));
    let package_name = name.to_case(Case::Snake);
    let module_name = package_name.clone();

    let handle = loading!("Writing TAP template files...");
    let files = scaffold_files(&name, &package_name, &module_name);

    create_dir_all(&root).await.map_err(NexusCliError::Io)?;

    for (path, contents) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await.map_err(NexusCliError::Io)?;
        }

        let mut file = File::create(path).await.map_err(NexusCliError::Io)?;
        file.write_all(contents.as_bytes())
            .await
            .map_err(NexusCliError::Io)?;
    }

    handle.success();
    notify_success!("Created TAP skill scaffold at {}", root.display());
    json_output(&json!({ "path": root }))?;

    Ok(())
}

pub(crate) fn scaffold_files(
    name: &str,
    package_name: &str,
    module_name: &str,
) -> Vec<(PathBuf, String)> {
    vec![
        (
            PathBuf::from("tap/Move.toml"),
            format!(
                r#"[package]
name = "{package_name}"
edition = "2024.beta"

[dependencies]
nexus_primitives = {{ local = "../../nexus/sui/primitives" }}
nexus_interface  = {{ local = "../../nexus/sui/interface" }}
nexus_registry   = {{ local = "../../nexus/sui/registry" }}
nexus_workflow   = {{ local = "../../nexus/sui/workflow" }}

[addresses]
{package_name} = "0x0"
"#
            ),
        ),
        (
            PathBuf::from(format!("tap/sources/{module_name}.move")),
            format!(
                r#"module {package_name}::{module_name};

/// Minimal third-party TAP package scaffold.
/// Fill this package with business logic, endpoint state, and standard TAP exports.
public struct {witness} has drop {{}}

public fun init_for_test(): {witness} {{
    {witness} {{}}
}}
"#,
                witness = name.to_case(Case::Pascal)
            ),
        ),
        (
            PathBuf::from("dag.json"),
            r#"{
  "vertices": [
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.weather_skill@1"
      },
      "name": "entry",
      "entry_ports": [
        {
          "name": "input"
        }
      ]
    }
  ],
  "edges": []
}
"#
            .to_string(),
        ),
        (
            PathBuf::from("skill.tap.json"),
            format!(
                r#"{{
  "name": "{name}",
  "tap_package_name": "{package_name}",
  "dag_path": "dag.json",
  "tap_package_path": "tap",
  "requirements": {{
    "input_schema_commitment": [1],
    "workflow_commitment": [1],
    "metadata_commitment": [1],
    "payment_policy": {{
      "mode": "user_funded",
      "max_budget": 0,
      "token_type_commitment": [],
      "refund_mode": 0
    }},
    "schedule_policy": {{
      "recurrence_kind": "once",
      "min_interval_ms": 0,
      "max_occurrences": 1,
      "allow_recursive": false
    }},
    "vertex_authorization_schema": {{
      "schema_commitment": [],
      "fixed_tools": [],
      "requires_payment": false
    }}
  }},
  "shared_objects": [],
  "interface_revision": 1
}}
"#
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_files_use_kebab_snake_and_expected_package_layout() {
        let files = scaffold_files("Weather Skill", "weather_skill", "weather_skill");
        let paths = files.iter().map(|(path, _)| path).collect::<Vec<_>>();

        assert!(paths.contains(&&PathBuf::from("tap/Move.toml")));
        assert!(paths.contains(&&PathBuf::from("tap/sources/weather_skill.move")));
        assert!(paths.contains(&&PathBuf::from("dag.json")));
        assert!(paths.contains(&&PathBuf::from("skill.tap.json")));
        assert!(files
            .iter()
            .any(|(_, contents)| contents.contains("module weather_skill::weather_skill;")));
    }

    #[test]
    fn scaffolded_move_toml_declares_all_four_nexus_dependencies() {
        // The TAP development guide compiles the scaffolded package against
        // the four published Nexus packages: primitives, interface, registry,
        // workflow. A missing entry surfaces as a downstream Move compile
        // error on the first `validate-skill` once the author replaces the
        // stub Move source, so pin the full set up front.
        let files = scaffold_files(
            "tutorial transfer",
            "tutorial_transfer",
            "tutorial_transfer",
        );
        let move_toml = files
            .iter()
            .find_map(|(path, contents)| {
                (path == &PathBuf::from("tap/Move.toml")).then_some(contents.as_str())
            })
            .expect("Move.toml present");
        for dep in [
            "nexus_primitives",
            "nexus_interface",
            "nexus_registry",
            "nexus_workflow",
        ] {
            assert!(
                move_toml.contains(dep),
                "scaffolded Move.toml missing {dep} dependency"
            );
        }
    }
}
